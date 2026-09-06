# CLI conventions

> **Status:** accepted convention, ahead of the CLI it governs. The `sindri`
> binary in `ROADMAP.md` does not exist yet; the first generated capability
> documents do. This page is written first on purpose — a command vocabulary
> discovered one command at a time accretes, and the second command is already
> too late to agree on the grammar.

`docs/decay-direction.md` says Sindri has two authoring clients: the editor, and
a CLI over the same project model, for text-editor workflows, automation, CI —
and, increasingly, for coding agents. This page says what the second one looks
like, so it is one tool rather than a pile of subcommands that grew apart.

## The grammar

```text
sindri <noun> <verb> [target] [--flags]
```

A noun is a thing Sindri has: `project`, `scene`, `entity`, `component`,
`asset`, `prefab`, `play`, `render`. A verb is what is being done to it. Nouns
stay singular. A verb that reads a thing is `inspect`; one that reads a list is
`list`; one that checks a thing without changing it is `validate`.

Commands do not gain a second grammar for convenience. `sindri inspect player`
would be shorter than `sindri entity inspect player` and it would be the first
of the exceptions that make a tool unlearnable.

## JSON is the interface; human output is a renderer

Every command takes `--format`, with `human` the default at a terminal and
`json` the contract:

```bash
sindri scene validate assets/main.scene.json            # for a person
sindri scene validate assets/main.scene.json --format json
```

Write the JSON first and render the human form from it, never the reverse. A
tool whose machine output is a reformatted paragraph makes every consumer a
parser of prose, and the prose is what changes.

Every JSON document carries `schema_version`, so a reader written against an
older layout can say so rather than silently misread it.

## Diagnostics are structured where they are raised

A failure answers with a stable `code`, the thing it is about, and a message:

```json
{
  "code": "SCENE_DUPLICATE_ENTITY_ID",
  "message": "Entity ID 'player' is already in use.",
  "entity": "player",
  "path": "assets/main.scene.json"
}
```

The code is produced by the crate that found the problem — `sindri-core`,
`sindri-scene`, `sindri-decay` — not assembled by the CLI from a string it
matched. A CLI that parses its own engine's prose is a second, worse copy of
that engine's error model, and the editor gets nothing out of it.

## Mutation is one invocation, one transaction

There is no `sindri transaction begin`. A CLI process that holds an open
transaction has to keep it somewhere between invocations, and that somewhere is
a new kind of project state nothing else understands.

Instead an edit is a batch, applied through the existing `WorldCommand` and
`Transaction` machinery in `sindri-core`, and it either all happens or none of
it does:

```bash
sindri scene edit assets/main.scene.json --ops changes.json
```

```json
{
  "operations": [
    { "op": "spawn", "id": "enemy" },
    { "op": "set_transform", "entity": "enemy", "position": [5, 2, 0] },
    { "op": "add_component", "entity": "enemy", "component": "sindri.sprite" }
  ]
}
```

### Operation names are a contract of their own

Each `op` maps one-to-one onto a `WorldCommand` variant, and to nothing else:
the CLI must not acquire an authoring behaviour the editor does not have.

But the names are written in `snake_case` and translated in one place, rather
than being the Rust variant names spelled out. `WorldCommand` derives no
`Serialize` today — it is internal, and renaming a variant is an ordinary
refactor. Emitting `{"op":"SetTransform3D"}` would quietly make Rust enum
naming an external interface, and the next rename a breaking change to
somebody's script.

The mapping is written as an exhaustive `match` over `WorldCommand`, so a new
variant fails to compile until someone decides what it is called from outside.

## No semantic constructors

There is no `sindri create sprite`. What a fresh component consists of is
already recorded once, in the component registry's default payload, and a
convenience command that assembles a "sprite entity" from its own idea of the
defaults is a second answer to a question that has one. Conflating a
component's *fields* with a fresh component's *payload* is exactly what made
`sindri.ui.text` inspect as two rows when it has seven — see
`docs/component-schema-registry.md`.

So the CLI composes primitives — create an entity, add a component, set a field
— and takes the blank from the registry. A component with no honest blank is
reported as not addable, with what it is missing, rather than added and then
refused.

## Discovery comes from generated documents

What this build of the engine offers is written down by
`cargo run -p sindri-capabilities -- --write` into `docs/generated/`, and a test
fails when those files disagree with the code. See
[`docs/generated/README.md`](generated/README.md).

`sindri capabilities --format json`, when it exists, serves that same model. It
does not gain a second description of the engine.
