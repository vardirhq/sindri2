# Orbital Last Stand

A ten-minute action game, and the second vertical-slice acceptance project
`docs/orbital-last-stand-audit.md` asked for.

```bash
cargo test -p orbital-last-stand                       # play it, in a test
cargo test --release -p orbital-last-stand -- --ignored --nocapture   # ten minutes
cargo run -p sindri-export --bin sindri-export -- games/orbital-last-stand dist
```

## What it is made of

```text
assets/orbital.scene.json    66 entities: the ship, the screens, the catalog
assets/prefabs/              8 things that get spawned
assets/scripts/              15 Decay scripts, and all of the game's rules
src/lib.rs                   a harness that plays it without a window
```

**There is no game code in Rust.** That is the test. The audit says that if the
game must reach into engine internals, the public authoring surface is still
incomplete — so everything the game does is authored, and the only Rust here
assembles the same public pieces a host assembles, in the order a host runs
them. If the game needed anything private, `src/lib.rs` could not be written.

## Two decisions worth knowing about

**The upgrade catalog is entities, not a table.** Each card is an entity with
its own words, its own numbers, and the tag `upgrade`; the chooser asks
`World.with_tag("upgrade")` and switches three of them on, and each card
applies its own effect because a script cannot call into another script.
Adding an upgrade is adding an entity to the scene.

The chooser reads its catalog once, in `start`, and holds it — `World.with_tag`
answers with *active* entities only, and a hidden card is not active. That is
also why the chooser owns the upgrade screen: anything that switched the screen
off before the catalog was read would take the catalog with it.

**Damage flows through the shared board, not through the collision.** A
collision event names the other half of a pair, not what that half is worth, so
an enemy that touches the ship writes what it costs and the ship reads it. It
writes the *worst* single touch rather than adding them up: the ship gets
invulnerability frames after a hit, so being inside a crowd of thirty is one
hit.

## What is not here yet

The audit's twelve points, and no more. The reference game is roughly 20,000
lines across 66 modules, and the rest of it — expeditions, companions, the
synergy catalog, hazards, sector routes, ships, contracts, the archive, the
debrief — is parity work that has not started. See the plan for the order.
