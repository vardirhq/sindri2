# Orbital Last Stand reference parity

This document records the current `MadsenDev/tester-repo` Last Stand (`campaign`) behavior that Sindri's `games/orbital-last-stand/` is trying to reproduce. It is a parity specification, not a claim that every item below is already implemented.

The reference was re-audited from the current `main` implementation in September 2026. Relevant source files are listed at the end so future work can verify behavior rather than rely on memory.

## Campaign contract

- Mode id: `campaign`, displayed as **LAST STAND**.
- Objective: survive to **10:00**.
- Hard run limit: **600 seconds**.
- Regular enemies are continuous pressure, not numbered waves.
- Boss opportunity interval: **60 seconds**.
- Normal boss encounters clear the arena and suppress ordinary enemy spawning until the boss is dead.
- Gameplay pauses for level-up choices, route choices and Black Signal choices, so those decisions do not consume run time.

At 600 seconds the reference clamps time to 600, updates the UI and finishes the run as a victory before running the rest of that frame's gameplay.

## Ten-minute timeline

| Time | Change |
| --- | --- |
| 0:00 | Scout available; Outer Drift; first regular spawn can happen immediately |
| 0:35 | Brute unlocks |
| 0:42 | First random event becomes due |
| 1:00 | Boss opportunity #1 |
| 1:10 | Dart unlocks |
| 2:00 | Bulwark unlocks; Ember Belt; route choice; boss opportunity #2 |
| 2:50 | Wisp unlocks |
| 3:00 | Boss opportunity #3; regular-enemy HP scaling starts |
| 3:10 | Anchor unlocks |
| 3:50 | Spitter unlocks |
| 4:00 | Violet Wake; route choice; boss opportunity #4; boss tier 2 unlocks |
| 4:45 | Swarm unlocks |
| 5:00 | Boss opportunity #5 |
| 5:10 | Relay unlocks |
| 5:35 | Sniper unlocks |
| 6:00 | Null Lattice; route choice; boss opportunity #6 |
| 6:45 | Orbiter unlocks |
| 7:00 | Boss opportunity #7 |
| 7:10 | Burrower unlocks |
| 7:35 | Leech unlocks |
| 8:00 | Core Approach; route choice; boss opportunity #8; boss tier 3 unlocks |
| 8:25 | Sentinel unlocks |
| 9:00 | Boss opportunity #9 |
| 9:05 | Phaser unlocks; full 15-enemy roster is available |
| 9:20 | No new random event may start from this point onward |
| 10:00 | Victory |

A scheduled boss does not stack on top of a living boss. If the previous boss survives past the next scheduled timestamp, the overdue boss waits until the arena is boss-free.

## Continuous spawn pressure

Reference spawn rate:

```text
rate = max(0.085, 0.7 - time * 0.00092)
       / (difficulty.spawn * sector.pressure * spawnPressure(mode, time) * modifiers.spawn)
```

`spawnTimer` starts at zero, so a regular enemy may spawn on the first gameplay update. The runtime subtracts `dt` and uses a `while spawnTimer <= 0` catch-up loop, adding `rate` after every enemy spawned.

Normal difficulty uses `difficulty.spawn = 1.12`.

Neutral Normal intervals, before event/route modifiers and while no boss suppresses regulars, are approximately:

| Time | Sector pressure | Interval |
| --- | ---: | ---: |
| 0:00 | 1.00 | 0.625 s |
| 1:00 | 1.00 | 0.576 s |
| 2:00 | 1.07 | 0.492 s |
| 3:00 | 1.07 | 0.446 s |
| 4:00 | 1.12 | 0.382 s |
| 5:00 | 1.12 | 0.338 s |
| 6:00 | 1.18 | 0.279 s |
| 7:00 | 1.18 | 0.237 s |
| 8:00 | 1.27 | 0.182 s |
| 9:00 | 1.27 | 0.143 s |
| 10:00 | 1.27 | about 0.104 s |

## Sectors

| Time | Sector | Pressure | Main hazard |
| --- | --- | ---: | --- |
| 0:00-2:00 | Outer Drift | 1.00 | seven destructible drifting asteroids |
| 2:00-4:00 | Ember Belt | 1.07 | periodic damaging flare zones at top/bottom edges |
| 4:00-6:00 | Violet Wake | 1.12 | three gravity wells bending friendly and hostile projectiles |
| 6:00-8:00 | Null Lattice | 1.18 | moving null field disables player weapons while inside |
| 8:00-10:00 | Core Approach | 1.27 | alternating telegraphed horizontal/vertical beam hazards |

Hazard reference details:

- Outer Drift starts with 7 tier-2 asteroids. Tier-2 HP 90 and radius 30-44; tier-1 HP 38; tier-0 HP 12. Large asteroids split into 2-3 medium pieces, medium pieces into 2 small pieces.
- Ember flare intensity is `(sin(time * 0.72) + 1) / 2`. It becomes dangerous above 0.82. Player raw damage in the dangerous edge is 54/sec; enemies take 12 HP/sec.
- Violet Wake creates 3 gravity wells with radius 80-115. Projectiles within `radius * 1.8` are pulled with force up to 115.
- Null Lattice creates one roaming field with radius roughly 62-86 and movement capped at 31. Inside it, `player.nullified` disables ordinary and special firing.
- Core Approach queues a beam every 3.1 s: 1.05 s warning, 0.55 s active, 18 px half-width. Player raw damage is 90/sec inside it; enemies take 34 HP/sec.

## Enemy roster

Unlocked archetypes are selected uniformly from the full currently available pool.

| Unlock | Kind | HP | Speed | Contact | Value | Behavior |
| ---: | --- | ---: | ---: | ---: | ---: | --- |
| 0 | Scout | 20 | 62 | 8 | 8 | chase |
| 35 | Brute | 52 | 42 | 15 | 18 | chase |
| 70 | Dart | 24 | 68 | 10 | 14 | charger |
| 120 | Bulwark | 96 | 34 | 20 | 30 | bulwark |
| 170 | Wisp | 48 | 78 | 12 | 24 | strafe |
| 190 | Anchor | 118 | 36 | 16 | 48 | anchor |
| 230 | Spitter | 64 | 52 | 14 | 32 | shooter |
| 285 | Swarm | 20 | 105 | 7 | 16 | swarm |
| 310 | Relay | 86 | 58 | 13 | 46 | relay |
| 335 | Sniper | 70 | 44 | 13 | 38 | sniper |
| 405 | Orbiter | 82 | 70 | 15 | 42 | orbiter |
| 430 | Burrower | 112 | 74 | 19 | 58 | burrower |
| 455 | Leech | 92 | 76 | 16 | 50 | leech |
| 505 | Sentinel | 142 | 38 | 18 | 62 | sentinel |
| 545 | Phaser | 98 | 84 | 17 | 58 | phaser |

Regular HP does not time-scale before 180 seconds. From 3:00 onward:

```text
hp = baseHp * (1 + max(0, time - 180) * 0.0014)
```

That is x1.084 at 4:00, x1.168 at 5:00, x1.252 at 6:00, x1.336 at 7:00, x1.420 at 8:00, x1.504 at 9:00 and x1.588 at 10:00.

## Elites

Elites begin only after 105 seconds.

```text
eliteChance = min(0.32, 0.025 + time / 3000 + eliteBonus * 0.65)
```

Without event/route elite bonus the chance is about 6.0% at 1:45, 10.5% at 4:00, 14.5% at 6:00, 18.5% at 8:00 and 22.5% at 10:00.

| Trait | HP | Damage | Speed | Value |
| --- | ---: | ---: | ---: | ---: |
| Armored | 2.35x | 1.05x | 0.88x | 2.20x |
| Frenzied | 1.55x | 1.45x | 1.38x | 2.10x |
| Volatile | 1.65x | 1.20x | 1.08x | 2.25x |
| Vampiric | 1.85x | 1.25x | 1.06x | 2.30x |
| Splitter | 1.70x | 1.12x | 1.12x | 2.25x |

## Boss director

Campaign boss tier by elapsed time:

- Tier 1 before 4:00: Warden, Harrower, Prism, Singularity.
- Tier 2 from 4:00: adds Crown, Brood, Mirror, Architect and Spine.
- Tier 3 from 8:00: adds Leviathan and Last Light.

The director excludes either of the two most recently selected bosses when a fresh eligible boss exists, then chooses randomly from the remaining eligible pool.

Normal boss profile:

- HP x1.16
- contact damage x1.08
- movement x1.08
- attack tempo x1.12
- projectile speed x1.08
- projectile damage x1.06
- base phase threshold 56%
- predictive aim lead 0.18 s
- arena cleared on boss entry
- ordinary enemies suppressed during the boss

Adaptive scaling estimates build strength from direct DPS, recent measured DPS, special weapons, companions, modules and durability. Previous boss kill performance also feeds future pressure. The runtime treats **18 seconds** as expected boss TTK.

Adaptive formulas:

```text
boss HP             *= (1 + pressure * 0.72) * (1 + performance * 0.95)
boss tempo          *= 1 + pressure * 0.16 + performance * 0.08
projectile speed    *= 1 + pressure * 0.07 + performance * 0.025
projectile damage   *= 1 + pressure * 0.025
phase threshold     += pressure * 0.12 + performance * 0.06
```

Final phase threshold is clamped to 0.25-0.80.

## Events

The first event is due at 42 seconds. After an event ends, another is scheduled 38-60 seconds later. No new event begins at or after 560 seconds.

| Event | Duration | Modifiers |
| --- | ---: | --- |
| System Overload | 14 s | fire interval x0.5; enemy speed x1.22 |
| Signal Harvest | 16 s | magnet x2.8 |
| Elite Hunt | 18 s | elite bonus +0.38; score x1.5 |
| Swarm Tide | 16 s | spawn x1.65; XP x1.55; score x1.25 |
| Glass Space | 14 s | incoming damage x1.75; score x2 |
| Temporal Echo | 13 s | fire interval x0.68; enemy speed x0.7; score x1.35 |

Event modifiers combine with the active route modifiers.

## Routes

Campaign pauses for a route choice whenever the 120-second leg number increases: 2:00, 4:00, 6:00 and 8:00.

Each offer contains one SAFE/BALANCED route, one DANGEROUS route and one VOLATILE route. The reward is permanent; the threat modifiers last until the next route choice.

Reference route examples relevant to pressure:

- Quiet Line: +8 max hull and repair 28% max hull; spawn x0.90; score x0.92.
- Salvage Current: permanent XP yield x1.12; magnet x1.7; spawn x1.16; score x1.15.
- Redline Gate: permanent fire interval x0.93; enemy speed x1.14; spawn x1.22.
- Hunter Array: permanent damage x1.09; elite bonus +0.24; spawn x1.08.
- Black Signal route: permanent damage x1.10 and XP x1.08; enemy speed x1.10; spawn x1.28.
- Swarm Nest: +1 projectile and damage x0.94; spawn x1.50; enemy speed x1.06.

There are 12 routes total in the current reference.

## XP and modules

Base player starts at level 1, XP 0, next XP 35.

After every level:

```text
nextXp = floor(nextXp * 1.28 + 8)
```

Sequence begins 35, 52, 74, 102, 138, 184, 243, 319, 416, 540...

Regular enemies drop one salvage gem worth their archetype value. Bosses drop 16 gems worth 24 each, for 384 base XP before multipliers.

Level-up module pool:

- level divisible by 7: companion / Foundry Signal
- otherwise level divisible by 5: boss relic
- otherwise: salvage

The current catalog contains 160 modules.

## Black Signal

After every second defeated boss, the player may receive a Black Signal offer. The run pauses and the player can accept one contract/module pairing or reject the connection.

Reference permanent prices/boons include:

- lose 18 max hull -> damage x1.12
- speed x0.82 -> fire interval x0.88
- incoming damage x1.22 -> +1 projectile

## Powerup economy

Normal base drop chances:

- regular enemy: 5.8%
- elite: 26%
- boss: 100%
- missing health adds up to 5.5 percentage points
- repair pity after 120 kills without a repair
- boss death below 62% player health forces the guaranteed boss drop to be a repair

Normal repair restores `round(28 + maxHp * 0.08)`.

Pulse clears hostile bullets and damages every enemy for `max(80, player.damage * 4)`.

Overdrive lasts 8 seconds and changes the ordinary firing interval to 55% of normal.

## Baseline player

Before ship modifiers:

- HP/max HP 100
- radius 11
- movement speed 245
- fire interval 0.42 s
- damage 18
- one shot
- no pierce
- projectile speed 520
- projectile radius 4
- magnet 110
- crit 5%
- armor 0
- regen 0

Auto-fire targets the nearest targetable enemy. Ordinary off-screen enemies are not targetable.

## Sindri parity status

Legend: `DONE`, `PARTIAL`, `TODO`, `INTENTIONAL`.

| System | Status | Notes |
| --- | --- | --- |
| Continuous regular spawning | DONE | Sindri no longer has numbered wave batches |
| Viewport combat rule | DONE | off-screen enemies may approach but do not attack/target; player does not target them |
| Regular HP post-3:00 curve | DONE | current authored enemies use the reference 0.0014 curve |
| 600-second victory | DONE | the timer clamps to 600 and ends the run before another gameplay pass |
| Exact Normal spawn formula | DONE | campaign pressure uses the reference base curve, Normal multiplier and active sector |
| Five sector pressure values | DONE | the Director publishes all five two-minute sectors and their pressure multipliers |
| Five sector hazards | DONE | asteroids, edge flares, gravity wells, null field and telegraphed core beams are authored |
| 15 enemy archetypes | DONE | the complete roster is selected uniformly from the unlocked pool |
| Exact enemy unlock timestamps | DONE | all fifteen reference timestamps drive the growing pool |
| Elite system | DONE | the post-105-second chance curve and all five stat traits are authored in Decay |
| 60-second boss cadence | DONE | overdue encounters wait for the living Warden rather than stacking |
| Tiered boss director | PARTIAL | tier-one rotation excludes the two most recent picks; later unlock tiers remain |
| Full boss roster | PARTIAL | Warden, Harrower, Prism and Singularity are authored; seven later bosses remain |
| Warden encounter | INTENTIONAL | Sindri version is a more elaborate three-phase interpretation, while targeting reference-like short TTK/readability |
| Route choices | TODO | missing |
| Random events | TODO | missing |
| Reference XP curve | TODO | Sindri currently uses a smaller core-count economy |
| 160-module catalog | PARTIAL | mechanism work exists; full catalog missing |
| Companion/foundry level cadence | TODO | missing |
| Black Signal | TODO | missing |
| Powerup/drop economy | DONE | Normal regular/elite/boss chances, missing-health bonus, repair pity, repair, pulse and overdrive are implemented; repair preserves the reference proportion on Sindri's normalized five-hull scale |
| Adaptive boss scaling | TODO | missing |

## Implementation order

Keep parity work reviewable. Do not attempt the remaining systems as one monolithic change.

1. Campaign skeleton: 600 s victory, exact Normal spawn formula, five sector pressure values, 60 s boss schedule, publish sector/boss state.
2. Complete the 15-enemy roster and exact unlock pool.
3. Sector hazards.
4. Elites and drop economy.
5. XP/module progression and level-specific pools.
6. Route choices and modifiers.
7. Random events.
8. Boss roster + tiered director + adaptive scaling.
9. Black Signal and remaining progression glue.
10. Final balance/performance pass against a full ten-minute acceptance run.

## Reference source map

Current `MadsenDev/tester-repo` files used for this audit:

- `src/modes.js`: run limit, boss interval, regular-enemy rules, boss-era spawn pressure.
- `src/game-runtime.js`: spawn formula, boss scheduling, victory check, XP collection and runtime ordering.
- `src/game.js`: initial timers/state, level-up and route/Black Signal pause flow.
- `src/entities.js`: archetype table, enemy scaling, elite chance/traits, targetability.
- `src/enemy-ai.js`: archetype behavior.
- `src/world.js`: sector boundaries and pressure.
- `src/hazards.js`: sector hazard mechanics.
- `src/events.js`: timed random events and modifiers.
- `src/sector-routes.js`: route cadence, choices and modifiers.
- `src/meta.js`: difficulty spawn/damage/score multipliers.
- `src/player-state.js`: base player stats and XP threshold.
- `src/combat-actions.js`: kill rewards, boss salvage and powerups.
- `src/drop-economy.js`: powerup probability, pity and repair values.
- `src/module-catalog.js`: 160-module catalog and level pool cadence.
- `src/black-signal.js`: every-second-boss offers and contracts.
- `src/bosses.js`: boss catalog and per-boss attacks.
- `src/boss-difficulty.js`: difficulty-specific boss tuning.
- `src/boss-runtime.js`: recent DPS, boss outcome pressure and expected 18 s TTK.
- `src/boss-director.js`: tier eligibility, no-repeat selection and adaptive build scaling.
- `src/boss-counterplay.js`: phase protection and boss counterplay mechanics.
