use crate::config::Config;
use crate::context::RecordMeta;
use crate::error::CliResult;
use crate::io::output::{SeqFormatter, WriteFinish};
use crate::io::{QualConverter, Record};

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

    // pub fn has_individual_output(&self) -> bool {
    //     self.unique1.is_some()
    //         || self.unique2.is_some()
    //         || self.common.is_some()
    //         || self.common2.is_some()
    // }

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
