"""Command-line surface for the cross-language conformance matrix (`task_023`).

`automation.conformance` holds the manifest reader and the matrix; this module holds the
subcommands, the JSON document, and the CI gate policy, so the library stays testable
without a parser and the module stays inside the project's line budget.

`--gate` is the CI entry point: a cell may be `pass` or `unavailable`, but a `fail` is a
defect and `not_implemented` means the suite claims nothing, so a green run there would
prove nothing.
"""

from __future__ import annotations

import argparse
import json
from collections.abc import Sequence
from pathlib import Path
from typing import Any

from automation._proc import emit_err, emit_line
from automation.conformance import (
    FAIL,
    NOT_IMPLEMENTED,
    PASS,
    SCHEMA_VERSION,
    UNAVAILABLE,
    CellResult,
    Manifest,
    ManifestError,
    build_matrix,
    load_manifest,
    render_table,
    summarize,
)

REPO_ROOT = Path(__file__).resolve().parent.parent


def _cmd_list(args: argparse.Namespace) -> int:
    manifest = load_manifest(args.root)
    document = _manifest_document(manifest)
    emit_line(json.dumps(document, indent=2, sort_keys=True))
    return 0


def _manifest_document(manifest: Manifest) -> dict[str, object]:
    """Describe the manifest: the capabilities and the suite claiming each one."""
    return {
        "schema_version": manifest.schema_version,
        "capabilities": [
            {
                "id": cap.id,
                "title": cap.title,
                "spec": cap.spec,
                "optional": cap.optional,
            }
            for cap in manifest.capabilities
        ],
        "suites": [
            {
                "lang": suite.lang,
                "target": suite.target,
                "cmd": list(suite.cmd),
                "capabilities": list(suite.capabilities),
            }
            for suite in manifest.suites
        ],
    }


def _matrix_document(rows: Sequence[CellResult], summary: dict[str, Any]) -> dict[str, Any]:
    """Build the versioned JSON document for `--format json`."""
    return {
        "schema_version": SCHEMA_VERSION,
        "matrix": {row.capability: row.states for row in rows},
        "counts": summary["counts"],
        "full_parity": summary["full_parity"],
        "blocking": summary["blocking"],
        "gate_rejected": summary["gate_rejected"],
    }


def _cmd_check(args: argparse.Namespace) -> int:
    manifest = load_manifest(args.root)
    rows = build_matrix(manifest, root=args.root)
    summary = summarize(manifest, rows)
    if args.format == "json":
        emit_line(json.dumps(_matrix_document(rows, summary), indent=2, sort_keys=True))
    else:
        emit_line(render_table(manifest, rows))
        counts = summary["counts"]
        assert isinstance(counts, dict)  # noqa: S101 - narrowing for the type checker.
        emit_line(
            f"pass={counts[PASS]} fail={counts[FAIL]} "
            f"not_implemented={counts[NOT_IMPLEMENTED]} unavailable={counts[UNAVAILABLE]}"
        )
    if args.require_full and not summary["full_parity"]:
        return _report_not_full(summary)
    if args.gate:
        return _gate_verdict(summary)
    return 0


def _report_not_full(summary: dict[str, Any]) -> int:
    """Report the required cells that are not passing and fail the check."""
    blocking = summary["blocking"]
    assert isinstance(blocking, list)  # noqa: S101
    emit_err(
        f"[conformance] full parity is not reached: {len(blocking)} required cell(s) "
        f"are not passing"
    )
    for entry in blocking[:20]:
        emit_err(f"  - {entry}")
    return 1


def _gate_verdict(summary: dict[str, Any]) -> int:
    """Evaluate the CI gate: a cell may be `pass` or `unavailable`, never anything else."""
    rejected = summary["gate_rejected"]
    assert isinstance(rejected, list)  # noqa: S101 - narrowing for the type checker.
    if not rejected:
        emit_line("[conformance] gate OK: no cell failed or went unclaimed.")
        return 0
    emit_err(
        f"[conformance] gate FAILED: {len(rejected)} cell(s) failed or claim nothing in an "
        f"environment that ran the suite"
    )
    for entry in rejected[:20]:
        emit_err(f"  - {entry}")
    emit_err(
        "[conformance] a capability that is `unavailable` here is fine; `fail` is a defect "
        "and `not_implemented` means the suite claims nothing, so running it proves nothing"
    )
    return 1


def build_parser(root: Path) -> argparse.ArgumentParser:
    """The argument parser, shared by the entry point and the tests."""
    parser = argparse.ArgumentParser(prog="conformance")
    parser.add_argument("--root", type=Path, default=root)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("list", help="print the manifest as JSON")
    check = sub.add_parser("check", help="run the suites and print the matrix")
    check.add_argument("--format", choices=("table", "json"), default="table")
    check.add_argument(
        "--require-full",
        action="store_true",
        help="exit 1 unless every required capability passes for every target",
    )
    check.add_argument(
        "--gate",
        action="store_true",
        help="exit 1 if any cell failed or claims nothing (unavailable is allowed)",
    )
    return parser


def main(argv: Sequence[str] | None = None, root: Path = REPO_ROOT) -> int:
    """Entry point for `python -m automation.conformance`."""
    args = build_parser(root).parse_args(argv)
    try:
        if args.command == "list":
            return _cmd_list(args)
        return _cmd_check(args)
    except ManifestError as exc:
        emit_err(f"[conformance] {exc}")
        return 2


if __name__ == "__main__":  # pragma: no cover - module entry point.
    raise SystemExit(main())
