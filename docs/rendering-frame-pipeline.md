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

The cube example is the reference integration. A versioned `SceneDocument` is deserialized from
JSON, validated, loaded into `World`, and extracted into an opaque textured-cube pass followed by a
five-instance overlay pass. Native, WebGPU, and software-Vulkan capture paths consume that same
prepared frame.

## Current boundary

The frame command enum intentionally contains only the render capabilities that exist today. It is
not a general render graph. New commands should be added when a working renderer needs them, while
asset resolution and GPU resource ownership remain outside serialized scene data.
