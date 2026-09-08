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
assets/orbital.scene.json    79 entities: the ship, screens, catalogs, FX palettes
assets/prefabs/              28 things that get spawned
assets/profiles/             reusable module and synergy catalogs
assets/scripts/              35 Decay scripts, and all of the game's rules
assets/ui.weave              composed responsive presentation entry point
assets/ui/                   HUD, overlay, and screen presentation modules
src/lib.rs                   a harness that plays it without a window
```

**There is no game code in Rust.** That is the test. The audit says that if the
game must reach into engine internals, the public authoring surface is still
incomplete — so everything the game does is authored, and the only Rust here
assembles the same public pieces a host assembles, in the order a host runs
them. If the game needed anything private, `src/lib.rs` could not be written.

The same authored UI entities are presented on desktop and portrait viewports.
Weave owns responsive geometry and visual styling; Decay continues to own screen
state, button actions, runtime text, and fill values. The migration is covered
by `crates/sindri-weave/tests/last_stand.rs` and documented in
`docs/weave-migration.md`.

## Three decisions worth knowing about

**Responsive UI is presentation, not gameplay.** The stylesheet is split into
HUD, overlay, and screen modules composed by `ui.weave`. Portrait rules
recompose the title and upgrade choices by viewport shape rather than guessing
a device from a fixed width. Stable entity IDs keep existing Decay behavior and
button hit targets intact.

**The upgrade and synergy catalogs are profile assets, not script branches.**
All 160 reference modules, their weighted pools and generic effects live in
`module-catalog.profile.json`. The 19 reference synergy recipes live beside
them in `synergy-catalog.profile.json`. The chooser, module interpreter and
synergy evaluator hold typed `Profile` fields, so ordinary additions are data
changes and the editor can author the same assets the runtime loads.

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
added to the engine. Six companion families and their Foundry Signal level pool
are authored the same way. Synergy flags react to the derived build every frame;
the core projectile and companion interactions consume them directly.

Combat presentation uses two complementary authored paths. Reusable additive
shape prefabs provide muzzle flashes, impact rings, synergy reveals and nova
waves; the fixed-size effect pool provides dense projectile, companion,
hostile-fire and death flecks without growing the entity graph. Weapon families
select their own mint, cyan, magenta, violet or gold vocabulary, while critical
hits, mines, player damage and boss deaths feed a short deterministic camera
trauma response. Run the `orbital-capture` binary with the `spectacle` shot to
put every weapon proc and four companion families into one review frame.

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

The reference game is much larger. Full-fidelity effects for every synergy,
sector routes, events, ships, contracts, archive and debrief remain parity work.
See `docs/last-stand-reference-parity.md` for the implementation order.
