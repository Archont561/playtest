"""Repository hygiene, vendor integrity, dependency-boundary, and asset manifest checks."""

from __future__ import annotations

import argparse
import hashlib
import shutil
import tomllib
from collections import deque
from collections.abc import Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd
from automation.stage_assets import (
    MANIFEST_REL,
    is_forbidden_asset_path,
    load_assets_manifest,
    validate_no_symlink_components,
)
from automation.stamp_version import read_manifest_versions

REQUIRED_FILES: tuple[str, ...] = (
    "Cargo.toml",
    "Cargo.lock",
    "LICENSE",
    "README.md",
    "AGENTS.md",
    "CONTRIBUTING.md",
    "pixi.toml",
    "pixi.lock",
    "pixi-sandbox.toml",
    "deny.toml",
    ".cargo/audit.toml",
    "doit.cfg",
    "rude.toml",
    "ruff.toml",
    "mypy.ini",
    "taplo.toml",
    "lefthook.yml",
    "package.json",
    "bun.lock",
    "scripts/restore.sh",
    "automation/stage_assets.py",
    "crates/playtest-assets/assets_manifest.toml",
    "go/VERSION",
    "java/src/main/resources/io/github/archont561/playtest/version.properties",
    "vendor/clean-code-presets/UPSTREAM.pin",
    "vendor/clean-code-presets/VENDOR.sha256",
)

FORBIDDEN_PATHS: tuple[str, ...] = (
    "cpp/test_cov",
    "java/src/main/java/io/github/archont561/playtest/Cover2Lcov.java",
    "python/pixi.toml",
    "typescript/pixi.toml",
    "cpp/pixi.toml",
    "c/pixi.toml",
    "csharp/pixi.toml",
    "go/pixi.toml",
    "java/pixi.toml",
    "r/pixi.toml",
)


def _check_required_and_forbidden(root: Path) -> list[str]:
    errors: list[str] = []
    for rel in REQUIRED_FILES:
        if not (root / rel).is_file():
            errors.append(f"missing required file: {rel}")
    for rel in FORBIDDEN_PATHS:
        if (root / rel).exists():
            errors.append(f"forbidden legacy path exists: {rel}")
    scripts_dir = root / "scripts"
    if scripts_dir.is_dir():
        for script in sorted(scripts_dir.iterdir()):
            if script.name != "restore.sh":
                errors.append(f"unexpected file in scripts/ (use automation/): {script.name}")
    return errors


def _verify_vendor_hashes(vendor_dir: Path, manifest_text: str) -> tuple[set[str], list[str]]:
    errors: list[str] = []
    recorded: set[str] = set()
    for raw_line in manifest_text.splitlines():
        if not raw_line.strip():
            continue
        expected_hash, _, rel_path = raw_line.strip().partition("  ")
        recorded.add(rel_path)
        target = vendor_dir / rel_path
        if not target.is_file():
            errors.append(f"missing vendored file: {rel_path}")
        elif hashlib.sha256(target.read_bytes()).hexdigest() != expected_hash:
            errors.append(f"checksum mismatch in vendor/clean-code-presets/{rel_path}")
    return recorded, errors


def _check_vendor_integrity(root: Path) -> list[str]:
    vendor_dir = root / "vendor" / "clean-code-presets"
    pin_file = vendor_dir / "UPSTREAM.pin"
    manifest = vendor_dir / "VENDOR.sha256"
    if not pin_file.is_file() or not manifest.is_file():
        return ["vendored clean-code presets pin or checksum manifest is missing"]
    pin = pin_file.read_text(encoding="utf-8").strip()
    if len(pin) != 40 or any(ch not in "0123456789abcdef" for ch in pin):
        return [f"invalid 40-char SHA in UPSTREAM.pin: {pin!r}"]
    recorded, errors = _verify_vendor_hashes(vendor_dir, manifest.read_text(encoding="utf-8"))
    actual = {
        p.relative_to(vendor_dir).as_posix()
        for p in vendor_dir.rglob("*")
        if p.is_file() and p.name != "VENDOR.sha256"
    }
    errors.extend(
        f"untracked file in vendor/clean-code-presets: {u}" for u in sorted(actual - recorded)
    )
    return errors


def _reachable_crates(graph: dict[str, set[str]], start: str) -> set[str]:
    visited: set[str] = set()
    queue: deque[str] = deque(graph.get(start, ()))
    while queue:
        node = queue.popleft()
        if node in visited:
            continue
        visited.add(node)
        queue.extend(child for child in graph.get(node, ()) if child not in visited)
    return visited


def _check_crate_dependency_boundaries(root: Path) -> list[str]:
    lock_path = root / "Cargo.lock"
    if not lock_path.is_file():
        return []
    lock_data = tomllib.loads(lock_path.read_text(encoding="utf-8"))
    graph: dict[str, set[str]] = {}
    for pkg in lock_data.get("package", []):
        name = str(pkg.get("name", ""))
        deps = {str(raw).split()[0] for raw in pkg.get("dependencies", []) if str(raw).strip()}
        graph.setdefault(name, set()).update(deps)
    errors: list[str] = []
    for lib_crate in ("playtest", "playtest-core", "playtest-macros"):
        closure = _reachable_crates(graph, lib_crate)
        for forbidden in ("playtest-assets", "playtest-cli", "clifacet"):
            if forbidden in closure:
                errors.append(
                    f"Spec 0012 §1 violation: `{lib_crate}` depends on `{forbidden}` in Cargo.lock"
                )
    for non_cli in ("playtest", "playtest-core", "playtest-macros", "playtest-assets"):
        direct = graph.get(non_cli, set())
        for cli_only in ("playtest-assets", "clifacet"):
            if cli_only in direct:
                errors.append(
                    f"Spec 0012 §1 violation: only `playtest-cli` may depend on `{cli_only}` "
                    f"(found on `{non_cli}`)"
                )
    return errors


def _check_assets_manifest(root: Path) -> list[str]:
    if not (root / MANIFEST_REL).is_file():
        return []
    try:
        languages = load_assets_manifest(root)
    except ValueError as exc:
        return [str(exc)]
    errors: list[str] = []
    for lang, spec in sorted(languages.items()):
        source_dir = root / str(spec.get("source_dir", ""))
        generated = {str(item) for item in spec.get("generated", [])}
        for rel_str in (str(item) for item in spec.get("files", [])):
            if is_forbidden_asset_path(rel_str):
                errors.append(f"forbidden test/lockfile/cache asset in {lang}: {rel_str}")
                continue
            if rel_str in generated and not (source_dir / rel_str).exists():
                continue
            err = validate_no_symlink_components(source_dir, Path(rel_str))
            if err is not None:
                errors.append(f"invalid SDK asset in {lang}: {err}")
    return errors


def _check_utf8_bom(root: Path) -> list[str]:
    errors: list[str] = []
    patterns = ("*.toml", "*.yml", "*.yaml", "*.json", "*.csproj", "*.xml")
    for pattern in patterns:
        for path in sorted(root.rglob(pattern)):
            rel_parts = path.relative_to(root).parts
            if rel_parts and rel_parts[0] in {
                ".git",
                ".pixi",
                ".pixi-sandbox",
                "node_modules",
                "target",
            }:
                continue
            if path.read_bytes().startswith(b"\xef\xbb\xbf"):
                errors.append(f"UTF-8 BOM is not permitted: {path.relative_to(root).as_posix()}")
    return errors


def _check_restore_shell(root: Path, *, skip_shellcheck: bool) -> list[str]:
    if skip_shellcheck:
        return []
    if shutil.which("shellcheck") is None:
        return ["shellcheck binary is not available on PATH"]
    res = run_cmd(
        ["shellcheck", "-e", "SC1087", "scripts/restore.sh"], cwd=root, timeout=60, capture=True
    )
    if res.returncode != 0:
        detail = (res.stdout + res.stderr).strip()
        return [f"shellcheck failed on scripts/restore.sh:\n{detail}"]
    return []


SDK_AUTODISCOVERY_SOURCES: tuple[str, ...] = (
    "crates/playtest/src/lib.rs",
    "crates/playtest-core/src/lib.rs",
    "crates/playtest-macros/src/lib.rs",
    "crates/playtest-assets/src/lib.rs",
    "crates/playtest-cli/src/lib.rs",
    "python/src/playtest/__init__.py",
    "typescript/src/index.ts",
    "typescript/src/version.ts",
    "cpp/include/playtest/playtest.hpp",
    "cpp/conanfile.py",
    "c/include/playtest/playtest.h",
    "c/conanfile.py",
    "csharp/src/Playtest/Playtest.cs",
    "go/playtest.go",
    "java/src/main/java/io/github/archont561/playtest/Playtest.java",
    "r/R/version.R",
)


def _check_manifest_versions_and_autodiscovery(root: Path) -> list[str]:
    try:
        versions = read_manifest_versions(root)
    except (FileNotFoundError, KeyError, ValueError) as exc:
        return [f"failed to read manifest versions: {exc}"]
    canonical = versions["rust"]
    errors: list[str] = []
    for target, ver in sorted(versions.items()):
        expected = canonical.split("-", 1)[0] if target == "r" else canonical
        if ver != expected:
            errors.append(
                f"manifest version drift in `{target}`: expected {expected!r}, found {ver!r}"
            )
    quoted_ver = f'"{canonical}"'
    for rel in SDK_AUTODISCOVERY_SOURCES:
        src_path = root / rel
        if src_path.is_file() and quoted_ver in src_path.read_text(encoding="utf-8"):
            errors.append(
                f"hardcoded version literal {quoted_ver} in {rel} "
                "(derive VERSION from the module manifest instead)"
            )
    return errors


def check_repository(root: Path = REPO_ROOT, *, skip_shellcheck: bool = False) -> list[str]:
    """Return all repository structure, vendor checksum, boundary, and BOM errors."""
    errors = _check_required_and_forbidden(root)
    errors.extend(_check_vendor_integrity(root))
    errors.extend(_check_crate_dependency_boundaries(root))
    errors.extend(_check_assets_manifest(root))
    errors.extend(_check_manifest_versions_and_autodiscovery(root))
    errors.extend(_check_utf8_bom(root))
    errors.extend(_check_restore_shell(root, skip_shellcheck=skip_shellcheck))
    return errors


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for repository hygiene verification."""
    parser = argparse.ArgumentParser(description="Verify Playtest repository hygiene.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT, help="Repository root path.")
    parser.add_argument("--skip-shellcheck", action="store_true", help="Skip shellcheck step.")
    args = parser.parse_args(list(argv) if argv is not None else None)
    errors = check_repository(args.root.resolve(), skip_shellcheck=args.skip_shellcheck)
    if errors:
        for err in errors:
            emit_err(f"[repo-check] ERROR: {err}")
        return 1
    emit_line("[repo-check] OK: repository layout, boundaries, assets, and scripts verified.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
