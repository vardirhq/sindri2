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

## Deliberate boundaries

This layer does not yet upload GPU resources automatically, provide fallback assets, define final root/URL rules, or watch files. Those responsibilities belong to the following asset-system milestones. Keeping storage, source, scheduling, decoding, and GPU upload concerns separate allows the same ownership and error semantics to be used by native and WebAssembly hosts.
