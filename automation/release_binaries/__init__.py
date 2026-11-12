"""Package, checksum, and verify GitHub Release CLI binary archives (Spec 0012 §§3, 8)."""

from __future__ import annotations

from automation.release_binaries._archives import package_platform_archive, sha256_hex
from automation.release_binaries._cli import main
from automation.release_binaries._platforms import (
    PLATFORM_BY_TRIPLE,
    RELEASE_PLATFORMS,
    REPRODUCIBLE_MTIME,
    ReleasePlatform,
)
from automation.release_binaries._verify import (
    combine_checksums,
    verify_platform_archive,
    verify_version_alignment,
)

__all__ = [
    "PLATFORM_BY_TRIPLE",
    "RELEASE_PLATFORMS",
    "REPRODUCIBLE_MTIME",
    "ReleasePlatform",
    "combine_checksums",
    "main",
    "package_platform_archive",
    "sha256_hex",
    "verify_platform_archive",
    "verify_version_alignment",
]
