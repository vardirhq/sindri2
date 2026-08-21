# Running the engine in a browser

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

## What this proves, and what it does not

Proven: the module instantiates, `run` executes, winit adopts the page's canvas,
a WebGPU adapter and device open, the surface configures, and the frame pipeline
draws — the cube example renders the same picture in a browser as it does
natively, in the same colours.

Not proven: **assets are never fetched.** The cube example embeds its texture
with `include_bytes!`, so `AssetLoader`, `UrlRoot`, and the browser's fetch path
are still untested — the decoder runs, the loader does not. Decay has still never
executed in a browser either, since the cube runs no scripts. Both want an
example that needs them.
