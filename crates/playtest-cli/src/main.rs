//! Entrypoint for the `playtest` command-line binary.

use std::process::ExitCode;

fn main() -> ExitCode {
    println!("playtest {}", playtest_cli::VERSION);
    ExitCode::SUCCESS
}
