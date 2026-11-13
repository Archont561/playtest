"""Fixture lifecycle parity tests (`Spec 0005 §1.1`, `task_007`)."""

from __future__ import annotations

from collections.abc import Generator, Iterator

import pytest
from playtest.fixtures import FixtureStack, TeardownError, TeardownReport, fixture


def test_stack_releases_in_reverse_creation_order() -> None:
    order: list[str] = []
    stack = FixtureStack()
    stack.push(lambda: order.append("first"))
    stack.push(lambda: order.append("second"))
    stack.push(lambda: order.append("third"))

    report = stack.teardown_all()

    assert order == ["third", "second", "first"]
    assert report.ran == 3
    assert report.is_clean
    assert len(stack) == 0


def test_a_raising_teardown_does_not_stop_the_others() -> None:
    order: list[str] = []
    stack = FixtureStack()

    def explode() -> None:
        raise ValueError("cleanup went wrong")

    stack.push(lambda: order.append("before"))
    stack.push(explode)
    stack.push(lambda: order.append("after"))

    report = stack.teardown_all()

    assert order == ["after", "before"], "both surviving hooks still ran"
    assert report.ran == 2
    assert len(report.failures) == 1
    assert "cleanup went wrong" in report.failures[0]


def test_teardown_is_idempotent() -> None:
    calls: list[str] = []
    stack = FixtureStack()
    stack.push(lambda: calls.append("once"))

    stack.teardown_all()
    second = stack.teardown_all()

    assert calls == ["once"]
    assert second.ran == 0


def test_teardown_error_lists_every_failure() -> None:
    stack = FixtureStack()

    def first() -> None:
        raise ValueError("one")

    def second() -> None:
        raise ValueError("two")

    stack.push(first)
    stack.push(second)
    report = stack.teardown_all()

    error = TeardownError(report)
    assert "2 fixture teardown(s) failed" in str(error)
    assert "one" in str(error)
    assert "two" in str(error)
    assert report.ran == 0


def test_fixture_decorator_marks_and_registers() -> None:
    @fixture
    def resource() -> Generator[str, None, None]:
        yield "value"

    assert getattr(resource, "__playtest_fixture__", False) is True
    # `pytest.fixture` preserves the wrapped function, which is what the wrapper runs.
    inner = resource.__wrapped__ if hasattr(resource, "__wrapped__") else resource
    generator = inner()
    assert next(generator) == "value"
    with pytest.raises(StopIteration):
        next(generator)


def test_fixture_reports_a_second_yield() -> None:
    @fixture
    def broken() -> Generator[int, None, None]:
        yield 1
        yield 2

    inner = broken.__wrapped__ if hasattr(broken, "__wrapped__") else broken
    generator = inner()
    assert next(generator) == 1
    with pytest.raises(RuntimeError, match="yielded more than once"):
        next(generator)


def test_fixture_reports_a_generator_that_never_yields() -> None:
    @fixture
    def empty() -> Iterator[int]:
        if False:  # pragma: no cover - makes this a generator without yielding.
            yield 1

    inner = empty.__wrapped__ if hasattr(empty, "__wrapped__") else empty
    generator = inner()
    with pytest.raises(RuntimeError, match="yielded nothing"):
        next(generator)


def test_teardown_report_describes_itself() -> None:
    """``str`` is what a reader sees in a failure diff, so it must carry both counters."""
    clean = TeardownReport(ran=2, failures=[])
    assert str(clean) == "ran=2 failures=[]"
    assert clean.is_clean is True

    dirty = TeardownReport(ran=0, failures=["boom"])
    assert "boom" in str(dirty)
    assert dirty.is_clean is False
