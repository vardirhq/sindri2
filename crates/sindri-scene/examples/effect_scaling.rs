//! Measuring what a visual fleck costs, two ways.
//!
//! `docs/orbital-last-stand-audit.md` asks for a pooled effect path *and* says
//! to measure both approaches before choosing one. This is that measurement.
//! Nothing here decides anything on its own — `docs/effect-scaling.md` records
//! what it found and what was built as a result.
//!
//! ```bash
//! cargo run --release -p sindri-scene --example effect_scaling
//! ```
//!
//! Release matters. A debug build measures `serde_json` and bounds checks
//! rather than the design, and the whole question here is which design costs
//! more.

use std::time::{Duration, Instant};

use serde_json::json;
use sindri_core::{EntityData, EntityId, SceneComponent, Transform3D, World};
use sindri_render::Viewport;
use sindri_scene::{CameraView, SceneExtractor, SpriteComponent, TextureBindings};

/// Live flecks to hold, from a busy moment to an unreasonable one.
///
/// The reference game runs above 2,500 kills with dense projectile and pickup
/// populations. A hit throws off a handful of flecks that live about half a
/// second, so a few hundred is ordinary, a couple of thousand is a bad moment
/// during a boss, and eight thousand is the number that decides whether the
/// approach has any headroom at all.
const DENSITIES: [usize; 3] = [500, 2_000, 8_000];

/// Frames to run at each density.
///
/// Enough that a single unlucky scheduling hiccup cannot decide the answer.
const FRAMES: usize = 120;

/// How often the whole population turns over, in frames.
///
/// A fleck that lived for ever would measure only the steady state and miss
/// what churn costs, which is most of what an effect system does.
const LIFETIME_FRAMES: usize = 30;

fn main() {
    println!("Effect scaling: an entity per fleck, against a pooled batch.\n");
    println!(
        "{FRAMES} frames at each density, with the whole population replaced \
         every {LIFETIME_FRAMES} frames.\n"
    );
    println!(
        "{:>8}  {:>10}  {:>10}  {:>10}  {:>10}  {:>10}  {:>9}",
        "flecks", "emit", "update", "extract", "retire", "per frame", "budget"
    );

    for density in DENSITIES {
        let entities = time_entities(density);
        let pooled = time_pooled(density);
        report("entities", density, &entities);
        report("pooled", density, &pooled);
        println!();
    }

    println!("Budget is the share of one 16.7 ms frame at 60 Hz.\n");
    parse_share();
    println!("\nSee docs/effect-scaling.md for what this decided.");
}

/// What one approach cost, split by the phase a frame actually performs.
#[derive(Default)]
struct Phases {
    emit: Duration,
    update: Duration,
    extract: Duration,
    retire: Duration,
}

impl Phases {
    fn total(&self) -> Duration {
        self.emit + self.update + self.extract + self.retire
    }
}

#[allow(clippy::cast_precision_loss)]
fn report(label: &str, density: usize, phases: &Phases) {
    let per_frame = phases.total().as_secs_f64() / FRAMES as f64;
    let budget = per_frame / (1.0 / 60.0) * 100.0;
    println!(
        "{density:>8}  {:>10}  {:>10}  {:>10}  {:>10}  {:>10}  {budget:>8.1}%   {label}",
        millis(phases.emit),
        millis(phases.update),
        millis(phases.extract),
        millis(phases.retire),
        format!("{:.3} ms", per_frame * 1_000.0),
    );
}

/// How much of extraction is re-reading each fleck's component payload.
///
/// Extraction dominates the entity path, and "extraction is slow" is not a
/// finding anyone can act on. This narrows it: a typed query is exactly the
/// `serde_json` round trip that turns a stored payload back into a struct, and
/// it runs once per entity per frame.
fn parse_share() {
    let extractor = SceneExtractor::new().expect("the builtin components register");
    let mut world = World::default();
    for index in 0..8_000 {
        spawn_fleck(&mut world, index);
    }
    let at = Instant::now();
    for _ in 0..FRAMES {
        let found = extractor
            .components()
            .query::<SpriteComponent>(&world)
            .expect("sprites are registered");
        std::hint::black_box(&found);
    }
    let queried = at.elapsed();
    #[allow(clippy::cast_precision_loss)]
    let per_frame = queried.as_secs_f64() / FRAMES as f64 * 1_000.0;
    println!(
        "Of that, re-reading 8000 payloads is {per_frame:.3} ms per frame — the typed query alone,"
    );
    println!("with no transforms, no batching, and nothing drawn.");
}

fn millis(duration: Duration) -> String {
    format!("{:.1} ms", duration.as_secs_f64() * 1_000.0)
}

/// The whole loop for one density, timed as one number.
///
/// Timed together rather than per phase because the question is what a frame
/// costs, and a frame does all of it: retire what died, emit what replaced it,
/// move what is left, and draw the lot.
fn time_entities(density: usize) -> Phases {
    let extractor = SceneExtractor::new().expect("the builtin components register");
    let textures = TextureBindings::new();
    let viewport = Viewport::new(1280, 720);
    let mut world = World::default();
    // A camera, because extraction without one draws nothing and would measure
    // an empty answer.
    world.spawn(EntityData {
        transform_3d: Some(Transform3D {
            position: [0.0, 0.0, 10.0],
            ..Transform3D::default()
        }),
        components: [(
            "sindri.camera".to_owned(),
            json!({
                "projection": "orthographic",
                "vertical_size": 10.0,
                "near": 0.1,
                "far": 100.0
            }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });

    let mut live: Vec<EntityId> = Vec::with_capacity(density);
    let mut phases = Phases::default();
    for frame in 0..FRAMES {
        if frame % LIFETIME_FRAMES == 0 {
            let at = Instant::now();
            for entity in live.drain(..) {
                world
                    .despawn_recursive(entity)
                    .expect("a fleck has no children");
            }
            phases.retire += at.elapsed();

            let at = Instant::now();
            for index in 0..density {
                live.push(spawn_fleck(&mut world, index));
            }
            phases.emit += at.elapsed();
        }
        let at = Instant::now();
        for entity in &live {
            if let Some(data) = world.get_mut(*entity)
                && let Some(transform) = data.transform_3d.as_mut()
            {
                transform.position[0] += 0.01;
                transform.position[1] += 0.01;
            }
        }
        phases.update += at.elapsed();

        let at = Instant::now();
        extractor
            .extract(&world, viewport, CameraView::default(), &textures)
            .expect("a frame extracts");
        phases.extract += at.elapsed();
    }
    phases
}

fn spawn_fleck(world: &mut World, index: usize) -> EntityId {
    #[allow(clippy::cast_precision_loss)]
    let offset = index as f32 * 0.001;
    world.spawn(EntityData {
        transform_3d: Some(Transform3D {
            position: [offset, offset, 0.0],
            scale: [0.05, 0.05, 1.0],
            ..Transform3D::default()
        }),
        components: [(
            SpriteComponent::TYPE_NAME.to_owned(),
            json!({ "texture": "sindri:white", "tint": [1.0, 0.8, 0.2, 1.0], "layer": 5 }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    })
}

/// The same loop against a pool of plain values.
///
/// Deliberately the smallest thing that could work: a `Vec` of flecks, a
/// swap-remove for the dead, and one pass that produces what a renderer would
/// draw. If this is not meaningfully cheaper than an entity, there is no case
/// for building it.
fn time_pooled(density: usize) -> Phases {
    #[derive(Clone, Copy)]
    struct Fleck {
        position: [f32; 2],
        velocity: [f32; 2],
        remaining: f32,
        tint: [f32; 4],
    }

    let mut pool: Vec<Fleck> = Vec::with_capacity(density);
    let mut drawn: Vec<([f32; 2], [f32; 4])> = Vec::with_capacity(density);
    let mut phases = Phases::default();
    for frame in 0..FRAMES {
        if frame % LIFETIME_FRAMES == 0 {
            let at = Instant::now();
            pool.clear();
            phases.retire += at.elapsed();

            let at = Instant::now();
            for index in 0..density {
                #[allow(clippy::cast_precision_loss)]
                let offset = index as f32 * 0.001;
                pool.push(Fleck {
                    position: [offset, offset],
                    velocity: [0.01, 0.01],
                    // Long enough to survive the window, so retirement is the
                    // clear at the top rather than a trickle that would make
                    // the two approaches turn over differently.
                    remaining: 10.0,
                    tint: [1.0, 0.8, 0.2, 1.0],
                });
            }
            phases.emit += at.elapsed();
        }
        let at = Instant::now();
        let mut index = 0;
        while index < pool.len() {
            let fleck = &mut pool[index];
            fleck.position[0] += fleck.velocity[0];
            fleck.position[1] += fleck.velocity[1];
            fleck.remaining -= 1.0 / 60.0;
            if fleck.remaining <= 0.0 {
                pool.swap_remove(index);
            } else {
                index += 1;
            }
        }
        phases.update += at.elapsed();

        let at = Instant::now();
        drawn.clear();
        drawn.extend(pool.iter().map(|fleck| (fleck.position, fleck.tint)));
        std::hint::black_box(&drawn);
        phases.extract += at.elapsed();
    }
    phases
}
