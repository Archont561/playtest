"""Unit tests for `automation.banner` and `automation.readme`."""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path

import pytest

from automation.banner import (
    WORDMARK,
    get_banner_width,
    render_banner,
    render_comment_banner,
)
from automation.readme import (
    ATTRIBUTION,
    BANNER_MARKER,
    PACKAGE_JSON,
    inject_banner,
    pinned_version,
    render_readme,
    write_readme,
)


def test_banner_renders_playtest_wordmark_and_comment_form() -> None:
    art = render_banner(WORDMARK)
    lines = art.splitlines()
    assert len(lines) == 6
    assert get_banner_width(WORDMARK) == max(len(line) for line in lines)
    commented = render_comment_banner("HI", prefix="# ")
    assert all(row.startswith("# ") for row in commented.splitlines())


def test_banner_rejects_blank_or_unknown_font() -> None:
    with pytest.raises(ValueError, match="blank"):
        render_banner("   ")
    with pytest.raises(ValueError, match="unknown bundled figlet font"):
        render_banner("PLAYTEST", font="missing_font")


def test_pinned_version_reads_exact_semver(tmp_path: Path) -> None:
    assert pinned_version(PACKAGE_JSON) == "1.0.0"
    bad_pkg = tmp_path / "package.json"
    bad_pkg.write_text('{"devDependencies": {"readme-md-generator": "^1.0.0"}}', encoding="utf-8")
    with pytest.raises(ValueError, match="exact version"):
        pinned_version(bad_pkg)


def test_render_and_check_readme_with_stub_runner(tmp_path: Path) -> None:
    template = tmp_path / "README.ejs"
    template.write_text("# Stub\n", encoding="utf-8")
    pkg = tmp_path / "package.json"
    pkg.write_text('{"devDependencies": {"readme-md-generator": "1.0.0"}}', encoding="utf-8")
    target = tmp_path / "README.md"

    def fake_runner(_cmd: Sequence[str], cwd: Path) -> int:
        (cwd / "README.md").write_text(
            f"# Playtest\n{BANNER_MARKER}\n\n{ATTRIBUTION}\n",
            encoding="utf-8",
        )
        return 0

    rendered = render_readme(template=template, package_json=pkg, runner=fake_runner)
    assert rendered.attributed
    assert BANNER_MARKER not in rendered.content
    assert inject_banner("plain", art="CUSTOM") == "plain"

    assert write_readme(template=template, package_json=pkg, target=target, runner=fake_runner) == 0
    assert (
        write_readme(
            check=True,
            template=template,
            package_json=pkg,
            target=target,
            runner=fake_runner,
        )
        == 0
    )

    target.write_text("stale\n", encoding="utf-8")
    assert (
        write_readme(
            check=True,
            template=template,
            package_json=pkg,
            target=target,
            runner=fake_runner,
        )
        == 1
    )
