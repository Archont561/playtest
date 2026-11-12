"""PyDoit task graph for Playtest repository governance and automation."""

from __future__ import annotations

import sys
from typing import Any

DOIT_CONFIG: dict[str, Any] = {
    "default_tasks": ["fast"],
    "verbosity": 2,
}

PYTHON: str = sys.executable


def _task(doc: str, cmd: list[str]) -> dict[str, Any]:
    return {
        "actions": [cmd],
        "doc": doc,
        "uptodate": [False],
    }


def task_repo_check() -> dict[str, Any]:
    """Verify repository structure, vendor checksums, UTF-8 BOMs, and restore.sh."""
    return _task(
        "Verify repository layout and vendor integrity", [PYTHON, "-m", "automation.repo_check"]
    )


def task_alint() -> dict[str, Any]:
    """Run vendored strict Alint preset and local .alint.yml rules."""
    return _task(
        "Run Alint strict and local rules",
        [PYTHON, "-m", "automation.clean_code", "--engine", "alint"],
    )


def task_semgrep() -> dict[str, Any]:
    """Run vendored Semgrep pedantic security and anti-pattern rules."""
    return _task(
        "Run Semgrep pedantic rules", [PYTHON, "-m", "automation.clean_code", "--engine", "semgrep"]
    )


def task_ast_grep() -> dict[str, Any]:
    """Run vendored ast-grep pedantic structural AST rules."""
    return _task(
        "Run ast-grep pedantic rules",
        [PYTHON, "-m", "automation.clean_code", "--engine", "ast-grep"],
    )


def task_vale() -> dict[str, Any]:
    """Run vendored Vale strict prose linter on repository documentation."""
    return _task(
        "Run Vale strict prose linter", [PYTHON, "-m", "automation.clean_code", "--engine", "vale"]
    )


def task_rude() -> dict[str, Any]:
    """Run rude + rude-clean-code complexity bounds on Python sources."""
    return _task("Run rude-clean-code complexity bounds", [PYTHON, "-m", "automation.rude_clean"])


def task_clean_code() -> dict[str, Any]:
    """Aggregate all five Clean Code preset and complexity engines."""
    return {
        "actions": None,
        "task_dep": ["alint", "semgrep", "ast_grep", "vale", "rude"],
        "doc": "Run all 5 Clean Code governance engines",
    }


def task_lint_workflows() -> dict[str, Any]:
    """Validate GitHub Actions workflows via actionlint."""
    return _task("Validate GitHub Actions workflows", ["actionlint"])


def task_lint_toml() -> dict[str, Any]:
    """Check TOML formatting across workspace manifests via taplo."""
    return _task("Verify TOML formatting with taplo", ["taplo", "fmt", "--check"])


def task_sandbox_check() -> dict[str, Any]:
    """Verify pixi-sandbox generated workflows and scripts/restore.sh are up to date."""
    return _task(
        "Verify pixi-sandbox generated files",
        ["pixi-sandbox", "init", "--check", "--script-path", "scripts/restore.sh"],
    )


def task_lint_automation() -> dict[str, Any]:
    """Check Python formatting, lint rules, and Clean Code bounds on automation/ and python/."""
    return {
        "actions": [
            ["ruff", "format", "--check", "automation", "python/src", "python/tests"],
            ["ruff", "check", "automation", "python/src", "python/tests"],
        ],
        "task_dep": ["rude"],
        "doc": "Lint, format-check, and Clean Code-check Python automation and SDK sources",
        "uptodate": [False],
    }


def task_typecheck_automation() -> dict[str, Any]:
    """Run strict mypy type checking across automation/."""
    return _task(
        "Type-check automation package with mypy",
        ["mypy", "--config-file", "mypy.ini", "automation"],
    )


def task_test_automation() -> dict[str, Any]:
    """Run unit tests for the automation/ package with pytest."""
    return {
        "actions": [[PYTHON, "-m", "pytest", "-q", "automation/tests"]],
        "task_dep": ["typecheck_automation"],
        "doc": "Type-check and unit-test automation/",
        "uptodate": [False],
    }


def task_advisory_check() -> dict[str, Any]:
    """Verify Rust advisory ignores in deny.toml and .cargo/audit.toml remain active."""
    return _task(
        "Verify Rust advisory ignore sync and expiry",
        [PYTHON, "-m", "automation.advisory_exception"],
    )


def task_readme() -> dict[str, Any]:
    """Regenerate README.md from automation/readme/README.ejs."""
    return _task("Regenerate README.md", [PYTHON, "-m", "automation.readme"])


def task_readme_check() -> dict[str, Any]:
    """Verify README.md matches a fresh render of automation/readme/README.ejs."""
    return _task("Verify README.md freshness", [PYTHON, "-m", "automation.readme", "--check"])


def task_audit_local() -> dict[str, Any]:
    """Run offline Rust dependency bans, licenses, sources, and advisory checks."""
    return _task(
        "Run offline cargo-deny and advisory checks", [PYTHON, "-m", "automation.audit_local"]
    )


def task_fast() -> dict[str, Any]:
    """Run all fast repository, Clean Code, workflow, TOML, sandbox, and automation gates."""
    return {
        "actions": None,
        "task_dep": [
            "repo_check",
            "readme_check",
            "clean_code",
            "lint_workflows",
            "lint_toml",
            "sandbox_check",
            "lint_automation",
            "test_automation",
            "advisory_check",
        ],
        "doc": "Run fast CI and local governance gates",
    }


def task_pre_push() -> dict[str, Any]:
    """Run fast governance gates plus offline Rust dependency policy checks."""
    return {
        "actions": None,
        "task_dep": ["fast", "audit_local"],
        "doc": "Run pre-push governance and offline dependency gates",
    }
