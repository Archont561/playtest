"""Guard that keeps README-documented CLI commands traceable to the spec grammar."""

from __future__ import annotations

import re
from collections.abc import Set
from pathlib import Path

SPEC_GRAMMAR_SOURCE: str = "backlog/docs/specs/0008-cross-language-cli-and-toolchain-detection.md"
README_SOURCE: str = "README.md"

_COMMAND_WORD = re.compile(r"[a-z][a-z-]*")
# `playtest <cmd>` calls, plus the backticked inventory a sentence ends with before
# naming the orchestration commands, e.g. "the `test`, `coverage`, and `mutation`
# orchestration commands".
_PLAYTEST_CALL = re.compile(r"`?playtest ([a-z][a-z-]*)\b")
_TICKED = re.compile(r"`([a-z][a-z-]*)`")
_INVENTORY_TAIL = re.compile(r"orchestration\s+commands")


def spec_command_grammar(spec_text: str) -> Set[str]:
    """Return the subcommand names declared in a spec's CLI grammar fenced block."""
    in_grammar = False
    names: set[str] = set()
    for line in spec_text.splitlines():
        if line.startswith("```text"):
            in_grammar = True
            continue
        if in_grammar and line.startswith("```"):
            if names:
                break
            in_grammar = False
            continue
        if not in_grammar or not line.startswith("  "):
            continue
        token = line.strip().split(" ", 1)[0]
        if _COMMAND_WORD.fullmatch(token):
            names.add(token)
    return names


def mentioned_commands(readme_text: str) -> set[str]:
    """Collect commands the README names, by `playtest <cmd>` call or inventory list."""
    mentioned = {match.group(1) for match in _PLAYTEST_CALL.finditer(readme_text)}
    collapsed = " ".join(readme_text.split())
    for tail in _INVENTORY_TAIL.finditer(collapsed):
        start = collapsed.rfind(". ", 0, tail.start())
        mentioned.update(_TICKED.findall(collapsed[start + 2 : tail.start()]))
    return mentioned


def check_documented_commands(root: Path) -> list[str]:
    """Require every command the README names to exist in the spec's CLI grammar."""
    spec_path = root / SPEC_GRAMMAR_SOURCE
    readme_path = root / README_SOURCE
    if not spec_path.is_file() or not readme_path.is_file():
        return []
    declared = spec_command_grammar(spec_path.read_text(encoding="utf-8"))
    if not declared:
        return [f"no command grammar found in {SPEC_GRAMMAR_SOURCE}"]
    return [
        f"README names `playtest {cmd}` but {SPEC_GRAMMAR_SOURCE} declares no such command"
        for cmd in sorted(mentioned_commands(readme_path.read_text(encoding="utf-8")) - declared)
    ]
