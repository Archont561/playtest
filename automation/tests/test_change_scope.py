"""Unit tests for CI path-scoped lane classification."""

from __future__ import annotations

import json

from automation.change_scope import LANE_NAMES, classify_paths
from automation.ci_gate import evaluate_needs


def test_doc_only_changes_skip_all_lanes() -> None:
    result = classify_paths(["README.md", "backlog/tasks.yaml", "docs/README.md"])
    assert result["any_code"] == "false"
    assert result["has_sdks"] == "false"
    assert json.loads(result["sdks"]) == []
    for lane in LANE_NAMES:
        assert result[lane] == "false"


def test_single_sdk_change_activates_only_target_lane() -> None:
    result = classify_paths(["python/src/playtest/__init__.py"])
    assert result["any_code"] == "true"
    assert result["has_sdks"] == "true"
    assert json.loads(result["sdks"]) == ["py"]
    assert result["py"] == "true"
    for lane in LANE_NAMES:
        if lane != "py":
            assert result[lane] == "false"


def test_global_trigger_activates_all_lanes() -> None:
    result = classify_paths(["pixi.toml"])
    assert result["any_code"] == "true"
    for lane in LANE_NAMES:
        assert result[lane] == "true"


def test_force_all_enables_every_lane_even_when_empty() -> None:
    result = classify_paths([], force_all=True)
    assert result["any_code"] == "true"
    for lane in LANE_NAMES:
        assert result[lane] == "true"


def test_unmapped_code_change_falls_back_to_rust_lane() -> None:
    result = classify_paths([".github/dependabot.yml"])
    assert result["any_code"] == "true"
    assert result["rust"] == "true"
    assert result["py"] == "false"


def test_ci_gate_aggregator_accepts_skipped_optional_and_rejects_failed() -> None:
    ok_needs = {
        "scope": {"result": "success"},
        "fast": {"result": "success"},
        "rust": {"result": "skipped"},
        "sdk": {"result": "success"},
        "coverage": {"result": "success"},
    }
    assert evaluate_needs(ok_needs) == []
    bad_needs = {**ok_needs, "rust": {"result": "failure"}}
    assert any("rust" in msg for msg in evaluate_needs(bad_needs))
