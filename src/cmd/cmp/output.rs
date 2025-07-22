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
            line_writer: DiffLineWriter::new(max_width, (Color::Magenta, Color::Cyan)),
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

        // debug_assert!(is_second ^ _is_second);
        debug_assert_eq!(fields0.len(), fields1.len());
        for (f0, f1) in fields0.iter().zip(fields1) {
            let aln = self.aligner.global(f0, f1);
            let mut i0 = aln.xstart;
            let mut i1 = aln.ystart;
            // dbg!(&aln.operations);
            for op in &aln.operations {
                use AlignmentOperation::*;
                match op {
                    Match => {
                        self.line_writer.add_char(f0[i0], f1[i1], true)?;
                        i0 += 1;
                        i1 += 1;
                    }
                    Subst => {
                        self.line_writer.add_char(f0[i0], f1[i1], false)?;
                        i0 += 1;
                        i1 += 1;
                    }
                    Del => {
                        self.line_writer.add_char(b' ', f1[i1], false)?;
                        i1 += 1;
                    }
                    Ins => {
                        self.line_writer.add_char(f0[i0], b' ', false)?;
                        i0 += 1;
                    }
                    Xclip(n) => {
                        for _ in 0..*n {
                            self.line_writer.add_char(f0[i0], b' ', false)?;
                            i0 += 1;
                        }
                    }
                    Yclip(n) => {
                        for _ in 0..*n {
                            self.line_writer.add_char(b' ', f1[i1], false)?;
                            i1 += 1;
                        }
                    }
                }
            }
            debug_assert!(i0 == f0.len() && i1 == f1.len());
        }
        self.line_writer.finish()?;
        Ok(())
    }
}

struct DiffLineWriter {
    max_width: usize,
    line_buf: Vec<(u8, u8, bool)>,
    inner: StdoutLock<'static>,
    colors: (Color, Color),
}

impl DiffLineWriter {
    fn new(max_width: usize, colors: (Color, Color)) -> Self {
        Self {
            max_width,
            line_buf: Vec::new(),
            colors,
            inner: stdout().lock(),
        }
    }

    fn write_key(&mut self, key: &Key) -> io::Result<()> {
        execute!(self.inner, ResetColor)?;
        // write!(self.writer, ">")?;
        key.write_delimited(&mut self.inner, b",")?;
        writeln!(self.inner, ":")
    }

    fn add_char(&mut self, c1: u8, c2: u8, matches: bool) -> io::Result<()> {
        self.line_buf.push((c1, c2, matches));
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
        macro_rules! write_line {
            ($line:expr, $color:expr) => {
                let mut colored = false;
                execute!(self.inner, ResetColor)?;
                for (c, matches) in $line {
                    if !matches && !colored {
                        execute!(self.inner, SetForegroundColor($color))?;
                        colored = true;
                    } else if matches && colored {
                        execute!(self.inner, ResetColor)?;
                        colored = false;
                    }
                    write!(self.inner, "{}", c as char)?;
                }
                write!(self.inner, "\n")?;
            };
        }
        write_line!(
            self.line_buf.iter().map(|&(c1, _, m)| (c1, m)),
            self.colors.0
        );
        write_line!(
            self.line_buf.iter().map(|&(_, c2, m)| (c2, m)),
            self.colors.1
        );
        self.line_buf.clear();
        Ok(())
    }
}
