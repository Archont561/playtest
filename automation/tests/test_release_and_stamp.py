"""Unit tests for release tag planning, milestone parsing, and manifest-only version stamping."""

from __future__ import annotations

import shutil
from pathlib import Path

import pytest

from automation import REPO_ROOT
from automation.release import (
    RELEASE_TARGETS,
    cut_or_summarize_tag,
    parse_milestone_tag,
    plan_release_tag,
    validate_cran_tag,
)
from automation.stamp_version import (
    extract_version_from_tag,
    read_manifest_versions,
    stamp_target,
)


def test_parse_milestone_tags() -> None:
    assert parse_milestone_tag("milestone-v0.2.0") == "0.2.0"
    assert parse_milestone_tag("milestone/M2-v0.2.0") == "0.2.0"
    assert parse_milestone_tag("m-v1.0.0-rc.1") == "1.0.0-rc.1"
    assert parse_milestone_tag("v0.2.0") is None


def test_plan_unified_release_enables_all_targets() -> None:
    planned = plan_release_tag("v0.2.0")
    assert planned["valid"] == "true"
    assert planned["version"] == "0.2.0"
    for target in RELEASE_TARGETS:
        assert planned[target] == "true"


def test_plan_go_subdirectory_tag_enables_only_go() -> None:
    planned = plan_release_tag("go/v0.2.0")
    assert planned["go"] == "true"
    assert planned["rust"] == "false"
    assert planned["python"] == "false"


def test_plan_rejects_milestone_tags_to_prevent_accidental_release() -> None:
    with pytest.raises(ValueError, match="milestone trigger tag"):
        plan_release_tag("milestone-v0.2.0")
    with pytest.raises(ValueError, match="milestone trigger tag"):
        plan_release_tag("m-v0.2.0")


def test_cut_tag_without_token_writes_step_summary(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    summary = tmp_path / "summary.md"
    monkeypatch.setenv("GITHUB_STEP_SUMMARY", str(summary))
    rc = cut_or_summarize_tag("v0.2.0", root=tmp_path, push=False)
    assert rc == 0
    assert "v0.2.0" in summary.read_text(encoding="utf-8")


def test_validate_cran_tag_accepts_stable_and_rejects_prerelease() -> None:
    assert validate_cran_tag("v0.2.0") == 0
    assert validate_cran_tag("r-v0.2.0") == 0
    assert validate_cran_tag("v0.2.0-rc.1") == 1


def _copy_stamp_fixture(dest: Path) -> None:
    rel_files = (
        "Cargo.toml",
        "package.json",
        "python/pyproject.toml",
        "python/src/playtest/__init__.py",
        "typescript/package.json",
        "typescript/src/index.ts",
        "cpp/CMakeLists.txt",
        "cpp/conanfile.py",
        "cpp/include/playtest/playtest.hpp",
        "c/CMakeLists.txt",
        "c/conanfile.py",
        "c/include/playtest/playtest.h",
        "c/src/playtest.c",
        "csharp/src/Playtest/Playtest.csproj",
        "csharp/src/Playtest/Playtest.cs",
        "go/VERSION",
        "go/playtest.go",
        "java/pom.xml",
        "java/src/main/java/io/github/archont561/playtest/Playtest.java",
        "java/src/main/resources/io/github/archont561/playtest/version.properties",
        "r/DESCRIPTION",
        "r/R/version.R",
    )
    for rel in rel_files:
        target = dest / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO_ROOT / rel, target)


def test_stamp_all_updates_manifests_only_and_autodiscovers(tmp_path: Path) -> None:
    _copy_stamp_fixture(tmp_path)
    stamped = stamp_target("all", "v0.4.2", root=tmp_path, author_email="cran@example.org")
    assert stamped == "0.4.2"
    assert extract_version_from_tag("go/v0.4.2") == "0.4.2"

    versions = read_manifest_versions(tmp_path)
    assert versions == {
        "rust": "0.4.2",
        "root_package_json": "0.4.2",
        "python": "0.4.2",
        "typescript": "0.4.2",
        "cpp": "0.4.2",
        "c": "0.4.2",
        "csharp": "0.4.2",
        "go": "0.4.2",
        "java": "0.4.2",
        "r": "0.4.2",
    }
    assert "cran@example.org" in (tmp_path / "r/DESCRIPTION").read_text(encoding="utf-8")

    # Verify source files were NOT mutated and contain no hardcoded stamped string.
    for src_rel in (
        "python/src/playtest/__init__.py",
        "typescript/src/index.ts",
        "cpp/include/playtest/playtest.hpp",
        "c/include/playtest/playtest.h",
        "csharp/src/Playtest/Playtest.cs",
        "go/playtest.go",
        "java/src/main/java/io/github/archont561/playtest/Playtest.java",
        "r/R/version.R",
    ):
        assert "0.4.2" not in (tmp_path / src_rel).read_text(encoding="utf-8")
