#!/usr/bin/env python3
"""Move whole Rust items out of a file, by name.

A refactoring aid, not part of the build: nothing in CI runs it, and a change
it produces is reviewed as if it had been made by hand. It exists because
splitting a large module means moving hundreds of items whose line numbers
shift under every previous move, and a name is stable where a line range is
not.

    # What is in this file, and where.
    scripts/carve.py editor/src/native/mod.rs

    # The methods of one impl block.
    scripts/carve.py editor/src/native/mod.rs --in "impl EditorApp"

    # Move three items out, printing them for a new module.
    scripts/carve.py editor/src/native/mod.rs text_section value_row > /tmp/piece.rs

An item carries the doc comments and attributes written above it, and runs to
the end of its block. What comes out is exactly what goes in: the caller adds
the module header, the imports, and whatever visibility the move now needs —
none of which this can know.

It reads Rust by brace-counting rather than by parsing it, which is enough for
the shape this repository is written in and is not a general Rust parser. Check
what it produced: `cargo fmt --check` and the compiler catch a bad cut
immediately, and comparing the lines before and after catches a silent drop.
"""

import re
import signal
import sys

# Listings are read through `head` and `less`, which close the pipe early.
signal.signal(signal.SIGPIPE, signal.SIG_DFL)

# A char or byte literal, which is not a lifetime and may hold any bracket.
CHAR_LITERAL = re.compile(r"'(?:\\.|[^'\\])'")

ITEM = re.compile(
    r"^(?:pub(?:\([^)]*\))? )?(?:async |unsafe |extern |const (?=fn ))*"
    r"(fn|struct|enum|trait|type|impl|mod|union|const|static)[ <]"
)


def items(lines, indent=0, span=None):
    """Every item at `indent` as (name, start, end), start including its docs.

    With `span`, only that half-open line range is scanned, which is how the
    methods of one impl block are reached without the rest of the file.
    """
    pad = " " * indent
    first, last = span if span else (0, len(lines))
    found = []
    depth = 0
    i = first
    attach = None  # first line of the doc/attribute run above the current item
    while i < last:
        line = lines[i]
        stripped = line.rstrip()
        if depth == 0 and (not pad or at_indent(line, pad)):
            bare = stripped.strip()
            if bare.startswith(("#[", "#!")):
                if attach is None:
                    attach = i
                i = attribute_end(lines, i) + 1
                continue
            if bare.startswith(("///", "//!", "//")):
                if attach is None:
                    attach = i
                i += 1
                continue
            if not bare:
                attach = None
                i += 1
                continue
            match = ITEM.match(bare) if at_indent(line, pad) else None
            if match:
                kind = match.group(1)
                name = signature_name(bare, kind, match.end(1))
                start = attach if attach is not None else i
                end = block_end(lines, i)
                found.append((name, start, end))
                attach = None
                i = end + 1
                continue
            # A statement-like top-level line (use, macro call): skip it.
            attach = None
            end = block_end(lines, i)
            i = end + 1
            continue
        i += 1
    return found


def at_indent(line, pad):
    """Whether the line begins exactly at this indentation."""
    return line.startswith(pad) and not line[len(pad) : len(pad) + 1].isspace()


def signature_name(bare, kind, at):
    if kind == "impl":
        # The whole header, so `impl<T> Store<T>` and `impl Store` stay apart.
        return bare.split("{", 1)[0].strip()
    rest = bare[at:].lstrip()
    if kind in ("const", "static"):
        return re.split(r"[:< =]", rest, 1)[0]
    return re.split(r"[(<:{ ;=]", rest, 1)[0]


def attribute_end(lines, start):
    """Index of the line closing an attribute that may span several lines."""
    depth = 0
    for i in range(start, len(lines)):
        for char in strip_strings(lines[i]):
            if char in "[(":
                depth += 1
            elif char in "])":
                depth -= 1
        if depth <= 0:
            return i
    return len(lines) - 1


def block_end(lines, start):
    """Index of the line closing the item that starts at `start`."""
    depth = 0
    opened = False
    for i in range(start, len(lines)):
        for char in strip_strings(lines[i]):
            if char in "{([":
                depth += 1
                opened = True
            elif char in "})]":
                depth -= 1
        if opened and depth <= 0:
            return i
        if not opened and lines[i].rstrip().endswith(";"):
            return i
    return len(lines) - 1


def strip_strings(line):
    """The line with string and char literals and comments blanked out."""
    out = []
    i = 0
    while i < len(line):
        char = line[i]
        if char == "/" and line[i : i + 2] == "//":
            break
        if char == '"':
            i += 1
            while i < len(line):
                if line[i] == "\\":
                    i += 2
                    continue
                if line[i] == '"':
                    break
                i += 1
            i += 1
            continue
        literal = CHAR_LITERAL.match(line, i)
        if literal:
            i = literal.end()
            continue
        out.append(char)
        i += 1
    return "".join(out)


def main():
    argv = sys.argv[1:]
    inside = None
    if "--in" in argv:
        at = argv.index("--in")
        inside = argv[at + 1]
        del argv[at : at + 2]
    source, names = argv[0], argv[1:]
    lines = open(source).read().split("\n")
    if inside:
        outer = {name: (start, end) for name, start, end in items(lines)}
        if inside not in outer:
            raise SystemExit(f"no such block: {inside}")
        start, end = outer[inside]
        found = items(lines, indent=4, span=(start + 1, end))
    else:
        found = items(lines)
    index = {name: (start, end) for name, start, end in found}

    if not names:
        for name, start, end in found:
            print(f"{start + 1}-{end + 1}\t{name}")
        return

    missing = [name for name in names if name not in index]
    if missing:
        raise SystemExit(f"not found: {missing}")

    taken = sorted(index[name] for name in names)
    carved = []
    for start, end in taken:
        carved.append("\n".join(lines[start : end + 1]))
    drop = set()
    for start, end in taken:
        drop.update(range(start, end + 1))
    kept = [line for i, line in enumerate(lines) if i not in drop]

    open(source, "w").write(collapse_blank_runs(kept))
    sys.stdout.write("\n\n".join(carved) + "\n")


def collapse_blank_runs(lines):
    out = []
    for line in lines:
        if not line.strip() and out and not out[-1].strip():
            continue
        out.append(line)
    return "\n".join(out)


if __name__ == "__main__":
    main()
