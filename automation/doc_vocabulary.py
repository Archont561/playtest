"""Guard that keeps retired positioning vocabulary out of doc comments.

The canonical description reaches every package manifest, but not the doc comment at the
top of each SDK's entry point. Those comments carried the retired "Playwright-style"
anchor for longer than the manifests did, because no check read them. This module makes
the same rule apply to the source tree.
"""

from __future__ import annotations

from collections.abc import Iterable
from pathlib import Path

# Vocabulary the positioning record retired. Matched case-insensitively against doc
# comments only, so package keywords (a search term, not a claim) stay untouched.
RETIRED_TERMS: tuple[str, ...] = ("playwright", "next-gen")

# Files whose opening comment states what the package is. Extending the list is a review
# decision, not an automated one: these are the nine SDK entry points.
DOC_SURFACES: tuple[str, ...] = (
    "crates/playtest/src/lib.rs",
    "python/src/playtest/__init__.py",
    "typescript/src/index.ts",
    "c/include/playtest/playtest.h",
    "cpp/include/playtest/playtest.hpp",
    "csharp/src/Playtest/Playtest.cs",
    "go/playtest.go",
    "java/src/main/java/io/github/archont561/playtest/Playtest.java",
)

# How many lines from the top of a file count as its header comment.
HEADER_LINES: int = 12


def header_comment(text: str, limit: int = HEADER_LINES) -> str:
    """Return the leading comment or docstring block of `text`, up to `limit` lines."""
    collected: list[str] = []
    for line in text.splitlines()[:limit]:
        stripped = line.strip()
        if collected and not stripped:
            break
        collected.append(line)
    return "\n".join(collected)


def find_retired_terms(text: str, terms: Iterable[str] = RETIRED_TERMS) -> list[str]:
    """Return the retired terms present in the header comment of `text`."""
    header = header_comment(text).lower()
    return [term for term in terms if term in header]


def check_doc_vocabulary(root: Path) -> list[str]:
    """Report every SDK entry point whose header comment uses retired vocabulary."""
    errors: list[str] = []
    for rel in DOC_SURFACES:
        path = root / rel
        if not path.is_file():
            continue
        for term in find_retired_terms(path.read_text(encoding="utf-8")):
            errors.append(
                f"retired positioning term {term!r} in the header comment of {rel}; "
                "use the canonical description from automation/repo_check.py"
            )
    return errors
