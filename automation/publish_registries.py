"""Ecosystem registry publication helpers invoked by .github/workflows/release.yml."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import time
import urllib.error
import urllib.request
from collections.abc import Callable, Sequence
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd
from automation.stage_assets import clean_staged_assets, stage_assets
from automation.stamp_version import extract_version_from_tag, stamp_target

CRATE_PUBLISH_ORDER: tuple[str, ...] = (
    "playtest-macros",
    "playtest-core",
    "playtest-assets",
    "playtest",
    "playtest-cli",
)
DEFAULT_CONDA_UPLOAD_URL: str = "https://prefix.dev/api/v1/upload/archont561/archont561"


def publish_crates(root: Path = REPO_ROOT) -> int:
    """Publish all five workspace crates to crates.io in topological dependency order."""
    try:
        stage_assets(root, build_generated=True)
        for crate in CRATE_PUBLISH_ORDER:
            res = run_cmd(
                ["cargo", "publish", "-p", crate, "--allow-dirty", "--locked"],
                cwd=root,
                timeout=600,
            )
            if res.returncode != 0:
                emit_err(f"[publish] ERROR: cargo publish failed for {crate}")
                return res.returncode
    finally:
        clean_staged_assets(root)
    emit_line("[publish] OK: published 5 crates to crates.io.")
    return 0


def _partition_npm_tarballs(tarballs: Sequence[Path]) -> tuple[list[Path], list[Path]]:
    platform_suffixes = ("linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64", "win32-x64")
    platform_pkgs: list[Path] = []
    wrapper_pkgs: list[Path] = []
    for pkg in sorted(tarballs):
        if any(f"playtest-{suf}-" in pkg.name for suf in platform_suffixes):
            platform_pkgs.append(pkg)
        else:
            wrapper_pkgs.append(pkg)
    return platform_pkgs, wrapper_pkgs


def publish_npm(root: Path = REPO_ROOT) -> int:
    """Publish platform npm tarballs first, then @archont561/playtest wrapper with provenance."""
    npm_dist = root / "npm-dist"
    search_dir = (
        npm_dist if npm_dist.is_dir() and list(npm_dist.glob("*.tgz")) else (root / "typescript")
    )
    tarballs = sorted(search_dir.glob("*.tgz"))
    if not tarballs:
        emit_err(f"[publish] ERROR: no *.tgz artifacts found in {search_dir}.")
        return 1
    platform_pkgs, wrapper_pkgs = _partition_npm_tarballs(tarballs)
    for pkg in [*platform_pkgs, *wrapper_pkgs]:
        res = run_cmd(
            ["npm", "publish", pkg.name, "--provenance", "--access", "public"],
            cwd=search_dir,
            timeout=300,
        )
        if res.returncode != 0:
            emit_err(f"[publish] ERROR: npm publish failed for {pkg.name}")
            return res.returncode
    emit_line(
        f"[publish] OK: published {len(platform_pkgs)} platform and "
        f"{len(wrapper_pkgs)} wrapper npm package(s)."
    )
    return 0


def _upload_conda_package(pkg: Path, upload_url: str, token: str) -> int:
    payload = pkg.read_bytes()
    sha256_hex = hashlib.sha256(payload).hexdigest()
    req = urllib.request.Request(
        upload_url,
        data=payload,
        method="POST",
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/octet-stream",
            "X-File-Name": pkg.name,
            "SHA-256": sha256_hex,
            "User-Agent": "playtest-release-workflow",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            emit_line(f"[publish] uploaded {pkg.name} -> HTTP {resp.status}")
    except (urllib.error.URLError, TimeoutError) as err:
        emit_err(f"[publish] ERROR: failed to upload {pkg.name}: {err}")
        return 1
    return 0


def publish_conda(root: Path = REPO_ROOT) -> int:
    """Upload Phase 1 `.conda` packages to prefix.dev channel `archont561/archont561`."""
    token = os.environ.get("PREFIX_DEV_TOKEN", "").strip()
    if not token:
        emit_line("::warning::Conda publish skipped; PREFIX_DEV_TOKEN is not configured.")
        return 0
    packages = sorted((root / "conda-dist").rglob("*.conda"))
    if not packages:
        emit_err("[publish] ERROR: no conda-dist/*.conda artifacts found to upload.")
        return 1
    upload_url = os.environ.get("PREFIX_DEV_UPLOAD_URL", DEFAULT_CONDA_UPLOAD_URL).strip()
    for pkg in packages:
        if _upload_conda_package(pkg, upload_url, token) != 0:
            return 1
    return 0


def publish_conan(root: Path = REPO_ROOT) -> int:
    """Create and upload Conan 2 packages when remote credentials are configured."""
    remote_url = os.environ.get("CONAN_REMOTE_URL", "").strip()
    username = os.environ.get("CONAN_USERNAME", "").strip()
    password = os.environ.get("CONAN_PASSWORD", "").strip()
    if not (remote_url and username and password):
        emit_line("::warning::Conan publish skipped; credentials are not configured.")
        return 0

    version = extract_version_from_tag(os.environ["RELEASE_TAG"])
    setup_cmds: tuple[Sequence[str], ...] = (
        ("conan", "profile", "detect", "--force"),
        ("conan", "remote", "add", "playtest", remote_url, "--force"),
        ("conan", "remote", "login", "playtest", username, "--password", password),
    )
    for cmd in setup_cmds:
        if run_cmd(cmd, cwd=root, timeout=180).returncode != 0:
            return 1

    for subdir, pkg_name in (("cpp", "playtest-cpp"), ("c", "playtest-c")):
        if os.environ.get(f"ENABLE_{subdir.upper()}", "true") != "true":
            continue
        create_cmd = ["conan", "create", subdir, f"--version={version}", "-s", "build_type=Release"]
        upload_cmd = ["conan", "upload", f"{pkg_name}/{version}", "--remote=playtest", "--confirm"]
        if run_cmd(create_cmd, cwd=root).returncode != 0:
            return 1
        if run_cmd(upload_cmd, cwd=root).returncode != 0:
            return 1
    return 0


def publish_nuget(root: Path = REPO_ROOT) -> int:
    """Push C# .nupkg artifacts to NuGet.org when NUGET_API_KEY is configured."""
    api_key = os.environ.get("NUGET_API_KEY", "").strip()
    if not api_key:
        emit_line("::warning::NuGet publish skipped; NUGET_API_KEY is not configured.")
        return 0
    nupkgs = sorted((root / "csharp" / "dist").glob("*.nupkg"))
    if not nupkgs:
        emit_err("[publish] ERROR: no csharp/dist/*.nupkg artifacts found.")
        return 1
    for pkg in nupkgs:
        res = run_cmd(
            [
                "dotnet",
                "nuget",
                "push",
                str(pkg),
                "--api-key",
                api_key,
                "--source",
                "https://api.nuget.org/v3/index.json",
                "--skip-duplicate",
            ],
            cwd=root,
            timeout=300,
        )
        if res.returncode != 0:
            return res.returncode
    return 0


def _maven_set_version_cmd(version: str) -> list[str]:
    return [
        "mvn",
        "--batch-mode",
        "org.codehaus.mojo:versions-maven-plugin:2.18.0:set",
        f"-DnewVersion={version}",
        "-DgenerateBackupPoms=false",
    ]


def publish_maven(root: Path = REPO_ROOT) -> int:
    """Stamp Java release version and deploy signed artifacts to Maven Central."""
    required = (
        "CENTRAL_USERNAME",
        "CENTRAL_PASSWORD",
        "MAVEN_GPG_PRIVATE_KEY",
        "MAVEN_GPG_PASSPHRASE",
    )
    if any(not os.environ.get(k, "").strip() for k in required):
        emit_line("::warning::Maven Central publish skipped; credentials are not configured.")
        return 0
    version = stamp_target("java", os.environ["RELEASE_TAG"], root=root)
    passphrase = os.environ["MAVEN_GPG_PASSPHRASE"]
    java_dir = root / "java"
    set_ver = run_cmd(_maven_set_version_cmd(version), cwd=java_dir)
    if set_ver.returncode != 0:
        return set_ver.returncode
    deploy_cmd = [
        "mvn",
        "--batch-mode",
        "--activate-profiles",
        "release",
        "-DskipTests",
        f"-Dgpg.passphrase={passphrase}",
        "deploy",
    ]
    return run_cmd(deploy_cmd, cwd=java_dir).returncode


def check_cran(root: Path = REPO_ROOT) -> int:
    """Verify the built R source tarball with `R CMD check --as-cran --no-manual`."""
    packages = sorted((root / "r" / "dist").glob("*.tar.gz"))
    if len(packages) != 1:
        emit_err(f"[publish] ERROR: expected 1 R tarball in r/dist, found {len(packages)}")
        return 1
    return run_cmd(
        ["R", "CMD", "check", "--as-cran", "--no-manual", str(packages[0])], cwd=root
    ).returncode


def _fetch_runiverse_pkg(req: urllib.request.Request) -> dict[str, object]:
    with urllib.request.urlopen(req, timeout=10) as resp:
        raw_bytes = resp.read()
    parsed = json.loads(raw_bytes.decode("utf-8"))
    return dict(parsed) if isinstance(parsed, dict) else {}


def _runiverse_poll_config() -> tuple[int, int]:
    max_attempts = int(os.environ.get("RUNIVERSE_MAX_ATTEMPTS", "60"))
    sleep_seconds = int(os.environ.get("RUNIVERSE_SLEEP_SECONDS", "60"))
    return max_attempts, sleep_seconds


def _runiverse_ready(req: urllib.request.Request, version: str) -> bool:
    pkg = _fetch_runiverse_pkg(req)
    return str(pkg.get("Version", "")) == version and pkg.get("_status") == "success"


def wait_runiverse(_root: Path = REPO_ROOT) -> int:
    """Poll the R-universe package API until the released version reports success."""
    raw_owner = os.environ.get("RUNIVERSE_OWNER", "archont561")
    owner = raw_owner.strip().lower()
    raw_version = extract_version_from_tag(os.environ["RELEASE_TAG"])
    version = raw_version.split("-", 1)[0]
    url = f"https://{owner}.r-universe.dev/api/packages/playtest"
    req = urllib.request.Request(url, headers={"User-Agent": "playtest-release-workflow"})
    max_attempts, sleep_seconds = _runiverse_poll_config()
    for attempt in range(max_attempts):
        try:
            if _runiverse_ready(req, version):
                emit_line(f"[publish] R-universe indexed playtest {version}: {url}")
                return 0
        except (urllib.error.URLError, TimeoutError, ValueError) as err:
            emit_line(f"[publish] waiting for R-universe ({attempt + 1}/{max_attempts}): {err}")
        if attempt + 1 < max_attempts:
            time.sleep(sleep_seconds)
    emit_err(f"[publish] ERROR: R-universe did not publish {version} in time: {url}")
    return 1


PUBLISH_ACTIONS: dict[str, Callable[[Path], int]] = {
    "crates": publish_crates,
    "npm": publish_npm,
    "conda": publish_conda,
    "conan": publish_conan,
    "nuget": publish_nuget,
    "maven": publish_maven,
    "check-cran": check_cran,
    "wait-runiverse": wait_runiverse,
}


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for registry publication steps."""
    parser = argparse.ArgumentParser(description="Execute a registry publish action.")
    parser.add_argument("action", choices=tuple(PUBLISH_ACTIONS.keys()))
    parser.add_argument("--root", type=Path, default=REPO_ROOT)
    args = parser.parse_args(list(argv) if argv is not None else None)
    return PUBLISH_ACTIONS[args.action](args.root.resolve())


if __name__ == "__main__":
    raise SystemExit(main())
