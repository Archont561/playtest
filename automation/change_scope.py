"""Compute affected CI lanes from git diffs so PRs run only impacted SDK suites."""

from __future__ import annotations

import argparse
import json
from collections.abc import Iterable, Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd, write_github_output

LANE_NAMES: tuple[str, ...] = ("rust", "py", "ts", "cpp", "c", "csharp", "go", "java", "r")
DOC_ONLY_PREFIXES: tuple[str, ...] = ("backlog/", "docs/")
GLOBAL_TRIGGERS: tuple[str, ...] = (
    "pixi.toml",
    "pixi.lock",
    ".github/workflows/ci.yml",
    "automation/",
)
LANE_PREFIXES: dict[str, tuple[str, ...]] = {
    "rust": ("Cargo.toml", "Cargo.lock", "crates/", "deny.toml", ".cargo/"),
    "py": ("python/",),
    "ts": ("typescript/",),
    "cpp": ("cpp/",),
    "c": ("c/",),
    "csharp": ("csharp/",),
    "go": ("go/",),
    "java": ("java/",),
    "r": ("r/",),
}


def _is_doc_only(rel_path: str) -> bool:
    if rel_path.endswith(".md"):
        return True
    return any(rel_path.startswith(prefix) for prefix in DOC_ONLY_PREFIXES)


def classify_paths(paths: Iterable[str], *, force_all: bool = False) -> dict[str, str]:
    """Map changed relative paths to `'true'` / `'false'` flags and active SDK list."""
    sdk_lanes = tuple(lane for lane in LANE_NAMES if lane != "rust")
    if force_all:
        flags = dict.fromkeys(LANE_NAMES, "true")
        flags["any_code"] = "true"
        flags["has_sdks"] = "true"
        flags["sdks"] = json.dumps(list(sdk_lanes))
        return flags

    active: set[str] = set()
    saw_code = False
    for raw in paths:
        rel = raw.strip().lstrip("./")
        if not rel or _is_doc_only(rel):
            continue
        saw_code = True
        if any(rel == trig or rel.startswith(trig) for trig in GLOBAL_TRIGGERS):
            active.update(LANE_NAMES)
            continue
        matched = False
        for lane, prefixes in LANE_PREFIXES.items():
            if any(rel == p or rel.startswith(p) for p in prefixes):
                active.add(lane)
                matched = True
        if not matched:
            active.add("rust")

    result = {lane: ("true" if lane in active else "false") for lane in LANE_NAMES}
    active_sdks = [lane for lane in sdk_lanes if lane in active]
    result["any_code"] = "true" if (saw_code or active) else "false"
    result["has_sdks"] = "true" if active_sdks else "false"
    result["sdks"] = json.dumps(active_sdks)
    return result


def diff_paths(base_ref: str, head_ref: str = "HEAD", *, root: Path = REPO_ROOT) -> list[str]:
    """Return changed relative paths between `base_ref` and `head_ref` via git diff."""
    res = run_cmd(
        ["git", "diff", "--name-only", f"{base_ref}...{head_ref}"],
        cwd=root,
        timeout=60,
        capture=True,
    )
    if res.returncode != 0:
        fallback = run_cmd(
            ["git", "diff", "--name-only", base_ref, head_ref],
            cwd=root,
            timeout=60,
            capture=True,
        )
        if fallback.returncode != 0:
            raise RuntimeError(f"git diff failed: {fallback.stderr.strip()}")
        return [line.strip() for line in fallback.stdout.splitlines() if line.strip()]
    return [line.strip() for line in res.stdout.splitlines() if line.strip()]


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for CI lane change-scope computation."""
    parser = argparse.ArgumentParser(description="Compute affected CI lanes.")
    parser.add_argument("--base", default="", help="Base git ref or SHA for PR diff.")
    parser.add_argument("--head", default="HEAD", help="Head git ref or SHA.")
    parser.add_argument("--all", action="store_true", dest="force_all", help="Enable all lanes.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT, help="Repository root path.")
    args = parser.parse_args(list(argv) if argv is not None else None)

    force_all = bool(args.force_all or not args.base.strip())
    changed: list[str] = []
    if not force_all:
        try:
            changed = diff_paths(args.base.strip(), args.head.strip(), root=args.root.resolve())
        except RuntimeError as err:
            emit_err(f"[change-scope] WARN: {err}; enabling all lanes as safe fallback.")
            force_all = True

    outputs = classify_paths(changed, force_all=force_all)
    write_github_output(outputs)
    for key, val in outputs.items():
        emit_line(f"{key}={val}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
