"""Unit tests for `playtest-assets` SDK staging, manifest validation, and symlink checks."""

from __future__ import annotations

from pathlib import Path

import pytest

from automation import REPO_ROOT
from automation.stage_assets import (
    MANIFEST_REL,
    STAGED_REL,
    clean_staged_assets,
    is_forbidden_asset_path,
    load_assets_manifest,
    main,
    stage_assets,
    validate_no_symlink_components,
)


def _write_minimal_fixture_repo(root: Path, files_list: str = '["src/mod.py"]') -> Path:
    manifest = root / MANIFEST_REL
    manifest.parent.mkdir(parents=True, exist_ok=True)
    manifest.write_text(
        f'[languages.python]\nsource_dir = "python"\nfiles = {files_list}\n',
        encoding="utf-8",
    )
    src = root / "python" / "src" / "mod.py"
    src.parent.mkdir(parents=True, exist_ok=True)
    src.write_text('VERSION = "0.1.0"\n', encoding="utf-8")
    return src


def test_real_assets_manifest_covers_all_eight_languages() -> None:
    manifest = load_assets_manifest(REPO_ROOT)
    assert set(manifest) == {"python", "typescript", "cpp", "c", "csharp", "go", "java", "r"}
    for lang, spec in manifest.items():
        files = spec["files"]
        assert files, f"{lang} must list at least one published file"
        for rel_str in files:
            assert not is_forbidden_asset_path(rel_str), f"{lang}:{rel_str} is forbidden"


def test_stage_and_clean_roundtrip_on_fixture_repo(tmp_path: Path) -> None:
    src = _write_minimal_fixture_repo(tmp_path)
    staged = stage_assets(tmp_path)
    expected = tmp_path / STAGED_REL / "python" / "src" / "mod.py"
    assert staged == [expected]
    assert expected.read_bytes() == src.read_bytes()
    assert not expected.is_symlink()

    assert main(["--root", str(tmp_path), "--clean"]) == 0
    assert not (tmp_path / STAGED_REL).exists()


def test_stage_assets_rejects_missing_listed_file(tmp_path: Path) -> None:
    _write_minimal_fixture_repo(tmp_path, '["src/mod.py", "src/missing.py"]')
    with pytest.raises(FileNotFoundError, match="missing listed SDK asset"):
        stage_assets(tmp_path)
    assert main(["--root", str(tmp_path)]) == 1


def test_stage_assets_rejects_symlinks_in_any_component(tmp_path: Path) -> None:
    src = _write_minimal_fixture_repo(tmp_path, '["src/link.py"]')
    link = tmp_path / "python" / "src" / "link.py"
    link.symlink_to(src)
    err = validate_no_symlink_components(tmp_path / "python", Path("src/link.py"))
    assert err is not None
    assert "symbolic link is forbidden" in err
    with pytest.raises(FileNotFoundError, match="symbolic link is forbidden"):
        stage_assets(tmp_path)


def test_stage_assets_rejects_forbidden_test_or_lockfile_paths(tmp_path: Path) -> None:
    _write_minimal_fixture_repo(tmp_path, '["go.sum"]')
    (tmp_path / "python" / "go.sum").write_text("", encoding="utf-8")
    with pytest.raises(ValueError, match="forbidden test/lockfile/cache path"):
        stage_assets(tmp_path)
    clean_staged_assets(tmp_path)
