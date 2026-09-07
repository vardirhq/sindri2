# Weave proof of concept

This branch tests one narrow architectural question: can a CSS-like responsive UI language sit behind a single `sindri-weave` bridge without teaching foundational Sindri crates about the language?

The POC keeps the language in a separate `weave/` workspace. The `weave` crate parses selectors, declarations, and a deliberately tiny media-query surface. It has no dependency on any `sindri-*` crate.

`crates/sindri-weave` is the only bridge. For now it clones the authored `World`, applies matching Weave declarations to that disposable presentation world using existing Sindri UI components/transforms, and hands the clone back to the normal Sindri scene/render/input pipeline. This is intentionally not the proposed shipping architecture; cloning is a low-risk way to prove language and responsive-layout semantics without modifying existing engine implementation.

The POC currently targets stable scene IDs and component-type selectors, `width`, `height`, `x`, `y`, `anchor`, `direction`, `gap`, and `font-size`, with `px`, `vw`, `vh`, numeric overlay units, `max-width`, `min-width`, and orientation media conditions.

## Boundary under test

- `weave/*` must not depend on Sindri.
- `sindri-weave` may depend on both Weave and Sindri.
- Existing foundational Sindri crate source should remain unchanged for the POC.
- The authored `World` must remain unchanged after style resolution.

If this experiment proves useful, the next architectural step is not to keep cloning worlds. It is to introduce a generic resolved-UI/presentation seam owned by Sindri and feed it from `sindri-weave`, while keeping selectors, cascade, media syntax, parsing, and diagnostics outside foundational engine crates.
