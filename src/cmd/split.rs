use std::fs::create_dir_all;
use std::path::Path;

use clap::Parser;

use var_provider::{dyn_var_provider, DynVarProviderInfo, VarType};
use variable_enum_macro::variable_enum;

use crate::cli::CommonArgs;
use crate::config::Config;
use crate::helpers::DefaultHashMap as HashMap;
use crate::io::output::{FormatWriter, OutputKind};
use crate::var::{modules::VarProvider, parser::Arg, symbols, varstring, VarBuilder};
use crate::CliResult;

#[derive(Parser, Clone, Debug)]
#[clap(next_help_heading = "Command options")]
pub struct SplitCommand {
    /// Split into chunks of <N> sequences and writes each chunk to a separate
    /// file with a numbered suffix.
    ///
    /// The output path can be changed using -o/--output. The default is:
    /// '{filestem}_{chunk}.{default_ext}' (e.g. input_name_1.fasta).
    ///
    /// Examples:
    ///
    /// Distribute sequences into different files by an attribute 'category'
    /// found in the sequence headers, producing files such as:
    /// outdir/category_A.fasta, outdir/category_B.fasta, etc.
    ///
    /// `st split input.fasta -o "outdir/{attr(category)}.fasta"`
    ///
    /// Group the input sequences by the recognized primer, which is recognized
    /// using the 'find' command
    ///
    /// `st find -f file:primers.fa input.fq -a primer='{pattern_name}' | st split -o "{attr(primer)}.fq"`
    #[arg(short, long, value_name = "N")]
    num_seqs: Option<usize>,

    /// Automatically create all parent directories of the output path.
    #[arg(short, long)]
    parents: bool,

    #[command(flatten)]
    pub common: CommonArgs,
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

    // register variable provider
    if let Some(n) = args.num_seqs {
        cfg.set_custom_varmodule(Box::new(SplitVars::new(n)))?;
    }

    let (out_key, _) =
        cfg.build_vars(|b| varstring::VarString::parse_register(out_key, b, false))?;
    // file path -> writer
    let mut outfiles = HashMap::default();
    // path buffer
    let mut path = vec![];
    // writer for formatted output
    // TODO: allow autorecognition of extension
    let mut format_writer = cfg.get_format_writer()?;

    cfg.read(|record, ctx| {
        // update chunk ndyn_var_provider!umber variable
        if num_seqs.is_some() {
            ctx.custom_vars(|opt_mod: Option<&mut SplitVars>, sym| {
                opt_mod.map(|m| m.increment(sym)).transpose()
            })?;
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

variable_enum! {
    /// # Variables available in the split command
    ///
    /// # Examples
    ///
    /// Split input into chunks of 1000 sequences, which will be named
    /// outdir/file_1.fq, outdir/file_2.fq, etc.
    ///
    /// `st split -n 1000 -po 'outdir/out_{chunk}.fq' input.fastq`
    SplitVar {
        /// Chunk number starting with 1. With the -n argument, it will
        /// increment by one each time the size limit <N> is reached.
        /// Otherwise, it will always be 1.
        Chunk(Number),
    }
}

#[derive(Debug)]
struct SplitVars {
    symbol_id: Option<usize>,
    limit: usize,
    seq_num: usize,
    chunk_num: usize,
}

impl SplitVars {
    // limit == 0 means no limit at all, chunk_num remains 1
    fn new(limit: usize) -> SplitVars {
        SplitVars {
            symbol_id: None,
            limit,
            seq_num: 0,
            chunk_num: 0,
        }
    }

    fn increment(&mut self, symbols: &mut symbols::SymbolTable) -> Result<(), String> {
        if let Some(var_id) = self.symbol_id {
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

impl VarProvider for SplitVars {
    fn info(&self) -> &dyn DynVarProviderInfo {
        &dyn_var_provider!(SplitVar)
    }

    fn register(
        &mut self,
        name: &str,
        args: &[Arg],
        builder: &mut VarBuilder,
    ) -> Result<Option<(usize, Option<VarType>)>, String> {
        Ok(SplitVar::from_func(name, args)?.map(|(var, out_type)| {
            let SplitVar::Chunk = var;
            let symbol_id = self.symbol_id.get_or_insert_with(|| builder.increment());
            (*symbol_id, out_type)
        }))
    }

    fn has_vars(&self) -> bool {
        self.symbol_id.is_some()
    }
}
