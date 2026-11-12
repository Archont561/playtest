//! Entrypoint for the `playtest` command-line binary.

use std::process::ExitCode;

fn main() -> ExitCode {
    playtest_cli::run_process()
}
