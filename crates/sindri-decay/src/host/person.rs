//! What the host tells a script about the person at the other end.
//!
//! The steering stick, the gestures, the block under the pointer, the pointer
//! itself and the fingers behind it. Split out of the host because they are one
//! subject and the host is not: everything here answers "what is the person
//! doing", and none of it touches the world.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};

use super::WorldHost;
use super::convert::{as_f32, number};
use crate::surface::{
    AimValue, CameraCall, CameraValue, GestureValue, PointerValue, StickValue, TouchCall,
};

impl WorldHost<'_> {
    /// What the steering finger is asking for.
    ///
    /// The host computes it rather than the script, because anchoring, the
    /// clamp past the radius and the dead zone are the same three decisions in
    /// every game that has ever needed a stick -- and a script doing the
    /// subtraction itself gets a slightly different feel and its own bugs.
    pub(super) fn stick_value(&self, value: StickValue) -> Value {
        let stick = self.context.input.stick();
        let pushed = stick.value();
        match value {
            StickValue::X => Value::Number(f64::from(pushed[0])),
            StickValue::Y => Value::Number(f64::from(pushed[1])),
            StickValue::Held => Value::Bool(stick.is_engaged()),
            // Zero when nothing is holding it, like a pointer position read
            // from outside the window: a script that cares asks `held` first.
            StickValue::AnchorX => Value::Number(f64::from(
                stick
                    .anchor(self.context.input.presses())
                    .unwrap_or([0.0, 0.0])[0],
            )),
            StickValue::AnchorY => Value::Number(f64::from(
                stick
                    .anchor(self.context.input.presses())
                    .unwrap_or([0.0, 0.0])[1],
            )),
        }
    }

    /// What the person is pointing at, in a world made of blocks.
    ///
    /// Every cell reads zero when nothing was hit, which is why `hit` exists
    /// and is not a convenience: zero is a real cell, and a script that
    /// skipped the question would build a tower at the origin every time the
    /// pointer left the world. Reporting it as an error instead would be
    /// wrong -- pointing at the sky is an ordinary thing to do.
    /// How far this frame's drag asks the camera to move.
    ///
    /// Zero when nothing is being dragged, and zero rather than an error when
    /// the host runs no camera at all: a script adding a pan every frame is
    /// doing the right thing, and adding nothing is the right answer.
    pub(super) fn camera_value(&self, value: CameraValue) -> Value {
        let pan = self.camera_pan.unwrap_or([0.0; 3]);
        Value::Number(f64::from(match value {
            CameraValue::PanX => pan[0],
            CameraValue::PanY => pan[1],
            CameraValue::PanZ => pan[2],
        }))
    }

    /// Applies gameplay intent to the authored world camera behavior.
    pub(super) fn camera_call(
        &mut self,
        call: CameraCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        let amount = number(path, args.first().unwrap_or(&Value::Null))?;
        if !amount.is_finite() || amount < 0.0 {
            return Err(RuntimeError::Host(format!(
                "{} takes a finite, non-negative trauma amount",
                path.dotted()
            )));
        }
        match call {
            CameraCall::AddTrauma => {
                if !sindri_scene::add_camera_trauma(self.world, as_f32(amount)) {
                    return Err(RuntimeError::Host(format!(
                        "{} needs exactly one authored camera with sindri.camera.behavior",
                        path.dotted()
                    )));
                }
                Ok(Value::Unit)
            }
        }
    }

    /// What the person just did.
    ///
    /// Nothing is the common answer, and every coordinate reads zero then --
    /// a real place on the screen -- so each gesture is guarded by its own
    /// question the way `Aim.hit` guards a cell.
    pub(super) fn gesture_value(&self, value: GestureValue) -> Value {
        use GestureValue::{
            DragX, DragY, Dragging, Held, HoldX, HoldY, Pinch, Pinching, TapX, TapY, Tapped,
        };
        let Some(gestures) = self.gestures else {
            return match value {
                Tapped | Held | Dragging | Pinching => Value::Bool(false),
                // A pinch that did not happen is a scale of one, not of zero:
                // zero would be a camera collapsing to a point the first frame
                // a script multiplied by it without asking.
                Pinch => Value::Number(1.0),
                _ => Value::Number(0.0),
            };
        };
        let at = |point: Option<[f32; 2]>, axis: usize| {
            Value::Number(point.map_or(0.0, |point| f64::from(point[axis])))
        };
        match value {
            Tapped => Value::Bool(gestures.tap().is_some()),
            TapX => at(gestures.tap(), 0),
            TapY => at(gestures.tap(), 1),
            Held => Value::Bool(gestures.long_press().is_some()),
            HoldX => at(gestures.long_press(), 0),
            HoldY => at(gestures.long_press(), 1),
            Dragging => Value::Bool(gestures.drag().is_some()),
            DragX => at(gestures.drag(), 0),
            DragY => at(gestures.drag(), 1),
            Pinching => Value::Bool(gestures.pinch().is_some()),
            Pinch => Value::Number(f64::from(gestures.pinch().unwrap_or(1.0))),
        }
    }

    pub(super) fn aim_value(&self, value: AimValue) -> Value {
        let Some(aim) = self.aim else {
            return match value {
                AimValue::Hit => Value::Bool(false),
                _ => Value::Number(0.0),
            };
        };
        match value {
            AimValue::Hit => Value::Bool(true),
            AimValue::X => Value::Number(f64::from(aim.cell.x)),
            AimValue::Y => Value::Number(f64::from(aim.cell.y)),
            AimValue::Z => Value::Number(f64::from(aim.cell.z)),
            AimValue::PlaceX => Value::Number(f64::from(aim.against.x)),
            AimValue::PlaceY => Value::Number(f64::from(aim.against.y)),
            AimValue::PlaceZ => Value::Number(f64::from(aim.against.z)),
        }
    }

    pub(super) fn pointer_value(&self, value: PointerValue) -> Value {
        let position = self.context.input.pointer_position();
        match value {
            PointerValue::Inside => Value::Bool(position.is_some()),
            // False with no screen UI running, rather than an error: a host
            // with no UI has no element to take the pointer, which is a true
            // answer rather than a missing one.
            PointerValue::OverUi => Value::Bool(
                self.screen_ui
                    .is_some_and(sindri_scene::ScreenUi::captures_pointer),
            ),
            PointerValue::X => Value::Number(f64::from(position.unwrap_or([0.0, 0.0])[0])),
            PointerValue::Y => Value::Number(f64::from(position.unwrap_or([0.0, 0.0])[1])),
            // Zero with no screen UI running, for the same reason a position
            // read while the pointer is outside reads zero: the overlay is
            // where the UI is laid out, and a host laying out none has no
            // overlay to answer about. A script that cares asks `inside`.
            PointerValue::OverlayX => Value::Number(f64::from(
                self.screen_ui
                    .and_then(sindri_scene::ScreenUi::pointer_overlay)
                    .unwrap_or([0.0, 0.0])[0],
            )),
            PointerValue::OverlayY => Value::Number(f64::from(
                self.screen_ui
                    .and_then(sindri_scene::ScreenUi::pointer_overlay)
                    .unwrap_or([0.0, 0.0])[1],
            )),
        }
    }

    /// Where one finger is.
    pub(super) fn touch_call(
        &self,
        call: TouchCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        let index = number(path, args.first().unwrap_or(&Value::Null))?;
        if !index.is_finite() || index.fract() != 0.0 || index < 0.0 {
            return Err(RuntimeError::Host(format!(
                "{} takes which finger, counting from zero, and the script gave {index}",
                path.dotted()
            )));
        }
        // Guarded above: finite, non-negative, and whole.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let position = self.context.input.touch_at(index as usize).ok_or_else(|| {
            // Named rather than answered with zero: a script reading finger
            // three when two are down has a bound that is wrong, and a zero
            // would read as a finger in the corner of the screen.
            RuntimeError::Host(format!(
                "{} was asked for finger {index}, and {} are down",
                path.dotted(),
                self.context.input.touch_count()
            ))
        })?;
        Ok(Value::Number(f64::from(match call {
            TouchCall::X => position[0],
            TouchCall::Y => position[1],
        })))
    }
}
