# Shared textured cube

This example exercises Sindri's perspective camera, indexed UV mesh buffers, procedural RGBA texture upload, filtering sampler, depth target, and textured cube renderer through one application shared by native desktops and WebGPU browsers.

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
cargo run -p sindri-cube --bin capture -- target/render-artifacts/textured-cube.png
```

Pull requests upload the resulting PNG as the `textured-cube-preview` workflow artifact.
