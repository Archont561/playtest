# ADR 0009: Python-Only Rule for Repository Automation

## Status
Accepted

## Context
Shell scripts (`.sh` / bash) often suffer from platform incompatibilities (e.g. GNU vs. BSD utils, macOS vs. Linux), fragile string manipulation, unhandled whitespace errors, and complex JSON/YAML parsing. In contrast, Python 3 is bundled hermetically within the Pixi environment, provides robust standard libraries (`subprocess`, `pathlib`, `argparse`, `json`, `re`), and offers superior testability and error handling across operating systems.

## Decision
All repository-wide automation scripts must be implemented exclusively in **Python 3** (`.py`).
- Shell scripts (`.sh`) are strictly prohibited across the repository for automation tasks.
- The sole permissible exception is `scripts/restore.sh`, which is vendor-provided by `pixi-sandbox` and part of the sandbox restoration contract.
- Release detection, manifest updates, code generation, and backlog maintenance scripts must reside in `scripts/*.py` and be invoked via `pixi run python ...`.

## Consequences
### Positive
- Cross-platform portability without depending on platform-specific bash flags or GNU coreutils.
- Strong structured typing, exception handling, and self-documenting CLI interfaces via `argparse`.
- Clean integration with `pixi run` without shell subshell escaping issues.

### Negative
- Requires Python 3 runtime available in the environment (guaranteed by Pixi).
