# The first time this engine ran in a browser

A session note rather than a contract. It records what happened when the engine
was finally loaded in a browser, and what that found.

## Why it is worth writing down

`PROJECT_OVERVIEW.md` and `docs/FEASIBILITY.md` call this a WebGPU-first engine.
CI has run `cargo check --target wasm32-unknown-unknown` across the whole
workspace for several releases, `wasm-pack build examples/cube --target web` is a
documented command, and `examples/cube/index.html` has been in the tree the whole
time. Nobody had ever opened the page.

That is this repository's own documented failure mode — depth without a caller,
the reason `docs/capabilities.md` exists at all. The browser target had a green
CI column and no caller, and it was broken in two ways that a compile check
cannot see.

## What was broken

**A failure in a browser was silent by construction.** On a desktop, `run` calls
`event_loop.run_app`, and afterwards reads the error the host recorded. On
`wasm32` it calls `spawn_app`, which hands the loop to the page and returns
immediately — so `run` returns `Ok` before the app has started, and the recorded
error has nobody to return to. The engine stopped at the device request and said
nothing at all: no panic, no console output, a blank canvas.

`Host::fail` now logs as well as records. On a desktop that is redundant with the
returned error, which is fine; in a browser it is the only report there is.

**The surface refused every canvas.** A browser canvas offers `bgra8unorm` and no
sRGB format, and `SurfaceProfile` required one, so every page load ended in
`GpuError::NoSrgbSurfaceFormat`. The guard was right about the danger and wrong
about the conclusion — a canvas can be encoded to, through a view format. See
`docs/rendering-color.md`.

Both had been true since the browser target was added.

## What runs now

The cube example draws the same picture in Chromium as it does natively, in the
same colours: the module instantiates, `run` executes, winit adopts the page's
canvas, a WebGPU adapter and device open, the surface configures, and the frame
pipeline renders a textured cube and five alpha-blended sprites.

`scripts/browser/smoke.mjs` is how that is checked, and it exits non-zero when
the page does not start the engine. The tell it uses is the canvas: one the
engine never configured keeps the HTML default of 300x150, which is exactly the
difference between "the page loaded" and "the engine started". That is worth
knowing, because both look like a blank screen.

## What still has never run in a browser

**Asset loading.** Not one HTTP request for an asset was made. The cube example
embeds its texture with `include_bytes!`, so the *decoder* ran and the loader did
not: `AssetLoader`, its queue, and `UrlRoot` — which exists specifically so
browser URL rules are exercised on every target — are still only exercised by
tests. Something in the browser has to actually want a file before that path is
proven.

**Decay.** The cube runs no scripts, so the language has still only been compiled
for `wasm32`, never executed there. `ROADMAP.md` has said so all along and still
should.

**Input.** Nothing was typed at the page.

The obvious way to close all three at once is the companion game, which loads its
scene, fetches nothing today because it embeds too, and runs four scripts. That
is a better second caller than another example.
