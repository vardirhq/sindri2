//! The part of the environment that describes the person at the controls.
//!
//! Split from `environment.rs` for the reason `surface::person` is split from
//! `surface::call`: a pointer, a thumb and the screen's shape are facts about
//! whoever is playing, decided by the host before a script ran. The rest of the
//! environment describes what a script may *do*.

use decay_semantic::{Environment, FunctionType, HostType, Type};

use crate::surface::{
    POINTER, POINTER_QUERIES, POINTER_VALUES, PointerValue, STICK, STICK_VALUES, StickValue, TOUCH,
    TOUCH_CALLS, TOUCH_COUNT, VIEWPORT, VIEWPORT_VALUES,
};

/// The shape of the screen the host is drawing into.
pub(super) fn add_viewport_surface(environment: &mut Environment) {
    let mut viewport = HostType::new();
    for (name, _) in VIEWPORT_VALUES {
        viewport = viewport.with_value(*name, Type::F32);
    }
    environment.add_type(VIEWPORT, viewport);
    environment.add_value(VIEWPORT, Type::Named(VIEWPORT.to_owned()));
}

/// Where the person is pointing, and the fingers behind it.
pub(super) fn add_pointer_surface(environment: &mut Environment) {
    let mut pointer = HostType::new();
    for (name, value) in POINTER_VALUES {
        pointer = pointer.with_value(
            *name,
            match value {
                PointerValue::X
                | PointerValue::Y
                | PointerValue::OverlayX
                | PointerValue::OverlayY => Type::F32,
                PointerValue::Inside | PointerValue::OverUi => Type::Bool,
            },
        );
    }
    for (name, _) in POINTER_QUERIES {
        pointer = pointer.with_function(
            *name,
            FunctionType {
                params: vec![Type::String],
                return_type: Type::Bool,
            },
        );
    }
    environment.add_type(POINTER, pointer);
    environment.add_value(POINTER, Type::Named(POINTER.to_owned()));

    let mut touch = HostType::new().with_value(TOUCH_COUNT, Type::F32);
    for (name, _) in TOUCH_CALLS {
        touch = touch.with_function(
            *name,
            FunctionType {
                params: vec![Type::F32],
                return_type: Type::F32,
            },
        );
    }
    environment.add_type(TOUCH, touch);
    environment.add_value(TOUCH, Type::Named(TOUCH.to_owned()));

    let mut stick = HostType::new();
    for (name, value) in STICK_VALUES {
        stick = stick.with_value(
            *name,
            match value {
                StickValue::Held => Type::Bool,
                StickValue::X | StickValue::Y | StickValue::AnchorX | StickValue::AnchorY => {
                    Type::F32
                }
            },
        );
    }
    environment.add_type(STICK, stick);
    environment.add_value(STICK, Type::Named(STICK.to_owned()));
}
