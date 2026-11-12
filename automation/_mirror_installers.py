"""Offline installer and mirror-upgrade helpers for `automation.verify_mirrors`."""

from __future__ import annotations

import base64
import hashlib
import io
import json
import os
import shutil
import sys
import tarfile
import tempfile
import zipfile
from collections.abc import Mapping
from pathlib import Path

from automation._proc import emit_err, emit_line, run_cmd


def _record_line(path_key: str, payload: bytes) -> str:
    digest = hashlib.sha256(payload).digest()
    b64 = base64.urlsafe_b64encode(digest).rstrip(b"=").decode("ascii")
    return f"{path_key},sha256={b64},{len(payload)}"


def _rewrite_wheel_entries(src_whl: Path, old_ver: str, new_ver: str) -> dict[str, bytes]:
    old_dist = f"playtest_py-{old_ver}.dist-info"
    new_dist = f"playtest_py-{new_ver}.dist-info"
    entries: dict[str, bytes] = {}
    with zipfile.ZipFile(src_whl, "r") as zf:
        for name in sorted(zf.namelist()):
            if name.endswith("/RECORD"):
                continue
            data = zf.read(name)
            if name == f"{old_dist}/METADATA":
                text = data.decode("utf-8").replace(f"Version: {old_ver}", f"Version: {new_ver}", 1)
                entries[f"{new_dist}/METADATA"] = text.encode("utf-8")
            elif name.startswith(f"{old_dist}/"):
                entries[name.replace(old_dist, new_dist, 1)] = data
            else:
                entries[name] = data
    rec_lines = [_record_line(key, payload) for key, payload in sorted(entries.items())]
    rec_path = f"{new_dist}/RECORD"
    rec_lines.append(f"{rec_path},,")
    entries[rec_path] = ("\n".join(rec_lines) + "\n").encode("utf-8")
    return entries


def build_upgraded_wheel(src_whl: Path, dest_whl: Path, old_ver: str, new_ver: str) -> None:
    """Repack `src_whl` as a deterministic stored wheel stamped at `new_ver`."""
    entries = _rewrite_wheel_entries(src_whl, old_ver, new_ver)
    dest_whl.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(dest_whl, "w", compression=zipfile.ZIP_STORED) as out_zf:
        for path_key, payload in sorted(entries.items()):
            info = zipfile.ZipInfo(path_key, date_time=(1980, 1, 1, 0, 0, 0))
            info.external_attr = 0o644 << 16
            out_zf.writestr(info, payload)


def build_upgraded_tgz(src_tgz: Path, dest_tgz: Path, new_ver: str) -> None:
    """Repack `src_tgz` as a deterministic npm tarball stamped at `new_ver`."""
    members: list[tuple[str, bytes]] = []
    with tarfile.open(src_tgz, "r:gz") as tf:
        for member in tf.getmembers():
            extracted = tf.extractfile(member)
            if extracted is None:
                continue
            raw = extracted.read()
            if member.name == "package/package.json":
                pkg = json.loads(raw.decode("utf-8"))
                pkg["version"] = new_ver
                raw = (json.dumps(pkg, indent=2) + "\n").encode("utf-8")
            members.append((member.name, raw))
    dest_tgz.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(dest_tgz, "w:gz") as out_tf:
        for arcname, payload in sorted(members):
            ti = tarfile.TarInfo(name=arcname)
            ti.size = len(payload)
            ti.mode = 0o644
            ti.mtime = 499_162_500
            out_tf.addfile(ti, io.BytesIO(payload))


def _same_dir(entry: str, target: Path) -> bool:
    try:
        resolved = Path(entry).resolve()
    except OSError:
        return False
    return os.path.normcase(str(resolved)) == os.path.normcase(str(target.resolve()))


def offline_env_without_cli(
    cli_dir: Path, extra: Mapping[str, str] | None = None
) -> dict[str, str]:
    """Build an offline environment with `cli_dir` removed from `PATH`."""
    env: dict[str, str] = dict(os.environ)
    env.pop("VIRTUAL_ENV", None)
    parts = env.get("PATH", "").split(os.pathsep)
    kept = [p for p in parts if p and not _same_dir(p, cli_dir)]
    env.update(
        {
            "PATH": os.pathsep.join(kept),
            "HTTP_PROXY": "http://127.0.0.1:9",
            "HTTPS_PROXY": "http://127.0.0.1:9",
            "http_proxy": "http://127.0.0.1:9",
            "https_proxy": "http://127.0.0.1:9",
            "UV_OFFLINE": "1",
            "npm_config_offline": "true",
            "npm_config_audit": "false",
            "npm_config_fund": "false",
        }
    )
    if extra:
        env.update(extra)
    return env


def _warm_uv_cache(uv_bin: str, uv_cache: Path, work_dir: Path) -> bool:
    warm_proj = work_dir / "warm"
    warm_proj.mkdir()
    (warm_proj / "pyproject.toml").write_text(
        '[project]\nname = "warm"\nversion = "0.0.1"\nrequires-python = ">=3.10"\n',
        encoding="utf-8",
    )
    cmd = [uv_bin, "add", "--cache-dir", str(uv_cache), "--python", sys.executable, "pytest>=8.0.0"]
    res = run_cmd(cmd, cwd=warm_proj, capture=True)
    if res.returncode != 0:
        detail = f"{res.stdout}\n{res.stderr}".strip()
        emit_err(f"[verify-mirrors] ERROR: failed to warm uv cache:\n{detail}")
        return False
    return True


def _install_and_assert_uv_mirror(
    uv_bin: str, proj: Path, whl_rel: str, expected_ver: str, env: Mapping[str, str]
) -> bool:
    add_cmd = [uv_bin, "add", "--dev", "--offline", "--python", sys.executable, whl_rel]
    add_res = run_cmd(add_cmd, cwd=proj, env=env, capture=True)
    if add_res.returncode != 0:
        detail = f"{add_res.stdout}\n{add_res.stderr}".strip()
        emit_err(f"[verify-mirrors] ERROR: offline `uv add` ({expected_ver}) failed:\n{detail}")
        return False
    assertion = f"import playtest; assert playtest.VERSION == {expected_ver!r}, playtest.VERSION"
    chk_cmd = [uv_bin, "run", "--offline", "--python", sys.executable, "python", "-c", assertion]
    chk_res = run_cmd(chk_cmd, cwd=proj, env=env, capture=True)
    if chk_res.returncode != 0:
        detail = f"{chk_res.stdout}\n{chk_res.stderr}".strip()
        emit_err(f"[verify-mirrors] ERROR: uv ({expected_ver}) import failed:\n{detail}")
        return False
    return True


def _setup_uv_consumer(base_mirrors: Path, root: Path) -> Path:
    proj = root / "consumer"
    proj.mkdir()
    shutil.copytree(base_mirrors / ".playtest", proj / ".playtest")
    (proj / "pyproject.toml").write_text(
        '[project]\nname = "consumer-py"\nversion = "0.0.1"\nrequires-python = ">=3.10"\n',
        encoding="utf-8",
    )
    return proj


def verify_uv_offline_and_upgrade(base_mirrors: Path, cli_dir: Path, v1: str, v2: str) -> int:
    """Verify `uv` offline wheel installation and v1 -> v2 upgrade."""
    uv_bin = shutil.which("uv")
    if uv_bin is None:
        emit_err("[verify-mirrors] ERROR: `uv` binary is required on PATH")
        return 1
    with tempfile.TemporaryDirectory(prefix="playtest-uv-") as tmp:
        root = Path(tmp)
        uv_cache = root / "uv-cache"
        if not _warm_uv_cache(uv_bin, uv_cache, root):
            return 1
        proj = _setup_uv_consumer(base_mirrors, root)
        env = offline_env_without_cli(cli_dir, {"UV_CACHE_DIR": str(uv_cache)})
        whl_v1 = f".playtest/python/playtest_py-{v1}-py3-none-any.whl"
        if not _install_and_assert_uv_mirror(uv_bin, proj, whl_v1, v1, env):
            return 1
        whl_v2 = f".playtest/python/playtest_py-{v2}-py3-none-any.whl"
        build_upgraded_wheel(proj / whl_v1, proj / whl_v2, v1, v2)
        (proj / whl_v1).unlink()
        if not _install_and_assert_uv_mirror(uv_bin, proj, whl_v2, v2, env):
            return 1
    emit_line("[verify-mirrors] OK: uv offline install + upgrade verified")
    return 0


def _js_add_cmd(manager: str, bin_path: str, tgz_rel: str, cache_dir: Path) -> list[str]:
    cache = str(cache_dir)
    pkg_spec = f"@archont561/playtest@file:{tgz_rel}"
    if manager == "npm":
        flags = ["--save-dev", "--offline", "--prefer-offline", "--no-audit", "--no-fund"]
        return [bin_path, "install", *flags, "--cache", cache, f"./{tgz_rel}"]
    if manager == "pnpm":
        return [bin_path, "add", "--save-dev", "--offline", "--store-dir", cache, pkg_spec]
    return [bin_path, "add", "--dev", "--offline", "--cache-dir", cache, pkg_spec]


def _check_ts_import(
    manager: str, runtime_bin: str, proj: Path, env: Mapping[str, str], expected_ver: str
) -> int:
    code = (
        f"import {{ VERSION }} from '@archont561/playtest'; "
        f"if (VERSION !== {expected_ver!r}) {{ console.error(VERSION); process.exit(1); }}"
    )
    cmd = (
        [runtime_bin, "-e", code]
        if manager == "bun"
        else [runtime_bin, "--input-type=module", "-e", code]
    )
    res = run_cmd(cmd, cwd=proj, env=env, capture=True)
    if res.returncode != 0:
        detail = f"{res.stdout}\n{res.stderr}".strip()
        emit_err(f"[verify-mirrors] ERROR: {manager} import expected {expected_ver}:\n{detail}")
        return 1
    return 0


def _setup_js_consumer(base_mirrors: Path, tmp_root: Path) -> tuple[Path, Path]:
    proj = tmp_root / "app"
    cache_dir = tmp_root / "cache"
    proj.mkdir()
    cache_dir.mkdir()
    shutil.copytree(base_mirrors / ".playtest", proj / ".playtest")
    (proj / "package.json").write_text(
        '{\n  "name": "consumer-ts",\n  "version": "0.0.1",\n'
        '  "private": true,\n  "type": "module"\n}\n',
        encoding="utf-8",
    )
    return proj, cache_dir


def _js_tgz_rel(version: str) -> str:
    return f".playtest/typescript/archont561-playtest-{version}.tgz"


def _js_install_step(
    manager: str,
    bins: tuple[str, str],
    proj_and_cache: tuple[Path, Path],
    env: Mapping[str, str],
    request: tuple[str, str],
) -> int:
    bin_path, _runtime_bin = bins
    proj, cache_dir = proj_and_cache
    tgz_rel, label = request
    res = run_cmd(
        _js_add_cmd(manager, bin_path, tgz_rel, cache_dir), cwd=proj, env=env, capture=True
    )
    if res.returncode != 0:
        detail = f"{res.stdout}\n{res.stderr}".strip()
        emit_err(f"[verify-mirrors] ERROR: {manager} {label} install failed:\n{detail}")
        return 1
    return 0


def _run_js_upgrade_cycle(
    manager: str,
    bins: tuple[str, str],
    proj_and_cache: tuple[Path, Path],
    env: Mapping[str, str],
    versions: tuple[str, str],
) -> int:
    proj, _cache_dir = proj_and_cache
    _bin_path, runtime_bin = bins
    v1, v2 = versions
    tgz_v1, tgz_v2 = _js_tgz_rel(v1), _js_tgz_rel(v2)
    if _js_install_step(manager, bins, proj_and_cache, env, (tgz_v1, "v1")) != 0:
        return 1
    if _check_ts_import(manager, runtime_bin, proj, env, v1) != 0:
        return 1
    build_upgraded_tgz(proj / tgz_v1, proj / tgz_v2, v2)
    (proj / tgz_v1).unlink()
    if _js_install_step(manager, bins, proj_and_cache, env, (tgz_v2, "v2 upgrade")) != 0:
        return 1
    return _check_ts_import(manager, runtime_bin, proj, env, v2)


def verify_js_manager(manager: str, base_mirrors: Path, cli_dir: Path, v1: str, v2: str) -> int:
    """Verify `npm`, `pnpm`, or `bun` offline tarball installation and v1 -> v2 upgrade."""
    bin_path = shutil.which(manager)
    runtime_bin = shutil.which("bun" if manager == "bun" else "node")
    if bin_path is None or runtime_bin is None:
        emit_err(f"[verify-mirrors] ERROR: `{manager}` (or node/bun runtime) not found on PATH")
        return 1
    with tempfile.TemporaryDirectory(prefix=f"playtest-{manager}-") as tmp:
        proj, cache_dir = _setup_js_consumer(base_mirrors, Path(tmp))
        env = offline_env_without_cli(cli_dir, {"BUN_INSTALL_CACHE_DIR": str(cache_dir)})
        rc = _run_js_upgrade_cycle(
            manager, (bin_path, runtime_bin), (proj, cache_dir), env, (v1, v2)
        )
        if rc != 0:
            return rc
    emit_line(f"[verify-mirrors] OK: {manager} offline install + upgrade verified")
    return 0
