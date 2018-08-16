// used everywhere
extern crate bio;
extern crate csv;
extern crate docopt;
extern crate fxhash;
extern crate itertools;
#[macro_use]
extern crate lazy_static;
extern crate memchr;
#[macro_use]
extern crate seq_io;
extern crate thread_io;
extern crate vec_map;

// used by specific commands
extern crate bit_vec;
extern crate bytecount;
#[cfg(feature = "exprtk")]
extern crate exprtk_rs;
extern crate meval;
extern crate rand;
extern crate regex;
extern crate twoway;
#[macro_use]
extern crate maplit;
extern crate byteorder;
extern crate ordered_float;
#[cfg(target_family = "unix")]
extern crate pager;
extern crate palette;
extern crate read_color;
extern crate termcolor;

// compression
extern crate bzip2;
extern crate flate2;
extern crate lz4;
extern crate zstd;

use self::error::*;
use std::process;

#[macro_use]
mod macros;
#[macro_use]
mod help;
mod cfg;
#[allow(unused_imports)]
mod cmd;
mod error;
#[allow(unused_imports)] // silence std::ascii::AsciiExt import warnings
mod io;
#[allow(unused)]
mod lib;
#[allow(unused_imports)]
mod opt;
mod var;

#[cfg(test)]
#[cfg_attr(rustfmt, rustfmt_skip)]
mod test;
#[cfg(test)]
extern crate assert_cli;
#[macro_use]
extern crate approx;

fn main() {

    // work around https://github.com/docopt/docopt.rs/issues/240
    let mut argv: Vec<_> = ::std::env::args().collect();
    if argv.len() > 1 && argv[1].starts_with("st") {
        argv[1] = argv[1][2..].to_string();
    }

    let args = docopt::Docopt::new(help::USAGE)
        .and_then(|d| {
            d.version(Some(lib::util::version()))
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
            Err(CliError::Io(e)) => if e.kind() != ::std::io::ErrorKind::BrokenPipe {
                exit(&format!("{}", e), 1)
            },
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
        #[cfg(feature = "exprtk")]
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
