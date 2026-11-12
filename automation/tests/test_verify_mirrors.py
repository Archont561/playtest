"""Unit tests for cross-OS mirror hash comparison (automation/verify_mirrors.py)."""

from __future__ import annotations

import json
from pathlib import Path

from automation.verify_mirrors import compare_mirror_hash_files, main


def _write_hash_manifest(path: Path, platform_name: str, artifacts: dict[str, str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {"platform": platform_name, "artifacts": artifacts}
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def test_compare_mirror_hash_files_passes_when_all_platforms_match(tmp_path: Path) -> None:
    artifacts = {
        "python/playtest_py-0.1.0-py3-none-any.whl": "a" * 64,
        "typescript/archont561-playtest-0.1.0.tgz": "b" * 64,
    }
    _write_hash_manifest(tmp_path / "linux/hashes.json", "linux-64", artifacts)
    _write_hash_manifest(tmp_path / "macos/hashes.json", "osx-arm64", artifacts)
    _write_hash_manifest(tmp_path / "windows/hashes.json", "win-64", artifacts)

    assert compare_mirror_hash_files([tmp_path]) == 0
    assert main(["compare-hashes", str(tmp_path)]) == 0


def test_compare_mirror_hash_files_reports_byte_drift_across_platforms(tmp_path: Path) -> None:
    _write_hash_manifest(
        tmp_path / "linux/hashes.json",
        "linux-64",
        {"python/playtest_py-0.1.0-py3-none-any.whl": "a" * 64},
    )
    _write_hash_manifest(
        tmp_path / "windows/hashes.json",
        "win-64",
        {"python/playtest_py-0.1.0-py3-none-any.whl": "c" * 64},
    )
    assert compare_mirror_hash_files([tmp_path]) == 1
    assert main(["compare-hashes", str(tmp_path)]) == 1
