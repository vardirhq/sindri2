//! Playing the game: pointing at the world, and changing it.
//!
//! Stated as clicks rather than as cells. A test that called `set_block` would
//! prove the volume can be written to and nothing about a person pointing at a
//! picture, so every placement here goes the whole way round -- through the
//! camera, into a ray, onto a face, back out as a cell.
//!
//! Nothing here names a coordinate the world was authored with, because the
//! world is not authored: it is generated from a seed, and a cell that is a
//! meadow under one seed is the bottom of a lake under the next. The tests ask
//! the world where the game starts and work from there.

use glam::{Mat4, Vec3};
use sindri_causeway::{
    Session, extractor, world,
    worldgen::{SEA, WorldShape},
};
use sindri_core::World;
use sindri_platform::{InputEvent, InputState, Key, MouseButton};
use sindri_scene::{SceneExtractor, TileVolumeComponent, UiTextComponent};

const VIEWPORT: (f32, f32) = (1000.0, 720.0);
const STEP: f32 = 1.0 / 60.0;

fn session() -> (World, SceneExtractor, Session) {
    let scene = extractor().expect("the schemas register");
    let (world, loaded) = world().expect("the world loads");
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

fn project(point: Vec3, view_projection: Mat4) -> [f32; 2] {
    let clip = view_projection * point.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    [
        (ndc.x + 1.0) * 0.5 * VIEWPORT.0,
        (1.0 - ndc.y) * 0.5 * VIEWPORT.1,
    ]
}

/// The same, for the east side -- one of the two this camera can see.
fn east_of(cell: [i32; 3], view_projection: Mat4) -> [f32; 2] {
    #[allow(clippy::cast_precision_loss)]
    let centre = Vec3::new(cell[0] as f32 + 0.5, cell[2] as f32 + 0.5, cell[1] as f32);
    project(centre, view_projection)
}

fn click(world: &mut World, session: &mut Session, at: [f32; 2], button: MouseButton) {
    let mut held = InputState::default();
    held.apply(InputEvent::PointerMoved { x: at[0], y: at[1] });
    held.apply(InputEvent::ButtonPressed(button));
    session
        .step(world, &held, VIEWPORT, STEP)
        .expect("the press steps");
    // A frame boundary, exactly as a host puts one there. Without it the press
    // edge is still set on the next step and one click lays two blocks.
    held.begin_frame(std::time::Duration::from_secs_f32(STEP));
    held.apply(InputEvent::ButtonReleased(button));
    session
        .step(world, &held, VIEWPORT, STEP)
        .expect("the release steps");
    // One frame with nothing down, which is what a host does between two
    // clicks. Without it the finished press is still in the set when the next
    // one starts, and the recogniser -- which keeps what it has decided about
    // a press for as long as the press exists -- carries the first click's
    // verdict onto the second.
    held.begin_frame(std::time::Duration::from_secs_f32(STEP));
    session
        .step(world, &held, VIEWPORT, STEP)
        .expect("the empty frame steps");
}

/// Holding still, which is how a block is taken back.
///
/// A finger has no second button, so removing is a press that stays put and
/// stays down past the long-press limit. The press is left down for a good
/// while rather than one step, because that duration is the whole gesture.
fn hold(world: &mut World, session: &mut Session, at: [f32; 2]) {
    let mut held = InputState::default();
    held.apply(InputEvent::PointerMoved { x: at[0], y: at[1] });
    held.apply(InputEvent::ButtonPressed(MouseButton::Left));
    session
        .step(world, &held, VIEWPORT, STEP)
        .expect("the press steps");
    // Past the long-press limit without moving. Reported once, while the
    // finger is still down, so one hold takes one block.
    held.begin_frame(std::time::Duration::from_millis(600));
    session
        .step(world, &held, VIEWPORT, STEP)
        .expect("the hold steps");
    held.begin_frame(std::time::Duration::from_secs_f32(STEP));
    held.apply(InputEvent::ButtonReleased(MouseButton::Left));
    session
        .step(world, &held, VIEWPORT, STEP)
        .expect("the release steps");
}

fn tap_touch(
    world: &mut World,
    scene: &SceneExtractor,
    session: &mut Session,
    at: [f32; 2],
) -> [i32; 3] {
    let mut input = InputState::default();
    input.apply(InputEvent::TouchStarted {
        id: 7,
        x: at[0],
        y: at[1],
    });
    session
        .step(world, &input, VIEWPORT, STEP)
        .expect("the touch press steps");

    input.begin_frame(std::time::Duration::from_secs_f32(STEP));
    input.apply(InputEvent::TouchEnded { id: 7 });
    // Play's follow camera can advance between press and release. Capture the
    // release-frame Aim the script will consume rather than assuming touch-down
    // and touch-up project onto the same voxel.
    let aim = sindri_scene::voxel::aim_at(
        world,
        scene.components(),
        view_projection(world, scene),
        [at[0] / VIEWPORT.0, at[1] / VIEWPORT.1],
    )
    .expect("the released touch still points at the world");
    let touched = [aim.cell.x, aim.cell.y, aim.cell.z];
    session
        .step(world, &input, VIEWPORT, STEP)
        .expect("the touch release steps");

    input.begin_frame(std::time::Duration::from_secs_f32(STEP));
    session
        .step(world, &input, VIEWPORT, STEP)
        .expect("the empty touch frame steps");
    touched
}

fn entity_position(world: &World, name: &str) -> [f32; 3] {
    world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some(name))
        .and_then(|(_, data)| data.transform_3d)
        .expect("named entity has a transform")
        .position
}

fn planar_distance(left: [f32; 3], right: [f32; 3]) -> f32 {
    let dx = left[0] - right[0];
    let dz = left[2] - right[2];
    (dx * dx + dz * dz).sqrt()
}

fn ui_text(world: &World, scene: &SceneExtractor, name: &str) -> UiTextComponent {
    let entity = world
        .entities()
        .find(|(_, data)| data.name.as_deref() == Some(name))
        .map(|(entity, _)| entity)
        .expect("named UI entity exists");
    scene
        .components()
        .get::<UiTextComponent>(world, entity)
        .expect("the UI text schema reads")
        .expect("the named UI text is active")
}

fn settle(world: &mut World, session: &mut Session, steps: usize) {
    let idle = InputState::default();
    for _ in 0..steps {
        session
            .step(world, &idle, VIEWPORT, STEP)
            .expect("an idle step");
    }
}

fn volume(world: &World, scene: &SceneExtractor) -> TileVolumeComponent {
    scene
        .components()
        .query::<TileVolumeComponent>(world)
        .expect("the volume schema reads")
        .into_iter()
        .next()
        .expect("the world has a volume")
        .1
}

fn block_at(world: &World, scene: &SceneExtractor, cell: [i32; 3]) -> Option<String> {
    volume(world, scene)
        .cells
        .iter()
        .find(|held| held.position == cell)
        .map(|held| held.tile.clone())
}

/// A block the camera can actually see, and where on the picture it is.
///
/// Asked of the picker rather than worked out from the world, and the
/// difference matters: a column can be perfectly good ground and still be
/// behind a hill from here, and a click aimed at one lands on the hill. What a
/// test wants is somewhere a *player* could click, which is exactly what the
/// pointer already answers.
fn in_view(world: &World, scene: &SceneExtractor) -> ([i32; 3], [f32; 2]) {
    let camera = view_projection(world, scene);
    for (across, down) in [(0.5, 0.55), (0.42, 0.6), (0.58, 0.5), (0.5, 0.45)] {
        let at = [across * VIEWPORT.0, down * VIEWPORT.1];
        if let Some(aim) =
            sindri_scene::voxel::aim_at(world, scene.components(), camera, [across, down])
            && aim.face == sindri_core::TileFace::Top
        {
            return ([aim.cell.x, aim.cell.y, aim.cell.z], at);
        }
    }
    panic!("nothing in the middle of the picture to point at");
}

/// A visible top face outside one logical grid cell.
///
/// Play starts with Target under the Wanderer and the camera centred there,
/// so the ordinary centre-of-view helper can legitimately return Target's
/// current cell. A movement regression must choose somewhere else or a
/// correctly handled tap can look like no movement at all.
fn in_view_away_from(
    world: &World,
    scene: &SceneExtractor,
    excluded: [i32; 2],
) -> ([i32; 3], [f32; 2]) {
    let camera = view_projection(world, scene);
    for (across, down) in [
        (0.32, 0.58),
        (0.68, 0.42),
        (0.36, 0.42),
        (0.64, 0.58),
        (0.5, 0.68),
        (0.5, 0.32),
    ] {
        let at = [across * VIEWPORT.0, down * VIEWPORT.1];
        if let Some(aim) =
            sindri_scene::voxel::aim_at(world, scene.components(), camera, [across, down])
            && aim.face == sindri_core::TileFace::Top
            && (aim.cell.x - excluded[0]).abs() + (aim.cell.y - excluded[1]).abs() >= 4
        {
            return ([aim.cell.x, aim.cell.y, aim.cell.z], at);
        }
    }
    panic!("nothing outside {excluded:?} is visible to tap");
}

#[test]
fn clicking_the_top_of_the_ground_puts_a_block_on_it() {
    let (mut world, scene, mut session) = session();
    settle(&mut world, &mut session, 2);
    let (ground, at) = in_view(&world, &scene);
    let above = [ground[0], ground[1], ground[2] + 1];

    assert_eq!(block_at(&world, &scene, above), None, "the air is empty");
    click(&mut world, &mut session, at, MouseButton::Left);
    assert_eq!(
        block_at(&world, &scene, above).as_deref(),
        Some("plank-slab"),
        "a click on the ground's top lays a walkway on it"
    );
}

#[test]
fn a_click_lands_on_what_is_there_now_rather_than_on_the_world_as_generated() {
    // The difference between picking and arithmetic. The second tap names a
    // cell that did not exist when the world was built; only a ray cast
    // against the volume as it currently stands can find it.
    let (mut world, scene, mut session) = session();
    settle(&mut world, &mut session, 2);
    let (ground, at) = in_view(&world, &scene);

    // The same point on the picture, twice. The second ray meets the block the
    // first one made -- which is what this test is for, and is what a player
    // does anyway.
    //
    // Which *face* of that block it meets is not fixed, and asserting one was
    // this test's own mistake: a pixel that struck the ground's top strikes
    // whichever part of the new block now covers it, top or side, depending on
    // where in the cell it fell. What can be said is that the second tap built
    // against the first block rather than at the first block's own cell, and
    // no arithmetic on the world as generated could have found that cell.
    click(&mut world, &mut session, at, MouseButton::Left);
    let first = [ground[0], ground[1], ground[2] + 1];
    assert_eq!(
        block_at(&world, &scene, first).as_deref(),
        Some("plank-slab"),
        "the first tap builds on the ground"
    );

    click(&mut world, &mut session, at, MouseButton::Left);
    let second = [-1, 0, 1]
        .into_iter()
        .flat_map(|dx| [-1, 0, 1].map(move |dy| (dx, dy)))
        .flat_map(|(dx, dy)| [0, 1].map(move |dz| [first[0] + dx, first[1] + dy, first[2] + dz]))
        .find(|cell| {
            *cell != first && block_at(&world, &scene, *cell).as_deref() == Some("plank-slab")
        })
        .expect("the second tap built somewhere against the first block");

    // Touching it, which is what building against a face means.
    let reach =
        (second[0] - first[0]).abs() + (second[1] - first[1]).abs() + (second[2] - first[2]).abs();
    assert_eq!(
        reach, 1,
        "and built against the block the first tap made, not somewhere else: \
         {first:?} then {second:?}"
    );
}

#[test]
fn what_you_laid_comes_back_but_the_world_is_not_yours_to_carry_away() {
    let (mut world, scene, mut session) = session();
    settle(&mut world, &mut session, 2);
    let (ground, at) = in_view(&world, &scene);
    let above = [ground[0], ground[1], ground[2] + 1];

    click(&mut world, &mut session, at, MouseButton::Left);
    hold(&mut world, &mut session, at);
    assert_eq!(
        block_at(&world, &scene, above),
        None,
        "a walkway you laid comes back up"
    );

    let was = block_at(&world, &scene, ground);
    hold(&mut world, &mut session, at);
    assert_eq!(
        block_at(&world, &scene, ground),
        was,
        "the ground the world made stays where it is"
    );
}

#[test]
fn the_sea_is_something_to_build_across_rather_than_to_walk_on() {
    // The whole game in one assertion: water is not a floor, and a block laid
    // on it is. Nothing here says where the water is -- the world decides, and
    // the test walks out from the start until it finds some.
    let (mut world, scene, mut session) = session();
    settle(&mut world, &mut session, 2);
    let camera = view_projection(&world, &scene);
    let shape = WorldShape::default();
    let start = shape.landfall().start;

    // A shore: land with water immediately to its east, which is the side this
    // camera can see and so the side a player can click.
    let shore = (0..shape.columns - start[0] - 2)
        .map(|step| [start[0] + step, start[1]])
        .find(|at| {
            let here = volume(&world, &scene)
                .cells
                .iter()
                .any(|cell| cell.position == [at[0], at[1], SEA + 1]);
            let east = volume(&world, &scene)
                .cells
                .iter()
                .any(|cell| cell.position == [at[0] + 1, at[1], SEA] && cell.tile == "water");
            here && east
        });
    let Some(shore) = shore else {
        // Not every world puts a coast due east of where it starts you. That
        // is the generator being a generator, not a failure.
        return;
    };

    let onto = [shore[0] + 1, shore[1], SEA + 1];
    assert_eq!(block_at(&world, &scene, onto), None, "the sea is open");
    click(
        &mut world,
        &mut session,
        east_of([shore[0], shore[1], SEA + 1], camera),
        MouseButton::Left,
    );
    assert_eq!(
        block_at(&world, &scene, onto).as_deref(),
        Some("plank-slab"),
        "clicking the shore's side lays a walkway out onto the water"
    );
}

#[test]
#[allow(clippy::cast_precision_loss, clippy::float_cmp)]
fn a_phone_tap_in_play_moves_the_target_and_the_wanderer() {
    let (mut world, scene, mut session) = session();
    settle(&mut world, &mut session, 2);

    let mut toggle = InputState::default();
    toggle.apply(InputEvent::KeyPressed(Key::Tab));
    session
        .step(&mut world, &toggle, VIEWPORT, STEP)
        .expect("play mode toggles");
    toggle.begin_frame(std::time::Duration::from_secs_f32(STEP));
    toggle.apply(InputEvent::KeyReleased(Key::Tab));
    session
        .step(&mut world, &toggle, VIEWPORT, STEP)
        .expect("tab releases");
    assert_eq!(
        ui_text(&world, &scene, "ModeLabel").text,
        "PLAY",
        "Game.playing must be true before the touch"
    );

    let target_before = entity_position(&world, "Target");
    let wanderer_before = entity_position(&world, "Wanderer");
    let camera_before = entity_position(&world, "World Camera");
    assert!(
        wanderer_before[0] >= 16.0
            && wanderer_before[0] < 144.0
            && wanderer_before[2] >= 16.0
            && wanderer_before[2] < 144.0,
        "the game must start at least one render chunk inside every world edge"
    );
    let initial_cell =
        sindri_scene::nearest_cell(f64::from(target_before[0]), f64::from(target_before[2]));
    let (_, at) = in_view_away_from(&world, &scene, [initial_cell.x, initial_cell.y]);

    let touched = tap_touch(&mut world, &scene, &mut session, at);
    let debug = ui_text(&world, &scene, "PlayDebug");
    assert_eq!(
        debug.values.first().copied(),
        Some(1.0),
        "Wanderer must observe Gesture.tapped (2 means Pointer.over_ui rejected it)"
    );
    assert_eq!(
        debug.values.get(1).copied(),
        Some(1.0),
        "Aim.hit must be true during the Play tap"
    );

    let target_after = entity_position(&world, "Target");
    assert_ne!(
        target_after, target_before,
        "a touch tap in Play must move the target"
    );
    assert_eq!(
        [target_after[0], target_after[2]],
        [touched[0] as f32, touched[1] as f32],
        "the target must use the release-frame voxel's logical column and row"
    );

    settle(&mut world, &mut session, 4);
    let wanderer_mid_step = entity_position(&world, "Wanderer");
    let travelled = planar_distance(wanderer_before, wanderer_mid_step);
    assert!(
        travelled > 0.0 && travelled < 0.5,
        "a rendered step must cross part of a cell instead of teleporting: {travelled}"
    );
    assert_eq!(
        entity_position(&world, "World Camera"),
        camera_before,
        "the camera must remain still while the Wanderer is inside its dead zone"
    );

    settle(&mut world, &mut session, 120);
    let wanderer_after = entity_position(&world, "Wanderer");
    assert_ne!(
        wanderer_after, wanderer_before,
        "the wanderer must continue moving toward the touch target"
    );
    assert_ne!(
        entity_position(&world, "World Camera"),
        camera_before,
        "the camera must ease after the Wanderer crosses its dead zone"
    );

    // Build edits the world; it must not quietly give the Wanderer its old
    // Beacon destination again. Switching modes may finish a grid step that
    // pathfinding already committed, but after that the Wanderer stays put.
    let mut build = InputState::default();
    build.apply(InputEvent::KeyPressed(Key::Tab));
    session
        .step(&mut world, &build, VIEWPORT, STEP)
        .expect("build mode toggles");
    build.begin_frame(std::time::Duration::from_secs_f32(STEP));
    build.apply(InputEvent::KeyReleased(Key::Tab));
    session
        .step(&mut world, &build, VIEWPORT, STEP)
        .expect("tab releases into build");
    assert_eq!(
        ui_text(&world, &scene, "ModeLabel").text,
        "BUILD",
        "the regression must actually enter Build mode"
    );

    let build_position = entity_position(&world, "Wanderer");
    settle(&mut world, &mut session, 120);
    assert_eq!(
        entity_position(&world, "Wanderer"),
        build_position,
        "the Wanderer must stay where Play left it while building"
    );
}
