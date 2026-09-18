//! Playing the game: building a way across, and somebody walking it.
//!
//! Stated as clicks rather than as cells. A test that called `set_block`
//! would prove the volume can be written to and nothing about the game, which
//! is a person pointing at a picture. Every placement here goes the whole way
//! round -- through the camera, into a ray, onto a face, back out as a cell --
//! because that trip is the thing that can break.

use glam::{Mat4, Vec3};
use sindri_causeway::{Session, extractor, world};
use sindri_core::World;
use sindri_platform::{InputEvent, InputState, MouseButton};
use sindri_scene::{SceneExtractor, TileVolumeComponent};

const VIEWPORT: (f32, f32) = (1000.0, 720.0);
const STEP: f32 = 1.0 / 60.0;

/// The blocks whose east side is clicked to extend the causeway.
///
/// Sides rather than tops, and the level is what decides that. A water tile's
/// top sits a course below the shore beside it, so the shore stands in front
/// of the last one and a click aimed at it never arrives. The side of the
/// block you are building *from* always faces you, which is why the game's own
/// hint says to click one.
const CROSSING: &[(i32, i32, i32)] = &[(6, 7, 0), (7, 7, 0), (8, 7, 0), (9, 7, 0)];
/// The stair, on the side of the pillar that faces the camera.
///
/// Not an arbitrary choice: a click is a ray, so a cell the pillar stands in
/// front of cannot be clicked at all. Aiming at one hits the pillar's own
/// south face and builds a staircase down its hidden side, which is what the
/// first version of this list did.
const STAIR: &[(i32, i32, i32)] = &[(15, 7, 0), (15, 7, 1), (16, 7, 0)];

fn session() -> (World, SceneExtractor, Session) {
    let scene = extractor().expect("the schemas register");
    let (world, loaded) = world().expect("the scene loads");
    let session = Session::new(scene.components().clone())
        .with_scenes(sindri_causeway::scenes().expect("the scenes load"), loaded)
        .with_tile_sets(sindri_causeway::bind_tile_sets().expect("the tile set binds"));
    (world, scene, session)
}

fn view_projection(world: &World, scene: &SceneExtractor) -> Mat4 {
    sindri_scene::world_camera_of(world, scene.components(), VIEWPORT.0 / VIEWPORT.1)
        .expect("the camera resolves")
        .expect("the scene has a world camera")
        .view_projection
}

/// Where the east side of a cell lands on the picture, in viewport pixels.
///
/// East and south are the two sides this camera can see, so they are the two
/// a player can click.
fn east_of(cell: (i32, i32, i32), view_projection: Mat4) -> [f32; 2] {
    #[allow(clippy::cast_precision_loss)]
    let centre = Vec3::new(cell.0 as f32 + 0.5, cell.2 as f32 + 0.5, cell.1 as f32);
    project(centre, view_projection)
}

/// Where the top of a cell lands on the picture, in viewport pixels.
fn top_of(cell: (i32, i32, i32), view_projection: Mat4) -> [f32; 2] {
    #[allow(clippy::cast_precision_loss)]
    let centre = Vec3::new(cell.0 as f32, (cell.2 + 1) as f32, cell.1 as f32);
    project(centre, view_projection)
}

fn project(point: Vec3, view_projection: Mat4) -> [f32; 2] {
    let clip = view_projection * point.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    [
        (ndc.x + 1.0) * 0.5 * VIEWPORT.0,
        (1.0 - ndc.y) * 0.5 * VIEWPORT.1,
    ]
}

fn click(world: &mut World, session: &mut Session, at: [f32; 2], button: MouseButton) {
    let mut held = InputState::default();
    held.apply(InputEvent::PointerMoved { x: at[0], y: at[1] });
    held.apply(InputEvent::ButtonPressed(button));
    session
        .step(world, &held, VIEWPORT, STEP)
        .expect("the press steps");
    // A frame boundary, exactly as a host puts one there. Without it the press
    // edge is still set on the next step and one click lays two blocks --
    // which is what a harness that skipped this quietly did.
    held.begin_frame(std::time::Duration::from_secs_f32(STEP));
    held.apply(InputEvent::ButtonReleased(button));
    session
        .step(world, &held, VIEWPORT, STEP)
        .expect("the release steps");
}

fn settle(world: &mut World, session: &mut Session, steps: usize) {
    let idle = InputState::default();
    for _ in 0..steps {
        session
            .step(world, &idle, VIEWPORT, STEP)
            .expect("an idle step");
    }
}

fn block_at(world: &World, scene: &SceneExtractor, cell: (i32, i32, i32)) -> Option<String> {
    let (_, volume) = scene
        .components()
        .query::<TileVolumeComponent>(world)
        .expect("the volume schema reads")
        .into_iter()
        .next()
        .expect("the level has a volume");
    volume
        .cells
        .iter()
        .find(|held| held.position == [cell.0, cell.1, cell.2])
        .map(|held| held.tile.clone())
}

fn wanderer_cell(world: &World) -> (i32, i32) {
    let position = world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some("Wanderer"))
        .and_then(|(_, data)| data.transform_3d)
        .expect("the wanderer is there")
        .position;
    #[allow(clippy::cast_possible_truncation)]
    let round = |value: f32| (value + 0.5).floor() as i32;
    (round(position[0]), round(position[2]))
}

#[test]
fn clicking_the_top_of_a_block_puts_one_on_top_of_it() {
    let (mut world, scene, mut session) = session();
    settle(&mut world, &mut session, 1);
    let camera = view_projection(&world, &scene);

    assert_eq!(
        block_at(&world, &scene, (7, 7, 0)),
        None,
        "the channel starts empty"
    );
    click(
        &mut world,
        &mut session,
        top_of((7, 7, -1), camera),
        MouseButton::Left,
    );
    assert_eq!(
        block_at(&world, &scene, (7, 7, 0)).as_deref(),
        Some("plank"),
        "a click on the water's top lays a plank on it"
    );
}

#[test]
fn a_click_lands_on_what_is_there_now_rather_than_on_the_authored_level() {
    // The difference between picking and arithmetic. The second click here
    // names a cell that did not exist when the level was written; only a ray
    // cast against the volume as it currently stands can find it.
    let (mut world, scene, mut session) = session();
    settle(&mut world, &mut session, 1);
    let camera = view_projection(&world, &scene);

    click(
        &mut world,
        &mut session,
        top_of((15, 7, 0), camera),
        MouseButton::Left,
    );
    assert_eq!(
        block_at(&world, &scene, (15, 7, 1)).as_deref(),
        Some("plank"),
        "the first click laid one on the shore"
    );
    click(
        &mut world,
        &mut session,
        top_of((15, 7, 1), camera),
        MouseButton::Left,
    );
    assert_eq!(
        block_at(&world, &scene, (15, 7, 2)).as_deref(),
        Some("plank"),
        "the second click stacked onto the block the first one made"
    );
}

#[test]
fn taking_a_block_back_returns_it_but_the_island_is_not_yours_to_carry_away() {
    let (mut world, scene, mut session) = session();
    settle(&mut world, &mut session, 1);
    let camera = view_projection(&world, &scene);

    click(
        &mut world,
        &mut session,
        top_of((7, 7, -1), camera),
        MouseButton::Left,
    );
    click(
        &mut world,
        &mut session,
        top_of((7, 7, 0), camera),
        MouseButton::Right,
    );
    assert_eq!(
        block_at(&world, &scene, (7, 7, 0)),
        None,
        "a plank you laid comes back up"
    );

    // The shore is ground rather than something the player put there.
    click(
        &mut world,
        &mut session,
        top_of((2, 7, 0), camera),
        MouseButton::Right,
    );
    assert_eq!(
        block_at(&world, &scene, (2, 7, 0)).as_deref(),
        Some("ground"),
        "the island stays where it is"
    );
}

#[test]
fn the_walker_waits_until_the_way_is_built_and_then_crosses_it() {
    // The whole game in one test. Nothing instructs the walker: the player
    // changes the world it keeps asking about, and the answer becomes yes.
    let (mut world, scene, mut session) = session();
    settle(&mut world, &mut session, 120);
    let camera = view_projection(&world, &scene);
    let waited = wanderer_cell(&world);
    assert!(
        waited.0 <= 7,
        "with no way across, the walker stays on its own shore: {waited:?}"
    );

    for cell in CROSSING {
        click(
            &mut world,
            &mut session,
            east_of(*cell, camera),
            MouseButton::Left,
        );
    }
    for cell in STAIR {
        click(
            &mut world,
            &mut session,
            top_of(*cell, camera),
            MouseButton::Left,
        );
    }
    settle(&mut world, &mut session, 900);

    let arrived = wanderer_cell(&world);
    assert_eq!(
        arrived,
        (14, 7),
        "once the way is built the walker takes it to the beacon: {arrived:?}"
    );
}
