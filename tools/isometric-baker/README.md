# Sindri isometric baker

An **offline asset baker**. A 3D model goes in; ordinary Sindri sprites come
out — a PNG and the `.sheet.json` beside it, in the formats the engine already
reads.

Isometric is what it was built for and is still the default, but it bakes three
views now — see [Views](#views). The tool keeps its name because a generated
prefab records its provenance under `sindri.isometric-baker`, and renaming that
key would rewrite every asset already baked with it; that is a migration worth
arguing on its own rather than smuggling in here.

This is not runtime 3D, and nothing here changes what Sindri renders. Sindri
still draws sprites, sheets, tilemaps and prefabs; the engine still has one cube
primitive, no glTF import, no material authoring and no lighting system. A 3D
model is an *authoring input* here, in the same way a `.ttf` is an authoring
input to a font atlas.

```bash
node src/cli.ts fixtures/project/prefabs/standing-stone.isobake.json --out fixtures/project
node src/cli.ts fixtures/project/prefabs/standing-stone.isobake.json --out fixtures/project --check
npm test
```

`fixtures/project` is a miniature Sindri asset root, laid out the way a real one
is: the recipe lives beside the prefab it generates, `--out` is the asset root,
and re-baking rewrites the generated files in place.

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
| generated furniture definitions  | `src/prefab.ts`             | A Sindri prefab, without IsoGame's game-specific fields. |

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

## Views

A recipe's `view` picks the camera. `isometric` is the default, so a recipe
written before this existed still means what it meant.

| `view` | The camera | Height | Scale is stated as |
| --- | --- | --- | --- |
| `isometric` | Pitched by the tile's own ratio, yawed 45° | Up the screen | `tile` + `tile_world` |
| `top-down` | Straight down | Invisible | `pixels_per_unit` |
| `side` | Straight along +Z | Up the screen | `pixels_per_unit` |

The two flat views exist because the isometric camera cannot reach them by any
choice of tile. Its pitch comes from the tile's ratio —
`sin(elevation) = tileHeight / tileWidth` — so a square tile would be a pitch of
90°, which is why `createCamera` refuses one: a top-down view draws no diamond
to derive a pitch from. Its yaw is fixed at 45° besides, so no tile at all
produces an axis-aligned view.

So the flat views state their scale rather than deriving it. `pixels_per_unit`
is the whole of it: one world unit is that many pixels in both screen axes,
because an axis-aligned orthographic view has no foreshortening to account for.

Each view refuses the other's fields. A `tile` on a `top-down` bake is somebody
expecting a relationship to a tilemap that the picture does not have, and baking
it at some default would be the quiet kind of wrong this format exists to
prevent. For the same reason a flat prefab may not name a `grid`: there is no
tilemap for it to occupy.

Two things follow from the geometry rather than from any decision here:

- **A top-down bake shows only top faces**, so the banded shading gives one tone
  per material and the result is flat by nature. Shape has to come from the
  model's silhouette and its materials, not from its lighting.
- **A flat bake skips the footprint check.** That check exists because a tilemap
  hands out ground and the picture has to stay inside what it was given. With no
  tilemap, no ground is being handed out, and there is nothing to overrun.

## Block-face atlases

Stackable tile volumes need to remove faces shared by neighbouring blocks. A
whole cube baked into one sprite cannot do that, so an isometric recipe may set
`"output": "block_faces"`. The baker then renders each named variant as three
aligned frames: `<name>-top`, `<name>-south`, and `<name>-east`.

All three frames use the canvas and floor-centre anchor measured from the whole
model. The runtime can therefore draw any subset at the same cell transform and
the pieces meet exactly. `directions` must be `1`: these are the three visible
layers of one fixed isometric view, not rotations of a freestanding object.

```jsonc
{
  "output": "block_faces",
  "directions": 1,
  "variants": [{
    "name": "grass",
    "model": {
      "materials": {
        "earth": { "colour": "#805236" },
        "grass": { "colour": "#6f9f52" }
      },
      "parts": [
        { "type": "box", "material": "earth", "position": [0, 0.5, 0], "size": [1, 1, 1] },
        { "type": "plate", "material": "grass", "position": [0, 1.001, 0], "size": [1, 1] }
      ]
    }
  }]
}
```

Face extraction follows outward axis-aligned normals. This deliberately suits
Minecraft-like blocks and their flat decals; rounded or diagonal geometry is a
normal sprite, not a block face. A thin top plate is how a grass block gives its
top a different material without duplicating the cube or hand-authoring three
separate models.

### Animation frames

There is no separate animation mode, and none is needed. `variants` already
packs several models onto one sheet as named frames, so a clip is a recipe with
`directions: 1` and one variant per frame:

```jsonc
"view": "side",
"pixels_per_unit": 32,
"directions": 1,
"variants": [
  { "name": "walk-0", "model": { … } },
  { "name": "walk-1", "model": { … } }
]
```

That is exactly the shape `sindri.animation.sprite` reads: its `frames` are
sheet sprite names. The frames share one canvas and one palette, which is what
keeps a walk cycle from shimmering between shades as it plays.

## The recipe

A bake is described by a `<name>.isobake.json` document, which is the durable
half of a baked asset: the PNG and its sheet are derived and can be regenerated,
the recipe is the source.

```jsonc
{
  "format_version": 1,
  "id": "standing-stone",
  "texture": "textures/standing-stone.png",  // where the sheet is written
  "view": "isometric",                       // or "top-down" or "side"
  "tile": { "width": 64, "height": 32 },     // pixels across one floor diamond
  "tile_world": { "width": 1.1, "height": 0.55 }, // the tilemap it stands on
  "facing": "south",                         // which way the model is authored
  "directions": 4,                           // 1, 2, 4 or 8 frames
  "footprint": { "width": 1, "height": 1 },  // tiles occupied
  "prefab": {                                // optional; omit for a sheet alone
    "path": "prefabs/standing-stone.prefab.json",
    "name": "Standing Stone",
    "recipe": "prefabs/standing-stone.isobake.json"
  },
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

Primitives are `box`, `plate`, `cylinder`, `cone` and `sphere`. A `plate` is a
single flat quad in the XZ plane, and it exists because a *flat* floor tile is
not a box: a box of zero height has its top and bottom faces in exactly the same
plane, and which of them wins a pixel then comes down to the last bit of an
interpolation — so a tile that should be one flat colour comes out dithered
between its brightest and darkest shade.

### Footprints

A recipe's `footprint` is what the *game* reserves for the thing: collision,
placement and the order sprites draw in all read it, and all of them assume the
picture stays inside it. So the baker refuses a model that stands on more ground
than its footprint claims. Isometric only — a flat view stands on no tilemap, so
nothing is reserving ground for the picture to overrun. Height is free — a tree is meant to tower over its
cell — and only the ground is measured.

The failure it prevents is quiet. Gather's shrine stood 1.12 tiles across on a
footprint of one, so it covered ground the game still handed out, and a player
standing on a neighbouring tile legally was drawn sliced by a plinth it was not
touching. Nothing errored; it just looked wrong.

It is refused rather than widened for you, because the fix depends on what the
thing is: a shrine that overhangs by a tenth of a tile wants a smaller model,
and one that genuinely covers four tiles wants a bigger footprint.

### Baking a floor tile

A tilemap draws each cell on a quad of its `tile_size`, so a tile's frame has to
be exactly the tile and nothing more — `padding: 0`, no outline, and a model
exactly one unit square. Anything raised above it, a tuft or a pebble, projects
a fraction of a pixel past the diamond, and because the canvas is symmetric
about the anchor that fraction costs a whole pixel each way and the cell stops
matching the quad it fills.

A *slab* tile is a box one unit square and however tall the slab is, centred on
the origin. Centred, so the canvas — symmetric about the anchor — comes out
exactly the tile plus the skirt with no wasted rows, and the top face lands on
the cell once the tilemap drops the quad by half the overhang. One material, not
two: the banded shading already gives a box three tones, the top face on the
ramp's highlight and the two sides below it, and a second darker material for
the sides darkens what the shading has already darkened.

Size the skirt a hair *under* the pixels you want rather than over. The canvas
rounds up, so 10.00002 pixels of skirt costs a whole pixel each way.

### Anchors

Every sheet the baker writes declares `"anchor": "center"`, and it is not a
guess: the canvas is padded symmetrically about the floor centre of tile (0,0),
so the middle of the frame *is* the point that stands on the tile. It is written
out rather than left to the format's default so the sheet says what it is — and
so hand-drawn art sitting beside it is visibly making a choice rather than
inheriting one.

### Several models on one sheet

A recipe may name `variants` instead of `model`, and each becomes a named frame
of one sheet. That is what a tile set is:

```jsonc
"variants": [
  { "name": "grass", "model": { "materials": { … }, "parts": [ … ] } },
  { "name": "path",  "model": { "materials": { … }, "parts": [ … ] } }
]
```

The frames share one canvas and one palette. The canvas because frames must be
uniform for the anchor to be the centre of every one; the palette because a
blended edge pixel of one tile must not snap to a shade only the tile beside it
declared — two tiles meant to match would then not.

Frames are named by direction for a lone model, by variant for a single-direction
sheet of several, and `variant-direction` when it is both. A tilemap's `palette`
is written in those names.

A field the reader does not understand is an error, not a default: a recipe with
`supersamples: 8` in it would otherwise bake at 4 and say nothing.

Model conventions: one unit is one tile edge, `y = 0` is the floor, the model is
centred on its own footprint in X/Z, and it faces `+X` — "south" — at rotation 0.

Model conventions hold in every view: the model is authored the same way and
only the camera differs, so one model can be baked isometric for one game and
top-down for another without being rebuilt.

Only `"kind": "primitives"` bakes today. Model-file input (GLB/glTF/OBJ) is
refused explicitly rather than ignored, so a recipe naming one fails instead of
quietly baking nothing.

## What a bake writes

For `"texture": "textures/standing-stone.png"` and a `prefab` block:

```text
textures/standing-stone.png          the frames, packed as one horizontal strip
textures/standing-stone.sheet.json   an edge-to-edge grid of one row, named by direction
prefabs/standing-stone.prefab.json   a one-entity prefab that draws it at the right size
```

Note the sheet's name: Sindri's suffix **replaces** the extension, so it is
`standing-stone.sheet.json`, not `standing-stone.png.sheet.json`
(`sheet_id_for` in `crates/sindri-core/src/sheet.rs`).

### Gutters

Frames are packed with two pixels of gutter, and the gutter is filled by
repeating each frame's own edge pixels outward.

This is not defensive tidiness. `TextureFilter::Nearest` in
`crates/sindri-render/src/texture.rs` sets `mag_filter` to Nearest but leaves
`min_filter` Linear, so a sheet drawn at even slightly under its authored size
is sampled bilinearly — and a frame packed edge to edge against its neighbour is
blended with it. On a sprite with transparent padding that is a faint rim; on a
floor tile, whose art fills its cell exactly, it is the tile beside it smeared
across every cell. A transparent gutter would only trade a colour seam for a
dark one, which is why the edge is repeated instead.

A scene refers to one frame by name — `textures/standing-stone.png#north` — and
a sprite needs a scale that makes one baked pixel one intended pixel. That scale
is arithmetic, so the tool does it rather than the author:

```json
"transform_3d": {
  "position": [0.0, 0.0, 0.0],
  "rotation": [0.0, 0.0, 0.0, 1.0],
  "scale": [0.859375, 2.028125, 1.0]
}
```

A generated prefab carries a transform, a sprite showing the default direction,
and — only when the recipe names the scene's tilemap — a `sindri.grid.occupant`
saying which cells it stands on. It does not carry categories, interaction
spots, stackability or a collision model: those are what IsoGame needs from a
chair, and a Sindri prefab that had them would make every project carry another
game's vocabulary.

### The generation record

The root's `editor` map holds a `sindri.isometric-baker` entry — the canvas, the
anchor, the directions, the recipe that can rebuild it, and whichever scale the
view actually has: `tile` for an isometric bake, `view` and `pixels_per_unit`
for a flat one. The view is written only when it is not the default, so a record
made before flat views existed still reads as the isometric bake it was, byte
for byte. An
entity's `editor` state is defined as something runtimes ignore, and a prefab's
is dropped when one is spawned, so recording provenance cannot change what the
asset does. It is also what an editor will need to offer a rebuild.

### Canonical output

A Sindri document is written in a canonical form that is a *fixed point*:
reading one and writing it again produces the same bytes. A generated prefab has
to already be at that fixed point, or the first time someone opens it in the
editor and saves, it rewrites lines nobody edited.

That means reproducing two things Sindri owns — the serialization rules in
`crates/sindri-core/src/scene/canonical.rs`, and the shortest decimal an `f32`
is spelled with. Both are in `src/canonical.ts` and `src/f32.ts`, and neither is
trusted on this side: `crates/sindri-core/tests/baked_documents_are_canonical.rs`
parses the generated fixture with the real implementation and asserts that
writing it back produces identical bytes.

## Determinism

`--check` re-bakes and fails if the result differs from what is on disk, which
is how CI asserts that a checked-in asset is still what its recipe produces.

It compares PNG **pixels**, not PNG bytes. A zlib upgrade that changes the
compressed stream and no pixel is not a regression, and a test that failed on it
would be turned off within a month.

Nothing written carries a timestamp. IsoGame's generator metadata records a
`renderedAt`, which is the right choice for a build artefact and the wrong one
for a file in version control.