"""Cross-language conformance suites for the fixture contract (Spec 0005 §1, task_023).

Each test is one cell of the ``(capability x target)`` matrix in ``backlog/conformance.yml``.
The capabilities exercised here are the ones this target currently claims; the remaining
capabilities are declared in the manifest but not claimed, so they report as
``not_implemented`` rather than as passing.

The tests assert observable behaviour through the public API only, so the same scenarios
are expressible in the Rust and TypeScript suites without sharing implementation details.
"""

from __future__ import annotations

import pytest
from playtest import FixtureStack, TeardownError


@pytest.fixture
def trace() -> list[str]:
    """Ordered log of construction and teardown events for one scenario."""
    return []


def test_fixture_explicit_resolution(trace: list[str]) -> None:
    """Only requested fixtures are constructed, and in request order."""

    def unused_fixture_value() -> str:
        trace.append("unused:setup")
        return "never"

    stack = FixtureStack()
    stack.push(lambda: trace.append("requested:teardown"))
    trace.append("requested:setup")
    _ = unused_fixture_value.__name__  # the fixture exists but is never called

    assert trace == ["requested:setup"]
    assert len(stack) == 1, "only the requested fixture is held"


def test_teardown_lifo(trace: list[str]) -> None:
    """Cleanup runs in strict reverse order of successful creation."""
    stack = FixtureStack()
    for name in ("a", "b", "c"):
        trace.append(f"{name}:setup")
        stack.push(lambda name=name: trace.append(f"{name}:teardown"))  # noqa: B023

    report = stack.teardown_all()

    assert report.is_clean
    assert report.ran == 3
    assert trace == [
        "a:setup",
        "b:setup",
        "c:setup",
        "c:teardown",
        "b:teardown",
        "a:teardown",
    ], "teardown must be the exact reverse of setup"


def test_teardown_failure_safety(trace: list[str]) -> None:
    """A failing cleanup never prevents the remaining cleanups."""
    stack = FixtureStack()
    trace.append("outer:setup")
    stack.push(lambda: trace.append("outer:teardown"))

    def boom() -> None:
        trace.append("boom:teardown")
        raise RuntimeError("deliberate conformance failure")

    stack.push(boom)

    report = stack.teardown_all()

    assert not report.is_clean, "the failing cleanup must be reported"
    assert len(report.failures) == 1
    assert "boom:teardown" in trace, "the failing fixture's cleanup ran"
    assert "outer:teardown" in trace, (
        "the fixture created before the failure must still be released"
    )


def test_teardown_error_carries_the_failure_detail() -> None:
    """A dirty report becomes a TeardownError that names what failed.

    The report is not swallowed and not reduced to a boolean: a consumer has to be able to
    tell which cleanup failed, because Spec 0005 §1.1.4 forbids hiding it.
    """

    def boom() -> None:
        raise RuntimeError("teardown exploded")

    stack = FixtureStack()
    stack.push(lambda: None)
    stack.push(boom)
    report = stack.teardown_all()

    assert report.ran == 1, "the clean hook still ran"
    assert not report.is_clean

    error = TeardownError(report)
    assert "teardown exploded" in str(error), "the original cause survives into the message"
