use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

/// A point or span on the timeline, stored as whole microseconds so that
/// arithmetic is exact and project files round-trip without float drift.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Time(pub i64);

impl Time {
    pub const ZERO: Time = Time(0);

    pub fn from_secs_f64(secs: f64) -> Self {
        Time((secs * 1_000_000.0).round() as i64)
    }

    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / 1_000_000.0
    }

    pub const fn from_millis(ms: i64) -> Self {
        Time(ms * 1000)
    }

    /// Multiplies a span by a factor, rounding to the nearest microsecond.
    pub fn scale(self, factor: f64) -> Time {
        Time((self.0 as f64 * factor).round() as i64)
    }

    pub fn abs(self) -> Time {
        Time(self.0.abs())
    }

    pub fn max(self, other: Time) -> Time {
        if self >= other {
            self
        } else {
            other
        }
    }

    pub fn min(self, other: Time) -> Time {
        if self <= other {
            self
        } else {
            other
        }
    }

    /// Formats as `HH:MM:SS:FF`, the timecode style shown under the preview.
    pub fn timecode(self, fps: FrameRate) -> String {
        let total_frames = fps.frames_at(self).max(0);
        let fps_round = fps.as_f64().round().max(1.0) as i64;
        let frames = total_frames % fps_round;
        let total_secs = total_frames / fps_round;
        format!(
            "{:02}:{:02}:{:02}:{:02}",
            total_secs / 3600,
            (total_secs / 60) % 60,
            total_secs % 60,
            frames
        )
    }
}

impl Add for Time {
    type Output = Time;
    fn add(self, rhs: Time) -> Time {
        Time(self.0 + rhs.0)
    }
}

impl AddAssign for Time {
    fn add_assign(&mut self, rhs: Time) {
        self.0 += rhs.0;
    }
}

impl SubAssign for Time {
    fn sub_assign(&mut self, rhs: Time) {
        self.0 -= rhs.0;
    }
}

impl Neg for Time {
    type Output = Time;
    fn neg(self) -> Time {
        Time(-self.0)
    }
}

impl Sub for Time {
    type Output = Time;
    fn sub(self, rhs: Time) -> Time {
        Time(self.0 - rhs.0)
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3}s", self.as_secs_f64())
    }
}

/// Frame rate as a rational number (e.g. 30000/1001 for 29.97).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameRate {
    pub num: u32,
    pub den: u32,
}

impl FrameRate {
    pub const fn new(num: u32, den: u32) -> Self {
        FrameRate { num, den }
    }

    pub fn as_f64(self) -> f64 {
        if self.den == 0 {
            0.0
        } else {
            self.num as f64 / self.den as f64
        }
    }

    pub fn frame_duration(self) -> Time {
        Time::from_secs_f64(self.den as f64 / self.num.max(1) as f64)
    }

    /// Index of the frame that is showing at `t`.
    pub fn frames_at(self, t: Time) -> i64 {
        (t.as_secs_f64() * self.as_f64() + 1e-6).floor() as i64
    }

    /// Start time of frame `index`.
    pub fn time_of_frame(self, index: i64) -> Time {
        Time::from_secs_f64(index as f64 / self.as_f64())
    }

    /// Snaps `t` down to the start of the frame it falls in.
    pub fn snap(self, t: Time) -> Time {
        self.time_of_frame(self.frames_at(t))
    }

    /// Parses ffprobe's `r_frame_rate` style strings like `30000/1001` or `25`.
    pub fn parse(s: &str) -> Option<FrameRate> {
        let mut parts = s.trim().split('/');
        let num: u32 = parts.next()?.trim().parse().ok()?;
        let den: u32 = match parts.next() {
            Some(d) => d.trim().parse().ok()?,
            None => 1,
        };
        if num == 0 || den == 0 {
            None
        } else {
            Some(FrameRate { num, den })
        }
    }
}

impl Default for FrameRate {
    fn default() -> Self {
        FrameRate::new(30, 1)
    }
}

impl fmt::Display for FrameRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = self.as_f64();
        if (v - v.round()).abs() < 0.005 {
            write!(f, "{}fps", v.round() as i64)
        } else {
            write!(f, "{:.2}fps", v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timecode_formats_hours_minutes_seconds_frames() {
        let fps = FrameRate::new(25, 1);
        assert_eq!(Time::from_secs_f64(0.0).timecode(fps), "00:00:00:00");
        assert_eq!(Time::from_secs_f64(1.5).timecode(fps), "00:00:01:12");
        assert_eq!(Time::from_secs_f64(3725.04).timecode(fps), "01:02:05:01");
    }

    #[test]
    fn parses_rational_frame_rates() {
        assert_eq!(
            FrameRate::parse("30000/1001"),
            Some(FrameRate::new(30000, 1001))
        );
        assert_eq!(FrameRate::parse("25"), Some(FrameRate::new(25, 1)));
        assert_eq!(FrameRate::parse("0/0"), None);
        assert_eq!(FrameRate::new(30000, 1001).to_string(), "29.97fps");
    }

    #[test]
    fn snapping_lands_on_frame_starts() {
        let fps = FrameRate::new(25, 1);
        assert_eq!(
            fps.snap(Time::from_secs_f64(0.05)),
            Time::from_secs_f64(0.04)
        );
        assert_eq!(
            fps.snap(Time::from_secs_f64(0.04)),
            Time::from_secs_f64(0.04)
        );
    }
}
