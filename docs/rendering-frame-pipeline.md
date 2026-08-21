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
world may hide, together with screen-space text ordered by the same layers. See [scene extraction](scene-extraction.md) for which space a sprite is in and
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

Text is a frame command rather than host UI. `TextInstance` carries a logical
font reference, pixel position, metrics, colour, and content; `TextRenderer`
resolves only project fonts a host explicitly bound, shapes them with Glyphon,
and loads the existing colour attachment so text composes at its ordered overlay
layer. An absent font skips the string and is diagnosed by asset loading rather
than silently selecting a system fallback.

## Every sprite batch owns its buffers

A frame can hold several sprite batches, and each of them draws with its own
camera and its own instances. That is now true because each batch takes a slot
of its own — its own uniform buffer, its own instance buffer, its own bind
groups — rather than sharing one set with the rest of the frame.

It was not true before, and the way it failed is worth keeping written down.
The renderer held one uniform buffer and one instance buffer, and wrote both
once per batch on the way to encoding that batch's pass. `queue.write_buffer`
does not write when it is called: it stages the write, and every staged write
lands *before* the command buffer that follows executes. So all of a frame's
batches wrote in turn, the last one won, and every pass drew with the last
batch's camera and the last batch's instances. A frame with a single sprite
batch is unaffected, which was every proof, every example, and every test in
the workspace — while any scene that mixed world sprites with a screen overlay
drew the world through the overlay's camera. Building the companion game is
what found it, because the game is the first thing here with both.

`SpriteBatchRenderer::begin_submission` hands the first slot back, and slots are
reused rather than freed, so a frame costs nothing after the first one of its
shape. It is a *submission* boundary and not a frame boundary, for the same
reason the bug existed: a slot may only be reused once the commands that read
it have been submitted. `encode_prepared_frame` calls it, so one call is one
submission — a host drawing two frames at once, as the editor does with its
scene view and its game view, submits between them.

`crates/sindri-gpu/tests/sprite_batches_are_independent.rs` is the check. It
draws two batches into one frame, at two scales and with two instance counts,
and reads back the pixels that only come out right when each batch kept its
own.

## Current boundary

The frame command enum intentionally contains only the render capabilities that exist today. It is
not a general render graph. New commands should be added when a working renderer needs them, while
asset resolution and GPU resource ownership remain outside serialized scene data.
