# Last Stand visual parity

This is the visual counterpart to `docs/last-stand-reference-parity.md`.
Gameplay parity is not enough if the recreated game does not read like the
reference at a glance. Orbital Last Stand should reproduce the reference game's
shape language, motion, colour hierarchy, UI composition, and combat feedback
before Sindri-specific embellishment is layered on top.

The reference for this document is `MadsenDev/tester-repo` at commit
`a2b799ae6a78c50c078d5053c26a389d397b4988`, especially:

- `src/render.js`
- `src/enemy-render.js`
- `src/ship-render.js`
- `src/ships.js`
- `src/entities.js`
- `src/game-ui.js`
- `gameplay-overlays.css`

## Reference motion language

The arena is almost never visually still.

- every ordinary enemy rotates at `time * 0.6 + phase`
- elite shells counter-rotate at `-time * 1.4`
- boss shells rotate independently at `time`
- salvage gems rotate at `time * 2`
- player orbitals move around the ship at `time * 2.1`
- mines rotate at `time * 2 + phase`
- the player shield rotates at `time * -1.5`
- Phaser opacity pulses at `sin(time * 12)`
- elite identity effects pulse at either 6 or 12 Hz depending on trait
- telegraphs, warnings, engine thrust, particle fields, and several hazard cues
  pulse or drift continuously
- the backdrop grid drifts at 8 px/s horizontally and 4 px/s vertically

Static equivalents are not visually faithful even when their gameplay timing is
correct.

## Regular enemy silhouettes

Sindri's numeric roster order is not the same as the reference array order. The
visual mapping below is the canonical one for Orbital Last Stand.

| Kind | Enemy | Polygon sides | Reference colour |
| ---: | --- | ---: | --- |
| 0 | Scout | 5 | `#ff845f` |
| 1 | Brute | 5 | `#ff6f5a` |
| 2 | Dart | 3 | `#ffb35a` |
| 3 | Bulwark | 6 | `#ff5f74` |
| 4 | Wisp | 4 | `#d76dff` |
| 5 | Anchor | 6 | `#a77cff` |
| 6 | Spitter | 6 | `#75d7ff` |
| 7 | Swarm | 3 | `#8dffcf` |
| 8 | Relay | 7 | `#62f4d0` |
| 9 | Sniper | 6 | `#ffef83` |
| 10 | Orbiter | 7 | `#91a2ff` |
| 11 | Burrower | 4 | `#ff986b` |
| 12 | Leech | 5 | `#ff72b6` |
| 13 | Sentinel | 6 | `#65f1ff` |
| 14 | Phaser | 3 | `#d8a0ff` |

The base polygon is only the first layer. The reference also gives several
archetypes secondary marks which must be reproduced:

- Leech: partial outer arc
- Sentinel: three radial prongs
- Phaser: pulsing outer triangle
- Anchor: four radial arms
- Relay: partial outer arc
- Burrower: dashed outer ring whose visibility follows submerged state
- Bulwark: support-radius treatment

## Player reference

The default Strider is an authored six-point hull, not a generic triangle. Its
normalized outline is:

```text
[0,-1.25], [.76,.82], [.25,.62], [0,.98], [-.25,.62], [-.76,.82]
```

It also has:

- an internal chevron line from `[-.5,.62]` through `[0,-.72]` to `[.5,.62]`
- two engines at `[-.3,.68]` and `[.3,.68]`
- pivot `[0,.14]`
- canopy `[0,-.12]`
- hull colour `#78ebff`
- accent `#e9fdff`
- thrust trails whose length pulses with movement
- a rotating six-sided shield around the hull

The other selectable ships have their own authored point sets and should be
ported only after the Strider establishes the rendering pattern.

## Backdrop

Each sector owns its background, grid, and accent colours. On top of the flat
sector colour the reference draws:

1. a 48 px grid drifting left/up with time;
2. 18 small accent particles with deterministic base positions and differing
   horizontal speeds;
3. sector hazards and encounter visuals above that background but below actors.

Sindri may add bloom or richer particles later, but the moving grid and sector
colour change are part of reference identity and should land first.

## UI parity

The visual-parity track must also cover the parts outside the arena. Equivalent
information in a different composition is not considered done.

Required reference passes:

1. title/menu composition and typography;
2. in-run HUD placement and hierarchy;
3. boss health treatment;
4. upgrade/module cards;
5. pause and result overlays;
6. route, event, Black Signal, and other campaign overlays as those systems land.

Mobile parity remains mandatory. A desktop-faithful HUD that becomes a different
design on a phone is not sufficient.

## Implementation order

1. **Enemy base silhouettes, colours, and constant spin.**
   Add runtime polygon-side authoring to Decay and use it from the shared roster.
   Make Dart use the same reference spin and make salvage gems rotate.
2. **Enemy secondary marks and elite shells.**
   Add the extra arcs/prongs/rings and independently animated elite treatment.
3. **Strider hull, details, engines, and rotating shield.**
4. **Moving sector backdrop.**
5. **Projectiles, pickups, hit flash, telegraphs, particles, and combat glow.**
6. **HUD and menu reconstruction.**
7. **Boss visual parity as the full boss roster lands.**
8. **Capture-based comparison pass on desktop and phone.**

## Current status

The first slice introduced Decay access to a procedural polygon's `count`, then
used it in Orbital Last Stand to give all 15 regular archetypes the reference
base side count and colour. Regular enemies and Dart receive the reference
0.6-rad/s continuous rotation, and salvage receives its reference 2-rad/s spin.

The second slice starts the layered enemy treatment. Every elite now spawns a
silhouette-matched outer polygon and a larger pulse ring as decorative children.
Because the enemy itself rotates at +0.6 rad/s while the shell rotates at -2.0
rad/s locally, the shell reaches the reference -1.4 rad/s world rotation. The
ring uses the reference `0.4 + 0.25 * sin(time * 7 + phase)` alpha range without
becoming a collider, target, or independently destructible entity.

Step 2 is still incomplete: the trait-specific elite identity marks and the
secondary archetype marks for Leech, Sentinel, Phaser, Anchor, Relay, Burrower,
and Bulwark remain open. The exact Strider hull, shield, backdrop, HUD, and full
combat presentation also remain open and should not be described as visually
complete until capture comparison says so.
