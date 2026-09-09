# Sindri isometric baker

An **offline asset baker**. A 3D model goes in; ordinary Sindri sprites come
out — a PNG and the `.sheet.json` beside it, in the formats the engine already
reads.

This is not runtime 3D, and nothing here changes what Sindri renders. Sindri
still draws sprites, sheets, tilemaps and prefabs; the engine still has one cube
primitive, no glTF import, no material authoring and no lighting system. A 3D
model is an *authoring input* here, in the same way a `.ttf` is an authoring
input to a font atlas.

```bash
node src/cli.ts fixtures/standing-stone.isobake.json --out fixtures/baked
node src/cli.ts fixtures/standing-stone.isobake.json --out fixtures/baked --check
npm test
```

## Where this came from

The pipeline is adapted from **IsoGame's Sprite Factory**
(<https://github.com/MadsenDev/isogame/tree/main/tools/sprite-factory>), which is
MIT licensed. That tool solved the hard part — how to get a 3D model to land on
an isometric grid and read as pixel art — and this is its shape, not a
reinvention of it:

| IsoGame                          | Here                        | What changed |
| -------------------------------- | --------------------------- | ------------ |
| `src/iso.ts`                     | `src/iso.ts`                | Same camera. The basis is derived from the tile ratio instead of `asin`/`sin` of it, so it is exact. |
| `src/palette.ts`                 | `src/palette.ts`            | Ported as-is. |
| `src/materials.ts`               | `src/shading.ts`            | The same banded shading, as TypeScript instead of GLSL. |
| `src/postprocess.ts`             | `src/postprocess.ts`        | Ported, minus the vertical flip (no GL readback) and the crop (see below). |
| `src/model.ts` (geometry)        | `src/primitives.ts`         | The four primitives, generated rather than asked of Three.js. |
| `src/model.ts` (specs)           | `src/model.ts`              | The spec types, minus IsoGame's furniture vocabulary. |
| `src/directions.ts`              | `src/directions.ts`         | Ported. |
| `src/render.ts`                  | `src/raster.ts`, `src/frames.ts` | A CPU rasteriser instead of a WebGL render target. |
| `src/sheet.ts`, `src/factory.ts` | `src/sheet.ts`, `src/bake.ts`    | Sindri's sheet format instead of IsoGame's metadata. |
| `src/catalog.ts`                 | `*.isobake.json`            | A document per asset instead of a TypeScript catalogue. |

Two deliberate departures, both because Sindri wants different things from the
output than IsoGame does.

### No GPU, and no dependencies

IsoGame renders through Three.js into a WebGL render target, so its headless
exporter needs Vite, Playwright and a Chromium with a working GL context. That
is a reasonable choice for a browser game whose interactive tool and CLI must
not drift apart.

Sindri wants baked assets **checked into the repository and regression-tested**,
and a GPU is the wrong thing to make that promise on: driver, ANGLE backend and
GPU model all change what comes back from `readRenderTargetPixels`. Everything
this pipeline asks a renderer to do is flat-shaded triangles under an
orthographic camera with a depth buffer, which is small enough to do exactly, on
the CPU, in the same arithmetic on every machine.

So `src/raster.ts` is a software rasteriser, and the tool has **no
dependencies** — no `node_modules`, nothing to install, nothing for
`docs/dependency-policy.md` to have an opinion about. It runs on Node 22.18 or
newer, which strips the TypeScript types without a build step.

The types are still worth checking, which needs a compiler that the tool
deliberately does not carry. Install one when you want it, and let it go again:

```bash
npm install --no-save typescript @types/node && npx tsc --noEmit
```

Nothing else needs that, and CI does not do it — a job that installed a package
tree to check an asset tool would be the exact cost this design avoids.

### No cropped frames

IsoGame crops each orientation to its own content and records an
`anchorX`/`anchorY` per frame, because its renderer draws a frame at
`(screenX - anchorX, screenY - anchorY)`.

Sindri's sheet format stores frame rectangles and nothing else
(`crates/sindri-core/src/sheet.rs`), and a world sprite is *centred* on its
transform (`crates/sindri-scene/src/extract/sprite.rs`). Cropped frames would
therefore be different sizes around the same centre, and an asset would slide
around its tile as it turned.

So frames are not cropped. Every direction is rendered onto one canvas, sized
symmetrically about the anchor and large enough for the widest rotation, which
puts the anchor on the exact centre of every frame. A centred quad of the right
size then stands on its tile in every direction, with the sheet format Sindri
already has and no engine change at all.

The cost is transparent atlas space: the canvas has to reach as far below the
anchor as the model reaches above it, so a tall asset is roughly twice as tall
as its silhouette. Per-frame pivot metadata in the sheet format would remove
that, and is a worthwhile engine feature — but one that should be argued on its
own merits rather than smuggled in under an asset tool.

## The recipe

A bake is described by a `<name>.isobake.json` document, which is the durable
half of a baked asset: the PNG and its sheet are derived and can be regenerated,
the recipe is the source.

```jsonc
{
  "format_version": 1,
  "id": "standing-stone",
  "texture": "textures/standing-stone.png",  // where the sheet is written
  "tile": { "width": 64, "height": 32 },     // pixels across one floor diamond
  "tile_world": { "width": 1.1, "height": 0.55 }, // the tilemap it stands on
  "facing": "south",                         // which way the model is authored
  "directions": 4,                           // 1, 2, 4 or 8 frames
  "footprint": { "width": 1, "height": 1 },  // tiles occupied
  "render": {
    "supersample": 4,
    "padding": 2,
    "alpha_cutoff": 128,
    "palette_snap": true,
    "outline": { "enabled": true, "colour": "#241d2b" },
    "shading": { "light": [-0.3, 0.89, 0.35], "thresholds": [0.25, 0.5, 0.8] }
  },
  "model": {
    "kind": "primitives",
    "materials": { "stone": { "colour": "#8d8f9a" } },
    "parts": [{ "type": "box", "material": "stone", "size": [0.34, 0.88, 0.4] }]
  }
}
```

A field the reader does not understand is an error, not a default: a recipe with
`supersamples: 8` in it would otherwise bake at 4 and say nothing.

Model conventions: one unit is one tile edge, `y = 0` is the floor, the model is
centred on its own footprint in X/Z, and it faces `+X` — "south" — at rotation 0.

Only `"kind": "primitives"` bakes today. Model-file input (GLB/glTF/OBJ) is
refused explicitly rather than ignored, so a recipe naming one fails instead of
quietly baking nothing.

## What a bake writes

For `"texture": "textures/standing-stone.png"`:

```text
textures/standing-stone.png          the frames, packed as one horizontal strip
textures/standing-stone.sheet.json   an edge-to-edge grid of one row, named by direction
```

Note the sheet's name: Sindri's suffix **replaces** the extension, so it is
`standing-stone.sheet.json`, not `standing-stone.png.sheet.json`
(`sheet_id_for` in `crates/sindri-core/src/sheet.rs`).

A scene then refers to one frame by name — `textures/standing-stone.png#north` —
and sets the sprite's scale from the bake report:

```text
standing-stone: 4 frames of 50x118
  anchor        25, 59 (the centre of every frame)
  sprite scale  0.8594 x 2.0281 world units
```

Generating that prefab, rather than reading the number off a report, is the next
change; this one stops at the assets.

## Determinism

`--check` re-bakes and fails if the result differs from what is on disk, which
is how CI asserts that a checked-in asset is still what its recipe produces.

It compares PNG **pixels**, not PNG bytes. A zlib upgrade that changes the
compressed stream and no pixel is not a regression, and a test that failed on it
would be turned off within a month.

Nothing written carries a timestamp. IsoGame's generator metadata records a
`renderedAt`, which is the right choice for a build artefact and the wrong one
for a file in version control.
