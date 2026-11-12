//! Build script enforcing the minimum Rust toolchain (`1.88.0`) with a clear diagnostic
//! (`Spec 0012 §§1.5, 4.2`).

use std::env;
use std::io::{self, Write};
use std::process::{Command, ExitCode};

const MIN_MINOR: u32 = 88;

fn emit_line(stream: &mut dyn Write, line: &str) {
    let _ = writeln!(stream, "{line}");
}

fn parse_rustc_major_minor(raw: &str) -> Option<(u32, u32)> {
    let token = raw
        .split_whitespace()
        .find(|part| part.chars().next().is_some_and(|ch| ch.is_ascii_digit()))?;
    let core = token.split('-').next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    Some((major, minor))
}

fn detect_rustc_version() -> Result<String, String> {
    if let Ok(simulated) = env::var("PLAYTEST_SIMULATE_RUSTC_VERSION")
        && !simulated.trim().is_empty()
    {
        return Ok(simulated);
    }
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let output = Command::new(&rustc)
        .arg("--version")
        .output()
        .map_err(|err| format!("failed to invoke `{rustc} --version`: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "`{rustc} --version` exited with status {}",
            output.status
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() -> ExitCode {
    let mut stdout = io::stdout().lock();
    emit_line(&mut stdout, "cargo:rerun-if-env-changed=RUSTC");
    emit_line(
        &mut stdout,
        "cargo:rerun-if-env-changed=PLAYTEST_SIMULATE_RUSTC_VERSION",
    );

    let raw_version = match detect_rustc_version() {
        Ok(ver) => ver,
        Err(reason) => {
            let mut stderr = io::stderr().lock();
            emit_line(
                &mut stderr,
                &format!(
                    "error: playtest-cli requires Rust 1.{MIN_MINOR} or newer ({reason}). \
                     Install or update Rust via `rustup update stable`."
                ),
            );
            return ExitCode::FAILURE;
        }
    };

    if let Some((major, minor)) = parse_rustc_major_minor(&raw_version)
        && (major < 1 || (major == 1 && minor < MIN_MINOR))
    {
        let mut stderr = io::stderr().lock();
        emit_line(
            &mut stderr,
            &format!(
                "error: playtest-cli (and playtest-py source builds) requires Rust 1.{MIN_MINOR}.0 \
                 or newer, but found `{raw_version}`. Upgrade your Rust toolchain with \
                 `rustup update stable` or install a prebuilt platform binary/wheel."
            ),
        );
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
