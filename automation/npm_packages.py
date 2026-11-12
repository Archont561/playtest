"""Generate, pack, verify, and publish @archont561/playtest npm packages (Spec 0012 §4.3)."""

from __future__ import annotations

import argparse
import io
import json
import os
import platform
import shutil
import tarfile
import tempfile
import tomllib
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd

CARGO_NPM_VERSION: str = "0.1.5"
REPRODUCIBLE_MTIME: int = 499_162_500
FORBIDDEN_LIFECYCLE_SCRIPTS: frozenset[str] = frozenset(
    {"preinstall", "install", "postinstall", "prepublish", "prepare"}
)


@dataclass(frozen=True)
class NpmPlatformSpec:
    """Mapping from Rust target triple to @archont561/playtest-<os>-<cpu> npm metadata."""

    triple: str
    suffix: str
    os_name: str
    cpu_name: str

    @property
    def package_name(self) -> str:
        """Scoped npm platform package name."""
        return f"@archont561/playtest-{self.suffix}"

    @property
    def binary_filename(self) -> str:
        """Executable filename inside the platform package."""
        return "playtest.exe" if self.os_name == "win32" else "playtest"


NPM_PLATFORMS: tuple[NpmPlatformSpec, ...] = (
    NpmPlatformSpec("x86_64-unknown-linux-musl", "linux-x64", "linux", "x64"),
    NpmPlatformSpec("aarch64-unknown-linux-musl", "linux-arm64", "linux", "arm64"),
    NpmPlatformSpec("x86_64-apple-darwin", "darwin-x64", "darwin", "x64"),
    NpmPlatformSpec("aarch64-apple-darwin", "darwin-arm64", "darwin", "arm64"),
    NpmPlatformSpec("x86_64-pc-windows-msvc", "win32-x64", "win32", "x64"),
)


def host_npm_platform() -> NpmPlatformSpec:
    """Resolve the current machine's `NpmPlatformSpec`."""
    sys_map = {"linux": "linux", "darwin": "darwin", "windows": "win32"}
    arch_map = {"x86_64": "x64", "amd64": "x64", "aarch64": "arm64", "arm64": "arm64"}
    os_name = sys_map.get(platform.system().lower(), "")
    cpu_name = arch_map.get(platform.machine().lower(), "")
    for spec in NPM_PLATFORMS:
        if spec.os_name == os_name and spec.cpu_name == cpu_name:
            return spec
    raise RuntimeError(
        f"unsupported host for npm verification: {platform.system()}/{platform.machine()}"
    )


def _workspace_version(root: Path) -> str:
    cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    return str(cargo["workspace"]["package"]["version"])


def _write_platform_package_dir(
    out_dir: Path,
    spec: NpmPlatformSpec,
    version: str,
    binary_src: Path,
    license_src: Path,
) -> Path:
    pkg_dir = out_dir / "@archont561" / f"playtest-{spec.suffix}"
    if pkg_dir.exists():
        shutil.rmtree(pkg_dir)
    pkg_dir.mkdir(parents=True, exist_ok=True)
    pkg_json = {
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
    (pkg_dir / "package.json").write_text(json.dumps(pkg_json, indent=2) + "\n", encoding="utf-8")
    dest_bin = pkg_dir / spec.binary_filename
    shutil.copy2(binary_src, dest_bin)
    if spec.os_name != "win32":
        dest_bin.chmod(0o755)
    shutil.copy2(license_src, pkg_dir / "LICENSE")
    return pkg_dir


def _copy_sdk_assets_to_wrapper(root: Path, wrapper_dir: Path) -> None:
    ts_dir = root / "typescript"
    for rel in ("dist/index.js", "src/index.d.ts"):
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
    base_pkg["files"] = ["bin", "dist", "src/index.d.ts", "LICENSE", "README.md"]
    base_pkg["engines"] = {"node": ">=18"}
    base_pkg["optionalDependencies"] = {spec.package_name: version for spec in platforms}
    (wrapper_dir / "package.json").write_text(
        json.dumps(base_pkg, indent=2) + "\n", encoding="utf-8"
    )


def _stage_wrapper_sdk_and_loader(
    root: Path,
    out_dir: Path,
    version: str,
    platforms: Sequence[NpmPlatformSpec],
    *,
    use_cargo_npm: bool,
) -> Path:
    wrapper_dir = out_dir / "@archont561" / "playtest"
    wrapper_dir.mkdir(parents=True, exist_ok=True)
    _copy_sdk_assets_to_wrapper(root, wrapper_dir)
    ts_dir = root / "typescript"
    base_pkg = json.loads((ts_dir / "package.json").read_text(encoding="utf-8"))
    _write_wrapper_manifest(wrapper_dir, base_pkg, version, platforms)

    if use_cargo_npm and run_cmd(["cargo", "npm", "generate"], cwd=root).returncode != 0:
        raise RuntimeError("`cargo npm generate` failed")

    bin_dir = wrapper_dir / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)
    loader_dest = bin_dir / "playtest.js"
    shutil.copy2(ts_dir / "loader" / "playtest.cjs", loader_dest)
    loader_dest.chmod(0o755)
    (bin_dir / "package.json").write_text('{"type":"commonjs"}\n', encoding="utf-8")

    merged = json.loads((wrapper_dir / "package.json").read_text(encoding="utf-8"))
    _write_wrapper_manifest(wrapper_dir, merged, version, platforms)
    return wrapper_dir


def assert_no_lifecycle_scripts(pkg_json_path: Path) -> None:
    """Fail if a generated `package.json` declares any npm lifecycle script."""
    data = json.loads(pkg_json_path.read_text(encoding="utf-8"))
    scripts = data.get("scripts")
    if isinstance(scripts, dict):
        bad = sorted(FORBIDDEN_LIFECYCLE_SCRIPTS.intersection(scripts.keys()))
        if bad:
            raise RuntimeError(f"forbidden lifecycle script(s) {bad} in {pkg_json_path}")


def pack_npm_dir(pkg_dir: Path, dest_tgz: Path, executable_rels: frozenset[str]) -> Path:
    """Pack `pkg_dir` into a deterministic npm `.tgz` (`package/...`) preserving 0o755 bits."""
    assert_no_lifecycle_scripts(pkg_dir / "package.json")
    dest_tgz.parent.mkdir(parents=True, exist_ok=True)
    files = sorted(p for p in pkg_dir.rglob("*") if p.is_file())
    with tarfile.open(dest_tgz, "w:gz") as tf:
        for file_path in files:
            rel = file_path.relative_to(pkg_dir).as_posix()
            raw = file_path.read_bytes()
            ti = tarfile.TarInfo(name=f"package/{rel}")
            ti.size = len(raw)
            ti.mtime = REPRODUCIBLE_MTIME
            ti.mode = 0o755 if rel in executable_rels else 0o644
            tf.addfile(ti, io.BytesIO(raw))
    return dest_tgz


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
        for spec in platforms:
            src_bin = binaries_by_triple[spec.triple]
            cargo_slot = root / "target" / spec.triple / "release" / spec.binary_filename
            cargo_slot.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src_bin, cargo_slot)
    staging = root / "npm" if use_cargo_npm else Path(tempfile.mkdtemp(prefix="playtest-npm-gen-"))
    try:
        written: list[Path] = []
        for spec in platforms:
            src_bin = binaries_by_triple[spec.triple]
            p_dir = _write_platform_package_dir(staging, spec, version, src_bin, root / "LICENSE")
            tgz = tarballs_dir / f"archont561-playtest-{spec.suffix}-{version}.tgz"
            exec_set = frozenset({spec.binary_filename}) if spec.os_name != "win32" else frozenset()
            written.append(pack_npm_dir(p_dir, tgz, exec_set))

        w_dir = _stage_wrapper_sdk_and_loader(
            root, staging, version, platforms, use_cargo_npm=use_cargo_npm
        )
        w_tgz = tarballs_dir / f"archont561-playtest-{version}.tgz"
        written.append(pack_npm_dir(w_dir, w_tgz, frozenset({"bin/playtest.js"})))
        return written
    finally:
        if staging.exists():
            shutil.rmtree(staging)


def _verify_one_npm_client(
    manager: str,
    platform_tgz: Path,
    wrapper_tgz: Path,
    expected_version: str,
) -> int:
    mgr_bin = shutil.which(manager)
    runtime_bin = shutil.which("bun" if manager == "bun" else "node")
    if mgr_bin is None or runtime_bin is None:
        emit_err(f"[npm-packages] ERROR: `{manager}` not found on PATH")
        return 1
    with tempfile.TemporaryDirectory(prefix=f"playtest-npm-e2e-{manager}-") as tmp:
        proj = Path(tmp) / "app"
        cache = Path(tmp) / "cache"
        proj.mkdir()
        cache.mkdir()
        (proj / "package.json").write_text(
            '{\n  "name": "e2e-consumer",\n  "version": "0.0.1",\n'
            '  "private": true,\n  "type": "module"\n}\n',
            encoding="utf-8",
        )
        add_cmd = _client_install_cmd(manager, mgr_bin, cache, platform_tgz, wrapper_tgz)
        env = {"npm_config_audit": "false", "npm_config_fund": "false"}
        res = run_cmd(add_cmd, cwd=proj, env=env, capture=True)
        if res.returncode != 0:
            emit_err(f"[npm-packages] ERROR: {manager} install failed:\n{res.stderr}")
            return 1
        bin_shim = (
            proj / "node_modules" / ".bin" / ("playtest.cmd" if os.name == "nt" else "playtest")
        )
        ver_res = run_cmd([str(bin_shim), "version"], cwd=proj, capture=True)
        if ver_res.returncode != 0 or f"playtest {expected_version}" not in ver_res.stdout:
            emit_err(
                f"[npm-packages] ERROR: {manager} `playtest version` failed:\n{ver_res.stderr}"
            )
            return 1
        imp_code = (
            f"import {{ VERSION }} from '@archont561/playtest'; "
            f"if (VERSION !== {expected_version!r}) process.exit(1);"
        )
        imp_cmd = (
            [runtime_bin, "-e", imp_code]
            if manager == "bun"
            else [runtime_bin, "--input-type=module", "-e", imp_code]
        )
        if run_cmd(imp_cmd, cwd=proj, capture=True).returncode != 0:
            emit_err(f"[npm-packages] ERROR: {manager} SDK import check failed")
            return 1
    emit_line(f"[npm-packages] OK: {manager} clean install verified (CLI + SDK {expected_version})")
    return 0


def _client_install_cmd(
    manager: str,
    mgr_bin: str,
    cache: Path,
    platform_tgz: Path,
    wrapper_tgz: Path,
) -> list[str]:
    if manager == "npm":
        return [
            mgr_bin,
            "install",
            "--save-dev",
            "--no-audit",
            "--no-fund",
            "--cache",
            str(cache),
            str(platform_tgz),
            str(wrapper_tgz),
        ]
    if manager == "pnpm":
        return [
            mgr_bin,
            "add",
            "--save-dev",
            "--store-dir",
            str(cache),
            str(platform_tgz),
            str(wrapper_tgz),
        ]
    return [mgr_bin, "add", "--dev", str(platform_tgz), str(wrapper_tgz)]


def verify_clean_installs(root: Path, cli_bin: Path) -> int:
    """Verify npm, pnpm, and Bun default installs give a working CLI and SDK import."""
    spec = host_npm_platform()
    version = _workspace_version(root)
    with tempfile.TemporaryDirectory(prefix="playtest-npm-tarballs-") as tmp:
        out_dir = Path(tmp)
        tarballs = build_npm_tarballs(
            root,
            {spec.triple: cli_bin.resolve()},
            out_dir,
            use_cargo_npm=False,
            platforms=(spec,),
        )
        platform_tgz, wrapper_tgz = tarballs[0], tarballs[1]
        for manager in ("npm", "pnpm", "bun"):
            if _verify_one_npm_client(manager, platform_tgz, wrapper_tgz, version) != 0:
                return 1
    return 0


def _cmd_pack_release(args: argparse.Namespace, root: Path) -> int:
    binaries: dict[str, Path] = {}
    for spec in NPM_PLATFORMS:
        candidate = args.binaries_dir.resolve() / spec.triple / spec.binary_filename
        if not candidate.is_file():
            emit_err(f"[npm-packages] ERROR: missing Phase 1 binary for {spec.triple}: {candidate}")
            return 1
        binaries[spec.triple] = candidate
    written = build_npm_tarballs(
        root,
        binaries,
        args.out_dir.resolve(),
        use_cargo_npm=args.use_cargo_npm,
    )
    for tgz in written:
        emit_line(f"[npm-packages] packed {tgz}")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for @archont561/playtest npm package generation and verification."""
    parser = argparse.ArgumentParser(description="Build and verify Playtest npm packages.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    sub = parser.add_subparsers(dest="command", required=True)

    p_pack = sub.add_parser("pack-release", help="Pack all 5 platform tarballs + wrapper tarball.")
    p_pack.add_argument("--binaries-dir", type=Path, required=True)
    p_pack.add_argument("--out-dir", type=Path, required=True)
    p_pack.add_argument("--use-cargo-npm", action="store_true")

    p_ver = sub.add_parser("verify-clean-installs", help="Test npm, pnpm, and Bun installs.")
    p_ver.add_argument("--cli-bin", type=Path, required=True)

    args = parser.parse_args(list(argv) if argv is not None else None)
    root = args.root.resolve()
    if args.command == "pack-release":
        return _cmd_pack_release(args, root)
    return verify_clean_installs(root, args.cli_bin)


if __name__ == "__main__":
    raise SystemExit(main())
