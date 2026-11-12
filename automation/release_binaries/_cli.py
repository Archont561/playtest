"""CLI assembly and dispatch for GitHub Release binary archives."""

from __future__ import annotations

import argparse
import os
import shutil
from collections.abc import Sequence
from pathlib import Path
from typing import Any, Protocol, cast

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd
from automation.release_binaries._archives import package_platform_archive
from automation.release_binaries._platforms import PLATFORM_BY_TRIPLE
from automation.release_binaries._verify import (
    combine_checksums,
    verify_platform_archive,
    verify_version_alignment,
)


def _emit_check_errors(errs: list[str], ok_msg: str | None = None) -> int:
    for err in errs:
        emit_err(f"[release-binaries] ERROR: {err}")
    if not errs and ok_msg is not None:
        emit_line(ok_msg)
    return 1 if errs else 0


def _compile_release_binary(root: Path, triple: str) -> int:
    ts_cmd = [
        "bun",
        "build",
        "typescript/src/index.ts",
        "--outdir",
        "typescript/dist",
        "--target",
        "node",
    ]
    ts_res = run_cmd(ts_cmd, cwd=root)
    if ts_res.returncode != 0:
        return ts_res.returncode
    cargo_cmd = [
        "cargo",
        "build",
        "--release",
        "--locked",
        "-p",
        "playtest-cli",
        "--target",
        triple,
    ]
    return run_cmd(cargo_cmd, cwd=root).returncode


def _build_and_verify_target(
    root: Path,
    triple: str,
    version_or_tag: str,
    raw_dir: Path,
    dist_dir: Path,
) -> int:
    status = _compile_release_binary(root, triple)
    if status != 0:
        return status
    spec = PLATFORM_BY_TRIPLE[triple]
    compiled = root / "target" / triple / "release" / spec.binary_name
    raw_slot = raw_dir / triple / spec.binary_name
    raw_slot.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(compiled, raw_slot)
    package_platform_archive(triple, raw_slot, dist_dir, root=root)
    errs = verify_platform_archive(triple, version_or_tag, dist_dir, execute_binary=True)
    return _emit_check_errors(errs)


class SubParserRegistry(Protocol):
    """Structural view of `argparse._SubParsersAction.add_parser`."""

    def add_parser(self, name: str, **kwargs: Any) -> argparse.ArgumentParser:
        """Register and return a named subcommand parser."""
        ...


def _add_verify_version_parser(sub: SubParserRegistry, default_ver: str) -> None:
    p_ver = sub.add_parser("verify-version", help="Fail on manifest/CLI/asset version drift.")
    p_ver.add_argument("--version", default=default_ver)
    p_ver.add_argument("--cli-bin", type=Path, default=None)


def _add_package_parser(sub: SubParserRegistry, targets: tuple[str, ...]) -> None:
    p_pkg = sub.add_parser("package", help="Package compiled binary + LICENSE + .sha256.")
    p_pkg.add_argument("--target", choices=targets, required=True)
    p_pkg.add_argument("--binary", type=Path, required=True)
    p_pkg.add_argument("--out-dir", type=Path, required=True)


def _add_verify_archive_parser(
    sub: SubParserRegistry, targets: tuple[str, ...], default_ver: str
) -> None:
    p_chk = sub.add_parser("verify-archive", help="Verify .sha256, LICENSE, and playtest version.")
    p_chk.add_argument("--target", choices=targets, required=True)
    p_chk.add_argument("--version", default=default_ver)
    p_chk.add_argument("--dist-dir", type=Path, required=True)
    p_chk.add_argument("--no-exec", action="store_true")


def _add_build_and_checksum_subparsers(sub: SubParserRegistry) -> None:
    targets = tuple(PLATFORM_BY_TRIPLE.keys())
    default_ver = os.environ.get("RELEASE_TAG", "")
    p_bld = sub.add_parser("build-target", help="Compile, package, and verify one CLI target.")
    p_bld.add_argument("--target", choices=targets, required=True)
    p_bld.add_argument("--version", default=default_ver)
    p_bld.add_argument("--raw-dir", type=Path, default=Path("raw-bin"))
    p_bld.add_argument("--dist-dir", type=Path, default=Path("release-assets"))

    p_sum = sub.add_parser("combine-checksums", help="Build combined sha256.sum.")
    p_sum.add_argument("--dist-dir", type=Path, required=True)
    p_sum.add_argument("--output", type=Path, required=True)
    p_sum.add_argument("--allow-subset", action="store_true")


def build_parser(root_default: Path) -> argparse.ArgumentParser:
    """Build the release-binaries argument parser."""
    parser = argparse.ArgumentParser(description="Manage GitHub Release CLI binary archives.")
    parser.add_argument("--root", type=Path, default=root_default)
    sub = cast(SubParserRegistry, parser.add_subparsers(dest="command", required=True))
    targets = tuple(PLATFORM_BY_TRIPLE.keys())
    default_ver = os.environ.get("RELEASE_TAG", "")
    _add_verify_version_parser(sub, default_ver)
    _add_package_parser(sub, targets)
    _add_verify_archive_parser(sub, targets, default_ver)
    _add_build_and_checksum_subparsers(sub)
    return parser


def _dispatch_archive_command(args: argparse.Namespace, root: Path) -> int:
    if args.command == "package":
        archive, sidecar = package_platform_archive(
            args.target, args.binary.resolve(), args.out_dir.resolve(), root=root
        )
        emit_line(f"[release-binaries] packaged {archive.name} and {sidecar.name}")
        return 0
    errs = verify_platform_archive(
        args.target, args.version, args.dist_dir.resolve(), execute_binary=not args.no_exec
    )
    return _emit_check_errors(errs, f"[release-binaries] OK: verified archive for {args.target}")


def _dispatch_subcommand(args: argparse.Namespace, root: Path) -> int:
    if args.command == "build-target":
        raw_dir, dist_dir = args.raw_dir.resolve(), args.dist_dir.resolve()
        return _build_and_verify_target(root, args.target, args.version, raw_dir, dist_dir)
    if args.command == "verify-version":
        errs = verify_version_alignment(args.version, root=root, cli_bin=args.cli_bin)
        return _emit_check_errors(errs, "[release-binaries] OK: version alignment verified.")
    if args.command in {"package", "verify-archive"}:
        return _dispatch_archive_command(args, root)
    out = combine_checksums(
        args.dist_dir.resolve(), args.output.resolve(), require_all_platforms=not args.allow_subset
    )
    emit_line(f"[release-binaries] wrote {out}")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    """Parse `argv` and dispatch one release-binaries subcommand."""
    args = build_parser(REPO_ROOT).parse_args(list(argv) if argv is not None else None)
    try:
        return _dispatch_subcommand(args, args.root.resolve())
    except (FileNotFoundError, KeyError, ValueError, shutil.Error) as err:
        emit_err(f"[release-binaries] ERROR: {err}")
        return 1
