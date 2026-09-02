# Effect scaling

`docs/orbital-last-stand-audit.md` asks for a pooled effect path and says to
**measure both approaches before choosing**. This is that measurement, and what
it decided.

```bash
cargo run --release -p sindri-scene --example effect_scaling
```

Release matters. A debug build measures `serde_json` and bounds checks rather
than the design, and which design costs more is the entire question.

## What it measures

A visual fleck — one spark from a hit, one mote of a trail — two ways:

- **entities**: an entity carrying a `Transform3D` and a `sindri.sprite`
  payload, moved each frame and drawn through the ordinary scene extraction.
- **pooled**: the same population as plain values in one `Vec`, moved in one
  pass and turned straight into what a renderer draws.

Both run 120 frames at each density, with the whole population replaced every
30 frames — a fleck that lived for ever would measure the steady state and miss
churn, which is most of what an effect system does.

Densities are 500, 2,000 and 8,000 live flecks. The reference game runs above
2,500 kills with dense projectile and pickup populations; a few hundred flecks
is ordinary, a couple of thousand is a bad moment during a boss, and eight
thousand is the number that decides whether an approach has headroom.

## What it found

Measured on the CI-class Linux machine this was developed on. "Budget" is the
share of one 16.7 ms frame at 60 Hz.

| Flecks | Approach | emit | update | extract | retire | per frame | budget |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 500 | entities | 1.0 ms | 0.1 ms | 22.8 ms | 0.3 ms | 0.201 ms | 1.2% |
| 500 | pooled | 0.0 ms | 0.1 ms | 0.0 ms | 0.0 ms | 0.001 ms | 0.0% |
| 2,000 | entities | 3.8 ms | 1.3 ms | 119.3 ms | 1.4 ms | 1.048 ms | 6.3% |
| 2,000 | pooled | 0.0 ms | 0.3 ms | 0.2 ms | 0.0 ms | 0.005 ms | 0.0% |
| 8,000 | entities | 16.6 ms | 6.7 ms | 598.4 ms | 8.3 ms | 5.250 ms | 31.5% |
| 8,000 | pooled | 0.1 ms | 1.3 ms | 0.8 ms | 0.0 ms | 0.018 ms | 0.1% |

Phase columns are totals across all 120 frames; "per frame" is the whole loop
divided by them.

These vary by a few percent between runs — a repeat put the entity path at
5.60 ms and the query at 2.35 ms. The conclusion does not depend on that
precision, and would not change if every number moved by half.

**Extraction is where the entity path goes.** It is 95% of the cost at every
density — not spawning, not despawning, not moving. And of that 5.25 ms a frame
at 8,000, **2.68 ms is the typed query alone**: `serde_json` turning each
entity's stored payload back into a `SpriteComponent`, once per entity, every
frame, with no transforms computed, nothing batched and nothing drawn.

## What it decided

**Flecks are pooled.** `Effects2d` holds them as plain values, and
`sindri.effect.burst` authors what a burst looks like.

The entity path is not unusable — 500 flecks costs 1.2% of a frame, which is
fine. It is *unaffordable at the density this game needs*: 8,000 flecks is a
third of the budget before the game has done anything else, and a game that has
to count its sparks is one whose effects are a balance problem rather than an
art decision.

Fixing the payload re-parse would not have changed the answer. Even a perfect
typed cache leaves the other 2.6 ms — per-entity iteration, transform matrices,
ordering, batching — which is still two orders of magnitude above 0.018 ms. The
pooled path skips all of it because there is nothing to look up.

**What a fleck gives up is everything an entity is for**: no identity a script
can hold, no components, no place in the hierarchy, nothing to collide with.
That is the trade, and it is only worth making because the measurement says the
alternative costs a third of a frame.

## What this did not decide

**The payload re-parse is worth fixing on its own.** 2.68 ms to re-read 8,000
sprite payloads is a cost every ordinary sprite pays too, and a game with 8,000
*real* entities would pay it with no pool to escape into. That is a change to
the component model with a real invalidation problem — scripts write payloads
directly — so it is recorded here rather than attempted alongside an effect
system.

**No instanced primitive renderer, and no custom materials.** The audit lists
both as things to consider. Flecks batch with ordinary sprites by layer and
texture today, which is one draw call for a burst; neither of the other two has
a consumer yet, and the acceptance target is readable high-volume combat rather
than reproducing Canvas 2D call for call.
