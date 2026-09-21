//! Scene-view-style camera controls for the interactive Voxel Lab.

use std::collections::BTreeMap;
use std::f32::consts::{FRAC_PI_4, FRAC_PI_6};

use glam::{Vec2, Vec3};
use sindri_platform::{InputEvent, Key, MouseButton};

const ORBIT_SENSITIVITY: f32 = 0.008;
const ZOOM_SENSITIVITY: f32 = 0.002;
const MIN_PITCH: f32 = -1.5;
const MAX_PITCH: f32 = 1.5;
const MIN_HALF_HEIGHT: f32 = 3.0;
// The lab deliberately renders a bounded residency window. Do not let camera
// controls pull its authoritative-but-undrawn outer halo into view again.
const MAX_HALF_HEIGHT: f32 = 20.0;
const TAP_SLOP: f32 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LabCamera {
    pub focus: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub half_height: f32,
}

impl Default for LabCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::new(0.0, 3.0, 0.0),
            yaw: FRAC_PI_4,
            pitch: FRAC_PI_6,
            half_height: 20.0,
        }
    }
}

impl LabCamera {
    fn forward(self) -> Vec3 {
        -Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
    }

    fn right(self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize_or_zero()
    }

    fn up(self) -> Vec3 {
        self.right().cross(self.forward()).normalize_or_zero()
    }

    fn orbit(&mut self, delta: Vec2) {
        self.yaw = (self.yaw + delta.x * ORBIT_SENSITIVITY) % std::f32::consts::TAU;
        self.pitch = (self.pitch + delta.y * ORBIT_SENSITIVITY).clamp(MIN_PITCH, MAX_PITCH);
    }

    fn pan(&mut self, delta: Vec2, viewport_height: f32) {
        let units_per_pixel = self.half_height * 2.0 / viewport_height.max(1.0);
        self.focus += (-self.right() * delta.x + self.up() * delta.y) * units_per_pixel;
    }

    fn zoom_scroll(&mut self, delta: f32) {
        self.zoom_factor((-delta * ZOOM_SENSITIVITY).exp());
    }

    fn zoom_factor(&mut self, factor: f32) {
        self.half_height = (self.half_height * factor).clamp(MIN_HALF_HEIGHT, MAX_HALF_HEIGHT);
    }
}

#[derive(Clone, Copy)]
struct PairSample {
    centre: Vec2,
    distance: f32,
}

/// Translates desktop and touch gestures into the same orbit/pan/zoom model as
/// the editor Scene view.
pub(super) struct CameraControls {
    camera: LabCamera,
    viewport_height: f32,
    pointer: Option<Vec2>,
    drag: Option<MouseButton>,
    shift: bool,
    touches: BTreeMap<u64, Vec2>,
    touch_start: Option<Vec2>,
    touch_moved: bool,
}

impl Default for CameraControls {
    fn default() -> Self {
        Self {
            camera: LabCamera::default(),
            viewport_height: 1.0,
            pointer: None,
            drag: None,
            shift: false,
            touches: BTreeMap::new(),
            touch_start: None,
            touch_moved: false,
        }
    }
}

impl CameraControls {
    #[must_use]
    pub fn camera(&self) -> LabCamera {
        self.camera
    }

    pub fn set_viewport_height(&mut self, height: u32) {
        #[allow(clippy::cast_precision_loss)]
        let height = height.max(1) as f32;
        self.viewport_height = height;
    }

    /// Returns `true` when an unmoved single-finger tap should dig.
    pub fn input(&mut self, event: InputEvent) -> bool {
        match event {
            InputEvent::KeyPressed(Key::ShiftLeft | Key::ShiftRight) => self.shift = true,
            InputEvent::KeyReleased(Key::ShiftLeft | Key::ShiftRight) => self.shift = false,
            InputEvent::ButtonPressed(button) => self.drag = Some(button),
            InputEvent::ButtonReleased(button) if self.drag == Some(button) => self.drag = None,
            InputEvent::PointerMoved { x, y } => self.pointer_moved(Vec2::new(x, y)),
            InputEvent::PointerLeft => {
                self.pointer = None;
                self.drag = None;
            }
            InputEvent::Scrolled { y, .. } => self.camera.zoom_scroll(y),
            InputEvent::TouchStarted { id, x, y } => {
                self.touch_started(id, Vec2::new(x, y));
            }
            InputEvent::TouchMoved { id, x, y } => self.touch_moved(id, Vec2::new(x, y)),
            InputEvent::TouchEnded { id } => return self.touch_ended(id),
            InputEvent::FocusChanged(false) => self.cancel_gestures(),
            _ => {}
        }
        false
    }

    fn pointer_moved(&mut self, at: Vec2) {
        let delta = self.pointer.map_or(Vec2::ZERO, |last| at - last);
        self.pointer = Some(at);
        let Some(button) = self.drag else {
            return;
        };
        if self.shift || button == MouseButton::Middle {
            self.camera.pan(delta, self.viewport_height);
        } else if matches!(button, MouseButton::Left | MouseButton::Right) {
            self.camera.orbit(delta);
        }
    }

    fn touch_started(&mut self, id: u64, at: Vec2) {
        if self.touches.len() >= 2 && !self.touches.contains_key(&id) {
            return;
        }
        if self.touches.is_empty() {
            self.touch_start = Some(at);
            self.touch_moved = false;
        } else {
            self.touch_moved = true;
        }
        self.touches.insert(id, at);
    }

    fn touch_moved(&mut self, id: u64, at: Vec2) {
        let Some(last) = self.touches.get(&id).copied() else {
            return;
        };
        if self.touches.len() == 1 {
            if self
                .touch_start
                .is_some_and(|start| start.distance(at) > TAP_SLOP)
            {
                self.touch_moved = true;
            }
            self.camera.orbit(at - last);
            self.touches.insert(id, at);
            return;
        }

        let previous = self.pair_sample();
        self.touches.insert(id, at);
        let current = self.pair_sample();
        if let (Some(previous), Some(current)) = (previous, current) {
            self.camera
                .pan(current.centre - previous.centre, self.viewport_height);
            if current.distance > f32::EPSILON {
                self.camera
                    .zoom_factor(previous.distance / current.distance);
            }
        }
        self.touch_moved = true;
    }

    fn touch_ended(&mut self, id: u64) -> bool {
        if !self.touches.contains_key(&id) {
            return false;
        }
        let was_tap = self.touches.len() == 1 && !self.touch_moved;
        self.touches.remove(&id);
        if self.touches.is_empty() {
            self.touch_start = None;
            self.touch_moved = false;
        }
        was_tap
    }

    fn pair_sample(&self) -> Option<PairSample> {
        let mut touches = self.touches.values().copied();
        let first = touches.next()?;
        let second = touches.next()?;
        Some(PairSample {
            centre: (first + second) * 0.5,
            distance: first.distance(second),
        })
    }

    fn cancel_gestures(&mut self) {
        self.drag = None;
        self.touches.clear();
        self.touch_start = None;
        self.touch_moved = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer(x: f32, y: f32) -> InputEvent {
        InputEvent::PointerMoved { x, y }
    }

    #[test]
    fn primary_drag_orbits_like_the_scene_view() {
        let mut controls = CameraControls::default();
        let before = controls.camera();
        controls.input(pointer(10.0, 10.0));
        controls.input(InputEvent::ButtonPressed(MouseButton::Left));
        controls.input(pointer(30.0, 20.0));

        let after = controls.camera();
        assert!(after.focus.distance(before.focus) <= f32::EPSILON);
        assert!(after.yaw > before.yaw);
        assert!(after.pitch > before.pitch);
    }

    #[test]
    fn shift_drag_pans_without_orbiting() {
        let mut controls = CameraControls::default();
        controls.set_viewport_height(500);
        let before = controls.camera();
        controls.input(pointer(10.0, 10.0));
        controls.input(InputEvent::KeyPressed(Key::ShiftLeft));
        controls.input(InputEvent::ButtonPressed(MouseButton::Left));
        controls.input(pointer(30.0, 20.0));

        let after = controls.camera();
        assert!(after.focus.distance(before.focus) > f32::EPSILON);
        assert!((after.yaw - before.yaw).abs() <= f32::EPSILON);
        assert!((after.pitch - before.pitch).abs() <= f32::EPSILON);
    }

    #[test]
    fn wheel_and_pinch_zoom_proportionally() {
        let mut controls = CameraControls::default();
        let before = controls.camera().half_height;
        controls.input(InputEvent::Scrolled { x: 0.0, y: 50.0 });
        assert!(controls.camera().half_height < before);

        let before_pinch = controls.camera().half_height;
        controls.input(InputEvent::TouchStarted {
            id: 1,
            x: 0.0,
            y: 0.0,
        });
        controls.input(InputEvent::TouchStarted {
            id: 2,
            x: 10.0,
            y: 0.0,
        });
        let focus_before_pinch = controls.camera().focus;
        controls.input(InputEvent::TouchMoved {
            id: 2,
            x: 20.0,
            y: 0.0,
        });
        assert!(controls.camera().half_height < before_pinch);
        assert!(controls.camera().focus.distance(focus_before_pinch) > f32::EPSILON);
    }

    #[test]
    fn only_an_unmoved_single_finger_tap_digs() {
        let mut controls = CameraControls::default();
        controls.input(InputEvent::TouchStarted {
            id: 1,
            x: 10.0,
            y: 10.0,
        });
        assert!(controls.input(InputEvent::TouchEnded { id: 1 }));

        controls.input(InputEvent::TouchStarted {
            id: 2,
            x: 10.0,
            y: 10.0,
        });
        controls.input(InputEvent::TouchMoved {
            id: 2,
            x: 30.0,
            y: 10.0,
        });
        assert!(!controls.input(InputEvent::TouchEnded { id: 2 }));
    }
}
