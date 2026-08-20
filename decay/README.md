# Decay

Decay is an experimental gameplay language for Sindri Next.

This directory is deliberately isolated from the engine workspace while the language model is being proven. Nothing under `decay/` may depend on a `sindri-*` crate during this phase, and no engine crate should need to change for Decay language work.

## Current slice

Decay now has two independent frontend crates:

- `decay-syntax` — source spans, tokens, lexer, AST, parser, precedence, member/call chains, assignments, declarations, blocks, and syntax diagnostics;
- `decay-semantic` — scopes, symbols, primitive/named types, mutability checks, duplicate detection, call arity/type checks, return checks, and semantic diagnostics.

The parser accepts gameplay-shaped code such as:

```rust
script PlayerController {
    @export
    let speed: f32 = 6.0;

    fn update(dt: f32) {
        var movement: f32 = 1.0;
        movement += speed * dt;
    }
}
```

The semantic layer deliberately does not know that Sindri has input, transforms, sprites, cameras, or any other engine concept. Hosts inject external values and functions through `Environment`; a future `sindri-decay` crate can therefore provide the game API without making engine details part of the language compiler.

Run the workspace independently from Sindri:

```bash
cd decay
cargo test
```

## Boundary

The language frontend should remain independent from Sindri. Future crates should preserve this split:

```text
decay-syntax      lexer, parser, AST, diagnostics
      |
decay-semantic    names, types, validation, host environment
      |
decay-runtime     portable execution model
      |
      +-------------------- no Sindri dependency

sindri-decay      engine bindings, added only after the language core is proven
```

This makes the language replaceable, testable, and extractable. It also prevents engine internals from becoming accidental language semantics.

## Current semantic rules

The intentionally small type model currently understands `f32`, `bool`, `String`, unit, null, named host/user types, and an explicit unknown type used at unresolved host boundaries.

The analyzer currently catches:

- unknown names;
- duplicate containers, members, parameters, and locals;
- assignment to immutable bindings;
- obvious assignment/type mismatches;
- invalid arithmetic, logical, and comparison operands;
- non-boolean `if` conditions;
- return type mismatches;
- calls to known functions with the wrong argument count or argument types.

Member access remains deliberately open until a host supplies a member schema. That lets the language core stay independent while preserving the eventual shape of APIs such as `this.transform.position`.

## Near-term plan

1. Improve semantic type information for user components and member access.
2. Define a small, versionable intermediate representation rather than committing directly to a VM instruction set.
3. Lower a representative script into that IR and prove deterministic output.
4. Add a minimal interpreter/runtime for ordinary language values and control flow.
5. Prove stateful lifecycle calls without any Sindri dependency.
6. Only then design the Sindri host boundary and engine-facing type schema.

The syntax is not stable. The point of this phase is to discover what deserves to become stable before user scripts make every early guess expensive.
