# AGENTS.md

Repository guidance for AI coding agents working on Sindri Next.

Read this file before making changes. Repository documents and the current code
are authoritative over remembered context from earlier sessions.

## Product direction

Sindri Next is a pre-alpha Rust game engine targeting native desktop and WebGPU
browsers, with a native editor, the Decay gameplay language, and Gather as its
companion game. The engine is developed vertically: runtime capability,
authoring, scripting, and a real game should evolve together where the feature
applies.

The companion game is not a disposable demo. **Relevant gameplay capabilities
are not complete until Gather uses them in a real gameplay context.** A unit
test, component type, editor control, or callable Decay API is necessary evidence
but is not a substitute for exercising the feature in the game.

## Read before changing architecture

These documents govern the work:

- `README.md` — current product identity and high-level capability statement.
- `ROADMAP.md` — engineering plan ordered by dependency. Check an item only when
  its acceptance criteria and relevant tests are complete.
- `docs/FEASIBILITY.md` — non-negotiable architectural decisions and risks.
- `docs/dependency-policy.md` — dependency, licence, source, and MSRV policy.
- `docs/decay-direction.md` — accepted Editor + Decay authoring direction.
- `docs/scripting.md` and `decay/LANGUAGE.md` — scripting contracts.
- `docs/capabilities.md` — detailed evidence for what actually works.
- `docs/function-matrix.md` — 30-second Engine / Editor / Script checklist.
- `docs/module-layout.md` — how a source file is sized and split.
- `docs/feature-integration-matrix.md` — cross-surface integration status.

Subsystem contracts live in `docs/`. If a subsystem's behaviour changes, update
its contract in the same change.

## Dependency boundaries

Do not casually change the crate graph. The intended in-workspace direction is:

```text
sindri-core       -> (nothing in-workspace)
sindri-grid       -> (nothing in-workspace)
sindri-platform   -> sindri-core
sindri-desktop    -> sindri-platform + sindri-gpu
sindri-assets     -> sindri-core
sindri-gpu        -> wgpu only (render is dev-only)
sindri-render     -> wgpu + glam + bytemuck only
sindri-scene      -> sindri-core + sindri-grid + sindri-render
sindri            -> assets + core + grid + optional gpu/render/scene
sindri-decay      -> core + grid + platform + decay language crates
editor            -> assets + core + decay + platform + render + scene
sindri-gather     -> consumer of the engine; nothing depends on it
```

Important constraints:

- `sindri-core` has no window, GPU, browser, editor, physics, scripting, or async
  executor dependency.
- `sindri-render` does not depend on `sindri-core`; `sindri-scene` is the seam.
- Engine crates never depend on the editor or Gather.
- `decay/` is a separate Cargo workspace and may not depend on `sindri-*` crates.
  `sindri-decay` is the one-way bridge into the language.
- Create a new crate only at a proven platform or dependency boundary.
- Before adding a dependency, check MSRV 1.95, WASM compatibility where required,
  licence policy, and `deny.toml`.

## Capability completion rule

A surface earns ✅ in `docs/function-matrix.md` only when the behaviour is
implemented **and exercised** on that surface. Do not mark an API, schema,
component, or editor control complete merely because it exists.

When a capability changes, update the relevant documentation in the same commit:

- `docs/function-matrix.md` for terse Engine / Editor / Script status.
- `docs/capabilities.md` for detailed evidence and limitations.
- `docs/feature-integration-matrix.md` when cross-surface status changes.
- The relevant subsystem contract in `docs/` when behaviour changes.
- `CHANGELOG.md` for user-visible behaviour.
- `ROADMAP.md` only when an item's real acceptance criteria are complete.

For gameplay capabilities, include the corresponding Gather evolution in the
same feature track. If Gather cannot reasonably exercise the capability, explain
why in the feature documentation rather than silently skipping the proof.

## Working method

Prefer small, reviewable feature slices over giant implementation commits. For a
large subsystem, establish the architecture and dependency boundary before
writing the implementation.

Do not use CI as the primary debugger. Before pushing code, run every relevant
check that can reasonably be reproduced locally. When a check fails, inspect the
whole affected path rather than patching only the first diagnostic and pushing
again.

Do not introduce temporary self-modifying workflows or repository automation to
work around ordinary development problems. If the implementation approach starts
requiring machinery whose only purpose is to repair the branch, stop and reassess
the approach.

Before declaring a PR ready:

1. Review the final diff from `main`, not just the latest commit.
2. Re-check dependency direction and target-specific `cfg` behaviour.
3. Verify documentation and Gather integration required by the capability rule.
4. Run the required native, Decay, WASM, browser, render, or dependency checks
   that the touched code can affect.
5. Confirm CI is green on the final head.

## Required checks

For the main workspace:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo check --workspace --all-features --target wasm32-unknown-unknown
scripts/check-file-size.py
```

CI sets `RUSTFLAGS=-D warnings`; warnings are failures. Changes affecting render
output must keep the deterministic captures and colour verification green.
Changes affecting browser behaviour must run the real browser smoke tests, not
only compile WASM. Changes affecting dependencies must satisfy `cargo deny` and
the repository dependency policy.

For the separate Decay workspace:

```bash
cd decay
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run additional subsystem-specific tests described by the relevant docs and CI
workflow. Do not claim a target or surface works merely because another target
compiled.

## Core conventions

- Rust 1.95.0, edition 2024, resolver 3. Do not raise the MSRV casually.
- Workspace forbids unsafe code.
- Prefer fixing Clippy pedantic warnings over adding `#[allow]`; justify any new
  allowance in a comment.
- Shared dependency versions live in `[workspace.dependencies]`.
- Library code returns typed errors rather than panicking.
- Runtime `EntityId` handles are not serialized `SceneEntityId` values.
- Scenes are versioned, canonical, and preserve unknown component payloads.
- Gameplay writes the world; `sindri-scene` derives renderer/navigation state.
- Project fonts are assets, never operating-system lookups.
- Asset loading is genuinely asynchronous; never fake synchronous browser I/O.
- Browser and native loops share semantics, not identical plumbing.
- WebGPU is the first browser backend; WebGL fallback remains deliberately
  deferred unless the roadmap changes.
- Editor mutations go through checked commands and undo/redo, not direct world
  writes.
- Rust source files stay under 600 lines and aim for 400. Split by
  responsibility, not by line count; see `docs/module-layout.md`.

## Commits

Use imperative, specific subject lines under roughly 55 characters. Commit
bodies should explain what was wrong, what changed, and why that shape was
chosen. Keep unrelated cleanup out of feature commits.
