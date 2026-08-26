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

A name that belongs to more than one item — two `#[cfg]` variants of the same
function — moves all of them, and says so. Taking one and leaving the other is
how a split loses code.

It reads Rust by brace-counting rather than by parsing it, which is enough for
the shape this repository is written in and is not a general Rust parser. It
knows about the things that used to fool it: byte and char literals holding a
bracket, strings that run across lines, a declaration whose brackets balance
before it ends, and generics after the item keyword. Where it can tell it has
misread — a cut whose brackets do not close — it refuses to make the cut rather
than writing a broken one.

Check what it produced anyway. `cargo fmt --check` and the compiler catch a bad
cut immediately, and comparing the line counts before and after catches a
silent drop.
"""

import re
import signal
import sys

# Listings are read through `head` and `less`, which close the pipe early.
signal.signal(signal.SIGPIPE, signal.SIG_DFL)

# A char or byte literal, which is not a lifetime and may hold any bracket.
CHAR_LITERAL = re.compile(r"'(?:\\.|[^'\\])'")

# The start of a string. Group 1 is the `#`s a raw string must close with.
STRING_OPEN = re.compile(r'(?:b?r(#*)|b)?"')

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
    string = None
    for i in range(start, len(lines)):
        clean, string = strip_strings(lines[i], string)
        for char in clean:
            if char in "[(":
                depth += 1
            elif char in "])":
                depth -= 1
        if string is None and depth <= 0:
            return i
    return len(lines) - 1


def block_end(lines, start):
    """Index of the line closing the item that starts at `start`.

    A braced item ends where its brace closes; everything else ends at the
    first `;` outside brackets. The two rules are separate because a
    declaration can balance its brackets long before it ends —
    `pub(crate) const CALLS: &[(&str, Call)] =` opens and closes three pairs
    and then continues on the next line.
    """
    depth = 0
    braced = False
    string = None
    for i in range(start, len(lines)):
        clean, string = strip_strings(lines[i], string)
        for char in clean:
            if char in "{([":
                depth += 1
                braced = braced or char == "{"
            elif char in "})]":
                depth -= 1
        if string is not None or depth > 0:
            continue
        if braced or clean.rstrip().endswith(";"):
            return i
    return len(lines) - 1


def strip_strings(line, string=None):
    """One line with strings, char literals, and comments blanked out.

    `string` carries an unterminated string in from the previous line and comes
    back out for the next one, because a raw string holding braces —
    `r"script Guard { ... }"` around a snippet of Decay — spans lines, and
    counting its braces as code ends an item in the middle of itself.
    """
    out = []
    i = 0
    while i < len(line):
        if string is not None:
            end = string_close(line, i, string)
            if end is None:
                return "".join(out), string
            i = end
            string = None
            continue
        char = line[i]
        if char == "/" and line[i : i + 2] == "//":
            break
        opening = STRING_OPEN.match(line, i)
        if opening:
            string = opening.group(1) or ""  # the `#`s a raw string closes with
            i = opening.end()
            continue
        literal = CHAR_LITERAL.match(line, i)
        if literal:
            i = literal.end()
            continue
        out.append(char)
        i += 1
    return "".join(out), string


def string_close(line, start, hashes):
    """Index just past the end of an open string, or None if it runs on."""
    if hashes:
        at = line.find('"' + hashes, start)
        return None if at < 0 else at + 1 + len(hashes)
    i = start
    while i < len(line):
        if line[i] == "\\":
            i += 2
            continue
        if line[i] == '"':
            return i + 1
        i += 1
    return None


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
    index = {}
    for name, start, end in found:
        index.setdefault(name, []).append((start, end))

    if not names:
        for name, start, end in found:
            print(f"{start + 1}-{end + 1}\t{name}")
        return

    missing = [name for name in names if name not in index]
    if missing:
        raise SystemExit(f"not found: {missing}")

    # A name can belong to more than one item — two `#[cfg]` variants of the
    # same function — and moving one of them and leaving the other is how a
    # split loses code. Every item with the name goes.
    taken = sorted(span for name in names for span in index[name])
    for name in names:
        if len(index[name]) > 1:
            print(f"note: {name} is {len(index[name])} items; moving all", file=sys.stderr)
    carved = []
    for start, end in taken:
        piece = "\n".join(lines[start : end + 1])
        check_balanced(piece, start)
        carved.append(piece)
    drop = set()
    for start, end in taken:
        drop.update(range(start, end + 1))
    kept = [line for i, line in enumerate(lines) if i not in drop]

    open(source, "w").write(collapse_blank_runs(kept))
    sys.stdout.write("\n\n".join(carved) + "\n")


def check_balanced(piece, start):
    """Refuse a cut whose brackets do not close, which is a misread, not code."""
    depth = 0
    string = None
    for line in piece.split("\n"):
        clean, string = strip_strings(line, string)
        for char in clean:
            if char in "{([":
                depth += 1
            elif char in "})]":
                depth -= 1
    if depth or string is not None:
        raise SystemExit(
            f"line {start + 1}: the item read here does not close "
            f"(bracket depth {depth}). Refusing to cut it."
        )


def collapse_blank_runs(lines):
    out = []
    for line in lines:
        if not line.strip() and out and not out[-1].strip():
            continue
        out.append(line)
    return "\n".join(out)


if __name__ == "__main__":
    main()
