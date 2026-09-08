# Function matrix

A deliberately terse view of Sindri's current functionality across the engine,
editor, and Decay scripting surface. For evidence and limitations, see
`capabilities.md` and `feature-integration-matrix.md`.

**Legend:** ✅ implemented · 🟡 partial · ❌ missing · — not applicable

| Function | Engine | Editor | Script |
| --- | :---: | :---: | :---: |
| Entities | ✅ | ✅ | 🟡 |
| Prefabs | ✅ | ❌ | ✅ |
| Reusable data profiles | ✅ | ✅ | ✅ |
| Tags | ✅ | ✅ | ✅ |
| Entity queries | ✅ | — | ✅ |
| Collections | — | — | 🟡 |
| Hierarchy / parenting | ✅ | ✅ | 🟡 |
| Transform / position | ✅ | ✅ | 🟡 |
| Rotation / scale | ✅ | ✅ | 🟡 |
| 2D sprites | ✅ | ✅ | 🟡 |
| Procedural 2D shapes | ✅ | 🟡 | ✅ |
| UI images | ✅ | ✅ | 🟡 |
| Sprite sheets | ✅ | ✅ | 🟡 |
| Sprite animation | ✅ | ✅ | ❌ |
| Tilemaps | ✅ | ✅ | ❌ |
| Orthogonal grids | ✅ | ✅ | ✅ |
| Isometric grids | ✅ | ✅ | ✅ |
| Grid walls | ✅ | ✅ | ✅ |
| Entity footprints | ✅ | ✅ | ✅ |
| Occupancy | ✅ | ✅ | ✅ |
| Pathfinding | ✅ | ✅ | ✅ |
| Text | ✅ | ✅ | 🟡 |
| UI buttons and layout | ✅ | 🟡 | 🟡 |
| Weave responsive presentation | 🟡 | ❌ | — |
| Seeded randomness | ✅ | ➖ | 🟡 |
| Game saves | ✅ | ➖ | 🟡 |
| Effects | ✅ | 🟡 | 🟡 |
| Static web export | ✅ | ➖ | ➖ |
| Prefabs in a shipped build | ✅ | ➖ | ➖ |
| Profiles in a shipped build | ✅ | ➖ | ➖ |
| Showing and hiding a screen | ✅ | ✅ | ✅ |
| Project fonts | ✅ | ✅ | ❌ |
| Perspective camera | ✅ | 🟡 | ❌ |
| Orthographic camera | ✅ | 🟡 | ❌ |
| Pixel snapping | ✅ | 🟡 | ❌ |
| Viewport aspect | ✅ | ✅ | ✅ |
| Keyboard input | ✅ | 🟡 | ✅ |
| Pointer / mouse input | ✅ | ✅ | 🟡 |
| Touch input | ✅ | ✅ | 🟡 |
| Audio playback | ✅ | 🟡 | ✅ |
| Audio looping | ✅ | 🟡 | ✅ |
| Audio pause / resume | ✅ | 🟡 | ✅ |
| WAV / Ogg / MP3 | ✅ | 🟡 | ✅ |
| Asset loading | ✅ | 🟡 | 🟡 |
| Asset manifests | ✅ | 🟡 | — |
| Hot reload | ✅ | 🟡 | 🟡 |
| Scene loading / saving | ✅ | ✅ | — |
| Undo / redo | ✅ | ✅ | — |
| Play / pause / stop | 🟡 | 🟡 | ✅ |
| Browser / WASM | ✅ | — | — |
| Native desktop | ✅ | ✅ | — |
| 3D rendering | 🟡 | 🟡 | ❌ |
| Depth testing | ✅ | 🟡 | ❌ |
| Meshes | 🟡 | 🟡 | ❌ |
| Materials | ❌ | ❌ | ❌ |
| Lighting | ❌ | ❌ | ❌ |
| glTF import | ❌ | ❌ | ❌ |
| 2D physics | ✅ | 🟡 | ✅ |
| 2D collision | ✅ | 🟡 | ✅ |
| 3D physics | ❌ | ❌ | ❌ |
| 3D collision | ❌ | ❌ | ❌ |
| Project management | ✅ | 🟡 | — |
| Game export | ✅ | ❌ | — |

A checkmark means the function is implemented and exercised on that surface,
not merely represented by a type, API, schema, or editor control. For a gameplay
capability, exercised means a game uses it. Gather proves established engine
features; Orbital Last Stand has now exercised spawning, prefabs, profiles,
queries, pointer input, procedural shapes, physics, audio, persistence, effects,
export, and responsive Weave presentation. A 🟡 entry therefore names a real
remaining limitation, not merely missing game evidence. Keep this file
intentionally short: detailed caveats belong in `capabilities.md`, while
cross-surface gaps belong in `feature-integration-matrix.md`.
