"""CLI entry point for @archont561/playtest npm package generation and verification."""

from __future__ import annotations

import argparse
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line
from automation.npm_packages._spec import NPM_PLATFORMS
from automation.npm_packages._tarballs import build_npm_tarballs
from automation.npm_packages._verify import verify_clean_installs


def _cmd_pack_release(args: argparse.Namespace, root: Path) -> int:
    binaries: dict[str, Path] = {}
    for spec in NPM_PLATFORMS:
        candidate = args.binaries_dir.resolve() / spec.triple / spec.binary_filename
        if not candidate.is_file():
            emit_err(f"[npm-packages] ERROR: missing Phase 1 binary for {spec.triple}: {candidate}")
            return 1
        binaries[spec.triple] = candidate
    written = build_npm_tarballs(
        root, binaries, args.out_dir.resolve(), use_cargo_npm=args.use_cargo_npm
    )
    for tgz in written:
        emit_line(f"[npm-packages] packed {tgz}")
    return 0


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Build and verify Playtest npm packages.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    sub = parser.add_subparsers(dest="command", required=True)

    p_pack = sub.add_parser("pack-release", help="Pack all 5 platform tarballs + wrapper tarball.")
    p_pack.add_argument("--binaries-dir", type=Path, required=True)
    p_pack.add_argument("--out-dir", type=Path, required=True)
    p_pack.add_argument("--use-cargo-npm", action="store_true")

    p_ver = sub.add_parser("verify-clean-installs", help="Test npm, pnpm, and Bun installs.")
    p_ver.add_argument("--cli-bin", type=Path, required=True)
    return parser


def run(args: argparse.Namespace) -> int:
    """Dispatch a parsed npm-packages subcommand."""
    root = args.root.resolve()
    if args.command == "pack-release":
        return _cmd_pack_release(args, root)
    return verify_clean_installs(root, args.cli_bin)


def main(argv: list[str] | None = None) -> int:
    """CLI entry point for npm package generation and clean-install verification."""
    return run(_build_parser().parse_args(argv))
