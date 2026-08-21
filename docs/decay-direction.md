# Decay and the editor-first authoring direction

> **Status:** accepted direction, with the first playable implementation in the
> repository. Decay now parses, type-checks, executes against Sindri's typed
> host, drives the companion game natively and in a browser, and exposes authored
> properties to the editor. The language and tooling remain pre-alpha. The
> superseded Rhai recommendation is retained later in this document as a dated
> decision record.

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

A current script looks like:

```rust
script Player {
    @export
    var speed: f32 = 6.0;

    fn update(dt: f32) {
        let movement = Input.axis("ArrowLeft", "ArrowRight");
        this.transform.position.x += movement * speed * dt;
    }
}
```

The complete implemented grammar and type rules live in
[`decay/LANGUAGE.md`](../decay/LANGUAGE.md).

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

---

# Evidence and recommendation

> **Status: superseded.** This section recorded engineering input from a spike run on 2026-08-20, and recommended deferring Decay in favour of Rhai. **That recommendation was not taken. Decay was built, and is now the decided direction; Rhai is not adopted.** The recommendation is kept below rather than deleted, because the reasoning it rests on is still the material for judging Decay later, and rewriting it to agree with what happened would destroy the only record of what was predicted. Read it as the case that was argued, not as current advice. What follows the recommendation — why Lua was disqualified, what the spike measured, and the conditions for revisiting — is unaffected and still holds. The reconciliation is at the end of this document.

## The recommendation

**Do not build Decay yet. Build the scripting host, and put an existing embeddable language behind it. Rhai is the one to use.**

Decay stays on the table as a name and as a future compiler. What should not happen now is writing a lexer, parser, type checker, IR, VM, memory model, LSP, and debugger for an engine that cannot yet support a game worth scripting.

## Why not now

Three reasons, in order of weight.

**There is nothing to script.** One mesh primitive, no text, no audio, no collision, no tilemaps. The first Decay program could move a transform and little else. A language designed against imagined requirements is the most expensive kind of mistake to undo, because user scripts are user data.

**The cost is not the compiler.** A parser and a tree-walking evaluator are weeks. Error messages that name the real problem, an LSP with completion over engine types, a debugger, a formatter, a stdlib, and documentation that stays true are a permanent commitment. GDScript is a decade of full-time work; Unity shipped two scripting languages and buried both.

**This codebase has a specific failure mode a language would amplify.** The asset queue went a release with no caller. The editor could not select an entity for a fortnight. Nineteen controls were drawn and inert. A compiler is that risk at maximum: months of work with nothing playable at the end.

## Why Rhai

The shortlist for a Rust engine that must run on `wasm32-unknown-unknown` is Rhai, Rune, and Koto.

**Lua is disqualified on a technicality that matters.** Lua is C, and `wasm32-unknown-unknown` has no libc. Reaching the browser means Emscripten or an immature pure-Rust reimplementation. Every part of this project — CI, `.cargo/config.toml`, `wasm-pack`, the decode compatibility tests — targets `wasm32-unknown-unknown`. Adopting the industry-standard game scripting language would mean abandoning the browser toolchain.

Rhai over the other two: it is the most maintained, it is built for embedding in a Rust host with a real story for exposing engine types, its syntax is already Rust-flavoured, and it ships the sandboxing a host running user scripts needs.

## What the spike measured

A throwaway crate, `rhai 1.25.1` on Rust 1.95.0, one test body run natively and in Node under `wasm32-unknown-unknown` — the same dual-attribute arrangement `decode_compatibility.rs` uses. Four tests: a script instance driving a component across three frames, a script writing to shared engine state, a runaway loop, and a syntax error.

**It runs on the browser target, with one caveat.** Rhai reaches `getrandom` through `ahash` and `const-random`, which refuses to compile for `wasm32-unknown-unknown` without an opt-in. Rhai's own `wasm-bindgen` feature resolves it, declared per target:

```toml
[dependencies]
rhai = { version = "1", features = ["f32_float"] }

[target.'cfg(target_arch = "wasm32")'.dependencies]
rhai = { version = "1", features = ["f32_float", "wasm-bindgen"] }
```

With that, all four tests pass in Node. This is a contained cost, not a standing tax, and the note already accepts that a browser build may carry JavaScript glue.

**Rhai's float is `f64` unless told otherwise.** The engine is `f32` throughout — `glam`, `Transform3D`, every component. Without the `f32_float` feature the mismatch surfaces inside the script as `function not found: * (f32, f64)`, which is a genuinely bad error for an author to hit. The feature fixes it; the lesson is that the numeric type is a decision to make deliberately rather than discover.

**Reserved words constrain the engine API.** `spawn` and `go` are both reserved and cannot be registered as function names. The API surface has to be checked against Rhai's reserved list rather than chosen freely.

**Write-back works, which was the open architectural question.** `this.transform.x += movement * this.speed * dt` persists through a custom type nested in a script instance's state map, for both compound and plain assignment. Per-instance state as a `this` map, with lifecycle functions called per frame, is a workable shape.

**Sandboxing works.** `set_max_operations` cut an infinite loop rather than hanging the frame. That is expensive to build and comes free.

**Parse errors carry line and column.**

To reproduce: the crate above, plus `wasm-bindgen-cli` at the version the workspace resolves and a `wasm32-unknown-unknown` runner in `.cargo/config.toml`, then `cargo test --target wasm32-unknown-unknown`. It is deliberately not in the tree or in CI, because the dependency has not been adopted.

**What the spike did not measure:** performance under a real entity count, hot reload of a script that already has live state, editor property introspection, or the debugging and editor-integration story. Those are the questions the host milestone should answer.

## What keeps the decision reversible

The language is replaceable; the host interface is not. Scripts should see only entities, components, handles, events, and input, through a narrow typed surface — the discipline `TextureBindings` and the command layer already follow. If a script can reach the engine only through that seam, swapping the language later costs a syntax migration rather than an architecture one. If it can reach anything else, no choice of language saves it.

A naming option worth considering: let **Decay** name the layer rather than the implementation — the `.decay` extension, the prelude, the engine API, the lifecycle callbacks, the editor integration — with Rhai evaluating it today. Say so plainly in the documentation. Do not build a front end to make Rhai resemble the sample syntax above: the moment a transform sits in between, every error message names a line the author did not write, and a compiler is being written anyway with none of the benefits.

## When to revisit

Write Decay when at least three of these hold:

1. A shipped game exists and profiling shows script execution is a real frame-time cost, measured rather than predicted.
2. Authors are hitting the dynamic-typing wall in ways the editor's property panel cannot cover.
3. A compiler, an LSP, and a debugger can be owned indefinitely, not as a project.
4. The engine is complete enough that language design has real requirements to serve.

## Proposed sequencing

The largest risk in the direction above is not the language. It is that the product model is *editor + CLI + export*, and there is no CLI and no export — the workflow today is `cargo run` and `wasm-pack`.

1. **CLI and export.** `sindri build --target web` producing a directory and the asset manifest that already exists. A fraction of a compiler's cost, and it validates "the same project model, editor optional" immediately.
2. **Milestone 6, with the companion game.** So that scripting has something to script, and so the game generates the language's requirements rather than the other way round.
3. **The scripting host**, with Rhai behind it.
4. **Judge Decay on evidence** against the four conditions above.

Milestone 5 as written — a first-class TypeScript SDK as the browser authoring story — should be reconsidered under this direction, as the note above says. That is a roadmap change and is not made here.

---

# Reconciliation, after Decay was built

Written 2026-08-20, after reading and running the foundation that landed in `decay/`.

## What the recommendation got wrong

**The static-typing argument was missed entirely.** The case above weighed effort, maintenance, and timing, and on all three Rhai still wins. What it never weighed is the one thing Rhai structurally cannot do: Rhai is dynamically typed, and this project's stated thesis is *editor-first authoring*. An editor property panel needs a typed, named, declared field it can draw a widget for without executing anything. Decay's `IrField { exported, type_name }` is exactly that, and it exists today. With Rhai the same capability is a convention layer and a lot of hoping.

That is the argument that justifies owning a compiler, and it is a product argument rather than a performance one. Performance was never the case: Rhai is also a tree-walking interpreter, so Decay's runtime is not buying speed over it.

**The effort estimate was right, and is not the point.** What exists is a lexer, parser, checker, IR and interpreter — the enjoyable fifth. Loops, collections, a stdlib, strings, closures, execution budgets, hot reload with live state, an LSP, a formatter, a debugger, and documentation that stays true are the other four fifths and the permanent commitment. Nothing observed changes that estimate. It is a price, not a refutation.

## What the recommendation got right, demonstrated

The third reason given for deferring was that *this codebase has a specific failure mode a language would amplify*: the asset queue went a release with no caller, the editor could not select an entity for a fortnight, nineteen controls were drawn and inert.

Decay reproduced it precisely. Three thousand lines, twenty-one passing tests, its own CI — and no engine caller. The first thing to actually run a script found three faults reachable from the first line anyone would write:

- every `let` local failed at runtime, because a binding's own initialization was refused for not being mutable;
- a shadowing declaration replaced the name it shadowed for the rest of the function, type-checking cleanly and returning the wrong number;
- unbounded recursion overflowed the host stack and aborted the process, which for a runtime meant to run inside the editor takes the editor with it.

The README's own headline example did not execute. All three are fixed and tested now. The lesson is not that the code was poor — the layering, the host boundary, and the parser's error recovery are all better than this stage usually gets. The lesson is that depth without a caller hides faults that one caller finds immediately.

## The decision

**Decay is the scripting language for Sindri Next. Rhai is not adopted, and the question is closed.** Decided 2026-08-20, after the foundation was built and reviewed.

This settles a real cost rather than a preference: a scripting host maintained against two runtimes is worse than either, and the typed-authoring argument that justifies Decay only pays off if Decay is the actual path rather than a hedge. Nothing in the tree ever depended on Rhai — the spike was deliberately never committed — so there is nothing to remove. What ends is treating it as a live option.

The rest of this document keeps the case for Rhai intact, because a decision recorded without the argument it beat is not a decision anyone can revisit competently. In particular, **why Lua was disqualified still governs**: vendored C Lua cannot target `wasm32-unknown-unknown`, browser reach is non-negotiable, and Decay is now checked against that target in its own CI for exactly that reason.

**Do not deepen the language next.** Build `sindri-decay`: one script, one transform, in the editor. Plan item 6 of `decay/README.md` became item 1 for this reason. Whatever survives contact with the engine is the language that was actually needed; everything added before that contact is a guess, and user scripts turn guesses into data that has to be migrated.

**Keep the isolation absolute — going all in on Decay does not mean fusing it to the engine.** Nothing under `decay/` may depend on a `sindri-*` crate. The moment engine internals become language semantics, the decision stops being reversible, and the whole reason the recommendation above could be overruled cheaply is that it is still reversible today.

## What the four conditions become

The "When to revisit" list above was written as a gate on *starting*. That gate is behind us, so the same four conditions are better read as the things that decide whether to *continue*, judged when `sindri-decay` exists and the companion game has been scripted with it:

1. Does the typed authoring surface actually make the editor better than a dynamic language would, in a panel someone used?
2. Are authors served by the type model, or fighting it because it is too thin — member types are still `Unknown`?
3. Can the compiler, LSP, and debugger be owned indefinitely, now that the size of the remaining work is visible rather than estimated?
4. Did the engine generate the language's requirements, or did the language keep guessing at them?

Two yeses and the decision is vindicated. Two nos and the isolation is what keeps the cost of having been wrong small — which is the point of keeping it, and the reason committing to Decay is affordable at all.
