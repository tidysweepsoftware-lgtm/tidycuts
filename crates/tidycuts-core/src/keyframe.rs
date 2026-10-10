use crate::time::Time;
use serde::{Deserialize, Serialize};

/// How a value moves from one keyframe to the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Hold,
    /// A custom curve like CSS `cubic-bezier(x1, y1, x2, y2)`: the curve
    /// runs from (0, 0) to (1, 1) pulled towards two handles. `x` is time
    /// (kept within 0..=1); `y` is progress and may go past 0 or 1 to
    /// overshoot.
    Bezier {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
}

impl Easing {
    /// Springs a little past the end value and settles back.
    pub const OVERSHOOT: Easing = Easing::Bezier {
        x1: 0.34,
        y1: 1.56,
        x2: 0.64,
        y2: 1.0,
    };

    /// The choices offered in the editor, with their names.
    pub const PRESETS: [(Easing, &'static str); 6] = [
        (Easing::Linear, "Linear"),
        (Easing::EaseIn, "Ease in"),
        (Easing::EaseOut, "Ease out"),
        (Easing::EaseInOut, "Ease in-out"),
        (Easing::Hold, "Hold"),
        (Easing::OVERSHOOT, "Overshoot"),
    ];

    /// Progress (0 at the first keyframe, 1 at the next) at time fraction `t`.
    pub fn apply(self, t: f64) -> f64 {
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::Hold => 0.0,
            Easing::Bezier { x1, y1, x2, y2 } => cubic_bezier(x1, y1, x2, y2, t),
        }
    }

    /// The bezier handles that draw this curve (exact for ease in and out,
    /// a close match for ease in-out), for editing it as a custom one.
    pub fn handles(self) -> [f64; 4] {
        match self {
            Easing::Linear | Easing::Hold => [0.0, 0.0, 1.0, 1.0],
            Easing::EaseIn => [1.0 / 3.0, 0.0, 2.0 / 3.0, 1.0 / 3.0],
            Easing::EaseOut => [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 1.0],
            Easing::EaseInOut => [0.455, 0.03, 0.515, 0.955],
            Easing::Bezier { x1, y1, x2, y2 } => [x1, y1, x2, y2],
        }
    }

    /// A custom curve with handles kept in range (time within 0..=1).
    pub fn bezier(h: [f64; 4]) -> Easing {
        let y = |v: f64| {
            if v.is_finite() {
                v.clamp(-1.0, 2.0)
            } else {
                0.0
            }
        };
        let x = |v: f64| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        Easing::Bezier {
            x1: x(h[0]),
            y1: y(h[1]),
            x2: x(h[2]),
            y2: y(h[3]),
        }
    }

    /// The name of the preset this is, if it is one.
    pub fn preset_name(self) -> Option<&'static str> {
        Self::PRESETS
            .iter()
            .find(|(e, _)| *e == self)
            .map(|(_, n)| *n)
    }
}

/// CSS-style cubic bezier easing: finds where the curve is at time `t` and
/// returns its height there.
fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let (x1, x2) = (x1.clamp(0.0, 1.0), x2.clamp(0.0, 1.0));
    // Polynomial coefficients of each coordinate.
    let (cx, cy) = (3.0 * x1, 3.0 * y1);
    let (bx, by) = (3.0 * (x2 - x1) - cx, 3.0 * (y2 - y1) - cy);
    let (ax, ay) = (1.0 - cx - bx, 1.0 - cy - by);
    let x_at = |u: f64| ((ax * u + bx) * u + cx) * u;
    let dx_at = |u: f64| (3.0 * ax * u + 2.0 * bx) * u + cx;
    // Newton's method, falling back to bisection where the slope is flat.
    let mut u = t;
    let mut solved = false;
    for _ in 0..8 {
        let err = x_at(u) - t;
        if err.abs() < 1e-9 {
            solved = true;
            break;
        }
        let d = dx_at(u);
        if d.abs() < 1e-6 {
            break;
        }
        u -= err / d;
        if !(0.0..=1.0).contains(&u) {
            break;
        }
    }
    if !solved {
        let (mut lo, mut hi) = (0.0, 1.0);
        u = t;
        for _ in 0..60 {
            let x = x_at(u);
            if (x - t).abs() < 1e-9 {
                break;
            }
            if x < t {
                lo = u;
            } else {
                hi = u;
            }
            u = 0.5 * (lo + hi);
        }
    }
    ((ay * u + by) * u + cy) * u
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    /// Offset from the start of the clip.
    pub at: Time,
    pub value: f64,
    #[serde(default)]
    pub easing: Easing,
}

/// A clip property that is either a constant or animated by keyframes.
/// Nearly every property in the inspector is one of these, matching other editors
/// where each control has its own keyframe button.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Animatable {
    pub value: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keyframes: Vec<Keyframe>,
}

impl Animatable {
    pub fn constant(value: f64) -> Self {
        Animatable {
            value,
            keyframes: Vec::new(),
        }
    }

    pub fn is_animated(&self) -> bool {
        !self.keyframes.is_empty()
    }

    /// Value at `at` (relative to the clip start).
    pub fn value_at(&self, at: Time) -> f64 {
        let kfs = &self.keyframes;
        match kfs.len() {
            0 => self.value,
            1 => kfs[0].value,
            _ => {
                if at <= kfs[0].at {
                    return kfs[0].value;
                }
                for pair in kfs.windows(2) {
                    let (a, b) = (&pair[0], &pair[1]);
                    if at < b.at {
                        let span = (b.at - a.at).0.max(1) as f64;
                        let t = a.easing.apply((at - a.at).0 as f64 / span);
                        return a.value + (b.value - a.value) * t;
                    }
                }
                kfs[kfs.len() - 1].value
            }
        }
    }

    /// Adds a keyframe, or replaces the one already at that time.
    pub fn set_keyframe(&mut self, at: Time, value: f64) {
        match self.keyframes.binary_search_by_key(&at, |k| k.at) {
            Ok(i) => self.keyframes[i].value = value,
            Err(i) => self.keyframes.insert(
                i,
                Keyframe {
                    at,
                    value,
                    easing: Easing::Linear,
                },
            ),
        }
    }

    /// Index of a keyframe within `tolerance` of `at`.
    pub fn keyframe_near(&self, at: Time, tolerance: Time) -> Option<usize> {
        self.keyframes
            .iter()
            .enumerate()
            .filter(|(_, k)| (k.at - at).abs() <= tolerance)
            .min_by_key(|(_, k)| (k.at - at).abs())
            .map(|(i, _)| i)
    }

    /// Changes the value showing at `at`: edits (or adds) a keyframe when the
    /// property is animated, otherwise the constant.
    pub fn set_value_at(&mut self, at: Time, value: f64, tolerance: Time) {
        if !self.is_animated() {
            self.value = value;
        } else if let Some(i) = self.keyframe_near(at, tolerance) {
            self.keyframes[i].value = value;
        } else {
            self.set_keyframe(at, value);
        }
    }

    /// Turns keyframing on (adding one at `at` with the current value) or,
    /// if a keyframe already sits at `at`, removes it. Removing the last
    /// keyframe makes the property constant again at that value.
    pub fn toggle_keyframe(&mut self, at: Time, tolerance: Time) {
        let current = self.value_at(at);
        if let Some(i) = self.keyframe_near(at, tolerance) {
            self.keyframes.remove(i);
            if self.keyframes.is_empty() {
                self.value = current;
            }
        } else {
            self.set_keyframe(at, current);
        }
    }

    /// Re-times the keyframes for a clip whose content now starts `offset`
    /// later and lasts `len`: keyframes outside are dropped and replaced by
    /// boundary keyframes so the motion inside looks the same.
    pub fn rebase(&mut self, offset: Time, len: Time) {
        if !self.is_animated() {
            return;
        }
        let at_start = self.value_at(offset);
        let at_end = self.value_at(offset + len);
        let had_before = self.keyframes.iter().any(|k| k.at < offset);
        let had_after = self.keyframes.iter().any(|k| k.at > offset + len);
        let mut kept: Vec<Keyframe> = self
            .keyframes
            .iter()
            .filter(|k| k.at >= offset && k.at <= offset + len)
            .map(|k| Keyframe {
                at: k.at - offset,
                ..k.clone()
            })
            .collect();
        if had_before && kept.first().is_none_or(|k| k.at > Time::ZERO) {
            kept.insert(
                0,
                Keyframe {
                    at: Time::ZERO,
                    value: at_start,
                    easing: Easing::Linear,
                },
            );
        }
        if had_after && kept.last().is_none_or(|k| k.at < len) {
            kept.push(Keyframe {
                at: len,
                value: at_end,
                easing: Easing::Linear,
            });
        }
        if kept.is_empty() {
            self.value = at_start;
        }
        self.keyframes = kept;
    }

    /// Moves every keyframe to `f(time)`; `f` must keep their order.
    pub fn remap_time(&mut self, f: impl Fn(Time) -> Time) {
        for k in &mut self.keyframes {
            k.at = f(k.at);
        }
        self.keyframes.dedup_by_key(|k| k.at);
    }

    /// Index of the keyframe that starts the stretch of animation playing
    /// at `at` (the one at or before it; the first before the start, the
    /// second-last after the end). `None` without two keyframes.
    pub fn segment_at(&self, at: Time) -> Option<usize> {
        if self.keyframes.len() < 2 {
            return None;
        }
        let i = self.keyframes.iter().rposition(|k| k.at <= at).unwrap_or(0);
        Some(i.min(self.keyframes.len() - 2))
    }

    /// Stretches keyframe times, e.g. after a speed change.
    pub fn scale_time(&mut self, factor: f64) {
        for k in &mut self.keyframes {
            k.at = k.at.scale(factor);
        }
    }

    /// Adds `delta` to the value and to every keyframe.
    pub fn offset(&mut self, delta: f64) {
        self.value += delta;
        for k in &mut self.keyframes {
            k.value += delta;
        }
    }

    pub fn remove_keyframe(&mut self, at: Time) -> bool {
        match self.keyframes.binary_search_by_key(&at, |k| k.at) {
            Ok(i) => {
                self.keyframes.remove(i);
                true
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_between_keyframes() {
        let mut a = Animatable::constant(1.0);
        assert_eq!(a.value_at(Time::from_secs_f64(3.0)), 1.0);
        a.set_keyframe(Time::from_secs_f64(0.0), 0.0);
        a.set_keyframe(Time::from_secs_f64(2.0), 100.0);
        assert_eq!(a.value_at(Time::from_secs_f64(-1.0)), 0.0);
        assert!((a.value_at(Time::from_secs_f64(1.0)) - 50.0).abs() < 1e-9);
        assert_eq!(a.value_at(Time::from_secs_f64(5.0)), 100.0);
    }

    #[test]
    fn rebase_keeps_motion_continuous() {
        let mut a = Animatable::constant(0.0);
        a.set_keyframe(Time(0), 0.0);
        a.set_keyframe(Time(1000), 100.0);
        let mut right = a.clone();
        a.rebase(Time(0), Time(400));
        right.rebase(Time(400), Time(600));
        assert!((a.value_at(Time(400)) - 40.0).abs() < 1e-9);
        assert!((right.value_at(Time(0)) - 40.0).abs() < 1e-9);
        assert!((right.value_at(Time(600)) - 100.0).abs() < 1e-9);
        assert_eq!(right.keyframes.len(), 2);
    }

    #[test]
    fn toggling_keyframes_turns_animation_on_and_off() {
        let mut a = Animatable::constant(5.0);
        a.toggle_keyframe(Time(100), Time(10));
        assert!(a.is_animated());
        a.set_value_at(Time(500), 9.0, Time(10));
        assert_eq!(a.keyframes.len(), 2);
        a.toggle_keyframe(Time(505), Time(10));
        a.toggle_keyframe(Time(100), Time(10));
        assert!(!a.is_animated());
        assert_eq!(a.value, 5.0);
    }

    #[test]
    fn bezier_easing_runs_from_start_to_end() {
        let linear = Easing::bezier([0.0, 0.0, 1.0, 1.0]);
        for t in [0.0, 0.1, 0.37, 0.5, 0.92, 1.0] {
            assert!((linear.apply(t) - t).abs() < 1e-6, "{t}");
        }
        let e = Easing::bezier([0.42, 0.0, 0.58, 1.0]);
        assert!(e.apply(0.0).abs() < 1e-9 && (e.apply(1.0) - 1.0).abs() < 1e-9);
        assert!(e.apply(0.25) < 0.25 && e.apply(0.75) > 0.75);
        assert!((e.apply(0.5) - 0.5).abs() < 1e-6);
        // Overshoot goes past the end value before settling.
        let peak = (1..100)
            .map(|i| Easing::OVERSHOOT.apply(i as f64 / 100.0))
            .fold(0.0, f64::max);
        assert!(peak > 1.05);
        // The handle versions of the built-in curves are close to them.
        for ease in [Easing::EaseIn, Easing::EaseOut, Easing::EaseInOut] {
            let b = Easing::bezier(ease.handles());
            for i in 0..=20 {
                let t = i as f64 / 20.0;
                assert!((b.apply(t) - ease.apply(t)).abs() < 0.03, "{ease:?} {t}");
            }
        }
    }

    #[test]
    fn custom_easing_shapes_the_motion_and_saves() {
        let mut a = Animatable::constant(0.0);
        a.set_keyframe(Time(0), 0.0);
        a.set_keyframe(Time(1000), 100.0);
        a.keyframes[0].easing = Easing::bezier([0.9, 0.0, 1.0, 0.1]);
        assert!(a.value_at(Time(500)) < 10.0);
        let json = serde_json::to_string(&a).unwrap();
        let back: Animatable = serde_json::from_str(&json).unwrap();
        assert_eq!(back, a);
        // Older projects only have the named curves.
        let old: Animatable = serde_json::from_str(
            r#"{"value":0,"keyframes":[{"at":0,"value":0,"easing":"EaseOut"},{"at":5,"value":1}]}"#,
        )
        .unwrap();
        assert_eq!(old.keyframes[0].easing, Easing::EaseOut);
        assert_eq!(old.keyframes[1].easing, Easing::Linear);
    }

    #[test]
    fn segments_and_remapping() {
        let mut a = Animatable::constant(0.0);
        assert_eq!(a.segment_at(Time(5)), None);
        a.set_keyframe(Time(100), 1.0);
        a.set_keyframe(Time(200), 2.0);
        a.set_keyframe(Time(300), 3.0);
        assert_eq!(a.segment_at(Time(0)), Some(0));
        assert_eq!(a.segment_at(Time(250)), Some(1));
        assert_eq!(a.segment_at(Time(300)), Some(1));
        a.remap_time(|t| Time(t.0 * 2));
        let times: Vec<_> = a.keyframes.iter().map(|k| k.at.0).collect();
        assert_eq!(times, vec![200, 400, 600]);
    }

    #[test]
    fn keyframes_stay_sorted_and_replace_in_place() {
        let mut a = Animatable::constant(0.0);
        a.set_keyframe(Time(300), 3.0);
        a.set_keyframe(Time(100), 1.0);
        a.set_keyframe(Time(300), 4.0);
        let times: Vec<_> = a.keyframes.iter().map(|k| k.at.0).collect();
        assert_eq!(times, vec![100, 300]);
        assert_eq!(a.keyframes[1].value, 4.0);
        assert!(a.remove_keyframe(Time(100)));
        assert!(!a.remove_keyframe(Time(100)));
    }
}
