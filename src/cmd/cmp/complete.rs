use deepsize::DeepSizeOf;
use indexmap::{IndexMap, IndexSet};

use crate::cmd::shared::key::Key;
use crate::context::SeqContext;
use crate::error::CliResult;
use crate::helpers::DefaultBuildHasher as BuildHasher;
use crate::io::{input::SeqReader, OwnedRecord, Record};
use crate::var::varstring::VarString;

use super::*;

pub fn cmp_complete(
    cfg: &mut Config,
    var_str: &[VarString],
    out: &mut Output,
    max_mem: usize,
    two_pass: bool,
    quiet: bool,
) -> CliResult<CmpStats> {
    let mut cmp = if !two_pass {
        Cmp::Records(RecordCmp::new(var_str, max_mem))
    } else {
        Cmp::Keys(KeyCmp::new(var_str, max_mem))
    };
    let mut stats = CmpStats::default();
    cfg.read2(|rdr0, rdr1, ctx| {
        while rdr0.read_next(&mut |rec| cmp.advance(rec, false, ctx, quiet))? {}
        while rdr1.read_next(&mut |rec| cmp.advance(rec, true, ctx, quiet))? {}
        if let Cmp::Records(c) = &mut cmp {
            stats = c.write_records(out, ctx)?;
        }
        Ok(())
    })?;
    if let Cmp::Keys(c) = &mut cmp {
        cfg.read2(|rdr0, rdr1, ctx| {
            stats = c.write_records((rdr0, rdr1), out, ctx)?;
            Ok(())
        })?;
    }

    Ok(stats)
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
                    *self = Cmp::Keys(cmp.into_indexed());
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

struct RecordCmp {
    records0: IndexMap<Key, OwnedRecord, BuildHasher>,
    records1: IndexMap<Key, (OwnedRecord, bool), BuildHasher>,
    var_string: Vec<VarString>,
    key: Key,
    text_buf: Vec<Vec<u8>>,
    mem_size: usize,
    max_mem: usize,
    stats: CmpStats,
}

impl RecordCmp {
    fn new(var_string: &[VarString], max_mem: usize) -> Self {
        let key = Key::with_size(var_string.len());
        let text_buf = vec![Vec::new(); var_string.len()];
        Self {
            records0: IndexMap::default(),
            records1: IndexMap::default(),
            var_string: var_string.to_vec(),
            key,
            text_buf,
            mem_size: 0,
            max_mem,
            stats: CmpStats::default(),
        }
    }

    fn add_record(
        &mut self,
        rec: &dyn Record,
        is_second: bool,
        ctx: &mut SeqContext,
    ) -> CliResult<bool> {
        ctx.set_record(rec, 0)?;
        self.key
            .compose_from(&self.var_string, &mut self.text_buf, ctx.symbols(), rec)?;
        let rec = OwnedRecord::from(rec);
        self.mem_size += self.key.deep_size_of() + rec.deep_size_of();
        // println!("{} {} => {:?} {}", is_second, self.key, rec, self.mem_size);
        if !is_second {
            if self.records0.contains_key(&self.key) {
                return fail!(
                    "Duplicate key in input no. {}: {}",
                    (is_second as u8) + 1,
                    self.key
                );
            }
            self.records0.insert(self.key.clone(), rec);
        } else {
            let rec = (rec, false);
            self.records1.insert(self.key.clone(), rec);
            self.mem_size += (false).deep_size_of();
        };
        Ok(self.mem_size <= self.max_mem)
    }

    fn into_indexed(&mut self) -> KeyCmp {
        let mut out = KeyCmp::new(&self.var_string, self.max_mem);
        for (key, _) in self.records0.drain(..) {
            out._insert_key_simple(key, false);
        }
        for (key, _) in self.records1.drain(..) {
            out._insert_key_simple(key, true);
        }
        out
    }

    fn write_records(&mut self, out: &mut Output, ctx: &mut SeqContext) -> CliResult<CmpStats> {
        // compare all records in self.records0 against self.records1
        // TODO: compare drain vs. iter performance
        for (key, rec) in &self.records0 {
            let cat = if let Some((_, is_common)) = self.records1.get_mut(key) {
                *is_common = true;
                self.stats.common += 1;
                Common
            } else {
                self.stats.unique1 += 1;

                Unique1
            };
            write_record(ctx, key, rec, cat, out, false)?;
        }
        // iterate in-order over self.records1, where we already know which elements are common
        for (key, (rec, is_common)) in &self.records1 {
            let cat = if *is_common {
                Common
            } else {
                self.stats.unique2 += 1;
                Unique2
            };
            write_record(ctx, key, rec, cat, out, true)?;
        }
        Ok(self.stats)
    }
}

struct KeyCmp {
    records0: IndexSet<Key, BuildHasher>,
    records1: IndexMap<Key, bool, BuildHasher>,
    var_string: Vec<VarString>,
    key: Key,
    text_buf: Vec<Vec<u8>>,
    mem_size: usize,
    max_mem: usize,
    stats: CmpStats,
}

impl KeyCmp {
    fn new(var_string: &[VarString], max_mem: usize) -> Self {
        let key = Key::with_size(var_string.len());
        let text_buf = vec![Vec::new(); var_string.len()];
        Self {
            records0: IndexSet::default(),
            records1: IndexMap::default(),
            var_string: var_string.to_vec(),
            key,
            text_buf,
            mem_size: 0,
            max_mem,
            stats: CmpStats::default(),
        }
    }

    fn add_record(
        &mut self,
        rec: &dyn Record,
        is_second: bool,
        ctx: &mut SeqContext,
    ) -> CliResult<bool> {
        ctx.set_record(rec, 0)?;
        self.key
            .compose_from(&self.var_string, &mut self.text_buf, ctx.symbols(), rec)?;
        self.mem_size += self.key.deep_size_of();
        if !is_second {
            if self.records0.contains(&self.key) {
                // TODO: duplicate code
                return fail!("Duplicate key in input no. 1: {}", self.key);
            }
            self.records0.insert(self.key.clone());
        } else {
            if self.records1.contains_key(&self.key) {
                return fail!("Duplicate key in input no. 2: {}", self.key);
            }

            self.records1.insert(self.key.clone(), false);
            self.mem_size += false.deep_size_of();
        }
        Ok(self.mem_size <= self.max_mem)
    }

    fn _insert_key_simple(&mut self, key: Key, is_second: bool) {
        self.mem_size += key.deep_size_of();
        if !is_second {
            self.records0.insert(key);
        } else {
            self.records1.insert(key, false);
            self.mem_size += false.deep_size_of();
        }
    }

    fn write_records(
        &mut self,
        rdr: (&mut dyn SeqReader, &mut dyn SeqReader),
        out: &mut Output,
        ctx: &mut SeqContext,
    ) -> CliResult<CmpStats> {
        // compare all records in self.records0 against self.records1
        // TODO: compare drain vs. iter performance
        for key in &self.records0 {
            let cat = if let Some(is_common) = self.records1.get_mut(key) {
                *is_common = true;
                self.stats.common += 1;
                Common
            } else {
                self.stats.unique1 += 1;
                Unique1
            };
            // println!("key1 {} cat {:?}", key, cat);
            if !rdr
                .0
                .read_next(&mut |rec| write_record(ctx, &key, rec, cat, out, false))?
            {
                // TODO: not nice
                return fail!("First input has less records than before");
            }
        }
        if rdr.0.read_next(&mut |_| Ok(()))? {
            return fail!("First input has more records than before");
        }
        // iterate in-order over self.records1, where we already know which elements are common
        for (key, is_common) in &self.records1 {
            let cat = if *is_common {
                self.stats.common += 1;
                Common
            } else {
                self.stats.unique2 += 1;

                Unique2
            };
            // println!("key2 {} cat {:?}", key, cat);
            if !rdr
                .1
                .read_next(&mut |rec| write_record(ctx, key, rec, cat, out, true))?
            {
                return fail!("Second input has less records than before");
            }
        }
        if rdr.1.read_next(&mut |_| Ok(()))? {
            return fail!("Second input has more records than before");
        }
        Ok(self.stats)
    }
}

fn write_record(
    ctx: &mut SeqContext,
    key: &Key,
    rec: &dyn Record,
    cat: Category,
    out: &mut Output,
    is_second: bool,
) -> CliResult<()> {
    ctx.set_record(rec, 0)?;
    ctx.with_custom_varmod(0, |m: &mut CmpVars, sym| m.set(key, cat, sym));
    out.write_record(rec, &ctx.meta[0], cat, is_second, &mut ctx.qual_converter)
}
