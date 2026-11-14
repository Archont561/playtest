"""Tests for the cross-language conformance matrix (task_023).

The suite commands are substituted with fakes: these tests cover the reader and the
matrix, not the language toolchains. The one thing they insist on is that a cell is never
reported as passing unless a suite actually ran and passed.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from automation import conformance, conformance_cli
from automation._proc import CommandResult

MANIFEST = """
schema_version = 1

[[capabilities]]
id = "alpha"
title = "Alpha capability"
spec = "0005 §1.1"
optional = false

[[capabilities]]
id = "beta"
title = "Beta capability"
spec = "0005 §1.2"
optional = true

[[suites]]
lang = "rust"
target = "cargo nextest"
cwd = "."
cmd = ["cargo", "nextest", "run"]
capabilities = ["alpha"]
"""


def _write_manifest(root: Path, body: str = MANIFEST) -> Path:
    path = root / conformance.MANIFEST_REL
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")
    return path


def _ok(_cmd: object, **_kwargs: object) -> CommandResult:
    return CommandResult(returncode=0, stdout="", stderr="")


def _fail(_cmd: object, **_kwargs: object) -> CommandResult:
    return CommandResult(returncode=1, stdout="", stderr="boom")


def test_loading_the_real_manifest_succeeds() -> None:
    """The committed manifest must be valid; this is the guard against drift."""
    manifest = conformance.load_manifest()

    assert manifest.schema_version == conformance.SCHEMA_VERSION
    ids = {cap.id for cap in manifest.capabilities}
    assert "teardown_lifo" in ids
    for suite in manifest.suites:
        assert suite.lang in conformance.KNOWN_LANGS
        assert suite.capabilities


def test_a_missing_manifest_is_an_error(tmp_path: Path) -> None:
    with pytest.raises(conformance.ManifestError, match="missing conformance manifest"):
        conformance.load_manifest(tmp_path)


def test_unknown_capability_in_a_suite_is_rejected(tmp_path: Path) -> None:
    body = MANIFEST.replace('capabilities = ["alpha"]', 'capabilities = ["nope"]')
    _write_manifest(tmp_path, body)

    with pytest.raises(conformance.ManifestError, match="unknown capability"):
        conformance.load_manifest(tmp_path)


def test_unknown_language_is_rejected(tmp_path: Path) -> None:
    body = MANIFEST.replace('lang = "rust"', 'lang = "cobol"')
    _write_manifest(tmp_path, body)

    with pytest.raises(conformance.ManifestError, match="unknown language target"):
        conformance.load_manifest(tmp_path)


def test_duplicate_suite_for_one_language_is_rejected(tmp_path: Path) -> None:
    suite_block = """
[[suites]]
lang = "rust"
target = "cargo nextest"
cwd = "."
cmd = ["cargo", "nextest", "run"]
capabilities = ["alpha"]
"""
    _write_manifest(tmp_path, MANIFEST + suite_block)

    with pytest.raises(conformance.ManifestError, match="duplicate suite"):
        conformance.load_manifest(tmp_path)


def test_a_passing_suite_fills_only_the_capabilities_it_claims(tmp_path: Path) -> None:
    _write_manifest(tmp_path)
    manifest = conformance.load_manifest(tmp_path)

    rows = conformance.build_matrix(manifest, root=tmp_path, run=_ok)

    by_cap = {row.capability: row.states for row in rows}
    assert by_cap["alpha"]["rust"] == conformance.PASS
    # `beta` is declared but claimed by nobody, so it must not read as passing anywhere.
    assert set(by_cap["beta"].values()) == {conformance.NOT_IMPLEMENTED}


def test_a_failing_suite_marks_every_claimed_capability(tmp_path: Path) -> None:
    _write_manifest(tmp_path)
    manifest = conformance.load_manifest(tmp_path)

    rows = conformance.build_matrix(manifest, root=tmp_path, run=_fail)

    assert rows[0].states["rust"] == conformance.FAIL


def test_a_missing_runner_is_unavailable_not_a_failure(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A toolchain that is not installed must not be reported as a conformance failure."""
    body = MANIFEST.replace('cmd = ["cargo", "nextest", "run"]', 'cmd = ["definitely-absent-tool"]')
    _write_manifest(tmp_path, body)
    manifest = conformance.load_manifest(tmp_path)
    monkeypatch.setattr(conformance, "_tool_available", lambda *_a, **_k: False)

    rows = conformance.build_matrix(manifest, root=tmp_path, run=_fail)

    assert rows[0].states["rust"] == conformance.UNAVAILABLE


def test_an_unimplemented_target_is_never_a_pass(tmp_path: Path) -> None:
    """Seven of the nine targets are unclaimed today; none may render as green."""
    _write_manifest(tmp_path)
    manifest = conformance.load_manifest(tmp_path)

    rows = conformance.build_matrix(manifest, root=tmp_path, run=_ok)

    unclaimed = {lang for lang, state in rows[0].states.items() if state != conformance.PASS}
    assert len(unclaimed) == len(conformance.KNOWN_LANGS) - 1
    assert all(rows[0].states[lang] == conformance.NOT_IMPLEMENTED for lang in unclaimed)


def test_full_parity_is_false_while_any_required_cell_is_missing(tmp_path: Path) -> None:
    _write_manifest(tmp_path)
    manifest = conformance.load_manifest(tmp_path)

    rows = conformance.build_matrix(manifest, root=tmp_path, run=_ok)
    summary = conformance.summarize(manifest, rows)

    assert summary["full_parity"] is False
    blocking = summary["blocking"]
    assert isinstance(blocking, list)
    assert any(entry.startswith("python/alpha=") for entry in blocking)


def test_an_optional_capability_does_not_block_full_parity(tmp_path: Path) -> None:
    """`beta` is optional, so leaving it unimplemented is not a parity blocker."""
    _write_manifest(tmp_path)
    manifest = conformance.load_manifest(tmp_path)

    rows = conformance.build_matrix(manifest, root=tmp_path, run=_ok)
    summary = conformance.summarize(manifest, rows)

    blocking = summary["blocking"]
    assert isinstance(blocking, list)
    assert not any("beta" in entry for entry in blocking)


def test_check_require_full_exits_nonzero_when_parity_is_missing(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    _write_manifest(tmp_path)
    monkeypatch.setattr(conformance, "run_cmd", _ok)

    assert conformance_cli.main(["--root", str(tmp_path), "check", "--require-full"]) == 1


def test_check_reports_manifest_errors_as_usage_exit_code(tmp_path: Path) -> None:
    assert conformance_cli.main(["--root", str(tmp_path), "check"]) == 2


def test_list_emits_schema_versioned_json(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    import json  # noqa: PLC0415 - only this test decodes JSON.

    _write_manifest(tmp_path)
    assert conformance_cli.main(["--root", str(tmp_path), "list"]) == 0
    document = json.loads(capsys.readouterr().out)

    assert document["schema_version"] == conformance.SCHEMA_VERSION
    assert [cap["id"] for cap in document["capabilities"]] == ["alpha", "beta"]
    assert document["suites"][0]["lang"] == "rust"


def test_gate_passes_only_the_cells_that_ran(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A suite that passes and claims its capabilities is enough for the gate.

    `unavailable` is allowed by design: a runner without the toolchain installed is a fact
    about the machine, not a defect in the SDK.
    """
    _write_manifest(tmp_path)
    monkeypatch.setattr(conformance, "run_suite", lambda *_a, **_k: conformance.PASS)

    assert conformance_cli.main(["--root", str(tmp_path), "check", "--gate"]) == 0


def test_gate_fails_when_a_claimed_capability_fails(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A failing suite must break the gate, otherwise it proves nothing in CI."""
    _write_manifest(tmp_path)
    monkeypatch.setattr(conformance, "run_suite", lambda *_a, **_k: conformance.FAIL)

    assert conformance_cli.main(["--root", str(tmp_path), "check", "--gate"]) == 1


def test_gate_rejects_an_unclaimed_required_capability_whose_suite_ran(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A claimed capability is allowed to report `not_implemented`; a claimed one is not.

    The distinction the gate draws: a suite that ran and claims ``alpha`` but reports
    ``not_implemented`` proves nothing about it, so the gate rejects the cell. A suite
    that does not claim ``alpha`` at all is a different target's responsibility and is
    reported by ``--require-full`` instead.
    """
    _write_manifest(tmp_path)
    manifest = conformance.load_manifest(tmp_path)
    rows = [
        conformance.CellResult(capability="alpha", states={"rust": conformance.NOT_IMPLEMENTED})
    ]

    summary = conformance.summarize(manifest, rows)

    assert summary["gate_rejected"] == ["rust/alpha=not_implemented"]


def test_gate_allows_an_optional_capability_to_stay_unclaimed(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """`beta` is optional, so leaving it unclaimed is not a gate failure."""
    _write_manifest(tmp_path)
    monkeypatch.setattr(conformance, "run_cmd", _ok)

    rows = conformance.build_matrix(conformance.load_manifest(tmp_path), root=tmp_path, run=_ok)
    summary = conformance.summarize(conformance.load_manifest(tmp_path), rows)
    rejected = summary["gate_rejected"]

    assert isinstance(rejected, list)  # noqa: S101 - narrowing for the type checker.
    assert not any("beta" in entry for entry in rejected)


REQUIRES_MANIFEST = MANIFEST.replace(
    'capabilities = ["alpha"]',
    'capabilities = ["alpha"]\nrequires = ["target/test-classes"]',
)


def _write_requires_manifest(root: Path) -> None:
    _write_manifest(root, REQUIRES_MANIFEST)


def test_a_suite_whose_build_output_is_absent_is_unavailable(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """`java` is on every runner, so a resolvable runner must not imply a built tree.

    The Java suite runs a class directory that `javac` produced elsewhere. When that
    directory is missing, the runner still exists and still exits non-zero -- reported as
    `fail`, that blames the SDK for a build step nobody ran.
    """
    _write_requires_manifest(tmp_path)
    monkeypatch.setattr(conformance, "_tool_available", lambda *_a, **_k: True)

    rows = conformance.build_matrix(conformance.load_manifest(tmp_path), root=tmp_path, run=_fail)

    assert rows[0].states["rust"] == conformance.UNAVAILABLE


def test_a_suite_whose_build_output_exists_runs_normally(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Declaring a requirement must not make the suite unprovable once it is satisfied."""
    _write_requires_manifest(tmp_path)
    (tmp_path / "target" / "test-classes").mkdir(parents=True)
    monkeypatch.setattr(conformance, "_tool_available", lambda *_a, **_k: True)

    rows = conformance.build_matrix(conformance.load_manifest(tmp_path), root=tmp_path, run=_ok)

    assert rows[0].states["rust"] == conformance.PASS


def test_list_reports_the_artifacts_a_suite_requires(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """The `list` document is the machine-readable contract, so it carries `requires`."""
    import json  # noqa: PLC0415 - only this test decodes JSON.

    _write_requires_manifest(tmp_path)
    assert conformance_cli.main(["--root", str(tmp_path), "list"]) == 0
    document = json.loads(capsys.readouterr().out)

    assert document["suites"][0]["requires"] == ["target/test-classes"]
