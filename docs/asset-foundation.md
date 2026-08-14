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

Invalid transitions return `AssetStoreError`. Loading failures retain the logical asset ID, a structured `AssetLoadErrorKind`, and a source-provided diagnostic message. The future asynchronous loading queue and native/web asset sources will drive this state machine without adding an executor dependency to `sindri-core`.

## Deliberate boundaries

This layer does not yet resolve IDs, perform I/O, decode formats, upload GPU resources, or watch files. Those responsibilities belong to the following asset-system milestones. Keeping this foundation renderer-, platform-, and executor-independent allows the same ownership and error semantics to be used by native and WebAssembly hosts.
