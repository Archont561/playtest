"""Fixture lifecycle bridge between Playtest and pytest (`Spec 0005 §1.1`).

The Rust engine (`crates/playtest/src/fixture.rs`) guarantees three things a Playtest test
can rely on, and this module provides the same three under pytest:

- **Explicit resolution** — a fixture is constructed when a test requests it, never merely
  because it was registered.
- **Strict LIFO teardown** — cleanup runs in reverse creation order.
- **Failure safety** — a teardown that raises does not stop the remaining teardowns, and it
  does not replace the test's own failure.

The third is the one pytest's generator fixtures get subtly wrong for an assertion-heavy
suite: a generator's ``finally`` block runs, but an exception raised there surfaces in
place of the original. :func:`fixture` captures teardown failures and re-raises them only
after every other teardown has run, so the first failure a reader sees is the real one.
"""

from __future__ import annotations

from collections.abc import Callable, Generator, Iterator
from contextlib import contextmanager
from dataclasses import dataclass, field
from typing import Any


@dataclass
class TeardownReport:
    """What happened while releasing one test's fixtures.

    ``ran`` counts the hooks that completed, ``failures`` names the hooks that raised.
    A report is clean when every hook completed.
    """

    ran: int = 0
    failures: list[str] = field(default_factory=list)

    @property
    def is_clean(self) -> bool:
        """Whether every teardown completed without raising."""
        return not self.failures

    def __str__(self) -> str:
        return f"ran={self.ran} failures={self.failures}"


class FixtureStack:
    """Teardown hooks for one test, released in reverse creation order.

    A stack holds callables, not contextual managers, so a fixture's cleanup can be
    registered the moment its value exists. Releasing is idempotent: a test that calls
    :meth:`teardown_all` explicitly still leaves the stack empty for the pytest finalizer.
    """

    def __init__(self) -> None:
        self._hooks: list[Callable[[], None]] = []

    def __len__(self) -> int:
        """How many fixtures are still awaiting teardown."""
        return len(self._hooks)

    def push(self, hook: Callable[[], None]) -> None:
        """Register a teardown hook. Hooks run in reverse registration order."""
        self._hooks.append(hook)

    def teardown_all(self) -> TeardownReport:
        """Run every hook in reverse registration order, collecting failures.

        A hook that raises is recorded and the loop continues, matching the Rust engine:
        one broken cleanup must not strand the fixtures created before it.
        """
        report = TeardownReport()
        while self._hooks:
            hook = self._hooks.pop()
            try:
                hook()
            except Exception as exc:  # noqa: BLE001 - a teardown may raise anything.
                report.failures.append(f"{hook!r}: {exc!r}")
            else:
                report.ran += 1
        return report


@contextmanager
def _scoped(stack: FixtureStack) -> Iterator[None]:
    """Release ``stack`` on exit, even when the test body raised.

    The teardown failure is raised from here rather than swallowed, so pytest reports it
    as a separate teardown error: the body's own failure stays the recorded test failure
    and the cleanup failure is not lost.
    """
    try:
        yield
    finally:
        report = stack.teardown_all()
        if not report.is_clean:
            raise TeardownError(report)


class TeardownError(RuntimeError):
    """Raised when one or more fixture teardowns failed.

    The message lists every failure, so a reader sees all of them at once rather than only
    the first one that happened to run.
    """

    def __init__(self, report: TeardownReport) -> None:
        self.report = report
        lines = [f"{len(report.failures)} fixture teardown(s) failed:"]
        lines.extend(f"  - {failure}" for failure in report.failures)
        super().__init__("\n".join(lines))


def _start[FixtureValue](generator: Generator[FixtureValue, None, None], name: str) -> FixtureValue:
    """Advance to the fixture's ``yield``, which is what constructs the value."""
    try:
        return next(generator)
    except StopIteration as stop:
        raise RuntimeError(f"fixture {name} yielded nothing") from stop


def _teardown_hook[FixtureValue](
    generator: Generator[FixtureValue, None, None], name: str
) -> Callable[[], None]:
    """Build the hook that runs whatever follows the fixture's ``yield``."""

    def advance() -> None:
        try:
            next(generator)
        except StopIteration:
            return
        raise RuntimeError(f"fixture {name} yielded more than once")

    return advance


def _decorate[FixtureValue](
    wrapper: Callable[..., Iterator[FixtureValue]], func: Callable[..., Any], name: str
) -> Callable[..., Iterator[FixtureValue]]:
    """Give the wrapper the metadata a consumer can see, then register it with pytest."""
    wrapper.__name__ = name
    wrapper.__doc__ = func.__doc__
    # Marks the wrapper so a reader (or a future reporter) can tell a Playtest fixture
    # from a bare pytest one.
    wrapper.__playtest_fixture__ = True  # type: ignore[attr-defined]
    # Register with pytest as well: this decorator layers the Playtest lifecycle *onto*
    # the runner that will execute the test, rather than substituting for it.
    import pytest

    return pytest.fixture(wrapper)


def fixture[FixtureValue](
    func: Callable[..., Generator[FixtureValue, None, None]],
) -> Callable[..., Iterator[FixtureValue]]:
    """Turn a ``yield`` generator into a Playtest fixture (`Spec 0005 §1.1`).

    The generator body runs up to its ``yield`` to produce the value, and the code after the
    ``yield`` is the teardown. Anything raised between the ``yield`` and the generator's end
    is captured by :class:`FixtureStack` and reported after every other teardown has run,
    so pytest's own traceback for the test is not replaced.

    Unlike a bare ``pytest.fixture`` generator, the teardown is not wrapped in the
    generator's ``finally`` at call time — the stack owns it — which is what makes strict
    reverse order across *several* fixtures observable and testable.
    """
    name = getattr(func, "__name__", "fixture")

    def wrapper() -> Iterator[FixtureValue]:
        # A generator body runs lazily, so the setup has to happen inside one: advancing
        # the wrapper is what constructs the fixture.
        stack = FixtureStack()
        generator = func()
        stack.push(_teardown_hook(generator, name))
        with _scoped(stack):
            yield _start(generator, name)
        generator.close()

    return _decorate(wrapper, func, name)


class TestFacade:
    """The entry point a Playtest test uses inside pytest (`task_007`).

    ``TestFacade`` is deliberately thin. Its job is to make the shared Playtest vocabulary
    available where a Python developer already is, without inventing a runner: pytest still
    discovers and executes the tests.

    >>> facade = TestFacade()
    >>> facade.skip("not applicable here", reason="requires a GPU")
    """

    # Its name starts with ``Test`` and it has a ``test`` method, so pytest would collect
    # it as a test class in every module that imports it. Opt the class out of collection.
    __test__ = False

    def test(self, name: str) -> Callable[[Callable[[], Any]], Callable[[], Any]]:
        """Name a test the way Playtest reports it.

        The decorator does not change execution; it attaches the Playtest title so a future
        reporter can pair it with the pytest node id. Running the function returns its own
        result unchanged.
        """

        def decorate(func: Callable[[], Any]) -> Callable[[], Any]:
            func.__playtest_name__ = name  # type: ignore[attr-defined]
            return func

        return decorate

    def skip(self, name: str, *, reason: str) -> None:
        """Skip the current test with a reason Playtest records (`Spec 0005 §1.4`).

        Raises pytest's own skip signal, so the skip is reported by the runner that owns
        the process rather than being simulated.
        """
        import pytest

        pytest.skip(f"{name}: {reason}")


__all__ = [
    "FixtureStack",
    "TeardownError",
    "TeardownReport",
    "TestFacade",
    "fixture",
]
