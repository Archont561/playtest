#!/usr/bin/env python3
"""Publish the Go module for a release tag.

Go resolves a module that lives in a repository subdirectory only from tags named
"<subdirectory>/vX.Y.Z". The module in go/ therefore needs a go/vX.Y.Z tag; a general
vX.Y.Z release tag does not version it. This script creates that tag at the release
commit unless the version is already published, then asks the Go module proxy for the
version so that pkg.go.dev indexes it.

Run it from a checkout of the release tag.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULE_DIR = "go"
PROXY_ATTEMPTS = 5
PROXY_RETRY_SECONDS = 15


def git(*args: str) -> str:
    """Run git in the repository and return its stdout; exit with git's message on failure."""
    result = subprocess.run(
        ["git", *args], cwd=ROOT, capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        raise SystemExit(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def go_version(release_tag: str) -> str:
    """Return the vX.Y.Z version carried by a release tag such as v0.2.0 or go/v0.2.0."""
    match = re.fullmatch(
        r"(?:.*?[/-])?(v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)", release_tag
    )
    if not match:
        raise SystemExit(
            f"Release tag '{release_tag}' does not end in a version Go accepts "
            "(vX.Y.Z or vX.Y.Z-prerelease)"
        )
    return match.group(1)


def module_path() -> str:
    """Return the module path declared in go/go.mod."""
    go_mod = ROOT / MODULE_DIR / "go.mod"
    match = re.search(
        r"^module\s+(\S+)", go_mod.read_text(encoding="utf-8"), flags=re.MULTILINE
    )
    if not match:
        raise SystemExit(f"No module directive found in {go_mod}")
    return match.group(1).strip('"')


def remote_tag_commit(remote: str, tag: str) -> str | None:
    """Return the commit a tag points at on the remote, or None when the remote lacks it."""
    ref = f"refs/tags/{tag}"
    lines = git("ls-remote", remote, ref, f"{ref}^{{}}").splitlines()
    commits = {name: sha for sha, name in (line.split("\t") for line in lines)}
    # An annotated tag lists its commit under the peeled "^{}" name.
    return commits.get(f"{ref}^{{}}") or commits.get(ref)


def same_module_content(remote: str, go_tag: str, existing: str, commit: str) -> bool:
    """Tell whether go_tag, which sits on another commit, already publishes this go/ tree."""
    shallow = git("rev-parse", "--is-shallow-repository") == "true"
    depth = ["--depth=1"] if shallow else []
    git("fetch", "--quiet", "--no-tags", *depth, remote, f"refs/tags/{go_tag}")
    trees = [git("rev-parse", f"{sha}:{MODULE_DIR}") for sha in (existing, commit)]
    return trees[0] == trees[1]


def ensure_tag(remote: str, release_tag: str, go_tag: str) -> None:
    """Publish the release commit's go/ tree as go_tag; never move an existing tag."""
    commit = remote_tag_commit(remote, release_tag)
    if commit is None:
        raise SystemExit(f"Release tag '{release_tag}' does not exist on {remote}")
    head = git("rev-parse", "HEAD")
    if head != commit:
        raise SystemExit(
            f"HEAD is {head}, but '{release_tag}' is {commit} on {remote}; "
            "check out the release tag first"
        )

    existing = remote_tag_commit(remote, go_tag)
    if existing is None:
        git("push", remote, f"{commit}:refs/tags/{go_tag}")
        print(f"Created {go_tag} at {commit}")
    elif existing == commit:
        print(f"{go_tag} already points at {commit}")
    elif same_module_content(remote, go_tag, existing, commit):
        # An earlier Go-only release took this version, and go/ has not changed since.
        print(
            f"{go_tag} at {existing} already publishes the {MODULE_DIR}/ tree of {commit}"
        )
    else:
        raise SystemExit(
            f"{go_tag} at {existing} already publishes a different {MODULE_DIR}/ tree, and Go "
            f"module versions are immutable. Cut a new release version to publish the "
            f"{MODULE_DIR}/ changes in '{release_tag}' ({commit})."
        )


def escape(text: str) -> str:
    """Case-encode a module path or version for a proxy URL: 'A' becomes '!a'."""
    return re.sub(r"[A-Z]", lambda match: "!" + match.group().lower(), text)


def request_indexing(proxy: str, module: str, version: str) -> None:
    """Ask the proxy for the version, which makes it fetch the tag and feeds pkg.go.dev."""
    url = f"{proxy.rstrip('/')}/{escape(module)}/@v/{escape(version)}.info"
    for attempt in range(1, PROXY_ATTEMPTS + 1):
        try:
            with urllib.request.urlopen(url, timeout=30) as response:
                print(
                    f"Go proxy serves {module}@{version}: {response.read().decode().strip()}"
                )
                return
        except urllib.error.HTTPError as error:
            reply = (
                f"HTTP {error.code}: {error.read().decode(errors='replace').strip()}"
            )
        except OSError as error:
            reply = str(error)
        print(
            f"Go proxy attempt {attempt}/{PROXY_ATTEMPTS} for {url} failed: {reply}",
            flush=True,
        )
        if attempt < PROXY_ATTEMPTS:
            time.sleep(PROXY_RETRY_SECONDS)
    print(
        f"::warning::The Go proxy did not confirm {module}@{version}. "
        f"The Git tag is pushed; request {url} again to index it."
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "release_tag", help="Release tag being published, e.g. v0.2.0 or go/v0.2.0"
    )
    parser.add_argument(
        "--remote", default="origin", help="Git remote that holds the release tag"
    )
    parser.add_argument(
        "--proxy", default="https://proxy.golang.org", help="Go module proxy"
    )
    args = parser.parse_args()

    version = go_version(args.release_tag)
    module = module_path()
    ensure_tag(args.remote, args.release_tag, f"{MODULE_DIR}/{version}")
    request_indexing(args.proxy, module, version)


if __name__ == "__main__":
    main()
