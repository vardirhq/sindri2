# Editor usability audit

> **Snapshot:** audit at `5e57ae3`, driven against `games/voxel-lab` in the
> Canvas and Wide arrangements at 1920×1080 and 1366×768. It records what was
> found. Use `capabilities.md` and `parity.md` for current behaviour once fixes
> land.

Prompted by four reports from using the editor on Voxel Lab: the voxel world
"gets stuck in one view" after changing Voxel World or Environment settings; the
Wide arrangement is buggy; panel content is cut off; and the inspector and
console are hard to work with. All four reproduce, and the first has a single
mechanism.

## Method

The method `editor-audit.md` established: run the real editor under Xvfb with a
window manager, drive it with `xdotool`, screenshot, and confirm mechanisms in
the source rather than inferring them from a reading. Preferences were reset
before the first run. Two displays were used, because the window manager keeps
the editor full-screen and a "resized" window is otherwise a cropped one.

## 1. An invalid edit freezes the viewport on its last good frame

**Reproduced.** Select Environment, drag Bloom → Intensity below zero. The
status bar changes to "Something went wrong", the console gains an error, and
the Scene view stops changing: six zoom steps later the image is pixel-identical.
The camera frustum, gizmos and selection outlines still move, because the editor
paints them over the viewport texture; the rendered scene underneath does not.
This is the voxel world "staying put while the camera moves". Undo resumes
rendering.

**Mechanism.** Extraction is all-or-nothing:

- `SceneExtractor::extract_*` calls `environment_clear(world)?` and
  `push_voxel_worlds(...)?` (`crates/sindri-scene/src/extract/mod.rs:262`,
  `:264`, `:319`, `:321`). One invalid Environment or Voxel World fails the
  whole frame.
- `Viewport::render` returns on that error before encoding anything
  (`editor/src/native/viewport.rs:113-126`), so the viewport texture keeps the
  previous frame and `render_view` paints it again.

**What triggers it in ordinary use.** None of these need an unusual edit:

| Edit | Why it fails |
| --- | --- |
| Voxel World → Materials → **+** | The new item is a copy of the template's first material, `voxel: 1`, which every world already has (`list.rs:37`, `blank_item`). Duplicate IDs are refused. Adding a material *always* freezes the view. |
| Voxel World → Materials → **−** on a material the generator uses | `MissingVoxelMaterial`. |
| Dragging Surface / Subsurface / Deep voxel | Integers drag at one per pixel through IDs no material defines, each one a failed frame and a distinct console line. The "material ID 22, 23, 25, 26…" errors in the report are one drag. |
| Environment → Shadows → Map size | Only 256/512/1024/2048 are valid; any drag of it fails. |
| Environment → Bloom knee/passes/intensity, Fog distance/density/falloff, Post-process ranges, AO strength | Validated ranges, unclamped widgets. |
| Voxel World → Generator → Kind | A free-text field over an enum tag; any keystroke is an invalid payload. |
| Voxel World → Render/Vertical radius, Height variation | Upper bounds exist in the engine; the widget has none. Unsigned fields also drag below zero, which fails deserialization. |

**Fix, in three layers.** Each is worth doing on its own.

1. *Prevent*: the inspector clamps and constrains (section 3).
2. *Degrade*: extraction reports per component instead of failing the frame.
   An invalid Voxel World is skipped, its last good meshes kept or released,
   and the rest of the scene drawn; an invalid Environment falls back to the
   last valid one. The failure becomes a diagnostic *about an entity*, not a
   frame error.
3. *Explain*: that diagnostic names the entity and the field, and the inspector
   marks the field. Environment's errors today only say "shadow settings are
   outside their supported ranges" without naming which setting or the range.

## 2. The voxel world is rebuilt from scratch on almost any change

`push_voxel_worlds` compares the whole definition — generator, every material,
radii — and the texture-binding generation (`voxel_world.rs:261`). Any
difference releases every section and meshes the world again. Two consequences:

- Dragging any Voxel World field rebuilds the world every frame of the drag.
  Changing one face texture remeshes terrain that did not change shape.
- `TextureBindings::generation` advances on *any* bind or unbind in the project
  (`textures.rs`), so an unrelated sprite loading or hot-reloading also rebuilds
  the voxel world.

Material and texture changes need re-resolving textures on existing meshes (or
remeshing only sections that contain the changed voxel IDs), not a new world.
Only generator and radius changes justify regeneration.

## 3. The inspector cannot edit these components well

### Declared meanings are ignored

`FieldMeaning` has `Range`, `Angle`, `Mask` and `Entity`, and components declare
them (`extract/meanings.rs`). The inspector reads only `Asset`, `Colour` and
`Choice`; everything else falls through to `_ => false`
(`editor/src/native/inspector_panel/rows.rs:346`) and is drawn as an unclamped
`DragValue`. A collider restitution declared `0..=1` drags to −5.

### Environment, Voxel World and Camera Behaviour declare nothing

From `docs/generated/sindri-capabilities.json`: `sindri.environment`,
`sindri.voxel_world`, `sindri.camera.behavior`, `sindri.grid.navigation`,
`sindri.grid.placement`, `sindri.tags` and `sindri.ui.button` have **zero**
field meanings. Seen in the editor:

- Material faces (`materials[].top/side/bottom`) are text boxes holding
  `textures/world-blocks-top.png#stone-0`. They name a texture and a sprite, and
  should be the sprite picker, with a thumbnail.
- Colours are X/Y/Z(/W) number boxes — Background is "X Y Z W". The inspector's
  colour row accepts only four-channel arrays, and Environment's
  `ambient_color`, `directional.color` and `fog.color` are three-channel.
- `generator.kind` is free text over an enum.
- `shadows.map_size` is a number with four legal values. `Choice` is strings
  only; it needs a numeric counterpart.
- Surface/Subsurface/Deep voxel should choose *from the materials this
  component defines*. No meaning expresses "one of the values of that list";
  it needs one (for example `FieldMeaning::KeyOf("materials[].voxel")`), and the
  same shape recurs wherever a component refers to its own list.
- `focus` is a whole-number section coordinate shown as `0.00`.

### Presentation

- **Clipped content.** An Environment row with four number boxes is wider than
  the inspector overlay, so the whole column scrolls sideways and clips its left
  edge: "arent", "ctive", "mbient color", "ackground", "om" for Bloom, with the
  W box cut on the right. Rows need to wrap or stack below a width, not demand
  it.
- **Labels truncate at ~15 characters** (`ui/widgets/property.rs:16`), and each
  nesting level takes width from the label column. Voxel World shows
  "Subsurface…" twice — depth and voxel are indistinguishable.
- **Order is alphabetical.** A material reads Bottom, Side, Top, Voxel; the
  generator reads Base height … Kind … Surface voxel. The ID a material *is*
  comes last, and the variant tag that decides the other fields is in the
  middle. Template order should win.
- **The Canvas inspector overlay has a fixed 560 px height** and ends at "Base
  height" with most of the screen free below it.
- **"Move up" renders as a box**: `"↑"` is not in the bundled font
  (`list.rs:98`). Use an icon glyph.
- **List items are numbered, not named.** A material heading could be its ID
  and a top-face swatch.

## 4. Console and error handling

- **Counts are historical.** After undoing the invalid bloom value, the scene
  renders and the status bar says "Renderer ready", but "1 Error, 0 Warnings"
  stays. A drag through invalid IDs leaves eighteen entries, one per ID, long
  after the value is valid. The console needs two surfaces: **Problems**, the
  current set, rebuilt from state and emptied when fixed; and the **Log**, the
  history.
- **Render failures have no subject.** `render_view` records the extraction
  error as a bare string, so the entity the console can already link to
  (`Entry::subject`) is never filled for the errors that matter most. Section 1's
  per-component diagnostics fix this at the source.
- **Group by cause.** Still open from `editor-direction.md`. Material 22, 23,
  25 and 26 missing is one problem with a changing value.
- **"Something went wrong"** in the status bar is not clickable and names
  nothing (`chrome.rs:536`). It should state the problem and open it.
- **The error chip covers the tabs.** The console's count is drawn right to
  left at the end of its tab strip with no reserved width
  (`dock_strip.rs:157`); in the Canvas overlay it sits on top of the History
  tab — the report's screenshot. The same end of the strip holds Hierarchy,
  Inspector and Assistant actions, so they can overlap in the same way.
- **No copy, search, timestamps or source.** Long paths wrap in a narrow dock
  and cannot be copied into a bug report.

## 5. Layout

Wide and Docked share a title bar and the same problems; Canvas has its own.

- **The title bar draws two things in one place.** The "Find anything… Ctrl K"
  hint is centred on the bar (`chrome.rs:189`) and, when docked, the transport
  is centred on the window (`chrome.rs:221`). They overlap at every width:
  "nyt" shows beside Play and "Ctrl K" is printed over Pause and Step.
- **Columns have no shared budget.** Each slot may take 75% of the window
  (`workspace.rs:82`), independently. Wide opens three columns — 260 + 300 +
  340 px — so at 1366×768 the Scene view is about 450 × 335 px.
- **Squeezed sizes are persisted.** The measured size is written back every
  frame (`workspace.rs:157`), so whatever egui squeezed a slot to becomes its
  saved size. A window made smaller for a moment leaves permanently narrower
  panels.
- **The viewport toolbar clips** ("Perspectiv") and gains a scrollbar in
  Wide at 1366.
- **The Project browser in the bottom dock** lists one of the folder's
  textures, with empty space below where the rest should be.
- **The Assistant takes a column by default** in Wide, to show an install
  prompt.
- **Canvas overlays collide with fixed furniture.** The viewport hint strip
  ("Select · Local … Drag: orbit · Shift: pan") is half under the title bar,
  and the axis gizmo is half under the transport.
- **Opening Voxel Lab looks at sky**, and **F cannot frame the voxel world**
  because it reports "Nothing drawn yet". A voxel world has bounds; it should
  report them.

### Freely movable panels

Requested, and currently a documented decision against it:
`editor-architecture.md` ("Overlays anchor rather than floating freely") argues
that free panels pile on each other by accident. Two things have changed since:
the anchored overlays already overlap fixed furniture (above), and they cannot
be sized to their content. A middle path keeps the reason and removes the
limit: an overlay becomes free-floating when dragged by its strip, snaps to
window and panel edges within a few pixels, cannot be dropped entirely
off-screen, and remembers its rectangle. Docking stays as it is. The decision
section in `editor-architecture.md` changes in the same commit.

## 6. What to fix, in order

Dependency order, cheapest unlock first.

**Status:** items 1 and 2 are done for Environment and Voxel World, with the
error banner moved clear of the status bar and wrapped. Other components still
fail the frame when invalid; they adopt the same tolerance as they are reached.

1. **Stop the freeze.** Per-component extraction diagnostics and fallback
   (section 1, layers 2–3). This removes the worst symptom for every
   component, including ones not audited here.
2. **Voxel material list is safe.** "+" chooses the next free ID; "−" is
   refused, with a reason, for a material the generator uses.
3. **Inspector honours meanings.** Range (clamped slider or bounded drag),
   Angle, Mask, Entity, three-channel colour, numeric choice, and a
   "key of list" meaning for voxel IDs. Then declare meanings for Environment,
   Voxel World and Camera Behaviour, with the sprite picker for material faces.
4. **Console: Problems and Log.** Current problems from state, linked to
   entity and field; history separately; status bar opens it; chip stops
   overlapping the tabs.
5. **Layout fixes.** Title bar collision, shared column budget, stop
   persisting squeezed sizes, rows that wrap instead of clipping, content-sized
   overlays, Unicode-free icons.
6. **Voxel world incremental updates** (section 2).
7. **Free-floating overlays** (section 5), with the architecture decision
   updated.

## What this audit does not cover

Play mode, the Game view's device frames, the slicer, tile and animation
tools, the palette, the Docked arrangement beyond its shared title bar, HiDPI
scaling, and keyboard-only use.
