# Decay and the editor-first authoring direction

> **Status:** design note / proposed direction. This records the current product discussion; it does not mean the scripting runtime has been implemented or that the existing roadmap has already been replaced.

Sindri Next's existing engine foundation remains a good fit for a simpler authoring model than the original Rust + TypeScript split.

The proposed direction is:

- **Sindri Editor + Decay** is the primary way to build games.
- **Sindri CLI + Decay** provides the same project model without requiring the editor, for text-editor workflows, automation, tests, and CI.
- **Rust is the engine implementation language**, not the normal user-facing gameplay language.
- Rust/native extension points may exist for advanced engine and plugin development, but ordinary game developers should not need a Rust toolchain or Rust knowledge.
- Native and web are **build/export targets**, rather than separate authoring-language experiences.

## Decay

**Decay** is the working name for Sindri's lightweight, Rust-inspired gameplay language. Source files use the `.decay` extension.

Decay should borrow useful ideas and familiar syntax from Rust without presenting Rust's systems-programming complexity to game authors. In particular, normal gameplay should not require developers to reason about lifetimes, borrowing, `Arc<Mutex<_>>`, unsafe code, or other implementation-level concerns just to manipulate a scene.

The language should be designed around Sindri concepts from the start: entities, components, scenes, assets, prefabs, input, events, editor-exposed properties, lifecycle callbacks, and hot reload.

A representative direction might look like:

```rust
script Player {
    @export
    speed: f32 = 6.0

    fn update(dt: f32) {
        let movement = Input.axis("move_left", "move_right")
        transform.position.x += movement * speed * dt
    }
}
```

This syntax is illustrative, not a committed language specification.

## Product model

The intended layering is:

```text
                    Sindri Editor
                         |
                       Decay
                         |
                         v
                    Sindri Engine
                         |
                        Rust
                  implementation detail

                       Export
             +-----------+-----------+
             |           |           |
             v           v           v
          Windows      macOS        Web
```

The editor should be the best and most complete frontend for Sindri, but it should not own a private representation of a game. Editor and CLI workflows should operate on the same project, scene, asset, and script files.

A developer should therefore be able to author a project entirely outside the editor when desired and use commands along the lines of:

```text
sindri run
sindri build
sindri build --target web
sindri test
```

Exact CLI commands are not specified here.

## Fit with Sindri Next today

This direction does **not** imply restarting the engine.

The existing foundation already separates renderer-independent core, scenes, assets, platform integration, GPU/rendering, desktop hosting, and the editor into focused crates. The scene model uses stable authored IDs distinct from runtime handles, canonical/versioned serialization, component schemas, and forward-compatible component payloads. The editor already operates on the real runtime scene representation and uses the command/undo model rather than maintaining a separate editor-only world.

Those properties are useful regardless of scripting language and should carry forward.

Likewise, the existing native/WebGPU work remains the basis for exported games. Decay should sit above the world/runtime model rather than change the renderer, GPU, platform, or asset foundations.

A likely high-level runtime shape is:

```text
Decay source
    |
    v
Decay compiler
    |
    v
Decay bytecode / IR
    |
    v
Decay VM (Rust)
    |
    v
Sindri world / engine
    |
    v
renderer + platform
    |
    +-- native
    +-- WebAssembly / WebGPU
```

A Rust implementation of the Decay VM would allow the same gameplay execution model to travel with Sindri to native and web targets. A browser build may still require JavaScript bootstrap/glue for WebAssembly, but JavaScript would not need to be the game's authoring language.

## Roadmap implication

The largest conceptual change is the planned first-class TypeScript/Web SDK milestone.

If this direction is adopted, that milestone should be reconsidered in favor of a Decay compiler/runtime/tooling milestone. The browser remains a first-class target, but the product goal changes from:

> browser games are authored through a public TypeScript SDK

into:

> Sindri games are authored in Decay and exported to the browser using the same engine/runtime model as native builds.

A future TypeScript API could still be added for embedding Sindri into web applications if there is a concrete product need. It would no longer need to define the primary browser game-authoring experience.

A first Decay milestone would likely prove only the narrow vertical slice needed to validate the architecture:

- parse and type-check a small `.decay` program;
- compile it to a portable executable representation;
- instantiate a script on an entity;
- call lifecycle functions such as `start`, `fixed_update`, and `update`;
- read/write a deliberately small set of engine values through safe handles;
- expose typed script properties to the editor;
- report useful compile/runtime diagnostics;
- hot-reload a changed script while developing;
- run the same small scripted scene natively and in a browser.

Only after that proof should the language grow features such as richer pattern matching, async/coroutines, user-defined components, broader collections/generics, or advanced tooling.

## Design constraint

Decay should not be marketed or designed as "simplified Rust" or "RustScript". It is **Rust-inspired**, which leaves it free to retain the ideas that suit game development while choosing simpler semantics where a scripting language benefits from them.

The product hierarchy should remain clear:

```text
Typical game developer
        |
   Editor + Decay

CLI-oriented game developer
        |
    CLI + Decay

Advanced engine/plugin developer
        |
       Rust
```

If ordinary gameplay routinely requires dropping into Rust, the Decay/editor abstraction is not doing enough.
