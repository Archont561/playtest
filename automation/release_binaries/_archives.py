"""Create deterministic release archives and SHA-256 sidecars."""

from __future__ import annotations

import hashlib
import io
import tarfile
import zipfile
from pathlib import Path

from automation import REPO_ROOT
from automation.release_binaries._platforms import (
    PLATFORM_BY_TRIPLE,
    REPRODUCIBLE_MTIME,
    ReleasePlatform,
)


def sha256_hex(path: Path) -> str:
    """Compute lowercase hexadecimal SHA-256 of `path`."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _write_unix_targz(
    archive_path: Path,
    spec: ReleasePlatform,
    binary_src: Path,
    license_src: Path,
) -> None:
    entries = (
        (f"{spec.archive_stem}/{spec.binary_name}", binary_src.read_bytes(), 0o755),
        (f"{spec.archive_stem}/LICENSE", license_src.read_bytes(), 0o644),
    )
    with tarfile.open(archive_path, "w:gz") as tf:
        for arcname, payload, mode in entries:
            ti = tarfile.TarInfo(name=arcname)
            ti.size = len(payload)
            ti.mtime = REPRODUCIBLE_MTIME
            ti.mode = mode
            tf.addfile(ti, io.BytesIO(payload))


def _write_windows_zip(
    archive_path: Path,
    spec: ReleasePlatform,
    binary_src: Path,
    license_src: Path,
) -> None:
    with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for arcname, payload in (
            (f"{spec.archive_stem}/{spec.binary_name}", binary_src.read_bytes()),
            (f"{spec.archive_stem}/LICENSE", license_src.read_bytes()),
        ):
            zi = zipfile.ZipInfo(filename=arcname, date_time=(1985, 10, 26, 8, 15, 0))
            zi.compress_type = zipfile.ZIP_DEFLATED
            zf.writestr(zi, payload)


def package_platform_archive(
    triple: str,
    binary_src: Path,
    out_dir: Path,
    *,
    root: Path = REPO_ROOT,
) -> tuple[Path, Path]:
    """Create `playtest-cli-<triple>.(tar.gz|zip)` + `.sha256` with `LICENSE` attached."""
    spec = PLATFORM_BY_TRIPLE[triple]
    license_src = root / "LICENSE"
    if not license_src.is_file():
        raise FileNotFoundError(f"missing root LICENSE: {license_src}")
    if not binary_src.is_file():
        raise FileNotFoundError(f"missing compiled binary: {binary_src}")
    out_dir.mkdir(parents=True, exist_ok=True)
    archive_path = out_dir / spec.archive_filename
    if spec.is_windows:
        _write_windows_zip(archive_path, spec, binary_src, license_src)
    else:
        _write_unix_targz(archive_path, spec, binary_src, license_src)
    digest = sha256_hex(archive_path)
    sidecar = out_dir / spec.checksum_filename
    sidecar.write_text(f"{digest}  {spec.archive_filename}\n", encoding="utf-8")
    return archive_path, sidecar
