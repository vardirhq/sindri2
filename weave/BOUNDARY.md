# Isolation contract

- `weave/*` never depends on `sindri-*`.
- `sindri-weave` is the sole language bridge.
- Existing foundational Sindri crates do not import Weave parser, selector, cascade, or media-query types.
- The proof of concept must not mutate the authored game world while resolving presentation.
