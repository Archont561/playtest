"""Hermetic SDK build, test, coverage, and packaging helpers invoked by Pixi tasks."""

from __future__ import annotations

import argparse
import os
import re
import shutil
from collections.abc import Callable, Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import CommandResult, emit_err, emit_line, run_cmd
from automation._sdk_native import (
    c_build_and_test,
    c_coverage,
    c_lint,
    c_package,
    cpp_build_and_test,
    cpp_coverage,
    cpp_lint,
    cpp_package,
    manifest_regex_version,
    workspace_version,
    write_tarball,
)
from automation.stage_assets import (
    clean_staged_assets,
    stage_assets,
    verify_packaged_playtest_assets,
)
from automation.stamp_version import POM_VER_RE

__all__ = ["ACTIONS", "main"]

# `maven.compiler.release` in the Java POM: the language level of the published jar.
JAVA_RELEASE_RE = re.compile(r"<maven\.compiler\.release>\s*(\d+)\s*</maven\.compiler\.release>")


def _csharp_coverage(root: Path) -> int:
    cs_dir = root / "csharp"
    cov_dir = cs_dir / "coverage"
    if cov_dir.exists():
        shutil.rmtree(cov_dir)
    res = run_cmd(
        [
            "dotnet",
            "test",
            "--collect:XPlat Code Coverage;Format=lcov",
            "--results-directory",
            "./coverage",
        ],
        cwd=cs_dir,
    )
    if res.returncode != 0:
        return res.returncode
    found = sorted(cov_dir.rglob("*.info"))
    if not found:
        emit_err("[sdk-tasks] ERROR: dotnet coverage produced no .info tracefile")
        return 1
    shutil.copy2(found[0], cov_dir / "lcov.info")
    return 0


def _csharp_package(root: Path) -> int:
    ver = workspace_version(root)
    cs_dir = root / "csharp"
    (cs_dir / "dist").mkdir(parents=True, exist_ok=True)
    cmd = [
        "dotnet",
        "pack",
        "src/Playtest/Playtest.csproj",
        "-c",
        "Release",
        "-o",
        "dist/",
        f"-p:Version={ver}",
    ]
    return run_cmd(cmd, cwd=cs_dir).returncode


def _go_fmt_check(root: Path) -> int:
    res = run_cmd(["gofmt", "-l", "."], cwd=root / "go", capture=True)
    if res.returncode != 0 or res.stdout.strip():
        emit_err(f"[sdk-tasks] ERROR: unformatted Go files:\n{res.stdout.strip()}")
        return 1
    return 0


def _go_entries(go_dir: Path) -> list[str]:
    entries = [
        "VERSION",
        "go.mod",
        *[p.name for p in sorted(go_dir.glob("*.go")) if not p.name.endswith("_test.go")],
    ]
    cmd_dir = go_dir / "cmd" / "playtest"
    if cmd_dir.is_dir():
        entries.extend(
            f"cmd/playtest/{p.name}"
            for p in sorted(cmd_dir.glob("*.go"))
            if not p.name.endswith("_test.go")
        )
    return entries


def _go_package(root: Path) -> int:
    ver = workspace_version(root)
    go_dir = root / "go"
    archive = go_dir / "dist" / f"playtest-go-{ver}.tar.gz"
    write_tarball(archive, go_dir, _go_entries(go_dir))
    emit_line(f"[sdk-tasks] packaged {archive.relative_to(root)}")
    return 0


def _py_build(root: Path) -> int:
    py_dir = root / "python"
    dist_dir = py_dir / "dist"
    if dist_dir.exists():
        shutil.rmtree(dist_dir)
    dist_dir.mkdir(parents=True, exist_ok=True)
    try:
        stage_assets(root, build_generated=True)
        wheel_cmd = ["maturin", "build", "--locked", "--out", "dist"]
        if os.environ.get("PIXI_OFFLINE", "").lower() in {"1", "true"}:
            wheel_cmd.append("--offline")
        if run_cmd(wheel_cmd, cwd=py_dir).returncode != 0:
            return 1
        if run_cmd(["maturin", "sdist", "--out", "dist"], cwd=py_dir).returncode != 0:
            return 1
    finally:
        clean_staged_assets(root)
    return 0


def _write_java_filtered_resources(java_dir: Path) -> None:
    ver = manifest_regex_version(java_dir / "pom.xml", POM_VER_RE)
    template = java_dir / "src/main/resources/io/github/archont561/playtest/version.properties"
    rendered = template.read_text(encoding="utf-8").replace("${project.version}", ver)
    out_prop = java_dir / "target/classes/io/github/archont561/playtest/version.properties"
    out_prop.parent.mkdir(parents=True, exist_ok=True)
    out_prop.write_text(rendered, encoding="utf-8")


def _java_sources(java_dir: Path, sub: str) -> list[str]:
    return [str(p.relative_to(java_dir)) for p in sorted((java_dir / sub).rglob("*.java"))]


def _java_release(java_dir: Path, *, default: str = "11") -> str:
    """The `--release` the published artifact is built for, read from the POM.

    `javac` without `--release` compiles for the running JDK, so a CI JDK newer than the
    POM's `maven.compiler.release` accepts syntax the released jar cannot load. Reading the
    value here makes the plain-`javac` path agree with the Maven build instead of
    disagreeing only at release time.
    """
    pom = java_dir / "pom.xml"
    if not pom.is_file():
        return default
    match = JAVA_RELEASE_RE.search(pom.read_text(encoding="utf-8"))
    return match.group(1) if match else default


def _java_compile(root: Path) -> int:
    java_dir = root / "java"
    (java_dir / "target/classes").mkdir(parents=True, exist_ok=True)
    (java_dir / "target/test-classes").mkdir(parents=True, exist_ok=True)
    _write_java_filtered_resources(java_dir)
    release = ["--release", _java_release(java_dir)]
    main_srcs = _java_sources(java_dir, "src/main/java")
    main_cmd = ["javac", "-Xlint:all", *release, "-d", "target/classes", *main_srcs]
    if run_cmd(main_cmd, cwd=java_dir).returncode != 0:
        return 1
    test_srcs = _java_sources(java_dir, "src/test/java")
    test_cmd = ["javac", "-cp", "target/classes", *release, "-d", "target/test-classes", *test_srcs]
    return run_cmd(test_cmd, cwd=java_dir).returncode


def _java_run(root: Path, main_class: str, *extra: str) -> int:
    cmd = [
        "java",
        "-cp",
        "target/classes:target/test-classes",
        f"io.github.archont561.playtest.{main_class}",
        *extra,
    ]
    return run_cmd(cmd, cwd=root / "java").returncode


def _java_test(root: Path) -> int:
    if _java_compile(root) != 0:
        return 1
    for entry in ("PlaytestTest", "ConformanceTest"):
        if (status := _java_run(root, entry)) != 0:
            return status
    return 0


def _java_coverage(root: Path) -> int:
    if _java_compile(root) != 0:
        return 1
    return _java_run(root, "Cover2Lcov", "coverage/lcov.info")


def _java_package(root: Path) -> int:
    if _java_compile(root) != 0:
        return 1
    ver = workspace_version(root)
    java_dir = root / "java"
    (java_dir / "dist").mkdir(parents=True, exist_ok=True)
    cmd = ["jar", "cf", f"dist/playtest-{ver}.jar", "-C", "target/classes", "io"]
    return run_cmd(cmd, cwd=java_dir).returncode


def _r_package(root: Path) -> int:
    r_dir = root / "r"
    dist_dir = r_dir / "dist"
    dist_dir.mkdir(parents=True, exist_ok=True)
    if run_cmd(["R", "CMD", "build", "."], cwd=r_dir).returncode != 0:
        return 1
    built = sorted(r_dir.glob("playtest_*.tar.gz"))
    if not built:
        emit_err("[sdk-tasks] ERROR: R CMD build produced no tarball")
        return 1
    for archive in built:
        shutil.move(str(archive), str(dist_dir / archive.name))
    return 0


def _diagnostic(res: CommandResult) -> str:
    """Return cargo's own explanation, never an empty string."""
    return (res.stderr or res.stdout or "").strip() or "cargo produced no diagnostic"


def _report_package_failure(crate: str, res: CommandResult) -> None:
    """Explain why a leaf crate failed to pack, quoting cargo's diagnostic."""
    emit_err(f"[sdk-package] ERROR: `cargo package -p {crate}` failed:\n{_diagnostic(res)}")


# Cargo's own words when a dependent crate cannot be packed because its workspace
# dependency is not in the registry yet. Any other failure is a defect, not the expected
# pre-publication state, and must not be downgraded to a warning.
_UNRESOLVED_DEPENDENCY = "failed to prepare local package for uploading"


def _is_unpublished_dependency(res: CommandResult) -> bool:
    """Whether a `cargo package` failure is the expected pre-publication state.

    A dependent crate cannot be packed until its workspace dependencies exist in the
    registry. Offline, and inside a vendored sandbox, they do not, so cargo fails here
    every time. That is a fact about the release order, not a defect in the crate.
    """
    return _UNRESOLVED_DEPENDENCY in _diagnostic(res)


def _package_dependent_crates(root: Path, dependent_crates: Sequence[str]) -> int:
    """Pack the crates that depend on the leaves.

    These crates cannot be packed before their workspace dependencies are published. Only
    that specific failure falls back to `cargo package --list`, which validates the file
    set without resolving dependencies; every other failure is fatal.

    The distinction matters because the earlier version downgraded *every* failure to a
    note. It then returned the `--list` result, which exits 0 even when the crate cannot
    be packaged at all, so a genuinely broken packaging step passed locally while CI — with
    no vendored registry and no network — failed on the same code.
    """
    for crate in dependent_crates:
        res = run_cmd(
            ["cargo", "package", "-p", crate, "--no-verify", "--allow-dirty", "--locked"],
            cwd=root,
            capture=True,
        )
        if res.returncode == 0:
            continue
        if not _is_unpublished_dependency(res):
            _report_package_failure(crate, res)
            return 1
        if (status := _validate_dependent_file_set(root, crate)) != 0:
            return status
    return 0


def _validate_dependent_file_set(root: Path, crate: str) -> int:
    """Validate a dependent crate's file set when packing it is not yet possible."""
    emit_line(
        f"[sdk-package] NOTE: `{crate}` depends on an unpublished workspace crate; "
        f"validating its file set instead of packing it."
    )
    list_cmd = ["cargo", "package", "-p", crate, "--list", "--offline", "--allow-dirty"]
    list_res = run_cmd(list_cmd, cwd=root, capture=True)
    if list_res.returncode != 0:
        _report_package_failure(crate, list_res)
        return list_res.returncode
    if listing := list_res.stdout.strip():
        emit_line(listing)
    return 0


def _package_leaf_crates(root: Path, leaf_crates: Sequence[str]) -> int:
    try:
        stage_assets(root, build_generated=True)
        for crate in leaf_crates:
            res = run_cmd(
                ["cargo", "package", "-p", crate, "--no-verify", "--allow-dirty", "--locked"],
                cwd=root,
                capture=True,
            )
            if res.returncode != 0:
                _report_package_failure(crate, res)
                return res.returncode
    finally:
        clean_staged_assets(root)
    return 0


def _rs_package(root: Path) -> int:
    leaf_crates = ("playtest-macros", "playtest-core", "playtest-assets")
    status = _package_leaf_crates(root, leaf_crates)
    if status != 0:
        return 1
    ver = workspace_version(root)
    assets_crate = root / "target" / "package" / f"playtest-assets-{ver}.crate"
    if verify_packaged_playtest_assets(root, assets_crate) != 0:
        return 1
    return _package_dependent_crates(root, ("playtest", "playtest-cli"))


ACTIONS: dict[str, Callable[[Path], int]] = {
    "cpp-test": cpp_build_and_test,
    "cpp-lint": cpp_lint,
    "cpp-coverage": cpp_coverage,
    "cpp-package": cpp_package,
    "c-test": c_build_and_test,
    "c-lint": c_lint,
    "c-coverage": c_coverage,
    "c-package": c_package,
    "csharp-coverage": _csharp_coverage,
    "csharp-package": _csharp_package,
    "go-fmt-check": _go_fmt_check,
    "go-package": _go_package,
    "java-compile": _java_compile,
    "java-test": _java_test,
    "java-coverage": _java_coverage,
    "java-package": _java_package,
    "py-build": _py_build,
    "r-package": _r_package,
    "rs-package": _rs_package,
}


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for cross-platform SDK task operations."""
    parser = argparse.ArgumentParser(description="Execute an SDK build/test/package task.")
    parser.add_argument("action", choices=tuple(ACTIONS.keys()))
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    args = parser.parse_args(list(argv) if argv is not None else None)
    return ACTIONS[args.action](args.root.resolve())


if __name__ == "__main__":
    raise SystemExit(main())
