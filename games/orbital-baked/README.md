# Orbital Last Stand — Baked

Orbital Last Stand with its engine-drawn vector art replaced by baked 3D sprite
sheets, produced by `tools/isometric-baker` from recipes that live beside the
textures they make.

It is a copy, deliberately. The original is the forcing function described in
`AGENTS.md` — a game built only through the editor and Decay, whose job is to
find what the engine cannot do yet. Nothing here is allowed to change that, so
this variant sits alongside it rather than replacing it, and the original's
assets and scripts are untouched.

## What is baked

Each from a `.isobake.json` recipe in `assets/textures`:

| sheet            | what it draws                      | frames |
| ---------------- | ---------------------------------- | ------ |
| `player`         | the hero interceptor               | 4      |
| `core`           | the drifting core                  | 4      |
| `drifter`        | the hex sentry                     | 4      |
| `charger`        | the ram                            | 4      |
| `challenger`     | the orange gunship, five armour tiers | 20  |
| `warden`         | the boss fortress                  | 4      |
| `aegis`          | the shielded fortress hub          | 4      |
| `shield-segment` | one plate of the Aegis’s ring, three states | 9      |
| `detonation`     | the mine                           | 5      |
| `asteroid`       | three sizes of rock                | 3      |

The four-frame sheets are animation clips, played by `sindri.animation.sprite`
and looping: drives pulse, lenses breathe, the warden's cannons charge in
sequence around its ring.

Re-bake any of them with:

```
cd tools/isometric-baker
node src/cli.ts ../../games/orbital-baked/assets/textures/<id>.isobake.json \
  --out ../../games/orbital-baked/assets
```

## What is deliberately not baked

Everything the game draws with `blend: "add"` — beams, arcs, novas, the shield,
the engine flames, hazard fields, the elite identity rings. Those are light
rather than objects: baking a glow into a sprite sheet flattens exactly the
additive blending that makes it read as light. They stay engine-drawn, which is
also why this variant still exercises the shape renderer.

The elite identity markers (shell, ring, prong, arc, phaser) stay vectors for
the same reason — they are drawn over an enemy to mark it, closer to interface
than to object.

## What the sprites cost

Two things the vector art did that a sprite cannot, and what replaced them:

- **The challenger shed a polygon side per damage tier.** A sheet has fixed
  geometry, but it can hold more than one, so the Spine's five armour tiers are
  baked as five sets of frames and selected by clip: ten plates ringing the hull
  at full health, five and a row of torn sockets at the end.
- **Every asteroid generated its own outline** from `World.set_shape_point`, so
  no two were the same rock. There are now three baked rocks, one per size.

Both are recorded here rather than in a commit message because they are what a
person notices when playing the two side by side.

## Seeing a boss on purpose

The bosses arrive on a timer, so most of them are a long wait away. The title
screen's rush mode opens on whichever boss its picker is showing, and the
capture binary can do the same from the command line:

```
cargo run -p orbital-baked --bin orbital-baked-capture -- \
  aegis.png 960 540 boss-11
```

`boss-0` is the Warden and `boss-11` is the Aegis; the shot presses the picker
that many times, starts the rush, and photographs the fight seven seconds in.

## A thing that only shows up on a boss

The Aegis's shield plates were spawned as children of the hull, which is how a
prefab says "these belong to it" and is what `docs/prefabs.md` describes. It
drew and collided at the arena's origin instead, because a world sprite and a
physics body both read an entity's own transform and neither folds a parent's
into it — shape extraction does, which is why the elite markers that had the
same bug were fixed and this one was not noticed. `docs/physics.md` already says
not to parent a body; the plates are siblings now and follow the boss by reading
where it is. Worth knowing before parenting anything that has to be *hit*.
