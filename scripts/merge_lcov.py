#!/usr/bin/env python3
"""
merge_lcov.py

Universal Cross-Language LCOV Aggregator.

Every Playtest language target (Rust, Python, TypeScript, C++, Go, Java, R)
is responsible for emitting standard LCOV in its own `coverage/lcov.info`
using its native CLI or built-in converter.

This aggregator:
1. Discovers standard LCOV files across all subproject packages.
2. Normalizes `SF:` source file paths relative to the repository root.
3. Applies .codecov.yml exclusions (tests/**, docs, etc.).
4. Concatenates all records into a single unified `coverage/lcov.info`.
5. Prints a multi-language coverage summary table and checks thresholds.
"""

import argparse
import os
import re
import sys
from pathlib import Path


def get_git_root() -> Path:
    """Return the repository root directory."""
    script_dir = Path(__file__).resolve().parent
    return script_dir.parent


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Merge multi-language LCOV files into a unified Codecov-compatible report."
    )
    parser.add_argument(
        "--output",
        "-o",
        default="coverage/lcov.info",
        help="Path for unified LCOV output file (default: coverage/lcov.info)",
    )
    parser.add_argument(
        "--threshold",
        "-t",
        type=float,
        default=None,
        help="Enforce minimum total line coverage percentage (e.g. 90.0)",
    )
    return parser.parse_args()


class LcovRecord:
    def __init__(self, source_file: str):
        self.source_file = source_file
        self.lines: dict[int, int] = {}
        self.functions: list[str] = []

    def add_line(self, line_no: int, count: int):
        self.lines[line_no] = self.lines.get(line_no, 0) + count

    def to_lcov(self) -> str:
        out = [f"SF:{self.source_file}"]
        for fn in self.functions:
            out.append(fn)
        for line_no in sorted(self.lines.keys()):
            out.append(f"DA:{line_no},{self.lines[line_no]}")
        lf = len(self.lines)
        lh = sum(1 for c in self.lines.values() if c > 0)
        out.append(f"LF:{lf}")
        out.append(f"LH:{lh}")
        out.append("end_of_record")
        return "\n".join(out)


def parse_lcov_file(file_path: Path, subproject_dir: Path, repo_root: Path) -> list[LcovRecord]:
    """Parse a standard LCOV file, rewriting SF paths relative to repository root."""
    if not file_path.exists():
        return []

    records = []
    current_record = None

    with open(file_path, "r", encoding="utf-8", errors="ignore") as f:
        for raw_line in f:
            line = raw_line.strip()
            if not line:
                continue

            if line.startswith("SF:"):
                raw_path = line[3:].strip()
                p = Path(raw_path)
                if not p.is_absolute():
                    p = (subproject_dir / p).resolve()
                else:
                    p = p.resolve()

                try:
                    rel_path = p.relative_to(repo_root)
                except ValueError:
                    # File outside repository root (e.g. /usr/include/*) -> ignore
                    current_record = None
                    continue

                current_record = LcovRecord(str(rel_path))

            elif current_record is not None:
                if line.startswith("DA:"):
                    parts = line[3:].split(",")
                    line_no = int(parts[0])
                    count = int(parts[1])
                    current_record.add_line(line_no, count)
                elif line.startswith("FN:") or line.startswith("FNDA:"):
                    current_record.functions.append(line)
                elif line == "end_of_record":
                    records.append(current_record)
                    current_record = None

    return records


def should_ignore(path_str: str) -> bool:
    """Filter out test files or ignored paths per .codecov.yml."""
    ignored_patterns = [
        r"^tests/",
        r"/tests/",
        r"^crates/[^/]+/tests/",
        r"^examples/",
        r"^benches/",
        r"^backlog/",
        r"^go/cmd/",  # CLI converter tools
        r"\.md$",
    ]
    for pat in ignored_patterns:
        if re.search(pat, path_str):
            return True
    return False


def main() -> int:
    args = parse_args()
    repo_root = get_git_root()

    # Search paths for decentralized standard LCOV files across language targets
    subproject_lcov_targets = {
        "Rust": (repo_root / "coverage" / "lcov-rs.info", repo_root),
        "Python": (repo_root / "python" / "coverage" / "lcov-py.info", repo_root / "python"),
        "TypeScript": (repo_root / "typescript" / "coverage" / "lcov.info", repo_root / "typescript"),
        "C++": (repo_root / "cpp" / "coverage" / "lcov-cpp.info", repo_root / "cpp"),
        "C": (repo_root / "c" / "coverage" / "lcov.info", repo_root / "c"),
        "C#": (repo_root / "csharp" / "coverage" / "lcov.info", repo_root / "csharp"),
        "Go": (repo_root / "go" / "coverage" / "lcov.info", repo_root / "go"),
        "Java": (repo_root / "java" / "coverage" / "lcov.info", repo_root / "java"),
        "R": (repo_root / "r" / "coverage" / "lcov.info", repo_root / "r"),
    }

    print("==================================================")
    print(" Playtest Universal LCOV Coverage Aggregator")
    print("==================================================")
    print(f"{'Target':<14} {'Files':<8} {'Lines Found':<14} {'Lines Hit':<12} {'Coverage':<10}")
    print("-" * 58)

    all_records: list[LcovRecord] = []
    total_lf = 0
    total_lh = 0

    for name, (lcov_path, subproject_dir) in subproject_lcov_targets.items():
        if not lcov_path.exists():
            # Fallback to standard coverage/lcov.info inside subproject dir
            alt_path = subproject_dir / "coverage" / "lcov.info"
            if alt_path.exists():
                lcov_path = alt_path

        records = parse_lcov_file(lcov_path, subproject_dir, repo_root)
        active_records = [r for r in records if not should_ignore(r.source_file)]

        t_lf = sum(len(r.lines) for r in active_records)
        t_lh = sum(sum(1 for c in r.lines.values() if c > 0) for r in active_records)
        pct = (t_lh / t_lf * 100.0) if t_lf > 0 else 100.0

        total_lf += t_lf
        total_lh += t_lh
        all_records.extend(active_records)

        status_str = f"{pct:>6.2f}%" if lcov_path.exists() else "skipped"
        print(f"{name:<14} {len(active_records):<8} {t_lf:<14} {t_lh:<12} {status_str}")

    print("-" * 58)
    total_pct = (total_lh / total_lf * 100.0) if total_lf > 0 else 100.0
    print(f"{'TOTAL':<14} {len(all_records):<8} {total_lf:<14} {total_lh:<12} {total_pct:>6.2f}%")
    print("==================================================")

    # Write unified output
    out_path = repo_root / args.output
    out_path.parent.mkdir(parents=True, exist_ok=True)

    with open(out_path, "w", encoding="utf-8") as f:
        for r in all_records:
            f.write(r.to_lcov())
            f.write("\n")

    print(f"Unified LCOV report successfully written to: {args.output}")

    if args.threshold is not None:
        if total_pct < args.threshold:
            print(
                f"::error:: Coverage {total_pct:.2f}% is below required threshold {args.threshold:.2f}%",
                file=sys.stderr,
            )
            return 1
        else:
            print(f"Coverage {total_pct:.2f}% meets required threshold {args.threshold:.2f}%.")

    return 0


if __name__ == "__main__":
    sys.exit(main())
