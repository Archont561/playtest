"""Unit tests for advisory expiry, LCOV merging, and automation architecture invariants."""

from __future__ import annotations

import ast
import json
import shutil
from collections.abc import Iterator, Sequence
from pathlib import Path

import pytest

from automation import REPO_ROOT
from automation._proc import CommandResult, run_cmd
from automation.advisory_exception import verify_advisory_exceptions
from automation.merge_lcov import merge_lcov_files
from automation.repo_check import (
    CANONICAL_DESCRIPTION_LEAD,
    _check_canonical_description,
    _check_crate_dependency_boundaries,
    _check_manifest_versions_and_autodiscovery,
    _check_required_and_forbidden,
    check_repository,
)
from automation.rude_clean import (
    WARNING_SUMMARY_RE,
    parse_rude_diagnostics,
    run_rude_clean,
)


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


def test_manifest_autodiscovery_check_detects_drift_and_hardcoded_source(tmp_path: Path) -> None:
    from automation.tests.test_release_and_stamp import _copy_stamp_fixture

    _copy_stamp_fixture(tmp_path)
    assert _check_manifest_versions_and_autodiscovery(tmp_path) == []

    (tmp_path / "go/VERSION").write_text("0.9.9\n", encoding="utf-8")
    drift_errors = _check_manifest_versions_and_autodiscovery(tmp_path)
    assert any("manifest version drift in `go`" in item for item in drift_errors)

    (tmp_path / "go/VERSION").write_text("0.1.0\n", encoding="utf-8")
    (tmp_path / "go/playtest.go").write_text(
        'package playtest\nconst Version = "0.1.0"\n', encoding="utf-8"
    )
    hardcode_errors = _check_manifest_versions_and_autodiscovery(tmp_path)
    assert any("hardcoded version literal" in item for item in hardcode_errors)


def test_canonical_description_guard_rejects_positioning_drift(tmp_path: Path) -> None:
    assert _check_canonical_description(REPO_ROOT) == []

    pkg = tmp_path / "package.json"
    pkg.write_text(
        '{"description": "A Playwright-inspired testing framework for Python"}\n', encoding="utf-8"
    )
    assert any(
        CANONICAL_DESCRIPTION_LEAD in item for item in _check_canonical_description(tmp_path)
    )

    pkg.write_text(f'{{"description": "{CANONICAL_DESCRIPTION_LEAD}: fixtures for Python"}}\n')
    assert _check_canonical_description(tmp_path) == []


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


def _iter_automation_sources() -> Iterator[Path]:
    automation_dir = REPO_ROOT / "automation"
    for py_file in sorted(automation_dir.rglob("*.py")):
        if "tests" not in py_file.parts:
            yield py_file


def _assert_no_doit_import(py_file: Path, node: ast.AST) -> None:
    if py_file.name == "dodo.py" or not isinstance(node, (ast.Import, ast.ImportFrom)):
        return
    mod = getattr(node, "module", "") or ""
    names = [alias.name for alias in node.names]
    assert "doit" not in (mod, *names), f"{py_file.name} must not import doit"


def _assert_no_print_or_shell_true(py_file: Path, node: ast.AST) -> None:
    if not isinstance(node, ast.Call):
        return
    if isinstance(node.func, ast.Name):
        assert node.func.id != "print", f"{py_file.name} must not call print()"
    for kw in node.keywords:
        if kw.arg == "shell" and isinstance(kw.value, ast.Constant):
            assert kw.value.value is not True, f"{py_file.name} uses shell=True"


def test_automation_modules_obey_doit_print_and_shell_boundaries() -> None:
    for py_file in _iter_automation_sources():
        tree = ast.parse(py_file.read_text(encoding="utf-8"), filename=str(py_file))
        for node in ast.walk(tree):
            _assert_no_doit_import(py_file, node)
            _assert_no_print_or_shell_true(py_file, node)


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


def test_parse_rude_diagnostics_reads_ndjson_diagnostic_lines() -> None:
    raw = "\n".join(
        (
            '{"file": "automation/x.py", "line": 3, "code": "CC101", "severity": "warning"}',
            "   ",
            '{"file": "automation/y.py", "line": 9, "code": "CC901", "severity": "info"}',
        )
    )
    diagnostics = parse_rude_diagnostics(raw)
    assert [entry["code"] for entry in diagnostics] == ["CC101", "CC901"]
    assert parse_rude_diagnostics("") == []


def _diagnostic_line(root: Path, code: str, severity: str) -> str:
    payload = {
        "file": f"{root}/clean.py",
        "line": 2,
        "column": 1,
        "code": code,
        "severity": severity,
    }
    return json.dumps(payload) + "\n"


def _patch_rude_run(monkeypatch: pytest.MonkeyPatch, result: CommandResult) -> list[list[str]]:
    calls: list[list[str]] = []

    def fake_run(cmd: Sequence[str], **_kwargs: object) -> CommandResult:
        calls.append(list(cmd))
        return result

    monkeypatch.setattr("automation.rude_clean.run_cmd", fake_run)
    return calls


def test_run_rude_clean_passes_without_diagnostics(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (tmp_path / "rude.toml").write_text("[tool.rude]\n", encoding="utf-8")
    calls = _patch_rude_run(monkeypatch, CommandResult(returncode=0, stdout="", stderr=""))
    assert run_rude_clean(tmp_path, ("clean.py",)) == 0
    assert calls and calls[0][-1] == "clean.py"


@pytest.mark.parametrize(("code", "severity"), [("CC101", "warning"), ("CC901", "info")])
def test_run_rude_clean_fails_on_any_diagnostic(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, code: str, severity: str
) -> None:
    (tmp_path / "rude.toml").write_text("[tool.rude]\n", encoding="utf-8")
    stdout = _diagnostic_line(tmp_path, code, severity)
    _patch_rude_run(monkeypatch, CommandResult(returncode=0, stdout=stdout, stderr=""))
    assert run_rude_clean(tmp_path, ("clean.py",)) == 1


def test_run_rude_clean_propagates_failing_rude_exit_code(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (tmp_path / "rude.toml").write_text("[tool.rude]\n", encoding="utf-8")
    _patch_rude_run(monkeypatch, CommandResult(returncode=2, stdout="", stderr="boom"))
    assert run_rude_clean(tmp_path, ("clean.py",)) == 2


def _copy_real_alint_base(dst_root: Path) -> Path:
    for rel in (
        "LICENSE",
        "Cargo.toml",
        "Cargo.lock",
        "deny.toml",
        ".cargo/audit.toml",
        "pixi-sandbox.toml",
        "typescript/bunup.config.ts",
        "typescript/package.json",
        "typescript/tsconfig.json",
        "vendor/clean-code-presets/UPSTREAM.pin",
        "vendor/clean-code-presets/VENDOR.sha256",
    ):
        target = dst_root / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO_ROOT / rel, target)
    cfg = dst_root / ".alint.yml"
    shutil.copy2(REPO_ROOT / ".alint.yml", cfg)
    return cfg


_GOOD_SRC_STMT = 'import { v } from "@/src/version";\nexport const X = v;\n'
_GOOD_TEST_STMT = 'import { X } from "@/src/index";\n'


def _write_ts_fixtures(tmp_path: Path) -> tuple[Path, Path]:
    scripts_dir = tmp_path / "scripts"
    scripts_dir.mkdir(parents=True)
    (scripts_dir / "restore.sh").write_text("#!/bin/sh\n", encoding="utf-8")
    good_src = tmp_path / "typescript/src/index.ts"
    good_test = tmp_path / "typescript/tests/version.test.ts"
    good_src.parent.mkdir(parents=True, exist_ok=True)
    good_test.parent.mkdir(parents=True, exist_ok=True)
    good_src.write_text(_GOOD_SRC_STMT, encoding="utf-8")
    good_test.write_text(_GOOD_TEST_STMT, encoding="utf-8")
    return good_src, good_test


def _assert_rejected_import(cfg: Path, root: Path, target: Path, bad_stmt: str) -> None:
    target.write_text(bad_stmt, encoding="utf-8")
    res = run_cmd(["alint", "check", "--config", str(cfg), str(root)], capture=True)
    assert res.returncode != 0
    assert "forbid-ts-relative-imports" in (res.stdout + res.stderr)


def test_alint_enforces_ts_bunup_and_forbids_relative_imports(tmp_path: Path) -> None:
    if shutil.which("alint") is None:
        return
    good_src, good_test = _write_ts_fixtures(tmp_path)
    cfg = _copy_real_alint_base(tmp_path)
    assert (
        run_cmd(["alint", "check", "--config", str(cfg), str(tmp_path)], capture=True).returncode
        == 0
    )
    for target, bad_stmt in (
        (good_src, 'import { v } from "./version";\n'),
        (good_src, 'import { v } from "../other";\n'),
        (good_test, 'import { X } from "../src/index";\n'),
        (good_test, 'import { X } from "../../nested/index";\n'),
    ):
        _assert_rejected_import(cfg, tmp_path, target, bad_stmt)
        target.write_text(
            _GOOD_SRC_STMT if target == good_src else _GOOD_TEST_STMT, encoding="utf-8"
        )


def test_scripts_directory_allows_only_restore_sh(tmp_path: Path) -> None:
    scripts_dir = tmp_path / "scripts"
    scripts_dir.mkdir(parents=True)
    (scripts_dir / "restore.sh").write_text("#!/bin/sh\n", encoding="utf-8")
    (scripts_dir / "extra.py").write_text("# forbidden\n", encoding="utf-8")
    errors = _check_required_and_forbidden(tmp_path)
    assert "unexpected file in scripts/ (use automation/): extra.py" in errors
    (scripts_dir / "extra.py").unlink()
    if shutil.which("alint") is None:
        return
    cfg = _copy_real_alint_base(tmp_path)
    assert (
        run_cmd(["alint", "check", "--config", str(cfg), str(tmp_path)], capture=True).returncode
        == 0
    )
    for bad_rel in ("extra.py", ".hidden", "nested/helper.py"):
        bad_file = scripts_dir / bad_rel
        bad_file.parent.mkdir(parents=True, exist_ok=True)
        bad_file.write_text("# forbidden\n", encoding="utf-8")
        res = run_cmd(["alint", "check", "--config", str(cfg), str(tmp_path)], capture=True)
        assert res.returncode != 0
        assert "forbid-non-restore-scripts" in (res.stdout + res.stderr)
        bad_file.unlink()
    if (scripts_dir / "nested").exists():
        (scripts_dir / "nested").rmdir()
