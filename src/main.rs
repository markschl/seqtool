/*!

*/

#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

//#![feature(alloc_system)] extern crate alloc_system;

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
extern crate nom;
#[macro_use]
extern crate seq_io;
extern crate vec_map;

// used by specific commands
extern crate bit_vec;
extern crate bytecount;
extern crate meval;
extern crate rand;
extern crate regex;
extern crate twoway;

// compression
extern crate bzip2;
extern crate flate2;
extern crate lz4;

use std::process;
use self::error::*;

#[macro_use]
mod macros;
#[macro_use]
mod help;
mod error;
#[allow(unused)]
mod lib;
mod io;
mod cfg;
mod opt;
mod var;
mod cmd;

#[cfg(test)]
mod test;
#[cfg(test)]
extern crate assert_cli;

fn main() {
    let args = docopt::Docopt::new(help::USAGE)
        .and_then(|d| {
            d.version(Some(lib::util::version()))
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
        "count" => cmd::count::run(),
        "stat" => cmd::stat::run(),
        "upper" => cmd::upper::run(),
        "lower" => cmd::lower::run(),
        "mask" => cmd::mask::run(),
        "revcomp" => cmd::revcomp::run(),
        _ => Err(CliError::Other(
            concat!("Unknown command! Available commands:\n", command_list!()).to_string(),
        )),
    }
}
