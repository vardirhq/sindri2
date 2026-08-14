# Shared triangle proof

This example uses the same `sindri-gpu` device negotiation and `sindri-render` triangle renderer on native desktops and in WebGPU browsers.

Native:

```bash
cargo run -p sindri-triangle
```

Web:

```bash
wasm-pack build examples/triangle --target web --out-dir pkg
python -m http.server --directory examples/triangle 8080
```

Then open `http://localhost:8080`. WebGPU support is required; WebGL fallback is intentionally deferred.

