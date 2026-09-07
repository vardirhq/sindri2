# Weave proof of concept

This experiment asks whether a CSS-like responsive UI language can remain isolated behind a single `sindri-weave` bridge.

The `weave/` workspace owns only language concerns and has no dependency on Sindri. `crates/sindri-weave` is the sole bridge. For the proof of concept, the bridge will resolve styles into a disposable clone of the authored `World`, allowing the existing Sindri UI/render/input pipeline to consume ordinary Sindri state without foundational engine crates learning Weave syntax or semantics.

The cloned-world approach is deliberately disposable. If the language proves useful, the intended production direction is a generic resolved-UI seam owned by Sindri rather than permanent world cloning.
