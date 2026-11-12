"""Shared subprocess and diagnostic output helpers for Playtest automation."""

from __future__ import annotations

import os
import subprocess
import sys
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path

from automation import REPO_ROOT

DEFAULT_TIMEOUT_SECONDS: int = 600


@dataclass(frozen=True)
class CommandResult:
    """Captured result of a hermetic child process invocation."""

    returncode: int
    stdout: str
    stderr: str


def emit_line(message: str = "") -> None:
    """Write a single line to standard output without invoking debug print."""
    sys.stdout.write(f"{message}\n")
    sys.stdout.flush()


def emit_err(message: str) -> None:
    """Write a single diagnostic line to standard error."""
    sys.stderr.write(f"{message}\n")
    if os.environ.get("GITHUB_ACTIONS") == "true":
        escaped = message.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
        sys.stderr.write(f"::error::{escaped}\n")
    sys.stderr.flush()


def merged_env(extra: Mapping[str, str] | None = None) -> dict[str, str]:
    """Return a copy of the process environment overlaid with extra variables."""
    env = dict(os.environ)
    if extra is not None:
        env.update(extra)
    return env


def run_cmd(
    cmd: Sequence[str],
    *,
    cwd: Path = REPO_ROOT,
    env: Mapping[str, str] | None = None,
    timeout: int = DEFAULT_TIMEOUT_SECONDS,
    capture: bool = False,
) -> CommandResult:
    """Run a child command without a shell and with an explicit timeout."""
    completed = subprocess.run(
        list(cmd),
        cwd=cwd,
        env=merged_env(env),
        timeout=timeout,
        check=False,
        text=True,
        capture_output=capture,
    )
    return CommandResult(
        returncode=completed.returncode,
        stdout=completed.stdout or "",
        stderr=completed.stderr or "",
    )


def write_github_output(values: Mapping[str, str], output_path: Path | None = None) -> None:
    """Append key-value pairs to GITHUB_OUTPUT when running in GitHub Actions."""
    resolved = output_path
    if resolved is None:
        raw_env = os.environ.get("GITHUB_OUTPUT", "").strip()
        if not raw_env:
            return
        resolved = Path(raw_env)
    lines = [f"{key}={value}\n" for key, value in values.items()]
    with resolved.open("a", encoding="utf-8") as handle:
        handle.writelines(lines)
