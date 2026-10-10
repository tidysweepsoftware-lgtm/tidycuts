//! Speed ramping (Speed > Speed Ramping): a clip's speed changing
//! over its length.
//!
//! The points sit at times in the source file, so splitting or trimming a
//! clip keeps every moment at the speed it had and the two halves of a cut
//! join up exactly. Between points the speed changes evenly on a log scale
//! (a straight line on the ramp graph, whose speed axis is logarithmic), so
//! the mapping between timeline time and source time has an exact formula
//! in both directions: the time spent on a stretch of source is the
//! integral of `1 / speed` over it.

use crate::time::Time;
use serde::{Deserialize, Serialize};

/// Slowest and fastest a ramp can go.
pub const RAMP_MIN: f64 = 0.1;
pub const RAMP_MAX: f64 = 10.0;

/// The ramp shapes on offer, plus the user's own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RampPreset {
    #[default]
    Custom,
    Montage,
    HeroMoment,
    BulletTime,
    Jumper,
    FlashIn,
    FlashOut,
}

impl RampPreset {
    pub const ALL: [RampPreset; 7] = [
        RampPreset::Custom,
        RampPreset::Montage,
        RampPreset::HeroMoment,
        RampPreset::BulletTime,
        RampPreset::Jumper,
        RampPreset::FlashIn,
        RampPreset::FlashOut,
    ];

    pub fn label(self) -> &'static str {
        match self {
            RampPreset::Custom => "Custom",
            RampPreset::Montage => "Montage",
            RampPreset::HeroMoment => "Hero Moment",
            RampPreset::BulletTime => "Bullet Time",
            RampPreset::Jumper => "Jumper",
            RampPreset::FlashIn => "Flash In",
            RampPreset::FlashOut => "Flash Out",
        }
    }

    /// The shape as (position along the clip as it plays, 0..=1, speed).
    pub fn shape(self) -> &'static [(f64, f64)] {
        match self {
            // A starting point to edit: five points at normal speed.
            RampPreset::Custom => &[(0.0, 1.0), (0.25, 1.0), (0.5, 1.0), (0.75, 1.0), (1.0, 1.0)],
            RampPreset::Montage => &[
                (0.0, 1.0),
                (0.2, 3.0),
                (0.4, 0.5),
                (0.6, 0.5),
                (0.8, 3.0),
                (1.0, 1.0),
            ],
            RampPreset::HeroMoment => &[
                (0.0, 3.0),
                (0.3, 3.0),
                (0.4, 0.3),
                (0.6, 0.3),
                (0.7, 3.0),
                (1.0, 3.0),
            ],
            RampPreset::BulletTime => &[
                (0.0, 1.0),
                (0.35, 1.0),
                (0.45, 0.15),
                (0.6, 0.15),
                (0.7, 1.0),
                (1.0, 1.0),
            ],
            RampPreset::Jumper => &[
                (0.0, 0.5),
                (0.2, 4.0),
                (0.4, 0.5),
                (0.6, 4.0),
                (0.8, 0.5),
                (1.0, 2.0),
            ],
            RampPreset::FlashIn => &[(0.0, 5.0), (0.3, 5.0), (0.5, 1.0), (1.0, 1.0)],
            RampPreset::FlashOut => &[(0.0, 1.0), (0.5, 1.0), (0.7, 5.0), (1.0, 5.0)],
        }
    }
}

/// The speed at one moment of the source.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RampPoint {
    /// Time in the source file.
    pub at: Time,
    pub speed: f64,
}

/// A speed curve. Before the first point and after the last the speed
/// stays at that point's value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeedRamp {
    /// The preset it came from (Custom once edited).
    #[serde(default)]
    pub preset: RampPreset,
    /// Sorted by time, no two at the same time.
    pub points: Vec<RampPoint>,
}

/// One stretch of the curve: from `x0` to `x1` (seconds of source) the
/// speed is `v0 * e^(k (x - x0))`.
#[derive(Debug, Clone, Copy)]
struct Piece {
    x0: f64,
    x1: f64,
    v0: f64,
    k: f64,
}

impl Piece {
    fn speed(&self, x: f64) -> f64 {
        if self.k == 0.0 {
            self.v0
        } else {
            self.v0 * (self.k * (x - self.x0)).exp()
        }
    }

    /// Timeline seconds to play from `a` to `b` (a <= b, both inside).
    fn time(&self, a: f64, b: f64) -> f64 {
        if b <= a {
            return 0.0;
        }
        let va = self.speed(a);
        if self.k.abs() < 1e-12 {
            (b - a) / va
        } else {
            -(-self.k * (b - a)).exp_m1() / (self.k * va)
        }
    }

    /// The source time reached by playing `secs` (>= 0) forward from `a`.
    fn forward(&self, a: f64, secs: f64) -> f64 {
        let va = self.speed(a);
        if self.k.abs() < 1e-12 {
            a + secs * va
        } else {
            a - (-self.k * va * secs).ln_1p() / self.k
        }
    }

    /// The source time reached by playing `secs` (>= 0) backward from `a`.
    fn backward(&self, a: f64, secs: f64) -> f64 {
        let va = self.speed(a);
        if self.k.abs() < 1e-12 {
            a - secs * va
        } else {
            a - (self.k * va * secs).ln_1p() / self.k
        }
    }
}

fn clamp_speed(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(RAMP_MIN, RAMP_MAX)
    } else {
        1.0
    }
}

impl SpeedRamp {
    /// Lays a preset's shape over the source from `from` to `to` in the
    /// order it plays (`from` > `to` for a reversed clip).
    pub fn preset(preset: RampPreset, from: Time, to: Time) -> SpeedRamp {
        Self::from_shape(preset, preset.shape(), from, to)
    }

    /// Lays (position 0..=1, speed) pairs over `from..to` as it plays.
    pub fn from_shape(preset: RampPreset, shape: &[(f64, f64)], from: Time, to: Time) -> SpeedRamp {
        let points = shape
            .iter()
            .map(|(f, v)| RampPoint {
                at: from + (to - from).scale(f.clamp(0.0, 1.0)),
                speed: *v,
            })
            .collect();
        let mut r = SpeedRamp { preset, points };
        r.tidy();
        r
    }

    /// Sorts the points, keeps speeds in range and drops points that share
    /// a time.
    pub fn tidy(&mut self) {
        for p in &mut self.points {
            p.speed = clamp_speed(p.speed);
        }
        self.points.sort_by_key(|p| p.at);
        self.points.dedup_by_key(|p| p.at);
    }

    fn pieces(&self) -> Vec<Piece> {
        let pts = &self.points;
        let Some(first) = pts.first() else {
            return vec![Piece {
                x0: f64::NEG_INFINITY,
                x1: f64::INFINITY,
                v0: 1.0,
                k: 0.0,
            }];
        };
        let mut out = Vec::with_capacity(pts.len() + 1);
        let mut x = f64::NEG_INFINITY;
        let mut v = clamp_speed(first.speed);
        for p in pts {
            let px = p.at.as_secs_f64();
            let pv = clamp_speed(p.speed);
            if px <= x {
                continue;
            }
            let k = if x.is_finite() {
                (pv / v).ln() / (px - x)
            } else {
                0.0
            };
            out.push(Piece {
                x0: x,
                x1: px,
                v0: v,
                k,
            });
            x = px;
            v = pv;
        }
        out.push(Piece {
            x0: x,
            x1: f64::INFINITY,
            v0: v,
            k: 0.0,
        });
        out
    }

    /// Speed at source time `s` (seconds).
    pub fn speed_at(&self, s: f64) -> f64 {
        let pieces = self.pieces();
        pieces
            .iter()
            .find(|p| s < p.x1)
            .unwrap_or(&pieces[pieces.len() - 1])
            .speed(s)
    }

    /// The same shape played `factor` times as fast, so the clip lasts
    /// `1 / factor` as long (speeds stay within [`RAMP_MIN`], [`RAMP_MAX`]).
    pub fn scaled(&self, factor: f64) -> SpeedRamp {
        let mut r = self.clone();
        for p in &mut r.points {
            p.speed = (p.speed * factor).clamp(RAMP_MIN, RAMP_MAX);
        }
        r.preset = RampPreset::Custom;
        r
    }

    /// Timeline seconds it takes to play the source from `a` to `b`
    /// (negative when `b` is before `a`).
    pub fn play_secs(&self, a: f64, b: f64) -> f64 {
        if b < a {
            return -self.play_secs(b, a);
        }
        self.pieces()
            .iter()
            .filter(|p| p.x1 > a && p.x0 < b)
            .map(|p| p.time(a.max(p.x0), b.min(p.x1)))
            .sum()
    }

    /// The source time reached by playing `secs` of timeline from source
    /// time `a`: forward through the source, or backward when negative.
    pub fn advance(&self, a: f64, secs: f64) -> f64 {
        let pieces = self.pieces();
        let mut cur = a;
        let mut left = secs.abs();
        if secs >= 0.0 {
            for p in &pieces {
                if p.x1 <= cur {
                    continue;
                }
                let here = cur.max(p.x0);
                let avail = p.time(here, p.x1);
                if left <= avail || p.x1.is_infinite() {
                    return p.forward(here, left);
                }
                left -= avail;
                cur = p.x1;
            }
        } else {
            for p in pieces.iter().rev() {
                if p.x0 >= cur {
                    continue;
                }
                let here = cur.min(p.x1);
                let avail = p.time(p.x0, here);
                if left <= avail || p.x0.is_infinite() {
                    return p.backward(here, left);
                }
                left -= avail;
                cur = p.x0;
            }
        }
        cur
    }

    /// The same curve between `lo` and `hi`, with points at both ends and
    /// none outside (for editing what a trimmed clip shows).
    pub fn clipped(&self, lo: Time, hi: Time) -> SpeedRamp {
        let mut points: Vec<RampPoint> = self
            .points
            .iter()
            .filter(|p| p.at > lo && p.at < hi)
            .copied()
            .collect();
        for at in [lo, hi] {
            points.push(RampPoint {
                at,
                speed: self.speed_at(at.as_secs_f64()),
            });
        }
        let mut r = SpeedRamp {
            preset: self.preset,
            points,
        };
        r.tidy();
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: f64) -> Time {
        Time::from_secs_f64(x)
    }

    #[test]
    fn scaling_a_ramp_stretches_its_length_and_keeps_its_shape() {
        let r = SpeedRamp::preset(RampPreset::HeroMoment, s(0.0), s(8.0));
        let before = r.play_secs(0.0, 8.0);
        let slower = r.scaled(0.5);
        assert!((slower.play_secs(0.0, 8.0) - before * 2.0).abs() < 1e-6);
        // The ratio between any two points is unchanged.
        let (a, b) = (r.points[0].speed, r.points[2].speed);
        let (c, d) = (slower.points[0].speed, slower.points[2].speed);
        assert!((a / b - c / d).abs() < 1e-9);
    }

    fn ramp(points: &[(f64, f64)]) -> SpeedRamp {
        let mut r = SpeedRamp {
            preset: RampPreset::Custom,
            points: points
                .iter()
                .map(|(a, v)| RampPoint {
                    at: s(*a),
                    speed: *v,
                })
                .collect(),
        };
        r.tidy();
        r
    }

    #[test]
    fn constant_ramp_matches_plain_speed() {
        let r = ramp(&[(0.0, 2.0), (10.0, 2.0)]);
        assert!((r.play_secs(0.0, 10.0) - 5.0).abs() < 1e-9);
        assert!((r.advance(1.0, 2.0) - 5.0).abs() < 1e-9);
        assert!((r.advance(5.0, -2.0) - 1.0).abs() < 1e-9);
        // Past the ends the speed carries on.
        assert!((r.play_secs(-4.0, 0.0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn time_is_the_integral_of_one_over_speed() {
        let r = ramp(&[(0.0, 0.5), (4.0, 4.0)]);
        // Numerical integral to compare with the exact one.
        let n = 200_000;
        let mut sum = 0.0;
        for i in 0..n {
            let x = 4.0 * (i as f64 + 0.5) / n as f64;
            sum += 4.0 / n as f64 / r.speed_at(x);
        }
        assert!((r.play_secs(0.0, 4.0) - sum).abs() < 1e-6);
        assert!((r.speed_at(2.0) - 2.0f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn advance_inverts_play_secs() {
        let r = RampPreset::Montage.shape();
        let r = SpeedRamp::from_shape(RampPreset::Montage, r, s(1.0), s(9.0));
        for a in [-1.0, 0.5, 1.0, 3.3, 7.9, 12.0] {
            for b in [-2.0, 0.0, 2.2, 5.0, 9.0, 11.0] {
                let t = r.play_secs(a, b);
                assert!((r.advance(a, t) - b).abs() < 1e-7, "{a} -> {b}");
            }
        }
    }

    #[test]
    fn mapping_is_monotonic() {
        let r = SpeedRamp::preset(RampPreset::BulletTime, s(0.0), s(6.0));
        let mut last = f64::NEG_INFINITY;
        for i in -50..400 {
            let x = r.advance(0.0, i as f64 * 0.05);
            assert!(x > last);
            last = x;
        }
    }

    #[test]
    fn clipping_keeps_the_curve() {
        let r = SpeedRamp::preset(RampPreset::Jumper, s(0.0), s(10.0));
        let c = r.clipped(s(3.0), s(7.0));
        assert_eq!(c.points.first().unwrap().at, s(3.0));
        assert_eq!(c.points.last().unwrap().at, s(7.0));
        assert!((c.play_secs(3.0, 7.0) - r.play_secs(3.0, 7.0)).abs() < 1e-9);
        for x in [3.0, 4.1, 5.5, 6.9] {
            assert!((c.speed_at(x) - r.speed_at(x)).abs() < 1e-9);
        }
    }

    #[test]
    fn presets_are_laid_out_in_play_order() {
        let fwd = SpeedRamp::preset(RampPreset::FlashIn, s(0.0), s(10.0));
        let rev = SpeedRamp::preset(RampPreset::FlashIn, s(10.0), s(0.0));
        assert_eq!(fwd.speed_at(0.0), 5.0);
        assert_eq!(rev.speed_at(10.0), 5.0);
        assert_eq!(rev.speed_at(0.0), 1.0);
        assert!(rev.points.windows(2).all(|w| w[0].at < w[1].at));
    }
}
