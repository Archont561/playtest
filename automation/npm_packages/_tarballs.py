"""Build platform and wrapper npm tarballs from compiled release binaries."""

from __future__ import annotations

import shutil
import tempfile
import tomllib
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

from automation.npm_packages._spec import NPM_PLATFORMS, NpmPlatformSpec, pack_npm_dir
from automation.npm_packages._staging import (
    stage_wrapper_sdk_and_loader,
    write_platform_package_dir,
)


def _workspace_version(root: Path) -> str:
    cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    return str(cargo["workspace"]["package"]["version"])


@dataclass(frozen=True)
class PackTargets:
    """Destination staging and tarball output directories for one packaging run."""

    staging: Path
    tarballs_dir: Path


@dataclass(frozen=True)
class PackInputs:
    """Source binaries, license, and version used to build npm packages."""

    binaries_by_triple: dict[str, Path]
    license_src: Path
    version: str


def _stage_cargo_npm_binaries(
    root: Path, binaries_by_triple: dict[str, Path], platforms: Sequence[NpmPlatformSpec]
) -> None:
    for spec in platforms:
        src_bin = binaries_by_triple[spec.triple]
        cargo_slot = root / "target" / spec.triple / "release" / spec.binary_filename
        cargo_slot.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src_bin, cargo_slot)


def _pack_platform_tarball(spec: NpmPlatformSpec, targets: PackTargets, inputs: PackInputs) -> Path:
    pkg_dir = write_platform_package_dir(
        targets.staging,
        spec,
        inputs.version,
        inputs.binaries_by_triple[spec.triple],
        inputs.license_src,
    )
    tgz = targets.tarballs_dir / f"archont561-playtest-{spec.suffix}-{inputs.version}.tgz"
    exec_set = frozenset({spec.binary_filename}) if spec.os_name != "win32" else frozenset()
    return pack_npm_dir(pkg_dir, tgz, exec_set)


def _pack_all_platform_tarballs(
    targets: PackTargets, inputs: PackInputs, platforms: Sequence[NpmPlatformSpec]
) -> list[Path]:
    return [_pack_platform_tarball(spec, targets, inputs) for spec in platforms]


def build_npm_tarballs(
    root: Path,
    binaries_by_triple: dict[str, Path],
    tarballs_dir: Path,
    *,
    use_cargo_npm: bool = False,
    platforms: Sequence[NpmPlatformSpec] = NPM_PLATFORMS,
) -> list[Path]:
    """Generate platform + wrapper npm directories and pack tarballs into `tarballs_dir`."""
    version = _workspace_version(root)
    if use_cargo_npm:
        _stage_cargo_npm_binaries(root, binaries_by_triple, platforms)
    staging = root / "npm" if use_cargo_npm else Path(tempfile.mkdtemp(prefix="playtest-npm-gen-"))
    try:
        targets = PackTargets(staging=staging, tarballs_dir=tarballs_dir)
        inputs = PackInputs(
            binaries_by_triple=binaries_by_triple,
            license_src=root / "LICENSE",
            version=version,
        )
        written = _pack_all_platform_tarballs(targets, inputs, platforms)
        w_dir = stage_wrapper_sdk_and_loader(
            root, staging, version, platforms, use_cargo_npm=use_cargo_npm
        )
        w_tgz = tarballs_dir / f"archont561-playtest-{version}.tgz"
        written.append(pack_npm_dir(w_dir, w_tgz, frozenset({"bin/playtest.js"})))
        return written
    finally:
        if staging.exists():
            shutil.rmtree(staging)
