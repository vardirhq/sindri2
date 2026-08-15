# CLAUDE.md

Guidance for AI assistants working in this repository.

## What this is

Sindri Next is a pre-alpha Rust game engine targeting native desktop and WebGPU
browsers, with a planned TypeScript SDK and an integrated native editor. It is a
Cargo workspace at foundation stage: the engine core, platform boundary, GPU and
renderer layers, asset pipeline, and an editor shell exist; the web SDK, 2D
migration, 3D product features, and isometric module do not.

Three documents govern the work and are the source of truth over anything
inferred from code:

- `PROJECT_OVERVIEW.md` — the product vision and architectural rules. Read the
  sections relevant to a change before making architectural decisions.
- `ROADMAP.md` — the checkable engineering plan, ordered by dependency. Milestone
  checkboxes reflect real state; tick one only when its acceptance criteria and
  tests are done.
- `docs/FEASIBILITY.md` — the non-negotiable decisions (runtime handles vs scene
  IDs, versioned scenes, coarse WASM calls, WebGPU-first, etc.).

Per-subsystem contracts live in `docs/`: `asset-foundation.md`,
`component-schema-registry.md`, `scene-serialization.md`, `scene-extraction.md`,
`rendering-frame-pipeline.md`, `rendering-transparency.md`, `rendering-color.md`,
`rendering-surface.md`, `platform-host.md`, `editor-architecture.md`. When you
change a subsystem's behaviour, update its doc in the same change.

Project policy lives alongside them: `docs/versioning.md` (crate and scene
format versions; the editor protocol and npm SDK are deliberately undecided) and
`docs/dependency-policy.md` (what `cargo deny` enforces). `CONTRIBUTING.md`
states the same conventions this file does, for humans.

## Workspace layout

```text
crates/
  sindri-core/      lifecycle, time, world/entities, scenes, commands, asset IDs
  sindri-platform/  host boundary: Clock, Game, EngineHost, input
  sindri-desktop/   winit window, event loop, and input translation (browser too)
  sindri-gpu/       wgpu adapter/device/queue and surface negotiation
  sindri-render/    target-independent renderers, frame stages, textures, cameras
  sindri-scene/     built-in sindri.* components; world -> frame extraction
  sindri-assets/    asset sources, load queue, decoding, URL resolution
  sindri/           public facade re-exporting the above (feature `render`)
editor/             sindri-editor: native eframe/egui editor (publish = false)
examples/triangle/  shared native + WebGPU triangle proof
examples/cube/      sindri-cube: scene-driven cube + sprite overlay, and the
                    `capture` binary used as a CI render artifact
docs/               subsystem contracts
scripts/            capture-editor.sh (Xvfb editor screenshot)
```

### Dependency rules (enforce these)

```text
sindri-core   -> (nothing in-workspace)
sindri-platform -> sindri-core
sindri-desktop  -> sindri-platform + sindri-gpu (it owns the window, so it
                   creates the surface)
sindri-assets   -> sindri-core
sindri-gpu      -> wgpu only (sindri-render is a dev-dependency, for tests)
sindri-render   -> wgpu, glam, bytemuck only
sindri-scene    -> sindri-core + sindri-render
sindri          -> assets, core, and (feature `render`) gpu, render, scene
```

- `sindri-core` depends on no window, GPU, browser, editor, physics, scripting,
  or async executor. Only `serde`, `serde_json`, `thiserror`.
- `sindri-render` does **not** depend on `sindri-core`. It knows nothing about
  worlds, components, or scenes. Do not add that dependency — `sindri-scene` is
  the seam that joins the two.
- Engine crates never depend on the editor.
- New crates are created at proven platform or dependency boundaries only, not
  to make the graph look complete.
- Known wrinkle: `editor` depends on `sindri-cube` (the example) for its demo
  scene and renderers. That is temporary scaffolding, not a pattern to extend.

## Commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features

cargo run --package sindri-editor          # native editor shell
cargo run -p sindri-triangle               # triangle proof
cargo run -p sindri-cube                   # cube + sprite overlay proof

# deterministic offscreen PNG (same one CI uploads)
cargo run -p sindri-cube --bin capture -- target/render-artifacts/scene-frame-pipeline.png

# browser targets
rustup target add wasm32-unknown-unknown
cargo check --workspace --all-features --target wasm32-unknown-unknown
wasm-pack build examples/cube --target web --out-dir pkg

# editor screenshot (needs imagemagick, xdotool, xvfb)
xvfb-run --auto-servernum ./scripts/capture-editor.sh target/editor.png

# regenerate golden scene fixtures, deliberately
SINDRI_UPDATE_SCENE_FIXTURES=1 cargo test --package sindri-core
```

CI (`.github/workflows/ci.yml`) runs fmt, clippy, tests, both render captures
(under Mesa software Vulkan, `WGPU_BACKEND=vulkan`), and the wasm32 check, with
`RUSTFLAGS: -D warnings`. **Any warning fails CI**, so clippy must be clean, not
merely non-fatal.

## Conventions

**Toolchain.** Rust 1.95.0 pinned in `rust-toolchain.toml`; edition 2024;
resolver 3. MSRV 1.95 is set by `wgpu` 30 and `egui` 0.36 — do not raise it
casually, and do not add a dependency that raises it.

**Lints.** `unsafe_code = "forbid"` workspace-wide; there is currently no unsafe
code anywhere. Clippy `all` + `pedantic` at warn, with only
`module_name_repetitions`, `must_use_candidate`, `missing_errors_doc`, and
`missing_panics_doc` allowed. Prefer fixing a pedantic lint over allowing it; a
new `#[allow]` needs a comment justifying it.

**Dependencies.** Shared versions go in `[workspace.dependencies]` and are
referenced as `dep.workspace = true`. Each crate ends with `[lints] workspace =
true`. `Cargo.lock` is gitignored, so a clean clone resolves fresh — a build that
depends on a pinned lockfile will not reproduce.

**Errors.** Every fallible API returns `Result<_, E>` with a `thiserror` enum
named after its module (`WorldError`, `SceneError`, `GpuError`, `HostError`,
`CommandError`). Errors name the thing that failed — the entity, the component
type, the asset ID, the unresolved reference. Do not panic in library code;
`expect` is used only where an invariant was just validated, with a message
saying which.

**Platform conditionals.** `#[cfg(target_arch = "wasm32")]` is confined to
`sindri-assets` (fetch vs filesystem), `sindri-desktop` (canvas attachment,
future spawning), `editor/src/main.rs`, and each example's logger setup.
Keep it there. Logic that is compiled only for wasm32 is logic nothing tests —
see `UrlRoot`, which exists specifically so browser URL rules are exercised on
every target.

**Documentation.** Every crate root has a `//!` doc comment stating what the
crate owns and, more importantly, what it deliberately does not. Public items
that carry a non-obvious rule get a doc comment explaining the rule rather than
restating the signature.

**Tests.** Unit tests live in `#[cfg(test)] mod tests` next to the code;
cross-cutting behaviour lives in `tests/` (`sindri-core/tests/scene_round_trip.rs`,
`sindri-scene/tests/extraction.rs`, `sindri-platform/tests/game_loop.rs`). Tests
assert behaviour that could actually regress. Anything GPU-dependent is designed
so the non-GPU half is testable — texture handles can be minted directly, frames
can be prepared and inspected without a device, and the whole game loop runs on
`ManualClock` with no window or sleeping.

**Commits.** Imperative, specific subject lines under ~55 characters ("Resolve
asset URLs where tests can reach them", "Make colour space impossible to get
wrong twice"). Bodies are prose paragraphs — no bullet lists — that explain what
was wrong, what the change does, and why that shape was chosen. Wrap at 80
columns.

**Changelog and roadmap.** Add user-visible behaviour to `CHANGELOG.md` under
`## [Unreleased]` → `### Added`, one sentence per entry in the same voice as the
existing lines. Tick the matching `ROADMAP.md` box when a milestone item is
genuinely complete; annotate partially-done items in parentheses rather than
ticking them.

## Architecture facts worth knowing before editing

**Runtime handles are not scene IDs.** `EntityId` is a generation-checked slot
handle that changes as entities spawn and despawn and is never serialized.
`SceneEntityId` is the project-authored stable string in the file. `to_scene`
writes back each entity's original ID; a runtime-spawned entity with no stable
ID produces `WorldError::UnstableEntity` rather than an invented one. Call
`World::assign_missing_source_ids` first.

**Scenes are canonical and round-trip byte for byte.** `SceneDocument::to_canonical_json`
sorts entities and keys, omits empty sections, keeps short scalar arrays on one
line, and is a fixed point. Golden fixtures in `crates/sindri-core/tests/fixtures`
enforce it — a formatting change fails tests instead of silently rewriting
everyone's scenes. Every document carries `format_version`; unsupported versions
are rejected, and `SceneMigrator` exists before format version 2 does.

**Unknown component payloads are preserved, not dropped.** Components are JSON;
`ComponentSchemaRegistry` types the ones code understands.
`UnknownComponentPolicy::Preserve` is the compatibility default,
`::Reject` is for proofs that require every component to be actionable. Editor-only
state lives in namespaced `editor` sections carried through untouched.

**Gameplay writes to the world; nothing tells the renderer.** `sindri-scene`'s
`SceneExtractor` derives an ordered frame from whatever the world currently
holds. Frames go through three stages — extraction, preparation (validates and
deterministically orders passes by stage, then layer, then insertion), rendering.
Stage order is `Opaque3d`, `Transparent2d`, `Overlay`. Do not add hand-written
extraction code to a scene.

**Colour space is load-bearing.** Offscreen and in-editor targets must use
`sindri_render::COLOR_TARGET_FORMAT`; swapchains must negotiate an sRGB format or
fail with `GpuError::NoSrgbSurfaceFormat`. A linear target renders, validates,
lints, and passes every test while being the wrong colour — which is why the
`capture` binary verifies the authored colours actually appear in the PNG. If you
touch render targets, keep that check working.

**Acquiring a swapchain texture has seven outcomes.** `WindowSurface::acquire` in
`sindri-gpu` applies the one policy for all of them and hands back a frame or
`None`; hosts must not re-derive it. Skipping and reconfiguring are not
interchangeable — reconfiguring on an occluded frame rebuilds the swapchain
every frame behind a minimised window. See `docs/rendering-surface.md`.

**Textures bind by reference.** A scene names `textures/badge.png`; the renderer
knows only `TextureId`. `TextureBindings` in `sindri-scene` is the only place
that knows both. An unbound reference draws the magenta `TextureRegistry::MISSING`
checker and is reported by `unresolved_textures`, rather than failing the frame
or reusing the last-bound texture.

**Editor/runtime parity is an explicit priority.** A feature is not complete if
only the runtime understands it. Adding a component means the engine understands
it, serialization saves it, the editor edits it, and (later) the web API exposes
it. The editor renders the real runtime frame through eframe's shared WGPU device
— it must never create a second device or reimplement scene rendering.

**Edits go through commands.** `WorldCommand` values each produce their own
inverse; `Transaction` is all-or-nothing; `CommandHistory` gives bounded
undo/redo with labelled grouping and merge runs that collapse a drag into one
step. Editor mutations use this path, not direct world writes.

**Asset loading is genuinely asynchronous.** `AssetId` is a validated relative
logical ID, never a path or URL. Sources resolve it against a root
(`FileSystemAssetSource` canonicalises to catch symlink escapes; `UrlRoot`
percent-encodes and normalises bases). `AssetLoadQueue` is bounded, rejects
duplicates, and carries a handle generation token so a late completion cannot
overwrite a replacement. Never fake synchronous browser I/O.

## Working style for this repo

- Follow the roadmap's dependency order. Do not start a later milestone's work to
  avoid a harder earlier one.
- Prefer making a mistake impossible over documenting that it is possible — one
  shared constant instead of two agreeing choices, a type that rejects bad input
  at construction instead of a validation call callers must remember.
- If a change cannot be caught by any existing check, add the check that would
  catch it (the pixel verification in `capture` is the model here).
- Keep examples curated. Two proofs exist because each proves something; do not
  accumulate demos.
- Deferred work is deferred deliberately — WebGL2 fallback, PBR, skeletal
  animation, networking, render graphs, and advanced ECS scheduling are listed as
  non-goals in `ROADMAP.md` and `PROJECT_OVERVIEW.md`. Do not add them
  opportunistically.
