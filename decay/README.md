# Decay

Decay is an experimental gameplay language for Sindri Next.

This directory is deliberately isolated from the engine workspace while the language model is being proven. Nothing under `decay/` may depend on a `sindri-*` crate during this phase, and no engine crate should need to change for Decay language work.

## Current foundation

Decay now has three independent compiler layers:

- `decay-syntax` — source spans, tokens, lexer, parser, AST, and syntax diagnostics;
- `decay-semantic` — scopes, symbols, the initial type model, mutability rules, function checks, and semantic diagnostics;
- `decay-ir` — portable symbolic IR with constants, paths, calls, operators, declarations, returns, and patched control-flow jumps.

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

`decay-semantic` deliberately receives host globals through an `Environment`; names such as `Input` are not Decay builtins. `decay-ir` preserves that boundary by lowering member chains to symbolic paths such as `this.transform.position.x` rather than importing engine types.

Run the Decay workspace independently:

```bash
cd decay
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Boundary

The language core should remain independent from Sindri:

```text
decay-syntax      lexer, parser, AST, diagnostics
      |
decay-semantic    names, types, validation, host environment contract
      |
decay-ir          portable symbolic execution representation
      |
decay-runtime     future interpreter / VM
      |
      +-------------------- no Sindri dependency

sindri-decay      future engine bindings
```

This makes the language replaceable, testable, and extractable. It also prevents engine internals from becoming accidental language semantics.

## IR direction

The initial IR is intentionally symbolic rather than tied to a particular VM layout. For example:

```text
this.transform.position.x += speed * dt
```

lowers conceptually to:

```text
LOAD this.transform.position.x
LOAD speed
LOAD dt
BINARY multiply
BINARY add
STORE this.transform.position.x
POP
```

Calls use symbolic paths as well:

```text
Input.axis("left", "right")
```

becomes a call to the path `Input.axis` with two arguments. A host must declare `Input` before semantic analysis accepts that source.

This representation gives the runtime a small contract without making the compiler know what an entity, transform, camera, or input service is.

## Near-term plan

1. Validate the IR against more control-flow and expression cases.
2. Define runtime value semantics and stack behavior explicitly.
3. Add the first portable interpreter for the IR.
4. Prove deterministic execution of ordinary Decay functions without a game engine.
5. Add a host-call interface to the runtime without adding Sindri dependencies.
6. Only after that create `sindri-decay` and expose engine concepts through the host boundary.

The syntax, type model, and IR are not stable. The point of this phase is to discover what deserves to become stable before user scripts make every early guess expensive.
