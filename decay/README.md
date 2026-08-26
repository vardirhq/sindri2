# Decay

Decay is an experimental gameplay language for Sindri Next.

**[`LANGUAGE.md`](LANGUAGE.md) is the language reference** — the full grammar,
every operator, and, more usefully at this stage, explicit lists of what does
*not* exist and which behaviours will surprise you. Read it before writing a
script. Its claims are enforced by `crates/decay-ir/tests/language_reference.rs`,
so a document that goes stale fails the build rather than quietly misleading
whoever reads it next.

`docs/scripting.md` in the engine repository covers the other half: how a script
reaches a world, what paths it can touch, and how the editor runs it.

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
- `let` and `var` bindings, scoped to the block that declares them;
- arithmetic including `%`, comparisons, short-circuiting boolean operators,
  and unary operators;
- `if`/`else`, `while` with `break` and `continue`, and returns;
- calls between Decay functions, bounded by a call-depth limit;
- persistent per-instance script fields across multiple function calls;
- host loads, stores, and calls through a narrow `Host` trait.

Two rules are worth stating because they are structural rather than incidental.

**A binding is not an assignment.** `Instruction::Declare` pops the initial
value and binds it; `Instruction::Store` assigns to a name that already exists
and is subject to its mutability. Collapsing the two is what made every `let`
local fail at runtime for the whole of the foundation branch — the binding's own
initialization was refused for not being mutable, including in the example
below. Keeping them separate means there is no initializing store to make an
exception for.

**A block is a scope in the IR, not only in the analyzer.** `ScopeEnter` and
`ScopeExit` bracket every branch and bare block. Without them the runtime had
one flat map per frame, so a shadowing declaration replaced the name it shadowed
for the rest of the function — type-checking cleanly and then returning the
wrong number.

**A runaway script is stopped rather than fatal.** Decay calls may nest
`DEFAULT_CALL_DEPTH_LIMIT` deep, after which the runtime returns
`RuntimeError::CallDepthExceeded`. Unbounded recursion previously overflowed the
host's own stack and aborted the process, which for a runtime meant to execute
author scripts inside the editor takes the editor and any unsaved work with it.
The limit is per `Runtime` and adjustable with `with_call_depth_limit`.

It bounds recursion only, which was enough until `while` arrived. A loop needs
no stack to run forever, so the same rule needed a second half: one outermost
call may execute `DEFAULT_OPERATION_BUDGET` instructions, after which the
runtime returns `RuntimeError::OperationBudgetExceeded`. It is counted per
outermost call rather than per frame, so a script cannot buy itself more by
recursing, and it is adjustable with `with_operation_budget`. The budget is not
a safeguard added beside loops; it is the thing that made offering them
acceptable.

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

## The example is a test

`examples/player.decay` is the script this README leads with, and
`crates/decay-runtime/tests/example_script.rs` analyzes, lowers, and runs it
against a recording host, asserting both the number it computes and the calls
that leave through the host boundary.

It exists because that example did not run. Every `let` local in it failed, and
nothing noticed for the length of the foundation branch: no unit test bound a
`let` local, and no test executed the example. A documented example that does
not execute is worse than none, because it is believed. Anything the
documentation claims Decay can do belongs in that test.

## Near-term plan

1. ~~**Bind Decay to the engine.**~~ Done: `crates/sindri-decay` drives a script
   on an entity's transform, and the editor's fixture spins its cube because a
   `.decay` file says so. See `docs/scripting.md`.
2. ~~**Define typed host members.**~~ Done: `HostType` and `Environment` check
   `Input.axis` and `this.transform.position`, so a misspelled component field
   is a compile error with a line number rather than a runtime `UnknownPath`,
   and completion after a `.` becomes possible. See `docs/scripting.md`.
3. ~~**Loops, and everything sharing their machinery.**~~ Done: `while`,
   `break`, `continue`, the operation budget that bounds them, short-circuiting
   `&&` and `||`, `else if`, `%`, and a compile error for a field initializer
   that reads a field declared below it.
4. **Everything after that is ordered in the engine repository's `ROADMAP.md`**,
   under *The language basics, ordered by what a script cannot say*: the numeric
   spelling this file ends by complaining about, then one collection, then a
   value for a position. `fixed_update` is there too, as a host-side item rather
   than a language one.

   That plan lives there rather than here because that is where checkboxes are
   audited, and a second plan in a second file is a second thing to go stale.
   This section says what has happened; that one says what is next.

Binding first was the right call, and it is the one thing this phase has already
learned. The foundation reached three thousand lines with no engine caller, and
what that cost was not visible until something ran the example: the `let` bug,
the scoping bug, and the recursion abort were all reachable from the first
script anyone would write. `docs/capabilities.md` in the engine repository
exists for the same reason.

The binding then found the next set immediately, which is the argument working
twice: `this.helper()` silently becomes a host call rather than calling a
sibling function, and a field initializer that reads a field declared below it
compiles and fails at runtime. Both are in `LANGUAGE.md` under
[Surprising behaviour](LANGUAGE.md#surprising-behaviour), and both want fixing
rather than documenting forever.

The syntax, type model, IR, and runtime are not stable. The point of this phase
is to discover what deserves to become stable before user scripts make every
early guess expensive. One decision worth making early rather than discovering:
the only numeric type is spelled `f32`, but every value it holds is an `f64`.
