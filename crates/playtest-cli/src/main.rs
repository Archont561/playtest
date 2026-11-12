//! Entrypoint for the `playtest` command-line binary.

use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    if writeln!(io::stdout(), "playtest {}", playtest_cli::VERSION).is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
