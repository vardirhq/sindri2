# Editor authoring audit

Can Gather be built in the editor, as of `4dd70b4`?

**No.** Four things stop it outright, one of them writes over your scene file,
and a fifth — the complete absence of right-click menus — is why several of the
others have nowhere to be fixed.

**Fixing has started.** §1, §2 and §3 are done, and each keeps its finding here
because how it hid is the useful part. What is left is §4 onwards. This audit is the second of its kind; `docs/editor-audit.md` asked whether
the controls did anything, and this one asks whether the controls that work add
up to a tool you could author a game in.

The test is deliberately concrete. Gather is the companion game, its scene is in
the repository, and every capability the engine claims is exercised by it. So
the question is not "what is missing in the abstract" but: open the editor on an
empty project and try to arrive at `game/assets/gather.scene.json`. Everywhere
that walk hits a wall is a finding.

## Method

The same one `docs/editor-audit.md` established, and for the same reason: the
first version of that document was a code reading, and it marked twelve controls
"works" that did not.

- Drive the real editor under Xvfb with `xdotool`, on both shipped scenes.
- Reach states a screenshot of the idle editor never shows: an entity selected,
  a component added, a menu open, play mode running.
- Prove a claim by producing the artefact. The play-mode finding below is not an
  argument about what `save` does; it is a scene file with a spun cube in it.
- Read the code afterwards to name the mechanism, not to find the symptom.

Where a finding says "confirmed", it was reproduced in a running editor.

---

## 1. A component you add is not the component the game uses

**Blocker.** The most consequential finding, and the one with the widest blast
radius, because it is invisible: nothing looks broken.

**Fixed.** The registry now records two payloads and they are different
questions: a *field template* saying what a component has, and a *default
payload* saying what a fresh one is. The inspector draws from the first, so a
type with no honest blank still shows all of its fields. Both are checked at
registration against the field list serde will ask the type for, so a template
that drifts from its struct is a startup error — which is how the tilemap's
missing `projection` was found the moment the check existed. See
`docs/component-schema-registry.md`.

The inspector draws a component's fields from the *registry's default payload*,
filled out with whatever the instance stored (`editor/src/inspector/fields.rs:25`).
That default is the only statement anywhere of "which fields this component
has". A component registered without one falls through to
`return payload.clone()` — so its rows are whatever keys happen to be in the
file, and no others.

Four component types are registered without a default
(`crates/sindri-scene/src/extract/mod.rs:134,138,159,193`), and the editor
completes two of them itself with a partial payload
(`editor/src/native/inspector_panel/draft.rs:129`).

| Component | Addable | Fields you get | Fields you can never reach |
| --- | --- | --- | --- |
| `sindri.ui.text` | yes | `text`, `font` | `font_size`, `line_height`, `color`, `anchor`, `layer` |
| `sindri.tilemap` | yes | `texture`, `palette`, `columns`, `rows`, `tiles` | `projection`, `tile_size`, `tint`, `layer` |
| `sindri.animation.sprite` | only if the entity's sheet is sliced | `clips`, `playing`, `speed` | — |
| `sindri.grid.occupant` | only if a tilemap entity exists | `grid`, `footprint` | — |
| `sindri.script` | **no** | — | all of them |
| `sindri.audio.source` | **no** | — | all of them |

Confirmed both ways. Gather's `title` entity shows seven rows in the inspector,
including its 22pt size, its amber colour, and its `top` anchor. A UI Text added
in the editor shows two: a text box and a font picker. Same component, same
panel, two different inspectors — because one of them was written by hand.

Two consequences worth stating separately:

- **Gather's board can never be built.** It is an isometric tilemap. A tilemap
  added in the editor has no Projection row, so it is flat forever.
- **A new tilemap arrives broken.** Its default texture is
  `procedural:checkerboard`, and the palette immediately says
  "procedural:checkerboard is not a file-backed texture". The first thing the
  component does is report an error about a value the editor chose for it.

The fix is not per-component. It is that "which fields does this component have"
must come from the schema rather than from a default payload someone remembered
to write, and that a type with no honest default is a type whose Add Component
entry needs a reason shown, not an absence.

## 2. You cannot add a script to an entity

**Blocker**, and the sharpest single gap in the tool, because scripting is the
headline capability.

**Fixed.** Both Script and Audio Source have field templates now, and the editor
completes each from the project beside the scene — the first `.decay` source
that declares a container, the first audio clip — exactly as it already did for
a font. A script added this way arrives with its source, its container, its
typed `@export` fields, and `enabled`, which was never visible before.

Making Script addable turned up a second bug behind it: `declared_space` treated
every non-UI component as a world component, so adding a script to a fresh
entity silently decided it was a world object and the menu stopped offering UI
Text. Gather's banner and pips are UI images driven by scripts, so the editor
could not have built its HUD. Only a component that *places* something — a
camera, a mesh, a sprite, a tilemap — decides a space now.

`ScriptComponent` is registered without a default
(`editor/src/native/scene_io.rs:89`), so `component_default` returns `None` and
`addable_components` filters it out. Confirmed: Add Component on a fresh entity
offers exactly eight types — Camera, Mesh, Collider 2D, Rigid Body 2D, Sprite,
Tilemap, UI Image, UI Text. Script is not among them. Neither is Audio Source.

Gather is thirteen `sindri.script` components and one `sindri.audio.source`.
Every one of them would have to be typed into JSON by hand.

The editor has excellent *support* for scripts that already exist: the source
picker offers what the project holds, the container picker offers what the
source declares, and `@export` properties are drawn as typed fields. All of it
is unreachable, because there is no way to get the component onto an entity.

Related: `ScriptComponent.enabled` exists and is never shown, for the reason in
§1. There is no way to disable a script without deleting it.

## 3. Saving while the scene is playing overwrites the file

**Sharp edge, and the only finding here that destroys work.**

**Fixed.** A running scene is not the document, and the editor says so in one
place: `authoring_allowed` answers for the transport, and the inspector, the
hierarchy's create and delete, the gizmo, the tile brush, undo, redo and every
File entry ask it. Each disabled control explains itself on hover; a refused
save says so in the console. What still works is everything that does not write:
the viewport orbits and selects, and the inspector shows the values changing.
Editing a running scene and *keeping* the changes needs history that can be
rebased or a play mode against a copy, and is not this.

`save()` writes `self.world` (`editor/src/native/scene_io.rs:111`), and during
play `self.world` is the running world that scripts have been moving. Nothing
gates saving on the transport.

Confirmed with a copy of the fixture project: press Play, wait four seconds,
File → Save scene. The cube's authored rotation goes from the identity
`[0, 0, 0, 1]` to `[0, 0, -0.8387, -0.5446]` — about 113° of `spin.decay` — and
that is now what is on disk.

It gets worse on the way out. Stop restores the pre-play snapshot
(`editor/src/native/runtime.rs:264`), so afterwards the editor shows the
authored pose while the file holds the spun one, and `saved_revision` was set by
the save, so the status bar says there is nothing to save. Every signal the
editor gives is that the work is safe.

The same mechanism silently discards edits made *while* playing: the snapshot
replaces them, and the history keeps the transactions, so undo is now describing
a world that no longer contains them. `docs/editor-audit.md` §4 recorded "Stop
discards everything, silently" and fixed it for the old meaning of Stop; this is
the same class of bug at the new boundary.

Nothing about play mode is read-only: the inspector, Add Component, Delete,
the gizmos, and Save are all live. The only thing play mode changes is that the
world underneath them is temporary.

## 4. The Project browser is a listing, not a browser

**Missing basics.** Four separate gaps, three of which you named.

**Three and three-quarters of the four fixed.** Folders fold, the folder pane navigates,
and the browser has a selection of its own — marked with the band a selected row wears,
while the open scene keeps a quieter rule in the margin, because "the scene I
have open" and "the thing I am pointing at" are different facts. Every row
answers a click now, including the ones the editor can do nothing else with,
because a row that cannot be selected cannot carry a right-click menu either.
The fourth gap is mostly closed: the file operations are done — make a folder,
make a script, rename, copy, import, delete — and every text file the browser
lists can be read in the inspector. What is left is a preview for the two kinds
that are not text: hearing an audio clip, and seeing a font.

**Folders do not open or close.** `ProjectTree` produces one flat, sorted, fully
expanded list with a `depth` per row (`editor/src/project/mod.rs`). There is no
fold state for directories anywhere — `expanded_sheets` exists, but only for a
sliced texture's sprites. A project with four asset folders is a wall of every
file in all of them.

**The folder pane does nothing.** In the Wide layout the browser draws a folder
tree beside the asset list. Its rows are `ui.label`s with no sense
(`editor/src/ui/widgets/asset.rs:211`); clicking one selects nothing and filters
nothing. The asset list always shows the whole tree regardless. It is decoration
that looks like navigation.

**Selection is not shown, and the wrong row is highlighted.** A row's "current"
flag is `open.is_some_and(|path| path == entry.path)` where `open` is *the open
scene file* (`editor/src/native/project_panel/row.rs:118`). So `fixture.scene.json`
wears the selection band permanently, and selecting a texture — which does
something real, it opens the slicer — marks nothing at all. Confirmed: with
`spin.png` open in the slicer, the highlighted row is still the scene.

**Most files do nothing when clicked.** Only Scene and Texture rows were
interactive (`editor/src/native/project_panel/row.rs:103`). Scripts, fonts,
audio, meshes and everything else were inert:

- ~~`.decay` scripts cannot be opened, previewed, or created.~~ **Fixed.**
  Selecting a text file shows it in the inspector, the same way selecting an
  image opens the slicer — the file, its line count, and its source in a
  monospace column that scrolls both ways, because source is written in columns
  and wrapping it puts a continuation where a statement was. Read-only, and the
  panel says so: an editor that opens a script in a text box is promising to be
  a code editor — syntax, errors at the line they are on, find, an undo stack of
  its own — and half of that is worse than none. What it answers is the question
  the browser could not, which is what is in this file. It covers every text
  file the browser lists, not only `.decay`: a scene, a sheet, a README, a
  `.toml`. And **New script here** writes one that compiles and does nothing,
  because a file that reports an error before anyone has typed a line of it is a
  worse start than an empty one.
- **Audio has no preview.** You cannot hear a clip before naming it in a
  component. Still open: it needs playback on demand rather than through a
  running scene.
- **Fonts have no preview.** You pick a font by filename. Still open: it needs
  the project's font loaded into the editor's own text stack to draw a sample,
  and reading a `.ttf` as text says nothing.

~~And there are no file operations of any kind: no create, no folder, no rename,
no delete, no duplicate, no reveal-in-file-manager, no import.~~ **Fixed**, bar
reveal-in-file-manager. A row's menu makes a folder, renames in place, copies
beside itself, imports files from anywhere, and deletes — and the browser
re-reads the directory itself afterwards, so Refresh is for changes made outside
it rather than for the editor's own.

None of these go through the undo history, and that is not an oversight: the
history describes a world and these describe a directory, so undoing a delete
would mean the editor holding the bytes of every file anyone removed for as long
as the session lasted. What keeps them honest instead is that each is checked
before it runs (`editor/src/project/ops.rs`) and refuses rather than overwrites,
that nothing can name a path outside the project — a browser row hands over
whatever was typed into it, and `../../etc/hosts` is a perfectly good string —
and that the one operation with nothing behind it asks first.

Renaming the open scene follows it, because the editor holds the path it saves
to and a rename it was not told about would write the scene back under its old
name and leave two of them on disk. A copy keeps the whole suffix that says what
kind of asset it is: `Path::file_stem` stops at the last dot, so a duplicated
scene would otherwise become `level.scene copy.json`, which the browser no
longer reads as a scene.

Delete and F2 act on whichever selection was made last, so the browser's menu
can print its keys honestly — the editor holds two selections, and until now
those keys always meant the entity.

What is still open in this section is opening and previewing what the browser
lists.

## 5. The hierarchy is missing most of its verbs

**Missing basics.** What it does: create empty, create child, create UI image,
select, delete the selection, drag to reparent, fold, filter by name.

**Three of the five fixed.** Duplicate, rename in place, and delete by keyboard
all exist now, each reachable three ways: the row's own right-click menu (§6),
its key, and — for rename — a double click. Multi-select and sibling reorder are
still open, and are the two that change what a *selection* is rather than what
one entity can be told to do.

What it does not:

- ~~**Duplicate.**~~ **Fixed.** Ctrl+D, or Duplicate in the row's menu. A copy
  takes everything under it, keeps the original's parent so it appears as a
  sibling, earns a stable ID nothing else is using (`orb-1-copy`, then
  `orb-1-copy-2`), and undoes in one step. The handles are the interesting
  part: `WorldCommand::Spawn` names the handle it spawns at and
  `World::next_handle` is a peek rather than an allocation, so the copy is
  rehearsed against a clone of the world to learn the handles the real one is
  about to hand out (`editor/src/native/editing/duplicate.rs`). Copy/paste of
  entities or of components is still open.
- ~~**Rename in place.**~~ **Fixed.** Double-click a row or press F2, and the
  name becomes a field in the row itself, focused the frame it appears. Enter
  commits through the same `SetName` command the inspector writes; Escape
  abandons it. Gather has forty entities and fixing one name should not move
  your eyes to another panel.
- ~~**Delete by keyboard.**~~ **Fixed.** Delete, or Backspace — the key a Mac
  keyboard labels "delete". The header icon stays, because a key nobody has
  been told about is not a discoverable verb.
- **Multi-select.** One entity at a time, so no bulk move, delete, or reparent.
- **Reorder siblings.** Order is `source_id` sorted (`hierarchy_sort_key`), so
  authoring order is alphabetical by an ID you cannot see or set.

Most of these are the actions a right-click would offer, which is §6.

## 6. Nothing has a right-click menu

**Missing basic**, and the one that makes several of the others feel worse than
they are.

**Partly fixed.** The two panels that list things have menus now: a hierarchy
row and a project row. `ui::widgets::menu` is where the shape lives — a fixed
width so a menu does not resize with the name of whatever is selected, a subject
line naming what the entries act on, entries that say their key, and a
destructive entry that reads as destructive. The other six surfaces in the table
below are still open, and the two notes under it still apply to whoever builds
them.

There is not a single context menu in the editor. `grep -rn context_menu
editor/src` returns nothing. Every action the tool has is a toolbar icon, a
top-level menu, or a control inside the inspector — which means the actions that
belong to *a specific thing* have nowhere to live, and so mostly do not exist.
§5 is the visible half of this: duplicate, rename, and delete are missing partly
because there was no obvious place to put them.

Where a right-click is expected and does nothing today:

| Where | What belongs there | State |
| --- | --- | --- |
| A hierarchy row | Rename, Duplicate, Delete, Create child, Copy/Paste, Focus (F), Move to top level | **Done** but for copy/paste and move-to-top-level |
| Hierarchy empty space | Create Empty, Create UI Image, Paste | Open |
| A project row | Open, Slice (a texture), Rename, Delete, Duplicate, Reveal in file manager | **Partly**: open, slice, and the two paths worth copying. Rename, delete and duplicate are file operations, which is the rest of §4 |
| Project empty space | New folder, Import, Refresh | Open |
| A component heading | Remove, Reset to default, Copy/Paste values, Move up/down | Open |
| A property row | Reset this field to its default, Copy value | Open |
| The Scene view | Frame selected, Frame all, Create at this point, Paste | Open |
| A console line | Copy message, Select the entity it names, Clear | Open |

Two things worth knowing before this is built, both found while checking it:

**The viewport's right button is already taken — but only its drag.** Secondary
drag orbits the scene camera (`editor/src/native/camera.rs:452`). A secondary
*click* does nothing at all, and egui distinguishes the two, so a viewport
context menu and the orbit can coexist. Whoever adds it should keep that
distinction deliberate rather than discovering it.

**Half the project browser cannot receive a right-click at all.** *(Fixed with
§4: every row is `Sense::click()` now, and the panel rather than the row decides
what pressing one does.)* A row that the
editor can do nothing with is given `Sense::hover()` rather than `Sense::click()`
(`editor/src/native/project_panel/row.rs:103`), which is a deliberate signal:
the comment beside it says a listing that lists is not the same as a control that
looks like it does something. That reasoning was sound when the only verb was
"open". It stops being sound the moment right-click offers Rename and Delete,
which every row should have. The sense rule has to change with it, and the
"is there anything to do here" signal has to move somewhere else — the hover
tooltip, or the absence of a primary-click response.

## 7. Half of a scene cannot be reached in the viewport

**Missing basic**, with one case that is actively misleading — and, found while
fixing it, one that was worse than this section knew.

**Fixed**, except for UI text. UI images are picked in a pass of their own
against the same overlay matrix the frame draws them through, and a UI element's
gizmo is drawn where the element is. UI text is deliberately not picked: what a
string covers is decided by glyph layout inside the text renderer, and a guessed
box for it would select the wrong thing near its edges. Its gizmo is now correct,
so it is still reachable from the hierarchy and movable in the view.

**Nothing at all could be clicked, in fact.** Found while fixing the rest, and
not visible from either side of it. The Scene view allocated its region with
`Sense::drag()`, and egui sets a response's clicked flag only for a widget whose
sense includes clicks — so `select_viewport_click`'s `clicked_by` was always
false, and no click anywhere in the viewport selected anything, however correct
the picking underneath. The tile brush was half-dead the same way: it painted on
a drag and ignored a single click. The coupling between "what the response
senses" and "what the panel then asks it" is stated in a test now, because it is
invisible from both ends.

**A transparent element swallowed every click.** Also found while fixing this,
by clicking Gather's player and selecting its win banner instead: the banner is
`tint` alpha zero, a third of the viewport wide, sitting in the middle of the
scene until the game says otherwise. A thing drawn as nothing is not a thing to
click, in the overlay or in the world, and picking skips both now. Exactly zero
rather than a threshold — anything above it is visible, however faintly, and a
faint thing is still something someone aimed at.

Picking covers meshes, world sprites, and filled tilemap cells
(`editor/src/picking.rs:24`), plus authored cameras through their overlay
markers. It did not cover UI images, UI text, or entities with no drawable
component. Twelve of Gather's twenty-two entities are UI: none of them could be
clicked in the view that draws them.

Worse, the gizmo still appeared for them, in the wrong place. A UI element's
position is an offset from its anchor in overlay space
(`crates/sindri-scene/src/extract/ui.rs:19`), and the gizmo was drawn at the
entity's transform in *world* space. Confirmed: select Gather's `title`, choose
Move, and the handle was a single red arm in the bottom-left corner of the Scene
view, mostly off screen, while the text it belongs to is at the top. Dragging it
did change the right numbers. Nothing about where it was drawn said so. The
gizmo is told where its handles belong now, separately from the transform it
edits, so the pointer maths happens against the drawn origin and the answer
lands on the authored value. A UI element is offered two arms rather than three:
its Z orders it within the overlay rather than placing it.

## 8. Scene-level authoring has no home

**Missing basics.** There is no place in the editor that is about the scene
rather than about an entity in it.

**The two that blocked starting a project are fixed.** A scene can be made and a
scene can be forked, so the editor no longer requires a project someone else
started. What is left in this section is a *surface* — a place that is about the
scene — which the remaining four items all want and none of them has.

- ~~**No New Scene.**~~ **Fixed.** File → New scene…, or Ctrl+N. It asks where
  the scene goes, writes it, and opens it through the ordinary path, so a new
  scene proves it loads before anyone works in it and everything a scene brings
  with it — the project beside it, its textures, its scripts — is arranged by
  the code that already knows how. A save box takes a name rather than an
  extension, so the suffix is the editor's: `level` becomes
  `level.scene.json`, which is what the browser lists as a scene and what
  reopening it finds. It contains one world camera, because a scene with none
  is a legal scene and a black Game view, and "why is the game view empty" is
  not the first question a new project should raise.
- ~~**No Save As.**~~ **Fixed.** File → Save scene as…, or Ctrl+Shift+S, and
  offered whether or not the scene has a file — giving a detached scene one is
  the case that had no way out at all. The path is adopted after the write, so
  a save that fails leaves the scene attached to the file it was attached to,
  and the project, the remembered scene, the textures and the scripts all
  follow the scene to its new directory.
- ~~**The scene's name is invisible.**~~ **Fixed.** `SceneMetadata.name` is
  `"Gather"` in the shipped scene and round-trips through a save, and nothing
  showed or edited it. The inspector with nothing selected shows the scene
  instead of a shrug: its name, editable; the file it is written to; and how
  many entities it holds. The rename goes through the command history like
  every other edit — not because renaming a scene is dangerous, but because the
  editor decides whether a document is unsaved by watching the history, and a
  change made outside it is one it would let someone close the window on.
- ~~**An entity's stable ID is invisible.**~~ **Fixed.** `source_id` is what a
  scene stores, what `sindri.grid.occupant` references, and what sibling order
  is derived from — and it was shown nowhere, so the editor could produce
  `game-object-N` and nothing else while Gather's are `player`, `floor`,
  `orb-1`. It is a field under the name now. Renaming one takes every occupant
  that points at it along in the same transaction, because a stable ID is a
  reference and not a label: renaming a grid without rewriting its occupants
  leaves a scene that still opens with nothing on the board. An ID that is
  blank or already taken is refused at the field, in the colour the editor uses
  for a refusal, rather than written and rejected — the draft is committed every
  frame, so a refused command would be refused again on the next one.
- **No snapping settings.** The snap toggle's tooltip names its increments —
  0.5 units, 15°, 0.1 scale — and they are constants
  (`editor/src/gizmo.rs:85`). Nothing can change them.
- **No preferences surface.** Layout is in the View menu; everything else the
  editor remembers is invisible.

## 9. Smaller things, confirmed

- ~~**Adding a second camera breaks the scene in one click.**~~ **Fixed.** Add
  Component offered Camera whether or not the scene already had one, and a
  second authored camera is a hard extract error: both viewports show "the scene
  contains more than one authored world camera", and nothing says which two
  entities are the cameras. The error handling was good; the offer should not
  have been made. Camera is listed and disabled on a scene that already has one,
  saying so on hover.
- ~~**Add Component says nothing about what it is not offering.**~~ **Fixed.**
  Every type the entity's space accepts is listed now, and one that cannot be
  added yet is disabled with the reason — "No font in the project beside this
  scene", "Slice an image into sprites first", "Add a Tilemap to this entity
  first". The old rule stands where it belongs: a button that adds a component
  the engine then rejects is worse than no button. An entry that says *why* is
  neither of those. Only the other space's components are still left out
  entirely, because a menu twice as long saying "no" to half of itself explains
  nothing.
- ~~**A file's kind is signalled by the wrong icon.**~~ **Fixed.** A `.txt` drew
  with the 3D-box glyph the editor uses for an entity, claiming the file is an
  object in the scene; it is a sheet of paper now. A font shared the image glyph
  with a texture, so a project's typefaces and its sprite sheets were the same
  row with different words after them; it has its own.
- **No entity enable/disable.** `EntityData` has no such field, so this is an
  engine gap rather than an editor one, but it is a basic an author will look
  for early.
- **The console cannot be filtered.** No level filter, and no way to get from an
  error naming an entity to that entity.
- **No undo history view**, so "what will Ctrl+Z do" is answerable only from the
  Edit menu's label.

---

## What to fix, in order

The ordering is by what unblocks the most authoring, not by effort.

1. ~~**Make Script and Audio Source addable**, and give every component a field
   set derived from its schema rather than from a hand-written default.~~ Done.
   The registry answers "what does this component have" separately from "what is
   a fresh one", and a template that drifts from its struct is a startup error.
2. ~~**Stop play mode from writing to the file.**~~ Done. A running scene is not
   the document, and one rule says so for every control that writes.
3. ~~**Give the Project browser selection and folding**, and make the folder
   pane filter the list.~~ Done. What is left in §4 is opening and previewing
   what it lists, and file operations.
4. ~~**Duplicate, rename in place, and Delete**, reached from a right-click menu
   on the thing they act on.~~ Done. All three exist on the hierarchy, each
   reachable from the row's menu and from its key, and the project browser has
   a menu of its own for what it can already do. What is still open from §5 is
   multi-select and sibling reorder, which change what a *selection* is rather
   than what one entity can be told to do.
5. ~~**New Scene and Save As**, so the editor can start a project rather than
   only continue one.~~ Done. What is left in §8 is a surface that is about the
   scene rather than about an entity in it: the scene's name, an entity's stable
   ID, the snapping increments, and everything else the editor remembers.
6. ~~**Pick UI elements in the viewport**, and either draw their gizmo where the
   element is or do not draw one.~~ Done, except for UI text, which needs glyph
   metrics the editor does not have. Two things nobody had noticed came out
   with it: the viewport sensed drags but not clicks, so *nothing* in it could
   be selected; and a fully transparent element swallowed every click over it.

7. ~~**File operations in the project browser**, so a project can be built
   without a file manager beside the window, and a way to look at what it
   lists.~~ Done. Every text file the browser lists opens in the inspector, and
   a script can be made there. What is left in §4 is a preview for the two kinds
   that are not text: an audio clip needs playback on demand, and a font needs
   loading into the editor's own text stack to draw a sample.

## What this audit does not cover

Performance with a large scene, Windows and macOS behaviour, the physics
components (no shipped scene uses them, so "added ≠ authored" is untested
there), and whether the Decay authoring loop is pleasant once a script component
can be created at all.
