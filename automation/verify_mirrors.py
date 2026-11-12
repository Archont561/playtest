"""Cross-platform mirror SHA-256 comparison and offline installer checks (Spec 0013 §9)."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import tempfile
from collections.abc import Sequence
from pathlib import Path

from automation._mirror_installers import (
    verify_js_manager,
    verify_uv_offline_and_upgrade,
)
from automation._proc import emit_err, emit_line, run_cmd

__all__ = [
    "compare_mirror_hash_files",
    "main",
    "record_mirror_hashes",
    "verify_all_installers",
]


def _sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def record_mirror_hashes(cli_bin: Path, output_file: Path, platform_label: str) -> int:
    """Run `playtest init` + `playtest sync` + `playtest sync --check` and record mirror hashes."""
    cli_abs = cli_bin.resolve()
    with tempfile.TemporaryDirectory(prefix="playtest-mirror-hash-") as tmp:
        ws = Path(tmp)
        for subcmd in (["init"], ["sync"], ["sync", "--check"]):
            res = run_cmd([str(cli_abs), *subcmd], cwd=ws, capture=True)
            if res.returncode != 0:
                emit_err(
                    f"[verify-mirrors] ERROR: playtest {' '.join(subcmd)} failed:\n{res.stderr}"
                )
                return 1
        artifacts: dict[str, dict[str, object]] = {}
        for path in sorted((ws / ".playtest").rglob("*")):
            if path.is_file() and path.name != ".gitignore":
                rel = path.relative_to(ws).as_posix()
                artifacts[rel] = {"sha256": _sha256_file(path), "size": path.stat().st_size}
    payload = {"platform": platform_label, "artifacts": artifacts}
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    emit_line(f"[verify-mirrors] recorded {len(artifacts)} mirror hash(es) -> {output_file}")
    return 0


def compare_mirror_hash_files(inputs: Sequence[Path]) -> int:
    """Verify every recorded mirror hash manifest across platforms is byte-for-byte identical."""
    files: list[Path] = []
    for item in inputs:
        files.extend(sorted(item.rglob("*.json")) if item.is_dir() else [item])
    if len(files) < 2:
        emit_err(
            f"[verify-mirrors] ERROR: need >=2 platform hash files to compare, got {len(files)}"
        )
        return 1
    loaded = [(f, json.loads(f.read_text(encoding="utf-8"))) for f in files]
    ref_file, ref_data = loaded[0]
    ref_artifacts = ref_data.get("artifacts", {})
    if not ref_artifacts:
        emit_err(f"[verify-mirrors] ERROR: empty artifacts map in {ref_file}")
        return 1
    for other_file, other_data in loaded[1:]:
        if other_data.get("artifacts") != ref_artifacts:
            emit_err(
                f"[verify-mirrors] ERROR: cross-platform mirror hash mismatch between "
                f"{ref_data.get('platform')} ({ref_file}) and "
                f"{other_data.get('platform')} ({other_file}):\n"
                f"expected={ref_artifacts}\nactual={other_data.get('artifacts')}"
            )
            return 1
    platforms = [str(item[1].get("platform", item[0].name)) for item in loaded]
    emit_line(f"[verify-mirrors] OK: mirror SHA-256 digests match across {', '.join(platforms)}")
    return 0


def verify_all_installers(cli_bin: Path, v1: str = "0.1.0", v2: str = "0.2.0") -> int:
    """Verify offline fresh-clone install and v1->v2 upgrade across uv, npm, pnpm, and bun."""
    cli_abs = cli_bin.resolve()
    cli_dir = cli_abs.parent
    with tempfile.TemporaryDirectory(prefix="playtest-seed-") as seed_tmp:
        seed = Path(seed_tmp)
        for subcmd in (["init"], ["sync"]):
            res = run_cmd([str(cli_abs), *subcmd], cwd=seed, capture=True)
            if res.returncode != 0:
                emit_err(f"[verify-mirrors] ERROR: seed playtest {subcmd} failed:\n{res.stderr}")
                return 1
        if verify_uv_offline_and_upgrade(seed, cli_dir, v1, v2) != 0:
            return 1
        for manager in ("npm", "pnpm", "bun"):
            if verify_js_manager(manager, seed, cli_dir, v1, v2) != 0:
                return 1
    emit_line("[verify-mirrors] OK: all 4 ecosystems (uv, npm, pnpm, bun) passed offline + upgrade")
    return 0


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Verify Playtest SDK mirrors and determinism.")
    sub = parser.add_subparsers(dest="command", required=True)

    p_rec = sub.add_parser("record-hashes", help="Record mirror SHA-256 hashes on this platform.")
    p_rec.add_argument("--cli-bin", type=Path, required=True)
    p_rec.add_argument("--output", type=Path, required=True)
    p_rec.add_argument("--platform", default=sys.platform)

    p_cmp = sub.add_parser("compare-hashes", help="Compare mirror SHA-256 JSON manifests.")
    p_cmp.add_argument("inputs", nargs="+", type=Path)

    p_inst = sub.add_parser("verify-installers", help="Run offline install + upgrade suite.")
    p_inst.add_argument("--cli-bin", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for mirror determinism and offline installer verification."""
    args = _build_parser().parse_args(list(argv) if argv is not None else None)
    if args.command == "record-hashes":
        return record_mirror_hashes(args.cli_bin, args.output, args.platform)
    if args.command == "compare-hashes":
        return compare_mirror_hash_files(args.inputs)
    return verify_all_installers(args.cli_bin)


if __name__ == "__main__":
    raise SystemExit(main())
