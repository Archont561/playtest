"""Numeric Clean Code enforcement for Python sources via rude + rude-clean-code."""

from __future__ import annotations

import argparse
import re
import shutil
from collections.abc import Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd

DEFAULT_TARGETS: tuple[str, ...] = (
    "automation",
    "scripts/stage_assets.py",
    "python/src",
    "python/tests",
)
WARNING_SUMMARY_RE = re.compile(r"Found\s+\d+\s+error\(s\)\s+and\s+([1-9]\d*)\s+warning\(s\)")


def run_rude_clean(root: Path = REPO_ROOT, targets: Sequence[str] = DEFAULT_TARGETS) -> int:
    """Run rude with rude.toml against Python automation and SDK directories."""
    if shutil.which("rude") is None:
        emit_err("[rude-clean] ERROR: `rude` binary not found on PATH (run via Pixi).")
        return 1
    config_path = root / "rude.toml"
    if not config_path.is_file():
        emit_err("[rude-clean] ERROR: rude.toml not found at repository root.")
        return 1
    existing_targets = [rel for rel in targets if (root / rel).exists()]
    cmd = ["rude", "check", "--config", str(config_path), *existing_targets]
    res = run_cmd(cmd, cwd=root, timeout=180, capture=True)
    combined = (res.stdout + res.stderr).strip()
    if combined:
        emit_line(combined)
    if res.returncode != 0 or WARNING_SUMMARY_RE.search(combined) is not None:
        emit_err("[rude-clean] ERROR: Python Clean Code complexity check reported findings.")
        return res.returncode or 1
    emit_line("[rude-clean] OK: Python complexity and Clean Code bounds satisfied.")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for rude + rude-clean-code checks."""
    parser = argparse.ArgumentParser(description="Run rude-clean-code checks.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT, help="Repository root path.")
    parser.add_argument("targets", nargs="*", default=list(DEFAULT_TARGETS))
    args = parser.parse_args(list(argv) if argv is not None else None)
    return run_rude_clean(args.root.resolve(), args.targets)


if __name__ == "__main__":
    raise SystemExit(main())
