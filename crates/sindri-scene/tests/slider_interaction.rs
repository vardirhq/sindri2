//! End-to-end pointer semantics for authored screen UI sliders.

use serde_json::json;
use sindri_core::{
    EntityData, EntityId, PointerDevice, PressId, PressPhase, Presses, SceneComponent, Transform3D,
    World,
};
use sindri_scene::{
    SceneExtractor, ScreenExtent, ScreenUi, UiSliderComponent, UiSliderOrientation,
};
use std::time::Duration;

const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 600.0;

fn registry() -> sindri_core::ComponentSchemaRegistry {
    SceneExtractor::new()
        .expect("the builtin components register")
        .components()
        .clone()
}

fn slider(world: &mut World, orientation: UiSliderOrientation) -> EntityId {
    world.spawn(EntityData {
        transform_3d: Some(Transform3D {
            scale: [1.0, 1.0, 1.0],
            ..Transform3D::default()
        }),
        components: [(
            UiSliderComponent::TYPE_NAME.to_owned(),
            json!({
                "orientation": orientation.as_str(),
                "min": 0.0,
                "max": 100.0,
                "step": 1.0,
                "value": 50.0
            }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    })
}

fn value(world: &World, entity: EntityId) -> f32 {
    let payload =
        &world.get(entity).expect("slider exists").components[UiSliderComponent::TYPE_NAME];
    serde_json::from_value::<UiSliderComponent>(payload.clone())
        .expect("valid slider")
        .value
}

fn update(screen: &mut ScreenUi, world: &mut World, presses: &Presses) {
    screen
        .update(
            world,
            &registry(),
            ScreenExtent::new(WIDTH, HEIGHT),
            presses,
        )
        .expect("registered components");
}

#[test]
fn vertical_slider_grows_from_bottom_to_top() {
    let mut world = World::default();
    let entity = slider(&mut world, UiSliderOrientation::Vertical);
    let id = PressId::new(PointerDevice::Mouse, 0);
    let mut presses = Presses::default();
    let mut screen = ScreenUi::new();

    presses.set_hover(Some([WIDTH / 2.0, HEIGHT * 0.75]));
    presses.begin(id, [WIDTH / 2.0, HEIGHT * 0.75]);
    update(&mut screen, &mut world, &presses);
    assert!((value(&world, entity) - 25.0).abs() < f32::EPSILON);

    presses.advance(Duration::from_millis(16));
    presses.move_to(id, [WIDTH / 2.0, HEIGHT * 0.25]);
    presses.set_hover(Some([WIDTH / 2.0, HEIGHT * 0.25]));
    update(&mut screen, &mut world, &presses);
    assert!((value(&world, entity) - 75.0).abs() < f32::EPSILON);
    assert!(screen.slider_changed(entity));
}

#[test]
fn second_finger_cannot_steal_an_active_slider_drag() {
    let mut world = World::default();
    let entity = slider(&mut world, UiSliderOrientation::Horizontal);
    let first = PressId::new(PointerDevice::Touch, 1);
    let second = PressId::new(PointerDevice::Touch, 2);
    let mut presses = Presses::default();
    let mut screen = ScreenUi::new();

    presses.begin(first, [WIDTH * 0.25, HEIGHT / 2.0]);
    update(&mut screen, &mut world, &presses);
    assert!((value(&world, entity) - 25.0).abs() < f32::EPSILON);

    presses.advance(Duration::from_millis(16));
    presses.begin(second, [WIDTH * 0.9, HEIGHT / 2.0]);
    presses.move_to(first, [WIDTH * 0.4, HEIGHT / 2.0]);
    update(&mut screen, &mut world, &presses);
    assert!((value(&world, entity) - 40.0).abs() < f32::EPSILON);
    assert!(screen.is_held(entity));

    presses.finish(second, PressPhase::Ended);
    presses.move_to(first, [WIDTH * 0.6, HEIGHT / 2.0]);
    update(&mut screen, &mut world, &presses);
    assert!((value(&world, entity) - 60.0).abs() < f32::EPSILON);
}

#[test]
fn cancelling_a_drag_does_not_apply_a_cancelled_position() {
    let mut world = World::default();
    let entity = slider(&mut world, UiSliderOrientation::Horizontal);
    let id = PressId::new(PointerDevice::Mouse, 0);
    let mut presses = Presses::default();
    let mut screen = ScreenUi::new();

    presses.set_hover(Some([WIDTH * 0.25, HEIGHT / 2.0]));
    presses.begin(id, [WIDTH * 0.25, HEIGHT / 2.0]);
    update(&mut screen, &mut world, &presses);
    assert!((value(&world, entity) - 25.0).abs() < f32::EPSILON);

    presses.advance(Duration::from_millis(16));
    presses.move_to(id, [WIDTH * 0.9, HEIGHT / 2.0]);
    presses.cancel_all();
    update(&mut screen, &mut world, &presses);

    assert!((value(&world, entity) - 25.0).abs() < f32::EPSILON);
    assert!(!screen.slider_changed(entity));
    assert!(!screen.is_held(entity));
}
