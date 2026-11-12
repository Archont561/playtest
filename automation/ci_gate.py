"""Required branch-protection gate aggregator for GitHub Actions CI."""

from __future__ import annotations

import json
import os
from collections.abc import Mapping, Sequence
from typing import Any

from automation._proc import emit_err, emit_line

MANDATORY_SUCCESS_JOBS: tuple[str, ...] = ("scope", "fast")
ALLOWED_OPTIONAL_RESULTS: frozenset[str] = frozenset({"success", "skipped"})


def evaluate_needs(needs: Mapping[str, Mapping[str, Any]]) -> list[str]:
    """Validate upstream GitHub Actions `needs` results and return any failure messages."""
    errors: list[str] = []
    for required in MANDATORY_SUCCESS_JOBS:
        result = str(needs.get(required, {}).get("result", "missing"))
        if result != "success":
            errors.append(f"mandatory job '{required}' finished with result={result!r}")
    for job_name, payload in sorted(needs.items()):
        if job_name in MANDATORY_SUCCESS_JOBS:
            continue
        result = str(payload.get("result", "missing"))
        if result not in ALLOWED_OPTIONAL_RESULTS:
            errors.append(f"job '{job_name}' finished with disallowed result={result!r}")
    return errors


def main(_argv: Sequence[str] | None = None) -> int:
    """Parse NEEDS_JSON from the environment and exit non-zero on any upstream failure."""
    raw = os.environ.get("NEEDS_JSON", "{}").strip() or "{}"
    try:
        parsed = json.loads(raw)
    except ValueError as err:
        emit_err(f"[ci-gate] ERROR: invalid NEEDS_JSON payload: {err}")
        return 1
    if not isinstance(parsed, dict):
        emit_err("[ci-gate] ERROR: NEEDS_JSON must decode to a JSON object")
        return 1
    for job_name, payload in sorted(parsed.items()):
        res = payload.get("result", "unknown") if isinstance(payload, dict) else "invalid"
        emit_line(f"[ci-gate] {job_name}: {res}")
    errors = evaluate_needs(parsed)
    if errors:
        for item in errors:
            emit_err(f"[ci-gate] FAILED: {item}")
        return 1
    emit_line("[ci-gate] OK: all CI gates satisfied.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
