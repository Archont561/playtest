"""C and C++ SDK build, lint, coverage, and tarball helpers for `automation.sdk_tasks`."""

from __future__ import annotations

import os
import re
import tarfile
import tomllib
from collections.abc import Sequence
from pathlib import Path

from automation._proc import emit_line, run_cmd
from automation.stamp_version import CMAKE_VER_RE


def workspace_version(root: Path) -> str:
    """Resolve active workspace version from `RELEASE_TAG`, `PLAYTEST_VERSION`, or `Cargo.toml`."""
    env_tag = os.environ.get("RELEASE_TAG", "").strip()
    if env_tag:
        return env_tag.rsplit("v", 1)[-1]
    env_ver = os.environ.get("PLAYTEST_VERSION", "").strip()
    if env_ver:
        return env_ver
    cargo_data = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    return str(cargo_data["workspace"]["package"]["version"])


def write_tarball(archive: Path, base_dir: Path, rel_entries: Sequence[str]) -> None:
    """Pack `rel_entries` under `base_dir` into gzip archive `archive`."""
    archive.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive, "w:gz") as tar:
        for rel in rel_entries:
            tar.add(base_dir / rel, arcname=rel)


def manifest_regex_version(manifest: Path, pattern: re.Pattern[str]) -> str:
    """Extract the single captured version group from `manifest` matching `pattern`."""
    match = pattern.search(manifest.read_text(encoding="utf-8"))
    if match is None:
        raise RuntimeError(f"missing version declaration in {manifest}")
    return match.group(1).strip()


def _cxx_bin() -> str:
    return os.environ.get("CXX", "").strip() or "g++"


def _cc_bin() -> str:
    return os.environ.get("CC", "").strip() or "gcc"


def _gcov_bin() -> str:
    return os.environ.get("GCOV", "").strip() or "gcov"


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


def cpp_build_and_test(root: Path) -> int:
    """Compile and execute the C++20 SDK unit test binary."""
    cpp_dir = root / "cpp"
    ver = manifest_regex_version(cpp_dir / "CMakeLists.txt", CMAKE_VER_RE)
    (cpp_dir / "target").mkdir(parents=True, exist_ok=True)
    cmd = [
        _cxx_bin(),
        "-std=c++20",
        f"-DPLAYTEST_CPP_VER={ver}",
        "tests/test_version.cpp",
        "-I",
        "include",
        "-o",
        "target/test_cpp",
    ]
    build = run_cmd(cmd, cwd=cpp_dir)
    if build.returncode != 0:
        return build.returncode
    return run_cmd([str(cpp_dir / "target/test_cpp")], cwd=cpp_dir).returncode


def cpp_lint(root: Path) -> int:
    """Run `clang-format --Werror` and strict C++20 syntax warnings on `cpp/`."""
    cpp_dir = root / "cpp"
    ver = manifest_regex_version(cpp_dir / "CMakeLists.txt", CMAKE_VER_RE)
    fmt_files = ["include/playtest/playtest.hpp", "tests/test_version.cpp"]
    fmt = run_cmd(["clang-format", "--dry-run", "--Werror", *fmt_files], cwd=cpp_dir)
    if fmt.returncode != 0:
        return fmt.returncode
    warn_flags = ["-std=c++20", "-Wall", "-Wextra", "-Wpedantic", "-fsyntax-only"]
    cmd = [
        _cxx_bin(),
        *warn_flags,
        f"-DPLAYTEST_CPP_VER={ver}",
        "-I",
        "include",
        "tests/test_version.cpp",
    ]
    return run_cmd(cmd, cwd=cpp_dir).returncode


def cpp_coverage(root: Path) -> int:
    """Generate `cpp/coverage/lcov-cpp.info` via gcov + lcov."""
    cpp_dir = root / "cpp"
    ver = manifest_regex_version(cpp_dir / "CMakeLists.txt", CMAKE_VER_RE)
    (cpp_dir / "target").mkdir(parents=True, exist_ok=True)
    (cpp_dir / "coverage").mkdir(parents=True, exist_ok=True)
    out_bin = cpp_dir / "target/test_cpp_cov"
    compile_cmd = [
        _cxx_bin(),
        "-std=c++20",
        "--coverage",
        f"-DPLAYTEST_CPP_VER={ver}",
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


def cpp_package(root: Path) -> int:
    """Package `cpp/` source archive into `cpp/dist/`."""
    ver = workspace_version(root)
    archive = root / "cpp" / "dist" / f"playtest-cpp-{ver}.tar.gz"
    write_tarball(archive, root / "cpp", ("include", "CMakeLists.txt", "conanfile.py"))
    emit_line(f"[sdk-tasks] packaged {archive.relative_to(root)}")
    return 0


def c_build_and_test(root: Path) -> int:
    """Compile and execute the C11 SDK unit test binary."""
    c_dir = root / "c"
    ver = manifest_regex_version(c_dir / "CMakeLists.txt", CMAKE_VER_RE)
    (c_dir / "target").mkdir(parents=True, exist_ok=True)
    cmd = [
        _cc_bin(),
        "-std=c11",
        f"-DPLAYTEST_C_VER={ver}",
        "src/playtest.c",
        "tests/test_version.c",
        "-I",
        "include",
        "-o",
        "target/test_c",
    ]
    build = run_cmd(cmd, cwd=c_dir)
    if build.returncode != 0:
        return build.returncode
    return run_cmd([str(c_dir / "target/test_c")], cwd=c_dir).returncode


def c_lint(root: Path) -> int:
    """Run `clang-format --Werror` and strict C11 syntax warnings on `c/`."""
    c_dir = root / "c"
    ver = manifest_regex_version(c_dir / "CMakeLists.txt", CMAKE_VER_RE)
    fmt_files = ["include/playtest/playtest.h", "src/playtest.c", "tests/test_version.c"]
    fmt = run_cmd(["clang-format", "--dry-run", "--Werror", *fmt_files], cwd=c_dir)
    if fmt.returncode != 0:
        return fmt.returncode
    warn_flags = ["-std=c11", "-Wall", "-Wextra", "-Wpedantic", "-fsyntax-only"]
    srcs = ["src/playtest.c", "tests/test_version.c"]
    cmd = [_cc_bin(), *warn_flags, f"-DPLAYTEST_C_VER={ver}", "-I", "include", *srcs]
    return run_cmd(cmd, cwd=c_dir).returncode


def c_coverage(root: Path) -> int:
    """Generate `c/coverage/lcov.info` via gcov + lcov."""
    c_dir = root / "c"
    ver = manifest_regex_version(c_dir / "CMakeLists.txt", CMAKE_VER_RE)
    (c_dir / "target").mkdir(parents=True, exist_ok=True)
    (c_dir / "coverage").mkdir(parents=True, exist_ok=True)
    out_bin = c_dir / "target/test_c_cov"
    compile_cmd = [
        _cc_bin(),
        "-std=c11",
        "--coverage",
        f"-DPLAYTEST_C_VER={ver}",
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


def c_package(root: Path) -> int:
    """Package `c/` source archive into `c/dist/`."""
    ver = workspace_version(root)
    archive = root / "c" / "dist" / f"playtest-c-{ver}.tar.gz"
    write_tarball(archive, root / "c", ("include", "src", "CMakeLists.txt", "conanfile.py"))
    emit_line(f"[sdk-tasks] packaged {archive.relative_to(root)}")
    return 0
