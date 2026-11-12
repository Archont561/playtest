"""Unit tests for unified release planning, milestone gating, Go/CRAN guards, and stamping."""

from __future__ import annotations

import http.server
import shutil
import subprocess
import threading
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

import pytest

from automation import REPO_ROOT
from automation.publish_go import ensure_go_module_tag
from automation.release import (
    RELEASE_TARGETS,
    cut_or_summarize_tag,
    detect_next_release,
    parse_milestone_tag,
    plan_release_tag,
    validate_cran_tag,
)
from automation.stamp_version import (
    extract_version_from_tag,
    read_manifest_versions,
    stamp_target,
)


@contextmanager
def _serve_release_tree(files: dict[str, bytes]) -> Iterator[str]:
    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            payload = files.get(self.path)
            if payload is None:
                self.send_response(404)
                self.end_headers()
                return
            self.send_response(200)
            self.end_headers()
            self.wfile.write(payload)

        def log_message(self, _fmt: str, *_args: object) -> None:
            return

    server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        host, port = server.server_address[:2]
        yield f"http://{host!s}:{port}"
    finally:
        server.shutdown()
        server.server_close()


def _init_git_repo(repo: Path) -> None:
    repo.mkdir(parents=True, exist_ok=True)
    shutil.copy2(REPO_ROOT / "Cargo.toml", repo / "Cargo.toml")
    (repo / "go").mkdir(parents=True, exist_ok=True)
    (repo / "go/playtest.go").write_text("package playtest\n", encoding="utf-8")
    for cmd in (
        ["git", "init"],
        ["git", "config", "user.name", "Test"],
        ["git", "config", "user.email", "test@example.com"],
        ["git", "add", "."],
        ["git", "commit", "-m", "feat: initial workspace"],
    ):
        subprocess.run(cmd, cwd=repo, check=True, capture_output=True)


def test_parse_milestone_tags() -> None:
    assert parse_milestone_tag("milestone-v0.2.0") == "0.2.0"
    assert parse_milestone_tag("milestone/M2-v0.2.0") == "0.2.0"
    assert parse_milestone_tag("m-v1.0.0-rc.1") == "1.0.0-rc.1"
    assert parse_milestone_tag("v0.2.0") is None


def test_plan_unified_release_enables_all_targets_and_rejects_scoped() -> None:
    planned = plan_release_tag("v0.2.0")
    assert planned["valid"] == "true"
    assert planned["version"] == "0.2.0"
    for target in RELEASE_TARGETS:
        assert planned[target] == "true"

    for retired in ("go/v0.2.0", "python-v0.2.0", "rust-v0.2.0", "r-v0.2.0"):
        with pytest.raises(ValueError, match="scoped release tag .* is retired"):
            plan_release_tag(retired)


def test_plan_rejects_milestone_tags_to_prevent_accidental_release() -> None:
    with pytest.raises(ValueError, match="milestone trigger tag"):
        plan_release_tag("milestone-v0.2.0")
    with pytest.raises(ValueError, match="milestone trigger tag"):
        plan_release_tag("m-v0.2.0")


def test_detect_blocks_accidental_first_release_and_cuts_unified_for_single_lang(
    tmp_path: Path,
) -> None:
    repo = tmp_path / "repo"
    _init_git_repo(repo)

    # Without a prior vX.Y.Z tag, pushing to main must NOT auto-tag v0.1.0.
    first_push = detect_next_release(repo, ref_name="main")
    assert first_push == {"should_release": "false", "version": "0.1.0", "tag_name": "v0.1.0"}

    # Milestone tag cuts v0.1.0 explicitly.
    milestone = detect_next_release(repo, ref_name="milestone-v0.1.0")
    assert milestone == {"should_release": "true", "version": "0.1.0", "tag_name": "v0.1.0"}

    subprocess.run(["git", "tag", "v0.1.0"], cwd=repo, check=True)
    # A commit confined solely to `go/` produces a unified v0.1.1 tag, never go/v0.1.1.
    (repo / "go/playtest.go").write_text("package playtest // updated\n", encoding="utf-8")
    subprocess.run(["git", "add", "go/playtest.go"], cwd=repo, check=True)
    subprocess.run(
        ["git", "commit", "-m", "feat(go): add download wrapper helper"], cwd=repo, check=True
    )
    single_lang = detect_next_release(repo, ref_name="main")
    assert single_lang == {"should_release": "true", "version": "0.1.1", "tag_name": "v0.1.1"}


def test_detect_ignores_backlog_and_markdown_only_changes(tmp_path: Path) -> None:
    repo = tmp_path / "repo"
    _init_git_repo(repo)
    subprocess.run(["git", "tag", "v0.1.0"], cwd=repo, check=True)
    (repo / "backlog").mkdir()
    (repo / "backlog/tasks.yaml").write_text("tasks: []\n", encoding="utf-8")
    (repo / "NOTES.md").write_text("# Notes\n", encoding="utf-8")
    subprocess.run(["git", "add", "."], cwd=repo, check=True)
    subprocess.run(["git", "commit", "-m", "feat(docs): update notes"], cwd=repo, check=True)
    res = detect_next_release(repo, ref_name="main")
    assert res["should_release"] == "false"


def test_go_and_cran_refuse_publication_without_binary_release(tmp_path: Path) -> None:
    sum_payload = (
        b"2eeb0072a8fc11eb5a6c3549525db7983878e20c924935db3df3e29f9ee6083e  "
        b"playtest-cli-x86_64-unknown-linux-musl.tar.gz\n"
    )
    with _serve_release_tree({"/v0.2.0/sha256.sum": sum_payload}) as base_url:
        assert validate_cran_tag("v0.2.0", release_base_url=base_url) == 0
        assert validate_cran_tag("v0.9.0", release_base_url=base_url) == 1
        assert validate_cran_tag("r-v0.2.0", release_base_url=base_url) == 1
        assert validate_cran_tag("v0.2.0-rc.1", release_base_url=base_url) == 1

        origin = tmp_path / "origin.git"
        subprocess.run(["git", "init", "--bare", str(origin)], check=True, capture_output=True)
        repo = tmp_path / "work"
        _init_git_repo(repo)
        subprocess.run(["git", "remote", "add", "origin", str(origin)], cwd=repo, check=True)

        with pytest.raises(RuntimeError, match="binary release v0.9.0 is not published"):
            ensure_go_module_tag("v0.9.0", root=repo, release_base_url=base_url)
        with pytest.raises(ValueError, match="requires unified release tag"):
            ensure_go_module_tag("go/v0.2.0", root=repo, release_base_url=base_url)

        pushed = ensure_go_module_tag("v0.2.0", root=repo, release_base_url=base_url)
        assert pushed == "go/v0.2.0"


def test_cut_tag_without_push_writes_step_summary(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    summary = tmp_path / "summary.md"
    monkeypatch.setenv("GITHUB_STEP_SUMMARY", str(summary))
    rc = cut_or_summarize_tag("v0.2.0", root=tmp_path, push=False)
    assert rc == 0
    assert "v0.2.0" in summary.read_text(encoding="utf-8")


def _copy_stamp_fixture(dest: Path) -> None:
    rel_files = (
        "Cargo.toml",
        "package.json",
        "conda/pixi.toml",
        "python/pyproject.toml",
        "python/src/playtest/__init__.py",
        "typescript/package.json",
        "typescript/src/index.ts",
        "typescript/src/version.ts",
        "cpp/CMakeLists.txt",
        "cpp/conanfile.py",
        "cpp/include/playtest/playtest.hpp",
        "c/CMakeLists.txt",
        "c/conanfile.py",
        "c/include/playtest/playtest.h",
        "c/src/playtest.c",
        "csharp/src/Playtest/Playtest.csproj",
        "csharp/src/Playtest/Playtest.cs",
        "go/VERSION",
        "go/playtest.go",
        "java/pom.xml",
        "java/src/main/java/io/github/archont561/playtest/Playtest.java",
        "java/src/main/resources/io/github/archont561/playtest/version.properties",
        "r/DESCRIPTION",
        "r/R/version.R",
    )
    for rel in rel_files:
        target = dest / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO_ROOT / rel, target)


def test_stamp_all_updates_manifests_only_and_autodiscovers(tmp_path: Path) -> None:
    _copy_stamp_fixture(tmp_path)
    stamped = stamp_target("all", "v0.4.2", root=tmp_path, author_email="cran@example.org")
    assert stamped == "0.4.2"
    assert extract_version_from_tag("go/v0.4.2") == "0.4.2"

    versions = read_manifest_versions(tmp_path)
    assert versions == {
        "rust": "0.4.2",
        "root_package_json": "0.4.2",
        "conda": "0.4.2",
        "python": "0.4.2",
        "typescript": "0.4.2",
        "cpp": "0.4.2",
        "c": "0.4.2",
        "csharp": "0.4.2",
        "go": "0.4.2",
        "java": "0.4.2",
        "r": "0.4.2",
    }
    assert "cran@example.org" in (tmp_path / "r/DESCRIPTION").read_text(encoding="utf-8")

    for src_rel in (
        "python/src/playtest/__init__.py",
        "typescript/src/index.ts",
        "cpp/include/playtest/playtest.hpp",
        "c/include/playtest/playtest.h",
        "csharp/src/Playtest/Playtest.cs",
        "go/playtest.go",
        "java/src/main/java/io/github/archont561/playtest/Playtest.java",
        "r/R/version.R",
    ):
        assert "0.4.2" not in (tmp_path / src_rel).read_text(encoding="utf-8")
