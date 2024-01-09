use std::fs::create_dir_all;
use std::path::Path;

use clap::Parser;
use fxhash::FxHashMap;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::error::CliResult;
use crate::io::output::{FormatWriter, OutputKind};
use crate::var::{
    func::Func,
    symbols::{self, VarType},
    varstring, VarBuilder, VarHelp, VarProvider,
};

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct SplitCommand {
    /// Split into chunks of <N> sequences and writes each chunk to a separate
    /// file with a numbered suffix.
    /// The output path can be changed using -o/--output. The default is:
    /// '{filestem}_{chunk}.{default_ext}' (e.g. input_name_1.fasta).
    #[arg(short, long, value_name = "N")]
    num_seqs: Option<usize>,

    /// Automatically create all parent directories of the output path.
    #[arg(short, long)]
    parents: bool,

    #[command(flatten)]
    pub common: CommonArgs,
}

pub fn get_varprovider(args: &SplitCommand) -> Option<Box<dyn VarProvider>> {
    if let Some(n) = args.num_seqs {
        Some(Box::new(ChunkNum::new(n)))
    } else {
        None
    }
}

pub fn run(mut cfg: Config, args: &SplitCommand) -> CliResult<()> {
    let num_seqs = args.num_seqs;
    let parents = args.parents;
    let verbose = args.common.general.verbose;

    let out_path = match args.common.output.output.as_ref() {
        Some(OutputKind::File(p)) => Some(p.as_str()),
        Some(OutputKind::Stdout) => {
            return fail!("The split command requires an output path with variables, not STDOUT.")
        }
        None => None,
    };

    // output path (or default) and chunk size
    let out_key = if num_seqs.is_some() {
        out_path.unwrap_or("{filestem}_{chunk}.{default_ext}")
    } else if let Some(key) = out_path {
        key
    } else {
        return fail!("The split command requires either '-n' or '-o'.");
    };

    let (out_key, _) = cfg.build_vars(|b| varstring::VarString::parse_register(out_key, b))?;
    // file path -> writer
    let mut outfiles = FxHashMap::default();
    // path buffer
    let mut path = vec![];
    // writer for formatted output
    // TODO: allow autorecognition of extension
    let mut format_writer = cfg.get_format_writer()?;

    cfg.read(|record, ctx| {
        // update chunk number variable
        if num_seqs.is_some() {
            ctx.command_vars::<ChunkNum, _>(|m, sym| m.unwrap().increment(sym))?;
        }

        // compose key
        path.clear();
        out_key.compose(&mut path, &ctx.symbols, record)?;

        // cannot use Entry API
        // https://github.com/rust-lang/rfcs/pull/1769 ??
        if let Some(io_writer) = outfiles.get_mut(&path) {
            format_writer.write(&record, io_writer, ctx)?;
            return Ok(true);
        }

        // if output file does not exist yet, initialize new one
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

        outfiles.insert(path.clone(), ctx.io_writer_from_path(path_str)?);
        let io_writer = outfiles.get_mut(&path).unwrap();
        format_writer.write(&record, io_writer, ctx)?;
        Ok(true)
    })?;

    // file handles from Config::io_writer_other() have to be finished
    for (_, f) in outfiles {
        f.finish()?.flush()?;
    }
    Ok(())
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
                symbols
                    .get_mut(var_id)
                    .inner_mut()
                    .set_int(self.chunk_num as i64);
            }
        }
        Ok(())
    }
}

impl VarProvider for ChunkNum {
    fn help(&self) -> &dyn VarHelp {
        &ChunkVarHelp
    }

    fn register(&mut self, var: &Func, b: &mut VarBuilder) -> CliResult<Option<Option<VarType>>> {
        if var.name == "chunk" {
            self.id = Some(b.symbol_id());
            return Ok(Some(Some(VarType::Int)));
        }
        Ok(None)
    }

    fn has_vars(&self) -> bool {
        true
    }
}
