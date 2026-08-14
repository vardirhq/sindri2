# Shared textured cube and sprite overlay

This example combines Sindri's perspective camera, depth-tested textured cube, orthographic camera, and alpha-blended textured sprite through one application shared by native desktops and WebGPU browsers. The circular badge is a transparent RGBA texture rendered as a 2D overlay after the 3D pass.

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
cargo run -p sindri-cube --bin capture -- target/render-artifacts/cube-sprite-overlay.png
```

Pull requests upload the resulting PNG as the `cube-sprite-overlay-preview` workflow artifact.
