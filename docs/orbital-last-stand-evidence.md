# Orbital Last Stand: what a ten-minute run costs

`docs/orbital-last-stand-audit.md` asks, as the twelfth of its twelve
acceptance points, for frame-time, entity-count, query, collision, and
allocation evidence from a late-run stress point — and says the workload should
be comparable to the reference game's, so that a quiet demo cannot satisfy a
high-churn criterion.

This is that evidence. Reproduce it with:

```bash
cargo test --release -p orbital-last-stand --test the_ten_minute_run -- --ignored --nocapture
```

## The run

Ten simulated minutes at a fixed 60 Hz — 36,000 steps — played by the harness
in `games/orbital-last-stand/src/lib.rs`, which assembles the same public
pieces a host assembles and runs them in the same order. Every upgrade offered
is taken. The ship's hull is topped up each step: what is being measured is
whether the engine carries the workload, not whether this balance is beatable.

| | |
| --- | --- |
| Steps | 36,000 |
| Kills | 26,072 |
| Waves | 206 |
| Upgrades offered and taken | 17 |
| Peak live entities | 228 |
| Peak live flecks | 1,690 |
| **Mean step** | **2.997 ms** |
| **Worst step** | **14.469 ms** |

Measured on the CI-class Linux machine this was developed on, in a release
build. The absolute milliseconds are that machine's; the ratio to the budget is
the number that means something.

## What it says

**A fixed step is 16.67 ms.** The mean step is 18% of it, and the worst single
step of thirty-six thousand stays inside it. The simulation keeps up with
itself with room to spare, which is the only threshold here that is a fact
rather than a property of this machine — and it is the one the test asserts.

**The churn is the reference's or more.** The reference game has produced runs
above 2,500 kills with dense projectile and pickup populations. This run kills
26,072 things, each of which was spawned, steered, collided, and despawned,
and each of which dropped a core that was spawned, pulled, collected, and
despawned in turn. Peak concurrent population is 228 entities and 1,690 flecks.

**Every kill is a query.** The ship asks `World.with_tag("enemy")` once a frame
to find what to shoot at — a walk of every entity in the world, 36,000 times —
and each enemy asks `World.find("Player")` once a frame to steer at it. That is
the shape `docs/scripting.md` warns is quadratic if asked per bullet, asked
once per frame instead, and it does not show up as a cost at this population.

**Every hit is a collision event.** Damage is `Physics.sensor_entered()` on
both halves: the shot counts what it has hit so piercing works, and the enemy
takes the damage. Nothing keeps a list of what it has already touched, because
`started, not touching` means it does not have to.

**Allocation is bounded by design rather than by measurement.** A pass may
create 4,096 entities and a fleck pool is fixed; the run never approached
either. The one number worth watching is that flecks are values rather than
entities — `docs/effect-scaling.md` measured 8,000 flecks-as-entities at
5.25 ms a frame against 0.018 ms as values, and the 1,690 flecks above are the
second of those.

## Where the time goes

The worst step is roughly five times the mean, and both are well inside the
budget. The spikes line up with the largest waves — a wave is up to 80 spawns
in one pass, each of which builds a Decay instance and runs its field
initializers, and the physics bodies for all of them are built by the next
synchronize.

Nothing here has been optimised. The point of the measurement is that it did
not need to be.

## What this does not measure

- **Rendering.** The harness runs the simulation, not a GPU. Frame time in a
  real build includes drawing 228 sprites and 1,690 flecks, which
  `docs/effect-scaling.md` covers for the flecks and `docs/entity-scaling.md`
  for the sprites.
- **The browser.** These are native release numbers. WebAssembly is slower, and
  by how much is not something this test can say.
- **A real player.** The harness takes the first upgrade offered and never
  dodges. A run that moved would put the ship somewhere else and the crowd
  somewhere else with it.
