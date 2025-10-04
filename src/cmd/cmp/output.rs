use std::io::{self, stdout, StdoutLock, Write};

use bio::alignment::{
    pairwise::{Aligner, MatchParams, Scoring},
    AlignmentOperation,
};
use crossterm::{
    execute,
    style::{Color, ResetColor, SetForegroundColor},
};

use crate::cmd::shared::key::Key;
use crate::config::Config;
use crate::context::RecordMeta;
use crate::error::CliResult;
use crate::io::{
    output::{SeqFormatter, WriteFinish},
    QualConverter, Record,
};
use crate::var::varstring::VarString;

use super::*;

pub struct Output {
    output: Option<(Box<dyn WriteFinish>, Box<dyn SeqFormatter>)>,
    output2: Option<(Box<dyn WriteFinish>, Box<dyn SeqFormatter>)>,
    common: Option<(Box<dyn WriteFinish>, Box<dyn SeqFormatter>)>,
    common2: Option<(Box<dyn WriteFinish>, Box<dyn SeqFormatter>)>,
    unique1: Option<(Box<dyn WriteFinish>, Box<dyn SeqFormatter>)>,
    unique2: Option<(Box<dyn WriteFinish>, Box<dyn SeqFormatter>)>,
}

impl Output {
    pub fn from_args(args: &mut CmpCommand, cfg: &mut Config) -> CliResult<Self> {
        let out = Self {
            output: cfg
                .output_config
                .kind
                .clone()
                .map(|kind| cfg.new_output(kind))
                .transpose()?,
            output2: args
                .output2
                .take()
                .map(|kind| cfg.new_output(kind))
                .transpose()?,
            common: args
                .common_
                .take()
                .map(|kind| cfg.new_output(kind))
                .transpose()?,
            common2: args
                .common2
                .take()
                .map(|kind| cfg.new_output(kind))
                .transpose()?,
            unique1: args
                .unique1
                .take()
                .map(|kind| cfg.new_output(kind))
                .transpose()?,
            unique2: args
                .unique2
                .take()
                .map(|kind| cfg.new_output(kind))
                .transpose()?,
        };
        Ok(out)
    }

    pub fn has_combined_output(&self) -> bool {
        self.output.is_some() || self.output2.is_some()
    }

    pub fn write_record(
        &mut self,
        rec: &dyn Record,
        data: &RecordMeta,
        cat: Category,
        is_second: bool,
        qc: &mut QualConverter,
    ) -> CliResult<()> {
        // println!("write {} {}, {:?} ", is_second, std::str::from_utf8(rec.id()).unwrap(), cat);
        macro_rules! write_if_some {
            ($out:ident) => {
                if let Some((io_writer, fmt_writer)) = self.$out.as_mut() {
                    fmt_writer.write_with(rec, data, io_writer, qc)?;
                }
            };
        }
        if !is_second {
            write_if_some!(output);
        } else {
            write_if_some!(output2);
        }
        match cat {
            Common => {
                if !is_second {
                    write_if_some!(common);
                } else {
                    write_if_some!(common2);
                }
            }
            Unique1 => {
                debug_assert!(!is_second);
                write_if_some!(unique1);
            }
            Unique2 => {
                debug_assert!(is_second);
                write_if_some!(unique2);
            }
        }
        Ok(())
    }
}

pub struct DiffWriter {
    fields: Vec<VarString>,
    aligner: Aligner<MatchParams>,
    line_writer: DiffLineWriter,
}

impl DiffWriter {
    pub fn new(fields: Vec<VarString>, max_width: usize) -> Self {
        Self {
            fields,
            aligner: Aligner::with_scoring(Scoring {
                gap_open: -6,
                gap_extend: -1,
                match_fn: MatchParams::new(1, -1),
                match_scores: None,
                xclip_prefix: -1,
                xclip_suffix: -1,
                yclip_prefix: -1,
                yclip_suffix: -1,
            }),
            line_writer: DiffLineWriter::new(max_width),
        }
    }

    pub fn compose_fields(
        &mut self,
        rec: &dyn Record,
        data: &RecordMeta,
        out: &mut Vec<Vec<u8>>,
    ) -> CliResult<()> {
        out.resize(self.fields.len(), Vec::new());
        for (vs, f) in self.fields.iter().zip(out) {
            f.clear();
            vs.compose(f, &data.symbols, rec)?;
        }
        Ok(())
    }

    pub fn write_comparison(
        &mut self,
        key: &Key,
        fields0: &[Vec<u8>],
        fields1: &[Vec<u8>],
    ) -> CliResult<()> {
        if fields0 == fields1 {
            return Ok(());
        }

        self.line_writer.write_key(key)?;
        let gap_char = '.';

        // debug_assert!(is_second ^ _is_second);
        debug_assert_eq!(fields0.len(), fields1.len());
        for (f0, f1) in fields0.iter().zip(fields1) {
            self.line_writer
                .add_char('┌', '└', DiffCharCategory::Separator)?;
            let aln = self.aligner.global(f0, f1);
            // println!("{:?}", aln);
            // println!("{:?}\n{}\n\n", key, aln.pretty(f0, f1, 80));
            let mut i0 = aln.xstart;
            let mut i1 = aln.ystart;
            for op in &aln.operations {
                use AlignmentOperation::*;
                match op {
                    Match => {
                        self.line_writer.add_char(
                            f0[i0] as char,
                            f1[i1] as char,
                            DiffCharCategory::Match,
                        )?;
                        i0 += 1;
                        i1 += 1;
                    }
                    Subst => {
                        self.line_writer.add_char(
                            f0[i0] as char,
                            f1[i1] as char,
                            DiffCharCategory::Mismatch,
                        )?;
                        i0 += 1;
                        i1 += 1;
                    }
                    Del => {
                        self.line_writer.add_char(
                            gap_char,
                            f1[i1] as char,
                            DiffCharCategory::Mismatch,
                        )?;
                        i1 += 1;
                    }
                    Ins => {
                        self.line_writer.add_char(
                            f0[i0] as char,
                            gap_char,
                            DiffCharCategory::Mismatch,
                        )?;
                        i0 += 1;
                    }
                    Xclip(n) => {
                        for _ in 0..*n {
                            self.line_writer.add_char(
                                f0[i0] as char,
                                gap_char,
                                DiffCharCategory::Mismatch,
                            )?;
                            i0 += 1;
                        }
                    }
                    Yclip(n) => {
                        for _ in 0..*n {
                            self.line_writer.add_char(
                                gap_char,
                                f1[i1] as char,
                                DiffCharCategory::Mismatch,
                            )?;
                            i1 += 1;
                        }
                    }
                }
            }
            debug_assert!(i0 == f0.len() && i1 == f1.len());
            self.line_writer
                .add_char('┐', '┘', DiffCharCategory::Separator)?;
        }
        self.line_writer.finish()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DiffCharCategory {
    Match,
    Mismatch,
    Separator,
}

impl DiffCharCategory {
    fn get_color(self) -> Option<[Color; 2]> {
        match self {
            Self::Match => None,
            Self::Mismatch => Some([Color::Red, Color::Cyan]),
            Self::Separator => Some([Color::Magenta, Color::Magenta]),
        }
    }
}

struct DiffLineWriter {
    max_width: usize,
    line_buf: Vec<([char; 2], DiffCharCategory)>,
    all_match: bool,
    inner: StdoutLock<'static>,
}

impl DiffLineWriter {
    fn new(max_width: usize) -> Self {
        Self {
            max_width,
            line_buf: Vec::new(),
            all_match: false,
            inner: stdout().lock(),
        }
    }

    fn write_key(&mut self, key: &Key) -> io::Result<()> {
        execute!(self.inner, ResetColor)?;
        key.write_delimited(&mut self.inner, b",")?;
        writeln!(self.inner, ":")
    }

    fn add_char(&mut self, c1: char, c2: char, category: DiffCharCategory) -> io::Result<()> {
        self.line_buf.push(([c1, c2], category));
        if self.line_buf.len() == self.max_width {
            self.write_line()?;
        }
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        if !self.line_buf.is_empty() {
            self.write_line()?;
        }
        writeln!(self.inner)
    }

    fn write_line(&mut self) -> io::Result<()> {
        execute!(self.inner, ResetColor)?;
        let _prev_all_match = self.all_match;
        self.all_match = self
            .line_buf
            .iter()
            .all(|(_, c)| *c == DiffCharCategory::Match);
        if self.all_match {
            if !_prev_all_match {
                writeln!(self.inner, " (...)")?;
            }
        } else {
            for i in 0..2 {
                write!(self.inner, " ")?;
                let mut prev_category = DiffCharCategory::Match;
                for (chars, category) in &self.line_buf {
                    if category != &prev_category {
                        if let Some(col) = category.get_color() {
                            execute!(self.inner, SetForegroundColor(col[i]))?;
                        } else {
                            execute!(self.inner, ResetColor)?;
                        }
                        prev_category = *category;
                    }
                    write!(self.inner, "{}", chars[i])?;
                }
                execute!(self.inner, ResetColor)?;
                writeln!(self.inner)?;
            }
        }
        self.line_buf.clear();
        Ok(())
    }
}
