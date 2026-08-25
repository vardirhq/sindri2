# Shared textured cube and sprite overlay

This example combines Sindri's authored perspective world camera, depth-tested textured cube, viewport-owned screen-space overlay, and an alpha-blended, five-instance sprite batch through one application shared by native desktops and WebGPU browsers. The world camera, cube, transforms, layers, and sprite instances are authored in `assets/demo.scene.json`, loaded through `sindri-core`, and extracted into a prepared frame before GPU work begins. The overlay needs no camera entity: its screen projection comes directly from the viewport. Five tinted instances of the circular badge share one transparent RGBA texture and render as a single instanced draw after the 3D pass. Frame and transparent ordering semantics are documented in [`docs/rendering-frame-pipeline.md`](../../docs/rendering-frame-pipeline.md) and [`docs/rendering-transparency.md`](../../docs/rendering-transparency.md).

Use the arrow keys to rotate the cube.

## Native

```bash
cargo run -p sindri-cube
```

## Browser

Install `wasm-pack`, then run from the repository root:

```bash
wasm-pack build examples/cube --target web --out-dir pkg
python -m http.server --directory examples/cube 8000
```

Open <http://localhost:8000>. A WebGPU-capable browser is required.

## Render capture

Generate the same deterministic offscreen image used by CI:

```bash
cargo run -p sindri-cube --bin capture -- target/render-artifacts/scene-frame-pipeline.png
```

Pull requests upload the resulting PNG as the `scene-frame-pipeline-preview` workflow artifact.
