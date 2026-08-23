# Function matrix

A deliberately terse view of Sindri's current functionality across the engine,
editor, and Decay scripting surface. For evidence and limitations, see
`capabilities.md` and `feature-integration-matrix.md`.

**Legend:** ✅ implemented · 🟡 partial · ❌ missing · — not applicable

| Function | Engine | Editor | Script |
| --- | :---: | :---: | :---: |
| Entities | ✅ | ✅ | 🟡 |
| Hierarchy / parenting | ✅ | ✅ | 🟡 |
| Transform / position | ✅ | ✅ | 🟡 |
| Rotation / scale | ✅ | ✅ | 🟡 |
| 2D sprites | ✅ | ✅ | 🟡 |
| Sprite sheets | ✅ | ✅ | 🟡 |
| Sprite animation | ✅ | ✅ | ❌ |
| Tilemaps | ✅ | ✅ | ❌ |
| Orthogonal grids | ✅ | ✅ | ✅ |
| Isometric grids | ✅ | ✅ | ✅ |
| Grid walls | ✅ | ✅ | ✅ |
| Entity footprints | ✅ | ✅ | ✅ |
| Occupancy | ✅ | ✅ | ✅ |
| Pathfinding | ✅ | ✅ | ✅ |
| Text | 🟡 | ✅ | ❌ |
| Project fonts | ✅ | ✅ | ❌ |
| Perspective camera | ✅ | 🟡 | ❌ |
| Orthographic camera | ✅ | 🟡 | ❌ |
| Pixel snapping | ✅ | 🟡 | ❌ |
| Keyboard input | ✅ | 🟡 | ✅ |
| Pointer / mouse input | ✅ | 🟡 | ❌ |
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
| Physics | ❌ | ❌ | ❌ |
| Collision | ❌ | ❌ | ❌ |
| Project management | 🟡 | ❌ | — |
| Game export | 🟡 | ❌ | — |

A checkmark means the function is implemented and exercised on that surface,
not merely represented by a type, API, schema, or editor control. Keep this
file intentionally short: detailed caveats belong in `capabilities.md`, while
cross-surface gaps belong in `feature-integration-matrix.md`.
