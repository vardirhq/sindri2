# Decay

Decay is an experimental gameplay language for Sindri Next.

This directory is deliberately isolated from the engine workspace while the language model is being proven. Nothing under `decay/` may depend on a `sindri-*` crate during this phase, and no engine crate should need to change for Decay language work.

## Current foundation

Decay now has four independent layers:

- `decay-syntax` — source spans, tokens, lexer, parser, AST, and syntax diagnostics;
- `decay-semantic` — scopes, symbols, the initial type model, mutability rules, function checks, and semantic diagnostics;
- `decay-ir` — portable symbolic IR with constants, paths, calls, operators, declarations, returns, and patched control-flow jumps;
- `decay-runtime` — a small interpreter with runtime values, stack execution, Decay-to-Decay calls, persistent script instances, and an injected host boundary.

The frontend already understands gameplay-shaped source such as:

```rust
script PlayerController {
    @export
    let speed: f32 = 6.0;

    fn update(dt: f32) {
        this.transform.position.x += speed * dt;
    }
}
```

`decay-semantic` deliberately receives host globals through an `Environment`; names such as `Input` are not Decay builtins. `decay-ir` preserves that boundary by lowering member chains to symbolic paths such as `this.transform.position.x` rather than importing engine types. `decay-runtime` crosses into external state only through its `Host` trait.

Run the Decay workspace independently:

```bash
cd decay
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Boundary

The language core remains independent from Sindri:

```text
decay-syntax      lexer, parser, AST, diagnostics
      |
decay-semantic    names, types, validation, host environment contract
      |
decay-ir          portable symbolic execution representation
      |
decay-runtime     interpreter, instances, host-call boundary
      |
      +-------------------- no Sindri dependency

sindri-decay      future engine bindings
```

This makes the language replaceable, testable, and extractable. It also prevents engine internals from becoming accidental language semantics.

## Runtime direction

The first interpreter intentionally executes the symbolic IR directly. It is not yet a bytecode VM and does not pretend to be optimized.

It supports:

- numeric, boolean, string, null, and unit values;
- locals and mutable bindings;
- arithmetic, comparisons, boolean operators, and unary operators;
- `if`/`else` jumps and returns;
- calls between Decay functions;
- persistent per-instance script fields across multiple function calls;
- host loads, stores, and calls through a narrow `Host` trait.

A host can therefore implement a path such as `Input.axis` without the Decay runtime knowing what input means. The same mechanism can later expose `this.transform.position.x`, entities, components, assets, and events from Sindri through a dedicated `sindri-decay` crate.

Persistent instances are important for gameplay. A script like:

```rust
script Counter {
    var count: f32 = 0.0;

    fn tick() -> f32 {
        count += 1.0;
        return count;
    }
}
```

can be instantiated once and called repeatedly, with `count` surviving between calls.

## Near-term plan

1. Harden runtime errors and execution limits.
2. Add loops and the control-flow IR they require.
3. Add arrays/maps only when a representative gameplay script needs them.
4. Define typed host members so `Input.axis` and `this.transform.position` can be checked instead of remaining unknown host paths.
5. Add lifecycle-oriented runtime helpers for `start`, `update`, and `fixed_update` without coupling them to Sindri.
6. Only after that create `sindri-decay` and expose engine concepts through the host boundary.

The syntax, type model, IR, and runtime are not stable. The point of this phase is to discover what deserves to become stable before user scripts make every early guess expensive.
