# Orbital Last Stand: the recreation plan

`docs/orbital-last-stand-audit.md` asked whether the reference game could be
built in Sindri as Sindri exists. The answer was no. This document is the plan
for making the answer yes, and the record of what that costs.

The rule that governs every line of it: **the game is authored through the
editor and Decay, and every gap that blocks it is closed as a general Sindri
capability.** Nothing here may add a survivor-like concept to the engine. No
wave system, no XP gems, no modules, no bosses. What lands is spawning,
queries, pointer input, collision access, a UI slice, randomness, saves, an
effect path, a complete Play loop, and an export — each of which an unrelated
tower-defense, puzzle, or simulation project would reach for.

If the game ever needs a private engine shortcut, the public authoring surface
is still incomplete and the shortcut is the wrong fix.

## Where the game lives

`games/orbital-last-stand/`, a bounded project consuming public Sindri
surfaces only. It is the second vertical-slice acceptance project the audit
recommends; Gather remains the continuity game for isometric, grid, and editor
work.

## What "the reference game" means

The reference is [MadsenDev/tester-repo](https://github.com/MadsenDev/tester-repo),
and it is the target — not the summary in the audit, which describes an earlier
and smaller game. The reference is roughly 20,000 lines across 66 JavaScript
modules and carries, beyond the combat loop the audit describes: expeditions
with generated rooms, companions with their own physics, a synergy catalog,
special and arena modules, hazards, sector routes, ships, manifestations, the
black signal contract system, an evolution guide, an archive, a debrief, a
dev mode, and offline delivery.

The work is staged: the combat core first, proven against the audit's
twelve-point acceptance test, then the meta systems layered on until parity.
Staging the *game* does not stage the *rule* — a capability added for the core
is general when it lands, not general later.

## The capability slices

Ordered by the earliest point at which a genuinely playable slice becomes
possible, which is the audit's order.

| # | Capability | Why the game is blocked without it |
|---|---|---|
| 1 | Reusable spawn definitions + `World.spawn` | The first enemy cannot be created |
| 2 | Bounded entity queries + Decay collections | Nothing can find the enemies to shoot |
| 3 | Pointer and touch input | The game is mouse- and touch-first |
| 4 | Typed 2D collision access and events | Every hit in the game |
| 5 | Dynamic text + interactive screen UI | Title, HUD, upgrade choice, every menu |
| 6 | Host-owned seeded randomness | Waves, elites, module offers, drops |
| 7 | Game persistence boundary | Settings, stats, unlocks, core upgrades |
| 8 | Measured particle/effect path | Thousands of short-lived visuals |
| 9 | Complete editor Play application loop | Playtesting the actual game |
| 10 | Static-web project export | The game ships as a web build |

Further gaps will surface once Decay is actually being written against a real
game. They are closed the same way and recorded here.

### 1. Reusable spawn definitions

A **prefab** is an authored reusable entity definition stored as a project
asset. It reuses the scene document's machinery — the same entity shape, the
same component payloads, the same versioning, migration, canonical
serialization, and validation — with one added rule: a prefab has exactly one
root. A second document format for "a subtree of entities" would be a copy of
the first that drifts from it.

`World.spawn(prefab)` returns a generation-checked `Entity`. Overrides are
ordinary writes through that reference — the transform paths a script already
has — rather than JSON escaping into Decay. A spawned entity's script starts
within the same frame, so a bullet spawned during an update moves during that
update.

### 2. Queries and collections

Decay gains a bounded, typed list value and the world gains a query surface
over authored tags and component types. Deterministic order, explicit limits,
and defined behaviour when an entity dies during iteration. This is the
collection work the Decay roadmap already describes, not an enemy list.

### 3–10

Designed when reached, against the audit's stated requirements for each. Every
one updates `docs/function-matrix.md`, `docs/capabilities.md`,
`docs/feature-integration-matrix.md`, its own subsystem contract, and
`CHANGELOG.md` in the same change, and exercises the capability in Gather where
Gather can reasonably reach it.

## The acceptance test

The audit's twelve points, unchanged, and passed without game-specific engine
patches. Parity work continues after they pass; the twelve points are the gate
that says the authoring surface is real, not the finish line for the game.

## Keeping the audit honest

`docs/orbital-last-stand-audit.md` records a status per requirement against a
named commit. Every slice here updates the statuses it changes, in the same
commit that changes them. An audit that still says **Missing** about something
that shipped is worse than no audit.
