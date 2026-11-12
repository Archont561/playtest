from __future__ import annotations

import re

from playtest import VERSION, __version__

SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")


def test_version_accessible() -> None:
    assert SEMVER_RE.match(VERSION) is not None
    assert __version__ == VERSION
