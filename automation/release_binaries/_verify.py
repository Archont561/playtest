"""Archive and version-alignment verification for GitHub Release assets."""

from __future__ import annotations

import tarfile
import tempfile
import zipfile
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import run_cmd
from automation.release_binaries._archives import sha256_hex
from automation.release_binaries._platforms import (
    PLATFORM_BY_TRIPLE,
    RELEASE_PLATFORMS,
    ReleasePlatform,
)
from automation.stamp_version import extract_version_from_tag, read_manifest_versions


def _safe_extract(archive: Path, dest: Path, *, is_windows: bool) -> None:
    if is_windows:
        with zipfile.ZipFile(archive, "r") as zf:
            for member in zf.infolist():
                target = (dest / member.filename).resolve()
                if not target.is_relative_to(dest.resolve()):
                    raise ValueError(f"unsafe zip path: {member.filename}")
            zf.extractall(dest)
        return
    with tarfile.open(archive, "r:gz") as tf:
        tf.extractall(dest, filter="data")


def _binary_version_errors(cli_bin: Path, expected: str) -> list[str]:
    res = run_cmd([str(cli_bin), "version"], capture=True)
    if res.returncode != 0:
        return [f"`{cli_bin} version` exited with {res.returncode}: {res.stderr.strip()}"]
    first_line = res.stdout.strip().splitlines()[0] if res.stdout.strip() else ""
    if first_line != f"playtest {expected}":
        return [f"CLI reported {first_line!r}, expected {'playtest ' + expected!r}"]
    return []


def _embedded_sdk_errors(cli_bin: Path, expected: str) -> list[str]:
    with tempfile.TemporaryDirectory(prefix="playtest-ver-sync-") as tmp:
        ws = Path(tmp)
        if run_cmd([str(cli_bin), "init"], cwd=ws, capture=True).returncode != 0:
            return ["`playtest init` failed during embedded SDK version check"]
        if run_cmd([str(cli_bin), "sync"], cwd=ws, capture=True).returncode != 0:
            return ["`playtest sync` failed during embedded SDK version check"]
        whl = ws / ".playtest" / "python" / f"playtest_py-{expected}-py3-none-any.whl"
        tgz = ws / ".playtest" / "typescript" / f"archont561-playtest-{expected}.tgz"
        if not whl.is_file() or not tgz.is_file():
            return [f"embedded SDK mirrors did not match version {expected!r}"]
    return []


def _verify_binary_and_embedded_version(cli_bin: Path, expected: str) -> list[str]:
    resolved = cli_bin.resolve()
    errors = _binary_version_errors(resolved, expected)
    if errors:
        return errors
    return _embedded_sdk_errors(resolved, expected)


def verify_version_alignment(
    expected_version_or_tag: str,
    *,
    root: Path = REPO_ROOT,
    cli_bin: Path | None = None,
) -> list[str]:
    """Return errors when any manifest, CLI binary, or embedded SDK version drifts."""
    expected = extract_version_from_tag(expected_version_or_tag)
    versions = read_manifest_versions(root)
    conda_manifest = root / "conda" / "pixi.toml"
    if conda_manifest.is_file():
        import tomllib

        conda_cfg = tomllib.loads(conda_manifest.read_text(encoding="utf-8"))
        versions["conda"] = str(conda_cfg["workspace"]["version"])

    errors: list[str] = []
    for name, actual in sorted(versions.items()):
        want = expected.split("-", 1)[0] if name == "r" else expected
        if actual != want:
            errors.append(f"manifest `{name}` has version {actual!r}, expected {want!r}")
    if cli_bin is not None:
        errors.extend(_verify_binary_and_embedded_version(cli_bin, expected))
    return errors


def _unpack_and_verify_contents(
    archive: Path, spec: ReleasePlatform, expected: str, *, execute_binary: bool
) -> list[str]:
    with tempfile.TemporaryDirectory(prefix="playtest-verify-archive-") as tmp:
        unpack_dir = Path(tmp)
        _safe_extract(archive, unpack_dir, is_windows=spec.is_windows)
        if not list(unpack_dir.rglob("LICENSE")):
            return [f"archive {archive.name} is missing LICENSE"]
        bin_matches = list(unpack_dir.rglob(spec.binary_name))
        if not bin_matches:
            return [f"archive {archive.name} is missing {spec.binary_name}"]
        exe = bin_matches[0]
        if not spec.is_windows:
            exe.chmod(0o755)
        return _verify_binary_and_embedded_version(exe, expected) if execute_binary else []


def verify_platform_archive(
    triple: str,
    expected_version_or_tag: str,
    dist_dir: Path,
    *,
    execute_binary: bool = True,
) -> list[str]:
    """Verify `.sha256`, unpack archive, require `LICENSE`, and run `playtest version`."""
    spec = PLATFORM_BY_TRIPLE[triple]
    expected = extract_version_from_tag(expected_version_or_tag)
    archive = dist_dir / spec.archive_filename
    sidecar = dist_dir / spec.checksum_filename
    if not archive.is_file() or not sidecar.is_file():
        return [f"missing archive or sidecar for {triple} in {dist_dir}"]

    recorded = sidecar.read_text(encoding="utf-8").strip().split()[0]
    actual = sha256_hex(archive)
    if recorded.lower() != actual.lower():
        return [f"SHA-256 mismatch for {archive.name}: expected {recorded}, got {actual}"]
    return _unpack_and_verify_contents(archive, spec, expected, execute_binary=execute_binary)


def _checksum_line(
    spec: ReleasePlatform, dist_dir: Path, *, require_all_platforms: bool
) -> str | None:
    sidecar = dist_dir / spec.checksum_filename
    archive = dist_dir / spec.archive_filename
    if not sidecar.is_file() or not archive.is_file():
        if require_all_platforms:
            raise FileNotFoundError(
                f"missing release asset or checksum for {spec.triple}: {spec.archive_filename}"
            )
        return None
    recorded = sidecar.read_text(encoding="utf-8").strip().split()[0].lower()
    if recorded != sha256_hex(archive):
        raise ValueError(f"checksum mismatch on {spec.archive_filename} while building sha256.sum")
    return f"{recorded}  {spec.archive_filename}"


def combine_checksums(
    dist_dir: Path,
    output_file: Path,
    *,
    require_all_platforms: bool = True,
) -> Path:
    """Merge per-archive `.sha256` sidecars into deterministic `sha256.sum`."""
    ordered = sorted(RELEASE_PLATFORMS, key=lambda p: p.archive_filename)
    lines = [
        line
        for spec in ordered
        if (line := _checksum_line(spec, dist_dir, require_all_platforms=require_all_platforms))
        is not None
    ]
    if not lines:
        raise FileNotFoundError(f"no `.sha256` sidecars found in {dist_dir}")
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return output_file
