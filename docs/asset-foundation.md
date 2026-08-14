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

Invalid transitions return `AssetStoreError`. Loading failures retain the logical asset ID, a structured `AssetLoadErrorKind`, and a source-provided diagnostic message. The future asynchronous loading queue will drive this state machine without adding an executor dependency to `sindri-core`.

## Asset sources

The separate `sindri-assets` crate resolves an `AssetId` to undecoded `AssetBytes` through one object-safe asynchronous `AssetSource` contract. Sources report structured, ID-aware errors that convert into the core load-error model.

The initial implementations are:

- `MemoryAssetSource` for deterministic tests, generated content, and editor tooling
- `FileSystemAssetSource` on native targets, with canonical-path checks that prevent symlinks from escaping the configured root
- `FetchAssetSource` on WebAssembly, using the browser Fetch API and retaining HTTP status and content-type information

Filesystem reads are intentionally not hidden behind a fake executor. They complete when their future is polled, so the upcoming native load queue must poll them on an I/O worker rather than the frame thread. Browser fetches remain genuinely asynchronous.

## Deliberate boundaries

This layer does not yet schedule concurrent loads, decode formats, upload GPU resources, define final root/URL rules, or watch files. Those responsibilities belong to the following asset-system milestones. Keeping storage and source concerns separate allows the same ownership and error semantics to be used by native and WebAssembly hosts.
