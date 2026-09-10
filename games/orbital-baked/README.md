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

Six sheets, each from a `.isobake.json` recipe in `assets/textures`:

| sheet        | what it draws                    | frames |
| ------------ | -------------------------------- | ------ |
| `player`     | the hero interceptor             | 4      |
| `core`       | the drifting core                | 4      |
| `drifter`    | the hex sentry                   | 4      |
| `charger`    | the ram                          | 4      |
| `challenger` | the orange gunship               | 4      |
| `warden`     | the boss fortress                | 4      |
| `asteroid`   | three sizes of rock              | 3      |

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
  geometry, so the tell is carried as colour instead: the hull loses green and
  blue as its armour goes, and a nearly dead one reads as raw red.
- **Every asteroid generated its own outline** from `World.set_shape_point`, so
  no two were the same rock. There are now three baked rocks, one per size.

Both are recorded here rather than in a commit message because they are what a
person notices when playing the two side by side.
