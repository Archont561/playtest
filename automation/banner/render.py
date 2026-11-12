"""Render deterministic FIGlet wordmarks from bundled `.flf` font data."""

from __future__ import annotations

import argparse
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

from automation._proc import emit_err, emit_line

FONT_DIR: Path = Path(__file__).resolve().parent
DEFAULT_FONT: str = "ansi_shadow"
WORDMARK: str = "PLAYTEST"
DEFAULT_COMMENT_PREFIX: str = "# "


@dataclass(frozen=True)
class FigletFontData:
    """Parsed FIGlet `.flf` header and printable ASCII glyph rows."""

    hardblank: str
    height: int
    glyphs: dict[str, tuple[str, ...]]


def _parse_glyphs(lines: list[str], start_pos: int, height: int) -> dict[str, tuple[str, ...]]:
    pos = start_pos
    glyphs: dict[str, tuple[str, ...]] = {}
    for code in range(32, 127):
        rows: list[str] = []
        for offset in range(height):
            raw = lines[pos + offset]
            endmark = raw[-1] if raw else "@"
            rows.append(raw.rstrip(endmark))
        glyphs[chr(code)] = tuple(rows)
        pos += height
    return glyphs


def _load_font(font: str = DEFAULT_FONT) -> FigletFontData:
    font_path = FONT_DIR / f"{font}.flf"
    if not font_path.is_file():
        raise ValueError(f"unknown bundled figlet font {font!r}")

    lines = font_path.read_text(encoding="utf-8").splitlines()
    header = lines[0]
    if not header.startswith("flf2a") or len(header) < 7:
        raise ValueError(f"invalid FIGlet header in {font_path.name}")

    meta = header[6:].split()
    height = int(meta[0])
    start_pos = 1 + int(meta[4])
    return FigletFontData(
        hardblank=header[5],
        height=height,
        glyphs=_parse_glyphs(lines, start_pos, height),
    )


def _row_gap(left: str, right: str) -> int:
    trailing = len(left) - len(left.rstrip(" "))
    leading = len(right) - len(right.lstrip(" "))
    return trailing + leading


def _min_row_shift(acc: list[str], glyph: tuple[str, ...], height: int) -> int:
    if not any(acc):
        return 0
    return min(_row_gap(acc[row], glyph[row]) for row in range(height))


def _kern_append(acc: list[str], glyph: tuple[str, ...], height: int) -> None:
    shift = _min_row_shift(acc, glyph, height)
    for row in range(height):
        left_spaces = len(acc[row]) - len(acc[row].rstrip(" "))
        left_trim = min(shift, left_spaces)
        right_trim = shift - left_trim
        left_part = acc[row][: len(acc[row]) - left_trim] if left_trim else acc[row]
        acc[row] = left_part + glyph[row][right_trim:]


def render_banner(text: str, *, font: str = DEFAULT_FONT) -> str:
    """Render `text` as kerned FIGlet art with trailing padding removed."""
    if not text.strip():
        raise ValueError("banner text cannot be blank")
    font_data = _load_font(font)
    acc = [""] * font_data.height
    for char in text:
        glyph = font_data.glyphs.get(char)
        if glyph is None:
            raise ValueError(f"unsupported non-ASCII character {char!r} in banner text")
        _kern_append(acc, glyph, font_data.height)

    rows = [row.replace(font_data.hardblank, " ").rstrip() for row in acc]
    while rows and not rows[-1]:
        rows.pop()
    return "\n".join(rows)


def get_banner_width(text: str, *, font: str = DEFAULT_FONT) -> int:
    """Return the widest row width in columns for `render_banner(text)`."""
    return max(len(line) for line in render_banner(text, font=font).splitlines())


def render_comment_banner(
    text: str,
    *,
    prefix: str = DEFAULT_COMMENT_PREFIX,
    font: str = DEFAULT_FONT,
) -> str:
    """Render `text` as a comment block prefixed on every line."""
    art = render_banner(text, font=font)
    return "\n".join(f"{prefix}{line}" for line in art.splitlines())


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entry point for rendering the project FIGlet wordmark."""
    parser = argparse.ArgumentParser(
        prog="python -m automation.banner",
        description="Render ASCII text as FIGlet art using bundled font data.",
    )
    parser.add_argument("text", nargs="?", default=WORDMARK, help="Text to render.")
    parser.add_argument("--font", default=DEFAULT_FONT, help="Bundled font name.")
    parser.add_argument(
        "--comment",
        nargs="?",
        const=DEFAULT_COMMENT_PREFIX,
        default=None,
        metavar="PREFIX",
        help="Prefix each rendered row as a comment.",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)
    try:
        output = (
            render_banner(args.text, font=args.font)
            if args.comment is None
            else render_comment_banner(args.text, prefix=args.comment, font=args.font)
        )
    except ValueError as err:
        emit_err(f"[banner] ERROR: {err}")
        return 2
    emit_line(output)
    return 0
