# Orbital Combat Lab

The Combat Lab is the proving ground for reusable attack primitives before they are assigned to bosses.

The rule is simple: a boss composes attack entities. It does not own their collision, damage, lifetime, visuals or movement logic. If an attack cannot be spawned and tested here by itself, it is too coupled to a boss.

## First attack set

- **Gas Cloud** — persistent drifting area denial with periodic damage. An
  explosion inside it ignites the whole cloud into a larger, short blast.
- **Hostile Mine** — arms, telegraphs, then detonates when the player enters its
  trigger radius. Its blast damages reactive props, chains nearby mines and
  ignites nearby gas.
- **Shockwave** — an expanding damaging ring. The empty middle remains safe
  after the wave passes; its advancing edge pushes reactive bodies and triggers
  mines without tunnelling across them between frames.
- **Gravity Well** — the existing capability-tagged force field, repackaged as
  a lab attack. It bends projectiles and pulls any physical prop that explicitly
  opts into `gravity_bendable`.

The attack prefabs are the same assets future bosses should spawn:

- `prefabs/attack-gas-cloud.prefab.json`
- `prefabs/attack-hostile-mine.prefab.json`
- `prefabs/attack-shockwave.prefab.json`
- `prefabs/attack-gravity-well.prefab.json`

## Playground

`assets/combat-lab.scene.json` is an alternate development scene. It intentionally does not replace the game's main scene.

The offscreen capture can open it directly and stage the stress preset with
`cargo run -p orbital-baked --bin orbital-baked-capture -- out.png 1000 700 lab`.
CI uploads that real frame with the other visual-test captures.

It starts in autoplay and cycles through each primitive followed by three combination presets. The lightweight lab player exists only to move around and receive damage; the attack entities themselves are production assets.

Keyboard controls:

| Key | Action |
| --- | --- |
| `1` | Gas cloud |
| `2` | Hostile mine |
| `3` | Shockwave |
| `4` | Gravity well |
| `C` | Cycle combination preset |
| `R` | Clear active lab attacks |
| `Space` | Toggle autoplay |
| `I` | Toggle invulnerability |

The controller can also be driven without input through the shared game board. This is deliberate: a touch UI can use exactly the same commands later, and automated tests already do.

- `lab_spawn = 1..4` spawns one primitive.
- `lab_combo = 1..3` spawns a combination preset.
- `lab_clear = 1` clears all lab attacks.
- `lab_autoplay = 0/1` controls automatic cycling.
- `lab_invulnerable = 0/1` controls whether damage is applied.
- `lab_attack_count` reports how many attack entities are alive.
- `lab_last` reports the last primitive/preset requested.

## Combination presets

1. **Gas + gravity** — the field pulls reactive props toward the damaging zone
   while bending tagged projectiles.
2. **Minefield + shockwave** — the moving front triggers the minefield, whose
   overlapping blasts chain through neighbouring mines and props.
3. **Stress preset** — gas, gravity, mines and a shockwave react together: the
   wave starts the chain, explosions ignite gas, and gravity moves surviving
   physical targets through the hazards.

The point is not that these three combinations are boss designs. They are
interaction tests. The baked mine and gravity core communicate identity; the
procedural rings communicate live radius and timing. Good boss patterns can be
discovered here first and then composed into phase logic.

## Next primitives

The next useful additions are tractor/repulsion fields, barriers, artillery markers, sweeping flame/plasma cones, tethers, teleport strikes and destructible deployables. They should follow the same entity-first rule and gain a Combat Lab preset before being assigned to a boss.
