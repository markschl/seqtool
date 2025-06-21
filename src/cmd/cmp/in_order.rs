use deepsize::DeepSizeOf;
use ringmap::RingMap;

use crate::cmd::shared::key::Key;
use crate::context::SeqContext;
use crate::error::CliResult;
use crate::helpers::DefaultBuildHasher as BuildHasher;
use crate::io::input::SeqReader;
use crate::io::{OwnedRecord, Record};
use crate::var::varstring::VarString;

use super::*;

pub fn cmp_in_order(
    cfg: &mut Config,
    var_key: &[VarString],
    out: &mut Output,
    max_mem: usize,
) -> CliResult<CmpStats> {
    let mut stats = CmpStats::default();
    cfg.require_meta_slots(3);
    cfg.read2(|rdr0, rdr1, ctx| {
        let mut cmp = in_order::OrderedCmp::new(ctx, var_key, out, max_mem);
        // buffers storing non-matching OwnedRecord instances
        let mut buf0 = RingMap::default();
        let mut buf1 = RingMap::default();
        let mut final_direction = None;
        loop {
            // choose stream from which to read from (will be 'rdr1').
            // that is, the stream whose buffer of non-matching records has fewer entries
            // (or, after one of the streams is exhausted, we have to finish the second one)
            let in_order = final_direction.unwrap_or_else(|| buf0.len() <= buf1.len());
            let exhausted = if in_order {
                !cmp.advance((rdr0, rdr1), (&mut buf0, &mut buf1), false)?
            } else {
                !cmp.advance((rdr1, rdr0), (&mut buf1, &mut buf0), true)?
            };
            if exhausted {
                if final_direction.is_none() {
                    final_direction = Some(!in_order);
                } else {
                    break;
                }
            }
        }
        stats = cmp.stats;
        Ok(())
    })?;
    Ok(stats)
}

pub type RecordMap = RingMap<Key, (OwnedRecord, usize), BuildHasher>;

pub struct OrderedCmp<'a> {
    ctx: &'a mut SeqContext,
    var_string: &'a [VarString],
    key: [Key; 2],
    text_buf: Vec<Vec<u8>>,
    out: &'a mut Output,
    mem_size: usize,
    max_mem: usize,
    stats: CmpStats,
}

impl<'a> OrderedCmp<'a> {
    pub fn new(
        ctx: &'a mut SeqContext,
        var_key: &'a [VarString],
        out: &'a mut Output,
        max_mem: usize,
    ) -> Self {
        let key_buf = [Key::with_size(var_key.len()), Key::with_size(var_key.len())];
        let text_buf = vec![Vec::new(); var_key.len()];
        Self {
            ctx,
            var_string: var_key,
            key: key_buf,
            text_buf,
            out,
            mem_size: 0,
            max_mem,
            stats: CmpStats::default(),
        }
    }

    /// Reads at most 1 record from each stream in `rdr`, compares them by key,
    /// and writes the result to the output files.
    /// Nonmatching records are added to the corresponding buffers.
    /// `rev` indicates that the streams are in reverse order (important when writing to output).
    /// The stream with the smaller corresponding buffer is meant to be provided first,
    /// as it *may* be more likely to have a matching record in stream 2.
    ///
    /// Returns `false` when stream 1 is exhausted.
    /// The caller should then call `advance()` in exchanged order until the other
    /// stream is exhausted as well.
    pub fn advance(
        &mut self,
        rdr: (&mut dyn SeqReader, &mut dyn SeqReader),
        buf: (&mut RecordMap, &mut RecordMap),
        rev: bool,
    ) -> CliResult<bool> {
        // flags passed on to write functions, indicating whether
        // it is the first (false) or second (true) stream
        let (s0, s1) = (rev, !rev);
        // eprintln!("=> new round {} || buf0: {} || buf1: {}", rev, buf.0.keys().join(", "), buf.1.keys().join(", "));
        // read new record from stream 0
        if !rdr.0.read_next(&mut |rec0| {
            // calculate the key
            self.set_record(rec0, 0, false)?;
            // eprintln!("read0: {} -> key {}", std::str::from_utf8(rec0.id()).unwrap(), &self.key[0]);
            // check if the key is present in the buffer of stream 1
            if let Some(i1) = buf.1.get_index_of(&self.key[0]) {
                // eprintln!("...in buf1: {}", &self.key[0]);
                // identical key found in buffer of stream 1
                // -> close a previous "gap" by reporting all records
                //    in the buffers up to the matching records as unique
                self.buf_drain(buf.0, None, s0)?;
                self.buf_drain(buf.1, Some(i1), s1)?;
                // then report the common elements
                self.write(0, s0, rec0, Common, false, true)?;
                let (_k, rec1) = self.buf_pop_front(buf.1);
                debug_assert_eq!(_k, self.key[0]);
                self.write(0, s1, &rec1, Common, true, false)?;
                self.stats.common += 1;
            } else
            // read new record from stream 1
            if !rdr.1.read_next(&mut |rec1| {
                // copy its metadata to `data2` (not ctx.record_data) and the key to self.key[1]
                self.set_record(rec1, 1, true)?;
                // eprintln!("read1: {} -> key {}", std::str::from_utf8(rec1.id()).unwrap(), &self.key[1]);
                if self.key[0] == self.key[1] {
                    // eprintln!("...equal {} == {}", std::str::from_utf8(rec0.id()).unwrap(), std::str::from_utf8(rec1.id()).unwrap());
                    // the two newly read records have the same key
                    // -> we don't need to store anything in the buffer, simply report the records
                    self.buf_drain(buf.0, None, s0)?;
                    self.buf_drain(buf.1, None, s1)?;
                    self.write(0, s0, rec0, Common, false, true)?;
                    self.write(1, s1, rec1, Common, false, true)?;
                    self.stats.common += 1;
                } else if let Some(i0) = buf.0.get_index_of(&self.key[1]) {
                    // eprintln!("...in buf0: {}", &self.key[1]);
                    // identical key found in buffer of 1st stream
                    // -> first, save the current record 0 in the buffer
                    self.buf_insert(self.key[0].clone(), rec0, buf.0)?;
                    // then, report unique elements
                    self.buf_drain(buf.0, Some(i0), s0)?;
                    self.buf_drain(buf.1, None, s1)?;
                    // obtain and write stored common record
                    let (_k, rec0) = self.buf_pop_front(buf.0);
                    debug_assert_eq!(_k, self.key[1]);
                    self.write(0, s0, &rec0, Common, true, true)?;
                    self.write(1, s1, rec1, Common, false, true)?;
                    self.stats.common += 1;
                } else {
                    // eprintln!("...not equal {} != {}, {} != {}", self.key[0], self.key[1], std::str::from_utf8(rec0.id()).unwrap(), std::str::from_utf8(rec1.id()).unwrap());
                    // none of the records match -> store them in the buffers
                    self.buf_insert(self.key[0].clone(), rec0, buf.0)?;
                    self.buf_insert(self.key[1].clone(), rec1, buf.1)?;
                }
                Ok(())
            })? {
                // eprintln!("- stream 1 exhausted {} || buf0: {} || buf1: {}", rev,  buf.0.keys().join(", "), buf.1.keys().join(", ");
                // stream 1 exhausted
                // -> report the current record 0 + contents of buffer 0 as unique
                // (we now know they are for sure not in stream 1)
                self.buf_insert(self.key[0].clone(), rec0, buf.0)?;
                self.buf_drain(buf.0, None, s0)?;
                // stream 0 and buffer 1 are both not necessarily exhausted
            }
            Ok(())
        })? {
            // eprintln!("- stream 0 exhausted {} || buf0: {} || buf1: {}", rev,  buf.0.keys().join(", "), buf.1.keys().join(", "));
            // stream 0 exhausted
            // -> report all elements in the buffer 1 as unique
            self.buf_drain(buf.1, None, s1)?;
            // stream 1 and buffer 0 are not necessarily exhausted;
            // the remaining calls to `advance()` need to be done with exchanged streams
            // until `false` is returned a second time.
            return Ok(false);
        }
        Ok(true)
    }

    /// Updates self.ctx.record_data given a sequence record, and composes the comparison key no. 1
    fn set_record(&mut self, rec: &dyn Record, meta_slot: usize, is_second: bool) -> CliResult<()> {
        self.ctx.set_record(rec, meta_slot)?;
        self.key[is_second as usize].compose_from(
            self.var_string,
            &mut self.text_buf,
            &self.ctx.meta[meta_slot].symbols,
            &rec,
        )?;
        Ok(())
    }

    fn buf_insert(
        &mut self,
        key: Key,
        rec: &dyn Record,
        buf: &mut RingMap<Key, (OwnedRecord, usize), BuildHasher>,
    ) -> CliResult<()> {
        let rec = OwnedRecord::from(rec);
        let size = key.deep_size_of() + rec.deep_size_of();
        buf.insert(key, (rec, size));
        self.mem_size += size;
        if size > self.max_mem {
            return fail!("Memory limit exceeded, consider raising with `-M/--max-mem`");
        }
        Ok(())
    }

    fn buf_pop_front(
        &mut self,
        buf: &mut RingMap<Key, (OwnedRecord, usize), BuildHasher>,
    ) -> (Key, OwnedRecord) {
        let (key, (rec, size)) = buf.pop_front().unwrap();
        self.mem_size -= size;
        (key, rec)
    }

    /// Reports (and removes) all unique records up to the given end index in the buffer
    /// (`None` = remove all records)
    fn buf_drain(
        &mut self,
        buf: &mut RingMap<Key, (OwnedRecord, usize), BuildHasher>,
        end: Option<usize>,
        second: bool,
    ) -> CliResult<()> {
        let (cat, counter) = if !second {
            (Unique1, &mut self.stats.unique1)
        } else {
            (Unique2, &mut self.stats.unique2)
        };
        for (key, (rec, size)) in buf.drain(..end.unwrap_or(buf.len())) {
            *counter += 1;
            self.mem_size -= size;
            self.ctx.set_record(&rec, 2)?;
            self.ctx
                .with_custom_varmod(2, |m: &mut CmpVars, sym| m.set(&key, cat, sym));
            self.out.write_record(
                &rec,
                &self.ctx.meta[2],
                cat,
                second,
                &mut self.ctx.qual_converter,
            )?;
        }
        Ok(())
    }

    /// Helper function for writing a single record
    fn write(
        &mut self,
        meta_slot: usize,
        is_second: bool,
        rec: &dyn Record,
        cat: Category,
        update_symbols: bool,
        set_vars: bool,
    ) -> CliResult<()> {
        if update_symbols {
            self.ctx.set_record(rec, meta_slot)?;
        }
        if set_vars {
            let key = &self.key[is_second as usize];
            self.ctx
                .with_custom_varmod(meta_slot, |m: &mut CmpVars, sym| m.set(key, cat, sym));
        }
        self.out.write_record(
            rec,
            &self.ctx.meta[meta_slot],
            cat,
            is_second,
            &mut self.ctx.qual_converter,
        )
    }
}
