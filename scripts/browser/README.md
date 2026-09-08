# Running the engine in a browser

> **Historical first-run note.** This records the cube proof that found the
> original browser-host failures. Current static exports fetch manifests and
> content-hashed assets, and Gather, Orbital Last Stand, Graphics Lab, and Weave
> run through the shared browser host; see `docs/export.md`.

The engine compiled for `wasm32` for several releases, CI checked it every time,
and nobody had ever loaded the page. Two things were broken the whole while, and
neither is the kind of thing a compile check can find:

- **A failure in a browser was silent.** `run` hands the event loop to the page
  with `spawn_app` and returns `Ok` immediately, so the error a host recorded had
  nobody to return to. The engine stopped at the device request and said nothing.
- **The surface refused every canvas.** A browser canvas offers `bgra8unorm` and
  no sRGB format at all, and the engine took that to mean colours could not be
  encoded. See `docs/rendering-color.md`: they can, through a view format.

So this exists.

```sh
npm install --prefix scripts/browser
wasm-pack build examples/cube --target web --out-dir pkg
node scripts/browser/smoke.mjs examples/cube target/browser.png
```

It serves the example, opens it in Chromium with WebGPU asked for explicitly,
waits for the device request, and exits non-zero if the page did not start the
engine. The tell is the canvas: one the engine never configured keeps the HTML
default of 300x150, which is the difference between "the page loaded" and "the
engine started".

`CHROME_PATH` points it at a browser that is already installed, for environments
that ship one rather than letting Playwright download it.

## What the first run proved

The module instantiated, `run` executed, winit adopted the page's canvas, a
WebGPU adapter and device opened, the surface configured, and the frame pipeline
drew the cube in the same colours as native.

At that snapshot the cube still embedded its texture and ran no Decay. Those
were real gaps then. They are closed by the shared project host and static
export path: browser smoke tests now load the generated manifest and hashed
assets, run Decay gameplay, verify audio promises, and exercise readable
capability failures under the GitHub Pages subpath.
