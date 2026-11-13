"""Polyglot test framework with the batteries included: dependency-injected fixtures,
strict LIFO teardown, property-based testing, and code-first BDD for Python.
"""

from __future__ import annotations

from playtest._version import VERSION, __version__
from playtest.fixtures import FixtureStack, TeardownError, TeardownReport, TestFacade, fixture

__all__ = [
    "VERSION",
    "FixtureStack",
    "TeardownError",
    "TeardownReport",
    "TestFacade",
    "__version__",
    "fixture",
]
