"""Stage published SDK files into `crates/playtest-assets/staged/` for `cargo package`."""

from __future__ import annotations

import argparse
import os
import shutil
import tarfile
import tempfile
import tomllib
from collections.abc import Sequence
from pathlib import Path
from typing import Any

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd

MANIFEST_REL = Path("crates/playtest-assets/assets_manifest.toml")
STAGED_REL = Path("crates/playtest-assets/staged")
FORBIDDEN_SUFFIXES: tuple[str, ...] = (
    "Cargo.lock",
    "bun.lock",
    "pixi.lock",
    "go.sum",
    "biome.json",
    ".Rbuildignore",
    ".DS_Store",
    "_test.go",
)


def load_assets_manifest(root: Path = REPO_ROOT) -> dict[str, dict[str, Any]]:
    """Parse and return the `[languages]` table from `assets_manifest.toml`."""
    manifest_path = root / MANIFEST_REL
    raw = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    languages = raw.get("languages")
    if not isinstance(languages, dict) or not languages:
        raise ValueError(f"{MANIFEST_REL} must define a non-empty [languages] table")
    return {str(key): dict(val) for key, val in languages.items()}


def _check_component_walk(base: Path, rel: Path) -> str | None:
    cursor = base
    for part in rel.parts:
        if part in {"", ".", ".."}:
            return f"unsafe path segment `{part}` in asset path: {rel.as_posix()}"
        cursor = cursor / part
        if cursor.is_symlink():
            return f"symbolic link is forbidden in SDK asset path: {cursor}"
        if not cursor.exists():
            return f"missing listed SDK asset: {cursor}"
    return None if cursor.is_file() else f"listed SDK asset is not a regular file: {cursor}"


def validate_no_symlink_components(base: Path, rel: Path) -> str | None:
    """Return an error string if `base` or any component of `rel` is a symlink or unsafe."""
    if base.is_symlink():
        return f"symbolic link is forbidden at SDK root: {base}"
    if rel.is_absolute() or not rel.parts:
        return f"invalid relative asset path: {rel.as_posix()}"
    return _check_component_walk(base, rel)


def is_forbidden_asset_path(rel_str: str) -> bool:
    """Return True if `rel_str` points to a test, lockfile, cache, or editor artifact."""
    norm = rel_str.replace("\\", "/")
    if norm.startswith("tests/") or "/tests/" in norm or "/src/test/" in f"/{norm}":
        return True
    if "__pycache__" in norm or ".pytest_cache" in norm:
        return True
    return any(norm.endswith(suffix) for suffix in FORBIDDEN_SUFFIXES)


def build_typescript_dist(root: Path = REPO_ROOT) -> int:
    """Compile `typescript/dist/` (`index.js` + `index.d.ts`) via `bunup`."""
    ts_dir = root / "typescript"
    if not (ts_dir / "node_modules" / "bunup").exists():
        install = run_cmd(["bun", "install", "--frozen-lockfile"], cwd=ts_dir)
        if install.returncode != 0:
            return install.returncode
    res = run_cmd(["bun", "run", "build"], cwd=ts_dir)
    return res.returncode


def clean_staged_assets(root: Path = REPO_ROOT) -> None:
    """Remove `crates/playtest-assets/staged/` if present."""
    staged_dir = root / STAGED_REL
    if staged_dir.is_symlink():
        staged_dir.unlink()
    elif staged_dir.exists():
        shutil.rmtree(staged_dir)


def _stage_language_files(
    lang: str, spec: dict[str, Any], root: Path, staged_root: Path
) -> list[Path]:
    source_dir = root / str(spec["source_dir"])
    copied: list[Path] = []
    for rel_str in (str(item) for item in spec.get("files", [])):
        if is_forbidden_asset_path(rel_str):
            raise ValueError(f"forbidden test/lockfile/cache path in {lang}: {rel_str}")
        err = validate_no_symlink_components(source_dir, Path(rel_str))
        if err is not None:
            raise FileNotFoundError(err)
        dst = staged_root / lang / rel_str
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_dir / rel_str, dst, follow_symlinks=False)
        copied.append(dst)
    return copied


def stage_assets(root: Path = REPO_ROOT, *, build_generated: bool = False) -> list[Path]:
    """Copy all manifest-listed SDK files into `crates/playtest-assets/staged/`."""
    if build_generated and build_typescript_dist(root) != 0:
        raise RuntimeError("failed to build typescript/dist before staging SDK assets")
    languages = load_assets_manifest(root)
    clean_staged_assets(root)
    staged_root = root / STAGED_REL
    staged_files: list[Path] = []
    for lang, spec in sorted(languages.items()):
        staged_files.extend(_stage_language_files(lang, spec, root, staged_root))
    return staged_files


def _write_offline_vendor_config(unpack_dir: Path, vendor_dir: Path) -> None:
    cargo_dir = unpack_dir / ".cargo"
    cargo_dir.mkdir(parents=True, exist_ok=True)
    vendor_posix = vendor_dir.resolve().as_posix()
    (cargo_dir / "config.toml").write_text(
        '[source.crates-io]\nreplace-with = "vendored-sources"\n\n'
        f'[source.vendored-sources]\ndirectory = "{vendor_posix}"\n',
        encoding="utf-8",
    )


def verify_packaged_playtest_assets(root: Path, crate_archive: Path) -> int:
    """Unpack `playtest-assets-*.crate` outside `root` and run `cargo test`."""
    if not crate_archive.is_file():
        emit_err(f"[stage-assets] ERROR: missing crate archive {crate_archive}")
        return 1
    vendor_dir = root / ".pixi-sandbox" / "vendor"
    use_offline = vendor_dir.is_dir() or os.environ.get("CARGO_NET_OFFLINE") == "true"
    with tempfile.TemporaryDirectory(prefix="playtest-assets-pkg-") as tmp:
        tmp_path = Path(tmp)
        with tarfile.open(crate_archive, "r:gz") as tar:
            tar.extractall(tmp_path, filter="data")
        unpacked_dirs = [p for p in tmp_path.iterdir() if p.is_dir()]
        if len(unpacked_dirs) != 1:
            emit_err(f"[stage-assets] ERROR: unexpected archive layout in {crate_archive}")
            return 1
        crate_dir = unpacked_dirs[0]
        if vendor_dir.is_dir():
            _write_offline_vendor_config(crate_dir, vendor_dir)
        cmd = ["cargo", "test"]
        if use_offline:
            cmd.append("--offline")
        res = run_cmd(cmd, cwd=crate_dir, timeout=300)
        return res.returncode


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point to stage or clean `playtest-assets` SDK files."""
    parser = argparse.ArgumentParser(description="Stage SDK files for playtest-assets.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT, help="Repository root.")
    parser.add_argument("--clean", action="store_true", help="Remove staged directory.")
    parser.add_argument(
        "--build-generated",
        action="store_true",
        help="Build compiled SDK outputs (typescript/dist) before staging.",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)
    resolved_root = args.root.resolve()
    if args.clean:
        clean_staged_assets(resolved_root)
        emit_line("[stage-assets] cleaned crates/playtest-assets/staged/")
        return 0
    try:
        staged = stage_assets(resolved_root, build_generated=args.build_generated)
    except (FileNotFoundError, RuntimeError, ValueError) as exc:
        emit_err(f"[stage-assets] ERROR: {exc}")
        return 1
    emit_line(f"[stage-assets] staged {len(staged)} SDK files into {STAGED_REL.as_posix()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
