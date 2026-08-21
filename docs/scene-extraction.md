# Scene extraction

`sindri-render` knows nothing about worlds, components, or scenes. `sindri-core` knows nothing about
drawing. `sindri-scene` is the seam between them: it owns the built-in `sindri.*` component schemas
and derives a frame from whatever a world currently holds.

That split is the point. Gameplay only ever writes to the world:

```rust
fn fixed_update(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
    let entity = self.player;
    let data = context.world.get_mut(entity).ok_or(Error::Missing)?;
    data.transform_3d = Some(moved);
    Ok(())
}
```

Nothing tells the renderer this happened. The next extraction reads the world as it now is.

## One world

That only holds while there is one world. A scene that owned its own copy would
leave gameplay writing one and the renderer reading another, and the bug would
look like a rendering problem rather than a bookkeeping one.

So a scene owns its component schemas and nothing else. The world belongs to
whoever is running the engine — `EngineCore` behind `EngineHost` at runtime, the
editor while authoring — and extraction takes it as an argument:

```rust
let prepared = scene.extract_frame(engine.world(), viewport, &bindings)?;
```

## Built-in components

| Type name | Draws |
| --- | --- |
| `sindri.camera` | The world camera (`perspective`) or the overlay camera (`orthographic`) |
| `sindri.mesh` | One opaque pass per mesh, at the mesh's render layer |
| `sindri.sprite` | One batched pass per space, sprite layer, and texture |
| `sindri.sprite_animation` | Nothing of its own — it decides which part of the sheet the entity's sprite draws |

A game registers its own component types alongside these with `SceneExtractor::register`.

## What extraction guarantees

- **Deterministic order.** Passes sort by stage, then layer, then insertion. Entities are visited in
  world index order, so the same world always produces the same frame.
- **Batching by layer.** Sprites group into one batch per render layer rather than requiring every
  sprite in a scene to share one, and sort back to front within a layer using the
  [transparent draw key](rendering-transparency.md).
- **Cameras are required only when something needs them.** A scene with no meshes and no
  world-space sprites needs no perspective camera. Drawing either without one reports
  `MissingWorldCamera` rather than silently rendering nothing, and a screen-space sprite with no
  orthographic camera reports `MissingOverlayCamera`. Anchored text uses that same overlay camera.

## Text

`sindri.text` is screen-space text. Its entity transform offsets one of the nine
overlay anchors, extraction projects that point into physical viewport pixels,
and the resulting `TextInstance` carries content, font asset reference, font
size, line height, colour, and layer. Strings on the same layer share one
ordered text pass.

`referenced_fonts` is the load list for a world. A host validates those bytes
with `FontAssetDecoder` and binds each logical reference to `TextRenderer`;
unbound references draw nothing rather than falling back to an installed face.
That asymmetry with missing textures is intentional: a checker can reveal a
missing image, while substitute text can look correct but differ by platform.

## Textures

A scene names a texture; the renderer knows only handles. `TextureBindings` is where the two meet,
and it is the only place that knows both.

```rust
let mut bindings = TextureBindings::new();
bindings.bind("textures/badge.png", registry.insert(decoded));
```

Binding cares about the reference, not where the pixels came from — a decoded PNG, a generated
checkerboard, and a render target all bind the same way.

A reference nothing has bound resolves to `TextureRegistry::MISSING`, a magenta checker. A missing
texture therefore draws as obviously wrong rather than failing the frame or, worse, silently reusing
whichever texture happened to be bound last. A `TextureId` is a slot and a generation, the same shape
as an `EntityId`: the registry reuses the slot of a released texture, and the generation is what stops
a handle nobody updated drawing whatever landed there next — which would be the same failure as
reusing the last-bound texture, arrived at from the other direction. `unresolved_textures` names every reference a world
draws that nothing has bound, so the diagnosis is a list rather than a magenta surface.

Sprites batch per texture as well as per layer, because a batch is a single draw call.

`referenced_textures` lists every texture a world draws with, which is the statement of what a scene
needs loading — `unresolved_textures` is that list narrowed to what nothing has bound. A host loads
the first and reports the second.

What a reference *is* decides how it is satisfied. One that parses as an `AssetId` names a file, and
resolves against the directory the scene lives in. One that does not is the engine's to generate:
`PROCEDURAL_TEXTURES` is the table of those, holding the reference a scene writes and the parameters
that produce it. The `procedural:` prefix is not decoration — a colon is a reserved delimiter in an
`AssetId`, so a procedural reference cannot be parsed as one, and the two kinds cannot be confused
without anybody remembering a rule. The table is shared rather than copied per host because the
capture verifies these exact colours in a rendered image and the editor draws the same scene; two
hosts choosing their own navy would be a difference nothing catches until a screenshot looks wrong.

## Sliced images

A sprite draws part of a texture rather than all of one, and **which part is the image's business
rather than the sprite's**. A scene writes `textures/tiles.png#floor`; a sheet document beside the
image says where `floor` is.

It was not always so, and the shape it replaced is the argument for it. Three components each said
how one sheet was divided — a sprite carried a raw rect, an animation carried a grid and cell
numbers, a tilemap carried a second grid and more cell numbers. Point two of them at one image and
each declared its layout separately, with nothing making the two agree. That is the duplication
`CLAUDE.md` warns about: two agreeing choices where one shared answer belongs.

```json
{
  "format_version": 1,
  "grid": { "columns": 2, "rows": 1, "names": ["light", "dark"] }
}
```

A packed sheet says so in pixels:

```json
{
  "format_version": 1,
  "grid": {
    "columns": 16, "rows": 16,
    "size": [512, 512], "margin": [2, 2], "spacing": [4, 4]
  }
}
```

`margin` is the border around the whole grid and `spacing` is the gutter between
neighbouring cells — the same two words a tileset has used for decades, meaning
the same two things. Sheets are exported with gutters so filtering cannot bleed
one frame into the next, and a slicer that cannot say so can only cut sheets that
were exported without them.

`size` records the image the grid was measured against, and is written **only**
when there is a margin or a spacing to measure. A grid that divides an image edge
to edge produces the same fractions whatever the image turns out to be, so it
does not carry a size and every sheet written before margins existed is unchanged.
A measured grid without one is an error rather than a sheet whose every cell
comes out as nothing.

**The sheet's ID is derived, not declared.** `textures/tiles.png` is sliced by
`textures/tiles.sheet.json`. A scene naming its sheets would be a fourth place that can disagree, so
no scene does; `sheet_id_for` is the one rule, and both the editor looking on disk and a game
shipping bytes use it.

**A grid generates names rather than sitting beside rects it agrees with.** Storing both the grid and
the cell rects it produces would be the same duplication one level down. A cell nobody named is
called by its own index — `#3` is a worse name than `#idle` and a much better one than nothing — so a
sheet that has been sliced but not named is still usable. Parts that are not on the grid are named
explicitly in `sprites`, and a name used twice is an error rather than a race.

**Names, not numbers**, everywhere a scene refers to a part: a name survives a re-slice that moves
the cell, and an index does not. That is what the whole arrangement buys.

### The fragment

`textures/tiles.png#floor` splits into a path and a name. `#` is a *rejected* character inside an
`AssetId`, and that is the argument for using it here rather than against — it is reserved precisely
so a fragment cannot leak into a path that becomes a URL. Splitting it off at the boundary, exactly
as a URL does, leaves the asset ID a pure path and nothing that resolves an asset ever sees the
fragment.

`SpriteRef` holds the raw reference rather than an `AssetId`, because not every texture is a file:
`procedural:checkerboard` is generated, and the colon that makes it unparseable as an asset ID is
what marks it as generated. `SpriteRef::asset` produces an ID where one is genuinely needed.

### Rects are still normalized and still checked

`UvRect` is what a sheet's authored numbers become, and it is constructed checked, because the ways
to get this wrong are quiet: a zero-width rect samples one column of texels down the whole sprite,
and one reaching past the edge samples whatever the sampler's addressing mode decides — a different
picture on a different clamp mode rather than an error. Checking happens once, in
`TextureBindings::bind_sheet`, rather than everywhere a rect is used.

Normalized rather than pixels remains the load-bearing choice. A sheet sliced into a grid has cells
`1 / columns` wide wherever the sheet's resolution lands, so a normalized rect survives an artist
doubling the sheet and a pixel rect does not; and nothing between the scene and the shader has to
know a texture's dimensions. The division happens in `f64` and narrows once, so the last column of a
sheet that does not divide evenly in binary still ends on the edge rather than a hair past it.

The rect rides on the instance, not on the pipeline, so every part of one sheet stays in a single
batch: a sprite sheet is one texture, one draw call, and as many rects as there are sprites.

### When a name resolves to nothing

A sprite naming a part no loaded sheet places draws the **whole image** and is reported by
`unresolved_sprites`. The same rule an unbound texture has always followed: the frame still draws, so
the failure has to be *said*, or the only clue would be a picture that is subtly the wrong part of an
image.

A *playing clip* decides outright — what it names, or the whole image. It deliberately does not fall
back to the clip's first frame, which would draw a plausible picture of the wrong moment, and that is
the failure that hides rather than the one that shows.

**An animated sprite needs its sheet even though its own reference names no part of one.** Which part
it draws is its clip's business and changes every few frames, so its `texture` carries no fragment —
and a host asking for sheets by looking at fragments alone never asks for its sheet. It then resolves
its whole texture, which is every frame at once. `referenced_sheets` handles this case explicitly and
a test holds it, because nothing about the sprite's own reference says it is needed.

## Tilemaps

`sindri.tilemap` is a grid of tiles cut from one sheet, drawn from one entity.

**The point is not draw calls.** Loose sprites sharing a texture already batch into one draw, so a
floor of 49 sprites and a tilemap of 49 tiles cost the same to render. What changes is authoring: 49
entities, each with a transform, a name, a stable ID, and a component, become one component holding
49 small integers. Gather's floor made the difference concrete — the scene went from 68 entities to
20 and from 45KB to 12KB, and its hierarchy went from a list you scroll past 49 `Floor r,c` rows to
find the player, to a list that fits on one screen.

```json
"sindri.tilemap": {
  "texture": "textures/tiles.png",
  "palette": ["light", "dark"],
  "columns": 7, "rows": 7,
  "tile_size": [1.1, 0.55],
  "projection": "isometric",
  "space": "world",
  "tiles": [1, 0, 1, 0, 1, 0, 1, 0, 1, 0, ...]
}
```

`tiles` holds `columns * rows` cells, row-major from the top-left. An empty cell is `null` and not a
sentinel index, because every index is a real tile: reserving `0` or `-1` to mean "empty" is how a
map ends up with an accidental floor in the corner nobody authored.

**A cell indexes the map's own palette, not the sheet.** The palette names the sprites this map
uses; the sheet places them. That keeps a 49-cell map 49 small integers rather than 49 repeated
strings, and it is what makes a re-slice survivable — the sheet can move `floor` to another cell and
every map drawing it still draws the right thing.

**Variation comes from the sheet, not from tinting.** The map has one tint, and a checkerboard is two
named sprites rather than two tints of one. Gather's floor was 25 sprites tinted `0.82` and 24 tinted
white; it is now a two-cell sheet, a palette of `["light", "dark"]`, and a parity test. Per-tile tint
would be a second way to say what a second sprite already says.

**A tilemap is not a second kind of thing to draw.** Its cells become sprite instances in the same
batches loose sprites use, keyed by the same space, layer, and texture. So a tilemap and a sprite on
one layer and one texture share a draw and sort against each other, and a prop can sit between two
rows of floor rather than behind a plane of it.

**The projection is the tilemap's own layout rule**, not Milestone 9's grid module. `orthogonal` runs
columns +X and rows -Y, so the map reads the way the array does. `isometric` runs them along the two
diagonals at half steps, which is what makes a square grid look like a diamond floor. Milestone 9
still owns coordinates, neighbours, and pathfinding for whatever wants them; a renderer needed to
know where a tile goes before any of that existed.

`tile_to_local` and `local_to_tile` are inverses, and a test holds them to that on every cell of both
projections. That property is the whole of what painting a map will need from the maths — turning a
click into a tile — so it is worth more than the two functions look.

A map that is not the shape it claims, or whose cell indexes past its palette, fails extraction with
the numbers in the message rather than drawing part of a floor. It still *opens*, for the reason a
bad UV rect does: the editor is where it gets fixed.

## Sprite animation

`sindri.sprite_animation` sits beside `sindri.sprite` on the same entity and decides which sprite of
the sheet is drawn. The sprite still owns the texture, tint, space, anchor, and layer; the animation
owns the clips and which one is playing. Where each named sprite *is* belongs to the sheet beside the
image, which is why a clip no longer carries a grid. They are two components rather
than one animated-sprite component on purpose — the legacy engine duplicated every sprite field onto
its animated variant, and a duplicated field is how a tint set on one of them stops being the tint
that draws.

```json
"sindri.sprite_animation": {
  "clips": {
    "walk": { "frames": ["lift", "plant", "lift", "plant"],
              "seconds_per_frame": 0.1, "looping": true }
  },
  "playing": "walk",
  "speed": 1.0
}
```

A clip's frames are sprite names from the sheet, in the order they play. Every frame lasts the same
time; a pose held longer is written by repeating its name, which is how sheet tools express it anyway
and is one way of saying it rather than two that can disagree.

**Where playback lives is the load-bearing decision.** A clip and its timing are authored, so they are
in the scene. The frame a sprite happens to be on halfway through a run is not, so it is not.
`SpriteAnimations` holds that cursor beside the world — `advance(world, components, dt)` moves every
animated sprite on, and `extract_animated` draws each one where the cursor says. The cursor holds a
*name*, not a rect: where a clip has got to does not depend on where its frames sit in an image, so
the rect is resolved during extraction, where the sheets are already in hand. Keeping it out of the
component is what stops watching an animation play from rewriting the file it came from: a scene saved
mid-run has to be the scene that was opened, and a test asserts exactly that.

The cursors that survive an `advance` are exactly the ones the world still justifies, so an entity
that is despawned or loses its animation loses its cursor with it. Switching clips restarts rather
than resuming at whatever frame number the last clip had reached. Which clip plays is authored, so a
game switches clips by writing to the world; `restart` is the one thing that is not a change to the
scene, and it exists because a clip that has finished otherwise has no way back to its start.

`extract` is `extract_animated` with nothing playing, and a sprite carrying clips that no cursor has
reached draws whatever its own reference names. That is how a scene picks a resting pose other than
the clip's first frame: name it.

A sprite that names *no* part is the exception, and it has to be. Its reference is the whole image,
and a sheet drawn whole is every frame of the clip at once squeezed into one quad — which is not a
pose anybody meant. So an animated sprite with a bare texture reference falls back to the first frame
of the clip it is playing, which is where advancing would have started it anyway. This matters
wherever a scene is drawn before it is run: a game's opening frame, an offscreen capture, and every
entity sitting in the editor outside play mode.

Once a cursor *has* reached it, the clip decides alone. A frame that resolves to nothing draws the
whole image rather than falling back to the resting pose, because a plausible picture of the wrong
moment is the failure that hides.

## Sprite space

A sprite says which space it is in, and the answer decides three things at once: which camera draws
it, which stage it lands in, and what its transform means.

| `space` | Camera | Stage | Depth | Transform |
| --- | --- | --- | --- | --- |
| `screen` (default) | Orthographic overlay | `Overlay` | Ignored — nothing in the world may hide a HUD | X and Y offset an anchor; Z orders it without moving it |
| `world` | Perspective world camera | `Transparent2d` | Tested, never written | The whole transform, exactly as a mesh reads it |

The default is what every sprite already was, so a scene written before the choice existed draws
where it always did. Two sprites in different spaces never share a batch however much else they have
in common, because a batch is one draw call and these differ in both the camera and the pipeline.

Within a batch, sprites sort back to front by how far they are from the camera that draws them,
measured along its forward axis. A world sprite's order therefore changes when the camera moves,
without the scene changing at all. A screen sprite's does not: the overlay camera does not move, and
its Z orders it without placing it — the one deliberate disagreement between where a sprite sorts
and where it is drawn, and what keeps a HUD from vanishing when it is pushed a long way back.

`layer` is the explicit override and beats both. A sprite in a higher layer draws in front of
something nearer the camera, which is the rule people are surprised by once and then rely on.

## Anchors

Sprite anchors resolve against the overlay camera's extent — half its `vertical_size`, widened by
the viewport aspect — so a sprite keeps its relationship to an edge as the window changes shape. An
anchor is a corner, edge, or centre; the sprite's transform position offsets from there in X and Y.

Anchoring is a screen-space idea: a world-space sprite has no edge to hold on to. That is a shape in
the type rather than a rule to remember — `SpriteComponent::screen_anchor` returns an `Option`, so
extraction cannot read an anchor for a sprite that has no business having one.

## Viewing without editing

Gameplay renders through the authored camera. An editor moves around it without touching the scene,
which is what `CameraView` describes: an orbit around the camera's target, a distance multiplier, a
pan across the view plane, and a projection choice. It changes the camera matrix only — models are
never moved, so an orbiting editor camera cannot be confused with a rotating object.

Pan is measured in fractions of the framed half-height rather than in world units, so dragging moves
the picture by the same amount whether the subject is a metre or a kilometre away, and the
perspective and orthographic projections agree about what a pan of one half means. A test holds them
to within a ten-thousandth of each other.

A viewport that paints its own chrome — an axis indicator, a grid, a gizmo — has to know which way
the world is facing to draw any of it truthfully, and one that moves the camera on the user's behalf
has to know how much of the world is framed. `SceneExtractor::world_camera` answers both under the
same `CameraView` a frame would be extracted with, returning a `ViewCamera` — the view matrix and
the framed half-height — or `None` when the world holds no perspective camera. The alternative is a
second copy of the orbit maths beside the first, which is how an indicator ends up disagreeing with
the picture it is drawn on top of; the editor's axis gizmo was painted at three fixed offsets and
claimed the same orientation from every angle.

The framed half-height is the pan's own unit: a pan of one moves the picture by exactly that much.
That makes it what turns a distance on screen back into a pan, which is how the editor's **Focus
selection** centres the view on something without knowing anything about how the camera got where it
is.

The orbit cannot be driven onto the pole. There the offset is parallel to the camera's up axis and
nothing says which way round the picture goes; `look_at` returns a matrix rather than failing, and
the roll it picks is decided by leftover rounding error, so dragging through straight down whips the
whole scene round to face the other way. Pitch turns the offset in the plane that holds it and `up`,
so it adds directly to the angle between them, and `orbited_offset` clamps it there — a hundredth of
a radian short of either pole. The clamp lives in the orbit maths rather than in the caller because
this is where the authored elevation is known: a viewport clamping its own pitch would be guessing
how far the scene had already tilted.
