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
  orthographic camera reports `MissingOverlayCamera`.

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
whichever texture happened to be bound last. `unresolved_textures` names every reference a world
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
