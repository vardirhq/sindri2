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
