# Physics architecture

Status: accepted implementation direction for the physics feature track.

Sindri exposes Sindri physics. Rapier is a backend implementation detail and is
never part of the scene format, editor contract, Decay language surface, or game
API.

## Goals

- Support both 2D and 3D without designing a 2D API that later has to be broken.
- Keep Rapier out of `sindri-core`, serialized scene data, the editor, Decay, and
  Gather.
- Run physics on the engine's fixed simulation step, never on render delta.
- Keep runtime `EntityId` ownership authoritative; physics handles are private
  implementation details.
- Give 2D and 3D parallel concepts without pretending dimension-specific values
  are interchangeable.
- Keep native and `wasm32-unknown-unknown` on the same gameplay semantics.
- Evolve Gather with the 2D implementation so the subsystem is proven by a real
  game rather than only fixtures.

## Crate boundary

Create one `sindri-physics` crate at a real dependency boundary:

```text
sindri-core                 sindri-grid
     ^                           ^
     |                           |
sindri-physics -----------------+
     |
     +-- rapier2d
     +-- rapier3d

sindri-scene -> sindri-core + sindri-grid + sindri-render + sindri-physics
sindri-decay -> sindri-core + sindri-grid + sindri-platform + sindri-physics
editor       -> ... + sindri-physics
Gather       -> ... + sindri-physics
```

`sindri-physics` owns the public physics model and the Rapier adapters. A second
`*-rapier` crate is not justified while there is one backend. Rapier types may
appear inside private implementation modules only. If another backend ever
becomes real, the adapter can be split without changing the public contract.

The crate depends on `sindri-core` because bodies are synchronized to runtime
entities, and on `sindri-grid` only if a concrete physics/navigation integration
requires it. Do not add the grid dependency speculatively. It has no renderer,
window, browser, editor, or Decay-language dependency.

Both `rapier2d` and `rapier3d` must satisfy the repository MSRV, licence policy,
`cargo deny`, native builds, and `wasm32-unknown-unknown` before either is merged.

## Public model

The concepts are intentionally parallel rather than generic over dimensionality.
Dimension-neutral configuration can be shared internally, but public APIs use
ordinary explicit types.

### Bodies

`RigidBodyKind` is shared:

- `Static`
- `Dynamic`
- `KinematicPosition`
- `KinematicVelocity`

The scene components are distinct:

- `sindri.rigid_body_2d`
- `sindri.rigid_body_3d`

They carry Sindri-owned configuration such as body kind, gravity scale, damping,
and whether rotation is locked. They never serialize Rapier handles, activation
state, solver state, or backend-specific flags.

### Colliders

The scene components are distinct:

- `sindri.collider_2d`
- `sindri.collider_3d`

The first 2D shapes are box, circle, and capsule. The first 3D shapes are box,
sphere, and capsule. Shape enums are Sindri types. Dimensions are expressed in
scene/world units and validated as finite and positive.

A collider carries:

- shape
- local offset
- sensor/trigger flag
- collision membership mask
- collision filter mask
- friction
- restitution

Collision masks are Sindri bit masks from the first implementation. They are not
Rapier `Group` values in public APIs or serialized JSON.

A collider may exist without a rigid-body component; it is treated as a static
collider owned by the entity. An entity may initially have one authored collider
per dimension. Compound/multiple colliders are deferred until a game proves the
need rather than forcing a collection-shaped scene schema immediately.

An entity may not participate in both the 2D and 3D physics worlds at once.
Validation reports that as an authored configuration error instead of choosing a
world silently.

## Transform ownership

`Transform3D` remains the authored and visible transform for both dimensions.
Physics does not introduce a second scene transform.

For 2D, physics reads/writes X, Y, and rotation about Z. Existing Z and 3D scale
are preserved. For 3D it reads/writes XYZ and quaternion-equivalent scene
rotation through the existing transform representation.

Synchronization rules are explicit:

- static body: world transform seeds physics; checked authored changes update the
  backend body/collider before the next step;
- dynamic body: physics owns position/rotation while simulation runs and writes
  the resulting transform back after each fixed step;
- kinematic-position body: gameplay/editor supplies the target transform before
  the fixed step; physics resolves contacts from that motion;
- kinematic-velocity body: gameplay supplies velocity; physics owns the resulting
  transform for the step.

Scale is not simulated. Collider dimensions are authored explicitly. Changing an
entity's visual scale must not secretly mutate collision geometry.

Parented dynamic rigid bodies are rejected initially. A physics body has a world
pose while a child transform is parent-relative, and silently mixing those
models produces surprising motion. Static and kinematic authored children may be
supported only once their synchronization semantics are tested. The first slice
keeps the rule narrow and explicit.

## Runtime ownership and stepping

`PhysicsWorld` is runtime state beside `World`, not serialized state inside it.
It owns separate private 2D and 3D backend worlds and maps `EntityId` to private
backend handles. Despawning an entity removes its body/collider before the next
step; generation-checked IDs prevent a reused slot from inheriting old physics
state.

The host advances physics exactly once for each engine fixed update using that
fixed step duration. Render frames never step Rapier directly. Pause therefore
pauses physics automatically, and time-scale/fixed-step semantics remain engine
semantics rather than backend semantics.

The fixed-update order for the first slice is:

1. apply deferred/checkable gameplay writes;
2. synchronize static/kinematic authored state into physics;
3. step physics with the engine fixed `dt`;
4. synchronize dynamic/velocity-driven results back to `World`;
5. publish normalized collision/sensor events;
6. run consumers that are defined to observe post-physics events.

The exact system-ordering API is still an open roadmap item, so physics must not
quietly invent a general scheduler. Its ordering is documented as part of the
engine fixed-step contract until that scheduler exists.

## Events and queries

Backend events are normalized to Sindri events containing `EntityId`s and Sindri
collider semantics only:

- collision started / stopped
- sensor entered / exited

Contact manifolds and raw solver data are deliberately not first-slice public
API. Add them only for a demonstrated gameplay requirement.

Queries are explicit by dimension:

- 2D ray cast and overlap queries
- 3D ray cast and overlap queries

Results contain Sindri entity IDs, hit position/normal in the appropriate vector
dimension, and distance. They never expose Rapier collider handles.

## Editor

The inspector authors the Sindri body/collider components through the existing
checked command and undo/redo path. It must provide:

- body kind and shared physical properties;
- dimension-appropriate shape and dimensions;
- sensor toggle;
- collision membership/filter masks;
- friction and restitution;
- validation errors in the inspector rather than backend panics.

The Scene view should draw simple collider gizmos before the Editor column earns
✅. Merely serializing a collider or showing a dropdown remains partial.

The editor never links against or imports Rapier types directly; it consumes the
public `sindri-physics` model/schema.

## Decay

Decay exposes typed Sindri physics operations, not Rapier terminology. The 2D
surface comes with the first vertical slice; 3D names are reserved by the design
but are not claimed implemented until the 3D slice exists.

Initial 2D gameplay operations should cover:

- get/set linear velocity;
- apply impulse;
- sensor/collision event observation;
- 2D ray/overlap query when Gather or another real game needs one.

Dimension is explicit in names/types where ambiguity would otherwise exist. The
language workspace remains independent: `decay/` gains no Sindri dependency;
`sindri-decay` performs all conversion and host calls.

## Gather proof

The first 2D physics track is incomplete until Gather visibly depends on it.
Gather should evolve so that:

- authored walls/obstacles have static colliders;
- the player has a collision-constrained body/collider rather than walking
  through those obstacles;
- pickups use sensor enter events instead of a hand-written distance check;
- at least one hazard interaction uses collision or sensor events;
- native and browser Gather exercise the same physics semantics.

Pathfinding remains navigation, not physics. The Wisp may plan through the grid,
but collision remains authoritative for physical overlap. Do not couple Rapier
into `sindri-grid` to make those systems agree implicitly.

## 3D parity

The architecture is not considered 3D physics support merely because
`rapier3d` is a dependency. The later 3D vertical slice must exercise bodies,
colliders, synchronization, editor authoring, and a real 3D proof before its
matrix cells become green.

The 2D implementation must avoid assumptions that block that slice: no common
API whose vectors are always `Vec2`, no scalar-only neutral rotation API, no
2D-only collision-event representation, and no backend handle in shared scene or
script contracts.

## Feature-track slices

Implement this in reviewable PRs rather than one giant patch:

1. **Physics foundation.** Add `sindri-physics`, dependency-policy changes,
   Sindri-owned 2D/3D public model, backend masking, fixed-step 2D runtime,
   collision layers/events, native + WASM tests. Do not mark Editor or Script
   complete.
2. **2D authoring.** Register/serialize the 2D scene components, inspector
   controls, checked edits, undo/redo, and collider gizmos.
3. **Decay 2D.** Add typed velocity/impulse/event access through `sindri-decay`
   with language/host contract tests.
4. **Gather physics.** Convert obstacles, pickups, player movement, and one hazard
   interaction to the real physics path; prove native/browser behavior and update
   capability docs/matrices.
5. **3D physics.** Activate the parallel 3D runtime and scene/editor/Decay
   surfaces only when a real 3D proof can exercise them end to end.

Each PR runs all checks relevant to its dependency and target surface. A slice
that introduces Rapier must run `cargo deny`; a slice touching browser-reachable
physics must compile WASM; the Gather slice must run the real browser smoke test.

## Non-goals for the first 2D slice

- joints
- continuous-character-controller abstraction
- compound/multiple authored colliders
- contact-manifold scripting
- physics-driven visual scale
- platformer navigation
- backend selection/plugin API
- deterministic cross-platform floating-point lockstep

Those are not forbidden forever. They are deliberately absent until a real game
needs them.
