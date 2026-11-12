"""Platform and wrapper package directory staging for npm release tarballs."""

from __future__ import annotations

import json
import shutil
from collections.abc import Sequence
from pathlib import Path

from automation._proc import run_cmd
from automation.npm_packages._spec import NpmPlatformSpec


def _platform_manifest(spec: NpmPlatformSpec, version: str) -> dict[str, object]:
    return {
        "name": spec.package_name,
        "version": version,
        "description": f"Playtest CLI binary for {spec.os_name}-{spec.cpu_name}",
        "license": "MIT",
        "homepage": "https://github.com/Archont561/playtest",
        "repository": {"type": "git", "url": "git+https://github.com/Archont561/playtest.git"},
        "os": [spec.os_name],
        "cpu": [spec.cpu_name],
        "files": [spec.binary_filename, "LICENSE"],
        "publishConfig": {"access": "public"},
    }


def _stage_platform_binary(pkg_dir: Path, spec: NpmPlatformSpec, binary_src: Path) -> None:
    dest_bin = pkg_dir / spec.binary_filename
    shutil.copy2(binary_src, dest_bin)
    if spec.os_name != "win32":
        dest_bin.chmod(0o755)


def write_platform_package_dir(
    out_dir: Path,
    spec: NpmPlatformSpec,
    version: str,
    binary_src: Path,
    license_src: Path,
) -> Path:
    """Write the `@archont561/playtest-<suffix>` platform package directory."""
    pkg_dir = out_dir / "@archont561" / f"playtest-{spec.suffix}"
    if pkg_dir.exists():
        shutil.rmtree(pkg_dir)
    pkg_dir.mkdir(parents=True, exist_ok=True)
    manifest = _platform_manifest(spec, version)
    (pkg_dir / "package.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    _stage_platform_binary(pkg_dir, spec, binary_src)
    shutil.copy2(license_src, pkg_dir / "LICENSE")
    return pkg_dir


def _copy_sdk_assets_to_wrapper(root: Path, wrapper_dir: Path) -> None:
    ts_dir = root / "typescript"
    for rel in ("dist/index.js", "dist/index.d.ts"):
        src = ts_dir / rel
        if not src.is_file():
            raise FileNotFoundError(f"missing built TypeScript SDK file: {src}")
        dest = wrapper_dir / rel
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dest)
    shutil.copy2(root / "LICENSE", wrapper_dir / "LICENSE")
    shutil.copy2(root / "README.md", wrapper_dir / "README.md")


def _write_wrapper_manifest(
    wrapper_dir: Path,
    base_pkg: dict[str, object],
    version: str,
    platforms: Sequence[NpmPlatformSpec],
) -> None:
    base_pkg.pop("devDependencies", None)
    base_pkg.pop("scripts", None)
    base_pkg["type"] = "module"
    base_pkg["version"] = version
    base_pkg["bin"] = {"playtest": "bin/playtest.js"}
    base_pkg["files"] = ["bin", "dist", "LICENSE", "README.md"]
    base_pkg["engines"] = {"node": ">=18"}
    base_pkg["optionalDependencies"] = {spec.package_name: version for spec in platforms}
    (wrapper_dir / "package.json").write_text(
        json.dumps(base_pkg, indent=2) + "\n", encoding="utf-8"
    )


def _stage_wrapper_loader(ts_dir: Path, wrapper_dir: Path) -> None:
    bin_dir = wrapper_dir / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)
    loader_dest = bin_dir / "playtest.js"
    shutil.copy2(ts_dir / "loader" / "playtest.cjs", loader_dest)
    loader_dest.chmod(0o755)
    (bin_dir / "package.json").write_text('{"type":"commonjs"}\n', encoding="utf-8")


def stage_wrapper_sdk_and_loader(
    root: Path,
    out_dir: Path,
    version: str,
    platforms: Sequence[NpmPlatformSpec],
    *,
    use_cargo_npm: bool,
) -> Path:
    """Stage the `@archont561/playtest` wrapper package (SDK dist, manifest, CLI loader)."""
    wrapper_dir = out_dir / "@archont561" / "playtest"
    wrapper_dir.mkdir(parents=True, exist_ok=True)
    _copy_sdk_assets_to_wrapper(root, wrapper_dir)
    ts_dir = root / "typescript"
    base_pkg = json.loads((ts_dir / "package.json").read_text(encoding="utf-8"))
    _write_wrapper_manifest(wrapper_dir, base_pkg, version, platforms)
    if use_cargo_npm and run_cmd(["cargo", "npm", "generate"], cwd=root).returncode != 0:
        raise RuntimeError("`cargo npm generate` failed")
    _stage_wrapper_loader(ts_dir, wrapper_dir)
    merged = json.loads((wrapper_dir / "package.json").read_text(encoding="utf-8"))
    _write_wrapper_manifest(wrapper_dir, merged, version, platforms)
    return wrapper_dir
