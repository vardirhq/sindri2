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

The companion game runs too, and is playable — see below.

The production-shaped build is published at
<https://vardirhq.github.io/sindri2/>. `.github/workflows/pages.yml` builds
`game/` with wasm-pack, assembles only its static HTML/module/Wasm output, and
deploys it through GitHub's Pages artifact flow whenever `main` changes. The
page uses relative imports, so the repository's `/sindri2/` base path is part of
the real delivery path rather than something development serving hides.

`scripts/browser/smoke.mjs` is how that is checked, and it exits non-zero when
the page does not start the engine. The tell it uses is the canvas: one the
engine never configured keeps the HTML default of 300x150, which is exactly the
difference between "the page loaded" and "the engine started". That is worth
knowing, because both look like a blank screen.

## The game, which is the more interesting caller

The cube proves a frame can be drawn. The companion game proves the engine can be
*played*, and it was the second thing pointed at a browser for that reason. It
needed a `cdylib` crate type and a page, and nothing else: the `wasm_bindgen`
entry point and the wasm-only dependencies were already there.

Driven through the same browser host and fixed-step gameplay as native, the
player crosses the floor, an orb disappears, and the first lamp lights. That
single frame is the whole chain running on the browser target:

- the keyboard reaches `player.decay`, which moves and clamps a logical
  coordinate before `Grid.place` projects it through the floor tilemap
- `orb.decay` calls `World.find("Player")` and compares both entities in that
  same grid through typed references
- the pickup writes the score to the blackboard with `Game.set`
- `pip.decay` reads it and lights a lamp

**So Decay executes in a browser**, which it never had. `ROADMAP.md` had said so
truthfully for as long as the item existed. Entity references, the blackboard,
the fixed step, and input all came with it.

Two frames seven hundred milliseconds apart also differ by around three thousand
pixels with nothing touched, because the orbs bob. That is the cheapest available
proof that scripts are running rather than that a scene loaded.

## What still has never run in a browser

**Asset loading.** Not one HTTP request for an asset has been made by either
caller. The cube embeds its texture with `include_bytes!` and the game embeds its
scene, scripts, sheets and textures the same way, so the *decoders* run and the
loader does not: `AssetLoader`, its queue, and `UrlRoot` — which exists
specifically so browser URL rules are exercised on every target — are still only
exercised by tests.

Embedding is a legitimate way to ship a game, so this is not a fault in the game.
It does mean the asset pipeline's browser half needs a caller of its own, and the
honest candidate is the editor rather than another example: an editor opens files
it was not compiled with, which is exactly what fetching is for.
