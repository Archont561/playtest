"""Rust dependency policy runner (offline cargo-deny + optional live advisory audit)."""

from __future__ import annotations

import argparse
import shutil
from collections.abc import Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd
from automation.advisory_exception import verify_advisory_exceptions


def run_offline_audit(root: Path = REPO_ROOT) -> int:
    """Verify advisory exceptions and run offline cargo-deny policy gates."""
    errors = verify_advisory_exceptions(root)
    if errors:
        for err in errors:
            emit_err(f"[audit-local] ERROR: {err}")
        return 1

    if shutil.which("cargo-deny") is None:
        emit_err("[audit-local] ERROR: cargo-deny is not installed on PATH.")
        return 1

    res = run_cmd(
        ["cargo", "deny", "--offline", "check", "bans", "licenses", "sources"],
        cwd=root,
        env={"CARGO_NET_OFFLINE": "true"},
        timeout=300,
    )
    if res.returncode != 0:
        emit_err("[audit-local] ERROR: cargo deny check failed.")
        return res.returncode
    emit_line("[audit-local] OK: offline dependency bans, licenses, sources, and ignores verified.")
    return 0


def run_live_audit(root: Path = REPO_ROOT) -> int:
    """Populate crate sources and run live cargo-deny + cargo-audit checks."""
    errors = verify_advisory_exceptions(root)
    if errors:
        for err in errors:
            emit_err(f"[audit-live] ERROR: {err}")
        return 1

    steps: tuple[Sequence[str], ...] = (
        ("cargo", "fetch", "--locked"),
        ("cargo", "deny", "check", "advisories", "bans", "licenses", "sources"),
        ("cargo", "audit"),
    )
    for cmd in steps:
        res = run_cmd(cmd, cwd=root, timeout=600)
        if res.returncode != 0:
            emit_err(f"[audit-live] ERROR: command failed: {' '.join(cmd)}")
            return res.returncode
    emit_line("[audit-live] OK: live cargo-deny and cargo-audit passed.")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for offline or live dependency policy auditing."""
    parser = argparse.ArgumentParser(description="Run Rust dependency policy checks.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT, help="Repository root path.")
    parser.add_argument("--live", action="store_true", help="Run live network advisory checks.")
    args = parser.parse_args(list(argv) if argv is not None else None)
    if args.live:
        return run_live_audit(args.root.resolve())
    return run_offline_audit(args.root.resolve())


if __name__ == "__main__":
    raise SystemExit(main())
