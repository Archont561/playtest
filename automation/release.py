"""Unified SemVer release planning, milestone tag parsing, tag cutting, and changelog generation."""

from __future__ import annotations

import argparse
import os
import re
import tomllib
from collections.abc import Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd, write_github_output
from automation.stamp_version import SEMVER_RE, extract_version_from_tag

MILESTONE_TAG_RE = re.compile(
    r"^(?:milestone-v|milestone/M\d+-v|milestone/v|m-v)(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$"
)
UNIFIED_RELEASE_TAG_RE = re.compile(r"^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$")
GO_RELEASE_TAG_RE = re.compile(r"^go/v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$")
CRAN_RELEASE_TAG_RE = re.compile(r"^(?:v|r[-/]v|cran[-/]v)(\d+\.\d+\.\d+)$")
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
    """Validate a release tag and return GitHub Actions planning outputs."""
    tag = raw_tag.strip()
    if parse_milestone_tag(tag) is not None:
        raise ValueError(f"milestone trigger tag {tag!r} must be handled by auto-release.yml")

    unified = UNIFIED_RELEASE_TAG_RE.match(tag)
    if unified:
        outputs = {"valid": "true", "tag_name": tag, "version": unified.group(1)}
        outputs.update(dict.fromkeys(RELEASE_TARGETS, "true"))
        return outputs

    go_only = GO_RELEASE_TAG_RE.match(tag)
    if go_only:
        outputs = {"valid": "true", "tag_name": tag, "version": go_only.group(1)}
        outputs.update(
            {target: ("true" if target == "go" else "false") for target in RELEASE_TARGETS}
        )
        return outputs

    raise ValueError(f"unsupported release tag {tag!r}; expected vX.Y.Z (or go/vX.Y.Z)")


def _bump_semver(current: str, commits: Sequence[str]) -> tuple[str, bool]:
    base = current.split("-", 1)[0]
    major, minor, patch = (int(part) for part in base.split("."))
    has_breaking = False
    has_feat = False
    has_fix = False
    for subject in commits:
        header = subject.strip()
        if "!:" in header or "BREAKING CHANGE" in header:
            has_breaking = True
        elif header.startswith("feat"):
            has_feat = True
        elif header.startswith(("fix", "perf")):
            has_fix = True
    if not (has_breaking or has_feat or has_fix):
        return current, False
    if has_breaking:
        return (f"0.{minor + 1}.0" if major == 0 else f"{major + 1}.0.0"), True
    if has_feat:
        return (f"0.{minor}.{patch + 1}" if major == 0 else f"{major}.{minor + 1}.0"), True
    return f"{major}.{minor}.{patch + 1}", True


def detect_next_release(
    root: Path = REPO_ROOT,
    *,
    ref_name: str = "",
    override_version: str = "",
) -> dict[str, str]:
    """Determine whether a unified release tag `vX.Y.Z` should be cut."""
    if override_version.strip():
        ver = extract_version_from_tag(override_version)
        return {"should_release": "true", "version": ver, "tag_name": f"v{ver}"}

    milestone_ver = parse_milestone_tag(ref_name)
    if milestone_ver is not None:
        return {"should_release": "true", "version": milestone_ver, "tag_name": f"v{milestone_ver}"}

    cargo_toml = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    current_ver = str(cargo_toml["workspace"]["package"]["version"])
    last_tag = run_cmd(
        ["git", "describe", "--tags", "--match", "v[0-9]*", "--abbrev=0"],
        cwd=root,
        capture=True,
    )
    rev_range = f"{last_tag.stdout.strip()}..HEAD" if last_tag.returncode == 0 else "HEAD"
    log_res = run_cmd(["git", "log", "--format=%s", rev_range], cwd=root, capture=True)
    subjects = [line for line in log_res.stdout.splitlines() if line.strip()]
    if last_tag.returncode != 0:
        has_releasable = any(s.startswith(("feat", "fix", "perf")) or "!:" in s for s in subjects)
        return {
            "should_release": "true" if has_releasable else "false",
            "version": current_ver,
            "tag_name": f"v{current_ver}",
        }
    next_ver, bump = _bump_semver(extract_version_from_tag(last_tag.stdout.strip()), subjects)
    return {
        "should_release": "true" if bump else "false",
        "version": next_ver,
        "tag_name": f"v{next_ver}",
    }


def cut_or_summarize_tag(tag_name: str, *, root: Path = REPO_ROOT, push: bool = False) -> int:
    """Push annotated `vX.Y.Z` when authorized, otherwise write manual tag summary."""
    if not UNIFIED_RELEASE_TAG_RE.match(tag_name):
        emit_err(f"[release] ERROR: invalid unified release tag {tag_name!r}")
        return 1
    if not push:
        summary_path = os.environ.get("GITHUB_STEP_SUMMARY", "").strip()
        note = (
            f"### Release Tag Ready: `{tag_name}`\n\n"
            "`RELEASE_PUSH_TOKEN` is not configured (default `GITHUB_TOKEN` tag pushes do not "
            "trigger `release.yml`). Push the verified tag manually:\n\n"
            f"```bash\ngit tag -a {tag_name} -m 'Release {tag_name}'\n"
            f"git push origin {tag_name}\n```\n"
        )
        if summary_path:
            Path(summary_path).write_text(note, encoding="utf-8")
        emit_line(note.strip())
        return 0

    run_cmd(["git", "config", "user.name", "github-actions[bot]"], cwd=root)
    run_cmd(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
    )
    if (
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


def validate_cran_tag(raw_tag: str) -> int:
    """Verify that `raw_tag` is a non-prerelease R/unified SemVer tag for CRAN."""
    match = CRAN_RELEASE_TAG_RE.match(raw_tag.strip())
    if not match or not SEMVER_RE.match(match.group(1)):
        emit_err(f"[release] ERROR: unsupported CRAN release tag {raw_tag!r} (expected vX.Y.Z).")
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
    has_token = bool(args.push or os.environ.get("HAVE_RELEASE_PUSH_TOKEN") == "true")
    return cut_or_summarize_tag(args.tag, root=root, push=has_token)


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
    return validate_cran_tag(args.tag)


if __name__ == "__main__":
    raise SystemExit(main())
