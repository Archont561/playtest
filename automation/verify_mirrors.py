"""Cross-platform mirror SHA-256 comparison and offline installer checks (Spec 0013 §9)."""

from __future__ import annotations

import argparse
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
from collections.abc import Mapping, Sequence
from pathlib import Path

from automation._proc import emit_err, emit_line, run_cmd


def _sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def record_mirror_hashes(cli_bin: Path, output_file: Path, platform_label: str) -> int:
    """Run `playtest init` + `playtest sync` + `playtest sync --check` and record mirror hashes."""
    cli_abs = cli_bin.resolve()
    with tempfile.TemporaryDirectory(prefix="playtest-mirror-hash-") as tmp:
        ws = Path(tmp)
        for subcmd in (["init"], ["sync"], ["sync", "--check"]):
            res = run_cmd([str(cli_abs), *subcmd], cwd=ws, capture=True)
            if res.returncode != 0:
                emit_err(
                    f"[verify-mirrors] ERROR: playtest {' '.join(subcmd)} failed:\n{res.stderr}"
                )
                return 1
        artifacts: dict[str, dict[str, object]] = {}
        for path in sorted((ws / ".playtest").rglob("*")):
            if path.is_file() and path.name != ".gitignore":
                rel = path.relative_to(ws).as_posix()
                artifacts[rel] = {"sha256": _sha256_file(path), "size": path.stat().st_size}
    payload = {"platform": platform_label, "artifacts": artifacts}
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    emit_line(f"[verify-mirrors] recorded {len(artifacts)} mirror hash(es) -> {output_file}")
    return 0


def compare_mirror_hash_files(inputs: Sequence[Path]) -> int:
    """Verify every recorded mirror hash manifest across platforms is byte-for-byte identical."""
    files: list[Path] = []
    for item in inputs:
        files.extend(sorted(item.rglob("*.json")) if item.is_dir() else [item])
    if len(files) < 2:
        emit_err(
            f"[verify-mirrors] ERROR: need >=2 platform hash files to compare, got {len(files)}"
        )
        return 1
    loaded = [(f, json.loads(f.read_text(encoding="utf-8"))) for f in files]
    ref_file, ref_data = loaded[0]
    ref_artifacts = ref_data.get("artifacts", {})
    if not ref_artifacts:
        emit_err(f"[verify-mirrors] ERROR: empty artifacts map in {ref_file}")
        return 1
    for other_file, other_data in loaded[1:]:
        if other_data.get("artifacts") != ref_artifacts:
            emit_err(
                f"[verify-mirrors] ERROR: cross-platform mirror hash mismatch between "
                f"{ref_data.get('platform')} ({ref_file}) and "
                f"{other_data.get('platform')} ({other_file}):\n"
                f"expected={ref_artifacts}\nactual={other_data.get('artifacts')}"
            )
            return 1
    platforms = [str(item[1].get("platform", item[0].name)) for item in loaded]
    emit_line(f"[verify-mirrors] OK: mirror SHA-256 digests match across {', '.join(platforms)}")
    return 0


def _build_upgraded_wheel(src_whl: Path, dest_whl: Path, old_ver: str, new_ver: str) -> None:
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
    rec_lines = []
    for path_key, payload in sorted(entries.items()):
        b64 = (
            base64.urlsafe_b64encode(hashlib.sha256(payload).digest()).rstrip(b"=").decode("ascii")
        )
        rec_lines.append(f"{path_key},sha256={b64},{len(payload)}")
    rec_path = f"{new_dist}/RECORD"
    rec_lines.append(f"{rec_path},,")
    entries[rec_path] = ("\n".join(rec_lines) + "\n").encode("utf-8")
    dest_whl.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(dest_whl, "w", compression=zipfile.ZIP_STORED) as out_zf:
        for path_key, payload in sorted(entries.items()):
            info = zipfile.ZipInfo(path_key, date_time=(1980, 1, 1, 0, 0, 0))
            info.external_attr = 0o644 << 16
            out_zf.writestr(info, payload)


def _build_upgraded_tgz(src_tgz: Path, dest_tgz: Path, new_ver: str) -> None:
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


def _offline_env_without_cli(
    cli_dir: Path, extra: Mapping[str, str] | None = None
) -> dict[str, str]:
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
    res = run_cmd(
        [uv_bin, "add", "--cache-dir", str(uv_cache), "--python", sys.executable, "pytest>=8.0.0"],
        cwd=warm_proj,
        capture=True,
    )
    if res.returncode != 0:
        detail = f"{res.stdout}\n{res.stderr}".strip()
        emit_err(f"[verify-mirrors] ERROR: failed to warm uv cache:\n{detail}")
        return False
    return True


def _install_and_assert_uv_mirror(
    uv_bin: str,
    proj: Path,
    whl_rel: str,
    expected_ver: str,
    env: Mapping[str, str],
) -> bool:
    add_res = run_cmd(
        [uv_bin, "add", "--dev", "--offline", "--python", sys.executable, whl_rel],
        cwd=proj,
        env=env,
        capture=True,
    )
    if add_res.returncode != 0:
        detail = f"{add_res.stdout}\n{add_res.stderr}".strip()
        emit_err(f"[verify-mirrors] ERROR: offline `uv add` ({expected_ver}) failed:\n{detail}")
        return False
    chk_res = run_cmd(
        [
            uv_bin,
            "run",
            "--offline",
            "--python",
            sys.executable,
            "python",
            "-c",
            f"import playtest; assert playtest.VERSION == {expected_ver!r}, playtest.VERSION",
        ],
        cwd=proj,
        env=env,
        capture=True,
    )
    if chk_res.returncode != 0:
        detail = f"{chk_res.stdout}\n{chk_res.stderr}".strip()
        emit_err(f"[verify-mirrors] ERROR: uv ({expected_ver}) import failed:\n{detail}")
        return False
    return True


def _verify_uv_offline_and_upgrade(
    base_mirrors: Path,
    cli_dir: Path,
    v1: str,
    v2: str,
) -> int:
    uv_bin = shutil.which("uv")
    if uv_bin is None:
        emit_err("[verify-mirrors] ERROR: `uv` binary is required on PATH")
        return 1
    with tempfile.TemporaryDirectory(prefix="playtest-uv-") as tmp:
        root = Path(tmp)
        uv_cache = root / "uv-cache"
        if not _warm_uv_cache(uv_bin, uv_cache, root):
            return 1
        proj = root / "consumer"
        proj.mkdir()
        shutil.copytree(base_mirrors / ".playtest", proj / ".playtest")
        (proj / "pyproject.toml").write_text(
            '[project]\nname = "consumer-py"\nversion = "0.0.1"\nrequires-python = ">=3.10"\n',
            encoding="utf-8",
        )
        env = _offline_env_without_cli(cli_dir, {"UV_CACHE_DIR": str(uv_cache)})
        whl_v1 = f".playtest/python/playtest_py-{v1}-py3-none-any.whl"
        if not _install_and_assert_uv_mirror(uv_bin, proj, whl_v1, v1, env):
            return 1
        whl_v2 = f".playtest/python/playtest_py-{v2}-py3-none-any.whl"
        _build_upgraded_wheel(proj / whl_v1, proj / whl_v2, v1, v2)
        (proj / whl_v1).unlink()
        if not _install_and_assert_uv_mirror(uv_bin, proj, whl_v2, v2, env):
            return 1
    emit_line("[verify-mirrors] OK: uv offline install + upgrade verified")
    return 0


def _js_add_cmd(manager: str, bin_path: str, tgz_rel: str, cache_dir: Path) -> list[str]:
    if manager == "npm":
        return [
            bin_path,
            "install",
            "--save-dev",
            "--offline",
            "--prefer-offline",
            "--no-audit",
            "--no-fund",
            "--cache",
            str(cache_dir),
            f"./{tgz_rel}",
        ]
    if manager == "pnpm":
        return [
            bin_path,
            "add",
            "--save-dev",
            "--offline",
            "--store-dir",
            str(cache_dir),
            f"./{tgz_rel}",
        ]
    return [bin_path, "add", "--dev", "--offline", "--cache-dir", str(cache_dir), f"./{tgz_rel}"]


def _verify_js_manager(
    manager: str,
    base_mirrors: Path,
    cli_dir: Path,
    v1: str,
    v2: str,
) -> int:
    bin_path = shutil.which(manager)
    node_or_bun = shutil.which("bun" if manager == "bun" else "node")
    if bin_path is None or node_or_bun is None:
        emit_err(f"[verify-mirrors] ERROR: `{manager}` (or node/bun runtime) not found on PATH")
        return 1
    with tempfile.TemporaryDirectory(prefix=f"playtest-{manager}-") as tmp:
        proj = Path(tmp) / "app"
        cache_dir = Path(tmp) / "cache"
        proj.mkdir()
        cache_dir.mkdir()
        shutil.copytree(base_mirrors / ".playtest", proj / ".playtest")
        (proj / "package.json").write_text(
            '{\n  "name": "consumer-ts",\n  "version": "0.0.1",\n'
            '  "private": true,\n  "type": "module"\n}\n',
            encoding="utf-8",
        )
        env = _offline_env_without_cli(cli_dir, {"BUN_INSTALL_CACHE_DIR": str(cache_dir)})
        tgz_v1 = f".playtest/typescript/archont561-playtest-{v1}.tgz"
        add1 = run_cmd(
            _js_add_cmd(manager, bin_path, tgz_v1, cache_dir), cwd=proj, env=env, capture=True
        )
        if add1.returncode != 0:
            detail = f"{add1.stdout}\n{add1.stderr}".strip()
            emit_err(f"[verify-mirrors] ERROR: {manager} offline v1 install failed:\n{detail}")
            return 1
        eval_v1 = _check_ts_import(manager, node_or_bun, proj, env, v1)
        if eval_v1 != 0:
            return eval_v1

        tgz_v2 = f".playtest/typescript/archont561-playtest-{v2}.tgz"
        _build_upgraded_tgz(proj / tgz_v1, proj / tgz_v2, v2)
        (proj / tgz_v1).unlink()
        add2 = run_cmd(
            _js_add_cmd(manager, bin_path, tgz_v2, cache_dir), cwd=proj, env=env, capture=True
        )
        if add2.returncode != 0:
            detail = f"{add2.stdout}\n{add2.stderr}".strip()
            emit_err(f"[verify-mirrors] ERROR: {manager} offline v2 upgrade failed:\n{detail}")
            return 1
        if _check_ts_import(manager, node_or_bun, proj, env, v2) != 0:
            return 1
    emit_line(f"[verify-mirrors] OK: {manager} offline install + upgrade verified")
    return 0


def _check_ts_import(
    manager: str,
    runtime_bin: str,
    proj: Path,
    env: Mapping[str, str],
    expected_ver: str,
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


def verify_all_installers(cli_bin: Path, v1: str = "0.1.0", v2: str = "0.2.0") -> int:
    """Verify offline fresh-clone install and v1->v2 upgrade across uv, npm, pnpm, and bun."""
    cli_abs = cli_bin.resolve()
    cli_dir = cli_abs.parent
    with tempfile.TemporaryDirectory(prefix="playtest-seed-") as seed_tmp:
        seed = Path(seed_tmp)
        for subcmd in (["init"], ["sync"]):
            res = run_cmd([str(cli_abs), *subcmd], cwd=seed, capture=True)
            if res.returncode != 0:
                emit_err(f"[verify-mirrors] ERROR: seed playtest {subcmd} failed:\n{res.stderr}")
                return 1
        if _verify_uv_offline_and_upgrade(seed, cli_dir, v1, v2) != 0:
            return 1
        for manager in ("npm", "pnpm", "bun"):
            if _verify_js_manager(manager, seed, cli_dir, v1, v2) != 0:
                return 1
    emit_line("[verify-mirrors] OK: all 4 ecosystems (uv, npm, pnpm, bun) passed offline + upgrade")
    return 0


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Verify Playtest SDK mirrors and determinism.")
    sub = parser.add_subparsers(dest="command", required=True)

    p_rec = sub.add_parser("record-hashes", help="Record mirror SHA-256 hashes on this platform.")
    p_rec.add_argument("--cli-bin", type=Path, required=True)
    p_rec.add_argument("--output", type=Path, required=True)
    p_rec.add_argument("--platform", default=sys.platform)

    p_cmp = sub.add_parser("compare-hashes", help="Compare mirror SHA-256 JSON manifests.")
    p_cmp.add_argument("inputs", nargs="+", type=Path)

    p_inst = sub.add_parser("verify-installers", help="Run offline install + upgrade suite.")
    p_inst.add_argument("--cli-bin", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for mirror determinism and offline installer verification."""
    args = _build_parser().parse_args(list(argv) if argv is not None else None)
    if args.command == "record-hashes":
        return record_mirror_hashes(args.cli_bin, args.output, args.platform)
    if args.command == "compare-hashes":
        return compare_mirror_hash_files(args.inputs)
    return verify_all_installers(args.cli_bin)


if __name__ == "__main__":
    raise SystemExit(main())
