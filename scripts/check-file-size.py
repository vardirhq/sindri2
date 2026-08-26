#!/usr/bin/env python3
"""Fail when a Rust source file has grown past what anyone will read.

The rule and the reasoning are in `docs/module-layout.md`: 400 lines is the
target, 600 is the cap, and a file over the cap is split by responsibility
rather than cut in half. This script is the part of that a machine can check,
and CI runs it beside `cargo fmt --check`.

    scripts/check-file-size.py            # check, exit 1 on a violation
    scripts/check-file-size.py --report   # every file over the target, sorted
    scripts/check-file-size.py --max 500  # a stricter cap, for a one-off sweep

Exemptions live in EXEMPT below, each with a reason. Adding one is a decision
someone reviews; it is not a way to opt out.
"""

import argparse
import signal
import subprocess
import sys
from pathlib import Path

# The report is read through `head` and `less`, which close the pipe early.
signal.signal(signal.SIGPIPE, signal.SIG_DFL)

TARGET = 400
CAP = 600

# Paths never counted: not ours, or not written by hand.
SKIP_DIRS = {"target", "node_modules", ".git", "assets", "site"}

# Files allowed past the cap, each with the reason it resists splitting.
# Keep this short. An entry is a reviewed decision, not an escape hatch.
EXEMPT: dict[str, str] = {}


def tracked_rust_files(root: Path) -> list[Path]:
    """Every hand-written .rs file git knows about."""
    listing = subprocess.run(
        ["git", "ls-files", "-z", "*.rs"],
        cwd=root,
        capture_output=True,
        text=True,
        check=True,
    )
    files = []
    for name in listing.stdout.split("\0"):
        if not name:
            continue
        path = Path(name)
        if SKIP_DIRS.intersection(path.parts):
            continue
        files.append(path)
    return sorted(files)


def line_count(path: Path) -> int:
    with path.open(encoding="utf-8", errors="replace") as handle:
        return sum(1 for _ in handle)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--max", type=int, default=CAP, help=f"cap (default {CAP})")
    parser.add_argument(
        "--report",
        action="store_true",
        help=f"list every file over the {TARGET}-line target instead of checking",
    )
    args = parser.parse_args()

    root = Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    )

    sized = [(line_count(root / path), path) for path in tracked_rust_files(root)]

    if args.report:
        over = sorted((n, p) for n, p in sized if n > TARGET)
        for count, path in reversed(over):
            note = "  (exempt)" if str(path) in EXEMPT else ""
            flag = "!" if count > args.max and not note else " "
            print(f"{flag} {count:5}  {path}{note}")
        print(f"\n{len(over)} file(s) over the {TARGET}-line target.")
        return 0

    violations = [
        (count, path)
        for count, path in sized
        if count > args.max and str(path) not in EXEMPT
    ]
    if not violations:
        print(f"All {len(sized)} Rust files are within {args.max} lines.")
        return 0

    print(f"{len(violations)} file(s) over the {args.max}-line cap:\n", file=sys.stderr)
    for count, path in sorted(violations, reverse=True):
        print(f"  {count:5}  {path}", file=sys.stderr)
    print(
        "\nSplit them by responsibility — see docs/module-layout.md."
        "\n`scripts/carve.py <file>` lists what is in one, and moves items by name.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
