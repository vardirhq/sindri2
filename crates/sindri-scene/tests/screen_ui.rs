//! Buttons, screens and layout: what the pointer is doing to the overlay.

use serde_json::json;
use sindri_core::{
    EntityData, EntityId, PointerDevice, PressId, PressPhase, Presses, SceneComponent, Transform3D,
    World,
};
use sindri_scene::{
    SafeArea, SceneExtractor, ScreenExtent, ScreenUi, UiButtonComponent, UiImageComponent,
    UiLayoutComponent,
};
use std::time::Duration;

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

/// One person's pointing, of a kind a test chooses.
///
/// Both kinds, because the difference between them is where this file was
/// blind: every check here used to build the pointer's state by hand, in the
/// shape a mouse makes, and so an interface that no finger could operate
/// passed all of them.
struct Hand {
    presses: Presses,
    id: PressId,
    hovers: bool,
}

impl Hand {
    fn mouse() -> Self {
        Self {
            presses: Presses::default(),
            id: PressId::new(PointerDevice::Mouse, 0),
            hovers: true,
        }
    }

    fn finger() -> Self {
        Self {
            presses: Presses::default(),
            id: PressId::new(PointerDevice::Touch, 1),
            hovers: false,
        }
    }

    /// Moves without pressing. A finger cannot do this, and saying so is the
    /// point: it is why a finger's press has to carry its own position.
    fn move_to(&mut self, at: [f32; 2]) {
        if self.hovers {
            self.presses.set_hover(Some(at));
        }
        self.presses.move_to(self.id, at);
    }

    fn press(&mut self, at: [f32; 2]) {
        self.move_to(at);
        self.presses.begin(self.id, at);
    }

    fn release(&mut self) {
        self.presses.finish(self.id, PressPhase::Ended);
    }

    /// The interaction taken away rather than let go.
    fn cancel(&mut self) {
        self.presses.cancel_all();
    }

    fn frame(&mut self) {
        self.presses.advance(Duration::from_millis(16));
    }
}

/// The pointer resting at a place in the viewport, in pixels.
fn at(x: f32, y: f32) -> Hand {
    let mut hand = Hand::mouse();
    hand.move_to([x, y]);
    hand
}

fn updated(world: &World, hand: &Hand) -> ScreenUi {
    let mut screen = ScreenUi::new();
    screen
        .update(world, &registry(), extent(), &hand.presses)
        .expect("the UI components are registered");
    screen
}

#[test]
fn the_pointer_over_a_button_hovers_it() {
    let mut world = World::default();
    let entity = button(&mut world, [0.0, 0.0], [0.5, 0.2]);
    let screen = updated(&world, &at(WIDTH / 2.0, HEIGHT / 2.0));
    assert!(screen.is_hovered(entity));
    assert!(screen.captures_pointer());
}

#[test]
fn the_pointer_beside_a_button_does_not() {
    let mut world = World::default();
    let entity = button(&mut world, [0.0, 0.0], [0.5, 0.2]);
    let screen = updated(&world, &at(10.0, 10.0));
    assert!(!screen.is_hovered(entity));
    assert!(!screen.captures_pointer());
}

/// A press and a release on the same element.
#[test]
fn a_click_is_a_press_and_a_release_on_the_same_button() {
    let mut world = World::default();
    let entity = button(&mut world, [0.0, 0.0], [0.5, 0.2]);
    let middle = [WIDTH / 2.0, HEIGHT / 2.0];
    let components = registry();
    let step = |hand: &Hand, screen: &mut ScreenUi| {
        screen
            .update(&world, &components, extent(), &hand.presses)
            .expect("registered");
    };

    // Played twice, with a mouse and with a finger, because the two are not
    // the same interaction and only one of them used to work.
    for mut hand in [Hand::mouse(), Hand::finger()] {
        let mut screen = ScreenUi::new();
        hand.press(middle);
        step(&hand, &mut screen);
        assert!(!screen.is_pressed(entity), "a press alone is not a click");
        assert!(screen.is_held(entity), "but it is a hold");

        hand.frame();
        hand.release();
        step(&hand, &mut screen);
        assert!(screen.is_pressed(entity), "release completes the click");
    }
}

/// Sliding off before letting go is how a person changes their mind.
#[test]
fn letting_go_somewhere_else_is_not_a_click() {
    let mut world = World::default();
    let entity = button(&mut world, [0.0, 0.0], [0.4, 0.2]);
    let components = registry();
    let mut screen = ScreenUi::new();
    let mut hand = Hand::mouse();
    hand.press([WIDTH / 2.0, HEIGHT / 2.0]);
    screen
        .update(&world, &components, extent(), &hand.presses)
        .expect("registered");

    hand.frame();
    hand.move_to([10.0, 10.0]);
    hand.release();
    screen
        .update(&world, &components, extent(), &hand.presses)
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
    let mut hand = Hand::mouse();
    hand.press([WIDTH / 2.0, HEIGHT / 2.0]);
    screen
        .update(&world, &components, extent(), &hand.presses)
        .expect("registered");

    // The interaction is taken away: no release ever arrives.
    hand.frame();
    hand.cancel();
    screen
        .update(&world, &components, extent(), &hand.presses)
        .expect("registered");
    assert!(!screen.is_held(entity));

    hand.frame();
    hand.release();
    screen
        .update(&world, &components, extent(), &hand.presses)
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
    let screen = updated(&world, &at(WIDTH / 2.0, HEIGHT / 2.0));
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
    let screen = updated(&world, &at(WIDTH / 2.0, HEIGHT / 2.0));
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
    let screen = updated(&world, &at(WIDTH / 2.0, HEIGHT / 2.0));
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

    let all = updated(&world, &Hand::mouse());
    let spread = |screen: &ScreenUi, of: &[EntityId]| {
        of.iter()
            .filter_map(|entity| screen.rect(*entity))
            .map(|rect| rect.center[1])
            .fold(f32::NEG_INFINITY, f32::max)
    };
    let three = spread(&all, &entries);

    world.get_mut(entries[1]).expect("there").disabled = true;
    let two = updated(&world, &Hand::mouse());
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
        .update(&world, &components, extent(), &Presses::default())
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
            &Presses::default(),
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
                &at(width - 1.0, height - 1.0).presses,
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
    let screen = updated(&world, &at(WIDTH / 2.0, HEIGHT / 2.0));
    assert!(!screen.is_hovered(entity));
    assert!(!screen.captures_pointer(), "a HUD swallowed the pointer");
}
