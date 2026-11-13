# Playtest for Python

Polyglot test framework with the batteries included: dependency-injected fixtures, strict
LIFO teardown, property-based testing, and code-first BDD for Python.

## Installation
```bash
pip install playtest-py
```

## Quick Start
```python
from collections.abc import Iterator

from playtest import fixture


@fixture
def database() -> Iterator["Connection"]:
    connection = connect()
    yield connection
    connection.close()


def test_reads_the_row(database: "Connection") -> None:
    assert database.query("select 1") == 1
```

Playtest fixtures run under pytest: they register with it, and are injected by parameter
name exactly as a pytest fixture is. That means pytest still discovers and executes the
tests — Playtest layers the shared lifecycle onto the runner you already use rather than
replacing it.

## Teardown

Code after the `yield` is the teardown, and the guarantee is strict last-in-first-out
across every fixture a test requested. Three properties hold, and each is covered by a test:

- Teardown runs in reverse creation order.
- Teardown runs even when the test body raises.
- One failing teardown does not stop the others, and it does not hide the test's own
  failure — the assertion error stays the headline and the cleanup failure is reported
  alongside it as a separate error.

A fixture whose teardown raises is surfaced after every other teardown has run, collected
into a `TeardownError` that lists all of the failures at once rather than only the first.

## Test names and skips

```python
from playtest import TestFacade

test = TestFacade()


@test.test("rejects an empty cart")
def test_empty_cart() -> None:
    ...


def test_needs_a_gpu() -> None:
    test.skip("renders a scene", reason="requires a GPU")
```

`TestFacade` is deliberately thin: it attaches the Playtest display name and raises
pytest's own skip signal, so the runner that owns the process reports the skip rather than
having it simulated.

## Status

This package currently ships the version contract and the fixture/test facade described
above. Code-first BDD and property-based testing land through
[Milestone M5](https://github.com/Archont561/playtest/blob/main/backlog/docs/milestones/M5-v0.5.0-polyglot-conformance.md).
The README tracks the same contract as the other eight targets; it does not promise more
than has shipped.
