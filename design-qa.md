# Editor design QA

## Visual contract

- Direction: selected option 2, the viewport-first harbor workspace.
- Reference viewport: 1487 × 1058.
- Native editor capture: 1440 × 1024 from GitHub Actions CI run 107.
- Comparison method: reference and native capture placed side by side at their original dimensions.

## Result

Pass. The implementation matches the selected direction's defining structure and tone:

- compact global menu and centered play controls;
- narrow scene tool rail and restrained hierarchy panel;
- dominant real-time scene viewport;
- right-side inspector with flat component sections;
- bottom project/console dock with folder tree, search, view controls, and asset tiles;
- near-black graphite palette with sparse amber selection and action emphasis;
- compact Inter typography and a coherent Material Symbols icon family.

## Issues found and resolved

- Replaced the inherited model rotation with a genuine editor camera orbit while preserving the standalone cube example's behavior.
- Added real perspective and orbit-matched orthographic projections instead of a visual-only toggle.
- Reset the initial view to the authored perspective camera so the cube reads as a conventional 3D box.
- Reduced project asset tile widths so the final item no longer clips at the inspector boundary.
- Enlarged and stabilized the Xvfb capture surface for the 1440 × 1024 editor window.

## Intentional differences

- The harbor artwork was a composition and mood reference. The editor renders Sindri's real WGPU demo scene rather than placing concept art in the viewport.
- The current demo scene contains fewer hierarchy and inspector rows than the concept. Panel density will grow naturally as the scene/component model expands.
- Transform gizmos and production scene thumbnails remain future engine features; they are not faked in this implementation.

## Second pass: the design system

The first pass established the palette and the arrangement. This one asked
whether the editor was *built* that way or merely painted that way, by driving
it rather than reading it: opening both scenes, selecting an entity of each
kind, opening the slicer, switching layouts, playing, and shrinking the window
to its minimum.

- Method: the native capture from `scripts/capture-editor.sh`, plus scripted
  pointer sequences through `xdotool` to reach states a screenshot of the idle
  editor never shows — a selected entity, an open menu, the confirm modal, play
  mode, and the minimum 1100 × 720 window.
- Colour contract: `cargo run -p sindri-cube --bin verify` still passes on the
  editor capture, so the chrome changed and the picture did not.

### Issues found and resolved

- The label column was as wide as each label rather than a column, so "Scale"
  put its fields eleven pixels left of "Position"'s. egui allocates a child
  region by what its contents measured; the widget now sets the width.
- Every segmented switch derived its ids from `ui.id()`, which a whole panel
  shares, so Z lock and the tilemap brush collided and egui painted its
  duplicate-id warning across the Transform section.
- Painted controls laid their text out with a real colour, which a galley keeps,
  so every segment and every tab was drawn in the selected one's white.
- A `ComboBox` told to fill the value column built itself wider than the row.
  The inspector widened to cover it until its labels ran off its own left edge.
- The search box drew its magnifier on the far side of the field inside the
  browser's right-aligned toolbar, and took the panel's remaining height with
  it. It is painted into a region it allocates now, as the tabs and the
  segmented switch are.
- `procedural:checkerboard` was marked as a texture the project does not have,
  because the picker was built from the asset directory and a procedural
  reference is deliberately not a file.
- The project dock named the same directory twice, once in its folder tree and
  once over the list of it.
- The scene tools clipped at a narrow viewport. They scroll, with a visible
  rail, so a control is never silently unavailable.

### Intentional differences

- The two-by-three arrangement at the minimum window size leaves the Scene view
  about 230 points wide. That is the arrangement, not a defect: the Wide layout
  in the View menu is the answer at that size, and the tools scroll rather than
  disappear in the meantime.
- Asset tiles still show a type glyph rather than a thumbnail. The tile draws a
  picture when there is one to draw; generating previews is engine work, and a
  faked thumbnail would be worse than an honest icon.
