# AGENTS.md

Repository guidance for AI coding agents working on Sindri Engine.

Read this file before making changes. Repository documents and the current code
are authoritative over remembered context from earlier sessions.

**If you cannot run commands in this checkout** (for example, you edit through a
GitHub connector and CI is the first place anything compiles), read
[Working without local execution](#working-without-local-execution) before
writing code. It lists the failures that have actually cost this repository the
most round trips, and what CI already fixes for you.

## Product direction

Sindri Engine is a pre-alpha Rust game engine targeting native desktop and WebGPU
browsers, with a native editor, the Decay gameplay language, and the games and
examples that prove it. The engine is developed vertically: runtime capability,
authoring, scripting, and a real game should evolve together where the feature
applies.

Projects that prove the engine come in three kinds, and each has a stated role.
More can be added; a new one says which kind it is and what it is for.

- **Flagship games** are deep, and each has a purpose of its own, described
  below: Causeway is the showcase, Orbital Last Stand the forcing function.
- **Genre showcases** (`games/<genre>`) are small, complete games, one per
  genre: the platformer first, then top-down, shooter, puzzle and others. Each
  proves Sindri can make that kind of game, finds the gaps the genre hits, and
  is what a new user copies to start their own. Keep one small: a level and a
  few scripts, not a campaign.
- **Feature examples** (`examples/<feature>`) show one bigger feature on its
  own, such as the camera, audio, Weave, Decay, tilemaps, isometric or voxels,
  small enough to read in a few minutes.

Every project, of every kind, carries a test that opens it, compiles its
scripts and plays a short scripted run to its goal, so none can rot unnoticed.

**Causeway is the showcase.** It demonstrates capabilities the engine already
has, in a real gameplay context. A unit test, component type, editor control,
or callable Decay API is necessary evidence but is not a substitute for a
feature being used in a game — and Causeway is where a finished capability
proves it can be used. It is not a disposable demo.

Causeway is a builder: the world is made of voxels, you place and remove blocks
by clicking their faces, and somebody walks the way you build. It is held to
the standard of a game somebody would choose to play rather than of a demo that
proves a point, because a showcase nobody wants to play does not showcase
anything.

It replaced Causeway, an isometric farming game, when the engine stopped drawing
a picture of blocks and started drawing blocks. Causeway was authored as a
projection all the way down — its art baked as isometric silhouettes, its
scene written in screen coordinates, its scripts assuming a flat plane — and
migrating it would have carried a design shaped around a limitation the engine
no longer has. The history holds it if it is ever wanted.

Being the showcase does not mean it may only use what already exists. Where the
game needs a capability Sindri lacks, that capability is added, as a general
one, on the same terms as any other. What separates it from the forcing
function below is *why* the gap is found: Causeway finds gaps by trying to be a
good game, Orbital by trying to be a faithful recreation. Causeway also carries
the art: it is the one place the isometric baker is pushed as far as it goes,
because a showcase that looks approximate sells an engine that looks
approximate.

**Orbital Last Stand is the forcing function.** It is a recreation of a real,
complete game built only through the editor and Decay, and its job is to find
what the engine, editor, and language cannot do yet. Every gap it hits is closed
as a *general* Sindri capability, never as something shaped around that game.
A new gameplay capability is proven there first. See
`docs/orbital-last-stand-plan.md`.

**The platformer is the first genre showcase.** A side-view game with a
painted, solid level, a hero who runs and jumps, coins and a flag, in
`games/platformer`. It is where the plain 2D path is proven: tilemap
collision, scene gravity, collider authoring, camera follow and the 2D Scene
view, and it finds gaps by being a platformer rather than a recreation.

**Scorchball is the local-multiplayer genre showcase.** A top-down couch
football game for two to four pads, in `games/scorchball`. It is where playing
together on one screen is proven: pads read by player slot, joining and
leaving mid-game, and players made from prefabs, and it finds gaps by being a
game several people play at once.

So: a capability the engine already had is not complete until a game uses it;
a capability found by recreating a known game is proven in Orbital Last Stand;
a capability found by making Causeway or a genre showcase good is added for
it, generally rather than shaped around it. Say which of these a change is, in
its documentation.

## Read before changing architecture

These documents govern the work:

- `README.md` — current product identity and high-level capability statement.
- `ROADMAP.md` — engineering plan ordered by dependency. Check an item only when
  its acceptance criteria and relevant tests are complete.
- `docs/FEASIBILITY.md` — non-negotiable architectural decisions and risks.
- `docs/dependency-policy.md` — dependency, licence, source, and MSRV policy.
- `docs/decay-direction.md` — accepted Editor + Decay authoring direction.
- `docs/cli-conventions.md` — the grammar and contracts the `sindri` CLI and
  its generated capability documents follow.
- `docs/project-format.md` — what a project is, and what `sindri.toml` holds.
- `docs/scripting.md` and `decay/LANGUAGE.md` — scripting contracts.
- `docs/decay-agent-guide.md` — mandatory preflight and runtime-contract checklist
  before changing Decay scripts or their host APIs.
- `docs/decay-lsp-modernization.md` — mandatory checklist for Decay language-server,
  editor tooling, batch preflight, structured diagnostics, and agent-tooling work.
- `docs/capabilities.md` — detailed evidence for what actually works.
- `docs/parity.md` — what an engine is expected to do, what Sindri does, and the
  distance between them; carries the Engine / Editor / Decay / proof status that
  the function and feature-integration matrices used to hold separately.
- `docs/module-layout.md` — how a source file is sized and split.

Subsystem contracts live in `docs/`. If a subsystem's behaviour changes, update
its contract in the same change.

## Dependency boundaries

Do not casually change the crate graph. The intended in-workspace direction is:

```text
sindri-core       -> (nothing in-workspace)
sindri-grid       -> (nothing in-workspace)
sindri-voxel      -> (nothing in-workspace initially)
sindri-export     -> sindri-assets + sindri-core + sindri-decay + sindri-scene
sindri-platform   -> sindri-core
sindri-desktop    -> sindri-platform + sindri-gpu
sindri-assets     -> sindri-core
sindri-gpu        -> wgpu only (render is dev-only)
sindri-render     -> wgpu + glam + bytemuck only
sindri            -> assets + core + grid + optional gpu/render/scene
sindri-physics    -> sindri-core (+ sindri-grid only when a real integration needs it)
sindri-scene      -> sindri-core + sindri-grid + sindri-render + sindri-physics + sindri-voxel
sindri-decay      -> core + grid + physics + platform + scene + decay language crates
editor            -> assets + core + decay + physics + platform + render + scene
sindri-causeway   -> consumer of the engine; nothing depends on it
games/*, examples/* -> consumers of the engine; nothing depends on them
```

Important constraints:

- `sindri-core` has no window, GPU, browser, editor, physics, scripting, or async
  executor dependency.
- `sindri-voxel` owns voxel coordinates, section/chunk storage, residency, dirty
  tracking, generation contracts, and CPU-side meshing policy. It must not
  depend on Causeway, scene JSON, the editor, Decay, wgpu, or renderer-specific
  GPU types.
- `sindri-render` does not depend on `sindri-core`; `sindri-scene` is the seam.
- Engine crates never depend on the editor or the companion game.
- `decay/` is a separate Cargo workspace and may not depend on `sindri-*` crates.
  `sindri-decay` is the one-way bridge into the language.
- Create a new crate only at a proven platform or dependency boundary.
- Before adding a dependency, check MSRV 1.95, WASM compatibility where required,
  licence policy, and `deny.toml`.

## Capability completion rule

A surface earns ✅ in `docs/parity.md` only when the behaviour is
implemented **and exercised** on that surface. Do not mark an API, schema,
component, or editor control complete merely because it exists.

`docs/parity.md` also carries rows for capabilities Sindri does not have. When
work uncovers a gap, add its row in the same change, marked ❌, even when
nothing is planned: a gap with no row is a gap nobody schedules.

When a capability changes, update the relevant documentation in the same commit:

- `docs/parity.md` for Engine / Editor / Decay / proof status, and for the
  judgement against what an engine is expected to do.
- `docs/capabilities.md` for detailed evidence and limitations.
- The relevant subsystem contract in `docs/` when behaviour changes.
- `docs/generated/` when the Decay host surface or a component registration
  changes — regenerate with `cargo run -p sindri-capabilities -- --write`.
  These files are never hand-edited, and a stale one fails the workspace
  tests. On a pull request from this repository, the autofix workflow
  regenerates and commits them when they are stale.
- `CHANGELOG.md` for user-visible behaviour.
- `ROADMAP.md` only when an item's real acceptance criteria are complete.

For gameplay capabilities, name the game that exercises them (Causeway or a
genre showcase for a capability that already existed, Orbital Last Stand or
the showcase that found it for one being added) in the same feature track. A capability exercised by neither is not complete, and
saying so is better than an unqualified checkmark.

## Working method

Prefer small, reviewable feature slices over giant implementation commits. For a
large subsystem, establish the architecture and dependency boundary before
writing the implementation.

Do not use CI as the primary debugger. Before pushing code, run every relevant
check that can reasonably be reproduced locally. When a check fails, inspect the
whole affected path rather than patching only the first diagnostic and pushing
again.

### Verify APIs before use

Do not infer a Sindri or Decay API from conventions in another engine, language,
or an earlier version of this repository. Before introducing an unfamiliar API
name, type, method, field, event, or syntax form, locate its current definition
or an existing valid use in this checkout and verify its ownership and signature.
If neither exists, treat the capability as absent rather than inventing it.

For Decay this rule is strict: use `decay/LANGUAGE.md`,
`docs/decay-agent-guide.md`, the current host registrations, and existing checked
scripts as the authority for syntax and callable host APIs. Search before writing
an unfamiliar construct, then run the typed `decay-lsp --check` preflight on every
changed script. Familiarity with Lua, JavaScript, Rust, or an older Decay script
is not evidence that a construct exists in current Decay.

### Mandatory pre-push gate

Do not push a code change merely because the edited code looks correct. Before
every push, including a small CI-fix commit, perform the cheapest applicable
checks first and inspect the changed files for warning-level problems.

For the common case, run `scripts/preflight.py` first. It discovers changes against
`origin/main`, includes committed, staged, working-tree, and untracked edits, runs formatting and file-size
checks, typed Decay preflight for changed scripts, and check/tests the affected
workspace crates. Use `--base <ref>` for another integration branch and `--list`
to inspect the discovered scope without running checks.

The command is the shortest route through the cheap gates, not a substitute for
surface-specific checks below. For Rust changes, the minimum pre-push sequence is:

1. Run or reproduce `cargo fmt --all --check`.
2. Compile/check every changed crate with warnings denied:
   `RUSTFLAGS="-D warnings" cargo check -p <changed-crate> --all-targets --all-features`.
3. Run the changed crate's tests:
   `cargo test -p <changed-crate> --all-features`.
4. If the crate is part of the WASM graph, check it for
   `wasm32-unknown-unknown`.
5. Before pushing, inspect the final diff for:
   - unused imports, variables, functions, and dependencies;
   - formatting changes still required;
   - exhaustive matches affected by new enum variants;
   - generated artifacts that need regeneration;
   - documentation/parity changes required by the capability rules;
   - files approaching repository size limits.

When local command execution is unavailable, do not silently substitute CI for
this gate. Work through
[Working without local execution](#working-without-local-execution), explicitly
checking the items above, and treat the subsequent CI run as unverified until it
completes.

A CI-fix commit must pass the same gate. "Only one line changed" is not an
exemption.

### CI failure triage

When CI fails, inspect every failed job on the current PR head before changing
code.

Determine whether failures:

- share one root cause;
- expose independent problems;
- are downstream or skipped consequences of an earlier gate.

Do not patch the first visible diagnostic and push without checking the other
failed jobs.

After fixing a CI failure, search the affected crate or file for the same class
of problem before pushing. For example:

- one unused import -> inspect all changed imports and warnings;
- one formatting failure -> format/check all changed Rust files;
- one non-exhaustive match -> search all matches of that enum;
- one stale generated file -> identify every generated artifact affected by the
  source change.

Never describe a PR as fixed or ready until the final head has passed the
required checks.

Before editing a `.decay` file, a scripted prefab, or the Decay host surface,
read `docs/decay-agent-guide.md`. Before changing `decay-lsp`, Decay editor
integration, batch-preflight diagnostics, or their semantic tooling contracts,
read `docs/decay-lsp-modernization.md` and update its checklist in the same
change when an item moves. Run the typed batch preflight for every changed
script before pushing; its runtime-contract reminders must be reviewed even
though only syntax and semantic diagnostics make the command fail.

Do not introduce temporary self-modifying workflows or repository automation to
work around ordinary development problems. The permanent autofix workflow
(`.github/workflows/autofix.yml`) is the reviewed exception: it only runs rustfmt
and the capability generator in write mode. Do not widen it to anything that
changes behaviour. If the implementation approach starts
requiring machinery whose only purpose is to repair the branch, stop and reassess
the approach.

Before declaring a PR ready:

1. Review the final diff from `main`, not just the latest commit.
2. Re-check dependency direction and target-specific `cfg` behaviour.
3. Verify the documentation and game integration the capability rule requires.
4. Run the required native, Decay, WASM, browser, render, or dependency checks
   that the touched code can affect.
5. Confirm CI is green on the final head.

## Required checks

For the main workspace:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo check --workspace --all-features --target wasm32-unknown-unknown
scripts/check-file-size.py
```

CI sets `RUSTFLAGS=-D warnings`; warnings are failures. Changes affecting render
output must keep the deterministic captures and colour verification green.
Changes affecting browser behaviour must run the real browser smoke tests, not
only compile WASM. Changes affecting dependencies must satisfy `cargo deny` and
the repository dependency policy.

For Sindri gameplay scripts (pass every changed file, or a project directory):

```bash
cargo run --quiet --package decay-lsp -- --check path/to/changed.decay
```

This compiles against Sindri's real host environment. It does not replace the
runtime regression required to prove frame ordering, signals, spawning, or
movement behavior.

For the separate Decay workspace:

```bash
cd decay
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run additional subsystem-specific tests described by the relevant docs and CI
workflow. Do not claim a target or surface works merely because another target
compiled.

## Working without local execution

Some agents edit this repository without a shell: every check first runs in CI.
The rules above still apply; this section makes the static review concrete.
Every item here is a failure that has repeatedly cost a push.

### What CI fixes for you

On a pull request from a branch of this repository, the **Autofix** workflow
runs `cargo fmt --all` (root and `decay/`) and
`cargo run -p sindri-capabilities -- --write`, and commits any change to the
branch as `Apply automatic formatting and regeneration`. So:

- Do not spend a commit on formatting alone, and never hand-edit
  `docs/generated/`. Write code in rustfmt's style as well as you can and let
  autofix settle the rest.
- **After an autofix commit lands, re-read every file you are about to edit from
  the branch head.** Writing a file from content you fetched before the autofix
  commit silently reverts its fixes, and autofix then has to run again.
- The CI run on the commit before the autofix commit is cancelled. The run that
  matters is the one on the autofix commit.

### One push should carry one complete repair

CI reports everything in one run: the compiling jobs no longer wait for the
formatting gate, Clippy and the WASM check use `--keep-going`, and tests run
with `--no-fail-fast`. Read the whole run — the `CI failure summary` job and
every failed job's step summary — and fix every reported occurrence before the
next push. Fixing the first diagnostic and pushing again is the pattern this
section exists to stop.

### Before adding code to a file

- **File length.** Rust files are capped at 600 lines, and several are already
  within a few lines of it. Before adding to a file, check its length. If the
  result would pass about 580 lines, split the file by responsibility first
  (`docs/module-layout.md`), in the same change.
- **Function length.** Clippy's `too_many_lines` rejects a function body over
  100 lines. It is the most frequent Clippy failure here. When a function you are
  extending is already near that, extract a helper rather than adding to it.
  Adding `#[allow(clippy::too_many_lines)]` is not a fix.

### Clippy patterns that have actually failed CI here

CI runs Clippy with `pedantic` enabled and warnings denied. Write these forms
from the start:

| Rejected | Write instead | Lint |
| --- | --- | --- |
| A type or identifier in a doc comment without backticks: `/// Uses VoxelSource` | ``/// Uses `VoxelSource` `` | `doc_markdown` |
| `output.push_str(&format!("…{x}"))` | `let _ = write!(output, "…{x}");` with `use std::fmt::Write;` | `format_push_string` |
| `format!("{}", name)` | `format!("{name}")` | `uninlined_format_args` |
| `assert_eq!(value, 0.0)` on floats | `assert!(value.abs() < f32::EPSILON)` or an explicit tolerance | `float_cmp` |
| `x as f32`, `len as u32`, `i as usize` | `f32::from`/`u32::from` where lossless, `u32::try_from(…)` where it is not, or an `#[allow]` on the smallest item with a comment saying why the cast is safe | `cast_precision_loss`, `cast_possible_truncation`, `cast_sign_loss` |
| `r#"…"#` for a string with no `"` in it | `r"…"` | `needless_raw_string_hashes` |
| `const` or `fn` declared after statements inside a function | declare it at the top of the function or at module level | `items_after_statements` |
| `3 \| 4 \| 5 =>` | `3..=5 =>` | `manual_range_patterns` |
| `match option { Some(x) => …, None => … }` with a block in each arm | `if let Some(x) = option { … } else { … }` | `single_match_else` |
| `if` nested directly inside a match arm or another `if` | a match guard or a combined condition | `collapsible_if`, `collapsible_match` |
| `let mut s = T::default(); s.field = …;` | `T { field: …, ..T::default() }` | `field_reassign_with_default` |
| `.map(\|v\| v.as_f64())` | `.map(serde_json::Value::as_f64)` | `redundant_closure_for_method_calls` |
| a non-public `&self` method that never reads `self` | an associated function, or a free function | `unused_self` |
| taking `String`/`Vec`/a struct by value and only reading it | take `&str`/`&[T]`/`&T` | `needless_pass_by_value` |

Also check, for every change:

- every new `use` is used on every target (`#[cfg(target_arch = "wasm32")]`
  code included), and every removed use leaves no orphaned import;
- every `match` on an enum you extended has the new variant;
- every new public item that `sindri-capabilities` documents is reflected in
  `docs/generated/` (autofix handles this when the generator runs cleanly);
- any Decay script you changed uses only syntax and host calls that appear in
  `decay/LANGUAGE.md`, `docs/decay-agent-guide.md`, or an existing checked script.

When CI reports a lint not in this table and it recurs, add it here in the same
pull request that fixes it.

## Core conventions

- Rust 1.95.0, edition 2024, resolver 3. Do not raise the MSRV casually.
- Workspace forbids unsafe code.
- Prefer fixing Clippy pedantic warnings over adding `#[allow]`; justify any new
  allowance in a comment.
- Shared dependency versions live in `[workspace.dependencies]`.
- Library code returns typed errors rather than panicking.
- Runtime `EntityId` handles are not serialized `SceneEntityId` values.
- Scenes are versioned, canonical, and preserve unknown component payloads.
- Gameplay writes the world; `sindri-scene` derives renderer/navigation state.
- Gameplay rules and decisions belong in Decay, including gameplay demos and shipped games. Rust implements engine capabilities, host/runtime plumbing, and low-level engine/platform/render examples; do not put bespoke movement, combat, camera triggers, scoring, or similar game rules in a Rust `Game` implementation.
- Project fonts are assets, never operating-system lookups.
- Asset loading is genuinely asynchronous; never fake synchronous browser I/O.
- Browser and native loops share semantics, not identical plumbing.
- WebGPU is the first browser backend; WebGL fallback remains deliberately
  deferred unless the roadmap changes.
- Editor mutations go through checked commands and undo/redo, not direct world
  writes.
- Rust source files stay under 600 lines and aim for 400. Split by
  responsibility, not by line count; see `docs/module-layout.md`.

## Commits

Use imperative, specific subject lines under roughly 55 characters. Commit
bodies should explain what was wrong, what changed, and why that shape was
chosen. Keep unrelated cleanup out of feature commits.
