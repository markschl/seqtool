use std::fmt;

use deepsize::DeepSizeOf;
use indexmap::{map::Iter as IndexMapIter, IndexMap, IndexSet};

use crate::cmd::shared::key::Key;
use crate::context::SeqContext;
use crate::error::{CliError, CliResult};
use crate::helpers::DefaultBuildHasher as BuildHasher;
use crate::io::{input::SeqReader, OwnedRecord, Record};
use crate::var::varstring::VarString;

use super::*;

pub fn cmp_complete(
    cfg: &mut Config,
    var_str: Vec<VarString>,
    out: &mut Output,
    diff_fields: Option<Vec<VarString>>,
    max_mem: usize,
    two_pass: bool,
    quiet: bool,
) -> CliResult<CmpStats> {
    let mut cmp = if !two_pass {
        Cmp::Records(RecordCmp::new(var_str, diff_fields.is_some(), max_mem))
    } else {
        Cmp::Keys(KeyCmp::new(var_str, diff_fields.is_some(), max_mem))
    };
    let mut stats = CmpStats::default();
    let mut diff_writer = diff_fields.map(|fields| DiffWriter::new(fields, 80));
    cfg.read2(|rdr0, rdr1, ctx| {
        while rdr0.read_next(&mut |rec| cmp.advance(rec, false, ctx, quiet))? {}
        while rdr1.read_next(&mut |rec| cmp.advance(rec, true, ctx, quiet))? {}
        if let Cmp::Records(c) = &mut cmp {
            stats = c.write_records(out, diff_writer.as_mut(), ctx)?;
        }
        Ok(())
    })?;
    if let Cmp::Keys(c) = &mut cmp {
        cfg.read2(|rdr0, rdr1, ctx| {
            stats = c.write_records((rdr0, rdr1), out, diff_writer.as_mut(), ctx)?;
            Ok(())
        })?;
    }
    Ok(stats)
}

#[derive(Debug)]
enum Records1Map<R> {
    Simple(IndexMap<Key, (bool, R), BuildHasher>),
    WithDiffFields0(IndexMap<Key, (bool, R, Vec<Vec<u8>>), BuildHasher>),
}

impl<R> Records1Map<R> {
    fn new(store_diff_key: bool) -> Self {
        if !store_diff_key {
            Self::Simple(IndexMap::default())
        } else {
            Self::WithDiffFields0(IndexMap::default())
        }
    }

    /// Inserts a key/record combination; returns `true` if key is already present
    fn insert(&mut self, key: Key, rec: R) -> (bool, usize) {
        match self {
            Records1Map::Simple(m) => (m.insert(key, (false, rec)).is_some(), false.deep_size_of()),
            Records1Map::WithDiffFields0(m) => {
                (
                    m.insert(key, (false, rec, Vec::new())).is_some(),
                    // TODO: machine code?
                    (false, Vec::<Vec<u8>>::new()).deep_size_of(),
                )
            }
        }
    }

    fn lookup(&mut self, key: &Key) -> Option<Option<&mut Vec<Vec<u8>>>> {
        match self {
            Records1Map::Simple(m) => m.get_mut(key).map(|(common, _)| {
                *common = true;
                None
            }),
            Records1Map::WithDiffFields0(m) => m.get_mut(key).map(|(common, _, fields)| {
                *common = true;
                Some(fields)
            }),
        }
    }

    fn iter(&self) -> Records1MapIter<R> {
        match self {
            Records1Map::Simple(m) => Records1MapIter::Simple(m.iter()),
            Records1Map::WithDiffFields0(m) => Records1MapIter::WithDiffFields0(m.iter()),
        }
    }
}

enum Records1MapIter<'a, R> {
    Simple(IndexMapIter<'a, Key, (bool, R)>),
    WithDiffFields0(IndexMapIter<'a, Key, (bool, R, Vec<Vec<u8>>)>),
}

impl<'a, R> Iterator for Records1MapIter<'a, R> {
    type Item = (&'a Key, bool, &'a R, Option<&'a [Vec<u8>]>);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Records1MapIter::Simple(m) => m.next().map(|(k, (common, r))| (k, *common, r, None)),
            Records1MapIter::WithDiffFields0(m) => m
                .next()
                .map(|(k, (common, r, f))| (k, *common, r, Some(f.as_slice()))),
        }
    }
}

impl Records1Map<OwnedRecord> {
    fn drop_records(&mut self) -> Records1Map<()> {
        match self {
            Records1Map::Simple(m) => {
                Records1Map::Simple(m.drain(..).map(|(k, (c, _))| (k, (c, ()))).collect())
            }
            Records1Map::WithDiffFields0(m) => Records1Map::WithDiffFields0(
                m.drain(..).map(|(k, (c, _, d))| (k, (c, (), d))).collect(),
            ),
        }
    }
}

enum Cmp {
    Records(RecordCmp),
    Keys(KeyCmp),
}

impl Cmp {
    fn advance(
        &mut self,
        rec: &dyn Record,
        is_second: bool,
        ctx: &mut SeqContext,
        quiet: bool,
    ) -> CliResult<()> {
        // println!("add rec {}", std::str::from_utf8(rec.id()).unwrap());
        match self {
            Cmp::Records(cmp) => {
                if cmp.add_record(rec, is_second, ctx)? {
                    return Ok(());
                } else {
                    if !quiet {
                        eprintln!(
                            "Memory limit exceeded, switching to two-pass mode. \
                        Consider raising the limit (-M/--max-mem) to speed up 'cmp'. \
                        Use -q/--quiet to silence this message."
                        );
                    }
                    *self = Cmp::Keys(cmp.get_indexed());
                }
            }
            Cmp::Keys(cmp) => {
                if !cmp.add_record(rec, is_second, ctx)? {
                    return fail!("Memory limit exceeded, consider raising with `-M/--max-mem`");
                }
            }
        }
        Ok(())
    }
}

/// Stores owned versions of all records of the two streams in memory
/// before doing the comparison and writing the output
struct RecordCmp {
    records0: IndexMap<Key, OwnedRecord, BuildHasher>,
    records1: Records1Map<OwnedRecord>,
    key: KeyHelper,
    mem_size: usize,
    max_mem: usize,
}

impl RecordCmp {
    fn new(var_string: Vec<VarString>, store_diff_key: bool, max_mem: usize) -> Self {
        Self {
            records0: IndexMap::default(),
            records1: Records1Map::new(store_diff_key),
            key: KeyHelper::new(var_string),
            mem_size: 0,
            max_mem,
        }
    }

    /// Adds a record to one of the two maps (depending on `is_second`).
    /// Returns `false` if the memory limit is exceeded
    fn add_record(
        &mut self,
        rec: &dyn Record,
        is_second: bool,
        ctx: &mut SeqContext,
    ) -> CliResult<bool> {
        self.mem_size += self.key.compose(rec, ctx, 0)?;
        let rec = OwnedRecord::from(rec);
        self.mem_size += rec.deep_size_of();
        // println!("{} {} => {:?} {}", is_second, self.key, rec, self.mem_size);
        let (num, has_key) = if !is_second {
            (
                1,
                self.records0
                    .insert(self.key.inner().clone(), rec)
                    .is_some(),
            )
        } else {
            let (exists, size) = self.records1.insert(self.key.inner().clone(), rec);
            self.mem_size += size;
            (2, exists)
        };
        if has_key {
            return fail!("Duplicate key in input no. {}: {}", num, self.key.inner());
        }
        Ok(self.mem_size <= self.max_mem)
    }

    fn get_indexed(&mut self) -> KeyCmp {
        let records0: IndexSet<Key, _> = self.records0.drain(..).map(|(k, _)| k).collect();
        let records1 = self.records1.drop_records();
        let mut mem_size = records0.iter().fold(0, |s, k| s + k.deep_size_of());
        mem_size += records1.iter().fold(0, |s, item| s + item.deep_size_of());
        KeyCmp {
            records0,
            records1,
            key: self.key.clone(),
            mem_size,
            max_mem: self.max_mem,
        }
    }

    fn write_records(
        &mut self,
        out: &mut Output,
        mut diff_writer: Option<&mut DiffWriter>,
        ctx: &mut SeqContext,
    ) -> CliResult<CmpStats> {
        // compare all records in self.records0 against self.records1
        let mut stats = CmpStats::default();
        for (key, rec) in &self.records0 {
            let (cat, diff_fields0) = if let Some(diff_fields0) = self.records1.lookup(key) {
                stats.common += 1;
                (Common, diff_fields0)
            } else {
                stats.unique1 += 1;
                (Unique1, None)
            };
            set_vars(self.key.inner(), rec, cat, 0, ctx)?;
            out.write_record(rec, &ctx.meta[0], cat, false, &mut ctx.qual_converter)?;
            if let Some(f) = diff_fields0 {
                diff_writer
                    .as_mut()
                    .unwrap()
                    .compose_fields(rec, &ctx.meta[0], f)?;
            }
        }
        // iterate in-order over self.records1, where we already know which elements are common
        let mut diff_fields1 = Vec::new();
        for (key, is_common, rec, diff_fields0) in self.records1.iter() {
            let (cat, diff_fields0) = if is_common {
                (Common, diff_fields0)
            } else {
                stats.unique2 += 1;
                (Unique2, None)
            };
            set_vars(key, rec, cat, 0, ctx)?;
            out.write_record(rec, &ctx.meta[0], cat, true, &mut ctx.qual_converter)?;
            if let Some(f) = diff_fields0 {
                let w = diff_writer.as_mut().unwrap();
                w.compose_fields(rec, &ctx.meta[0], &mut diff_fields1)?;
                w.write_comparison(key, f, &diff_fields1)?;
            }
        }
        Ok(stats)
    }
}

struct InputChanged(u8);

impl fmt::Display for InputChanged {
    #[cold]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Input no. {} changed when reading the second time",
            self.0
        )
    }
}
from_err!(InputChanged);

#[derive(Debug)]
struct KeyCmp {
    records0: IndexSet<Key, BuildHasher>,
    records1: Records1Map<()>,
    key: KeyHelper,
    mem_size: usize,
    max_mem: usize,
}

impl KeyCmp {
    fn new(var_string: Vec<VarString>, store_diff_key: bool, max_mem: usize) -> Self {
        Self {
            records0: IndexSet::default(),
            records1: Records1Map::new(store_diff_key),
            key: KeyHelper::new(var_string),
            mem_size: 0,
            max_mem,
        }
    }

    // **Note**: this code is almost identical to `RecordCmp::add_record`
    fn add_record(
        &mut self,
        rec: &dyn Record,
        is_second: bool,
        ctx: &mut SeqContext,
    ) -> CliResult<bool> {
        self.mem_size += self.key.compose(rec, ctx, 0)?;
        // println!("{} {} => {:?} {}", is_second, self.key, rec, self.mem_size);
        let (num, has_key) = if !is_second {
            (1, !self.records0.insert(self.key.inner().clone()))
        } else {
            let (exists, size) = self.records1.insert(self.key.inner().clone(), ());
            self.mem_size += size;
            (2, exists)
        };
        if has_key {
            return fail!("Duplicate key in input no. {}: {}", num, self.key.inner());
        }
        Ok(self.mem_size <= self.max_mem)
    }

    fn write_records(
        &mut self,
        rdr: (&mut dyn SeqReader, &mut dyn SeqReader),
        out: &mut Output,
        mut diff_writer: Option<&mut DiffWriter>,
        ctx: &mut SeqContext,
    ) -> CliResult<CmpStats> {
        let mut stats = CmpStats::default();
        // First, compare all records in self.records0 against self.records1.
        // Records1Map::lookup() flags any common records
        for key in &self.records0 {
            let (cat, mut diff_fields0) = if let Some(diff_fields0) = self.records1.lookup(key) {
                stats.common += 1;
                (Common, diff_fields0)
            } else {
                stats.unique1 += 1;
                (Unique1, None)
            };
            // println!("key1 {} cat {:?}", key, cat);
            // TODO: not validating that records are in sync
            let has_record = rdr.0.read_next(&mut |rec| {
                set_vars(key, rec, cat, 0, ctx)?;
                out.write_record(rec, &ctx.meta[0], cat, false, &mut ctx.qual_converter)?;
                if let Some(f) = diff_fields0.as_mut() {
                    diff_writer
                        .as_mut()
                        .unwrap()
                        .compose_fields(rec, &ctx.meta[0], f)?;
                }
                Ok(())
            })?;
            if !has_record {
                return fail!(InputChanged(1));
            }
        }

        if rdr.0.read_next(&mut |_| Ok(()))? {
            return fail!(InputChanged(1));
        }

        // iterate in-order over self.records1, where we already know which elements are common
        let mut diff_fields1 = Vec::new();
        for (key, is_common, _, diff_fields0) in self.records1.iter() {
            let (cat, diff_fields0) = if is_common {
                stats.common += 1;
                (Common, diff_fields0)
            } else {
                stats.unique2 += 1;
                (Unique2, None)
            };
            // println!("key2 {} cat {:?}", key, cat);
            let has_record = rdr.1.read_next(&mut |rec| {
                set_vars(key, rec, cat, 0, ctx)?;
                out.write_record(rec, &ctx.meta[0], cat, true, &mut ctx.qual_converter)?;
                if let Some(f) = diff_fields0 {
                    let w = diff_writer.as_mut().unwrap();
                    w.compose_fields(rec, &ctx.meta[0], &mut diff_fields1)?;
                    w.write_comparison(key, f, &diff_fields1)?;
                }
                Ok(())
            })?;
            if !has_record {
                return fail!(InputChanged(2));
            }
        }
        if rdr.1.read_next(&mut |_| Ok(()))? {
            return fail!(InputChanged(2));
        }
        Ok(stats)
    }
}

#[derive(Debug, Clone)]
struct KeyHelper {
    var_string: Vec<VarString>,
    key: Key,
    key_buf: Vec<Vec<u8>>,
}

impl KeyHelper {
    fn new(vs: Vec<VarString>) -> Self {
        Self {
            key: Key::with_size(vs.len()),
            key_buf: vec![Vec::new(); vs.len()],
            var_string: vs,
        }
    }

    fn inner(&self) -> &Key {
        &self.key
    }

    fn compose(
        &mut self,
        rec: &dyn Record,
        ctx: &mut SeqContext,
        meta_slot: usize,
    ) -> CliResult<usize> {
        ctx.set_record(rec, meta_slot)?;
        self.key
            .compose_from(&self.var_string, &mut self.key_buf, ctx.symbols(), rec)?;
        Ok(self.key.deep_size_of())
    }
}

fn set_vars(
    key: &Key,
    rec: &dyn Record,
    cat: Category,
    meta_slot: usize,
    ctx: &mut SeqContext,
) -> CliResult<()> {
    ctx.set_record(rec, meta_slot)?;
    ctx.with_custom_varmod(meta_slot, |m: &mut CmpVars, sym| m.set(key, cat, sym));
    Ok(())
}
