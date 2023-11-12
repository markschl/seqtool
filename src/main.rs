/*
 Fast and flexible tool for reading, modifying and writing biological sequences
*/

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate seq_io;
#[macro_use]
extern crate maplit;
#[cfg(target_family = "unix")]
extern crate pager;
// #[macro_use]

use self::error::*;
use std::process;

#[macro_use]
mod macros;
#[macro_use]
mod help;
mod cmd;
mod config;
mod error;
mod io;
mod helpers;
mod opt;
mod var;

#[cfg(test)]
// #[cfg_attr(rustfmt, rustfmt_skip)]
mod test;
#[macro_use]
extern crate approx;

fn main() {
    // work around https://github.com/docopt/docopt.rs/issues/240
    let mut argv: Vec<_> = std::env::args().collect();
    if argv.len() > 1 && argv[1].starts_with("st") {
        argv[1] = argv[1][2..].to_string();
    }

    let args = docopt::Docopt::new(help::USAGE)
        .and_then(|d| {
            d.version(Some(helpers::util::version()))
                .argv(argv)
                .options_first(true)
                .help(false)
                .parse()
        })
        .unwrap_or_else(|e| e.exit());

    let cmd = args.get_str("<command>");
    if args.get_bool("--help-vars") {
        exit(&var::var_help(), 0);
    }
    if cmd.is_empty() {
        exit(help::USAGE, 0);
    } else {
        match run_cmd(cmd) {
            // normal exit
            Ok(()) => {}
            Err(CliError::Io(e)) => {
                if e.kind() != std::io::ErrorKind::BrokenPipe {
                    exit(&format!("{}", e), 1)
                }
            }
            Err(e) => exit(&format!("{}", e), 1),
        }
    }
}

fn exit(msg: &str, code: i32) {
    eprintln!("{}", msg);
    process::exit(code);
}

fn run_cmd(cmd: &str) -> CliResult<()> {
    match cmd {
        "." | "pass" => cmd::pass::run(),
        "slice" => cmd::slice::run(),
        "sample" => cmd::sample::run(),
        "head" => cmd::head::run(),
        "tail" => cmd::tail::run(),
        "split" => cmd::split::run(),
        "trim" => cmd::trim::run(),
        "set" => cmd::set::run(),
        "del" => cmd::del::run(),
        "find" => cmd::find::run(),
        "replace" => cmd::replace::run(),
        "filter" => cmd::filter::run(),
        "count" => cmd::count::run(),
        "at" => cmd::stat::run(),
        "upper" => cmd::upper::run(),
        "lower" => cmd::lower::run(),
        "mask" => cmd::mask::run(),
        "revcomp" => cmd::revcomp::run(),
        "interleave" => cmd::interleave::run(),
        "concat" => cmd::concat::run(),
        "view" => cmd::view::run(),
        _ => Err(CliError::Other(
            concat!("Unknown command! Available commands:\n", command_list!()).to_string(),
        )),
    }
}
