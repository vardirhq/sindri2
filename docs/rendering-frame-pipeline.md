# Rendering frame pipeline

Sindri separates a frame into three explicit stages:

1. **Extraction** converts world and scene state into render commands without issuing GPU work.
2. **Preparation** validates frame-wide state and deterministically orders passes by stage, layer,
   and stable insertion order.
3. **Rendering** consumes only the prepared frame and dispatches commands to GPU renderers.

`ExtractedFrame` owns the viewport, clear operations, and unordered passes. Calling `prepare`
rejects empty viewports and non-finite clear values before returning `PreparedFrame`.

The current stage order is:

1. `Opaque3d`
2. `Transparent2d`
3. `Overlay`

Layers order passes within a stage. Equal stage/layer pairs preserve extraction order. Transparent
sprites inside a batch continue to use `TransparentOrder` before they enter the frame packet.

`Transparent2d` holds world-space sprites, which are drawn through the world camera and tested
against the depth the opaque stage wrote; `Overlay` holds screen-anchored ones, which nothing in the
world may hide. See [scene extraction](scene-extraction.md) for which space a sprite is in and
[transparency](rendering-transparency.md) for what each does about depth.

## Clearing belongs to the frame

`ClearOperations` is frame-wide state, and the frame is cleared once by
`encode_clear` before any pass draws. Every renderer afterwards loads both
attachments.

It used to belong to the opaque mesh pass, which held only while a scene had
exactly one mesh and at least one: a second mesh cleared away the first, and a
scene with no mesh cleared nothing at all, leaving its sprites drawing over the
previous frame and testing against a depth buffer nobody had filled. A scene of
only sprites is what a 2D game is, so that case is the ordinary one.

## Drawing a prepared frame

`encode_prepared_frame` is the third stage: given the renderers, a target, and a prepared frame, it
clears once and encodes each pass in the order preparation put them in. It knows nothing about
worlds, components, or scenes — a prepared frame is a list of commands and the matrices to draw them
with — which is why it belongs in `sindri-render` rather than in whichever host needed it first. It
lived in the cube example for a release, which made the editor depend on an example in order to draw
anything at all.

The cube example is the reference integration. A versioned `SceneDocument` is deserialized from
JSON, validated, loaded into `World`, and extracted into an opaque textured-cube pass followed by a
five-instance overlay pass. Native, WebGPU, the software-Vulkan capture, and the editor's two
viewports all consume that same prepared frame through that same encoder.

## Current boundary

The frame command enum intentionally contains only the render capabilities that exist today. It is
not a general render graph. New commands should be added when a working renderer needs them, while
asset resolution and GPU resource ownership remain outside serialized scene data.
