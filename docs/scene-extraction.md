# Scene extraction

`sindri-render` knows nothing about worlds, components, or scenes. `sindri-core` knows nothing about
drawing. `sindri-scene` is the seam between them: it owns the built-in `sindri.*` component schemas
and derives a frame from whatever a world currently holds.

That split is the point. Gameplay only ever writes to the world:

```rust
fn fixed_update(&mut self, context: &mut FrameContext<'_>) -> Result<(), Self::Error> {
    let entity = self.player;
    let data = context.world.get_mut(entity).ok_or(Error::Missing)?;
    data.transform_2d = Some(moved);
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
| `sindri.sprite` | One batched overlay pass per sprite layer |

A game registers its own component types alongside these with `SceneExtractor::register`.

## What extraction guarantees

- **Deterministic order.** Passes sort by stage, then layer, then insertion. Entities are visited in
  world index order, so the same world always produces the same frame.
- **Batching by layer.** Sprites group into one batch per render layer rather than requiring every
  sprite in a scene to share one, and sort back to front within a layer using the
  [transparent draw key](rendering-transparency.md).
- **Cameras are required only when something needs them.** A scene with no meshes needs no
  perspective camera. Drawing a mesh without one reports `MissingWorldCamera` rather than silently
  rendering nothing.

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

## Anchors

Sprite anchors resolve against the overlay camera's extent — half its `vertical_size`, widened by
the viewport aspect — so a sprite keeps its relationship to an edge as the window changes shape. An
anchor is a corner, edge, or centre; the sprite's `Transform2D` position offsets from there.

## Viewing without editing

Gameplay renders through the authored camera. An editor moves around it without touching the scene,
which is what `CameraView` describes: an orbit around the camera's target, a distance multiplier, a
pan across the view plane, and a projection choice. It changes the camera matrix only — models are
never moved, so an orbiting editor camera cannot be confused with a rotating object.

Pan is measured in fractions of the framed half-height rather than in world units, so dragging moves
the picture by the same amount whether the subject is a metre or a kilometre away, and the
perspective and orthographic projections agree about what a pan of one half means. A test holds them
to within a ten-thousandth of each other.
