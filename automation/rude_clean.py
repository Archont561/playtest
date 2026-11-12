"""Enforce rude + rude-clean-code Clean Code rules with zero diagnostics."""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd

WARNING_SUMMARY_RE = re.compile(r"Found \d+ error\(s\) and ([1-9]\d*) warning\(s\)")
DEFAULT_TARGETS: tuple[str, ...] = ("automation", "python/src", "python/tests")


def parse_rude_diagnostics(raw_output: str) -> list[dict[str, object]]:
    """Parse newline-delimited JSON diagnostics emitted by `rude check --format json`."""
    diagnostics: list[dict[str, object]] = []
    for raw_line in raw_output.splitlines():
        stripped = raw_line.strip()
        if stripped:
            diagnostics.append(json.loads(stripped))
    return diagnostics


def _format_diagnostic(diag: dict[str, object], root: Path) -> str:
    raw_file = Path(str(diag.get("file", "<unknown>")))
    rel_file = raw_file.relative_to(root) if raw_file.is_relative_to(root) else raw_file
    line = diag.get("line", 0)
    col = diag.get("column", 0)
    code = diag.get("code", "CC???")
    severity = diag.get("severity", "warning")
    message = diag.get("message", "")
    return f"{rel_file.as_posix()}:{line}:{col}: {code} ({severity}) {message}"


def run_rude_clean(root: Path = REPO_ROOT, targets: Sequence[str] = DEFAULT_TARGETS) -> int:
    """Run `rude` with `rude.toml` and fail on any error, warning, or info diagnostic."""
    cfg = root / "rude.toml"
    cmd = [sys.executable, "-m", "rude", "check", "--config", str(cfg), "--format", "json"]
    result = run_cmd([*cmd, *targets], cwd=root, capture=True)
    if result.returncode != 0:
        for stream in (result.stdout.strip(), result.stderr.strip()):
            if stream:
                emit_err(stream)
        return result.returncode
    diagnostics = parse_rude_diagnostics(result.stdout)
    if diagnostics:
        for diag in diagnostics:
            emit_err(_format_diagnostic(diag, root))
        emit_err(f"rude-clean-code failed: {len(diagnostics)} diagnostic(s) must be resolved.")
        return 1
    emit_line(f"rude-clean-code passed (0 diagnostics across {', '.join(targets)}).")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entrypoint for Python Clean Code enforcement."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    parser.add_argument("targets", nargs="*", default=list(DEFAULT_TARGETS))
    args = parser.parse_args(list(argv) if argv is not None else None)
    return run_rude_clean(args.root.resolve(), args.targets)


if __name__ == "__main__":
    raise SystemExit(main())
