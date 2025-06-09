use crate::config::Config;
use crate::error::CliResult;
use crate::var::{modules::VarProvider, varstring::register_var_list};

mod cli;
mod complete;
mod in_order;
mod output;
mod vars;

pub use self::cli::*;
pub use self::output::*;
pub use self::vars::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Category {
    Common,
    Unique1,
    Unique2,
}

use self::Category::*;

impl Category {
    fn long_text(self) -> &'static str {
        match self {
            Common => "common",
            Unique1 => "unique1",
            Unique2 => "unique2",
        }
    }

    fn short_text(self) -> &'static str {
        match self {
            Common => "c",
            Unique1 => "u1",
            Unique2 => "u2",
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
struct CmpStats {
    pub common: u64,
    pub unique1: u64,
    pub unique2: u64,
}

/// Factor for adjusting the calculated memory usage (based on size of items)
/// to obtain the approximately correct total memory usage.
/// It corrects for the extra memory that may not be in the calculation otherwise.
static MEM_OVERHEAD: f32 = 1.1;

pub fn run(mut cfg: Config, mut args: CmpCommand) -> CliResult<()> {
    let quiet = args.common.general.quiet;
    let two_pass = args.two_pass;
    let max_mem = (args.max_mem as f32 / MEM_OVERHEAD) as usize;

    // register variables/functions:
    // tuples of (varstring, text buffer)
    cfg.set_custom_varmodule(Box::<CmpVars>::default())?;

    let mut out = Output::from_args(&mut args, &mut cfg)?;

    let mut var_key = Vec::with_capacity(1);
    cfg.build_vars(|b| {
        for key in &args.key {
            register_var_list(key.as_ref(), b, &mut var_key, true, true)?;
        }
        Ok::<_, String>(())
    })?;

    cfg.with_custom_varmod(|v: &mut CmpVars| {
        if out.has_combined_output() && !v.has_vars() {
            return fail!(
                "Specified mixed output in 'cmp' command ' -o/--output/--output2', \
                but no variables are used to distinguish records. Please specify \
                one of `category`, `category_short` or `key`, or specify unique \
                output instead (--unique1/--unique2)."
            );
        }
        Ok::<_, String>(())
    })?;

    let stats = if args.in_order {
        in_order::cmp_in_order(&mut cfg, &var_key, &mut out, max_mem)?
    } else {
        complete::cmp_complete(&mut cfg, &var_key, &mut out, max_mem, two_pass, quiet)?
    };
    if !quiet {
        eprintln!(
            "common\t{}\nunique1\t{}\nunique2\t{}",
            stats.common, stats.unique1, stats.unique2
        );
    }
    Ok(())
}
