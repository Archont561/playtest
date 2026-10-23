#!/usr/bin/env python3
"""Stamp tag-derived versions into source files used to build release packages."""

from __future__ import annotations

import argparse
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def replace_once(path: Path, pattern: str, replacement: str, *, flags: int = 0) -> None:
    text = path.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, count=1, flags=flags)
    if count != 1:
        raise SystemExit(f"Expected exactly one version field in {path}, found {count}")
    path.write_text(updated, encoding="utf-8")


def stamp(target: str, version: str, author_email: str | None = None) -> None:
    if target == "csharp":
        replace_once(
            ROOT / "csharp/src/Playtest/Playtest.csproj",
            r"(<Version>)[^<]+(</Version>)",
            rf"\g<1>{version}\g<2>",
        )
        replace_once(
            ROOT / "csharp/src/Playtest/Playtest.cs",
            r'(const string Version = ")[^"]+(";)',
            rf"\g<1>{version}\g<2>",
        )
    elif target == "java":
        replace_once(
            ROOT / "java/src/main/java/io/github/archont561/playtest/Playtest.java",
            r'(VERSION = ")[^"]+(";)',
            rf"\g<1>{version}\g<2>",
        )
    elif target == "r":
        if not re.fullmatch(r"\d+\.\d+\.\d+", version):
            raise SystemExit("R/CRAN releases require a numeric X.Y.Z version")
        if not author_email or not re.fullmatch(r"[^@\s]+@[^@\s]+\.[^@\s]+", author_email):
            raise SystemExit("R/CRAN releases require a valid AUTHOR_EMAIL")
        replace_once(
            ROOT / "r/DESCRIPTION",
            r"^Version: .*$",
            f"Version: {version}",
            flags=re.MULTILINE,
        )
        description = ROOT / "r/DESCRIPTION"
        text = description.read_text(encoding="utf-8")
        updated, count = re.subn(
            r'^(Authors@R:.*email = ")[^"]+(".*)$',
            lambda match: f"{match.group(1)}{author_email}{match.group(2)}",
            text,
            count=1,
            flags=re.MULTILINE,
        )
        if count != 1:
            raise SystemExit(f"Expected one Authors@R email in {description}, found {count}")
        description.write_text(updated, encoding="utf-8")
        replace_once(
            ROOT / "r/R/version.R",
            r'^(VERSION <- ")[^"]+("$)',
            rf"\g<1>{version}\g<2>",
            flags=re.MULTILINE,
        )
    else:
        raise SystemExit(f"Unsupported release target: {target}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target", choices=("csharp", "java", "r"))
    parser.add_argument("version", help="Release version without the tag scope/prefix")
    parser.add_argument("--author-email", help="Maintainer email to stamp into R DESCRIPTION")
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9A-Za-z][0-9A-Za-z.+-]*", args.version):
        raise SystemExit("Version contains unsupported characters")
    stamp(args.target, args.version, author_email=args.author_email)


if __name__ == "__main__":
    main()
