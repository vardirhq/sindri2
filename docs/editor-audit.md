# Editor audit

What the editor actually does, control by control, as of `5661ea6`.

**The finding that mattered: the editor could not edit anything.** Every path
that writes to the world is behind a selection, and a selection could not be
made, because the hierarchy row handed back the wrong response object. It was a
scene viewer from 16 August until this audit, and nothing noticed — not the
tests, not the screenshots, not the first version of this document.

**Fixed** in the commit that follows this one; §1 keeps the story, because how it
hid for a fortnight is the most useful thing in here. Everything else below
stands as found.

## How this was done, and how the first attempt failed

The first pass read every path in `editor/src/` and wrote down what the code
appeared to do. It produced a plausible document with two categories of error in
it.

**It never ran the thing.** Twelve controls were marked "works" on the strength
of a code reading. One of those — the hierarchy row, the single most important
control in the editor — has never worked at all. Reading found the call to
`.clicked()` and stopped there, without asking what object it was called on.

**It swept controls, not pixels.** Anything painted straight onto the viewport
was outside the net, which is how a static axis gizmo survived and had to be
pointed out afterwards.

So this pass is built differently, and the method is written down because the
next sweep should use it.

1. **Drive the real editor.** Xvfb with `matchbox-window-manager` — without a
   window manager, clicks never reach the app and every probe silently reads as
   "nothing happened" — then `xdotool` for input.
2. **Measure, do not look.** Screenshot before and after each click and diff
   with `compare -metric AE`. A control that changes zero pixels did nothing.
   This is objective, and it catches the controls that only look like they
   responded.
3. **Establish preconditions.** Reset persisted preferences before every run: a
   remembered panel choice silently moves what a coordinate lands on. Locate
   targets in the current frame rather than reusing coordinates from an earlier
   session. Two probes here were invalidated by exactly that and had to be rerun.
4. **Prove mechanisms by patching.** When a control does nothing, change the
   suspected line, rebuild, and re-measure. A fix that makes the control work is
   proof of the cause; an argument about egui's semantics is not.

## 1. The editor was read-only

Every write to the world goes through one of two places, and both sit inside
`inspector_panel`, which returns immediately when nothing is selected
(`native.rs:727`):

- `commit_draft` — name, transform, and the Z lock (`native.rs:761`)
- `reparent` — the parent menu (`native.rs:764`)

Undo and redo need history, and history needs an edit. So every mutation in the
editor depends on a selection.

A selection can only be set in one place, `native.rs:708`, when a hierarchy row
reports a click. It never does:

```rust
fn hierarchy_row(...) -> Response {
    ui.horizontal(|ui| {
        ...
        ui.add(egui::Button::new(...))   // this response is the .inner
    })
    .response                            // this one is the layout's, Sense::hover()
}
```

`ui.horizontal` allocates its region with `Sense::hover()`, so `.clicked()` on
the value it returns is false forever. The button's own response — the one that
knows it was pressed — is `.inner`, and it is discarded.

**Measured.** Clicking the row label, its icon, and the far end of the row each
changed **0 pixels**, in a session where clicking Ortho changed 111,798 and the
Console tab 122,990. Changing `.response` to `.inner`, rebuilding, and clicking
the same pixel changed **35,351**, of which **34,263** were the inspector
filling in. One word.

**The fix, and one thing it taught.** Returning the button's response is enough
to make the name clickable, and the row is more than its name. Wrapping the row
in a scope that senses clicks covers the rest of it — except that a widget
inside such a scope takes precedence over the scope, so the icon's `Label`,
which senses hover, swallowed every click that landed on it. Probing offsets
across the row found a dead band from 10 to 22 pixels in, exactly the icon's
width. The icon now senses clicks too. None of that was visible from reading;
all of it came from measuring.

**Verified end to end.** In the running editor: clicking the row selects it, the
inspector fills, typing `5` into Position X sets it to 5.00, and the status bar
turns to `demo.scene.json (unsaved)`.

**How long.** `hierarchy_row` has returned the layout's response since
`f0e8c41`, the first editor commit, so row clicking has never worked. Until
`548633c` (16 August) the editor preselected `checker-cube` from the demo scene,
so the inspector was always populated and the bug looked like nothing: choosing
a different entity failed the way a misclick does. That commit changed the
initial selection to `None` for good reasons, and turned a latent bug into a
total one.

**What it costs.** Name editing, transform editing, reparenting, undo, redo, and
the Z lock shipped an hour before this audit are all unreachable. So is every
finding below about losing unsaved work: no edit can be made, so there is
nothing to lose. Two serious bugs have been masking each other.

It also means the Z lock's verification was worth less than it looked. An entity
was preselected with a temporary patch to photograph the inspector, which
stepped around the exact bug that makes the feature unreachable. Showing that a
widget draws is not showing that a user can get to it.

## 2. It crashed on a scene file it should open

Starting the editor with a scene carrying a component the built-in schemas do
not know **panics before the window opens**:

```
$ cargo run -p sindri-editor -- custom.scene.json
thread 'main' panicked at editor/src/native.rs:280:14:
  the opened scene must satisfy the demo component schema
```

The file is valid. It parses, it migrates, and the format exists to carry
payloads a runtime does not understand — `CLAUDE.md` calls
`UnknownComponentPolicy::Preserve` "the compatibility default" and `Reject` the
setting for proofs. The editor loads through `DemoScene::load_world`, which uses
`Reject`.

Two paths, two behaviours, both wrong. **From the command line**,
`EditorApp::new` unwraps the failure and the process dies — `open_requested_scene`
carefully handles a file that cannot be *read*, then hands the document to an
`.expect`. **From File → Open scene**, the failure is caught and shown as a
notice, so the editor survives and still refuses the scene.

Any project that defines a component of its own cannot be opened, and opening it
the obvious way crashes.

**Fixed.** The editor loads through its own `SceneExtractor` with `Preserve`, so
a component it has never heard of survives a load, an edit, and a save; the
inspector lists the fields it carries. `EditorApp::new` no longer unwraps a
failed load — it opens on an empty world and says what happened — and a file
that cannot be read now falls back to an empty scene rather than quietly
standing the demo scene in for the one that was asked for. Verified by opening
the file that used to panic: the window comes up, the entity is selectable, and
its unknown component shows as its own section.

It also took the editor's scene loading off the cube example, which is one
strand of the scaffolding §6 wants gone.

## 3. Every control, measured

Pixel deltas come from the live sweep. "hover only" means the sole change was
the pointer highlight: the control is drawn and does nothing when pressed.

| Control | Measured | Verdict |
| --- | --- | --- |
| File menu | 35,984 px | works |
| File → Open / Save / Reload / Discard | — | work |
| View → Layout | — | works, persists |
| Edit, Scene, Build, Tools, Help | 0 px each | **inert** ×5 — Edit is a real undo/redo menu now; the other four are gone |
| Undo / Redo | — | work; correctly disabled when empty |
| Stop | 364 px | resets the scene; see §4 — **fixed**, it now only stops |
| Play / Pause | 364 px | button state only — no frame is advanced |
| Project name "isogame ⌄" | 0 px | **inert** — **removed**; the browser names the real directory |
| Hierarchy `+` | hover only | **inert** — **removed** until spawning is a command |
| **Hierarchy row** | **0 px** | **broken — §1** |
| Hierarchy search | 112,072 px | works; filtered rows keep their indentation, so a match under a hidden parent sits indented under nothing |
| Hierarchy empty space | — | clears a selection that cannot be made |
| Select / Move / Rotate / Scale | 1,848 px each | **worse than inert** — the icon highlights, and `EditorMode` is written and never read. **Removed**, with the enum |
| Reset view | 129,795 px | works |
| Perspective / Ortho | 111,798 px | works, persists |
| Orbit, pan, zoom | 129,835 px | works; zoom clamped to 0.65–1.8, pitch to ±1.1 rad. **Widened**: zoom 0.05–20 and proportional, pitch ±1.5 with the pole guarded in the extractor |
| **Axis gizmo** | **0 px while the scene moved 116,782 px** | **static** — three hardcoded pixel offsets; it cannot turn. **Fixed**: 259 px under the same orbit |
| Viewport click to select | 0 px | no picking |
| Project / Console tabs | 122,990 px | work |
| Grid / List | 154,079 / 384,933 px | work |
| Asset filter icon | hover only | **inert** — **replaced** by a working refresh |
| **Asset search box** | typing draws glyphs (579 px), list unchanged | **lies** — the needle is never read. **Fixed**: 2,651 px, filtering real files |
| Asset rows, folder rows | 0–40 px | **inert**; `demo.scene` is highlighted by string comparison. **Fixed**: real files, the open scene highlighted because it is open, and a scene row opens on a double click |
| Console | — | three synthesized lines; nothing the engine reports reaches it. **Fixed**: a real log, feeding the status bar's counts |
| Inspector: name, transform, parent, Z lock | — | **unreachable** (§1); they work when a selection is forced |
| Inspector: Tag, Layer | — | **inert** fixed text — **removed**; a Sindri entity has neither |
| Inspector: section chevron and ⋮ | — | **inert** ×2 — **removed**, along with the hierarchy root's chevron |
| Inspector: Rotation | — | the word "Quaternion"; no rotation editing exists |
| Inspector: Add Component | — | **inert** — **removed** until it can write through the schema registry |
| Settings gear | 0 px | **inert** — **removed** |

**Nineteen inert controls, four tool modes wired to nothing, two that lie, one
broken, one crash.**

Since fixed: everything above is either working or gone, except the four that
are their own builds — Play and Pause advancing no frame, the console, viewport
picking, and rotation editing — which §7 keeps.

## 4. What else breaks

**Stop discards everything, silently.** Stop calls `reset_to_authored()`
(`native.rs:619`), which rebuilds the world and clears the history. It sits
between Pause and Play, where that symbol means "stop playing". Unreachable
damage today, because no edit can be made; the moment §1 is fixed it becomes the
sharpest edge in the editor. **Fixed:** the button stops the lifecycle and
nothing else, and is enabled only while something is running.

**Nothing warns before discarding.** `open_path`, `reload`, `reset_to_authored`,
and closing the window all drop unsaved edits without asking. **Fixed:** each of
the four asks first, naming what it is about to do, and offers to save instead.
Closing cancels the window's close request while the question stands.

**The unsaved marker cannot return to clean.** `undo` and `redo` set
`unsaved = true` unconditionally (`native.rs:491`, `499`), so undoing back to the
saved state still claims unsaved work. **Fixed:** the history numbers the state
the world is in, the editor remembers the number it saved, and "unsaved" is the
two differing. Undoing back to the saved state is clean again, and a state left
behind is never numbered twice.

**Ctrl+Shift+Z undoes.** Found while checking the above: egui ignores an extra
Shift when matching a shortcut, so Ctrl+Shift+Z matched the undo binding, which
was tested first and consumed the key. Redo is now asked for first, with a test
that presses the keys through a real frame.

**Missing textures are silent.** The editor binds the two textures
`sindri_cube::demo_textures` provides; anything else draws the magenta checker
with no explanation, though `sindri_scene::unresolved_textures` exists to name
them. **Fixed:** every scene announces its unresolved references in the console
as it opens.

**The open scene is forgotten.** Nothing remembers the last file, and the window
title is always "Sindri Editor". **Fixed:** both.

**Write-only state.** `mode` and `asset_search` are the only two `EditorApp`
fields whose every reference is a write, and each corresponds to a control that
appears to work. **`asset_search` is read now**; `mode` still is not.

## 5. Engine capability the editor does not use

Counted mechanically against `editor/src/`.

| Capability | Editor uses | What it would give |
| --- | --- | --- |
| `WorldCommand::SetComponent` / `RemoveComponent` | 0 | Add Component, already undoable |
| `ComponentSchemaRegistry`, `query` | 0 | An inspector driven by schemas rather than a `match` on three type names |
| `unresolved_textures` | 0 | Naming the textures a scene references and nothing binds |
| `World::despawn_recursive`, `spawn` | 0 | Delete and create |
| `World::assign_missing_source_ids` | 0 | **Needed before create ships**: `to_scene` refuses a runtime-spawned entity with no stable ID, so the first new entity would make the scene unsaveable |
| `EngineCore`, `FixedStepConfig` | 0 | A play mode that runs |
| `Transform3D::position_2d` and the rest | 0 | The inspector writes `position[2]` directly, which is the pattern the 2D accessors exist to replace |

## 6. Readiness for Milestone 6's authoring surfaces

Sprite sheets, tilemaps, and text each want a panel. Four things would be built
three times otherwise.

**The inspector is a `match` on three type names.** Every new component adds an
arm. The schema registry is right there, unused.

**There is no asset concept.** A sheet slicer needs a sheet, a font picker needs
fonts, a tile palette needs a tileset; all three need a browser that reads a
directory. **The browser reads one now**, which is the first half; picking an
asset for a component field is the second.

**`EditorMode` was dead state waiting for exactly this.** A tile painter is a
tool mode. The enum and the toolbar existed and nothing read them, so both are
gone — a mode enum is four lines, and keeping a toolbar of promises for a
milestone that has not started is how the editor got here. Milestone 6 brings
back the rail with tools behind it.

**The editor still depends on `sindri-cube`** for component schemas and
textures. Sprite sheets are where that starts to hurt, because the editor will
need to bind textures the demo has never heard of.

## 7. What to fix, in order

1. ~~**`.inner` instead of `.response`** in `hierarchy_row`, with a test that
   clicking a row selects an entity. One word, and it unblocks every editing
   feature in the tool.~~ Done — and the row answers across its whole width.
2. ~~**Do not panic on a scene that parses.** Handle the failure `EditorApp::new`
   unwraps, and open with `Preserve` so components the editor does not know
   survive a load, an edit, and a save.~~ Done.
3. ~~**Stop losing work.** Confirm before discarding, and reconsider what Stop
   means while nothing runs.~~ Done.
4. ~~**Make the unsaved marker true.**~~ Done.
5. ~~**Remove or implement** the nineteen inert controls, the four dead tool
   modes, and the two that lie.~~ Done. The two that lied were implemented —
   the axis indicator is drawn from the camera's own view, and the asset search
   filters a browser that reads a real directory. The rest were removed, which
   for the tool modes took `EditorMode` with them and for "Add Component" and
   the hierarchy's `+` means waiting on builds items 8 and 9 name.
6. ~~**Give the console something real**: notices, render failures, unresolved
   textures.~~ Done, and bounded, with a repeated message collapsed into a count
   so a per-frame render failure cannot bury what explains it.
7. ~~**Remember the open scene**, name it in the title, and widen the camera
   limits.~~ Done. Framing the selection came with the camera work, and the
   orbit is now incapable of reaching the pole.

Then the larger builds: spawning and despawning as commands, so creating and
deleting an entity is undoable and a new entity gets a stable ID before the
scene is saved; component add and remove through the schema registry; rotation
editing; and viewport picking.

## What this audit still does not cover

Windows and macOS behaviour, repaint cost, colour contrast, and what the
hierarchy does with hundreds of entities. The live method above extends to all
four.
