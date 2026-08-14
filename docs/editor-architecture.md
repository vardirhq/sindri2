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
- Use familiar editor geography: hierarchy left, scene/game center, inspector
  right, and project/console below the authored workspace.
- One warm Sindri accent, inherited from the legacy editor; semantic axis and
  status colors remain distinct.
- Borrow Unity's proven workspace grammar without copying its visual chrome or
  exposing controls for engine features that do not exist yet.
- Useful at 1080p, with resizable hierarchy and inspector panels.
- Visual polish is maintained continuously rather than postponed to a rewrite.

## First shell boundary

The first editor slice loads the real versioned demo scene, displays its entity
hierarchy, selects entities, exposes editable transform values, and provides an
interactive viewport composition. The shell includes Scene/Game and
Project/Console workspace tabs so future capabilities have stable homes. The
Game tab and asset indexing remain honest placeholders until their runtime
systems land. Scene saving, command-based mutations, undo/redo, and rendering
the actual runtime into that viewport remain explicit follow-up work.
