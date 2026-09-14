# Decay agent guide

Use this checklist before an AI agent edits a `.decay` script, a scripted prefab,
or the Sindri host surface. The full language and host contracts remain in
[`decay/LANGUAGE.md`](../decay/LANGUAGE.md),
[`docs/scripting.md`](scripting.md), and
[`docs/generated/decay-api.md`](generated/decay-api.md).

## Before writing code

1. Read the existing script, its scene or prefab, and the host API entry it uses.
2. Compile the changed scripts against Sindri's real typed environment:

   ```bash
   cargo run --quiet --package decay-lsp -- --check path/to/changed.decay
   ```

   Multiple files and directories are accepted. Directories are searched
   recursively; `.git`, `target`, and `node_modules` are skipped.
3. Run the gameplay regression that observes the behavior, not only the compiler.
4. Run `cargo fmt --all --check` when Rust, generated files, or workflow code
   also changed.

The preflight exits nonzero for syntax and semantic errors. Runtime-contract
reminders are warnings because some valid uses require context the checker
cannot prove.

## Authored properties are not live state

This is the most important Sindri/Decay distinction.

| API | What it means | Valid timing |
| --- | --- | --- |
| `World.set_property(entity, name, value)` | Authors an exported field on the target script component | After spawning and before that script starts |
| `World.property_number(entity, name)` | Reads the numeric value authored on the script component | Any time, but it does not read the script's current field |
| `World.send_signal(entity, name, value)` | Sends runtime data to a running script | During gameplay |
| `World.take_signal(name)` | Consumes accumulated runtime signal data on the receiving script | During gameplay |

Do not use `set_property` as a setter for a running script:

```decay
// Wrong once part's script has started: the host rejects the write.
World.set_property(part, "leader_x", x);
```

Configure a newly spawned entity before it is allowed to start:

```decay
let part = World.spawn("prefabs/body-part.prefab.json");
World.set_property(part, "group", group);
World.set_property(part, "leader", this.entity);
```

For later changes, send a signal and let the target script update its own state:

```decay
World.send_signal(part, "leader_x", x);
```

A read has the same boundary. If a script changes its own `health` field,
`World.property_number(entity, "health")` still describes authored component
data; it does not inspect that live field.

## Runtime behavior the compiler cannot prove

- Script updates and physics integration happen at defined frame boundaries.
  Check the host contract before assuming a transform or velocity write is
  visible in the same update.
- Signals accumulate until consumed. Confirm whether the receiver should sum,
  clamp, or treat the value as an event.
- A spawned script does not automatically inherit the spawner's runtime state.
  Author its exported startup fields before it starts, then communicate through
  explicit runtime APIs.
- A follower should consume the leader's sampled positions over time. Copying
  the leader's current translation every frame produces rigid simultaneous
  movement, not a trailing body.
- Splitting or re-parenting a chain must rebuild ownership and history for both
  resulting chains. Do not reuse one mutable history buffer for two leaders.
- Boundary movement needs a gameplay route, not merely a legal coordinate.
  A hazard that patrols only the screen edge may be technically active and still
  pose no threat.

These are behavior contracts, so add or update a deterministic runtime test when
they matter. Static checking cannot establish that a boss route is threatening
or that a segmented body follows the intended historical path.

## Syntax habits that avoid common failures

- Copy the shape of a compiling nearby Decay script before inventing syntax.
- Treat `decay/LANGUAGE.md` as authoritative; Decay is not Rust, JavaScript,
  or GDScript.
- Use only names and signatures listed in
  `docs/generated/decay-api.md`. Remembered APIs are not evidence.
- Keep changes small enough that the preflight diagnostic points at one idea.
- Do not repair only the first compiler message. Re-run the complete changed
  script set after every fix.

## Definition of done

A Decay change is ready to push only when:

- the batch preflight passes for every changed `.decay` file;
- runtime-contract reminders have been consciously checked;
- the relevant gameplay/runtime regression passes;
- the final diff contains the required scene, prefab, documentation, and
  generated API updates; and
- the behavior was observed on the surface it is meant to prove.
