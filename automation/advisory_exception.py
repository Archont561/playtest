"""Verify that Rust advisory exceptions in deny.toml and .cargo/audit.toml remain valid."""

from __future__ import annotations

import argparse
import tomllib
from collections.abc import Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line

EXPECTED_TRANSITIVE_SOURCE: dict[str, tuple[str, str]] = {
    "RUSTSEC-2026-0374": ("i18n-config", "basic-toml"),
}


def _load_lock_edges(cargo_lock: Path) -> dict[str, set[str]]:
    data = tomllib.loads(cargo_lock.read_text(encoding="utf-8"))
    edges: dict[str, set[str]] = {}
    for pkg in data.get("package", []):
        name = str(pkg.get("name", ""))
        deps = {str(raw).split(" ", 1)[0] for raw in pkg.get("dependencies", [])}
        edges.setdefault(name, set()).update(deps)
    return edges


def _parse_deny_ignores(deny_toml: Path) -> dict[str, str]:
    data = tomllib.loads(deny_toml.read_text(encoding="utf-8"))
    raw_items = data.get("advisories", {}).get("ignore", [])
    ignores: dict[str, str] = {}
    for entry in raw_items:
        if isinstance(entry, dict):
            adv_id = str(entry.get("id", "")).strip()
            reason = str(entry.get("reason", "")).strip()
            ignores[adv_id] = reason
        elif isinstance(entry, str):
            ignores[entry.strip()] = ""
    return ignores


def _parse_audit_ignores(audit_toml: Path) -> set[str]:
    data = tomllib.loads(audit_toml.read_text(encoding="utf-8"))
    raw_items = data.get("advisories", {}).get("ignore", [])
    return {str(item).strip() for item in raw_items if str(item).strip()}


def verify_advisory_exceptions(root: Path = REPO_ROOT) -> list[str]:
    """Validate synchronization and continued necessity of ignored RUSTSEC advisories."""
    deny_path = root / "deny.toml"
    audit_path = root / ".cargo" / "audit.toml"
    lock_path = root / "Cargo.lock"
    for required in (deny_path, audit_path, lock_path):
        if not required.is_file():
            return [f"missing required file: {required.relative_to(root).as_posix()}"]

    deny_ignores = _parse_deny_ignores(deny_path)
    audit_ignores = _parse_audit_ignores(audit_path)
    edges = _load_lock_edges(lock_path)
    errors: list[str] = []

    if set(deny_ignores) != audit_ignores:
        errors.append(
            f"deny.toml ({sorted(deny_ignores)}) and .cargo/audit.toml ({sorted(audit_ignores)}) "
            "ignore sets must match"
        )
    for adv_id, reason in sorted(deny_ignores.items()):
        if not reason:
            errors.append(f"{adv_id} in deny.toml must specify a non-empty reason")
        mapping = EXPECTED_TRANSITIVE_SOURCE.get(adv_id)
        if mapping is None:
            errors.append(f"{adv_id} is not registered in EXPECTED_TRANSITIVE_SOURCE")
            continue
        parent_pkg, target_pkg = mapping
        if target_pkg not in edges:
            errors.append(f"stale ignore {adv_id}: {target_pkg} is no longer in Cargo.lock")
        elif target_pkg not in edges.get(parent_pkg, set()):
            errors.append(f"stale ignore {adv_id}: {parent_pkg} no longer depends on {target_pkg}")
    return errors


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for advisory exception validation."""
    parser = argparse.ArgumentParser(description="Validate Rust advisory exceptions.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT, help="Repository root path.")
    args = parser.parse_args(list(argv) if argv is not None else None)
    errors = verify_advisory_exceptions(args.root.resolve())
    if errors:
        for err in errors:
            emit_err(f"[advisory-exception] ERROR: {err}")
        return 1
    emit_line("[advisory-exception] OK: advisory ignores are synchronized and active.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
