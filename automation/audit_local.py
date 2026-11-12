"""Offline Rust dependency policy runner (advisory expiry check + cargo-deny)."""

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

    if shutil.which("cargo-deny") is None and shutil.which("cargo") is None:
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


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for offline dependency policy auditing."""
    parser = argparse.ArgumentParser(description="Run offline Rust dependency policy checks.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT, help="Repository root path.")
    args = parser.parse_args(list(argv) if argv is not None else None)
    return run_offline_audit(args.root.resolve())


if __name__ == "__main__":
    raise SystemExit(main())
