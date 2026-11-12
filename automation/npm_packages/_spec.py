"""Shared npm platform metadata and deterministic tarball packing (Spec 0012 §4.3)."""

from __future__ import annotations

import io
import json
import platform
import tarfile
from dataclasses import dataclass
from pathlib import Path

CARGO_NPM_VERSION: str = "0.1.5"
REPRODUCIBLE_MTIME: int = 499_162_500
FORBIDDEN_LIFECYCLE_SCRIPTS: frozenset[str] = frozenset(
    {"preinstall", "install", "postinstall", "prepublish", "prepare"}
)


@dataclass(frozen=True)
class NpmPlatformSpec:
    """Mapping from Rust target triple to @archont561/playtest-<os>-<cpu> npm metadata."""

    triple: str
    suffix: str
    os_name: str
    cpu_name: str

    @property
    def package_name(self) -> str:
        """Scoped npm platform package name."""
        return f"@archont561/playtest-{self.suffix}"

    @property
    def binary_filename(self) -> str:
        """Executable filename inside the platform package."""
        return "playtest.exe" if self.os_name == "win32" else "playtest"


NPM_PLATFORMS: tuple[NpmPlatformSpec, ...] = (
    NpmPlatformSpec("x86_64-unknown-linux-musl", "linux-x64", "linux", "x64"),
    NpmPlatformSpec("aarch64-unknown-linux-musl", "linux-arm64", "linux", "arm64"),
    NpmPlatformSpec("x86_64-apple-darwin", "darwin-x64", "darwin", "x64"),
    NpmPlatformSpec("aarch64-apple-darwin", "darwin-arm64", "darwin", "arm64"),
    NpmPlatformSpec("x86_64-pc-windows-msvc", "win32-x64", "win32", "x64"),
)


def host_npm_platform() -> NpmPlatformSpec:
    """Resolve the current machine's `NpmPlatformSpec`."""
    sys_map = {"linux": "linux", "darwin": "darwin", "windows": "win32"}
    arch_map = {"x86_64": "x64", "amd64": "x64", "aarch64": "arm64", "arm64": "arm64"}
    os_name = sys_map.get(platform.system().lower(), "")
    cpu_name = arch_map.get(platform.machine().lower(), "")
    for spec in NPM_PLATFORMS:
        if spec.os_name == os_name and spec.cpu_name == cpu_name:
            return spec
    raise RuntimeError(
        f"unsupported host for npm verification: {platform.system()}/{platform.machine()}"
    )


def assert_no_lifecycle_scripts(pkg_json_path: Path) -> None:
    """Fail if a generated `package.json` declares any npm lifecycle script."""
    data = json.loads(pkg_json_path.read_text(encoding="utf-8"))
    scripts = data.get("scripts")
    if isinstance(scripts, dict):
        bad = sorted(FORBIDDEN_LIFECYCLE_SCRIPTS.intersection(scripts.keys()))
        if bad:
            raise RuntimeError(f"forbidden lifecycle script(s) {bad} in {pkg_json_path}")


def _tarball_member_info(rel: str, size: int, executable_rels: frozenset[str]) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name=f"package/{rel}")
    info.size = size
    info.mtime = REPRODUCIBLE_MTIME
    info.mode = 0o755 if rel in executable_rels else 0o644
    return info


def pack_npm_dir(pkg_dir: Path, dest_tgz: Path, executable_rels: frozenset[str]) -> Path:
    """Pack `pkg_dir` into a deterministic npm `.tgz` (`package/...`) preserving 0o755 bits."""
    assert_no_lifecycle_scripts(pkg_dir / "package.json")
    dest_tgz.parent.mkdir(parents=True, exist_ok=True)
    files = sorted(p for p in pkg_dir.rglob("*") if p.is_file())
    with tarfile.open(dest_tgz, "w:gz") as tf:
        for file_path in files:
            rel = file_path.relative_to(pkg_dir).as_posix()
            raw = file_path.read_bytes()
            tf.addfile(_tarball_member_info(rel, len(raw), executable_rels), io.BytesIO(raw))
    return dest_tgz
