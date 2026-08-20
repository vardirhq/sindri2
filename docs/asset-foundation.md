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

Source completions remain encoded bytes until a runtime selects an `AssetDecoder`. `TextureAssetDecoder` accepts PNG and JPEG data and produces dimensions plus tightly packed RGBA8 pixels suitable for the existing `Texture2D::from_rgba8` GPU upload path. `SceneAssetDecoder` deserializes a `SceneDocument` and runs the normal version, stable-ID, parent, and hierarchy validation before the scene can become ready.

`decode_completion` preserves the request token and turns both source and decode failures into the common `AssetLoadError` model. Its `DecodedAssetCompletion::apply` method checks the retained handle generation before mutating an `AssetStore`, then drives the entry from loading to either ready or failed. A late completion for an expired generation returns a stale-completion error without touching the replacement entry.

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

## Deliberate boundaries

GPU upload stays outside. A loader that owned a device could not be tested without one, and the host is the only thing that has one: it reads a ready `TextureAsset` and puts it on the GPU itself. Fallback assets, final root/URL rules, hot reload, and a content-hashed manifest remain for the rest of the asset-system milestone. Keeping storage, source, scheduling, decoding, and GPU upload separate is what lets native and WebAssembly hosts share the same ownership and error semantics.
