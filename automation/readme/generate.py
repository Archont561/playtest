"""Regenerate and verify `README.md` from `automation/readme/README.ejs`."""

from __future__ import annotations

import argparse
import json
import shutil
import tempfile
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path

from automation import REPO_ROOT
from automation._proc import emit_err, emit_line, run_cmd
from automation.banner import WORDMARK, render_banner

TEMPLATE: Path = Path(__file__).resolve().parent / "README.ejs"
PACKAGE_JSON: Path = REPO_ROOT / "package.json"
TARGET: Path = REPO_ROOT / "README.md"
GENERATOR_PACKAGE: str = "readme-md-generator"
ATTRIBUTION: str = "This README was generated"
BANNER_MARKER: str = "<!-- playtest-banner -->"

Runner = Callable[[Sequence[str], Path], int]


@dataclass(frozen=True)
class ReadmeResult:
    """Rendered README text and the generator command that produced it."""

    content: str
    command: tuple[str, ...]

    @property
    def attributed(self) -> bool:
        """Whether the rendered README retains the generator attribution footer."""
        return ATTRIBUTION in self.content


def subprocess_runner(command: Sequence[str], cwd: Path) -> int:
    """Execute `command` in `cwd` with captured output to keep check logs clean."""
    res = run_cmd(command, cwd=cwd, timeout=120, capture=True)
    return res.returncode


def pinned_version(package_json: Path = PACKAGE_JSON) -> str:
    """Return the exact `readme-md-generator` version pinned in `package.json`."""
    data = json.loads(package_json.read_text(encoding="utf-8"))
    dev_deps = data.get("devDependencies", {})
    version = dev_deps.get(GENERATOR_PACKAGE) if isinstance(dev_deps, dict) else None
    if not version:
        raise ValueError(f"{GENERATOR_PACKAGE} is not pinned in {package_json.name}")
    ver_str = str(version)
    if not all(part.isdigit() for part in ver_str.split(".")):
        raise ValueError(f"pin {GENERATOR_PACKAGE} to an exact version, got {ver_str!r}")
    return ver_str


def generator_command(version: str, root: Path = REPO_ROOT) -> list[str]:
    """Build the local Bun (preferred) or npx command for `readme-md-generator`."""
    local_cli = root / "node_modules" / GENERATOR_PACKAGE / "src" / "index.js"
    bun_bin = shutil.which("bun")
    if bun_bin is not None and local_cli.is_file():
        return [bun_bin, str(local_cli)]
    if bun_bin is not None:
        return [bun_bin, "x", f"{GENERATOR_PACKAGE}@{version}"]
    npx_bin = shutil.which("npx") or "npx"
    return [npx_bin, "--yes", f"{GENERATOR_PACKAGE}@{version}"]


def inject_banner(content: str, *, art: str | None = None) -> str:
    """Replace `BANNER_MARKER` with the FIGlet `PLAYTEST` wordmark."""
    if BANNER_MARKER not in content:
        return content
    rendered = render_banner(WORDMARK) if art is None else art
    return content.replace(BANNER_MARKER, rendered)


def render_readme(
    *,
    template: Path = TEMPLATE,
    package_json: Path = PACKAGE_JSON,
    runner: Runner = subprocess_runner,
) -> ReadmeResult:
    """Render `README.md` inside an isolated temporary directory."""
    if not template.is_file():
        raise FileNotFoundError(f"template not found: {template}")
    if not package_json.is_file():
        raise FileNotFoundError(f"metadata not found: {package_json}")

    version = pinned_version(package_json)
    command = [
        *generator_command(version, package_json.parent),
        "-y",
        "-p",
        str(template.resolve()),
    ]
    workdir = Path(tempfile.mkdtemp(prefix="playtest-readme-"))
    try:
        shutil.copyfile(package_json, workdir / "package.json")
        status = runner(command, workdir)
        produced = workdir / "README.md"
        if status != 0:
            raise RuntimeError(f"{GENERATOR_PACKAGE} exited with status {status}")
        if not produced.is_file():
            raise RuntimeError(f"{GENERATOR_PACKAGE} did not produce a README.md")
        raw_text = produced.read_text(encoding="utf-8")
        normalized = "\n".join(line.rstrip() for line in raw_text.splitlines()) + "\n"
        content = inject_banner(normalized)
        return ReadmeResult(content=content, command=tuple(command))
    finally:
        shutil.rmtree(workdir, ignore_errors=True)


def write_readme(
    *,
    check: bool = False,
    template: Path = TEMPLATE,
    package_json: Path = PACKAGE_JSON,
    target: Path = TARGET,
    runner: Runner = subprocess_runner,
) -> int:
    """Regenerate `README.md` or fail if `--check` detects drift."""
    try:
        result = render_readme(template=template, package_json=package_json, runner=runner)
    except (FileNotFoundError, ValueError) as err:
        emit_err(f"[readme] ERROR: {err}")
        return 2
    except RuntimeError as err:
        emit_err(f"[readme] ERROR: {err}")
        return 1

    if not result.attributed:
        emit_err(f"[readme] ERROR: generated output lacks {GENERATOR_PACKAGE} attribution")
        return 1

    existing = target.read_text(encoding="utf-8") if target.is_file() else ""
    if check:
        if result.content != existing:
            emit_err(
                "README.md is out of date. Run `python -m automation.readme` and commit the result."
            )
            return 1
        emit_line("[readme] OK: README.md matches automation/readme/README.ejs.")
        return 0

    if result.content == existing:
        emit_line("[readme] OK: README.md unchanged.")
    else:
        target.write_text(result.content, encoding="utf-8")
        emit_line(f"[readme] OK: wrote {target.name} from {template.name}.")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for `python -m automation.readme`."""
    parser = argparse.ArgumentParser(
        prog="python -m automation.readme",
        description="Regenerate or check README.md via readme-md-generator.",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Fail if README.md differs from a fresh render.",
    )
    parser.add_argument(
        "--template",
        type=Path,
        default=TEMPLATE,
        help="Path to the EJS README template.",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)
    return write_readme(check=args.check, template=args.template)


if __name__ == "__main__":
    raise SystemExit(main())
