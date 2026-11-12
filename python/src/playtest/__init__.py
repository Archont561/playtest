"""playtest: A Playwright-inspired async testing framework for Python."""

from __future__ import annotations

import re
from importlib.metadata import PackageNotFoundError
from importlib.metadata import version as _dist_version
from pathlib import Path

_VERSION_LINE_RE = re.compile(r'^version\s*=\s*"([^"]+)"', re.MULTILINE)


def _discover_version(
    dist_name: str = "playtest-py",
    pyproject: Path | None = None,
) -> str:
    """Resolve package version from installed metadata or local pyproject.toml."""
    try:
        return _dist_version(dist_name)
    except PackageNotFoundError as exc:
        manifest = pyproject or (Path(__file__).resolve().parents[2] / "pyproject.toml")
        if manifest.is_file():
            match = _VERSION_LINE_RE.search(manifest.read_text(encoding="utf-8"))
            if match is not None:
                return match.group(1)
        raise RuntimeError(f"Unable to resolve version for {dist_name}") from exc


VERSION: str = _discover_version()
__version__: str = VERSION

__all__ = ["VERSION", "__version__"]
