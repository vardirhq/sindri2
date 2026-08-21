<p align="center">
  <img src="assets/branding/sindri-readme-hero.jpg" alt="Sindri Next" width="680">
</p>

<p align="center"><strong>A lightweight 2D + 3D game engine built in Rust, with a native visual editor and Decay scripting.</strong></p>

Sindri Next is a from-the-foundation evolution of Sindri Engine. It is built around one runtime model, one scene format, one asset pipeline, and one renderer that can target native desktops and WebGPU browsers without turning the web into a separate edition of the engine.

> **Status:** pre-alpha and under active development. Public APIs, Decay, and serialized formats may change while the engine is being proven through real editor and gameplay use.

## What Sindri is trying to be

Sindri is deliberately small enough to understand and opinionated enough to build with.

- **2D and 3D in one engine** without forcing 2D through needlessly complicated 3D abstractions.
- **A real visual editor** that edits the same world and scene representation the runtime uses.
- **Decay scripting** for gameplay, with typed editor-visible properties and a narrow, checked engine API.
- **Native and web from the same foundation**, using Rust, `wgpu`, `winit`, and WebGPU/WASM where appropriate.
- **Portable project data** with versioned, canonical, review-friendly scene files and logical asset IDs.
- **Measured complexity**: systems are kept simple until profiling or real use proves they need to become more complicated.

Sindri is not trying to win a feature-count contest with mature engines. The current goal is to make the foundation coherent, testable, authorable, and difficult to accidentally lie about.

## Decay

Decay is Sindri's gameplay scripting language. The language itself is engine-agnostic; `sindri-decay` is the single binding layer that gives symbolic script paths meaning inside a Sindri world.

```rust
script Player {
    @export
    var speed: f32 = 6.0;

    fn update(dt: f32) {
        let movement = Input.axis("ArrowLeft", "ArrowRight");
        this.transform.position.x += movement * speed * dt;

        if Input.just_pressed("Space") {
            print("Back to the middle");
            this.transform.position.x = 0.0;
        }
    }
}
```

A script can currently read and write its transform and sprite properties, read keyboard input and frame time, call a small maths standard library, and print to the host log. Host members are typed: a misspelling such as `this.transfrom.position.x` is a compile error with a source location rather than a runtime surprise.

`@export` fields are visible in the editor without running the script. The script declares the property's name, type, and default; the scene stores only the value authored for that particular entity.

Decay is intentionally still small. It does not yet have entity-reference values, spawning/despawning, structured value types such as vectors, or a large standard library. See [`decay/LANGUAGE.md`](decay/LANGUAGE.md) for the language reference and [`docs/scripting.md`](docs/scripting.md) for the exact Sindri surface scripts can reach.

## What works today

### Engine

- strict lifecycle semantics with fixed-step simulation, capped frame time, and deterministic manual-clock tests
- generation-checked entities, safe hierarchies, recursive destruction, and one 2D/3D transform model
- versioned scene documents with stable authored IDs separate from runtime handles
- canonical, lossless world-to-scene saving and a scene migration API
- reversible world commands, transactions, bounded undo/redo, and dirty-state tracking
- keyboard and pointer input behind a platform-independent boundary
- headless, native-windowed, and browser host loops
- portable logical asset IDs, asynchronous loading, typed decoding, manifests, hot reload, and generation-checked texture handles
- shared `wgpu` device/surface policy for native and WebGPU
- extraction/preparation/rendering stages with deterministic pass ordering
- depth-tested 3D meshes, perspective and orthographic cameras, textured/tinted/layered 2D sprites, sprite sheets, and runtime sprite animation
- deterministic offscreen PNG rendering exercised by CI
- Decay scripts that run against the live world through a typed host surface

### Editor

The native editor is already an authoring tool rather than a mock shell. It works on the same world and scene representation as the runtime.

It currently supports:

- opening, saving, reloading, and discarding canonical scene files
- a searchable live hierarchy with selection and parenting
- creating and deleting entities, including undo that restores deleted entities at their original handles
- editing names and transforms
- adding, removing, and editing component payloads through the component schema registry
- editing unknown preserved components instead of silently throwing their data away
- editing Decay `@export` properties directly in the inspector
- undo/redo with drag merging and saved-state tracking
- Scene and Game views with perspective/orthographic viewing, orbit, pan, zoom, and focus-selection
- Play, Pause, and Stop using the real engine lifecycle
- Decay execution and sprite animation during play, with the world restored to its pre-play state on Stop
- keyboard input routed to scripts only while the game owns it
- project browsing, console output, preferences, remembered layouts, and reopening the previous scene
- texture and Decay source hot reload through the real asset pipeline
- deterministic editor screenshot capture in CI

For the deliberately exhaustive and evidence-based inventory, including controls that are still incomplete, see [`docs/capabilities.md`](docs/capabilities.md).

## Architecture

Sindri keeps engine concerns split into focused crates, while Decay remains its own workspace.

```text
                         Sindri Editor
                              |
                       live World / Scene
                      /       |        \
                 Assets    Rendering   Decay host
                    |          |           |
              sindri-assets  sindri-render  sindri-decay
                               |           |
                           sindri-gpu   decay-runtime
                               |           |
                              wgpu      decay-ir
                               |           |
                       Native / Web    decay-semantic
                                           |
                                      decay-syntax
```

The important boundary is intentional:

```text
decay/*                 knows the language, not Sindri
crates/sindri-decay     knows both halves
sindri engine crates    do not depend on Decay
```

That keeps the scripting decision reversible and prevents engine concepts from leaking into the language implementation. `sindri-decay` also performs no I/O; script sources arrive through the same asset pipeline as the rest of a project.

The renderer-independent core has no dependency on a window, GPU, browser, editor, physics engine, scripting runtime, or async executor.

## Scene and component model

Scenes are versioned JSON documents intended to survive both source control and editor round-trips. Stable authored IDs are deliberately different from generation-checked runtime handles, unknown component payloads are preserved by default, and serialization is canonical so saving an untouched scene reproduces it byte-for-byte.

Components are backed by a schema registry used by both runtime validation and editor authoring. This is why the inspector can edit component payloads generically instead of accumulating a hand-written panel for every component Sindri ever gains.

## Native and web

The web remains a first-class runtime target. Native and browser hosts share the same engine concepts, scene extraction, assets, GPU abstraction, renderer, and presentation policy.

The old plan described a separate TypeScript authoring API for browser games. That is no longer the primary gameplay direction: Decay is being developed as the common gameplay scripting layer so a game's authoring model does not depend on where it will run.

## What is still missing

Sindri is pre-alpha, and several large pieces are intentionally not pretending to exist yet. Among the important gaps are:

- physics and collision gameplay
- audio
- richer Decay value types, entity references, spawning, and access to other entities
- broader gameplay stepping beyond scripts and sprite animation in editor play mode
- transform rotation editing and scene gizmos
- sprite-sheet and animation authoring tools
- richer asset inspectors and project authoring workflows
- production build/export/package tooling
- a mature 3D feature set beyond the current rendering foundation

The roadmap is in [`ROADMAP.md`](ROADMAP.md). What is demonstrably true **today** lives in [`docs/capabilities.md`](docs/capabilities.md); that file is updated with the implementation it describes rather than being treated as an aspirational checklist.

## Try the editor

The workspace currently requires Rust 1.95.

```bash
cargo run --package sindri-editor
```

A scene path can also be supplied to the editor. The repository includes fixtures and rendering examples used to prove the engine and editor paths rather than separate mock implementations.

## Development

Engine/editor workspace:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
```

Decay is intentionally a separate Cargo workspace and has its own CI checks.

```bash
cd decay
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The root workspace explicitly excludes `decay/` so path dependencies cannot silently absorb the language crates into the engine workspace and give them the engine's version and lint configuration.

## Documentation

The detailed contracts live alongside the code:

- [`docs/capabilities.md`](docs/capabilities.md) — what demonstrably works today
- [`docs/scripting.md`](docs/scripting.md) — the exact Decay-to-Sindri host surface
- [`decay/LANGUAGE.md`](decay/LANGUAGE.md) — Decay language reference
- [`docs/decay-direction.md`](docs/decay-direction.md) — why Decay exists and how the scripting direction evolved
- [`docs/2d-model.md`](docs/2d-model.md) — how 2D fits the shared transform/world model
- [`docs/component-schema-registry.md`](docs/component-schema-registry.md) — component validation and authoring metadata
- [`docs/scene-serialization.md`](docs/scene-serialization.md) — scene format and canonical round-tripping
- [`docs/scene-extraction.md`](docs/scene-extraction.md) — world-to-frame extraction
- [`docs/asset-foundation.md`](docs/asset-foundation.md) — portable asset model
- [`docs/rendering-transparency.md`](docs/rendering-transparency.md) — transparent rendering
- [`docs/rendering-color.md`](docs/rendering-color.md) — colour handling
- [`docs/rendering-surface.md`](docs/rendering-surface.md) — presentation surfaces
- [`docs/platform-host.md`](docs/platform-host.md) — native/browser host model
- [`docs/entity-scaling.md`](docs/entity-scaling.md) — measured entity-storage scaling
- [`docs/versioning.md`](docs/versioning.md) — current versioning rules
- [`PROJECT_OVERVIEW.md`](PROJECT_OVERVIEW.md) — longer-term project direction
- [`ROADMAP.md`](ROADMAP.md) — checkable development roadmap

## Why Sindri?

The original Sindri Engine proved a lot of useful ideas, but Sindri Next is using those lessons to build a more coherent foundation rather than carrying every old architectural decision forward.

The project has a simple rule: **working code beats plausible architecture**. Features are exercised through real scenes, editor workflows, browser/native paths, deterministic captures, tests, and small gameplay examples before they are described as capabilities.

That makes progress slower than ticking boxes on a roadmap. It also makes the boxes mean something.

## Contributing

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) for checks, conventions, and commit style, and [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) for participation guidelines. The dependency policy is documented in [`docs/dependency-policy.md`](docs/dependency-policy.md).

## License

Dual-licensed under either the [Apache License 2.0](LICENSE-APACHE) or the [MIT license](LICENSE-MIT), at your option.

Unless you state otherwise, any contribution intentionally submitted for inclusion in Sindri is dual-licensed as above, with no additional terms or conditions.
