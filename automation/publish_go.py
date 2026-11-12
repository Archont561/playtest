"""Tag the Go subdirectory module (`go/vX.Y.Z`) and request Go proxy indexing."""

from __future__ import annotations

import argparse
import urllib.error
import urllib.request
from collections.abc import Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd
from automation.stamp_version import extract_version_from_tag

GO_MODULE_PATH: str = "github.com/archont561/playtest/go"


def ensure_go_module_tag(release_tag: str, *, root: Path = REPO_ROOT, dry_run: bool = False) -> str:
    """Ensure `go/vX.Y.Z` exists on origin at HEAD and return the Go tag name."""
    version = extract_version_from_tag(release_tag)
    go_tag = f"go/v{version}"
    if dry_run:
        emit_line(f"[publish-go] DRY-RUN: would tag and push {go_tag}")
        return go_tag

    existing = run_cmd(
        ["git", "rev-parse", "-q", "--verify", f"refs/tags/{go_tag}"], cwd=root, capture=True
    )
    if existing.returncode != 0:
        cfg_name = run_cmd(["git", "config", "user.name", "github-actions[bot]"], cwd=root)
        cfg_mail = run_cmd(
            [
                "git",
                "config",
                "user.email",
                "41898282+github-actions[bot]@users.noreply.github.com",
            ],
            cwd=root,
        )
        if cfg_name.returncode != 0 or cfg_mail.returncode != 0:
            raise RuntimeError("failed to configure git identity for Go module tag")
        tag_res = run_cmd(
            ["git", "tag", "-a", go_tag, "-m", f"Go module release {go_tag}"], cwd=root
        )
        if tag_res.returncode != 0:
            raise RuntimeError(f"failed to create tag {go_tag}")
    push_res = run_cmd(["git", "push", "origin", f"refs/tags/{go_tag}"], cwd=root)
    if push_res.returncode != 0:
        raise RuntimeError(f"failed to push {go_tag} to origin")
    return go_tag


def request_go_proxy_index(version: str, *, dry_run: bool = False) -> None:
    """Ask proxy.golang.org to index `vX.Y.Z` for the `go/` subdirectory module."""
    url = f"https://proxy.golang.org/{GO_MODULE_PATH}/@v/v{version}.info"
    if dry_run:
        emit_line(f"[publish-go] DRY-RUN: would GET {url}")
        return
    req = urllib.request.Request(url, headers={"User-Agent": "playtest-release-automation"})
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            emit_line(f"[publish-go] Go proxy responded HTTP {resp.status} for v{version}")
    except (urllib.error.URLError, TimeoutError) as err:
        emit_err(f"[publish-go] WARN: Go proxy warm-up returned {err}; tag is pushed.")


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for Go module subdirectory tag publishing."""
    parser = argparse.ArgumentParser(description="Publish Go subdirectory module tag.")
    parser.add_argument("release_tag", help="Release tag (e.g. v0.2.0 or go/v0.2.0).")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    args = parser.parse_args(list(argv) if argv is not None else None)
    try:
        version = extract_version_from_tag(args.release_tag)
        go_tag = ensure_go_module_tag(
            args.release_tag, root=args.root.resolve(), dry_run=args.dry_run
        )
        request_go_proxy_index(version, dry_run=args.dry_run)
    except (ValueError, RuntimeError) as err:
        emit_err(f"[publish-go] ERROR: {err}")
        return 1
    emit_line(f"[publish-go] OK: published {go_tag}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
