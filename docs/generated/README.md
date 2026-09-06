# Generated capability documents

Nothing in this directory is written by hand. Every file is produced by

```bash
cargo run -p sindri-capabilities -- --write   # rewrite
cargo run -p sindri-capabilities              # check; exit 1 when stale
```

and `cargo test --workspace` fails when a file here disagrees with the code it
was generated from. Editing one by hand is therefore not a way to change what
Sindri offers; it is a way to fail CI.

| File | Answers | Read from |
| --- | --- | --- |
| `decay-api.json` | What gameplay code may I write? | `sindri_decay::environment()` |
| `decay-api.md` | The same, for a person | the same model as the JSON |
| `sindri-capabilities.json` | What may I author? | the built-in `ComponentSchemaRegistry` |

## Why generated

Both questions already had exactly one authority in the repository, and neither
was readable without opening Rust.

The Decay surface is described once in `crates/sindri-decay/src/surface/`
because the analyzer and the runtime host must not disagree — a path one accepts
and the other cannot answer is a clean compile followed by a failure on frame
one. `environment()` turns that description into the types the analyzer checks
against, and this is the same call, asked to write down what it found.

The components are registered once in `crates/sindri-scene/src/extract/`, with
the distinction that registry exists to keep: **fields** are what a component
has, a **default payload** is what a fresh one is, and only some types have one.

A hand-maintained reference for either would be wrong a release later, and
wrong in the direction that matters: it would describe calls that do not exist.

## What is deliberately not here

**Parameter names.** The host surface registers parameter *types*, because that
is what type-checking needs. Naming the arguments here would be a second source
of truth for something this tooling cannot know. What each argument means is in
[`../scripting.md`](../scripting.md).

**Prose descriptions.** Same reason. The doc comments in the surface tables are
not available at runtime, and copying them would create the drift the whole
arrangement is built to prevent. Giving the surface tables a description field
that both the generator and the analyzer read is the honest way to add them, and
is a change to the surface, not to this tool.

**Anything about the editor.** These files describe the engine, not one client
of it.
