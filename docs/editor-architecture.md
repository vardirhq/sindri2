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
