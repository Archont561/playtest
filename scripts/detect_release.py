#!/usr/bin/env python3
"""
detect_release.py

Inspects git history and conventional commits to:
1. Detect which packages/directories have changed since the last release tag.
2. Determine target scope: 'all' (general release) vs. single scoped package.
3. Calculate next SemVer version via convco.
4. Formulate the proper release tag (e.g. 'v0.2.0' or 'python-v0.2.0').
"""

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path


def run_cmd(cmd: list[str], cwd: Path | None = None) -> tuple[int, str, str]:
    """Execute a subprocess command returning exit code, stdout, and stderr."""
    res = subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return res.returncode, res.stdout.strip(), res.stderr.strip()


def get_git_root() -> Path:
    """Return the repository root directory."""
    script_dir = Path(__file__).resolve().parent
    repo_root = script_dir.parent
    return repo_root


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Detect monorepo changes and calculate release tags using convco."
    )
    parser.add_argument(
        "--scope",
        choices=["all", "rust", "python", "typescript", "go", "java", "cpp", "r", "c", "csharp"],
        help="Force target scope",
    )
    parser.add_argument(
        "--bump",
        help="Force version bump: patch, minor, major, or explicit X.Y.Z",
    )
    parser.add_argument(
        "--milestone-tag",
        help="Milestone tag that triggered the run (e.g. milestone-v0.2.0)",
    )
    parser.add_argument(
        "--github-output",
        help="Path to write GitHub Actions output key-value pairs",
    )
    return parser.parse_args()


def detect_base_reference(repo_root: Path) -> tuple[str, str]:
    """Find the latest general release tag and root commit."""
    code, out, _ = run_cmd(
        ["git", "tag", "-l", "v[0-9]*", "--sort=-v:refname"], cwd=repo_root
    )
    latest_global_tag = ""
    if code == 0 and out:
        tags = [t.strip() for t in out.splitlines() if t.strip()]
        if tags:
            latest_global_tag = tags[0]

    code, out, _ = run_cmd(
        ["git", "rev-list", "--max-parents=0", "HEAD"], cwd=repo_root
    )
    root_commit = out.splitlines()[0].strip() if code == 0 and out else ""

    base_ref = latest_global_tag or root_commit
    return base_ref, latest_global_tag


def categorize_changed_files(repo_root: Path, base_ref: str) -> dict[str, bool]:
    """Inspect git diff and classify changed files into target categories."""
    flags = {
        "rust": False,
        "python": False,
        "typescript": False,
        "cpp": False,
        "c": False,
        "csharp": False,
        "go": False,
        "java": False,
        "r": False,
        "core": False,
    }

    if not base_ref:
        return flags

    code, out, _ = run_cmd(
        ["git", "diff", "--name-only", f"{base_ref}..HEAD"], cwd=repo_root
    )
    if code != 0 or not out:
        return flags

    changed_files = [f.strip() for f in out.splitlines() if f.strip()]

    for f in changed_files:
        # Ignore docs, backlog, devcontainer, and agents
        if (
            f.startswith("backlog/")
            or f.startswith(".agents/")
            or f.startswith(".devcontainer/")
            or f.endswith(".md")
            or f == ".codecov.yml"
        ):
            continue

        if f.startswith("crates/") or f in ["Cargo.toml", "Cargo.lock"]:
            flags["rust"] = True
        elif f.startswith("python/"):
            flags["python"] = True
        elif f.startswith("typescript/"):
            flags["typescript"] = True
        elif f.startswith("cpp/"):
            flags["cpp"] = True
        elif f.startswith("c/"):
            flags["c"] = True
        elif f.startswith("csharp/"):
            flags["csharp"] = True
        elif f.startswith("go/"):
            flags["go"] = True
        elif f.startswith("java/"):
            flags["java"] = True
        elif f.startswith("r/"):
            flags["r"] = True
        else:
            # Root build files, workflows, global configs
            flags["core"] = True

    return flags


def resolve_scope(forced_scope: str | None, flags: dict[str, bool]) -> str:
    """Determine whether release is general ('all'), scoped to a package, or 'none'."""
    if forced_scope:
        return forced_scope

    targets = [
        "rust",
        "python",
        "typescript",
        "cpp",
        "c",
        "csharp",
        "go",
        "java",
        "r",
    ]
    active = [t for t in targets if flags.get(t, False)]

    if flags.get("core", False) or len(active) > 1:
        return "all"
    elif len(active) == 1:
        return active[0]
    return "none"


def get_scope_mapping(scope: str) -> tuple[str, str]:
    """Return prefix and path filter for convco."""
    mapping = {
        "all": ("v", ""),
        "rust": ("rust-v", "crates"),
        "python": ("python-v", "python"),
        "typescript": ("ts-v", "typescript"),
        "cpp": ("cpp-v", "cpp"),
        "c": ("c-v", "c"),
        "csharp": ("csharp-v", "csharp"),
        "go": ("go-v", "go"),
        "java": ("java-v", "java"),
        "r": ("r-v", "r"),
        "none": ("", ""),
    }
    return mapping.get(scope, ("", ""))


def calculate_version(
    repo_root: Path,
    scope: str,
    prefix: str,
    path_filter: str,
    forced_bump: str | None,
    milestone_tag: str | None,
) -> tuple[bool, str, str]:
    """Calculate next version using convco or explicit overrides."""
    if scope == "none":
        return False, "", ""

    # If milestone tag provided (e.g. milestone-v0.2.0 or milestone/v0.2.0)
    if milestone_tag:
        match = re.search(r"v([0-9]+\.[0-9]+\.[0-9]+.*)", milestone_tag)
        if match:
            ver = match.group(1)
            tag = f"{prefix}{ver}"
            return True, ver, tag

    # Explicit bump override
    if forced_bump:
        if re.match(r"^[0-9]+\.[0-9]+\.[0-9]+", forced_bump):
            ver = forced_bump
            return True, ver, f"{prefix}{ver}"
        else:
            bump_flag = f"--{forced_bump}"
            cmd = [
                "pixi",
                "run",
                "convco",
                "version",
                "--bump",
                bump_flag,
                "--prefix",
                prefix,
                "--treat-major-zero-as-stable",
            ]
            if path_filter:
                cmd.extend(["--paths", path_filter])
            code, out, _ = run_cmd(cmd, cwd=repo_root)
            if code == 0 and out:
                return True, out, f"{prefix}{out}"
            return False, "", ""

    # Check if conventional commits warrant a bump
    label_cmd = [
        "pixi",
        "run",
        "convco",
        "version",
        "--bump",
        "--label",
        "--prefix",
        prefix,
    ]
    if path_filter:
        label_cmd.extend(["--paths", path_filter])

    code, label, _ = run_cmd(label_cmd, cwd=repo_root)
    if code != 0 or label in ["release", ""] or not label:
        return False, "", ""

    # Perform actual version calculation
    ver_cmd = [
        "pixi",
        "run",
        "convco",
        "version",
        "--bump",
        "--prefix",
        prefix,
        "--treat-major-zero-as-stable",
    ]
    if path_filter:
        ver_cmd.extend(["--paths", path_filter])

    code, ver, _ = run_cmd(ver_cmd, cwd=repo_root)
    if code == 0 and ver:
        return True, ver, f"{prefix}{ver}"

    return False, "", ""


def main() -> int:
    args = parse_args()
    repo_root = get_git_root()

    base_ref, latest_global_tag = detect_base_reference(repo_root)
    if not base_ref:
        print("::warning:: No git commits found in repository.", file=sys.stderr)
        if args.github_output:
            with open(args.github_output, "a") as f:
                f.write("release_needed=false\nscope=none\nversion=\ntag=\n")
        return 0

    flags = categorize_changed_files(repo_root, base_ref)
    scope = resolve_scope(args.scope, flags)
    prefix, path_filter = get_scope_mapping(scope)

    release_needed, version, tag = calculate_version(
        repo_root,
        scope,
        prefix,
        path_filter,
        args.bump,
        args.milestone_tag,
    )

    print("==================================================")
    print(" Playtest Release Detection Summary (Python)")
    print("==================================================")
    print(f" Detected Scope  : {scope}")
    print(f" Release Needed  : {str(release_needed).lower()}")
    print(f" Target Version  : {version or 'none'}")
    print(f" Release Tag     : {tag or 'none'}")
    print(
        f" Changed Targets : rust={flags['rust']} py={flags['python']} "
        f"ts={flags['typescript']} cpp={flags['cpp']} go={flags['go']} "
        f"java={flags['java']} r={flags['r']} core={flags['core']}"
    )
    print("==================================================")

    if args.github_output:
        with open(args.github_output, "a") as f:
            f.write(f"release_needed={str(release_needed).lower()}\n")
            f.write(f"scope={scope}\n")
            f.write(f"version={version}\n")
            f.write(f"tag={tag}\n")

    return 0


if __name__ == "__main__":
    sys.exit(main())
