//! CSS transitions: a value that changes eases to its new value instead of
//! jumping, over the time a `transition` declaration gives it.
//!
//! ```css
//! button { background: #1f2937; transition: background 150ms ease-out; }
//! button:hover { background: #374151; }
//! ```
//!
//! As in CSS, a transition starts when a property's computed value changes,
//! and starts from whatever is showing at that moment, so a hover that ends
//! halfway through its fade turns round smoothly. Colours, lengths and plain
//! numbers ease; anything else changes at once.

use std::collections::BTreeMap;

use sindri_core::EntityId;

/// How one property, or `all`, eases when it changes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Timing {
    duration: f32,
    delay: f32,
    easing: Easing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Easing {
    Linear,
    /// A cubic Bézier through (0,0), (x1,y1), (x2,y2), (1,1), as CSS's
    /// `cubic-bezier()` and its named curves are.
    Bezier(f32, f32, f32, f32),
}

impl Easing {
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "linear" => Self::Linear,
            "ease" => Self::Bezier(0.25, 0.1, 0.25, 1.0),
            "ease-in" => Self::Bezier(0.42, 0.0, 1.0, 1.0),
            "ease-out" => Self::Bezier(0.0, 0.0, 0.58, 1.0),
            "ease-in-out" => Self::Bezier(0.42, 0.0, 0.58, 1.0),
            _ => return None,
        })
    }

    /// Progress along the curve at time fraction `t`.
    fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let Self::Bezier(x1, y1, x2, y2) = self else {
            return t;
        };
        let bezier = |a: f32, b: f32, s: f32| {
            let inverse = 1.0 - s;
            3.0 * inverse * inverse * s * a + 3.0 * inverse * s * s * b + s * s * s
        };
        // Find the curve parameter whose x is `t` by bisection: always
        // converges, and twenty steps is far below a pixel.
        let (mut low, mut high) = (0.0_f32, 1.0_f32);
        for _ in 0..20 {
            let middle = f32::midpoint(low, high);
            if bezier(x1, x2, middle) < t {
                low = middle;
            } else {
                high = middle;
            }
        }
        bezier(y1, y2, f32::midpoint(low, high))
    }
}

/// Reads `transition: background 150ms ease-out, color 0.2s` into what each
/// property does. A malformed item is skipped rather than failing the frame.
fn parse_transitions(value: &str) -> BTreeMap<String, Timing> {
    let mut timings = BTreeMap::new();
    for item in value.split(',') {
        let mut property = None;
        let mut times = Vec::new();
        let mut easing = Easing::Bezier(0.25, 0.1, 0.25, 1.0);
        for word in item.split_whitespace() {
            if let Some(seconds) = time(word) {
                times.push(seconds);
            } else if let Some(named) = Easing::named(word) {
                easing = named;
            } else if property.is_none() {
                property = Some(word.to_owned());
            }
        }
        let (Some(property), Some(&duration)) = (property, times.first()) else {
            continue;
        };
        timings.insert(
            property,
            Timing {
                duration: duration.max(0.0),
                delay: times.get(1).copied().unwrap_or(0.0),
                easing,
            },
        );
    }
    timings
}

fn time(word: &str) -> Option<f32> {
    let seconds = if let Some(ms) = word.strip_suffix("ms") {
        ms.parse::<f32>().ok()? / 1000.0
    } else {
        word.strip_suffix('s')?.parse::<f32>().ok()?
    };
    seconds.is_finite().then_some(seconds)
}

/// A value that can be part way between two others.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mixable {
    /// sRGB channels and alpha, 0 to 1, as written.
    Colour([f32; 4]),
    /// A number and the unit it was written in.
    Length(f32, Unit),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Unit {
    None,
    Pixels,
    ViewWidth,
    ViewHeight,
    Percent,
}

impl Mixable {
    fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if let Some(colour) = colour(value) {
            return Some(Self::Colour(colour));
        }
        for (suffix, unit) in [
            ("px", Unit::Pixels),
            ("vw", Unit::ViewWidth),
            ("vh", Unit::ViewHeight),
            ("%", Unit::Percent),
        ] {
            if let Some(number) = value.strip_suffix(suffix) {
                return number.trim().parse().ok().map(|n| Self::Length(n, unit));
            }
        }
        value.parse().ok().map(|n| Self::Length(n, Unit::None))
    }

    fn mix(self, other: Self, t: f32) -> Option<Self> {
        let lerp = |a: f32, b: f32| a + (b - a) * t;
        match (self, other) {
            (Self::Colour(a), Self::Colour(b)) => Some(Self::Colour([
                lerp(a[0], b[0]),
                lerp(a[1], b[1]),
                lerp(a[2], b[2]),
                lerp(a[3], b[3]),
            ])),
            (Self::Length(a, unit), Self::Length(b, other_unit)) if unit == other_unit => {
                Some(Self::Length(lerp(a, b), unit))
            }
            _ => None,
        }
    }

    fn write(self) -> String {
        match self {
            Self::Colour(channels) => {
                let byte = |channel: f32| {
                    // Channels are clamped to 0..=1 first, so the product fits.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let byte = (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
                    byte
                };
                format!(
                    "#{:02x}{:02x}{:02x}{:02x}",
                    byte(channels[0]),
                    byte(channels[1]),
                    byte(channels[2]),
                    byte(channels[3])
                )
            }
            Self::Length(number, unit) => {
                let suffix = match unit {
                    Unit::None => "",
                    Unit::Pixels => "px",
                    Unit::ViewWidth => "vw",
                    Unit::ViewHeight => "vh",
                    Unit::Percent => "%",
                };
                format!("{number}{suffix}")
            }
        }
    }
}

/// The sRGB colours Weave accepts, as channels from 0 to 1.
fn colour(value: &str) -> Option<[f32; 4]> {
    match value {
        "transparent" => return Some([0.0; 4]),
        "black" => return Some([0.0, 0.0, 0.0, 1.0]),
        "white" => return Some([1.0; 4]),
        _ => {}
    }
    let hex = value.strip_prefix('#')?;
    let digit = |byte: u8| char::from(byte).to_digit(16).map(|d| d as u8);
    let mut channels = [255_u8; 4];
    match hex.len() {
        3 | 4 => {
            for (index, byte) in hex.bytes().enumerate() {
                channels[index] = digit(byte)? * 17;
            }
        }
        6 | 8 => {
            for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
                channels[index] = digit(pair[0])? * 16 + digit(pair[1])?;
            }
        }
        _ => return None,
    }
    Some(channels.map(|channel| f32::from(channel) / 255.0))
}

/// One property on its way from one value to another.
#[derive(Clone, Debug)]
struct Track {
    from: Mixable,
    to: Mixable,
    /// The value it is heading for, as the stylesheet wrote it.
    target: String,
    started: f32,
    timing: Timing,
}

impl Track {
    fn value_at(&self, now: f32) -> Option<Mixable> {
        let elapsed = now - self.started - self.timing.delay;
        if elapsed <= 0.0 {
            return Some(self.from);
        }
        if self.timing.duration <= 0.0 || elapsed >= self.timing.duration {
            return None;
        }
        let progress = self.timing.easing.apply(elapsed / self.timing.duration);
        self.from.mix(self.to, progress)
    }
}

/// What the presentation remembers between frames, so a change can ease
/// instead of jump. One per host, living as long as the running game does.
#[derive(Clone, Debug, Default)]
pub struct Transitions {
    now: f32,
    /// What each property was last computed as, by stylesheet, entity and
    /// property: the stylesheet's value, before any easing.
    last: BTreeMap<(usize, EntityId, String), String>,
    running: BTreeMap<(usize, EntityId, String), Track>,
}

impl Transitions {
    /// Moves the clock on. Call once a frame, before presenting.
    pub fn advance(&mut self, seconds: f32) {
        if seconds.is_finite() && seconds > 0.0 {
            self.now += seconds;
        }
    }

    /// Whether anything is still easing, so a host that only redraws on
    /// change knows to keep drawing.
    #[must_use]
    pub fn animating(&self) -> bool {
        !self.running.is_empty()
    }

    /// Replaces each declaration that is easing with where it has got to,
    /// and starts easing any transitioned property whose value changed.
    pub(crate) fn ease(
        &mut self,
        sheet: usize,
        entity: EntityId,
        declarations: &mut BTreeMap<String, String>,
    ) {
        let timings = declarations
            .get("transition")
            .map(|value| parse_transitions(value))
            .unwrap_or_default();
        let now = self.now;
        for (property, value) in declarations.iter_mut() {
            if property == "transition" {
                continue;
            }
            let key = (sheet, entity, property.clone());
            let previous = self.last.insert(key.clone(), value.clone());
            let timing = timings
                .get(property.as_str())
                .or_else(|| timings.get("all"))
                .copied();
            let changed = previous.as_ref().is_some_and(|previous| previous != value);
            if changed && let Some(timing) = timing {
                // From what is showing now: the running track's position if
                // there is one, or the value it held until this frame.
                let showing = self
                    .running
                    .get(&key)
                    .and_then(|track| track.value_at(now))
                    .or_else(|| previous.as_deref().and_then(Mixable::parse));
                if let (Some(from), Some(to)) = (showing, Mixable::parse(value)) {
                    self.running.insert(
                        key.clone(),
                        Track {
                            from,
                            to,
                            target: value.clone(),
                            started: now,
                            timing,
                        },
                    );
                }
            }
            if let Some(track) = self.running.get(&key) {
                match (track.target == *value)
                    .then(|| track.value_at(now))
                    .flatten()
                {
                    Some(showing) => *value = showing.write(),
                    None => {
                        self.running.remove(&key);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity() -> EntityId {
        sindri_core::World::default().spawn(sindri_core::EntityData::default())
    }

    fn frame(transitions: &mut Transitions, entity: EntityId, background: &str) -> String {
        let mut declarations = BTreeMap::from([
            ("background".to_owned(), background.to_owned()),
            (
                "transition".to_owned(),
                "background 100ms linear".to_owned(),
            ),
        ]);
        transitions.ease(0, entity, &mut declarations);
        declarations["background"].clone()
    }

    #[test]
    fn a_changed_colour_eases_to_its_new_value() {
        let mut transitions = Transitions::default();
        let button = entity();
        assert_eq!(frame(&mut transitions, button, "#000000"), "#000000");
        // The pointer arrives: the target changes, and the first frame still
        // shows where it starts.
        assert_eq!(frame(&mut transitions, button, "#ffffff"), "#000000ff");
        transitions.advance(0.05);
        assert_eq!(frame(&mut transitions, button, "#ffffff"), "#808080ff");
        assert!(transitions.animating());
        transitions.advance(0.06);
        assert_eq!(frame(&mut transitions, button, "#ffffff"), "#ffffff");
        assert!(!transitions.animating());
    }

    #[test]
    fn leaving_halfway_turns_round_from_where_it_got_to() {
        let mut transitions = Transitions::default();
        let button = entity();
        frame(&mut transitions, button, "#000000");
        frame(&mut transitions, button, "#ffffff");
        transitions.advance(0.05);
        assert_eq!(frame(&mut transitions, button, "#ffffff"), "#808080ff");
        // The pointer leaves: back towards black, from grey, not from white.
        assert_eq!(frame(&mut transitions, button, "#000000"), "#808080ff");
        transitions.advance(0.05);
        assert_eq!(frame(&mut transitions, button, "#000000"), "#404040ff");
    }

    #[test]
    fn lengths_ease_in_their_own_unit_and_other_values_jump() {
        fn at(
            transitions: &mut Transitions,
            panel: EntityId,
            width: &str,
            anchor: &str,
        ) -> (String, String) {
            let mut declarations = BTreeMap::from([
                ("width".to_owned(), width.to_owned()),
                ("anchor".to_owned(), anchor.to_owned()),
                ("transition".to_owned(), "all 1s linear".to_owned()),
            ]);
            transitions.ease(0, panel, &mut declarations);
            (
                declarations["width"].clone(),
                declarations["anchor"].clone(),
            )
        }
        let mut transitions = Transitions::default();
        let panel = entity();
        at(&mut transitions, panel, "100px", "left");
        at(&mut transitions, panel, "200px", "right");
        transitions.advance(0.25);
        assert_eq!(
            at(&mut transitions, panel, "200px", "right"),
            ("125px".to_owned(), "right".to_owned())
        );
    }

    #[test]
    fn the_named_curves_start_and_end_where_they_should() {
        for name in ["ease", "ease-in", "ease-out", "ease-in-out", "linear"] {
            let easing = Easing::named(name).unwrap();
            assert!(easing.apply(0.0).abs() < 1.0e-3, "{name}");
            assert!((easing.apply(1.0) - 1.0).abs() < 1.0e-3, "{name}");
        }
        let ease_out = Easing::named("ease-out").unwrap();
        assert!(
            ease_out.apply(0.5) > 0.5,
            "ease-out is ahead of linear halfway"
        );
    }

    #[test]
    fn transition_lists_read_durations_delays_and_curves() {
        let timings = parse_transitions("background 150ms ease-out, color 0.2s 50ms");
        assert!((timings["background"].duration - 0.15).abs() < 1.0e-6);
        assert_eq!(
            timings["background"].easing,
            Easing::named("ease-out").unwrap()
        );
        assert!((timings["color"].delay - 0.05).abs() < 1.0e-6);
    }
}
