"""Hermetic SDK build, test, coverage, and packaging helpers invoked by Pixi tasks."""

from __future__ import annotations

import argparse
import os
import shutil
import tarfile
import tomllib
from collections.abc import Callable, Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd
from automation.stage_assets import (
    clean_staged_assets,
    stage_assets,
    verify_packaged_playtest_assets,
)


def _workspace_version(root: Path) -> str:
    env_tag = os.environ.get("RELEASE_TAG", "").strip()
    if env_tag:
        return env_tag.rsplit("v", 1)[-1]
    env_ver = os.environ.get("PLAYTEST_VERSION", "").strip()
    if env_ver:
        return env_ver
    cargo_data = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    return str(cargo_data["workspace"]["package"]["version"])


def _cxx_bin() -> str:
    return os.environ.get("CXX", "").strip() or "g++"


def _cc_bin() -> str:
    return os.environ.get("CC", "").strip() or "gcc"


def _gcov_bin() -> str:
    return os.environ.get("GCOV", "").strip() or "gcov"


def _write_tarball(archive: Path, base_dir: Path, rel_entries: Sequence[str]) -> None:
    archive.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive, "w:gz") as tar:
        for rel in rel_entries:
            tar.add(base_dir / rel, arcname=rel)


def _cpp_build_and_test(root: Path) -> int:
    cpp_dir = root / "cpp"
    (cpp_dir / "target").mkdir(parents=True, exist_ok=True)
    build = run_cmd(
        [
            _cxx_bin(),
            "-std=c++20",
            "tests/test_version.cpp",
            "-I",
            "include",
            "-o",
            "target/test_cpp",
        ],
        cwd=cpp_dir,
    )
    if build.returncode != 0:
        return build.returncode
    return run_cmd([str(cpp_dir / "target/test_cpp")], cwd=cpp_dir).returncode


def _cpp_lint(root: Path) -> int:
    cpp_dir = root / "cpp"
    fmt = run_cmd(
        [
            "clang-format",
            "--dry-run",
            "--Werror",
            "include/playtest/playtest.hpp",
            "tests/test_version.cpp",
        ],
        cwd=cpp_dir,
    )
    if fmt.returncode != 0:
        return fmt.returncode
    return run_cmd(
        [
            _cxx_bin(),
            "-std=c++20",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-fsyntax-only",
            "-I",
            "include",
            "tests/test_version.cpp",
        ],
        cwd=cpp_dir,
    ).returncode


def _capture_lcov(work_dir: Path, out_file: str) -> int:
    cap_cmd = [
        "lcov",
        "--capture",
        "--directory",
        "target",
        "--gcov-tool",
        _gcov_bin(),
        "--output-file",
        out_file,
        "--ignore-errors",
        "mismatch,gcov,negative",
    ]
    if run_cmd(cap_cmd, cwd=work_dir).returncode != 0:
        return 1
    rm_cmd = [
        "lcov",
        "--remove",
        out_file,
        "/usr/*",
        "*/conda/*",
        "--output-file",
        out_file,
        "--ignore-errors",
        "unused",
    ]
    return run_cmd(rm_cmd, cwd=work_dir).returncode


def _cpp_coverage(root: Path) -> int:
    cpp_dir = root / "cpp"
    (cpp_dir / "target").mkdir(parents=True, exist_ok=True)
    (cpp_dir / "coverage").mkdir(parents=True, exist_ok=True)
    out_bin = cpp_dir / "target/test_cpp_cov"
    compile_cmd = [
        _cxx_bin(),
        "-std=c++20",
        "--coverage",
        "tests/test_version.cpp",
        "-I",
        "include",
        "-o",
        str(out_bin),
    ]
    if run_cmd(compile_cmd, cwd=cpp_dir).returncode != 0:
        return 1
    if run_cmd([str(out_bin)], cwd=cpp_dir).returncode != 0:
        return 1
    return _capture_lcov(cpp_dir, "coverage/lcov-cpp.info")


def _cpp_package(root: Path) -> int:
    ver = _workspace_version(root)
    archive = root / "cpp" / "dist" / f"playtest-cpp-{ver}.tar.gz"
    _write_tarball(archive, root / "cpp", ("include", "CMakeLists.txt", "conanfile.py"))
    emit_line(f"[sdk-tasks] packaged {archive.relative_to(root)}")
    return 0


def _c_build_and_test(root: Path) -> int:
    c_dir = root / "c"
    (c_dir / "target").mkdir(parents=True, exist_ok=True)
    build = run_cmd(
        [
            _cc_bin(),
            "-std=c11",
            "src/playtest.c",
            "tests/test_version.c",
            "-I",
            "include",
            "-o",
            "target/test_c",
        ],
        cwd=c_dir,
    )
    if build.returncode != 0:
        return build.returncode
    return run_cmd([str(c_dir / "target/test_c")], cwd=c_dir).returncode


def _c_lint(root: Path) -> int:
    c_dir = root / "c"
    fmt = run_cmd(
        [
            "clang-format",
            "--dry-run",
            "--Werror",
            "include/playtest/playtest.h",
            "src/playtest.c",
            "tests/test_version.c",
        ],
        cwd=c_dir,
    )
    if fmt.returncode != 0:
        return fmt.returncode
    return run_cmd(
        [
            _cc_bin(),
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-fsyntax-only",
            "-I",
            "include",
            "src/playtest.c",
            "tests/test_version.c",
        ],
        cwd=c_dir,
    ).returncode


def _c_coverage(root: Path) -> int:
    c_dir = root / "c"
    (c_dir / "target").mkdir(parents=True, exist_ok=True)
    (c_dir / "coverage").mkdir(parents=True, exist_ok=True)
    out_bin = c_dir / "target/test_c_cov"
    compile_cmd = [
        _cc_bin(),
        "-std=c11",
        "--coverage",
        "src/playtest.c",
        "tests/test_version.c",
        "-I",
        "include",
        "-o",
        str(out_bin),
    ]
    if run_cmd(compile_cmd, cwd=c_dir).returncode != 0:
        return 1
    if run_cmd([str(out_bin)], cwd=c_dir).returncode != 0:
        return 1
    return _capture_lcov(c_dir, "coverage/lcov.info")


def _c_package(root: Path) -> int:
    ver = _workspace_version(root)
    archive = root / "c" / "dist" / f"playtest-c-{ver}.tar.gz"
    _write_tarball(archive, root / "c", ("include", "src", "CMakeLists.txt", "conanfile.py"))
    emit_line(f"[sdk-tasks] packaged {archive.relative_to(root)}")
    return 0


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
    ver = _workspace_version(root)
    cs_dir = root / "csharp"
    (cs_dir / "dist").mkdir(parents=True, exist_ok=True)
    return run_cmd(
        [
            "dotnet",
            "pack",
            "src/Playtest/Playtest.csproj",
            "-c",
            "Release",
            "-o",
            "dist/",
            f"-p:Version={ver}",
        ],
        cwd=cs_dir,
    ).returncode


def _go_fmt_check(root: Path) -> int:
    res = run_cmd(["gofmt", "-l", "."], cwd=root / "go", capture=True)
    if res.returncode != 0 or res.stdout.strip():
        emit_err(f"[sdk-tasks] ERROR: unformatted Go files:\n{res.stdout.strip()}")
        return 1
    return 0


def _go_package(root: Path) -> int:
    ver = _workspace_version(root)
    go_dir = root / "go"
    entries = [
        "go.mod",
        *[p.name for p in sorted(go_dir.glob("*.go")) if not p.name.endswith("_test.go")],
    ]
    archive = go_dir / "dist" / f"playtest-go-{ver}.tar.gz"
    _write_tarball(archive, go_dir, entries)
    emit_line(f"[sdk-tasks] packaged {archive.relative_to(root)}")
    return 0


def _java_compile(root: Path) -> int:
    java_dir = root / "java"
    main_out = java_dir / "target/classes"
    test_out = java_dir / "target/test-classes"
    main_out.mkdir(parents=True, exist_ok=True)
    test_out.mkdir(parents=True, exist_ok=True)
    main_srcs = [
        str(p.relative_to(java_dir)) for p in sorted((java_dir / "src/main/java").rglob("*.java"))
    ]
    if (
        run_cmd(
            ["javac", "-Xlint:all", "-d", "target/classes", *main_srcs], cwd=java_dir
        ).returncode
        != 0
    ):
        return 1
    test_srcs = [
        str(p.relative_to(java_dir)) for p in sorted((java_dir / "src/test/java").rglob("*.java"))
    ]
    return run_cmd(
        ["javac", "-cp", "target/classes", "-d", "target/test-classes", *test_srcs], cwd=java_dir
    ).returncode


def _java_test(root: Path) -> int:
    if _java_compile(root) != 0:
        return 1
    return run_cmd(
        [
            "java",
            "-cp",
            "target/classes:target/test-classes",
            "io.github.archont561.playtest.PlaytestTest",
        ],
        cwd=root / "java",
    ).returncode


def _java_coverage(root: Path) -> int:
    if _java_compile(root) != 0:
        return 1
    return run_cmd(
        [
            "java",
            "-cp",
            "target/classes:target/test-classes",
            "io.github.archont561.playtest.Cover2Lcov",
            "coverage/lcov.info",
        ],
        cwd=root / "java",
    ).returncode


def _java_package(root: Path) -> int:
    if _java_compile(root) != 0:
        return 1
    ver = _workspace_version(root)
    java_dir = root / "java"
    (java_dir / "dist").mkdir(parents=True, exist_ok=True)
    return run_cmd(
        ["jar", "cf", f"dist/playtest-{ver}.jar", "-C", "target/classes", "io"],
        cwd=java_dir,
    ).returncode


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


def _package_dependent_crates(root: Path, dependent_crates: Sequence[str]) -> int:
    for crate in dependent_crates:
        res = run_cmd(
            ["cargo", "package", "-p", crate, "--no-verify", "--allow-dirty", "--locked"],
            cwd=root,
            capture=True,
        )
        if res.returncode == 0:
            continue
        list_res = run_cmd(
            ["cargo", "package", "-p", crate, "--list", "--allow-dirty", "--locked"],
            cwd=root,
        )
        if list_res.returncode != 0:
            return list_res.returncode
    return 0


def _rs_package(root: Path) -> int:
    leaf_crates = ("playtest-macros", "playtest-core", "playtest-assets")
    dependent_crates = ("playtest", "playtest-cli")
    ver = _workspace_version(root)
    try:
        stage_assets(root, build_generated=True)
        for crate in leaf_crates:
            res = run_cmd(
                ["cargo", "package", "-p", crate, "--no-verify", "--allow-dirty", "--locked"],
                cwd=root,
            )
            if res.returncode != 0:
                return res.returncode
    finally:
        clean_staged_assets(root)
    assets_crate = root / "target" / "package" / f"playtest-assets-{ver}.crate"
    if verify_packaged_playtest_assets(root, assets_crate) != 0:
        return 1
    return _package_dependent_crates(root, dependent_crates)


ACTIONS: dict[str, Callable[[Path], int]] = {
    "cpp-test": _cpp_build_and_test,
    "cpp-lint": _cpp_lint,
    "cpp-coverage": _cpp_coverage,
    "cpp-package": _cpp_package,
    "c-test": _c_build_and_test,
    "c-lint": _c_lint,
    "c-coverage": _c_coverage,
    "c-package": _c_package,
    "csharp-coverage": _csharp_coverage,
    "csharp-package": _csharp_package,
    "go-fmt-check": _go_fmt_check,
    "go-package": _go_package,
    "java-compile": _java_compile,
    "java-test": _java_test,
    "java-coverage": _java_coverage,
    "java-package": _java_package,
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
