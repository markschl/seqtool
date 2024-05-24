use std::env::temp_dir;
use std::io::Write;
use std::path::Path;

use crate::config::Config;
use crate::error::CliResult;
use crate::helpers::vec::VecFactory;
use crate::var::varstring::register_var_list;

use super::shared::sort_item::{Item, Key};

pub mod cli;
pub mod file;
pub mod mem;
pub mod vars;

pub use self::cli::*;
pub use self::file::*;
pub use self::mem::*;
pub use self::vars::*;

/// Factor for adjusting the calculated memory usage (based on size of items)
/// to obtain the approximately correct total memory usage.
/// It corrects for the extra memory used by Vec::sort() and other allocations
/// that may not be in the calculation otherwise.
/// (factor found by memory profiling on Linux)
static MEM_OVERHEAD: f32 = 1.1;

pub fn run(mut cfg: Config, args: &SortCommand) -> CliResult<()> {
    let verbose = args.common.general.verbose;
    let quiet = args.common.general.quiet;
    let max_mem = (args.max_mem as f32 / MEM_OVERHEAD) as usize;
    // TODO: not activated, since we use a low limit for testing
    // if args.max_mem < 1 << 22 {
    //     return fail!("The memory limit should be at least 2MiB");
    // }
    let mut record_buf_factory = VecFactory::new();
    let tmp_path = args.temp_dir.clone().unwrap_or_else(temp_dir);
    let mut sorter = Sorter::new(args.reverse, max_mem);

    cfg.set_custom_varmodule(Box::<SortVars>::default())?;

    let mut format_writer = cfg.get_format_writer()?;

    cfg.with_io_writer(|io_writer, mut cfg| {
        // assemble key
        let mut varstring_keys = Vec::with_capacity(1);
        cfg.build_vars(|b| register_var_list(&args.key, b, &mut varstring_keys, true))?;
        let mut keys = Key::with_size(varstring_keys.len());
        let mut text_buf = vec![Vec::new(); varstring_keys.len()];

        cfg.read(|record, ctx| {
            // assemble key
            ctx.custom_vars::<SortVars, _, String>(|key_mod, symbols| {
                keys.compose_from(&varstring_keys, &mut text_buf, symbols, record)?;
                if let Some(m) = key_mod {
                    m.set(&keys, symbols);
                }
                Ok(())
            })?;
            // write formatted record to a buffer
            let record_out =
                record_buf_factory.get(|out| format_writer.write(&record, out, ctx))?;
            // add both to the object handing the sorting
            sorter.add(
                Item::new(keys.clone(), record_out.into_boxed_slice()),
                &tmp_path,
                args.temp_file_limit,
                quiet,
            )?;
            Ok(true)
        })?;
        // write sorted output
        sorter.write(io_writer, quiet, verbose)
    })
}

#[derive(Debug)]
enum Sorter {
    Mem(MemSorter),
    File(FileSorter),
}

impl Sorter {
    fn new(reverse: bool, max_mem: usize) -> Self {
        Self::Mem(MemSorter::new(reverse, max_mem))
    }

    fn add(
        &mut self,
        item: Item<Box<[u8]>>,
        tmp_path: &Path,
        file_limit: usize,
        quiet: bool,
    ) -> CliResult<()> {
        match self {
            Self::Mem(m) => {
                if !m.add(item) {
                    if !quiet {
                        eprintln!(
                            "Memory limit reached after {} records, writing to temporary file(s). \
                            Consider raising the limit (-M/--max-mem) to speed up sorting. \
                            Use -q/--quiet to silence this message.",
                            m.len()
                        );
                    }
                    let mut f = m.get_file_sorter(tmp_path.to_owned(), file_limit)?;
                    f.write_to_file(quiet)?;
                    *self = Self::File(f);
                }
            }
            Self::File(f) => {
                f.add(item, quiet)?;
            }
        }
        Ok(())
    }

    fn write(&mut self, io_writer: &mut dyn Write, quiet: bool, verbose: bool) -> CliResult<()> {
        match self {
            Self::Mem(m) => m.write_sorted(io_writer),
            Self::File(f) => f.write_records(io_writer, quiet, verbose),
        }
    }
}
