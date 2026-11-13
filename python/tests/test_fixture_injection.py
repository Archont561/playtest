"""Integration tests: Playtest fixtures injected under the real pytest runner (`task_007`).

These exercise what a consumer actually writes — ``@fixture``-decorated generators injected
by parameter name — rather than the building blocks, which ``test_fixtures.py`` covers.

Ordering and failure-visibility claims are asserted in a subprocess. pytest does not
promise the order sibling tests run in, so a module-level log shared between test
functions cannot prove anything about LIFO teardown or about which cleanups still ran.
"""

from __future__ import annotations

import subprocess
import sys
import textwrap
from collections.abc import Iterator
from pathlib import Path

import pytest
from playtest.fixtures import TeardownError, TestFacade, fixture

PREAMBLE = """
from collections.abc import Generator

from playtest.fixtures import fixture
"""


def run_session(tmp_path: Path, body: str) -> subprocess.CompletedProcess[str]:
    """Run ``body`` as a throwaway pytest session, with the imports it needs already there."""
    source = textwrap.dedent(PREAMBLE) + textwrap.dedent(body)
    (tmp_path / "test_session.py").write_text(source, encoding="utf-8")
    return subprocess.run(
        [sys.executable, "-m", "pytest", "-q", "-p", "no:cacheprovider", str(tmp_path)],
        capture_output=True,
        text=True,
        check=False,
    )


def output_of(result: subprocess.CompletedProcess[str]) -> str:
    return result.stdout + result.stderr


# --- what a consumer writes -------------------------------------------------


@fixture
def first() -> Iterator[str]:
    yield "one"


@fixture
def second() -> Iterator[str]:
    yield "two"


def test_fixtures_are_injected_by_parameter_name(first: str, second: str) -> None:
    assert (first, second) == ("one", "two")


def test_the_facade_sets_the_playtest_name() -> None:
    @TestFacade().test("a display name")
    def body() -> None:
        return None

    assert body.__playtest_name__ == "a display name"


def test_the_facade_is_not_collected_as_a_test_class(tmp_path: Path) -> None:
    """pytest collects any class named ``Test*``; importing the facade must not run it."""
    result = run_session(
        tmp_path,
        """
        from playtest.fixtures import TestFacade

        def test_import_only() -> None:
            assert TestFacade is not None
        """,
    )
    output = output_of(result)
    assert "fixture 'name' not found" not in output, output
    assert result.returncode == 0, output
    assert "1 passed" in output, output


def test_the_facade_skips_with_a_reason() -> None:
    with pytest.raises(pytest.skip.Exception) as caught:
        TestFacade().skip("not now", reason="needs a database")

    assert "needs a database" in str(caught.value)


# --- explicit resolution ----------------------------------------------------


def test_an_unused_fixture_is_not_constructed(tmp_path: Path) -> None:
    result = run_session(
        tmp_path,
        """
        ORDER: list[str] = []

        @fixture
        def never_requested() -> Generator[str, None, None]:
            ORDER.append("constructed")
            yield "value"

        @fixture
        def used() -> Generator[str, None, None]:
            yield "used"

        def test_only_uses_one(used: str) -> None:
            assert used == "used"

        def test_zz_never_constructed() -> None:
            assert ORDER == [], "unused fixture was constructed: %r" % (ORDER,)
        """,
    )
    assert "unused fixture was constructed" not in output_of(result), output_of(result)
    assert result.returncode == 0, output_of(result)


# --- teardown parity --------------------------------------------------------


def test_teardown_runs_lifo_after_the_body(tmp_path: Path) -> None:
    result = run_session(
        tmp_path,
        """
        ORDER: list[str] = []

        @fixture
        def alpha() -> Generator[str, None, None]:
            yield "a"
            ORDER.append("alpha")

        @fixture
        def beta() -> Generator[str, None, None]:
            yield "b"
            ORDER.append("beta")

        def test_pair(alpha: str, beta: str) -> None:
            assert (alpha, beta) == ("a", "b")
            ORDER.append("body")

        def test_zz_order() -> None:
            assert ORDER == ["body", "beta", "alpha"], "ORDER_MISMATCH %r" % (ORDER,)
        """,
    )
    assert "ORDER_MISMATCH" not in output_of(result), output_of(result)
    assert result.returncode == 0, output_of(result)


def test_teardown_runs_even_when_the_body_fails(tmp_path: Path) -> None:
    result = run_session(
        tmp_path,
        """
        ORDER: list[str] = []

        @fixture
        def alpha() -> Generator[str, None, None]:
            yield "a"
            ORDER.append("alpha")

        def test_fails(alpha: str) -> None:
            raise AssertionError("body blew up")

        def test_zz_order() -> None:
            assert ORDER == ["alpha"], "ORDER_MISMATCH %r" % (ORDER,)
        """,
    )
    output = output_of(result)
    assert "ORDER_MISMATCH" not in output, output
    assert "body blew up" in output, output


def test_a_broken_cleanup_does_not_stop_the_others(tmp_path: Path) -> None:
    result = run_session(
        tmp_path,
        """
        ORDER: list[str] = []

        @fixture
        def alpha() -> Generator[str, None, None]:
            yield "a"
            ORDER.append("alpha")

        @fixture
        def broken() -> Generator[str, None, None]:
            yield "b"
            ORDER.append("broken")
            raise ValueError("cleanup exploded")

        @fixture
        def gamma() -> Generator[str, None, None]:
            yield "c"
            ORDER.append("gamma")

        def test_body(alpha: str, broken: str, gamma: str) -> None:
            assert (alpha, broken, gamma) == ("a", "b", "c")

        def test_zz_order() -> None:
            assert ORDER == ["gamma", "broken", "alpha"], "ORDER_MISMATCH %r" % (ORDER,)
        """,
    )
    assert "ORDER_MISMATCH" not in output_of(result), output_of(result)


def test_a_broken_cleanup_surfaces_without_failing_the_body(tmp_path: Path) -> None:
    """The body passes; the cleanup failure is reported separately, as an error."""
    result = run_session(
        tmp_path,
        """
        @fixture
        def broken() -> Generator[str, None, None]:
            yield "b"
            raise ValueError("cleanup exploded")

        def test_body(broken: str) -> None:
            assert broken == "b"
        """,
    )
    output = output_of(result)
    assert result.returncode != 0, output
    assert "1 passed, 1 error" in output, output
    assert "cleanup exploded" in output, output


def test_a_broken_cleanup_does_not_hide_the_body_failure(tmp_path: Path) -> None:
    """Both surface: the assertion failure stays the headline, the cleanup is an extra."""
    result = run_session(
        tmp_path,
        """
        @fixture
        def broken() -> Generator[str, None, None]:
            yield "b"
            raise ValueError("cleanup exploded")

        def test_body(broken: str) -> None:
            raise AssertionError("body blew up")
        """,
    )
    output = output_of(result)
    assert "body blew up" in output, output
    assert "cleanup exploded" in output, output
    assert "1 failed, 1 error" in output, output


def test_teardown_runs_when_the_consumer_throws_into_the_fixture() -> None:
    """Direct use: the guarantee is a ``finally``, so an exception cannot strand setup."""
    order: list[str] = []

    @fixture
    def resource() -> Iterator[str]:
        yield "value"
        order.append("torn down")

    generator = resource.__wrapped__()  # type: ignore[attr-defined]
    assert next(generator) == "value"
    with pytest.raises(AssertionError):
        generator.throw(AssertionError("body blew up"))

    assert order == ["torn down"]


# --- the error type ---------------------------------------------------------


def test_teardown_error_lists_every_failure() -> None:
    from playtest.fixtures import TeardownReport

    error = TeardownError(TeardownReport(ran=2, failures=["first", "second"]))

    message = str(error)
    assert "2 fixture teardown(s) failed" in message
    assert "first" in message and "second" in message
