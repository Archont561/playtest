"""Package, checksum, and verify GitHub Release CLI binary archives (Spec 0012 §§3, 8)."""

from __future__ import annotations

import argparse
import hashlib
import io
import os
import shutil
import tarfile
import tempfile
import zipfile
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd
from automation.stamp_version import extract_version_from_tag, read_manifest_versions

REPRODUCIBLE_MTIME: int = 499_162_500


@dataclass(frozen=True)
class ReleasePlatform:
    """Supported binary distribution target for GitHub Releases (Spec 0012 §3)."""

    triple: str
    runner: str
    archive_ext: str
    conda_platform: str

    @property
    def is_windows(self) -> bool:
        """Whether this target uses Windows `.zip` and `.exe` conventions."""
        return self.archive_ext == ".zip"

    @property
    def binary_name(self) -> str:
        """Executable filename inside the archive."""
        return "playtest.exe" if self.is_windows else "playtest"

    @property
    def archive_stem(self) -> str:
        """Root prefix and filename stem (`playtest-cli-<triple>`)."""
        return f"playtest-cli-{self.triple}"

    @property
    def archive_filename(self) -> str:
        """Release asset filename (`playtest-cli-<triple>.tar.gz` or `.zip`)."""
        return f"{self.archive_stem}{self.archive_ext}"

    @property
    def checksum_filename(self) -> str:
        """Per-archive SHA-256 sidecar filename."""
        return f"{self.archive_filename}.sha256"


RELEASE_PLATFORMS: tuple[ReleasePlatform, ...] = (
    ReleasePlatform("x86_64-unknown-linux-musl", "ubuntu-24.04", ".tar.gz", "linux-64"),
    ReleasePlatform("aarch64-unknown-linux-musl", "ubuntu-24.04-arm", ".tar.gz", "linux-aarch64"),
    ReleasePlatform("x86_64-apple-darwin", "macos-15-intel", ".tar.gz", "osx-64"),
    ReleasePlatform("aarch64-apple-darwin", "macos-15", ".tar.gz", "osx-arm64"),
    ReleasePlatform("x86_64-pc-windows-msvc", "windows-2025", ".zip", "win-64"),
)

PLATFORM_BY_TRIPLE: dict[str, ReleasePlatform] = {p.triple: p for p in RELEASE_PLATFORMS}


def sha256_hex(path: Path) -> str:
    """Compute lowercase hexadecimal SHA-256 of `path`."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_version_alignment(
    expected_version_or_tag: str,
    *,
    root: Path = REPO_ROOT,
    cli_bin: Path | None = None,
) -> list[str]:
    """Return errors when any manifest, CLI binary, or embedded SDK version drifts."""
    expected = extract_version_from_tag(expected_version_or_tag)
    errors: list[str] = []
    versions = read_manifest_versions(root)
    conda_manifest = root / "conda" / "pixi.toml"
    if conda_manifest.is_file():
        import tomllib

        conda_ver = str(
            tomllib.loads(conda_manifest.read_text(encoding="utf-8"))["workspace"]["version"]
        )
        versions["conda"] = conda_ver

    for name, actual in sorted(versions.items()):
        want = expected.split("-", 1)[0] if name == "r" else expected
        if actual != want:
            errors.append(f"manifest `{name}` has version {actual!r}, expected {want!r}")

    if cli_bin is not None:
        errors.extend(_verify_binary_and_embedded_version(cli_bin, expected))
    return errors


def _verify_binary_and_embedded_version(cli_bin: Path, expected: str) -> list[str]:
    cli_bin = cli_bin.resolve()
    errors: list[str] = []
    res = run_cmd([str(cli_bin), "version"], capture=True)
    if res.returncode != 0:
        return [f"`{cli_bin} version` exited with {res.returncode}: {res.stderr.strip()}"]
    first_line = res.stdout.strip().splitlines()[0] if res.stdout.strip() else ""
    if first_line != f"playtest {expected}":
        errors.append(f"CLI reported {first_line!r}, expected {'playtest ' + expected!r}")

    with tempfile.TemporaryDirectory(prefix="playtest-ver-sync-") as tmp:
        ws = Path(tmp)
        if run_cmd([str(cli_bin), "init"], cwd=ws, capture=True).returncode != 0:
            errors.append("`playtest init` failed during embedded SDK version check")
            return errors
        if run_cmd([str(cli_bin), "sync"], cwd=ws, capture=True).returncode != 0:
            errors.append("`playtest sync` failed during embedded SDK version check")
            return errors
        whl = ws / ".playtest" / "python" / f"playtest_py-{expected}-py3-none-any.whl"
        tgz = ws / ".playtest" / "typescript" / f"archont561-playtest-{expected}.tgz"
        if not whl.is_file() or not tgz.is_file():
            errors.append(f"embedded SDK mirrors did not match version {expected!r}")
    return errors


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

    with tempfile.TemporaryDirectory(prefix="playtest-verify-archive-") as tmp:
        unpack_dir = Path(tmp)
        _safe_extract(archive, unpack_dir, is_windows=spec.is_windows)
        lic_matches = list(unpack_dir.rglob("LICENSE"))
        bin_matches = list(unpack_dir.rglob(spec.binary_name))
        if not lic_matches:
            return [f"archive {archive.name} is missing LICENSE"]
        if not bin_matches:
            return [f"archive {archive.name} is missing {spec.binary_name}"]
        exe = bin_matches[0]
        if not spec.is_windows:
            exe.chmod(0o755)
        if execute_binary:
            return _verify_binary_and_embedded_version(exe, expected)
    return []


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


def combine_checksums(
    dist_dir: Path,
    output_file: Path,
    *,
    require_all_platforms: bool = True,
) -> Path:
    """Merge per-archive `.sha256` sidecars into deterministic `sha256.sum`."""
    lines: list[str] = []
    for spec in sorted(RELEASE_PLATFORMS, key=lambda p: p.archive_filename):
        sidecar = dist_dir / spec.checksum_filename
        archive = dist_dir / spec.archive_filename
        if not sidecar.is_file() or not archive.is_file():
            if require_all_platforms:
                raise FileNotFoundError(
                    f"missing release asset or checksum for {spec.triple}: {spec.archive_filename}"
                )
            continue
        recorded = sidecar.read_text(encoding="utf-8").strip().split()[0].lower()
        actual = sha256_hex(archive)
        if recorded != actual:
            raise ValueError(
                f"checksum mismatch on {spec.archive_filename} while building sha256.sum"
            )
        lines.append(f"{recorded}  {spec.archive_filename}")
    if not lines:
        raise FileNotFoundError(f"no `.sha256` sidecars found in {dist_dir}")
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return output_file


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Manage GitHub Release CLI binary archives.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    sub = parser.add_subparsers(dest="command", required=True)

    p_ver = sub.add_parser("verify-version", help="Fail on manifest/CLI/asset version drift.")
    p_ver.add_argument("--version", default=os.environ.get("RELEASE_TAG", ""))
    p_ver.add_argument("--cli-bin", type=Path, default=None)

    p_pkg = sub.add_parser("package", help="Package compiled binary + LICENSE + .sha256.")
    p_pkg.add_argument("--target", choices=tuple(PLATFORM_BY_TRIPLE.keys()), required=True)
    p_pkg.add_argument("--binary", type=Path, required=True)
    p_pkg.add_argument("--out-dir", type=Path, required=True)

    p_chk = sub.add_parser("verify-archive", help="Verify .sha256, LICENSE, and playtest version.")
    p_chk.add_argument("--target", choices=tuple(PLATFORM_BY_TRIPLE.keys()), required=True)
    p_chk.add_argument("--version", default=os.environ.get("RELEASE_TAG", ""))
    p_chk.add_argument("--dist-dir", type=Path, required=True)
    p_chk.add_argument("--no-exec", action="store_true")

    p_bld = sub.add_parser("build-target", help="Compile, package, and verify one CLI target.")
    p_bld.add_argument("--target", choices=tuple(PLATFORM_BY_TRIPLE.keys()), required=True)
    p_bld.add_argument("--version", default=os.environ.get("RELEASE_TAG", ""))
    p_bld.add_argument("--raw-dir", type=Path, default=Path("raw-bin"))
    p_bld.add_argument("--dist-dir", type=Path, default=Path("release-assets"))

    p_sum = sub.add_parser("combine-checksums", help="Build combined sha256.sum.")
    p_sum.add_argument("--dist-dir", type=Path, required=True)
    p_sum.add_argument("--output", type=Path, required=True)
    p_sum.add_argument("--allow-subset", action="store_true")
    return parser


def _build_and_verify_target(
    root: Path,
    triple: str,
    version_or_tag: str,
    raw_dir: Path,
    dist_dir: Path,
) -> int:
    spec = PLATFORM_BY_TRIPLE[triple]
    ts_res = run_cmd(
        [
            "bun",
            "build",
            "typescript/src/index.ts",
            "--outdir",
            "typescript/dist",
            "--target",
            "node",
        ],
        cwd=root,
    )
    if ts_res.returncode != 0:
        return ts_res.returncode
    cargo_res = run_cmd(
        ["cargo", "build", "--release", "--locked", "-p", "playtest-cli", "--target", triple],
        cwd=root,
    )
    if cargo_res.returncode != 0:
        return cargo_res.returncode
    compiled = root / "target" / triple / "release" / spec.binary_name
    raw_slot = raw_dir / triple / spec.binary_name
    raw_slot.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(compiled, raw_slot)
    package_platform_archive(triple, raw_slot, dist_dir, root=root)
    errs = verify_platform_archive(triple, version_or_tag, dist_dir, execute_binary=True)
    for err in errs:
        emit_err(f"[release-binaries] ERROR: {err}")
    return 1 if errs else 0


def _dispatch_subcommand(args: argparse.Namespace, root: Path) -> int:
    if args.command == "build-target":
        return _build_and_verify_target(
            root,
            args.target,
            args.version,
            args.raw_dir.resolve(),
            args.dist_dir.resolve(),
        )
    if args.command == "verify-version":
        errs = verify_version_alignment(args.version, root=root, cli_bin=args.cli_bin)
        for err in errs:
            emit_err(f"[release-binaries] ERROR: {err}")
        if not errs:
            emit_line("[release-binaries] OK: version alignment verified.")
        return 1 if errs else 0
    if args.command == "package":
        archive, sidecar = package_platform_archive(
            args.target, args.binary.resolve(), args.out_dir.resolve(), root=root
        )
        emit_line(f"[release-binaries] packaged {archive.name} and {sidecar.name}")
        return 0
    if args.command == "verify-archive":
        errs = verify_platform_archive(
            args.target,
            args.version,
            args.dist_dir.resolve(),
            execute_binary=not args.no_exec,
        )
        for err in errs:
            emit_err(f"[release-binaries] ERROR: {err}")
        if not errs:
            emit_line(f"[release-binaries] OK: verified archive for {args.target}")
        return 1 if errs else 0
    out = combine_checksums(
        args.dist_dir.resolve(),
        args.output.resolve(),
        require_all_platforms=not args.allow_subset,
    )
    emit_line(f"[release-binaries] wrote {out}")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for GitHub Release archive packaging and verification."""
    args = _build_parser().parse_args(list(argv) if argv is not None else None)
    try:
        return _dispatch_subcommand(args, args.root.resolve())
    except (FileNotFoundError, KeyError, ValueError, shutil.Error) as err:
        emit_err(f"[release-binaries] ERROR: {err}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
