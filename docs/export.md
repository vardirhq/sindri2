# Exporting a project to the web

```bash
cargo run -p sindri-export --bin sindri-export -- game dist --base /sindri2/
wasm-pack build game --target web --out-dir pkg
cp -R game/pkg/. dist/pkg/
```

The export writes the project; `wasm-pack` writes the host. They are separate on
purpose: building WebAssembly needs a toolchain the export does not want to own
or version, and a step that silently ran it would be one nobody could reproduce
by hand.

## What comes out

```text
index.html                 the page, with the base path and the project's name
pkg/                       the host, as wasm-pack built it
assets/manifest.json       what the project is made of — never cache this
assets/<content hash>/     every asset — cache this for ever
```

## What ships

Everything is worked out from the scene. A texture ships because a component
names it, a font because a text element does, a script because an entity runs
one, a sheet because it sits beside a texture that has one. So an asset that
stopped being used stops being carried, and one that started being used cannot
be forgotten.

The exception is in `sindri.toml`:

```toml
[assets]
include = ["audio/pickup.wav", "audio/victory.wav"]
```

A script can name a clip at run time — `Audio.play("pickup.wav")` is a string
inside a program, and no walk of a scene can see it. Scanning script text for
anything that looks like a path would ship whatever a comment mentioned and miss
whatever was built from a variable, so a project says instead.

## Cache invalidation

**`assets/manifest.json` must never be cached. Everything under
`assets/<hash>/` can be cached for ever.**

The directory is named after what every asset in it hashes to, so a build
differs from another build exactly when something a player would download
differs. A changed asset cannot land in a directory anyone has already cached,
and an unchanged build keeps its name so a re-deploy re-downloads nothing.

The manifest is the one file that has to be re-fetched, because it is how a
browser learns which directory to look in. It is small.

For a static host, that is one rule each:

```
/assets/manifest.json   Cache-Control: no-cache
/assets/*               Cache-Control: public, max-age=31536000, immutable
```

Exporting again removes the previous build's directory. Without that, every edit
would leave a whole copy of the project behind.

## Deployment, and the subpath

`--base` is where the export will be served from: `/` for a domain of its own,
`/repository-name/` for a GitHub Pages project site. It is baked into the page's
`<base href>` rather than guessed at run time, because a page that guessed is a
page that works locally and 404s once it is deployed.

The trailing slash is not optional and the export adds it: `<base href="/repo">`
resolves `pkg/host.js` against the *site* root, and `<base href="/repo/">`
resolves it inside the project. That is the whole GitHub Pages subpath problem.

## The host is not project-specific

The browser host reads the manifest and asks for what it names, kind by kind. It
used to carry a list of asset IDs per kind, compiled in — which meant adding a
texture meant editing Rust, and a project the host crate had never heard of
could not be exported at all. `AssetKind` in the manifest is what replaced that.

## What a browser is told when it cannot run this

The page checks for a canvas, for `navigator.gpu`, and for an adapter that
actually answers — because `navigator.gpu` existing is not the same as WebGPU
working, and Chrome on Android exposes the interface more widely than its
drivers can serve. Each failure produces a sentence a player can read rather
than a blank canvas. The engine's own failures arrive on `window` as
`sindri:failed` and are shown the same way.

`scripts/browser/smoke.mjs` runs against the exported directory in CI — the page,
the manifest, the hashed assets, and the deliberate removal of each capability
to prove the message appears.

## What this does not do yet

- **No compression.** Assets ship as they are; a host that serves gzip or brotli
  will do it on the wire, and pre-compressing is a size decision nothing has had
  to make yet.
- **No unused-scene pruning.** One scene is exported: the one the project names.
  A project with several would need to say which ship.
- **The host crate is named by hand.** The export takes a module name because a
  project does not say which binary will run it.
