# Weave proof of concept

> **Status:** successful architectural proof, retained as a design record.
> [`weave.md`](weave.md) is the current integration guide and
> [`weave-reference.md`](weave-reference.md) is the implemented language
> reference.

This experiment asked whether a CSS-like responsive UI language could remain
isolated behind a single `sindri-weave` bridge. The answer is yes.

The `weave/` workspace owns language concerns and has no dependency on Sindri.
`crates/sindri-weave` is the sole bridge. It resolves styles into a disposable
clone of the authored `World`, allowing the existing Sindri UI, rendering, and
input pipelines to consume ordinary Sindri state without foundational engine
crates learning Weave syntax or semantics.

That boundary now supports selectors and specificity, responsive conditions,
composed stylesheets, percentage sizing, constraints, padding, gaps, wrapping,
alignment, shapes, and text presentation. `games/weave-poc` remains the focused
responsive showcase; Orbital Last Stand is the production-oriented proof, with
its screen UI split across four composed Weave files.

The cloned-world approach remains intentionally replaceable. A future generic
resolved-UI seam could remove the clone without changing the independent
language workspace or teaching Weave syntax to core engine crates.
