"""README generation and freshness checks driven by readme-md-generator."""

from __future__ import annotations

from automation.readme.generate import (
    ATTRIBUTION,
    BANNER_MARKER,
    GENERATOR_PACKAGE,
    PACKAGE_JSON,
    TARGET,
    TEMPLATE,
    ReadmeResult,
    generator_command,
    inject_banner,
    main,
    pinned_version,
    render_readme,
    subprocess_runner,
    write_readme,
)

__all__ = [
    "ATTRIBUTION",
    "BANNER_MARKER",
    "GENERATOR_PACKAGE",
    "PACKAGE_JSON",
    "TARGET",
    "TEMPLATE",
    "ReadmeResult",
    "generator_command",
    "inject_banner",
    "main",
    "pinned_version",
    "render_readme",
    "subprocess_runner",
    "write_readme",
]
