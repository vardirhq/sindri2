# Shared depth-tested cube

This example exercises Sindri's perspective camera, indexed mesh buffers, depth target, and cube renderer through one application shared by native desktops and WebGPU browsers.

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
