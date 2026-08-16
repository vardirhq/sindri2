# Entity scaling

`ROADMAP.md` gates the question of an archetype ECS on measuring 1k, 10k, and
100k entity workloads. This is that measurement, and what it decided.

```bash
cargo run --release -p sindri-core --example entity_scaling
```

Release matters: a debug build measures `serde_json` and bounds checks rather
than the design. Each phase runs three times and reports its fastest run, which
is the one least disturbed by whatever else the machine was doing.

## What it measures

Every phase is one a real frame or a real save already performs. Entities carry
a `Transform3D` and one JSON component with a payload worth deserializing, and
they are parented in chains of ten, because a scene is mostly shallow and a
single parent with 100k children is not a shape anyone authors.

| Phase | What it stands for |
| --- | --- |
| spawn | building a world |
| iterate | the scan extraction performs every frame |
| typed query | `ComponentSchemaRegistry::query`, which extraction uses |
| despawn all | tearing a world down |
| world → scene | the first half of a save |
| scene → json | canonical serialization, the second half of a save |
| json → scene | parsing, the first half of a load |
| scene → world | the second half of a load |

## What it found

Validation was quadratic in the size of the scene, and every load, save, and
canonical serialization paid it. Each entity walked its own ancestor chain, and
each step of that walk searched the whole entity list for the parent it named.

Measured on the CI-class Linux machine this was developed on:

| Phase (10k entities) | Before | After |
| --- | ---: | ---: |
| world → scene | 1464 ms | 9.9 ms |
| scene → json | 1424 ms | 44 ms |
| json → scene | 1397 ms | 17 ms |
| scene → world | 1442 ms | 13 ms |

At 100k the same phases did not finish in four minutes; they now complete in
150 ms, 455 ms, 272 ms, and 297 ms. Parents resolve through a map, and an entity
proven to reach a root is remembered, so a chain shared by many entities is
walked once rather than once per descendant.

That is worth stating plainly: **the benchmark's first result was a bug, not a
number.** Nothing else would have found it. Every scene in the repository is
small enough that a quadratic validator looks instant, and the property only
becomes visible at a size nobody had tried.

## Where the time goes now

Per-entity costs at 100k, after the fix:

| Phase | ns/entity |
| --- | ---: |
| iterate | 55 |
| typed query | 327 |
| despawn all | 668 |
| spawn | 1 367 |
| world → scene | 1 483 |
| scene → world | 2 968 |
| json → scene | 2 724 |
| scene → json | 4 549 |

Iteration is the frame path, and at 55 ns per entity a 100k-entity world scans
in about five milliseconds. The typed query costs six times that because it
clones and deserializes each component payload from JSON, which is the obvious
next thing to improve if it ever matters — component storage, not entity
storage.

The scene phases dominate everything, and all of them are `serde_json` work on a
38 MB document. That is a save cost, not a frame cost, and it is paid when a
human presses a key.

## The decision

**No archetype ECS.** Nothing in these numbers points at entity storage. A frame
scans 100k entities in five milliseconds, which is not where a frame's budget
goes, and the two slowest paths are both JSON: deserializing component payloads,
and serializing documents. An archetype ECS addresses neither.

`PROJECT_OVERVIEW.md` asks for profiling before introducing complicated systems,
and this is what profiling says. Revisit when a real game — not a synthetic
world — spends measurable frame time in `World::entities`, or when component
queries run in a loop rather than once per frame.
