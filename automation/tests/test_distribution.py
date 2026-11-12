"""Unit tests for release binary archives and npm platform/wrapper packaging."""

from __future__ import annotations

import json
import tarfile
import zipfile
from pathlib import Path

import pytest

from automation import REPO_ROOT
from automation.npm_packages import (
    NPM_PLATFORMS,
    assert_no_lifecycle_scripts,
    build_npm_tarballs,
)
from automation.publish_registries import _partition_npm_tarballs
from automation.release_binaries import (
    RELEASE_PLATFORMS,
    combine_checksums,
    package_platform_archive,
    verify_platform_archive,
    verify_version_alignment,
)


def test_package_and_verify_all_five_release_archives(tmp_path: Path) -> None:
    fake_bin = tmp_path / "fake-playtest"
    fake_bin.write_bytes(b"#!/usr/bin/env sh\necho 'playtest 0.1.0'\n")
    out_dir = tmp_path / "dist"

    for spec in RELEASE_PLATFORMS:
        archive, sidecar = package_platform_archive(spec.triple, fake_bin, out_dir, root=REPO_ROOT)
        assert archive.is_file()
        assert sidecar.is_file()
        errs = verify_platform_archive(spec.triple, "v0.1.0", out_dir, execute_binary=False)
        assert errs == []

    sum_file = combine_checksums(out_dir, out_dir / "sha256.sum")
    sum_lines = [ln for ln in sum_file.read_text(encoding="utf-8").splitlines() if ln.strip()]
    assert len(sum_lines) == 5

    # Tamper with sidecar to confirm checksum mismatch is caught
    bad_sidecar = out_dir / "playtest-cli-x86_64-pc-windows-msvc.zip.sha256"
    bad_sidecar.write_text(
        f"{'0' * 64}  playtest-cli-x86_64-pc-windows-msvc.zip\n", encoding="utf-8"
    )
    mismatch = verify_platform_archive(
        "x86_64-pc-windows-msvc", "v0.1.0", out_dir, execute_binary=False
    )
    assert mismatch and "SHA-256 mismatch" in mismatch[0]


def test_windows_zip_and_unix_targz_both_bundle_license(tmp_path: Path) -> None:
    fake_bin = tmp_path / "bin"
    fake_bin.write_bytes(b"binary")
    out_dir = tmp_path / "dist"

    unix_arc, _ = package_platform_archive(
        "x86_64-unknown-linux-musl", fake_bin, out_dir, root=REPO_ROOT
    )
    with tarfile.open(unix_arc, "r:gz") as tf:
        names = {m.name for m in tf.getmembers()}
    assert "playtest-cli-x86_64-unknown-linux-musl/LICENSE" in names
    assert "playtest-cli-x86_64-unknown-linux-musl/playtest" in names

    win_arc, _ = package_platform_archive(
        "x86_64-pc-windows-msvc", fake_bin, out_dir, root=REPO_ROOT
    )
    with zipfile.ZipFile(win_arc, "r") as zf:
        znames = set(zf.namelist())
    assert "playtest-cli-x86_64-pc-windows-msvc/LICENSE" in znames
    assert "playtest-cli-x86_64-pc-windows-msvc/playtest.exe" in znames


def test_verify_version_alignment_detects_manifest_drift() -> None:
    assert verify_version_alignment("v0.1.0", root=REPO_ROOT) == []
    drift = verify_version_alignment("v9.9.9", root=REPO_ROOT)
    assert len(drift) >= 9


def test_npm_tarball_generation_no_lifecycle_scripts_and_publish_order(tmp_path: Path) -> None:
    ts_dist = REPO_ROOT / "typescript" / "dist" / "index.js"
    created_dist = False
    if not ts_dist.is_file():
        ts_dist.parent.mkdir(parents=True, exist_ok=True)
        ts_dist.write_text('export const VERSION = "0.1.0";\n', encoding="utf-8")
        created_dist = True
    try:
        fake_bin = tmp_path / "playtest"
        fake_bin.write_bytes(b"#!/usr/bin/env sh\nexit 0\n")
        binaries = {spec.triple: fake_bin for spec in NPM_PLATFORMS}
        out_dir = tmp_path / "npm-out"
        tarballs = build_npm_tarballs(REPO_ROOT, binaries, out_dir, use_cargo_npm=False)
        assert len(tarballs) == 6

        platform_pkgs, wrapper_pkgs = _partition_npm_tarballs(tarballs)
        assert len(platform_pkgs) == 5
        assert len(wrapper_pkgs) == 1
        assert wrapper_pkgs[0].name == "archont561-playtest-0.1.0.tgz"

        with tarfile.open(wrapper_pkgs[0], "r:gz") as tf:
            pkg_raw = tf.extractfile("package/package.json")
            assert pkg_raw is not None
            pkg_data = json.loads(pkg_raw.read().decode("utf-8"))
            bin_pkg = tf.extractfile("package/bin/package.json")
            assert bin_pkg is not None
            assert json.loads(bin_pkg.read().decode("utf-8")) == {"type": "commonjs"}
        assert "scripts" not in pkg_data
        assert len(pkg_data["optionalDependencies"]) == 5
    finally:
        if created_dist and ts_dist.parent.exists():
            ts_dist.unlink(missing_ok=True)

    bad_pkg = tmp_path / "bad-package.json"
    bad_pkg.write_text('{"name":"x","scripts":{"postinstall":"node dl.js"}}', encoding="utf-8")
    with pytest.raises(RuntimeError, match="forbidden lifecycle script"):
        assert_no_lifecycle_scripts(bad_pkg)
