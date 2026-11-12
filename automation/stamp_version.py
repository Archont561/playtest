"""Stamp and read unified SemVer across workspace and SDK manifests (manifest-only)."""

from __future__ import annotations

import argparse
import json
import re
import tomllib
from collections.abc import Callable, Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line

SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")
CMAKE_VER_RE = re.compile(r"project\([^\)]*?\bVERSION\s+([0-9A-Za-z.+-]+)")
CSPROJ_VER_RE = re.compile(r"<Version>([^<]+)</Version>")
POM_VER_RE = re.compile(r"<artifactId>playtest</artifactId>\s*<version>([^<]+)</version>")
R_DESC_VER_RE = re.compile(r"^Version:\s*(\S+)\s*$", re.MULTILINE)


def extract_version_from_tag(tag_or_version: str) -> str:
    """Extract and validate `X.Y.Z` (with optional prerelease) from a tag or bare SemVer."""
    cleaned = tag_or_version.strip()
    if "v" in cleaned and not SEMVER_RE.match(cleaned):
        cleaned = cleaned.rsplit("v", 1)[-1]
    if not SEMVER_RE.match(cleaned):
        raise ValueError(f"invalid semantic version: {tag_or_version!r}")
    return cleaned


def _extract_regex(path: Path, pattern: re.Pattern[str]) -> str:
    match = pattern.search(path.read_text(encoding="utf-8"))
    if match is None:
        raise ValueError(f"missing version field in manifest: {path}")
    return match.group(1).strip()


def read_manifest_versions(root: Path = REPO_ROOT) -> dict[str, str]:
    """Read the declared SemVer from every canonical language and workspace manifest."""
    cargo_toml = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    py_toml = tomllib.loads((root / "python/pyproject.toml").read_text(encoding="utf-8"))
    ts_pkg = json.loads((root / "typescript/package.json").read_text(encoding="utf-8"))
    versions: dict[str, str] = {
        "rust": str(cargo_toml["workspace"]["package"]["version"]),
        "python": str(py_toml["project"]["version"]),
        "typescript": str(ts_pkg["version"]),
        "cpp": _extract_regex(root / "cpp/CMakeLists.txt", CMAKE_VER_RE),
        "c": _extract_regex(root / "c/CMakeLists.txt", CMAKE_VER_RE),
        "csharp": _extract_regex(root / "csharp/src/Playtest/Playtest.csproj", CSPROJ_VER_RE),
        "go": (root / "go/VERSION").read_text(encoding="utf-8").strip(),
        "java": _extract_regex(root / "java/pom.xml", POM_VER_RE),
        "r": _extract_regex(root / "r/DESCRIPTION", R_DESC_VER_RE),
    }
    root_pkg = root / "package.json"
    if root_pkg.is_file():
        versions["root_package_json"] = str(
            json.loads(root_pkg.read_text(encoding="utf-8"))["version"]
        )
    return versions


def _replace_once(path: Path, pattern: str, replacement: str) -> None:
    original = path.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, original, count=1, flags=re.MULTILINE)
    if count != 1:
        raise RuntimeError(f"failed to stamp {path}: pattern {pattern!r} matched {count} times")
    path.write_text(updated, encoding="utf-8")


def _stamp_rust(root: Path, version: str) -> None:
    _replace_once(
        root / "Cargo.toml",
        r'(\[workspace\.package\][^\[]*?^version\s*=\s*)"[^"]+"',
        rf'\g<1>"{version}"',
    )


def _stamp_python(root: Path, version: str) -> None:
    _replace_once(root / "python/pyproject.toml", r'^(version\s*=\s*)"[^"]+"', rf'\g<1>"{version}"')


def _stamp_typescript(root: Path, version: str) -> None:
    pkg_path = root / "typescript/package.json"
    data = json.loads(pkg_path.read_text(encoding="utf-8"))
    data["version"] = version
    pkg_path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def _stamp_cpp(root: Path, version: str) -> None:
    _replace_once(
        root / "cpp/CMakeLists.txt",
        r"(project\(playtest-cpp\s+VERSION\s+)[0-9A-Za-z.+-]+",
        rf"\g<1>{version}",
    )


def _stamp_c(root: Path, version: str) -> None:
    _replace_once(
        root / "c/CMakeLists.txt",
        r"(project\(playtest-c\s+VERSION\s+)[0-9A-Za-z.+-]+",
        rf"\g<1>{version}",
    )


def _stamp_csharp(root: Path, version: str) -> None:
    _replace_once(
        root / "csharp/src/Playtest/Playtest.csproj",
        r"(<Version>)[^<]+(</Version>)",
        rf"\g<1>{version}\g<2>",
    )


def _stamp_go(root: Path, version: str) -> None:
    (root / "go/VERSION").write_text(f"{version}\n", encoding="utf-8")


def _stamp_java(root: Path, version: str) -> None:
    _replace_once(
        root / "java/pom.xml",
        r"(<artifactId>playtest</artifactId>\s*<version>)[^<]+(</version>)",
        rf"\g<1>{version}\g<2>",
    )


def _stamp_r(root: Path, version: str, author_email: str = "") -> None:
    base_ver = version.split("-", 1)[0]
    _replace_once(root / "r/DESCRIPTION", r"^(Version:\s*).+$", rf"\g<1>{base_ver}")
    if author_email.strip():
        _replace_once(
            root / "r/DESCRIPTION",
            r'(email\s*=\s*)"[^"]+"',
            rf'\g<1>"{author_email.strip()}"',
        )


STAMPERS: dict[str, Callable[[Path, str], None]] = {
    "rust": _stamp_rust,
    "python": _stamp_python,
    "typescript": _stamp_typescript,
    "cpp": _stamp_cpp,
    "c": _stamp_c,
    "csharp": _stamp_csharp,
    "go": _stamp_go,
    "java": _stamp_java,
}


def stamp_target(
    target: str,
    version_or_tag: str,
    *,
    root: Path = REPO_ROOT,
    author_email: str = "",
) -> str:
    """Stamp `version_or_tag` into canonical manifest(s) (`'all'` or one target)."""
    version = extract_version_from_tag(version_or_tag)
    if target == "all":
        root_pkg = root / "package.json"
        if root_pkg.is_file():
            pkg_data = json.loads(root_pkg.read_text(encoding="utf-8"))
            pkg_data["version"] = version
            root_pkg.write_text(json.dumps(pkg_data, indent=2) + "\n", encoding="utf-8")
        for fn in STAMPERS.values():
            fn(root, version)
        _stamp_r(root, version, author_email=author_email)
        return version
    if target == "r":
        _stamp_r(root, version, author_email=author_email)
        return version
    if target not in STAMPERS:
        raise ValueError(f"unknown stamp target: {target!r}")
    STAMPERS[target](root, version)
    return version


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for manifest version stamping."""
    parser = argparse.ArgumentParser(description="Stamp unified Playtest release version.")
    parser.add_argument("target", choices=("all", *STAMPERS.keys(), "r"))
    parser.add_argument("version", help="SemVer or git tag (e.g. 0.2.0 or v0.2.0).")
    parser.add_argument("--author-email", default="", help="Optional CRAN maintainer email.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    args = parser.parse_args(list(argv) if argv is not None else None)
    try:
        stamped = stamp_target(
            args.target,
            args.version,
            root=args.root.resolve(),
            author_email=args.author_email,
        )
    except (ValueError, RuntimeError) as err:
        emit_err(f"[stamp-version] ERROR: {err}")
        return 1
    emit_line(f"[stamp-version] OK: stamped {args.target} -> {stamped}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
