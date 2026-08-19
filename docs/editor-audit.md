# Editor audit

What the editor actually does, control by control, as of `5150854`.

`docs/capabilities.md` already carries a shorter version of this, written from
memory of the code. This one was written by reading every path in
`editor/src/`, running the editor under Xvfb, and probing the claims that
looked load-bearing. It found things the shorter list had wrong, which is the
argument for doing it this way.

The point is not to be discouraging. The parts of this editor that work are the
hard parts: every edit goes through the command layer, undo is real and groups
a drag into one step, two live views render the actual runtime frame through
eframe's device, and failures reach the user instead of the log. What follows
is the distance between that and something you could hand to someone.

## The shape of it

Three kinds of problem, in the order they matter.

**It loses work.** Four separate paths discard unsaved edits with no
confirmation, and one of them is a button that reads as "stop playing".

**It refuses work the engine accepts.** A scene carrying any component the
built-in schemas do not know fails to open at all, which is the opposite of
what the format was designed for.

**It says things that are not true.** Twenty controls are drawn and inert. One
of them — the asset search box — accepts typing and filters nothing, which is
worse than a button that visibly does nothing. Play moves a lifecycle state and
runs no game.

Underneath all three is one structural fact: the editor is a faithful *viewer*
of a world and a competent *transform editor*, and almost nothing else. It
cannot create an entity, delete one, add a component, rotate anything, or click
something in the viewport to select it.

## 1. Every control

Verdicts: **works**, **inert** (drawn, nothing behind it), **lies** (appears to
work, doesn't), **partial** (works, with a caveat worth knowing).

### Top bar

| Control | Verdict | Note |
| --- | --- | --- |
| File → Open scene… | works | Native file dialog, loads and adopts on success only |
| File → Save scene | works | Disabled with no path, which is honest |
| File → Reload from disk | **partial** | Works; discards unsaved edits without asking |
| File → Discard changes | works | Does what it says |
| View → Layout | works | Persists across launches |
| Edit, Scene, Build, Tools, Help | **inert** ×5 | Plain buttons, not menus (`native.rs:584`, `591`) |
| Undo / Redo icons | works | Correctly disabled, tooltips name the step |
| Stop | **dangerous** | Calls `reset_to_authored` (`native.rs:619`) — see F1 |
| Pause | **partial** | Moves lifecycle state; nothing is running to pause |
| Play (icon and button) | **partial** | Same; see F5 |
| "isogame ⌄" project name | **inert** | A label with a chevron (`native.rs:645`) |

### Hierarchy

| Control | Verdict | Note |
| --- | --- | --- |
| `+` add entity | **inert** | Response dropped (`native.rs:1218`) |
| Search field | **partial** | Filters by name but keeps each row's depth, so children of filtered-out parents appear indented under nothing |
| Entity rows | works | Selection, with the accent applied to the selected row |
| Click on empty space | works | Clears the selection, as does Escape |
| Drag to reparent | missing | Reparenting exists only in the inspector's menu |
| Right-click / context menu | missing | No delete, duplicate, or rename in place |

### Inspector

| Control | Verdict | Note |
| --- | --- | --- |
| Name field | **partial** | Edits and commits; a name can never be cleared back to unnamed |
| Tag "Untagged" | **inert** | Fixed text (`native.rs:1370`) |
| Layer "Default" | **inert** | Fixed text; also not the render layer, which is a different thing with the same word |
| Parent menu | works | Offers only moves `World::check_set_parent` allows |
| Section chevron and ⋮ | **inert** ×2 | Neither collapses a section nor opens a menu (`native.rs:1393`) |
| Position X/Y/Z | works | Z disabled when the transform declares its Z locked |
| Scale X/Y/Z | works | |
| Rotation | **read-only** | The word "Quaternion" (`native.rs:1410`). Nothing in the editor can turn anything |
| Z lock | works | |
| Component rows | works | Real values from the entity's own payload, read-only |
| Add Component | **inert** | Response dropped (`native.rs:754`) |

### Project and Console dock

| Control | Verdict | Note |
| --- | --- | --- |
| Project / Console tabs | works | Choice persists |
| Folder tree | **inert** | Six hardcoded rows, "Assets" permanently highlighted |
| List / Grid toggle | works | |
| Filter icon | **inert** | (`native.rs:1669`) |
| Asset search box | **lies** | Accepts typing; the list is a fixed array that never sees the needle (`native.rs:1615`) |
| Asset rows and tiles | **inert** | Not selectable, not openable; `demo.scene` is highlighted by string comparison |
| Console | **inert** | Three synthesized lines. Two interpolate real values, so it is a status readout; nothing the engine reports reaches it (`native.rs:1768`) |

### Scene tools and viewport

| Control | Verdict | Note |
| --- | --- | --- |
| Select / Move / Rotate / Scale | **inert** ×4 | `EditorMode` is written and never read anywhere |
| Reset view | works | Correctly disabled when the view has not moved |
| Perspective / Ortho | works | Persists |
| Drag to orbit, shift-drag to pan, wheel to zoom | **partial** | Zoom is clamped to 0.65–1.8 and pitch to ±1.1 rad (`native.rs:411`) |
| Click to select in the viewport | missing | There is no picking |
| Gizmos | missing | |

### Status bar

| Control | Verdict | Note |
| --- | --- | --- |
| Status dot and text | works | Reflects the real problem state |
| File name and unsaved marker | **partial** | See F4 — it can never return to clean without saving |
| "1 Error, 0 Warnings" | **partial** | A boolean dressed as a count |
| Settings gear | **inert** | A label (`native.rs:918`) |

**Twenty inert controls, one that lies, one that is dangerous.**

## 2. What breaks under use

**F1 — Stop throws your work away.** The Stop button calls
`reset_to_authored()`, which rebuilds the world from the file and clears the
history. It sits between Pause and Play, where the universal meaning of that
symbol is "stop playing". Its tooltip says "Stop and reset to the authored
scene", which is accurate and is the only thing standing between a user and
losing an afternoon. Because play mode does not run anything (F5), resetting is
its *only* effect: it cannot be undone, and there is no confirmation.

**F2 — Nothing warns before discarding.** `open_path`, `reload`,
`reset_to_authored`, and closing the window all drop unsaved edits silently.
The roadmap already wants this; it deserves to be first now rather than
someday.

**F3 — The editor refuses scenes the engine is built to carry.** The editor
loads through `DemoScene::load_world`, which validates with
`UnknownComponentPolicy::Reject`. Probed directly: a scene holding
`game.health` alongside a normal camera parses, migrates, and then fails to
open with *"entity 'player' contains unknown component type 'game.health'"*.
`CLAUDE.md` calls `Preserve` "the compatibility default" and `Reject` the
setting "for proofs that require every component to be actionable" — the editor
is a tool, not a proof, and has the strict one. Any project that adds a
component of its own cannot be opened at all.

**F4 — The unsaved marker cannot return to clean.** `undo` and `redo` set
`unsaved = true` unconditionally (`native.rs:491`, `499`). Undoing back to
exactly the saved state still reports unsaved work, so the marker means "you
have touched something", not "the file and the world differ".

**F5 — Play does not run the game.** There is no `EngineHost`, no frame delta,
and no fixed update anywhere in the editor; `toggle_playback` moves
`EngineLifecycle` between states and that is all. The demo scene's own gameplay
— the cube that turns under the arrow keys — never runs. The console line
reading "Engine running" is true about the lifecycle and misleading about
everything else.

**F6 — Missing textures are invisible.** The editor binds exactly the two
textures `sindri_cube::demo_textures` provides. Any other reference draws the
magenta checker with no explanation, even though `sindri_scene::unresolved_textures`
exists to name them and the editor never calls it.

**F7 — The open scene is forgotten.** Nothing remembers the last file. Open a
scene through the dialog, restart, and you are back on the demo. The window
title is always "Sindri Editor", so the only place the open file appears is the
status bar.

## 3. What it cannot express at all

Not bugs — things a person would reach for and not find.

- **Create, delete, or duplicate an entity.** The command layer deliberately
  does not spawn or destroy (a respawned entity invalidates recorded handles),
  so this needs a decision at the core before the editor can offer it. This is
  the one gap in this section that is genuinely engine-blocked.
- **Add or remove a component.** `WorldCommand::SetComponent` and
  `RemoveComponent` already exist, are already undoable, and the editor uses
  neither. This is editor work only.
- **Edit a rotation.** The format stores a quaternion, the renderer applies it,
  `Transform3D::set_rotation_z_radians` exists for the 2D case, and the
  inspector prints the word "Quaternion".
- **Edit a component's fields.** Rows are read-only.
- **Select in the viewport, or select more than one thing.**
- **See the scene from far away.** The zoom clamp cannot frame anything much
  bigger than the demo.
- **Read a project directory.** There is no notion of a project, only a file.

## 4. Readiness for what is coming

Milestone 6 adds sprite sheets, tilemaps, and text, and the roadmap pairs each
with an authoring surface. Four things would each be built three times unless
they are built once first.

**The inspector is a hardcoded match on type name.** `component_rows` matches
`"sindri.camera"`, `"sindri.mesh"`, `"sindri.sprite"` and falls through to a
field list. Every new component adds an arm. Meanwhile
`ComponentSchemaRegistry` — which the extractor already owns and can hand over
— knows the registered types and their metadata. A schema-driven inspector is
the difference between three new panels and three new registrations.

**There is no asset concept.** A sprite sheet editor needs a sheet to open, a
font picker needs fonts to list, and a tile palette needs a tileset. All three
need the project browser to read a directory, which it has never done.

**`EditorMode` is dead state waiting for exactly this.** A tile painter and a
sheet slicer are tool modes. The enum exists, the toolbar draws it, and nothing
reads it — so the wiring is a smaller job than it looks, once there is
something for a mode to mean.

**The editor still depends on `sindri-cube`.** Component schemas and textures
both come from the example. `CLAUDE.md` calls this temporary scaffolding; every
authoring surface built on top of it makes the scaffolding harder to remove.
Sprite sheets are the point where it starts to hurt, because the editor will
need to bind textures the demo has never heard of.

## 5. What I would fix first

Ordered by damage prevented per hour spent. The first four are small.

1. **Stop losing work** (F1, F2). Confirm before any discard, and reconsider
   what Stop should mean while nothing runs.
2. **Open what the engine can open** (F3). `Preserve` instead of `Reject`, so
   unknown components survive a load, an edit, and a save — which the scene
   format already guarantees.
3. **Make the unsaved marker true** (F4). Compare the world against the
   document rather than setting a flag on every command.
4. **Remove or implement the twenty inert controls.** Deleting one is a
   ten-line change and makes the editor more honest immediately; the asset
   search box in particular should either filter or go.
5. **Give the console something real** (F6). Notices, render errors, and
   unresolved textures already exist as values and have nowhere to be seen.
6. **Remember the open scene, and put it in the title** (F7).
7. **Widen the camera limits, and add frame-selected.**

Then the larger builds, in the order the milestone wants them: component add
and remove through the schema registry, rotation editing, viewport picking, and
a project browser that reads a directory.

## What this audit did not cover

Performance (the editor repaints continuously by design, which has not been
measured), anything about how it behaves on Windows or macOS, and the
accessibility of the colour scheme. All three are worth their own pass.
