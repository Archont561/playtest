from __future__ import annotations

import re
from pathlib import Path

import pytest
from playtest import VERSION, __version__
from playtest._version import _discover_version

SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")


def test_version_matches_pyproject_manifest() -> None:
    pyproject = Path(__file__).resolve().parents[1] / "pyproject.toml"
    match = re.search(
        r'^version\s*=\s*"([^"]+)"', pyproject.read_text(encoding="utf-8"), re.MULTILINE
    )
    assert match is not None
    assert SEMVER_RE.match(VERSION) is not None
    assert VERSION == match.group(1)
    assert __version__ == VERSION


def test_discover_version_reads_custom_pyproject_and_fails_when_missing(tmp_path: Path) -> None:
    custom = tmp_path / "pyproject.toml"
    custom.write_text('[project]\nname = "playtest-py"\nversion = "9.8.7"\n', encoding="utf-8")
    assert _discover_version("nonexistent-playtest-pkg", pyproject=custom) == "9.8.7"

    with pytest.raises(RuntimeError, match="Unable to resolve version"):
        _discover_version("nonexistent-playtest-pkg", pyproject=tmp_path / "missing.toml")
