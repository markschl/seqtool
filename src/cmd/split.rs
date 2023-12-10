use std::fs::create_dir_all;
use std::path::Path;

use clap::Parser;
use fxhash::FxHashMap;

use crate::config::Config;
use crate::error::{CliError, CliResult};
use crate::io::output::{OutputKind, Writer};
use crate::opt::CommonArgs;
use crate::var::{symbols, varstring, Func, VarBuilder, VarHelp, VarProvider};

/// This command distributes sequences into multiple files based on different
/// criteria. In contrast to other commands, the output (-o) argument can
/// contain variables in order to determine the file path for each sequence.
///
/// Example splitting a file into evenly sized chunks:
#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct SplitCommand {
    /// Split into chunks of <N> sequences and writes them to
    /// '{filestem}_{chunk}.{default_ext}'.
    /// This is a variable string which can be changed using -o/--output.
    #[arg(short, long, value_name = "N")]
    num_seqs: Option<usize>,

    /// Automatically create all parent directories found in -o
    #[arg(short, long)]
    parents: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn run(cfg: Config, args: &SplitCommand) -> CliResult<()> {
    let num_seqs = args.num_seqs;
    let parents = args.parents;
    let verbose = args.common.general.verbose;

    let out_path = match args.common.output.output.as_ref() {
        Some(OutputKind::File(p)) => Some(p.as_str()),
        Some(OutputKind::Stdout) => {
            return fail!("The split command requires an output path, not STDOUT.")
        }
        None => None,
    };

    let (key, limit) = if let Some(n) = num_seqs {
        (
            out_path
                .as_deref()
                .unwrap_or("{filestem}_{chunk}.{default_ext}"),
            n,
        )
    } else if let Some(key) = out_path {
        (key, 0)
    } else {
        return fail!("The split command requires either '-n' or '-o'.");
    };

    // initialize variable provider
    let m: Option<Box<dyn VarProvider>> = if limit == 0 {
        None
    } else {
        Some(Box::new(ChunkNum::new(limit)))
    };
    cfg.with_vars(m, |vars| {
        let var_key = vars.build(|b| varstring::VarString::var_or_composed(key, b))?;

        let mut outfiles: FxHashMap<_, Box<dyn Writer<_>>> = FxHashMap::default();
        let mut path = vec![]; // path buffer

        cfg.read(vars, |record, mut vars| {
            // update chunk number variable
            if limit != 0 {
                vars.custom_mod::<ChunkNum, _>(|m, sym| m.unwrap().increment(sym))?;
            }

            // compose key
            path.clear();
            var_key.compose(&mut path, vars.symbols(), record);

            // cannot use Entry API
            // https://github.com/rust-lang/rfcs/pull/1769 ??
            if let Some(w) = outfiles.get_mut(&path) {
                w.write(&record, vars)?;
                return Ok(true);
            }

            // initialize new file
            let path_str = std::str::from_utf8(&path)?;
            report!(verbose, "New file: '{}'", path_str);
            let p = Path::new(path_str);
            if let Some(par) = p.parent() {
                if !par.exists() && !par.as_os_str().is_empty() && !parents {
                    return fail!(format!(
                        "Could not create file '{}' because the parent directory does not exist. \
                        Use -p/--parents to create automatically",
                        path_str
                    ));
                }
                create_dir_all(par)?;
            }

            let w = cfg.other_writer(
                path_str,
                // only register output variables the first time since different files
                // do not have different variable sets
                if outfiles.is_empty() {
                    Some(&mut vars)
                } else {
                    None
                },
            )?;
            outfiles.insert(path.clone(), w);

            let writer = outfiles.get_mut(&path).unwrap();
            writer.write(&record, vars)?;
            Ok(true)
        })?;

        // file handles from Config::other_writer() have to be finished
        for (_, f) in outfiles {
            f.into_inner()
                .map(|w| {
                    w?.finish()?.flush()?;
                    Ok::<_, CliError>(())
                })
                .transpose()?;
        }
        Ok(())
    })
}

#[derive(Debug)]
pub struct ChunkVarHelp;

impl VarHelp for ChunkVarHelp {
    fn name(&self) -> &'static str {
        "Split command variables"
    }

    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[(
            "chunk",
            "Chunk number starting with 1. With the -n argument, it will \
             increment by one each time the size limit <N> is reached. \
             Otherwise, it will always be 1.",
        )])
    }
}

#[derive(Debug)]
struct ChunkNum {
    id: Option<usize>,
    limit: usize,
    seq_num: usize,
    chunk_num: usize,
}

impl ChunkNum {
    // limit == 0 means no limit at all, chunk_num remains 1
    fn new(limit: usize) -> ChunkNum {
        ChunkNum {
            id: None,
            limit,
            seq_num: 0,
            chunk_num: 0,
        }
    }

    fn increment(&mut self, symbols: &mut symbols::SymbolTable) -> CliResult<()> {
        if let Some(var_id) = self.id {
            self.seq_num += 1;
            if self.chunk_num == 0 || self.seq_num > self.limit {
                self.seq_num = 1;
                self.chunk_num += 1;
                symbols.get_mut(var_id).set_int(self.chunk_num as i64);
            }
        }
        Ok(())
    }
}

impl VarProvider for ChunkNum {
    fn help(&self) -> &dyn VarHelp {
        &ChunkVarHelp
    }

    fn register(&mut self, var: &Func, b: &mut VarBuilder) -> CliResult<bool> {
        if var.name == "chunk" {
            self.id = Some(b.symbol_id());
            return Ok(true);
        }
        Ok(false)
    }

    fn has_vars(&self) -> bool {
        true
    }
}
