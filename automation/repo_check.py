"""Repository hygiene, vendor integrity, and layout verification checks."""

from __future__ import annotations

import argparse
import hashlib
import shutil
from collections.abc import Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd

REQUIRED_FILES: tuple[str, ...] = (
    "Cargo.toml",
    "Cargo.lock",
    "LICENSE-MIT",
    "LICENSE-APACHE",
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


def check_repository(root: Path = REPO_ROOT, *, skip_shellcheck: bool = False) -> list[str]:
    """Return all repository structure, vendor checksum, and BOM errors."""
    errors = _check_required_and_forbidden(root)
    errors.extend(_check_vendor_integrity(root))
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
    emit_line("[repo-check] OK: repository layout, vendor checksums, and scripts verified.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
