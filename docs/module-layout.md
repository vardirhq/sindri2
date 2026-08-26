# Module layout

How a Sindri source file is expected to be sized and split, and what to do when
one outgrows that.

This is a maintenance rule, not an aesthetic one. A module that has grown past a
few hundred lines stops being a unit anyone holds in their head: reviewers read
diffs without the surrounding shape, merges collide inside one file that four
tracks all touch, and the compiler and the test runner lose the granularity that
makes a failure point at something. The limit exists so that growth lands as a
new file rather than as another five hundred lines in an old one.

## The limit

- **400 lines is the target.** A file at or under it needs no justification.
- **600 lines is the cap.** `scripts/check-file-size.py` fails above it, and CI
  runs that script.
- The cap counts everything in the file, tests included. A module whose code is
  small and whose tests are enormous is still a file nobody can navigate.

Both numbers apply to `.rs` files in the main workspace, in `decay/`, in
`editor/`, in `game/`, and in `examples/`. Generated files and vendored code are
exempt and are listed in the script.

Passing the cap is not the goal. A 590-line file that does four unrelated things
is worse than three 200-line files that each do one, and the reviewer is entitled
to say so.

## Split by responsibility, never by line count

The wrong way to get under the cap is to cut a file in half and call the pieces
`foo_1.rs` and `foo_2.rs`, or to move the bottom third into `helpers.rs`. That
keeps every problem the limit exists to prevent and adds an import list.

The right question is: *what are the separate things this file currently does?*
Answer that first, then move whole items to match. In practice the seams are:

- **A phase of a pipeline.** Parsing, lowering, checking, and running are four
  files even when they share types.
- **A surface of a subsystem.** The editor's hierarchy panel, project browser,
  and inspector are three files because they are three panels.
- **A kind of data.** One component family, one command family, one migration
  step per file, so adding the next one is a new file rather than an edit to a
  growing match arm list.
- **A boundary someone crosses.** Serialization, the host FFI seam, the public
  re-export surface.

If no seam is visible, the file is probably fine and the length is telling you
something else — usually that a single function is too long, which is a separate
fix.

## Growth-shaped modules

Several parts of this repository are known to keep growing: Decay statements and
expressions, the builtin catalogue, scene components, scene migrations, editor
panels, and inspector rows. For these, the layout must make the *next* addition
a new file or a new arm in one obvious place, not a rewrite.

The pattern is a thin dispatcher over a directory:

```text
decay-semantic/src/
    lib.rs          re-exports, nothing else
    check/
        mod.rs      the walk, and only the walk
        expr.rs     one function per expression form
        stmt.rs     one function per statement form
        builtins.rs the catalogue and its signatures
```

`mod.rs` holds the traversal and delegates; the leaves hold the cases. Adding a
statement form touches `stmt.rs` and the one match in `mod.rs`. Nothing else in
the crate has to move, and no file has grown.

When a leaf file itself approaches the cap, split it the same way again rather
than starting a second dispatcher.

## Where the module goes

Prefer a directory module over a flat sibling once a subsystem has more than one
file:

```text
src/command/mod.rs      not     src/command.rs
src/command/apply.rs            src/command_apply.rs
src/command/undo.rs             src/command_undo.rs
```

The directory keeps the subsystem's files together on disk and in a file
listing, and it makes the privacy story below work without `pub(crate)`.

`mod.rs` should declare the modules, re-export the public surface, and hold the
type the subsystem is named after — not the implementation. A `mod.rs` doing
real work is the same problem one level up.

## Visibility after a split

Moving an item out of a file changes who can see it. The rule:

- An item used only inside its own directory module is `pub(super)`.
- An item used elsewhere in the crate is `pub(crate)`.
- An item that was already `pub` stays `pub`, and the parent `mod.rs`
  re-exports it so no external path changes.

**A split must not change the crate's public API.** If `use sindri_core::Foo`
worked before, it works after. Re-export from `mod.rs`; do not make callers
learn the new internal path. The same holds for the workspace dependency
directions in `AGENTS.md`: splitting a file never justifies a new edge in the
crate graph, and if it seems to, the split is in the wrong place.

## Where tests go

Unit tests stay with the code they test. After a split, a module's tests move
with the items they exercise, into that module's own `#[cfg(test)] mod tests`.
Tests that cover the subsystem as a whole rather than one file go in a sibling
`tests.rs` inside the directory module.

A child module can see its ancestors' private items, so a directory module's
tests reach everything in it without widening any visibility. Never loosen
visibility to satisfy a test; move the test instead.

Integration tests under `tests/` follow the same cap. Split them by the
behaviour under test — `tests/extraction/sprites.rs`, `tests/extraction/text.rs`
— with a `tests/extraction/main.rs` or a `tests/extraction.rs` declaring the
modules, so `cargo test` still names a real thing when one fails.

Test helpers shared by several test modules go in one `support` module rather
than being copied. A helper copied into a second file is the moment to move it.

## The mechanical part

`scripts/carve.py` moves whole Rust items by name, carrying their doc comments
and attributes, and prints them for the new file:

```bash
# What is in this file, and where.
scripts/carve.py editor/src/native/mod.rs

# The methods of one impl block.
scripts/carve.py editor/src/native/mod.rs --in "impl EditorApp"

# Move three items out into a new module.
scripts/carve.py editor/src/native/mod.rs text_section value_row > /tmp/piece.rs
```

It is a refactoring aid, not part of the build. What it prints is exactly what
it removed: the caller writes the module header, the imports, and the new
visibility. Check its work — `cargo fmt --check` and the compiler catch a bad
cut immediately, and comparing the line counts before and after catches a silent
drop.

## Reviewing a split

A pure split should be readable as one:

1. The moved code is unchanged apart from visibility and imports.
2. No behaviour change rides along. Fix the bug in a separate commit, before or
   after, so the reviewer can see it.
3. Line counts add up. Sum the new files, compare with the old file, and account
   for the difference — headers and imports explain a few dozen lines, not a few
   hundred.
4. The public API is identical. `cargo public-api` is not wired up here, so the
   check is that no caller outside the split needed editing.
5. Every required check in `AGENTS.md` passes, including the WASM target, which
   is where a misplaced `cfg` shows up.

## When the cap is genuinely wrong

Some files resist splitting: a single generated table, a match over an external
enum, a shader's uniform layout. If a file must exceed the cap, add it to the
exemption list in `scripts/check-file-size.py` with a comment saying why. The
list is short on purpose, and an entry is a decision someone reviews, not a way
to opt out.
