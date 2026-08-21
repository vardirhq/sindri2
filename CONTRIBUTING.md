# Contributing to Sindri Next

Sindri Next is pre-alpha. Public APIs and serialized formats still move, so the
most useful contributions are ones that make the foundation harder to get wrong
rather than ones that add surface area.

## Before writing code

Four documents govern the work and outrank anything inferred from the code:

- [`README.md`](README.md) — the current product identity, architecture summary,
  and high-level capability statement.
- [`ROADMAP.md`](ROADMAP.md) — the checkable plan, ordered by dependency rather
  than by excitement. Work the next item rather than a later, more appealing one.
- [`docs/FEASIBILITY.md`](docs/FEASIBILITY.md) — the decisions that are settled:
  runtime handles are not scene IDs, scenes are versioned from day one, WASM
  calls are coarse, WebGPU is the first browser target.
- [`docs/decay-direction.md`](docs/decay-direction.md) — the accepted Editor +
  Decay authoring direction and its decision record.

[`PROJECT_OVERVIEW.md`](PROJECT_OVERVIEW.md) preserves the original Rust +
TypeScript architecture proposal. Its TypeScript-first product model is
historical and does not override the current sources above.

Per-subsystem contracts live in [`docs/`](docs). If your change alters how a
subsystem behaves, update its document in the same change. A contract that
describes last month's behaviour is worse than no contract.

## Checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo check --workspace --all-features --target wasm32-unknown-unknown
```

CI runs these with `RUSTFLAGS: -D warnings`, so a warning is a failure. It also
renders three images — a headless scene capture, a screenshot of the editor, and
the companion game part-way through a scripted run — and uploads all of them,
because several classes of rendering mistake compile, lint, test, and run while
producing the wrong picture.

Dependency changes additionally run `cargo deny`; see
[`docs/dependency-policy.md`](docs/dependency-policy.md).

## What the code is expected to look like

**No unsafe.** `unsafe_code = "forbid"` applies to the whole workspace and there
is currently no unsafe code anywhere.

**Clippy pedantic is on.** Four lints are allowed workspace-wide; everything else
is a warning and therefore an error in CI. Prefer restructuring the code over
adding an `#[allow]`, and give any new `#[allow]` a comment saying why it earns
its place.

**Errors name what failed.** Every fallible API returns `Result<_, E>` with a
`thiserror` enum named after its module, and the message names the entity, the
component type, the asset ID, or the reference that could not be resolved.
Library code does not panic; `expect` appears only where an invariant was
validated a line or two earlier, with a message saying which one.

**Tests assert behaviour that could regress.** Unit tests live beside the code in
`#[cfg(test)] mod tests`; behaviour spanning crates lives in `tests/`. Anything
GPU-dependent is built so its non-GPU half stays testable — texture handles can
be minted directly, frames can be prepared and inspected without a device, and
the game loop runs on a manual clock with no window and no sleeping. If a change
cannot be caught by any existing check, add the check that would catch it.

**Platform conditionals stay at the edges.** `#[cfg(target_arch = "wasm32")]`
belongs in `sindri-assets`, the examples, and the editor's entry point. Logic
compiled only for `wasm32` is logic nothing tests, which is why browser URL rules
live in a module every target compiles.

**Crate boundaries are load-bearing.** `sindri-core` knows nothing about windows,
GPUs, browsers, or executors. `sindri-render` knows nothing about worlds,
components, or scenes — `sindri-scene` is the seam that joins them. Engine crates
never depend on the editor. New crates are created at proven boundaries, not to
round out the diagram.

## Commits

Subject lines are imperative, specific, and under about 55 characters:

```text
Resolve asset URLs where tests can reach them
Make colour space impossible to get wrong twice
```

Bodies are prose paragraphs wrapped at 80 columns — not bullet lists. Explain
what was wrong, what the change does, and why it has the shape it has. A reader
six months from now should be able to reconstruct the reasoning without the pull
request.

## Changelog and roadmap

User-visible behaviour goes in [`CHANGELOG.md`](CHANGELOG.md) under
`## [Unreleased]`, one sentence per entry, in the voice of the entries already
there. Tick the matching roadmap box only when the item is genuinely finished,
including its tests; annotate a partially finished item in parentheses instead of
ticking it.

## The capability list

[`docs/capabilities.md`](docs/capabilities.md) records what the engine and the
editor can actually do, what the editor draws without wiring up, and what is
missing. Update it in the same commit as any change that adds a capability,
removes one, or connects a control that was previously inert.

Entries describe what someone ran, not what a roadmap promises or a type
signature implies. A wrong entry is worse than a missing one, so correcting a
claim counts as a change worth making on its own.

## Versioning

Crate versions, the scene `format_version`, and what a breaking change means for
each are described in [`docs/versioning.md`](docs/versioning.md). Scene format
changes in particular need a migration before the version moves.

## Licensing of contributions

Sindri Next is dual-licensed under [Apache 2.0](LICENSE-APACHE) and
[MIT](LICENSE-MIT). Unless you state otherwise, any contribution you submit for
inclusion is licensed under both, with no additional terms, as described in
section 5 of the Apache 2.0 licence.
