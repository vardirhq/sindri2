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

## Anchors

Sprite anchors resolve against the overlay camera's extent — half its `vertical_size`, widened by
the viewport aspect — so a sprite keeps its relationship to an edge as the window changes shape. An
anchor is a corner, edge, or centre; the sprite's `Transform2D` position offsets from there.

## Viewing without editing

Gameplay renders through the authored camera. An editor moves around it without touching the scene,
which is what `CameraView` describes: an orbit around the camera's target, a distance multiplier,
and a projection choice. It changes the camera matrix only — models are never moved, so an orbiting
editor camera cannot be confused with a rotating object.
