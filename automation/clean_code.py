"""Unified runner for vendored polyglot-clean-code presets and rude-clean-code."""

from __future__ import annotations

import argparse
from collections.abc import Callable, Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd
from automation.rude_clean import run_rude_clean

AST_GREP_TARGETS: tuple[str, ...] = ("crates", "automation", "python", "typescript", "go", "java")
VALE_TARGETS: tuple[str, ...] = (
    "README.md",
    "CONTRIBUTING.md",
    "AGENTS.md",
    "docs",
    "python/README.md",
    "typescript/README.md",
    "cpp/README.md",
    "c/README.md",
    "csharp/README.md",
    "java/README.md",
    "go/README.md",
)
SEMGREP_EXCLUDES: tuple[str, ...] = (
    "vendor/clean-code-presets",
    ".agents",
    ".pixi",
    ".pixi-sandbox",
    "target",
    "node_modules",
    "backlog",
)


def run_alint(root: Path = REPO_ROOT) -> int:
    """Run both vendored strict Alint preset and repository-local .alint.yml rules."""
    preset = root / "vendor" / "clean-code-presets" / "alint" / "strict.yml"
    local_cfg = root / ".alint.yml"
    for cfg in (preset, local_cfg):
        res = run_cmd(
            ["alint", "check", ".", "-c", str(cfg), "--fail-on-warning"],
            cwd=root,
            timeout=120,
        )
        if res.returncode != 0:
            emit_err(f"[clean-code] ERROR: alint failed for {cfg.relative_to(root)}")
            return res.returncode
    emit_line("[clean-code] OK: Alint strict preset and local rules passed.")
    return 0


def run_semgrep(root: Path = REPO_ROOT) -> int:
    """Run vendored Semgrep pedantic rules with telemetry and update checks disabled."""
    preset = root / "vendor" / "clean-code-presets" / "semgrep" / "pedantic.yml"
    cmd: list[str] = [
        "semgrep",
        "scan",
        "--config",
        str(preset),
        "--error",
        "--quiet",
        "--metrics=off",
    ]
    for pattern in SEMGREP_EXCLUDES:
        cmd.extend(["--exclude", pattern])
    cmd.append(".")
    res = run_cmd(
        cmd,
        cwd=root,
        env={
            "SEMGREP_SEND_METRICS": "off",
            "SEMGREP_ENABLE_VERSION_CHECK": "0",
        },
        timeout=300,
    )
    if res.returncode != 0:
        emit_err("[clean-code] ERROR: Semgrep pedantic scan reported violations.")
        return res.returncode
    emit_line("[clean-code] OK: Semgrep pedantic scan passed.")
    return 0


def run_ast_grep(root: Path = REPO_ROOT) -> int:
    """Run vendored ast-grep pedantic structural rules against source directories."""
    preset = root / "vendor" / "clean-code-presets" / "ast-grep" / "pedantic" / "sgconfig.yml"
    targets = [rel for rel in AST_GREP_TARGETS if (root / rel).exists()]
    res = run_cmd(["ast-grep", "scan", "-c", str(preset), *targets], cwd=root, timeout=180)
    if res.returncode != 0:
        emit_err("[clean-code] ERROR: ast-grep pedantic scan reported violations.")
        return res.returncode
    emit_line("[clean-code] OK: ast-grep pedantic scan passed.")
    return 0


def run_vale(root: Path = REPO_ROOT) -> int:
    """Run vendored Vale strict prose linter against repository governance docs."""
    preset = root / "vendor" / "clean-code-presets" / "vale" / "strict.ini"
    targets = [rel for rel in VALE_TARGETS if (root / rel).exists()]
    res = run_cmd(["vale", "--no-global", f"--config={preset}", *targets], cwd=root, timeout=120)
    if res.returncode != 0:
        emit_err("[clean-code] ERROR: Vale strict prose check reported violations.")
        return res.returncode
    emit_line("[clean-code] OK: Vale strict prose check passed.")
    return 0


ENGINE_MAP: dict[str, Callable[[Path], int]] = {
    "alint": run_alint,
    "semgrep": run_semgrep,
    "ast-grep": run_ast_grep,
    "vale": run_vale,
    "rude": run_rude_clean,
}


def run_selected_engines(engine: str, root: Path = REPO_ROOT) -> int:
    """Execute either a single Clean Code engine or the complete five-engine suite."""
    if engine != "all":
        return ENGINE_MAP[engine](root)
    for name, runner in ENGINE_MAP.items():
        rc = runner(root)
        if rc != 0:
            emit_err(f"[clean-code] FAILED at engine: {name}")
            return rc
    emit_line("[clean-code] OK: all 5 Clean Code engines passed.")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for Clean Code preset enforcement."""
    parser = argparse.ArgumentParser(description="Run Clean Code governance engines.")
    parser.add_argument(
        "--engine",
        choices=("all", *ENGINE_MAP.keys()),
        default="all",
        help="Clean Code engine to execute.",
    )
    parser.add_argument("--root", type=Path, default=REPO_ROOT, help="Repository root path.")
    args = parser.parse_args(list(argv) if argv is not None else None)
    return run_selected_engines(args.engine, args.root.resolve())


if __name__ == "__main__":
    raise SystemExit(main())
