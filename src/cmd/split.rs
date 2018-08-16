use std::fs::create_dir_all;
use std::path::Path;

use fxhash::FxHashMap;

use cfg;
use error::{CliError, CliResult};
use io::output::{WriteFinish, Writer};
use lib::inner_result::MapRes;
use opt;
use var::{symbols, varstring, VarHelp, VarProvider, VarStore};

pub static USAGE: &'static str = concat!(
    "
This command distributes sequences into multiple files based on different
criteria. In contrast to other commands, the output (-o) argument can
contain variables in order to determine the file path for each sequence.

Usage:
    st split [options][-a <attr>...][-l <list>...] [<input>...]
    st split (-h | --help)
    st split --help-vars

Options:
    -n, --num-seqs <N>  Split into chunks of <N> sequences and writes them to
                        'f_{split:chunk}.{default_ext}'. This is actually a
                        variable string which can be changed using -o/--output.
    -p, --parents       Automatically create all parent directories found in -o

",
    common_opts!()
);

pub fn run() -> CliResult<()> {
    let args = opt::Args::new(USAGE)?;
    let cfg = cfg::Config::from_args_with_help(&args, &CunkVarHelp)?;

    let n = args.opt_value("--num-seqs")?;
    let out = args.opt_str("--output");
    let parents = args.get_bool("--parents");
    let verbose = args.get_bool("--verbose");

    let (key, limit) = if let Some(n) = n {
        (out.unwrap_or("f_{split:chunk}.{default_ext}"), n)
    } else if let Some(key) = out {
        (key, 0)
    } else {
        return fail!("The split command requires either '-n' or '-o'.");
    };

    let mut vars = cfg.vars()?;
    let mut chunk_vars = ChunkNum::new(limit);
    let var_key = vars.build_with(Some(&mut chunk_vars), |b| {
        varstring::VarString::var_or_composed(key, b)
    })?;

    let mut outfiles: FxHashMap<_, Box<Writer<_>>> = FxHashMap::default();
    let mut path = vec![];

    cfg.read_sequential_var(&mut vars, |record, mut vars| {
        // update chunk number variable
        chunk_vars.increment(&mut vars.mut_data().symbols)?;

        // compose key
        path.clear();
        var_key.compose(&mut path, vars.symbols());

        // cannot use Entry API
        // https://github.com/rust-lang/rfcs/pull/1769 ??
        if let Some(w) = outfiles.get_mut(&path) {
            w.write(&record, vars)?;
            return Ok(true);
        }

        // initialize new file
        let path_str = ::std::str::from_utf8(&path)?;
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
            None,
        )?;
        outfiles.insert(path.clone(), w);

        let writer = outfiles.get_mut(&path).unwrap();
        writer.write(&record, vars)?;
        Ok(true)
    })?;

    // file handles from Config::other_writer() have to be finished
    for (_, f) in outfiles {
        f.into_inner()
            .map_res(|w| {
                w?.finish()?.flush()?;
                Ok::<_, CliError>(())
            })?;
    }
    Ok(())
}

pub struct CunkVarHelp;

impl VarHelp for CunkVarHelp {
    fn name(&self) -> &'static str {
        "Split command variables"
    }
    fn usage(&self) -> &'static str {
        "split:<variable>"
    }
    fn vars(&self) -> Option<&'static [(&'static str, &'static str)]> {
        Some(&[(
            "split:chunk",
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
            limit: limit,
            seq_num: 0,
            chunk_num: 0,
        }
    }

    fn increment(&mut self, symbols: &mut symbols::Table) -> CliResult<()> {
        if let Some(var_id) = self.id {
            self.seq_num += 1;
            if self.chunk_num == 0 || self.seq_num > self.limit {
                self.seq_num = 1;
                self.chunk_num += 1;
                symbols.set_int(var_id, self.chunk_num as i64);
            }
        }
        Ok(())
    }
}

impl VarProvider for ChunkNum {
    fn prefix(&self) -> Option<&str> {
        Some("split")
    }
    fn name(&self) -> &'static str {
        "split"
    }

    fn register_var(&mut self, name: &str, id: usize, _: &mut VarStore) -> CliResult<bool> {
        if name == "chunk" {
            self.id = Some(id);
            return Ok(true);
        }
        Ok(false)
    }

    fn has_vars(&self) -> bool {
        self.id.is_some()
    }
}
