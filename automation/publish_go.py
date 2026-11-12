"""Tag the Go subdirectory module (`go/vX.Y.Z`) after binary release verification."""

from __future__ import annotations

import argparse
import urllib.error
import urllib.request
from collections.abc import Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd
from automation.release import (
    DEFAULT_RELEASE_DOWNLOAD_URL,
    UNIFIED_RELEASE_TAG_RE,
    verify_binary_release_exists,
)
from automation.stamp_version import extract_version_from_tag

# Go module proxy case-encodes uppercase 'A' in 'Archont561' as '!archont561'.
GO_PROXY_MODULE_PATH: str = "github.com/!archont561/playtest/go"


def _parse_go_release_version(release_tag: str) -> str:
    cleaned = release_tag.strip()
    if not UNIFIED_RELEASE_TAG_RE.match(cleaned):
        raise ValueError(
            f"Go module publication requires unified release tag vX.Y.Z, got {release_tag!r}"
        )
    return extract_version_from_tag(cleaned)


def _create_annotated_go_tag(root: Path, go_tag: str) -> None:
    existing = run_cmd(
        ["git", "rev-parse", "-q", "--verify", f"refs/tags/{go_tag}"], cwd=root, capture=True
    )
    if existing.returncode == 0:
        return
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
    tag_res = run_cmd(["git", "tag", "-a", go_tag, "-m", f"Go module release {go_tag}"], cwd=root)
    if tag_res.returncode != 0:
        raise RuntimeError(f"failed to create tag {go_tag}")


def preview_go_module_release(release_tag: str) -> str:
    """Log the planned Go module tag and proxy index URL without mutating remotes."""
    version = _parse_go_release_version(release_tag)
    go_tag = f"go/v{version}"
    url = f"https://proxy.golang.org/{GO_PROXY_MODULE_PATH}/@v/v{version}.info"
    emit_line(f"[publish-go] DRY-RUN: would verify v{version} binaries and push {go_tag}")
    emit_line(f"[publish-go] DRY-RUN: would GET {url}")
    return go_tag


def ensure_go_module_tag(
    release_tag: str,
    *,
    root: Path = REPO_ROOT,
    require_binary_release: bool = True,
    release_base_url: str = DEFAULT_RELEASE_DOWNLOAD_URL,
) -> str:
    """Verify GitHub Release `vX.Y.Z` exists, then create and push `go/vX.Y.Z`."""
    version = _parse_go_release_version(release_tag)
    go_tag = f"go/v{version}"
    if require_binary_release:
        verify_binary_release_exists(version, base_url=release_base_url)
    _create_annotated_go_tag(root, go_tag)
    push_res = run_cmd(["git", "push", "origin", f"refs/tags/{go_tag}"], cwd=root)
    if push_res.returncode != 0:
        raise RuntimeError(f"failed to push {go_tag} to origin")
    return go_tag


def request_go_proxy_index(version: str) -> None:
    """Ask proxy.golang.org to index `vX.Y.Z` for the `go/` subdirectory module."""
    url = f"https://proxy.golang.org/{GO_PROXY_MODULE_PATH}/@v/v{version}.info"
    req = urllib.request.Request(url, headers={"User-Agent": "playtest-release-automation"})
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            emit_line(f"[publish-go] Go proxy responded HTTP {resp.status} for v{version}")
    except (urllib.error.URLError, TimeoutError) as err:
        emit_err(f"[publish-go] WARN: Go proxy warm-up returned {err}; tag is pushed.")


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for Go module subdirectory tag publishing."""
    parser = argparse.ArgumentParser(description="Publish Go subdirectory module tag.")
    parser.add_argument("release_tag", help="Unified release tag (e.g. v0.2.0).")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--skip-release-check", action="store_true")
    parser.add_argument("--release-base-url", default=DEFAULT_RELEASE_DOWNLOAD_URL)
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    args = parser.parse_args(list(argv) if argv is not None else None)
    try:
        if args.dry_run:
            go_tag = preview_go_module_release(args.release_tag)
        else:
            version = extract_version_from_tag(args.release_tag)
            go_tag = ensure_go_module_tag(
                args.release_tag,
                root=args.root.resolve(),
                require_binary_release=not args.skip_release_check,
                release_base_url=args.release_base_url,
            )
            request_go_proxy_index(version)
    except (ValueError, RuntimeError) as err:
        emit_err(f"[publish-go] ERROR: {err}")
        return 1
    emit_line(f"[publish-go] OK: published {go_tag}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
