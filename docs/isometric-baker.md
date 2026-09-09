# The isometric baker

`tools/isometric-baker` turns a 3D model into ordinary Sindri sprites, offline.
Its own README is the reference; this says what it is for, what it is not, and
what the rest of the repository may assume about it.

## What it is

An **asset tool**. A model goes in; a PNG and the `.sheet.json` beside it come
out, in the formats `sindri-core` already reads and `sindri-render` already
draws. It runs when someone runs it, and its output is checked in like any other
art.

The pipeline is adapted from [IsoGame's Sprite
Factory](https://github.com/MadsenDev/isogame/tree/main/tools/sprite-factory)
(MIT). Provenance is recorded in `tools/isometric-baker/ATTRIBUTION.md` and
mapped file by file in that tool's README.

## What it is not

**It is not runtime 3D, and it does not imply any.** Nothing about it changes
what the engine can do. As of this document Sindri still has:

- one mesh primitive, a cube;
- no runtime glTF import;
- no general material authoring;
- no lighting system.

None of those move because a tool renders triangles offline. `docs/function-matrix.md`
and `docs/capabilities.md` describe the engine, and the baker earns no row in
either: a tool is not a surface a capability is exercised on.

It is also not a dependency of anything Sindri ships. The engine, editor, native
game and browser export do not know it exists, and nothing they build calls it.

## Dependency boundary

The tool is Node and TypeScript, and it has **no dependencies at all** — no
`node_modules`, no Vite, no Playwright, no Three.js. It renders with a software
rasteriser of its own, so a bake needs nothing but a Node 22.18 or newer, which
runs its TypeScript without a build step.

That boundary is deliberate and is the thing to defend in review. The moment the
baker needs a package tree, it acquires a supply chain that
`docs/dependency-policy.md` has no way to check, for output that is a PNG.

CI runs the tool's tests and re-bakes its fixture in the `Isometric baker` job of
`.github/workflows/ci.yml`. That job installs nothing.

## The compatibility problem it works around

Sindri's sheet format stores frame rectangles and no per-frame pivot
(`crates/sindri-core/src/sheet.rs`), and a world sprite is centred on its
transform (`crates/sindri-scene/src/extract/sprite.rs`). IsoGame crops each
orientation to its own content and records an anchor per frame; frames like that
copied into Sindri would be different sizes around one centre, so an asset would
slide around its tile as it turned.

The baker therefore does not crop. Every direction is rendered onto one canvas,
sized symmetrically about the anchor and large enough for the widest rotation,
which puts the anchor — the floor centre of footprint tile (0, 0) — on the exact
centre of every frame. A centred quad of the right size then stands on its tile
in every direction, with the format the engine already has.

The cost is transparent atlas space: the canvas reaches as far below the anchor
as the model reaches above it, so a tall asset is roughly twice the height of its
silhouette.

**Per-frame pivot metadata in the sheet format would remove that cost, and is a
reasonable engine feature.** It is deliberately not part of this: it changes a
serialized format the editor and runtime both read, and should be argued on its
own merits rather than arriving as a side effect of an asset tool.

## Determinism

Baked assets are checked into the repository, so a bake must be reproducible.

- The camera basis is derived from the tile ratio with square roots rather than
  `asin` and `sin` of it, and rotations come from an exact eighth-turn table, so
  neither depends on a trigonometric approximation that may move between engine
  versions.
- Rendering is a CPU rasteriser rather than a GPU readback, because driver,
  ANGLE backend and GPU model all change what a `readRenderTargetPixels` returns.
- Nothing written carries a timestamp. IsoGame's generator metadata records a
  `renderedAt`, which is right for a build artefact and wrong for a file in
  version control.
- `--check` re-bakes and compares. It compares PNG *pixels*, not PNG bytes: a
  zlib upgrade that changes the compressed stream and no pixel is not a
  regression.

## Generated documents must be canonical

A bake can write a prefab as well as a sheet, and both are Sindri documents.
Sindri writes a document in a canonical form that is a fixed point — reading one
and writing it again produces the same bytes — so a *generated* document has to
already be at that fixed point. Otherwise the first time someone opens it in the
editor and saves, the editor rewrites lines nobody edited, and the diff is noise
attributed to a person.

Meeting that means the tool reimplements two things this workspace owns: the
serialization rules in `crates/sindri-core/src/scene/canonical.rs`, and the
shortest decimal an `f32` is spelled with. That is the kind of agreement that
holds until it quietly does not, so it is not trusted on the tool's side:
`crates/sindri-core/tests/baked_documents_are_canonical.rs` parses the generated
fixture with the real implementation and asserts writing it back is byte
identical. That test reaches out of the crate on purpose — `sindri-core` defines
canonical form, so it is the only place the claim can be tested.

Anyone adding a generated document type to the baker should extend that test
rather than assume the writer already covers it.

## What a generated prefab may carry

A transform, a `sindri.sprite` showing the default direction, and — only when
the recipe names the scene entity holding the tilemap — a `sindri.grid.occupant`
saying which cells it stands on. Provenance goes in the root's `editor` map,
which runtimes ignore and a spawn drops.

Nothing game-specific. IsoGame's generated furniture definitions carry
categories, sit and lay spots, stackability and a collision model, because that
is what its game needs from a chair. A Sindri prefab gets what any renderer
needs, and a game says the rest itself — the alternative is every project
carrying another game's vocabulary in a generic format.

## Scope today

Baked: primitive models, a fixed configurable isometric camera, banded
palette-based materials, supersampling, alpha thresholding, palette snapping, an
optional inner outline, 1/2/4/8 directional frames, uniform anchor-aligned
frames, deterministic PNG, `.sheet.json` and `.prefab.json` output, and a
persistent `.isobake.json` recipe.

Not baked, each its own change: model-file input (GLB/glTF/OBJ — refused
explicitly rather than ignored), contact shadows, animation poses, layered
character parts, editor integration, and any use in a game.
