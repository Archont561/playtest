#!/usr/bin/env python3
"""Stage published SDK files into `crates/playtest-assets/staged/` (`Spec 0012 §2.5`)."""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from automation.stage_assets import main  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(main())
