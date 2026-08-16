# Editor architecture decision

Sindri's editor is a native Rust application built with `egui`, `egui-winit`, and
`egui-wgpu`. The editor and game preview share the same `winit` event loop and
`wgpu` device so the viewport can render through the real Sindri runtime without
a browser boundary or a second graphics stack.

The initial shell uses `eframe` to establish the visual language and editor
workflow quickly. Runtime viewport work must use the exposed WGPU render state;
the editor must not create a second device or reimplement scene rendering. Once
the shared desktop platform host exists, Sindri will own the event loop directly
and retain the same `egui` UI layer.

## Dependency direction

```text
sindri-editor
      |
      v
editor commands / protocol
      |
      v
public Sindri engine crates
```

Engine crates never depend on the editor. Editor interactions will become
versioned commands before undo/redo, remote inspection, or AI-assisted actions
are added.

## Visual principles

- Dense, calm workspace rather than default toolkit styling.
- Clear hierarchy between authored content, tools, and runtime status.
- One warm Sindri accent; semantic axis and status colors remain distinct.
- Useful at 1080p, with resizable hierarchy and inspector panels.
- Visual polish is maintained continuously rather than postponed to a rewrite.

## First shell boundary

The editor opens a scene file, displays its entity hierarchy, selects entities,
exposes editable transform values, and renders the prepared Sindri
cube-and-sprite frame into a texture registered with egui. The runtime target
and editor UI share eframe's WGPU device and queue; resizing rebuilds the
viewport's colour and depth targets together through
`sindri_render::ViewportTarget`, which also owns the rule that a target drawn
into through sRGB is sampled through linear.

## Two views of one world

The scene view is where the editor moves around: orbit, pan, zoom, and a choice
of projection, all through `CameraView`. The game view renders the same world
through the authored camera and nothing else, which is the only question it
exists to answer — what would the player see.

That distinction is a tested rule rather than a convention. `camera_for` maps a
tab to a camera, the game tab maps to `CameraView::default()`, and a test holds
it there: an orbit or a pan leaking into the game view would quietly turn it
into a second scene view.

The two share their renderers and textures, because pipelines do not depend on
which camera is looking, and each owns a `ViewportTarget` so egui has a texture
per view. Only the visible one is drawn — rendering the hidden view would spend
a frame's GPU work on something nobody is looking at.

The game view carries no editor chrome: no selection label, no camera hints, no
axis gizmo. A render failure is still reported across it, because a blank view
with no explanation is worse than a view with a message on it.

## Moving the view without moving the scene

Left drag orbits, middle drag or shift-drag pans, and the wheel zooms. All three
go through `CameraView`, so the authored camera stays exactly where the scene put
it and only the editor's view of it moves — which is why a save after looking
around writes nothing. Panning can carry the subject off screen, so there is a
reset control rather than an expectation that the viewer finds their way back.

## Defaults, and what is remembered

Settings survive a launch through eframe's storage: the project browser's
presentation, the viewport projection, and which bottom dock is open, alongside
the window geometry and panel sizes egui persists itself. Anything derived from
the scene, the selection, or the current camera is state rather than preference,
and restoring it would be restoring a moment rather than a choice.

Persistence is what makes a default a small decision. A default only has to be a
reasonable first guess, because disagreeing with it costs one click ever rather
than one per launch, so defaults are chosen on their merits and not to satisfy
whoever complained most recently.

Two of them are worth writing down:

- **The project browser opens as a list.** The grid's tiles show a generic icon
  per file type rather than a picture of the asset, so until a thumbnail is a
  thumbnail the grid spends more space to say less. This flips back when there
  is something to look at.
- **The workspace layout stays viewport-first**, as `design-qa.md` chose. A
  layout that quarters the viewport to fit a second view is a different product
  decision, not a preference, and it belongs behind a named layout rather than
  in the default.

## A scene is a file

The editor takes a path — the demo scene by default, or one named on the command
line — and reads it from disk. A missing or unreadable file is reported in the
interface and the editor opens on the copy compiled into it, so it starts
anywhere while saying what went wrong.

Saving writes the world back through `World::to_scene` and canonical
serialization, which is what makes it safe to offer: saving a scene nobody
edited reproduces the file byte for byte, so a review sees the edit and nothing
else. Reloading re-reads the file and discards unsaved edits along with their
history, because every runtime handle is replaced.

That closes the loop the milestone is judged on — edit a transform, save,
reopen, and the scene is what it was left as — and it is the same file the
runtime and the headless capture load.

A versioned editor/runtime protocol, and editing anything beyond names and
transforms, remain explicit follow-up work.
