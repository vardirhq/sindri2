//! Buttons, screens and layout: what the pointer is doing to the overlay.

use serde_json::json;
use sindri_core::{EntityData, EntityId, SceneComponent, Transform3D, World};
use sindri_scene::{
    PointerFrame, SafeArea, SceneExtractor, ScreenExtent, ScreenUi, UiButtonComponent,
    UiImageComponent, UiLayoutComponent,
};

/// A viewport wider than it is tall, so the two axes cannot be confused.
const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 600.0;

/// The builtin registry, which is where the UI components are declared.
fn registry() -> sindri_core::ComponentSchemaRegistry {
    SceneExtractor::new()
        .expect("the builtin components register")
        .components()
        .clone()
}

fn extent() -> ScreenExtent {
    ScreenExtent::new(WIDTH, HEIGHT)
}

/// A button of the given size, at the given offset from the centre.
fn button(world: &mut World, at: [f32; 2], size: [f32; 2]) -> EntityId {
    world.spawn(EntityData {
        transform_3d: Some(Transform3D {
            position: [at[0], at[1], 0.0],
            scale: [size[0], size[1], 1.0],
            ..Transform3D::default()
        }),
        components: [(
            UiButtonComponent::TYPE_NAME.to_owned(),
            json!({ "label": "Start" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    })
}

/// The pointer at a place in the viewport, in pixels.
fn at(x: f32, y: f32) -> PointerFrame {
    PointerFrame {
        position: Some([x, y]),
        ..PointerFrame::default()
    }
}

fn updated(world: &World, pointer: PointerFrame) -> ScreenUi {
    let mut screen = ScreenUi::new();
    screen
        .update(world, &registry(), extent(), pointer)
        .expect("the UI components are registered");
    screen
}

#[test]
fn the_pointer_over_a_button_hovers_it() {
    let mut world = World::default();
    let entity = button(&mut world, [0.0, 0.0], [0.5, 0.2]);
    let screen = updated(&world, at(WIDTH / 2.0, HEIGHT / 2.0));
    assert!(screen.is_hovered(entity));
    assert!(screen.captures_pointer());
}

#[test]
fn the_pointer_beside_a_button_does_not() {
    let mut world = World::default();
    let entity = button(&mut world, [0.0, 0.0], [0.5, 0.2]);
    let screen = updated(&world, at(10.0, 10.0));
    assert!(!screen.is_hovered(entity));
    assert!(!screen.captures_pointer());
}

/// A press and a release on the same element.
#[test]
fn a_click_is_a_press_and_a_release_on_the_same_button() {
    let mut world = World::default();
    let entity = button(&mut world, [0.0, 0.0], [0.5, 0.2]);
    let middle = [WIDTH / 2.0, HEIGHT / 2.0];
    let mut screen = ScreenUi::new();
    let components = registry();

    let step = |pointer: PointerFrame, screen: &mut ScreenUi| {
        screen
            .update(&world, &components, extent(), pointer)
            .expect("registered");
    };
    step(
        PointerFrame {
            position: Some(middle),
            pressed: true,
            down: true,
            ..PointerFrame::default()
        },
        &mut screen,
    );
    assert!(!screen.is_pressed(entity), "a press alone is not a click");
    assert!(screen.is_held(entity), "but it is a hold");

    step(
        PointerFrame {
            position: Some(middle),
            released: true,
            ..PointerFrame::default()
        },
        &mut screen,
    );
    assert!(screen.is_pressed(entity), "release completes the click");
}

/// Sliding off before letting go is how a person changes their mind.
#[test]
fn letting_go_somewhere_else_is_not_a_click() {
    let mut world = World::default();
    let entity = button(&mut world, [0.0, 0.0], [0.4, 0.2]);
    let components = registry();
    let mut screen = ScreenUi::new();
    screen
        .update(
            &world,
            &components,
            extent(),
            PointerFrame {
                position: Some([WIDTH / 2.0, HEIGHT / 2.0]),
                pressed: true,
                down: true,
                ..PointerFrame::default()
            },
        )
        .expect("registered");
    screen
        .update(
            &world,
            &components,
            extent(),
            PointerFrame {
                position: Some([10.0, 10.0]),
                released: true,
                ..PointerFrame::default()
            },
        )
        .expect("registered");
    assert!(!screen.is_pressed(entity));
}

/// A pointer that left the window without a release must not leave a button
/// armed for ever.
#[test]
fn a_press_that_never_ended_does_not_survive_the_pointer_leaving() {
    let mut world = World::default();
    let entity = button(&mut world, [0.0, 0.0], [0.4, 0.2]);
    let components = registry();
    let mut screen = ScreenUi::new();
    screen
        .update(
            &world,
            &components,
            extent(),
            PointerFrame {
                position: Some([WIDTH / 2.0, HEIGHT / 2.0]),
                pressed: true,
                down: true,
                ..PointerFrame::default()
            },
        )
        .expect("registered");
    // The pointer is simply gone: no release ever arrives.
    screen
        .update(&world, &components, extent(), PointerFrame::default())
        .expect("registered");
    assert!(!screen.is_held(entity));
    screen
        .update(
            &world,
            &components,
            extent(),
            PointerFrame {
                position: Some([WIDTH / 2.0, HEIGHT / 2.0]),
                released: true,
                ..PointerFrame::default()
            },
        )
        .expect("registered");
    assert!(!screen.is_pressed(entity), "a stale press became a click");
}

/// Showing a screen is switching it on, and a switched-off one cannot be
/// clicked through.
#[test]
fn a_disabled_button_is_not_hit_tested() {
    let mut world = World::default();
    let entity = button(&mut world, [0.0, 0.0], [0.5, 0.2]);
    world.get_mut(entity).expect("there").disabled = true;
    let screen = updated(&world, at(WIDTH / 2.0, HEIGHT / 2.0));
    assert!(!screen.is_hovered(entity));
    assert!(!screen.captures_pointer());
}

/// A screen is an entity with children, so switching the parent off takes the
/// whole menu with it.
#[test]
fn a_button_under_a_disabled_screen_is_not_hit_tested() {
    let mut world = World::default();
    let screen_root = world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        ..EntityData::default()
    });
    let entity = button(&mut world, [0.0, 0.0], [0.5, 0.2]);
    world
        .set_parent(entity, Some(screen_root))
        .expect("parented");
    world.get_mut(screen_root).expect("there").disabled = true;
    let screen = updated(&world, at(WIDTH / 2.0, HEIGHT / 2.0));
    assert!(!screen.is_hovered(entity));
}

/// A modal is a modal because it is on a higher layer.
#[test]
fn the_top_element_takes_the_click() {
    let mut world = World::default();
    let under = button(&mut world, [0.0, 0.0], [0.5, 0.2]);
    let over = button(&mut world, [0.0, 0.0], [0.5, 0.2]);
    for (entity, layer) in [(under, 0), (over, 5)] {
        world.get_mut(entity).expect("there").components.insert(
            UiImageComponent::TYPE_NAME.to_owned(),
            json!({ "texture": "panel.png", "layer": layer }),
        );
    }
    let screen = updated(&world, at(WIDTH / 2.0, HEIGHT / 2.0));
    assert!(screen.is_hovered(over));
    assert!(!screen.is_hovered(under));
}

/// The whole reason layout exists: a hidden entry does not leave a hole.
#[test]
fn a_column_closes_up_around_a_hidden_entry() {
    let mut world = World::default();
    let parent = world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        components: [(
            UiLayoutComponent::TYPE_NAME.to_owned(),
            json!({ "direction": "column", "spacing": 0.5 }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let entries: Vec<EntityId> = (0..3)
        .map(|_| {
            let child = button(&mut world, [0.0, 0.0], [0.5, 0.2]);
            world.set_parent(child, Some(parent)).expect("parented");
            child
        })
        .collect();

    let all = updated(&world, PointerFrame::default());
    let spread = |screen: &ScreenUi, of: &[EntityId]| {
        of.iter()
            .filter_map(|entity| screen.rect(*entity))
            .map(|rect| rect.center[1])
            .fold(f32::NEG_INFINITY, f32::max)
    };
    let three = spread(&all, &entries);

    world.get_mut(entries[1]).expect("there").disabled = true;
    let two = updated(&world, PointerFrame::default());
    assert!(
        spread(&two, &entries) < three,
        "the remaining entries did not close up"
    );
    // And they are still centred on the parent.
    let middle: f32 = entries
        .iter()
        .filter_map(|entity| two.rect(*entity))
        .map(|rect| rect.center[1])
        .sum();
    assert!(middle.abs() < 1.0e-5, "the menu drifted off its middle");
}

/// A notch is not in the scene; it is in the hardware the scene is running on.
#[test]
fn a_safe_area_moves_a_top_anchored_button_down() {
    let mut world = World::default();
    let entity = button(&mut world, [0.0, 0.0], [0.5, 0.2]);
    world.get_mut(entity).expect("there").components.insert(
        UiImageComponent::TYPE_NAME.to_owned(),
        json!({ "texture": "panel.png", "anchor": "top" }),
    );
    let components = registry();
    let mut plain = ScreenUi::new();
    plain
        .update(&world, &components, extent(), PointerFrame::default())
        .expect("registered");
    let mut inset = ScreenUi::new();
    inset
        .update(
            &world,
            &components,
            extent().with_safe_area(SafeArea {
                top: 60.0,
                ..SafeArea::default()
            }),
            PointerFrame::default(),
        )
        .expect("registered");
    assert!(
        inset.rect(entity).expect("placed").center[1]
            < plain.rect(entity).expect("placed").center[1],
        "the notch did not move the button"
    );
}

/// The same authored scene has to work on a phone and a desktop window.
#[test]
fn a_corner_button_is_reachable_in_portrait_and_landscape() {
    let mut world = World::default();
    let entity = button(&mut world, [0.0, 0.0], [0.4, 0.2]);
    world.get_mut(entity).expect("there").components.insert(
        UiImageComponent::TYPE_NAME.to_owned(),
        json!({ "texture": "panel.png", "anchor": "bottom_right" }),
    );
    let components = registry();
    for (width, height) in [(1920.0_f32, 1080.0_f32), (390.0, 844.0)] {
        let shape = ScreenExtent::new(width, height);
        let mut screen = ScreenUi::new();
        screen
            .update(
                &world,
                &components,
                shape,
                // The pointer at the bottom-right corner of that window.
                PointerFrame {
                    position: Some([width - 1.0, height - 1.0]),
                    ..PointerFrame::default()
                },
            )
            .expect("registered");
        assert!(screen.is_hovered(entity), "unreachable at {width}x{height}");
    }
}

/// An element that draws but says nothing about being pressable is not.
#[test]
fn an_image_that_is_not_a_button_does_not_take_the_pointer() {
    let mut world = World::default();
    let entity = world.spawn(EntityData {
        transform_3d: Some(Transform3D {
            scale: [1.0, 1.0, 1.0],
            ..Transform3D::default()
        }),
        components: [(
            UiImageComponent::TYPE_NAME.to_owned(),
            json!({ "texture": "panel.png" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let screen = updated(&world, at(WIDTH / 2.0, HEIGHT / 2.0));
    assert!(!screen.is_hovered(entity));
    assert!(!screen.captures_pointer(), "a HUD swallowed the pointer");
}
