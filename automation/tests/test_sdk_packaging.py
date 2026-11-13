"""Tests for the Rust packaging path in `automation.sdk_tasks`.

These cover failure reporting rather than packaging itself: a dry-run packaging step that
fails must say *why*, because the alternative is a CI job that fails in zero seconds with
no diagnostic, which is exactly what a post-merge run did.
"""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path

import pytest

from automation import sdk_tasks
from automation._proc import CommandResult


def _result(returncode: int, stdout: str = "", stderr: str = "") -> CommandResult:
    return CommandResult(returncode=returncode, stdout=stdout, stderr=stderr)


class _Recorder:
    """Stand-in for `run_cmd` that records commands and replays scripted results."""

    def __init__(self, results: dict[tuple[str, ...], CommandResult]) -> None:
        self.results = results
        self.calls: list[tuple[str, ...]] = []

    def __call__(self, cmd: Sequence[str], **_kwargs: object) -> CommandResult:
        key = tuple(cmd)
        self.calls.append(key)
        if key in self.results:
            return self.results[key]
        raise AssertionError(f"unexpected command: {key}")


def _dependent_probe(
    monkeypatch: pytest.MonkeyPatch, results: dict[tuple[str, ...], CommandResult]
) -> tuple[_Recorder, list[str], list[str]]:
    """Wire the dependent-crate path to a recorder and capture both message channels."""
    recorder = _Recorder(results)
    notes: list[str] = []
    errors: list[str] = []
    monkeypatch.setattr(sdk_tasks, "run_cmd", recorder)
    monkeypatch.setattr(sdk_tasks, "emit_line", notes.append)
    monkeypatch.setattr(sdk_tasks, "emit_err", errors.append)
    return recorder, notes, errors


_PLAYTEST_PACKAGE = (
    "cargo",
    "package",
    "-p",
    "playtest",
    "--no-verify",
    "--allow-dirty",
    "--locked",
)
_PLAYTEST_LIST = ("cargo", "package", "-p", "playtest", "--list", "--offline", "--allow-dirty")


_PLAYTEST_UNPUBLISHED = _result(
    101,
    stderr=(
        "error: failed to prepare local package for uploading\n"
        "  no matching package named `playtest-core` found"
    ),
)


def test_an_unpublished_dependency_falls_back_to_the_file_set(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A dependent crate that awaits publication falls back to `--list`, and says so.

    `playtest` and `playtest-cli` cannot be packed until their workspace dependencies are
    in the registry. That is the release order, not a defect, so the step stays green and
    validates the file set instead.
    """
    recorder, notes, errors = _dependent_probe(
        monkeypatch,
        {
            _PLAYTEST_PACKAGE: _PLAYTEST_UNPUBLISHED,
            _PLAYTEST_LIST: _result(0, stdout="src/lib.rs"),
        },
    )

    status = sdk_tasks._package_dependent_crates(Path(), ("playtest",))

    assert status == 0, "an unpublished workspace dependency is not a failure"
    assert errors == [], "the expected pre-publication state is not an error"
    assert _PLAYTEST_LIST in recorder.calls, "the fallback must still run"
    joined = "\n".join(notes)
    assert "playtest" in joined, "the note must name the crate"
    assert "unpublished workspace crate" in joined, "the note must explain why"


def test_any_other_dependent_crate_failure_is_fatal(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A failure that is not the pre-publication state must fail the step.

    The earlier version downgraded every failure to a note and returned the `--list`
    result, which exits 0 even when the crate cannot be packaged. A broken packaging step
    therefore passed locally while CI failed on the same code.
    """
    recorder, _, errors = _dependent_probe(
        monkeypatch,
        {
            _PLAYTEST_PACKAGE: _result(101, stderr="error: failed to verify package"),
        },
    )

    status = sdk_tasks._package_dependent_crates(Path(), ("playtest",))

    assert status != 0, "an unexpected failure must not be downgraded to a warning"
    assert _PLAYTEST_LIST not in recorder.calls, "the file-set fallback must not mask it"
    joined = "\n".join(errors)
    assert "playtest" in joined
    assert "failed to verify package" in joined


def test_a_failing_file_set_validation_is_a_real_failure(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """When the fallback itself fails, the step must fail and quote that error."""
    _, _, errors = _dependent_probe(
        monkeypatch,
        {
            _PLAYTEST_PACKAGE: _PLAYTEST_UNPUBLISHED,
            _PLAYTEST_LIST: _result(101, stderr="failed to read src/missing.rs"),
        },
    )

    status = sdk_tasks._package_dependent_crates(Path(), ("playtest",))

    assert status != 0
    joined = "\n".join(errors)
    assert "playtest" in joined
    assert "missing.rs" in joined


def test_a_leaf_crate_failure_is_reported_with_cargo_stderr(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A leaf crate that fails to pack is fatal and must quote cargo's message."""
    messages: list[str] = []
    package_cmd = (
        "cargo",
        "package",
        "-p",
        "playtest-macros",
        "--no-verify",
        "--allow-dirty",
        "--locked",
    )
    recorder = _Recorder({package_cmd: _result(101, stderr="spurious network error")})
    monkeypatch.setattr(sdk_tasks, "run_cmd", recorder)
    monkeypatch.setattr(sdk_tasks, "emit_err", messages.append)
    monkeypatch.setattr(sdk_tasks, "stage_assets", lambda *_a, **_k: [])
    monkeypatch.setattr(sdk_tasks, "clean_staged_assets", lambda *_a, **_k: None)

    status = sdk_tasks._package_leaf_crates(Path(), ("playtest-macros",))

    assert status != 0
    joined = "\n".join(messages)
    assert "playtest-macros" in joined
    assert "spurious network error" in joined


def test_a_leaf_crate_failure_with_empty_stderr_still_explains_itself(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """An empty diagnostic must not produce a blank explanation."""
    package_cmd = (
        "cargo",
        "package",
        "-p",
        "playtest-core",
        "--no-verify",
        "--allow-dirty",
        "--locked",
    )
    messages: list[str] = []
    recorder = _Recorder({package_cmd: _result(1)})
    monkeypatch.setattr(sdk_tasks, "run_cmd", recorder)
    monkeypatch.setattr(sdk_tasks, "emit_err", messages.append)
    monkeypatch.setattr(sdk_tasks, "stage_assets", lambda *_a, **_k: [])
    monkeypatch.setattr(sdk_tasks, "clean_staged_assets", lambda *_a, **_k: None)

    sdk_tasks._package_leaf_crates(Path(), ("playtest-core",))

    assert "cargo produced no diagnostic" in "\n".join(messages)


def test_a_successful_dependent_crate_returns_zero(monkeypatch: pytest.MonkeyPatch) -> None:
    """The happy path must stay quiet and successful."""
    package_cmd = ("cargo", "package", "-p", "playtest", "--no-verify", "--allow-dirty", "--locked")
    recorder = _Recorder({package_cmd: _result(0)})
    messages: list[str] = []
    monkeypatch.setattr(sdk_tasks, "run_cmd", recorder)
    monkeypatch.setattr(sdk_tasks, "emit_err", messages.append)

    assert sdk_tasks._package_dependent_crates(Path(), ("playtest",)) == 0
    assert messages == []
