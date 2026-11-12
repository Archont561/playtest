"""Repository automation and governance package for Playtest."""

from __future__ import annotations

from pathlib import Path

REPO_ROOT: Path = Path(__file__).resolve().parent.parent

__all__ = ["REPO_ROOT"]
