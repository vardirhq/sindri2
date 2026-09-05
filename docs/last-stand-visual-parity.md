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

The layered enemy treatment now gives every elite a silhouette-matched outer
polygon and larger pulse ring as decorative children. The shell applies the
reference local `-time * 1.4` counter-rotation inside the enemy's own rotating
transform, while the ring uses the reference 7 Hz, 0.4-through-0.65 pulse range
with per-instance phase so groups of elites do not pulse in lockstep.

Regular archetype secondary marks are represented across the whole roster.
Leech carries its 1.35-PI partial outer arc, Relay its three-quarter outer arc,
Burrower a dashed full outer marker, Phaser the separately pulsing outer
triangle, Sentinel three cyan-white radial prongs, Anchor four pale-violet arms,
and Bulwark the large faint 4 Hz support-radius ring. Burrower's outer marker
also follows the reference submerged-state visibility exactly: alpha 0.2 for the
first 1.7 seconds of its 3.4-second cycle, then 0.75 while surfaced. These remain
decorative children only: no tag, collider, targeting identity, or independent
combat lifetime is added.

Trait-specific elite identity cues are now authored from the reference renderer:
Armored receives three offset pulsing arcs, Volatile a breathing bright core,
Vampiric a large breathing violet ring, and Splitter three crack branches.
Frenzied intentionally has no additional identity mark beyond the generic elite
shell and pulse ring, matching the reference. The identity children reuse the
pulse ring's existing phase instead of consuming another random draw.

The Strider slice now has a real exact-hull path instead of approximating the
ship as a regular hexagon. Sindri's procedural shape renderer can carry up to
eight authored polygon vertices, and Decay exposes the bounded
`World.set_shape_point(index, x, y)` bridge. Orbital Last Stand writes the six
reference Strider hull coordinates through that path, uses the reference hull
and accent colours, and turns the ship toward movement with the same
frame-rate-independent `1 - exp(-12 * dt)` response used by the reference.

Step 3 is therefore **in progress**, not complete. Still open on the Strider are
the internal chevron, canopy/pivot marks, two animated engine trails, and the
rotating six-sided shield. Step 2 also retains two cleanup corrections before it
is signed off: elite base bodies are still recoloured to the trait colour instead
of preserving the archetype colour, and the generic elite pulse ring's phase is
still generated inside the decorative child rather than propagated from the
parent enemy. The backdrop, HUD, full combat presentation, and capture-based
comparison remain open as later visual-parity steps.
