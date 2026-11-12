"""Supported GitHub Release binary platform matrix (Spec 0012 §3)."""

from __future__ import annotations

from dataclasses import dataclass

REPRODUCIBLE_MTIME: int = 499_162_500


@dataclass(frozen=True)
class ReleasePlatform:
    """Supported binary distribution target for GitHub Releases (Spec 0012 §3)."""

    triple: str
    runner: str
    archive_ext: str
    conda_platform: str

    @property
    def is_windows(self) -> bool:
        """Whether this target uses Windows `.zip` and `.exe` conventions."""
        return self.archive_ext == ".zip"

    @property
    def binary_name(self) -> str:
        """Executable filename inside the archive."""
        return "playtest.exe" if self.is_windows else "playtest"

    @property
    def archive_stem(self) -> str:
        """Root prefix and filename stem (`playtest-cli-<triple>`)."""
        return f"playtest-cli-{self.triple}"

    @property
    def archive_filename(self) -> str:
        """Release asset filename (`playtest-cli-<triple>.tar.gz` or `.zip`)."""
        return f"{self.archive_stem}{self.archive_ext}"

    @property
    def checksum_filename(self) -> str:
        """Per-archive SHA-256 sidecar filename."""
        return f"{self.archive_filename}.sha256"


RELEASE_PLATFORMS: tuple[ReleasePlatform, ...] = (
    ReleasePlatform("x86_64-unknown-linux-musl", "ubuntu-24.04", ".tar.gz", "linux-64"),
    ReleasePlatform("aarch64-unknown-linux-musl", "ubuntu-24.04-arm", ".tar.gz", "linux-aarch64"),
    ReleasePlatform("x86_64-apple-darwin", "macos-15-intel", ".tar.gz", "osx-64"),
    ReleasePlatform("aarch64-apple-darwin", "macos-15", ".tar.gz", "osx-arm64"),
    ReleasePlatform("x86_64-pc-windows-msvc", "windows-2025", ".zip", "win-64"),
)

PLATFORM_BY_TRIPLE: dict[str, ReleasePlatform] = {p.triple: p for p in RELEASE_PLATFORMS}
