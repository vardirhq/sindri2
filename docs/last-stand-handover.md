# Last Stand parity: where the work is, and how to continue it

A handover for whoever picks this up next. It assumes you can read the
repository but were not present for the decisions, so it says why as well as
what — the *why* is the part that stops you undoing something on purpose.

Read `AGENTS.md` first. It is the canonical, tool-neutral guidance and it
overrides anything here that contradicts it.

## The goal

`games/orbital-last-stand/` is a recreation of the "Last Stand" mode of a
reference game. Last Stand is that game's `campaign` mode: a ten-minute
survival run with a boss every sixty seconds.

The recreation is **not** the point on its own. It is the forcing function for
the engine: every gap it exposes is a gap in Sindri, and the fix belongs in the
engine rather than in the game wherever the shape (not the tuning) is general.
That principle has already been applied twice and is the single most important
thing to preserve.

## The reference

Clone it read-only:

```bash
GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 \
  https://github.com/MadsenDev/tester-repo /home/user/madsendev/tester-repo
```

Plain JavaScript, canvas 2D, ~12k lines in `src/`. The files that matter:

| File | What it holds |
| --- | --- |
| `src/modes.js` | Last Stand is `campaign`: `runLimit` 600s, `bossInterval` 60s |
| `src/module-catalog.js` | 160 modules, as a `\|`-separated name list plus stat tables |
| `src/special-modules.js` | The legendary tier |
| `src/entities.js` | 15 enemy kinds (`kind: "..."` — anchor, brute, bulwark, burrower, dart, leech, orbiter, phaser, relay, scout, sentinel, sniper, spitter, swarm, wisp) |
| `src/enemy-ai.js` | How each kind behaves |
| `src/bosses.js` | Boss definitions |
| `src/synergies.js` | What combinations do |
| `src/companions.js`, `src/companion-physics.js` | Orbitals |
| `src/player-state.js` | The stat hooks a module can touch |
| `src/enemy-render.js`, `src/ship-render.js`, `src/canvas-shapes.js` | All procedural drawing |

## Where we are

Counting honestly: reference has 15 enemy kinds, 8 ships, 160 modules, a 600s
run with ten bosses. We have 4 enemies, 6 modules, one boss, a 180s run.

**Just landed** (PR #154, on branch `claude/orbital-last-stand-recreation-9sq1je`):

- A stat block. `assets/scripts/stats.decay` derives fourteen stats from a base
  plus an *additive* pile and a *multiplicative* pile per stat. Modules
  contribute to the piles; they never write a stat directly.
- `assets/scripts/module.decay` replaced `upgrade-card.decay`. A module is now
  two key names and two numbers, not a branch in an if/else.

**The agreed plan**, in order. Mechanisms first, then content; visual polish
interleaved so nothing ships looking placeholder:

1. **Weapon flags.** `pierce`, `shots` and `crit` are already stats. The ones
   that make a build *feel* different are `missile` (seeking), `arc` (chain),
   `nova` (on-death burst), `mines`, `beam`. Reference spells these as `flags:`
   entries in `module-catalog.js`.
2. **Enemy roster** toward fifteen kinds. See the rule below about tells.
3. **Boss cadence.** 600-second run, a boss every 60 seconds, several distinct
   bosses. Currently `director.decay` has `boss_at: 180.0` and one warden.
4. **Companions / orbitals.** Mechanically distinct and the best-looking thing
   in the reference.
5. **The catalog**, ~40 modules chosen so every mechanism above is exercised at
   least twice. This is data entry once 1–4 exist, which is exactly why it is
   last. Authoring it first would produce forty ways to spell "+12% damage".

## Rules that are not negotiable

**Run the required checks before every commit.** From `AGENTS.md`:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo check --workspace --all-features --target wasm32-unknown-unknown
scripts/check-file-size.py
```

CI sets `RUSTFLAGS=-D warnings`, so warnings fail. If you touch `decay/`, that
is a separate workspace with its own checks — they are listed in `AGENTS.md`.

**Develop on `claude/orbital-last-stand-recreation-9sq1je`.** Push there. If
its pull request has already merged, restart the branch from `origin/main`
rather than stacking on merged history.

**The 600-line file cap is enforced.** When a Rust file crosses it, split the
tests out beside it: `gesture.rs` / `gesture_tests.rs` is the established
pattern, and `state.rs` / `state_tests.rs` follows it.

**Docs and the script surface are checked against each other.** Adding a Decay
namespace without documenting it in `docs/scripting.md` fails
`the_document_lists_exactly_the_namespaces_a_script_can_reach`. This is a
feature; do not route around it.

## Things the language and engine will not let you do

These are real constraints, discovered the hard way:

- **A Decay script cannot call another script.** Scripts communicate through
  the blackboard (`Game.set` / `Game.get`) or by an entity acting on itself.
  This is why a module applies its own effect.
- **Decay has no string concatenation.** `+` does not join text. That is why
  `stats.decay` writes out `"damage_add"` and `"damage_mul"` in full rather
  than building them from `"damage"`. Do not add concatenation to the language
  to save typing here.
- **`@export let x: String` works**, and `Game.set` accepts a variable key. That
  combination is what makes a data-driven module possible at all.
- **`World.with_tag` answers with active entities only.** A hidden card is not
  active, so anything reading a catalog must read it while the screen is up —
  see the comment at the top of `upgrade-chooser.decay`.
- **A prefab is referenced through an `@export let x: Prefab`**, never a string
  literal, so the asset pipeline can see it.

## Traps I walked into, so you do not have to

**Test at a scale factor that is not 1.0.** A phone could not press Start for
three rounds of fixes because positions arrived in logical pixels while the
viewport they were tested against was physical. On a desktop those are the same
number, so every test passed. Two tests asserted the bug *in their names*. When
something works on desktop and fails on a phone, suspect a unit mismatch before
suspecting logic.

**Verify a regression test fails without its fix.** One test I wrote did not
guard the fix it shipped with — it passed with the fix reverted. Revert, run,
confirm red, restore. It costs one command.

**Disk fills up.** The linker dies with `Bus error` or `No space left on
device`, which reads like a compiler bug and is not. Recover with:

```bash
find target -maxdepth 3 -type f ! -name "*.rlib" ! -name "*.rmeta" ! -name "*.d" -size +5M -delete
rm -rf target/debug/incremental target/debug/build
```

Use `CARGO_INCREMENTAL=0` for full-workspace runs.

**Do not `git checkout <file>` to undo an experiment** if that file also holds
work you want. It reverts everything. Comment out the one line instead.

## The design rule that matters most

Everything in the reference that reads as "good game feel" is a *tell*: a
telegraph the player can read and answer. The charger is the one the author
singled out as working, and it is the template — it changes colour when it
spots you, stops, then commits. Three beats, all legible.

So when you add an enemy, the behaviour and its tell are one piece of work, not
two. An enemy that arrives without a tell is not a smaller version of a good
enemy; it is a different and worse one.

The same goes for looking better. The reference is flat canvas 2D with no glow.
Sindri already has SDF text, stroked procedural shapes and a measured particle
path, so layered strokes give real bloom, and flecks give trails, impacts and
death bursts the original never had. Screen shake, hit flash and damage numbers
are all reachable. Matching the reference visually is the floor, not the target.

## Working notes

- The game's own suite is the fastest signal: `cargo test -p orbital-last-stand`.
  It includes a full ten-minute run and an export check.
- `cargo test -p sindri-editor --test orbital_project` compiles every script in
  the project and checks every prefab the scripts spawn is loaded.
- Deployed builds stamp their commit in the corner of the page, and
  `?input-debug` on the game URL traces what the browser delivers to the canvas.
  Use the stamp before debugging anything reported from a device.
