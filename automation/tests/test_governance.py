"""Unit tests for advisory expiry, LCOV merging, and automation architecture invariants."""

from __future__ import annotations

import ast
import shutil
from pathlib import Path

import pytest

from automation import REPO_ROOT
from automation._proc import run_cmd
from automation.advisory_exception import verify_advisory_exceptions
from automation.merge_lcov import merge_lcov_files
from automation.repo_check import (
    _check_crate_dependency_boundaries,
    _check_required_and_forbidden,
    check_repository,
)
from automation.rude_clean import WARNING_SUMMARY_RE


def test_real_repository_advisory_exceptions_are_valid() -> None:
    assert verify_advisory_exceptions(REPO_ROOT) == []


def test_stale_advisory_ignore_is_detected(tmp_path: Path) -> None:
    (tmp_path / ".cargo").mkdir()
    (tmp_path / "deny.toml").write_text(
        '[advisories]\nignore = [{ id = "RUSTSEC-2026-0374", reason = "test" }]\n',
        encoding="utf-8",
    )
    (tmp_path / ".cargo/audit.toml").write_text(
        '[advisories]\nignore = ["RUSTSEC-2026-0374"]\n',
        encoding="utf-8",
    )
    (tmp_path / "Cargo.lock").write_text(
        'version = 4\n[[package]]\nname = "playtest"\nversion = "0.1.0"\n',
        encoding="utf-8",
    )
    errors = verify_advisory_exceptions(tmp_path)
    assert any("stale ignore RUSTSEC-2026-0374" in item for item in errors)


def test_merge_lcov_normalizes_paths_and_rejects_empty(tmp_path: Path) -> None:
    with pytest.raises(FileNotFoundError):
        merge_lcov_files(root=tmp_path, require_any=True)

    py_cov = tmp_path / "python" / "coverage"
    py_cov.mkdir(parents=True)
    (py_cov / "lcov-py.info").write_text(
        "SF:src/playtest/__init__.py\nDA:1,1\nLF:1\nLH:1\nend_of_record\n",
        encoding="utf-8",
    )
    count, out = merge_lcov_files(root=tmp_path)
    assert count == 1
    assert "SF:python/src/playtest/__init__.py" in out.read_text(encoding="utf-8")


def test_real_repository_hygiene_passes() -> None:
    assert check_repository(REPO_ROOT, skip_shellcheck=True) == []


def test_crate_dependency_boundary_rejects_transitive_cli_or_assets_leak(tmp_path: Path) -> None:
    (tmp_path / "Cargo.lock").write_text(
        "version = 4\n"
        '[[package]]\nname = "playtest"\nversion = "0.1.0"\n'
        'dependencies = ["playtest-core"]\n'
        '[[package]]\nname = "playtest-core"\nversion = "0.1.0"\n'
        'dependencies = ["clifacet 0.5.0"]\n',
        encoding="utf-8",
    )
    errors = _check_crate_dependency_boundaries(tmp_path)
    assert any("`playtest` depends on `clifacet`" in item for item in errors)
    assert any("only `playtest-cli` may depend on `clifacet`" in item for item in errors)


def test_automation_modules_obey_doit_print_and_shell_boundaries() -> None:
    automation_dir = REPO_ROOT / "automation"
    for py_file in sorted(automation_dir.rglob("*.py")):
        if "tests" in py_file.parts:
            continue
        tree = ast.parse(py_file.read_text(encoding="utf-8"), filename=str(py_file))
        for node in ast.walk(tree):
            if py_file.name != "dodo.py" and isinstance(node, (ast.Import, ast.ImportFrom)):
                mod = getattr(node, "module", "") or ""
                names = [alias.name for alias in node.names]
                assert "doit" not in (mod, *names), f"{py_file.name} must not import doit"
            if isinstance(node, ast.Call):
                if isinstance(node.func, ast.Name):
                    assert node.func.id != "print", f"{py_file.name} must not call print()"
                for kw in node.keywords:
                    if kw.arg == "shell" and isinstance(kw.value, ast.Constant):
                        assert kw.value.value is not True, f"{py_file.name} uses shell=True"


def test_clean_code_preset_configs_and_rude_warning_detector() -> None:
    assert (REPO_ROOT / "vendor/clean-code-presets/alint/strict.yml").is_file()
    assert (REPO_ROOT / "vendor/clean-code-presets/semgrep/pedantic.yml").is_file()
    assert (REPO_ROOT / "vendor/clean-code-presets/ast-grep/pedantic/sgconfig.yml").is_file()
    assert (REPO_ROOT / "vendor/clean-code-presets/vale/strict.ini").is_file()
    alint_text = (REPO_ROOT / ".alint.yml").read_text(encoding="utf-8")
    assert "id: forbid-non-restore-scripts" in alint_text
    assert "!scripts/restore.sh" in alint_text
    assert WARNING_SUMMARY_RE.search("Found 0 error(s) and 1 warning(s)") is not None
    assert WARNING_SUMMARY_RE.search("Found 0 error(s) and 0 warning(s)") is None


def test_scripts_directory_allows_only_restore_sh(tmp_path: Path) -> None:
    scripts_dir = tmp_path / "scripts"
    scripts_dir.mkdir(parents=True)
    (scripts_dir / "restore.sh").write_text("#!/bin/sh\n", encoding="utf-8")
    (scripts_dir / "extra.py").write_text("# forbidden\n", encoding="utf-8")
    errors = _check_required_and_forbidden(tmp_path)
    assert "unexpected file in scripts/ (use automation/): extra.py" in errors
    if shutil.which("alint") is not None:
        cfg = tmp_path / ".alint-scripts.yml"
        cfg.write_text(
            "version: 1\nrules:\n"
            "  - id: forbid-non-restore-scripts\n"
            "    kind: file_absent\n"
            "    paths:\n"
            "      - scripts/**\n"
            '      - "!scripts/restore.sh"\n'
            "    level: error\n"
            "    message: Only restore.sh allowed.\n",
            encoding="utf-8",
        )
        res = run_cmd(["alint", "check", "--config", str(cfg), str(tmp_path)], capture=True)
        assert res.returncode != 0
        (scripts_dir / "extra.py").unlink()
        ok_res = run_cmd(["alint", "check", "--config", str(cfg), str(tmp_path)], capture=True)
        assert ok_res.returncode == 0
