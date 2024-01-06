/*
 Fast and flexible tool for reading, modifying and writing biological sequences
*/

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate seq_io;

use crate::cli::Cli;
use crate::config::Config;

use self::error::*;
use std::process;

#[macro_use]
mod helpers;
mod cli;
mod cmd;
mod config;
mod error;
mod io;
mod var;

#[cfg(test)]
mod test;

fn main() {
    let mut cli = Cli::new();
    match cli.run() {
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

fn exit(msg: &str, code: i32) {
    eprintln!("{}", msg);
    process::exit(code);
}
