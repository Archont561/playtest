"""Unified SemVer release planning, milestone tag parsing, tag cutting, and CRAN gating."""

from __future__ import annotations

import argparse
import os
import re
import tomllib
import urllib.error
import urllib.request
from collections.abc import Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd, write_github_output
from automation.stamp_version import SEMVER_RE, extract_version_from_tag

MILESTONE_TAG_RE = re.compile(
    r"^(?:milestone-v|milestone/M\d+-v|milestone/v|m-v)(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$"
)
UNIFIED_RELEASE_TAG_RE = re.compile(r"^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$")
STABLE_UNIFIED_TAG_RE = re.compile(r"^v(\d+\.\d+\.\d+)$")
RETIRED_SCOPED_TAG_RE = re.compile(r"^(?:go/v|[a-z]+[-/]v)(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$")
DEFAULT_RELEASE_DOWNLOAD_URL: str = "https://github.com/Archont561/playtest/releases/download"
NON_RELEASE_PATH_PREFIXES: tuple[str, ...] = ("backlog/", ".agents/", ".devcontainer/")
RELEASE_TARGETS: tuple[str, ...] = (
    "rust",
    "python",
    "typescript",
    "go",
    "java",
    "cpp",
    "c",
    "csharp",
    "r",
)


def parse_milestone_tag(raw_tag: str) -> str | None:
    """Extract target SemVer from `milestone-v*`, `milestone/*`, or `m-v*` tags."""
    match = MILESTONE_TAG_RE.match(raw_tag.strip())
    return match.group(1) if match else None


def plan_release_tag(raw_tag: str) -> dict[str, str]:
    """Validate a unified `vX.Y.Z` release tag and return GitHub Actions outputs."""
    tag = raw_tag.strip()
    if parse_milestone_tag(tag) is not None:
        raise ValueError(f"milestone trigger tag {tag!r} must be handled by auto-release.yml")
    if RETIRED_SCOPED_TAG_RE.match(tag):
        raise ValueError(
            f"scoped release tag {tag!r} is retired (ADR 0010 / Spec 0012 §8); "
            "cut unified `vX.Y.Z` so all SDKs and CLI binaries share one version"
        )
    unified = UNIFIED_RELEASE_TAG_RE.match(tag)
    if unified:
        outputs = {"valid": "true", "tag_name": tag, "version": unified.group(1)}
        outputs.update(dict.fromkeys(RELEASE_TARGETS, "true"))
        return outputs
    raise ValueError(f"unsupported release tag {tag!r}; expected unified vX.Y.Z")


def _is_releasable_path(rel_path: str) -> bool:
    norm = rel_path.strip().replace("\\", "/")
    if not norm or norm == ".codecov.yml" or norm.endswith(".md"):
        return False
    return not any(norm.startswith(prefix) for prefix in NON_RELEASE_PATH_PREFIXES)


def _classify_commit_impact(commits: Sequence[str]) -> str | None:
    impact: str | None = None
    for subject in commits:
        header = subject.strip()
        if "!:" in header or "BREAKING CHANGE" in header:
            return "breaking"
        if header.startswith("feat"):
            impact = "feat"
        elif impact is None and header.startswith(("fix", "perf")):
            impact = "fix"
    return impact


def _bump_semver(current: str, commits: Sequence[str]) -> tuple[str, bool]:
    impact = _classify_commit_impact(commits)
    if impact is None:
        return current, False
    major, minor, patch = (int(part) for part in current.split("-", 1)[0].split("."))
    if impact == "breaking":
        return (f"0.{minor + 1}.0" if major == 0 else f"{major + 1}.0.0"), True
    if impact == "feat":
        return (f"0.{minor}.{patch + 1}" if major == 0 else f"{major}.{minor + 1}.0"), True
    return f"{major}.{minor}.{patch + 1}", True


def _release_payload(version: str, *, should_release: bool) -> dict[str, str]:
    return {
        "should_release": "true" if should_release else "false",
        "version": version,
        "tag_name": f"v{version}",
    }


def _detect_from_git_history(root: Path, current_ver: str) -> dict[str, str]:
    last_tag = run_cmd(
        ["git", "describe", "--tags", "--match", "v[0-9]*", "--abbrev=0"],
        cwd=root,
        capture=True,
    )
    if last_tag.returncode != 0:
        return _release_payload(current_ver, should_release=False)
    base_tag = last_tag.stdout.strip()
    base_ver = extract_version_from_tag(base_tag)
    diff_res = run_cmd(["git", "diff", "--name-only", f"{base_tag}..HEAD"], cwd=root, capture=True)
    if not any(_is_releasable_path(line) for line in diff_res.stdout.splitlines()):
        return _release_payload(base_ver, should_release=False)
    log_res = run_cmd(["git", "log", "--format=%s", f"{base_tag}..HEAD"], cwd=root, capture=True)
    subjects = [line for line in log_res.stdout.splitlines() if line.strip()]
    next_ver, bump = _bump_semver(base_ver, subjects)
    return _release_payload(next_ver, should_release=bump)


def detect_next_release(
    root: Path = REPO_ROOT,
    *,
    ref_name: str = "",
    override_version: str = "",
) -> dict[str, str]:
    """Determine whether a unified release tag `vX.Y.Z` should be cut."""
    if override_version.strip():
        return _release_payload(extract_version_from_tag(override_version), should_release=True)
    milestone_ver = parse_milestone_tag(ref_name)
    if milestone_ver is not None:
        return _release_payload(milestone_ver, should_release=True)
    cargo_toml = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    current_ver = str(cargo_toml["workspace"]["package"]["version"])
    return _detect_from_git_history(root, current_ver)


def verify_binary_release_exists(
    version_or_tag: str,
    *,
    base_url: str = DEFAULT_RELEASE_DOWNLOAD_URL,
) -> None:
    """Verify GitHub Release `vX.Y.Z` has published `sha256.sum` for all CLI archives."""
    version = extract_version_from_tag(version_or_tag)
    clean_base = base_url.rstrip("/")
    sum_url = f"{clean_base}/v{version}/sha256.sum"
    req = urllib.request.Request(sum_url, headers={"User-Agent": "playtest-release-guard"})
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            body = resp.read().decode("utf-8", errors="replace")
            status = resp.status
    except (urllib.error.URLError, TimeoutError) as err:
        msg = f"binary release v{version} is not published at {sum_url}: {err}"
        raise RuntimeError(msg) from err
    if status != 200 or "playtest-cli-" not in body:
        raise RuntimeError(
            f"binary release v{version} at {sum_url} is missing verified CLI checksums"
        )


def _write_dry_run_tag_summary(tag_name: str) -> int:
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY", "").strip()
    note = (
        f"### Release Tag Ready: `{tag_name}`\n\n"
        "Dry-run completed without pushing. To cut and publish manually:\n\n"
        f"```bash\ngit tag -a {tag_name} -m 'Release {tag_name}'\n"
        f"git push origin {tag_name}\n```\n"
    )
    if summary_path:
        Path(summary_path).write_text(note, encoding="utf-8")
    emit_line(note.strip())
    return 0


def cut_or_summarize_tag(tag_name: str, *, root: Path = REPO_ROOT, push: bool = False) -> int:
    """Create and push annotated `vX.Y.Z` when `push=True`, otherwise write summary."""
    if not UNIFIED_RELEASE_TAG_RE.match(tag_name):
        emit_err(f"[release] ERROR: invalid unified release tag {tag_name!r}")
        return 1
    if not push:
        return _write_dry_run_tag_summary(tag_name)
    run_cmd(["git", "config", "user.name", "github-actions[bot]"], cwd=root)
    run_cmd(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
    )
    existing = run_cmd(
        ["git", "rev-parse", "-q", "--verify", f"refs/tags/{tag_name}"], cwd=root, capture=True
    )
    if existing.returncode != 0 and (
        run_cmd(["git", "tag", "-a", tag_name, "-m", f"Release {tag_name}"], cwd=root).returncode
        != 0
    ):
        return 1
    return run_cmd(["git", "push", "origin", f"refs/tags/{tag_name}"], cwd=root).returncode


def write_release_notes(tag_name: str, output_file: Path, *, root: Path = REPO_ROOT) -> int:
    """Generate `RELEASE_NOTES.md` via convco with deterministic fallback."""
    res = run_cmd(["convco", "changelog"], cwd=root, capture=True)
    body = (
        res.stdout.strip() if res.returncode == 0 and res.stdout.strip() else f"Release {tag_name}"
    )
    output_file.write_text(f"{body}\n", encoding="utf-8")
    emit_line(f"[release] wrote {output_file}")
    return 0


def validate_cran_tag(
    raw_tag: str,
    *,
    require_binary_release: bool = True,
    release_base_url: str = DEFAULT_RELEASE_DOWNLOAD_URL,
) -> int:
    """Verify `raw_tag` is unified `vX.Y.Z` and its GitHub Release binaries exist."""
    match = STABLE_UNIFIED_TAG_RE.match(raw_tag.strip())
    if not match or not SEMVER_RE.match(match.group(1)):
        emit_err(
            f"[release] ERROR: unsupported CRAN release tag {raw_tag!r} "
            "(expected unified non-prerelease vX.Y.Z)."
        )
        return 1
    if require_binary_release:
        try:
            verify_binary_release_exists(match.group(1), base_url=release_base_url)
        except RuntimeError as err:
            emit_err(f"[release] ERROR: {err}")
            return 1
    emit_line(f"[release] OK: CRAN release version {match.group(1)}")
    return 0


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Playtest release orchestration CLI.")
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    sub = parser.add_subparsers(dest="command", required=True)

    p_detect = sub.add_parser("detect", help="Detect next unified release version.")
    p_detect.add_argument("--ref-name", default=os.environ.get("GITHUB_REF_NAME", ""))
    p_detect.add_argument("--override-version", default="")

    p_plan = sub.add_parser("plan", help="Validate and plan a release tag.")
    p_plan.add_argument("--tag", default=os.environ.get("RELEASE_TAG", ""))

    p_cut = sub.add_parser("cut-tag", help="Push annotated release tag or write summary.")
    p_cut.add_argument("--tag", required=True)
    p_cut.add_argument("--push", action="store_true")

    p_notes = sub.add_parser("changelog", help="Write RELEASE_NOTES.md.")
    p_notes.add_argument("--tag", default=os.environ.get("RELEASE_TAG", "v0.1.0"))
    p_notes.add_argument("--output", type=Path, default=Path("RELEASE_NOTES.md"))

    p_cran = sub.add_parser("validate-cran-tag", help="Validate CRAN submission tag.")
    p_cran.add_argument("--tag", default=os.environ.get("RELEASE_TAG", ""))
    p_cran.add_argument("--skip-release-check", action="store_true")
    p_cran.add_argument("--release-base-url", default=DEFAULT_RELEASE_DOWNLOAD_URL)
    return parser


def _cmd_detect(args: argparse.Namespace, root: Path) -> int:
    try:
        outputs = detect_next_release(
            root,
            ref_name=args.ref_name,
            override_version=args.override_version,
        )
    except ValueError as err:
        emit_err(f"[release] ERROR: {err}")
        return 1
    write_github_output(outputs)
    for key, val in outputs.items():
        emit_line(f"{key}={val}")
    return 0


def _cmd_plan(args: argparse.Namespace, _root: Path) -> int:
    try:
        outputs = plan_release_tag(args.tag)
    except ValueError as err:
        emit_err(f"[release] ERROR: {err}")
        return 1
    write_github_output(outputs)
    for key, val in outputs.items():
        emit_line(f"{key}={val}")
    return 0


def _cmd_cut_tag(args: argparse.Namespace, root: Path) -> int:
    should_push = bool(args.push or os.environ.get("HAVE_RELEASE_PUSH_TOKEN") == "true")
    return cut_or_summarize_tag(args.tag, root=root, push=should_push)


def _cmd_changelog(args: argparse.Namespace, root: Path) -> int:
    out = args.output if args.output.is_absolute() else (root / args.output)
    return write_release_notes(args.tag, out, root=root)


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for release detection, planning, tag cutting, and notes."""
    args = _build_parser().parse_args(list(argv) if argv is not None else None)
    root = args.root.resolve()
    handlers = {
        "detect": _cmd_detect,
        "plan": _cmd_plan,
        "cut-tag": _cmd_cut_tag,
        "changelog": _cmd_changelog,
    }
    handler = handlers.get(args.command)
    if handler is not None:
        return handler(args, root)
    return validate_cran_tag(
        args.tag,
        require_binary_release=not args.skip_release_check,
        release_base_url=args.release_base_url,
    )


if __name__ == "__main__":
    raise SystemExit(main())
