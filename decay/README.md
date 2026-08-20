# Decay

Decay is an experimental gameplay language for Sindri Next.

This directory is deliberately isolated from the engine workspace while the language model is being proven. Nothing under `decay/` may depend on a `sindri-*` crate during this phase, and no engine crate should need to change for Decay syntax work.

## Current slice

The first implementation is intentionally small:

- a standalone Rust workspace;
- `decay-syntax`, containing source spans, tokens, diagnostics, and a lexer;
- Rust-like gameplay-oriented tokens such as `script`, `component`, `fn`, `let`, `var`, `@`, member access, assignments, comparisons, and basic literals;
- line/column diagnostics;
- tests built around the proposed gameplay syntax rather than generic calculator expressions.

Run it independently from the Sindri workspace:

```bash
cd decay
cargo test
```

## Boundary

The language frontend should remain independent from Sindri. Future crates should preserve this split:

```text
decay-syntax      lexer, parser, AST, diagnostics
      |
decay-semantic    names, types, validation
      |
decay-runtime     portable execution model
      |
      +-------------------- no Sindri dependency

sindri-decay      engine bindings, added only after the language core is proven
```

This makes the language replaceable, testable, and extractable. It also prevents engine internals from becoming accidental language semantics.

## Near-term plan

1. Parse scripts, components, fields, functions, statements, and expressions into an AST.
2. Add error recovery so one syntax mistake does not hide the rest of the file.
3. Define the deliberately small Decay type model.
4. Type-check a representative gameplay script.
5. Choose and prove a portable execution representation.
6. Only then design the Sindri host boundary.

The syntax is not stable. The point of this phase is to discover what deserves to become stable before user scripts make every early guess expensive.
