# Asset foundation

Sindri's asset foundation separates stable authored identity from platform-specific location and runtime ownership.

## Logical identity

`AssetId` is a validated, serializable, relative identifier such as `textures/player.png`. It is not a filesystem path or URL. A later asset-source layer will resolve the same ID against the editor, a packaged native game, or an HTTP origin.

IDs use `/` separators and reject absolute paths, empty or dot segments, backslashes, control characters, query strings, and fragments. Validation happens during construction and deserialization so invalid identifiers cannot enter scene or project data unnoticed.

## Typed runtime handles

`AssetStore<T>` returns `AssetHandle<T>`, preventing a texture handle from being used with an audio or mesh store. Repeated requests for the same live logical ID share one generation and one load operation.

Strong handles own liveness, not the loaded value directly. `WeakAssetHandle<T>` can refer to an asset without keeping it alive and can be upgraded only while a strong handle from the same generation exists. Expired weak handles are never revived by a later request.

The store intentionally releases nothing during `Drop`. A runtime calls `collect_unused` at a deliberate maintenance point, making destruction timing observable and testable instead of dependent on whichever thread drops the final handle.

## State and error model

Each entry follows a checked state machine:

```text
queued -> loading -> ready
                  -> failed -> queued (retry)
```

Invalid transitions return `AssetStoreError`. Loading failures retain the logical asset ID, a structured `AssetLoadErrorKind`, and a source-provided diagnostic message. `sindri-core` remains executor-independent; the separate asset queue drives source work and returns completions for the runtime to apply to the store.

## Asset sources

The separate `sindri-assets` crate resolves an `AssetId` to undecoded `AssetBytes` through one object-safe asynchronous `AssetSource` contract. Sources report structured, ID-aware errors that convert into the core load-error model.

The initial implementations are:

- `MemoryAssetSource` for deterministic tests, generated content, and editor tooling
- `FileSystemAssetSource` on native targets, with canonical-path checks that prevent symlinks from escaping the configured root
- `FetchAssetSource` on WebAssembly, using the browser Fetch API and retaining HTTP status and content-type information

### Where a source looks

The same `AssetId` must find the same file whether it is read from disk or fetched over HTTP, so
both sources resolve it against a configured root and neither lets an ID reach outside one.

`AssetId` does most of the work up front: an ID is relative, uses `/` separators, and cannot contain
`\`, `:`, `?`, `#`, control characters, or a `.` or `..` segment. There is no ID that escapes a root
by construction.

On native targets `FileSystemAssetSource` joins the ID to its root and canonicalises the result,
rejecting anything that lands outside — which also catches symlinks pointing away from the root, a
case path arithmetic alone would miss.

On the web `UrlRoot` builds the URL. A base may be empty (assets sit beside the page), relative
(`assets/`), root-relative (`/games/demo/`), or absolute (`https://cdn.example.com/v2/`), and is
normalised to end in a single slash. Bases carrying a query string or fragment are refused: the
asset path is appended to the base, so anything after it would land in the middle of the URL and
quietly request the wrong thing. Each path segment is percent-encoded to RFC 3986's unreserved set,
so an ID containing spaces or non-ASCII resolves to a URL that requests the file it names, while the
separators between segments stay separators.

Resolution lives in `sindri-assets` rather than inside the browser source, so it is tested on every
target. Rules compiled only for `wasm32` are rules nothing exercises.

Filesystem reads are intentionally not hidden behind a fake executor. They complete when their future is polled, so the native load queue creates and polls them entirely on bounded I/O workers rather than the frame thread. Browser fetches remain genuinely asynchronous.

## Asynchronous load queue

`AssetLoadQueue` accepts bounded `AssetLoadRequest` values and produces `AssetLoadCompletion` values. Every request contains both the logical ID and the originating typed handle's generation. A runtime checks that token against its retained handle before applying a completion, preventing a slow result for an expired asset from overwriting a replacement generation.

On native targets, a fixed worker pool owns the complete source future lifecycle. This keeps synchronous filesystem work off the frame thread. On WebAssembly, the queue holds local futures and polls them without blocking whenever the runtime drains completions; the browser Fetch API supplies actual asynchronous progress between those polls. Neither path requires an executor in `sindri-core`.

Queue capacity includes waiting, active, and completed-but-undrained requests. Enqueueing never waits: duplicate requests and backpressure are returned as explicit errors. The intended runtime flow is:

1. Create a typed handle with `AssetStore::request`.
2. Build and enqueue `AssetLoadRequest::new(&handle)`.
3. Mark the store entry as loading only after enqueue succeeds.
4. Drain completions at a deliberate update point.
5. Verify `completion.request().matches(&handle)` before completing or failing the store entry.

## Typed decoding

Source completions remain encoded bytes until a runtime selects an `AssetDecoder`. `TextureAssetDecoder` accepts PNG and JPEG data and produces dimensions plus tightly packed RGBA8 pixels suitable for the existing `Texture2D::from_rgba8` GPU upload path. `SceneAssetDecoder` deserializes a `SceneDocument` and runs the normal version, stable-ID, parent, and hierarchy validation before the scene can become ready. `SpriteSheetAssetDecoder` does the same for the sidecar that slices a texture into named sprites — its own decoder rather than text a caller parses, because a malformed sheet should fail as the asset it is, naming itself, rather than arriving as a string that fails somewhere later with no idea where it came from.

`decode_completion` preserves the request token and turns both source and decode failures into the common `AssetLoadError` model. Its `DecodedAssetCompletion::apply` method checks the retained handle generation before mutating an `AssetStore`, then drives the entry from loading to either ready or failed. A late completion for an expired generation returns a stale-completion error without touching the replacement entry.

## Assets that describe other assets

A sprite sheet is an asset *about* a texture, and its ID says so without anybody declaring it:
`textures/tiles.png` is sliced by `textures/tiles.sheet.json`. Derived rather than declared, because
a scene naming its sheets would be a place the pairing could be got wrong. `sheet_id_for` is that one
rule, and both the editor looking on disk and a game shipping embedded bytes go through it.

A sheet is only requested when something references a part of the texture it slices — a reference to
a whole image needs no sheet. That is what keeps a *missing* sheet an error worth reporting rather
than the ordinary case for every unsliced texture in a project. The one exception is an animated
sprite, whose own reference names no part because its clips do; `referenced_sheets` knows about that
case explicitly, because nothing in the sprite's reference reveals it.

## Driving all of it

Everything above was in place for a release and had no caller. The demo's badge was `include_bytes!`, the editor bound two textures a demo crate handed it, and the only stage with a user was the decoder. The reason is visible in the shape: loading one asset correctly is six steps in a particular order — request a handle, enqueue it, move the entry to loading, drain, decode, apply against the handle that is still current — and each one fails quietly rather than loudly when it is skipped.

`AssetLoader<D>` is those six steps written once. It owns a store, a queue, and a decoder; `request` is idempotent, so a scene naming one texture from twenty entities costs one load; `poll` drains, decodes, applies, and reports each asset as ready or failed exactly once. A completion whose handle generation has been superseded is dropped in silence, because the replacement is still coming. A failure is an answer: asking again does not retry, and `retry` says so explicitly.

One trap it hides is worth naming, because it is the kind that would be found late. Taking a fresh handle for an asset that failed does not reset the entry behind it, so a naive retry moves a `Failed` entry straight to `Loading` and is refused. The loader puts it back to queued first.

The loader also owns liveness. It holds a strong handle per asset, which is what keeps store entries alive at all, and `retain` is how a host narrows that to what it still wants — returning the IDs that were released, so whatever was built on top of them can go too.

## Reloading

`AssetWatch` notices that the file behind an asset has changed, and `AssetLoader::reload` loads it again. Together they are hot reload for native development, which is the point at which an editor stops being a thing you restart to see a texture you just saved.

It polls modification times rather than subscribing to filesystem events. A watcher crate would be more efficient and would bring a background thread, a platform-specific event model, and a set of coalescing rules to get wrong; the set of files being watched is a scene's assets, which is tens, and stating tens of paths once a second costs nothing measurable. Both the modification time and the length are read, because either alone misses edits — a filesystem recording whole seconds cannot separate two saves within one, and a rewrite of the same length leaves the length saying nothing. A same-second edit that preserves the length exactly is missed, which is stated rather than pretended away.

Existing and not existing are both states, so an asset appearing where one was missing is a change: a load that failed becomes a working one without a restart. Reporting a change updates what is remembered, so one change is reported once; a caller that declines to act is not told again, which is better than a watcher that repeats itself forever.

`reload` differs from `retry` in what it starts from and what it means: one is "that did not work", the other is "that worked and is now stale". An asset already in flight is left alone, because the read under way may be picking up the new bytes anyway.

Native only. A browser has no modification time to read, and no editor to reload into.

## Decoding the same way everywhere

The promise this layer exists for is that one scene file works from disk and from static web hosting. That has to hold for the bytes as well as the paths: a texture that decodes natively and not in the browser build, or decodes to something slightly different, turns "the same scene" into a claim nobody checked — and it would be found by someone looking at a picture rather than by a test.

`crates/sindri-assets/tests/decode_compatibility.rs` runs one body on both targets: `#[test]` natively, `#[wasm_bindgen_test]` under `wasm32-unknown-unknown`, where wasm-bindgen's runner executes it in Node. The corpus is embedded rather than read from disk, because what is under test is the decoder and not the source.

It is deliberately awkward — every colour type PNG defines, sixteen bits per channel, an interlaced encoding, and a JPEG — because those are the paths where a decoder's feature set can differ between builds. Each image is two by two, small enough that every expected pixel is written down: an encoding with no alpha channel has to arrive opaque, a palette has to be resolved to colours, sixteen bits have to narrow to eight, and an interlaced encoding has to produce the same pixels as the progressive encoding of the same image. The JPEG is checked with a tolerance, since it is lossy and a decoder producing the wrong picture would miss by far more than a rounding step. Bytes that are not an image have to be an error naming the asset on both targets, because a decoder that panicked would take a browser tab down with it.

Running it needs `wasm-bindgen-cli` at the version the workspace resolves; `.cargo/config.toml` names the runner and the install command.

## What a project ships

A scene names `textures/badge.png`. On a developer's machine that resolves to a file, and a wrong one fails loudly. On static web hosting it resolves to a URL, and the ways it can be wrong are quieter: a truncated response, a stale CDN entry, a deploy that replaced half the files. The bytes arrive, they decode, and the picture is last week's.

`AssetManifest` is the project saying in advance what each asset is — its length and the SHA-256 of its stored bytes, before any decoding. A build knows what to publish without walking a directory at deploy time, and a load can check what arrived against what was promised. It is not a security boundary, since anyone who can replace an asset can replace the manifest beside it, but it is the digest the browser's subresource integrity uses, so the day it feeds a `<link integrity>` the numbers are already right.

The file is versioned like a scene, ordered by asset ID so a diff shows the asset that changed and nothing else, and canonical without needing a canonicaliser: the structure is flat and the assets sit in a sorted map, so pretty-printing is already stable. The hash is written as `sha256:` and sixty-four hex characters — hex rather than base64 because a manifest is read in review, and the bytes base64 would save are not worth squinting at.

An asset the manifest does not mention loads normally. A manifest is a statement about what it lists, not a claim that nothing else exists, which is what keeps it a promise rather than a requirement. `AssetLoader::with_manifest` holds arriving bytes to it, checking the length first because that is free and is what a truncated response fails on. The editor picks up `sindri.manifest.json` from the directory a scene lives in if there is one, and treats a malformed one as absent rather than fatal — it describes the assets, and refusing to open a scene because a file beside it is broken would be refusing to let anyone fix it.

`examples/cube/assets/sindri.manifest.json` is committed rather than generated at deploy time, because a manifest built from whatever happened to be on the deploy machine is not a promise about anything. A test regenerates it and compares, so editing an asset without updating the manifest fails there rather than at somebody's browser.

## Deliberate boundaries

GPU upload stays outside. A loader that owned a device could not be tested without one, and the host is the only thing that has one: it reads a ready `TextureAsset` and puts it on the GPU itself. Fallback assets, final root/URL rules, hot reload, and a content-hashed manifest remain for the rest of the asset-system milestone. Keeping storage, source, scheduling, decoding, and GPU upload separate is what lets native and WebAssembly hosts share the same ownership and error semantics.
