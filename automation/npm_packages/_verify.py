"""npm, pnpm, and Bun clean-install verification for generated tarballs."""

from __future__ import annotations

import json
import os
import shutil
import tempfile
import tomllib
from pathlib import Path

from automation._proc import emit_err, emit_line, run_cmd
from automation.npm_packages._spec import NpmPlatformSpec, host_npm_platform
from automation.npm_packages._tarballs import build_npm_tarballs


def _workspace_version(root: Path) -> str:
    cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    return str(cargo["workspace"]["package"]["version"])


def _write_e2e_consumer_manifest(proj: Path, spec: NpmPlatformSpec, platform_tgz: Path) -> None:
    plat_ref = f"file:{platform_tgz.as_posix()}"
    manifest = {
        "name": "e2e-consumer",
        "version": "0.0.1",
        "private": True,
        "type": "module",
        "overrides": {spec.package_name: plat_ref},
        "pnpm": {"overrides": {spec.package_name: plat_ref}},
    }
    (proj / "package.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def _assert_installed_cli_and_sdk(
    manager: str, runtime_bin: str, proj: Path, expected_version: str
) -> int:
    shim_name = "playtest.cmd" if os.name == "nt" else "playtest"
    bin_shim = proj / "node_modules" / ".bin" / shim_name
    ver_res = run_cmd([str(bin_shim), "version"], cwd=proj, capture=True)
    if ver_res.returncode != 0 or f"playtest {expected_version}" not in ver_res.stdout:
        detail = f"{ver_res.stdout}\n{ver_res.stderr}".strip()
        emit_err(f"[npm-packages] ERROR: {manager} `playtest version` failed:\n{detail}")
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
    imp_res = run_cmd(imp_cmd, cwd=proj, capture=True)
    if imp_res.returncode != 0:
        detail = f"{imp_res.stdout}\n{imp_res.stderr}".strip()
        emit_err(f"[npm-packages] ERROR: {manager} SDK import check failed:\n{detail}")
        return 1
    return 0


def _client_install_cmd(manager: str, mgr_bin: str, cache: Path, wrapper_tgz: Path) -> list[str]:
    cache_arg, tgz_arg = str(cache), str(wrapper_tgz)
    if manager == "npm":
        flags = ["--save-dev", "--no-audit", "--no-fund"]
        return [mgr_bin, "install", *flags, "--cache", cache_arg, tgz_arg]
    if manager == "pnpm":
        return [mgr_bin, "add", "--save-dev", "--store-dir", cache_arg, tgz_arg]
    return [mgr_bin, "add", "--dev", "--cache-dir", cache_arg, tgz_arg]


def _install_into_consumer_project(
    manager: str, mgr_bin: str, proj: Path, cache: Path, wrapper_tgz: Path
) -> int:
    add_cmd = _client_install_cmd(manager, mgr_bin, cache, wrapper_tgz)
    env = {
        "npm_config_audit": "false",
        "npm_config_fund": "false",
        "BUN_INSTALL_CACHE_DIR": str(cache),
    }
    res = run_cmd(add_cmd, cwd=proj, env=env, capture=True)
    if res.returncode != 0:
        detail = f"{res.stdout}\n{res.stderr}".strip()
        emit_err(f"[npm-packages] ERROR: {manager} install failed:\n{detail}")
    return res.returncode


def _verify_one_npm_client(
    manager: str,
    spec: NpmPlatformSpec,
    tarballs: tuple[Path, Path],
    expected_version: str,
) -> int:
    platform_tgz, wrapper_tgz = tarballs
    mgr_bin = shutil.which(manager)
    runtime_bin = shutil.which("bun" if manager == "bun" else "node")
    if mgr_bin is None or runtime_bin is None:
        emit_err(f"[npm-packages] ERROR: `{manager}` not found on PATH")
        return 1
    with tempfile.TemporaryDirectory(prefix=f"playtest-npm-e2e-{manager}-") as tmp:
        proj, cache = Path(tmp) / "app", Path(tmp) / "cache"
        proj.mkdir()
        cache.mkdir()
        _write_e2e_consumer_manifest(proj, spec, platform_tgz)
        if _install_into_consumer_project(manager, mgr_bin, proj, cache, wrapper_tgz) != 0:
            return 1
        if _assert_installed_cli_and_sdk(manager, runtime_bin, proj, expected_version) != 0:
            return 1
    emit_line(f"[npm-packages] OK: {manager} clean install verified (CLI + SDK {expected_version})")
    return 0


def verify_clean_installs(root: Path, cli_bin: Path) -> int:
    """Verify npm, pnpm, and Bun default installs give a working CLI and SDK import."""
    spec = host_npm_platform()
    version = _workspace_version(root)
    with tempfile.TemporaryDirectory(prefix="playtest-npm-tarballs-") as tmp:
        tarballs = build_npm_tarballs(
            root,
            {spec.triple: cli_bin.resolve()},
            Path(tmp),
            use_cargo_npm=False,
            platforms=(spec,),
        )
        for manager in ("npm", "pnpm", "bun"):
            client_tgz = (tarballs[0], tarballs[1])
            if _verify_one_npm_client(manager, spec, client_tgz, version) != 0:
                return 1
    return 0
