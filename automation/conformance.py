"""Run the cross-language conformance suites and report the parity matrix (task_023).

The suite of record is `backlog/conformance.yml`: per language target, the capabilities
that target claims and the command that proves them. Two rules keep the report honest: a
cell is `pass` only when a suite ran, exited zero, and claimed that capability, and a
target whose toolchain is absent is `unavailable` rather than `fail`. An unclaimed cell is
`not_implemented`, which never renders as a pass.
"""

from __future__ import annotations

import argparse
import json
import shutil
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import CommandResult, emit_err, emit_line, run_cmd

try:  # Python 3.11+
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - the py environment ships 3.12
    tomllib = None  # type: ignore[assignment]

MANIFEST_REL = Path("backlog/conformance.yml")
SCHEMA_VERSION = 1

# Statuses of one matrix cell.
PASS = "pass"
FAIL = "fail"
NOT_IMPLEMENTED = "not_implemented"
UNAVAILABLE = "unavailable"

# Spec 0005 names these nine targets; a manifest entry for anything else is rejected.
KNOWN_LANGS: tuple[str, ...] = (
    "rust",
    "python",
    "typescript",
    "cpp",
    "c",
    "csharp",
    "go",
    "java",
    "r",
)


class ManifestError(ValueError):
    """The conformance manifest is missing, malformed, or names something unknown."""


@dataclass(frozen=True)
class Capability:
    """One requirement of Spec 0005 §1."""

    id: str
    title: str
    spec: str
    optional: bool = False


@dataclass(frozen=True)
class Suite:
    """One language target's claim of conformance and the command that proves it."""

    lang: str
    target: str
    cwd: str
    cmd: tuple[str, ...]
    capabilities: tuple[str, ...]


@dataclass(frozen=True)
class Manifest:
    """The parsed conformance manifest."""

    schema_version: int
    capabilities: tuple[Capability, ...]
    suites: tuple[Suite, ...]


@dataclass
class CellResult:
    """The outcome for every target of one capability."""

    capability: str
    states: dict[str, str] = field(default_factory=dict)


def _require(condition: object, message: str) -> None:
    if not condition:
        raise ManifestError(message)


def _as_mapping(value: object, what: str) -> dict[str, object]:
    """Return `value` as a string-keyed mapping or raise `ManifestError`."""
    if not isinstance(value, dict):
        raise ManifestError(f"{what} must be a mapping")
    return {str(key): item for key, item in value.items()}


def _as_list(value: object, what: str) -> list[object]:
    """Return `value` as a list or raise `ManifestError`."""
    if not isinstance(value, list):
        raise ManifestError(f"{what} must be a list")
    return list(value)


def _as_string_list(value: object, what: str, *, non_empty: bool) -> list[str]:
    """Return `value` as a list of strings or raise `ManifestError`."""
    _require(all(isinstance(item, str) for item in _as_list(value, what)), f"{what}: strings only")
    items = [item for item in _as_list(value, what) if isinstance(item, str)]
    if non_empty:
        _require(items, f"{what} must not be empty")
    return items


def _parse_capability(entry: object) -> Capability:
    """Validate one `[[capabilities]]` entry."""
    item = _as_mapping(entry, "each capability")
    cap_id = item.get("id")
    _require(isinstance(cap_id, str) and bool(cap_id), "capability id must be a non-empty string")
    optional = item.get("optional", False)
    _require(isinstance(optional, bool), f"capability {cap_id}: optional must be a boolean")
    return Capability(
        id=str(cap_id),
        title=str(item.get("title", "")),
        spec=str(item.get("spec", "")),
        optional=bool(optional),
    )


def _parse_suite(entry: object, known: set[str], seen: set[str]) -> Suite:
    """Validate one `[[suites]]` entry against the declared capabilities."""
    item = _as_mapping(entry, "each suite")
    lang = item.get("lang")
    _require(lang in KNOWN_LANGS, f"unknown language target: {lang!r}")
    _require(lang not in seen, f"duplicate suite for language target: {lang}")
    seen.add(str(lang))
    cmd = _as_string_list(item.get("cmd"), f"suite {lang}: cmd", non_empty=True)
    claimed = _as_string_list(
        item.get("capabilities", []), f"suite {lang}: capabilities", non_empty=False
    )
    for cap in claimed:
        _require(cap in known, f"suite {lang} claims unknown capability: {cap!r}")
    return Suite(
        lang=str(lang),
        target=str(item.get("target", "")),
        cwd=str(item.get("cwd", ".")),
        cmd=tuple(cmd),
        capabilities=tuple(claimed),
    )


def parse_manifest(raw: object) -> Manifest:
    """Validate a decoded manifest and return it, or raise `ManifestError`."""
    document = _as_mapping(raw, "manifest")

    version = document.get("schema_version")
    _require(
        isinstance(version, int) and version == SCHEMA_VERSION,
        f"schema_version must be {SCHEMA_VERSION}, got {version!r}",
    )
    if not isinstance(version, int):  # pragma: no cover - the guard above raises.
        raise ManifestError("schema_version must be an integer")

    raw_caps = _as_list(document.get("capabilities"), "capabilities")
    _require(raw_caps, "capabilities must be a non-empty list")
    capabilities = [_parse_capability(entry) for entry in raw_caps]
    known = {cap.id for cap in capabilities}
    _require(len(known) == len(capabilities), "capability ids must be unique")

    raw_suites = _as_list(document.get("suites", []), "suites")
    seen: set[str] = set()
    suites = [_parse_suite(entry, known, seen) for entry in raw_suites]

    return Manifest(
        schema_version=version,
        capabilities=tuple(capabilities),
        suites=tuple(suites),
    )


def load_manifest(root: Path = REPO_ROOT) -> Manifest:
    """Read and validate the manifest from `root`."""
    path = root / MANIFEST_REL
    if not path.is_file():
        raise ManifestError(f"missing conformance manifest: {MANIFEST_REL.as_posix()}")
    if tomllib is None:  # pragma: no cover - the py environment ships 3.12
        raise ManifestError("tomllib is unavailable; Python 3.11 or newer is required")
    import tomllib as parser

    try:
        raw = parser.loads(path.read_text(encoding="utf-8"))
    except parser.TOMLDecodeError as exc:
        raise ManifestError(f"{MANIFEST_REL.as_posix()}: {exc}") from exc
    return parse_manifest(raw)


def _suite_env(root: Path) -> dict[str, str]:
    """Environment for a suite, matching what a Pixi run would provide.

    A cargo suite must use the restored vendored registry. Without `CARGO_HOME` cargo
    reaches for the network, spends minutes on retries, and exits with a metadata failure
    that looks like a conformance failure. Point it at the same tree the Pixi activation
    hook uses when one is present.
    """
    env = {"PYTHONPATH": str(root / "python" / "src")}
    vendor_home = root / ".pixi-sandbox" / "cargo-home"
    if vendor_home.is_dir():
        env["CARGO_HOME"] = str(vendor_home)
    return env


def _tool_available(cmd: Sequence[str], root: Path, suite: Suite) -> bool:
    """Whether the suite's runner exists, resolved from `PATH` and the suite cwd."""
    del root  # resolution is relative to PATH; cwd only matters for the process.
    runner = cmd[0]
    if Path(runner).is_absolute() or "/" in runner:
        return (Path(suite.cwd) / runner).exists()
    return shutil.which(runner) is not None


def run_suite(
    suite: Suite,
    *,
    root: Path,
    run: Callable[..., CommandResult] = run_cmd,
    timeout: int = 900,
) -> str:
    """Run one suite and return its cell status for every capability it claims."""
    cwd = root / suite.cwd
    if not cwd.is_dir():
        emit_err(f"[conformance] {suite.lang}: missing directory {suite.cwd}")
        return FAIL
    if not _tool_available(list(suite.cmd), root, suite):
        emit_err(
            f"[conformance] {suite.lang}: runner `{suite.cmd[0]}` is not installed; "
            f"install the toolchain to claim this row"
        )
        return UNAVAILABLE
    res = run(list(suite.cmd), cwd=cwd, capture=True, timeout=timeout, env=_suite_env(root))
    if res.returncode == 0:
        emit_line(f"[conformance] {suite.lang}: passed ({suite.target})")
        return PASS
    detail = (res.stderr or res.stdout or "").strip()
    emit_err(f"[conformance] {suite.lang}: FAILED ({suite.target}, exit {res.returncode})")
    if detail:
        emit_err(detail)
    return FAIL


def build_matrix(
    manifest: Manifest,
    *,
    root: Path,
    run: Callable[..., CommandResult] = run_cmd,
) -> list[CellResult]:
    """Run every suite once and fold the outcomes into the capability matrix."""
    suite_status = {suite.lang: run_suite(suite, root=root, run=run) for suite in manifest.suites}
    claimed = {suite.lang: suite.capabilities for suite in manifest.suites}
    rows: list[CellResult] = []
    for cap in manifest.capabilities:
        states: dict[str, str] = {}
        for lang in KNOWN_LANGS:
            if lang not in suite_status:
                states[lang] = NOT_IMPLEMENTED
            elif cap.id not in claimed[lang]:
                states[lang] = NOT_IMPLEMENTED
            else:
                states[lang] = suite_status[lang]
        rows.append(CellResult(capability=cap.id, states=states))
    return rows


def summarize(manifest: Manifest, rows: Sequence[CellResult]) -> dict[str, object]:
    """Roll the matrix up into counts and the full-parity verdict."""
    required = [cap.id for cap in manifest.capabilities if not cap.optional]
    counts = {PASS: 0, FAIL: 0, NOT_IMPLEMENTED: 0, UNAVAILABLE: 0}
    blocking: list[str] = []
    for row in rows:
        for lang, state in row.states.items():
            counts[state] = counts.get(state, 0) + 1
            if state != PASS and row.capability in required:
                blocking.append(f"{lang}/{row.capability}={state}")
    return {
        "schema_version": SCHEMA_VERSION,
        "counts": counts,
        "full_parity": not blocking,
        "blocking": blocking,
        "targets": list(KNOWN_LANGS),
        "capabilities": [cap.id for cap in manifest.capabilities],
    }


def _render_table(manifest: Manifest, rows: Sequence[CellResult]) -> None:
    marks = {PASS: "pass", FAIL: "FAIL", NOT_IMPLEMENTED: " -  ", UNAVAILABLE: "n/a "}
    header = "capability".ljust(32) + " ".join(lang[:4].rjust(4) for lang in KNOWN_LANGS)
    emit_line(header)
    for row in rows:
        cells = " ".join(marks[row.states[lang]].rjust(4) for lang in KNOWN_LANGS)
        optional = any(c.id == row.capability and c.optional for c in manifest.capabilities)
        suffix = " (optional)" if optional else ""
        emit_line(row.capability.ljust(32) + cells + suffix)


def _cmd_list(args: argparse.Namespace) -> int:
    manifest = load_manifest(args.root)
    document = {
        "schema_version": SCHEMA_VERSION,
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
    emit_line(json.dumps(document, indent=2, sort_keys=True))
    return 0


def _matrix_document(rows: Sequence[CellResult], summary: dict[str, object]) -> dict[str, object]:
    """Build the versioned JSON document for `--format json`."""
    return {
        "schema_version": SCHEMA_VERSION,
        "matrix": {row.capability: row.states for row in rows},
        "counts": summary["counts"],
        "full_parity": summary["full_parity"],
        "blocking": summary["blocking"],
    }


def _cmd_check(args: argparse.Namespace) -> int:
    manifest = load_manifest(args.root)
    rows = build_matrix(manifest, root=args.root)
    summary = summarize(manifest, rows)
    if args.format == "json":
        emit_line(json.dumps(_matrix_document(rows, summary), indent=2, sort_keys=True))
    else:
        _render_table(manifest, rows)
        counts = summary["counts"]
        assert isinstance(counts, dict)  # noqa: S101 - narrowing for the type checker.
        emit_line(
            f"pass={counts[PASS]} fail={counts[FAIL]} "
            f"not_implemented={counts[NOT_IMPLEMENTED]} unavailable={counts[UNAVAILABLE]}"
        )
    if args.require_full and not summary["full_parity"]:
        blocking = summary["blocking"]
        assert isinstance(blocking, list)  # noqa: S101
        emit_err(
            f"[conformance] full parity is not reached: {len(blocking)} required cell(s) "
            f"are not passing"
        )
        for entry in blocking[:20]:
            emit_err(f"  - {entry}")
        return 1
    return 0


def main(argv: Sequence[str] | None = None, root: Path = REPO_ROOT) -> int:
    """Entry point for `python -m automation.conformance`."""
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
    args = parser.parse_args(argv)
    try:
        if args.command == "list":
            return _cmd_list(args)
        return _cmd_check(args)
    except ManifestError as exc:
        emit_err(f"[conformance] {exc}")
        return 2


if __name__ == "__main__":  # pragma: no cover - module entry point.
    raise SystemExit(main())
