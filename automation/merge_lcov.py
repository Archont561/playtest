"""Merge per-language LCOV tracefiles into a normalized root coverage/lcov.info."""

from __future__ import annotations

import argparse
from collections.abc import Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line

DEFAULT_SOURCES: tuple[tuple[str, str], ...] = (
    ("", "coverage/lcov-rs.info"),
    ("python", "python/coverage/lcov-py.info"),
    ("typescript", "typescript/coverage/lcov.info"),
    ("cpp", "cpp/coverage/lcov-cpp.info"),
    ("c", "c/coverage/lcov.info"),
    ("csharp", "csharp/coverage/lcov.info"),
    ("go", "go/coverage/lcov.info"),
    ("java", "java/coverage/lcov.info"),
)


def _rewrite_sf_record(raw_text: str, prefix: str, root: Path) -> list[str]:
    lines: list[str] = []
    root_prefix = f"{root.as_posix()}/"
    for raw_line in raw_text.splitlines():
        if raw_line.startswith("SF:"):
            sf_val = raw_line[3:].strip().replace("\\", "/")
            if sf_val.startswith(root_prefix):
                sf_val = sf_val[len(root_prefix) :]
            elif prefix and not sf_val.startswith(f"{prefix}/") and not sf_val.startswith("/"):
                sf_val = f"{prefix}/{sf_val.lstrip('./')}"
            lines.append(f"SF:{sf_val}")
        else:
            lines.append(raw_line)
    return lines


def _append_candidate(
    candidate: Path,
    prefix: str,
    root: Path,
    resolved_out: Path,
    merged_lines: list[str],
) -> int:
    if not candidate.is_file() or candidate.resolve() == resolved_out.resolve():
        return 0
    content = candidate.read_text(encoding="utf-8").strip()
    if not content:
        return 0
    merged_lines.extend(_rewrite_sf_record(content, prefix, root))
    return 1


def _expand_extra_paths(root: Path, extra_inputs: Sequence[Path]) -> list[Path]:
    expanded: list[Path] = []
    for extra in extra_inputs:
        base_extra = extra if extra.is_absolute() else (root / extra)
        if base_extra.is_dir():
            expanded.extend(sorted(base_extra.rglob("*.info")))
        else:
            expanded.append(base_extra)
    return expanded


def merge_lcov_files(
    root: Path = REPO_ROOT,
    output: Path | None = None,
    extra_inputs: Sequence[Path] = (),
    *,
    require_any: bool = True,
) -> tuple[int, Path]:
    """Merge discovered LCOV inputs into `output`, returning `(file_count, output_path)`."""
    resolved_out = output if output is not None else (root / "coverage" / "lcov.info")
    merged_lines: list[str] = []
    merged_count = 0

    for prefix, rel_path in DEFAULT_SOURCES:
        merged_count += _append_candidate(root / rel_path, prefix, root, resolved_out, merged_lines)

    for candidate in _expand_extra_paths(root, extra_inputs):
        merged_count += _append_candidate(candidate, "", root, resolved_out, merged_lines)

    if merged_count == 0 and require_any:
        raise FileNotFoundError("no non-empty LCOV tracefiles were found to merge")

    resolved_out.parent.mkdir(parents=True, exist_ok=True)
    payload = "\n".join(merged_lines) + ("\n" if merged_lines else "")
    resolved_out.write_text(payload, encoding="utf-8")
    return merged_count, resolved_out


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for polyglot LCOV tracefile merging."""
    parser = argparse.ArgumentParser(description="Merge LCOV coverage files.")
    parser.add_argument("-o", "--output", type=Path, default=Path("coverage/lcov.info"))
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    parser.add_argument("--allow-empty", action="store_true")
    parser.add_argument("inputs", nargs="*", type=Path)
    args = parser.parse_args(list(argv) if argv is not None else None)

    root = args.root.resolve()
    out_path = args.output if args.output.is_absolute() else (root / args.output)
    try:
        count, written = merge_lcov_files(
            root=root,
            output=out_path,
            extra_inputs=args.inputs,
            require_any=not args.allow_empty,
        )
    except FileNotFoundError as err:
        emit_err(f"[merge-lcov] ERROR: {err}")
        return 1
    emit_line(f"[merge-lcov] OK: merged {count} LCOV file(s) -> {written.relative_to(root)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
