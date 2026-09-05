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
assets/orbital.scene.json    69 entities: the ship, the screens, the catalog
assets/prefabs/              19 things that get spawned
assets/scripts/              25 Decay scripts, and all of the game's rules
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

**Damage belongs to the thing that deals it.** A spawned projectile carries its
authored per-instance damage, and the enemy reads it from the entity named by
the collision. That is what lets a critical round, a 27% arc and a 30% nova
share collision code without racing through a global damage slot. Enemy touch
damage still flows through the shared board because it is a message to the
player, and only the worst single touch is kept: invulnerability makes a crowd
one hit rather than thirty simultaneous ones.

Five reference weapon flags are playable through the upgrade catalog. Guidance
steers rounds, arc impacts jump, nova kills burst across an area, gravity
anchors leave delayed mines, and prism impacts continue as piercing beams. The
effects are ordinary authored prefabs and Decay scripts; no weapon kind was
added to the engine.

The complete fifteen-enemy reference roster arrives on its original unlock
timeline. After 1:45, regular spawns can become one of five visibly distinct
elite traits with the reference chance curve and health, damage, speed and
value multipliers. Defeated enemies feed the Normal drop economy: ordinary,
elite and boss chances differ, missing hull improves the roll, and a
120-kill pity guarantees a repair. Repair, arena-clearing pulse and eight-second
overdrive pickups are ordinary authored prefabs too.

Bosses arrive every minute from a director that unlocks larger pools at 4:00
and 8:00 while excluding its two most recent choices. All eleven reference
bosses are present, from Harrower's telegraphed charges and Brood's summons to
Spine's breakable armor and Leviathan's edge volleys. Like regular enemies, a
boss outside the viewport may approach but cannot target or attack the ship.

## What is not here yet

The reference game is much larger. Companions, the broader
module and synergy catalog, sector routes, events, ships, contracts, archive
and debrief remain parity work. See `docs/last-stand-reference-parity.md` for
the implementation order.
