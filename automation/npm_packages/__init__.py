"""Generate, pack, verify, and publish @archont561/playtest npm packages (Spec 0012 §4.3)."""

from __future__ import annotations

from automation.npm_packages._cli import main
from automation.npm_packages._spec import (
    CARGO_NPM_VERSION,
    FORBIDDEN_LIFECYCLE_SCRIPTS,
    NPM_PLATFORMS,
    NpmPlatformSpec,
    assert_no_lifecycle_scripts,
    host_npm_platform,
    pack_npm_dir,
)
from automation.npm_packages._tarballs import build_npm_tarballs
from automation.npm_packages._verify import verify_clean_installs

__all__ = [
    "CARGO_NPM_VERSION",
    "FORBIDDEN_LIFECYCLE_SCRIPTS",
    "NPM_PLATFORMS",
    "NpmPlatformSpec",
    "assert_no_lifecycle_scripts",
    "build_npm_tarballs",
    "host_npm_platform",
    "main",
    "pack_npm_dir",
    "verify_clean_installs",
]
