"""Rust dependency policy runner (cargo-deny bans/licenses/sources + live advisory audit)."""

from __future__ import annotations

import argparse
import os
import shutil
from collections.abc import Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd
from automation.advisory_exception import verify_advisory_exceptions


def run_offline_audit(root: Path = REPO_ROOT) -> int:
    """Verify advisory exceptions and run cargo-deny bans, licenses, and sources gates."""
    errors = verify_advisory_exceptions(root)
    if errors:
        for err in errors:
            emit_err(f"[audit-local] ERROR: {err}")
        return 1

    if shutil.which("cargo-deny") is None:
        emit_err("[audit-local] ERROR: cargo-deny is not installed on PATH.")
        return 1

    use_offline = (root / ".pixi-sandbox" / "vendor").is_dir() or (
        os.environ.get("CARGO_NET_OFFLINE", "").lower() == "true"
    )
    cmd = ["cargo", "deny", "--locked"]
    if use_offline:
        cmd.append("--offline")
    cmd.extend(["check", "bans", "licenses", "sources"])
    env_overrides = {"CARGO_NET_OFFLINE": "true"} if use_offline else None

    res = run_cmd(cmd, cwd=root, env=env_overrides, timeout=300)
    if res.returncode != 0:
        emit_err("[audit-local] ERROR: cargo deny check failed.")
        return res.returncode
    emit_line("[audit-local] OK: dependency bans, licenses, sources, and ignores verified.")
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
        ("cargo", "deny", "--locked", "check", "advisories", "bans", "licenses", "sources"),
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
    """CLI entry point for local or live dependency policy auditing."""
    parser = argparse.ArgumentParser(description="Run Rust dependency policy checks.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT, help="Repository root path.")
    parser.add_argument("--live", action="store_true", help="Run live network advisory checks.")
    args = parser.parse_args(list(argv) if argv is not None else None)
    if args.live:
        return run_live_audit(args.root.resolve())
    return run_offline_audit(args.root.resolve())


if __name__ == "__main__":
    raise SystemExit(main())
