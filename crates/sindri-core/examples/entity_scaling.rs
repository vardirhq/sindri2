//! Measures what the world costs at 1k, 10k, and 100k entities.
//!
//! `ROADMAP.md` gates the question of an archetype ECS on this: the point is not
//! to produce a number to be proud of, it is to find out whether the simple
//! slot-and-map world stops being adequate before there is a reason to replace
//! it. Every phase below is one a real frame or a real save already performs.
//!
//! Run it in release, because a debug build measures `serde_json` and bounds
//! checks rather than the design:
//!
//! ```bash
//! cargo run --release -p sindri-core --example entity_scaling
//! ```

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sindri_core::{
    ComponentSchemaRegistry, EntityData, SceneComponent, SceneDocument, SceneEntityId, Transform3D,
    World,
};

/// A component with a payload worth deserializing, so the typed query is
/// measured doing real work rather than reading an empty object.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Body {
    velocity: [f32; 3],
    mass: f32,
    awake: bool,
}

impl SceneComponent for Body {
    const TYPE_NAME: &'static str = "bench.body";
}

/// Entity counts the roadmap asks about.
const SIZES: [usize; 3] = [1_000, 10_000, 100_000];

/// Each phase runs this many times and reports its fastest run.
///
/// The fastest is the one least disturbed by whatever else the machine was
/// doing, which is what makes two runs of this comparable at all.
const ROUNDS: u32 = 3;

fn body_payload(index: usize) -> serde_json::Value {
    let index = u32::try_from(index % 1_000).unwrap_or(0);
    let offset = f64::from(index) / 1_000.0;
    serde_json::json!({
        "velocity": [offset, 1.0 - offset, 0.5],
        "mass": 1.0 + offset,
        "awake": index % 2 == 0,
    })
}

/// A world of `count` entities, each carrying a transform and one component,
/// parented in chains of ten so hierarchy work has something to walk.
fn populated(count: usize) -> World {
    let mut world = World::default();
    let mut previous = None;
    for index in 0..count {
        let mut data = EntityData {
            source_id: Some(
                SceneEntityId::new(format!("entity-{index:06}")).expect("generated IDs are valid"),
            ),
            transform_3d: Some(Transform3D::default()),
            ..EntityData::default()
        };
        data.components
            .insert(Body::TYPE_NAME.to_owned(), body_payload(index));
        let entity = world.spawn(data);

        // Chains rather than one enormous fan-out: a scene is mostly shallow,
        // and a single parent with 100k children is not a shape anyone authors.
        if index % 10 != 0
            && let Some(parent) = previous
        {
            world
                .set_parent(entity, Some(parent))
                .expect("a fresh chain cannot cycle");
        }
        previous = Some(entity);
    }
    world
}

fn fastest(rounds: u32, mut phase: impl FnMut() -> Duration) -> Duration {
    (0..rounds).map(|_| phase()).min().unwrap_or(Duration::ZERO)
}

fn time(work: impl FnOnce()) -> Duration {
    let start = Instant::now();
    work();
    start.elapsed()
}

/// Prints one row: total time, and what that is per entity.
fn report(label: &str, count: usize, elapsed: Duration) {
    let total_micros = elapsed.as_secs_f64() * 1_000_000.0;
    // Entity counts here are thousands, so a `u32` conversion is exact and
    // avoids a lossy cast the lints would rightly complain about.
    let count = f64::from(u32::try_from(count).unwrap_or(u32::MAX));
    let per_entity_nanos = elapsed.as_secs_f64() * 1_000_000_000.0 / count;
    println!("  {label:<22} {total_micros:>10.0} us   {per_entity_nanos:>8.0} ns/entity");
}

fn main() {
    let mut components = ComponentSchemaRegistry::default();
    components
        .register::<Body>("Body")
        .expect("the benchmark component registers");

    for count in SIZES {
        println!("\n{count} entities");
        let world = measure_world(&components, count);
        measure_scene(&world, count);
    }
}

/// The phases a running frame performs: building a world, walking it, and
/// reading typed components out of it.
fn measure_world(components: &ComponentSchemaRegistry, count: usize) -> World {
    report(
        "spawn",
        count,
        fastest(ROUNDS, || {
            time(|| {
                let world = populated(count);
                std::hint::black_box(&world);
            })
        }),
    );

    let world = populated(count);

    report(
        "iterate",
        count,
        fastest(ROUNDS, || {
            time(|| {
                let carrying = world
                    .entities()
                    .filter(|(_, data)| data.components.contains_key(Body::TYPE_NAME))
                    .count();
                std::hint::black_box(carrying);
            })
        }),
    );

    report(
        "typed query",
        count,
        fastest(ROUNDS, || {
            time(|| {
                let bodies = components
                    .query::<Body>(&world)
                    .expect("every payload is valid");
                std::hint::black_box(bodies);
            })
        }),
    );

    report(
        "despawn all",
        count,
        fastest(ROUNDS, || {
            let mut world = populated(count);
            let roots: Vec<_> = world
                .entities()
                .filter(|(_, data)| data.parent.is_none())
                .map(|(entity, _)| entity)
                .collect();
            time(|| {
                for root in roots {
                    world
                        .despawn_recursive(root)
                        .expect("a root spawned above is still alive");
                }
            })
        }),
    );

    world
}

/// The phases a save and a load perform, in both directions.
fn measure_scene(world: &World, count: usize) {
    report(
        "world -> scene",
        count,
        fastest(ROUNDS, || {
            time(|| {
                let document = world.to_scene().expect("every entity has a stable ID");
                std::hint::black_box(document);
            })
        }),
    );

    let document = world.to_scene().expect("every entity has a stable ID");

    report(
        "scene -> json",
        count,
        fastest(ROUNDS, || {
            time(|| {
                let text = document
                    .to_canonical_json()
                    .expect("a populated scene serializes");
                std::hint::black_box(text);
            })
        }),
    );

    let text = document
        .to_canonical_json()
        .expect("a populated scene serializes");
    println!("  {:<22} {:>10} bytes", "scene size", text.len());

    report(
        "json -> scene",
        count,
        fastest(ROUNDS, || {
            time(|| {
                let parsed = SceneDocument::from_json(&text).expect("canonical JSON parses");
                std::hint::black_box(parsed);
            })
        }),
    );

    report(
        "scene -> world",
        count,
        fastest(ROUNDS, || {
            time(|| {
                let loaded = World::from_scene(&document).expect("a saved scene loads");
                std::hint::black_box(loaded);
            })
        }),
    );
}
