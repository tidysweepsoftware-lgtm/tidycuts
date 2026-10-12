//! Shared motion helpers for the built-in titles, stickers, effects and
//! transitions: easing curves, springs and staggering. Everything is a pure
//! function of time, so a frame always looks the same however it is reached.
//!
//! Times and progress values are 0..1 unless noted.

use std::f32::consts::{PI, TAU};

pub fn clamp01(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// 0 below `e0`, 1 above `e1`, smooth in between.
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp01((x - e0) / (e1 - e0));
    t * t * (3.0 - 2.0 * t)
}

/// Where `x` sits between `a` and `b` (0 at `a`, 1 at `b`), not clamped.
pub fn inverse_lerp(a: f32, b: f32, x: f32) -> f32 {
    if (b - a).abs() < 1e-9 {
        0.0
    } else {
        (x - a) / (b - a)
    }
}

// ---------------------------------------------------------------------------
// Easing curves. Each maps 0..1 to 0..1 (the "back", "elastic" and "bounce"
// ones overshoot on the way, then land on 1).

pub fn ease_in_quad(t: f32) -> f32 {
    let t = clamp01(t);
    t * t
}

pub fn ease_out_quad(t: f32) -> f32 {
    let t = clamp01(t);
    1.0 - (1.0 - t) * (1.0 - t)
}

pub fn ease_in_cubic(t: f32) -> f32 {
    let t = clamp01(t);
    t * t * t
}

pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - clamp01(t)).powi(3)
}

pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = clamp01(t);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

pub fn ease_out_quart(t: f32) -> f32 {
    1.0 - (1.0 - clamp01(t)).powi(4)
}

pub fn ease_in_out_quart(t: f32) -> f32 {
    let t = clamp01(t);
    if t < 0.5 {
        8.0 * t * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
    }
}

pub fn ease_in_out_sine(t: f32) -> f32 {
    -((PI * clamp01(t)).cos() - 1.0) / 2.0
}

/// Fast start, very long tail: the "whip" curve for pans and zooms.
pub fn ease_out_expo(t: f32) -> f32 {
    let t = clamp01(t);
    if t >= 1.0 {
        1.0
    } else {
        1.0 - 2f32.powf(-10.0 * t)
    }
}

/// Slow start, sudden finish.
pub fn ease_in_expo(t: f32) -> f32 {
    let t = clamp01(t);
    if t <= 0.0 {
        0.0
    } else {
        2f32.powf(10.0 * t - 10.0)
    }
}

pub fn ease_in_out_expo(t: f32) -> f32 {
    let t = clamp01(t);
    if t <= 0.0 {
        0.0
    } else if t >= 1.0 {
        1.0
    } else if t < 0.5 {
        2f32.powf(20.0 * t - 10.0) / 2.0
    } else {
        (2.0 - 2f32.powf(-20.0 * t + 10.0)) / 2.0
    }
}

/// Overshoots the target by about `overshoot` (0.1 = 10%) and settles.
pub fn ease_out_back(t: f32, overshoot: f32) -> f32 {
    let t = clamp01(t);
    // The standard constant 1.70158 gives a 10% overshoot.
    let c1 = 1.70158 * (overshoot / 0.1).max(0.0).sqrt();
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

pub fn ease_out_elastic(t: f32) -> f32 {
    let t = clamp01(t);
    if t <= 0.0 {
        0.0
    } else if t >= 1.0 {
        1.0
    } else {
        2f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * (TAU / 3.0)).sin() + 1.0
    }
}

pub fn ease_out_bounce(t: f32) -> f32 {
    let t = clamp01(t);
    let (n1, d1) = (7.5625, 2.75);
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

// ---------------------------------------------------------------------------
// Springs.

/// A damped spring that starts at rest at 0 and settles on 1. It is the
/// closed-form solution, so it can be sampled at any time without stepping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    /// Natural frequency in radians per second.
    omega: f32,
    /// 1 = no overshoot, below 1 = bouncy.
    zeta: f32,
}

impl Spring {
    /// `frequency` is oscillations per second (about 1 to 4 for titles),
    /// `damping` is 0.05 (very bouncy) to 1.0 (no overshoot).
    pub fn new(frequency: f32, damping: f32) -> Self {
        Spring {
            omega: TAU * frequency.max(0.01),
            zeta: damping.clamp(0.01, 4.0),
        }
    }

    /// Gentle settle, a hint of overshoot.
    pub const SOFT: Spring = Spring {
        omega: TAU * 1.4,
        zeta: 0.75,
    };
    /// Quick and lively, visible bounce.
    pub const SNAPPY: Spring = Spring {
        omega: TAU * 2.4,
        zeta: 0.5,
    };
    /// Fast with no overshoot.
    pub const STIFF: Spring = Spring {
        omega: TAU * 3.0,
        zeta: 1.0,
    };

    /// Position after `t` seconds: 0 at the start, 1 once settled.
    pub fn at(&self, t: f32) -> f32 {
        if t <= 0.0 {
            return 0.0;
        }
        let (w, z) = (self.omega, self.zeta);
        if z < 1.0 {
            let wd = w * (1.0 - z * z).sqrt();
            1.0 - (-z * w * t).exp() * ((wd * t).cos() + (z * w / wd) * (wd * t).sin())
        } else if z == 1.0 {
            1.0 - (-w * t).exp() * (1.0 + w * t)
        } else {
            // Over-damped: two decaying exponentials.
            let s = (z * z - 1.0).sqrt();
            let (r1, r2) = (-w * (z - s), -w * (z + s));
            1.0 - (r2 * (r1 * t).exp() - r1 * (r2 * t).exp()) / (r2 - r1)
        }
    }

    /// How long until it stays within 0.5% of 1 (an upper estimate).
    pub fn settle_time(&self) -> f32 {
        let (w, z) = (self.omega, self.zeta);
        if z < 1.0 {
            // The envelope decays as e^(-zeta*omega*t); 0.005 is about e^-5.3.
            5.3 / (z * w)
        } else {
            // The slow pole decides: critical damping has rate omega, and an
            // over-damped spring is slower (rate omega * (zeta - sqrt(zeta^2 - 1))).
            let rate = w * (z - (z * z - 1.0).sqrt());
            7.5 / rate
        }
    }
}

// ---------------------------------------------------------------------------
// Staggering: many items, each starting a little after the last.

/// Progress of item `index` of `count`, when the whole group runs over
/// `t` (0..1) and the starts are spread across the first `spread` (0..1) of
/// that time. Each item still gets the remaining `1 - spread` to play out.
pub fn stagger(index: usize, count: usize, spread: f32, t: f32) -> f32 {
    if count <= 1 {
        return clamp01(t);
    }
    let spread = spread.clamp(0.0, 0.95);
    let delay = spread * index as f32 / (count - 1) as f32;
    clamp01((t - delay) / (1.0 - spread))
}

/// Same, but the order is shuffled by a hash so the items arrive randomly.
pub fn stagger_shuffled(index: usize, count: usize, spread: f32, t: f32, seed: u32) -> f32 {
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by_key(|&i| hash_u32(i as u32 ^ seed.wrapping_mul(0x9E37_79B1)));
    let rank = order.iter().position(|&i| i == index).unwrap_or(index);
    stagger(rank, count, spread, t)
}

fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^ (x >> 16)
}

// ---------------------------------------------------------------------------
// Looping.

/// 0 -> 1 -> 0 over one period (a triangle wave).
pub fn ping_pong(t: f32) -> f32 {
    let f = t.rem_euclid(1.0);
    1.0 - (2.0 * f - 1.0).abs()
}

/// A short bump: 0 before `start`, rising to 1 at `peak`, back to 0 at `end`.
pub fn bump(start: f32, peak: f32, end: f32, t: f32) -> f32 {
    if t <= start || t >= end {
        0.0
    } else if t < peak {
        smoothstep(start, peak, t)
    } else {
        1.0 - smoothstep(peak, end, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Curve = fn(f32) -> f32;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn easings_start_at_0_and_end_at_1() {
        let curves: [(&str, Curve); 12] = [
            ("in_quad", ease_in_quad),
            ("out_quad", ease_out_quad),
            ("in_cubic", ease_in_cubic),
            ("out_cubic", ease_out_cubic),
            ("in_out_cubic", ease_in_out_cubic),
            ("out_quart", ease_out_quart),
            ("in_out_quart", ease_in_out_quart),
            ("in_out_sine", ease_in_out_sine),
            ("out_expo", ease_out_expo),
            ("in_expo", ease_in_expo),
            ("in_out_expo", ease_in_out_expo),
            ("out_bounce", ease_out_bounce),
        ];
        for (name, f) in curves {
            assert!(f(0.0).abs() < 2e-3, "{name}(0) = {}", f(0.0));
            assert!(close(f(1.0), 1.0), "{name}(1) = {}", f(1.0));
            assert!(
                close(f(-5.0), f(0.0)) && close(f(5.0), f(1.0)),
                "{name} clamps"
            );
        }
        assert!(close(ease_out_elastic(0.0), 0.0) && close(ease_out_elastic(1.0), 1.0));
        assert!(close(ease_out_back(1.0, 0.1), 1.0));
    }

    #[test]
    fn back_overshoots_by_about_the_asked_amount() {
        let peak = (0..=100)
            .map(|i| ease_out_back(i as f32 / 100.0, 0.1))
            .fold(0.0f32, f32::max);
        assert!((peak - 1.1).abs() < 0.02, "peak {peak}");
        let big = (0..=100)
            .map(|i| ease_out_back(i as f32 / 100.0, 0.4))
            .fold(0.0f32, f32::max);
        assert!(big > 1.3, "bigger overshoot {big}");
    }

    #[test]
    fn expo_is_fast_then_slow() {
        assert!(ease_out_expo(0.2) > 0.7);
        assert!(ease_in_expo(0.8) < 0.3);
    }

    #[test]
    fn springs_start_at_0_and_settle_on_1() {
        for s in [
            Spring::SOFT,
            Spring::SNAPPY,
            Spring::STIFF,
            Spring::new(2.0, 2.0),
        ] {
            assert_eq!(s.at(0.0), 0.0);
            assert_eq!(s.at(-1.0), 0.0);
            let late = s.at(s.settle_time() * 1.5 + 0.5);
            assert!((late - 1.0).abs() < 0.01, "{s:?} settles at {late}");
        }
    }

    #[test]
    fn bouncy_springs_overshoot_and_stiff_ones_do_not() {
        let peak = |s: Spring| {
            (0..400)
                .map(|i| s.at(i as f32 / 200.0))
                .fold(0.0f32, f32::max)
        };
        assert!(
            peak(Spring::SNAPPY) > 1.1,
            "snappy {}",
            peak(Spring::SNAPPY)
        );
        assert!(peak(Spring::SOFT) > 1.0);
        assert!(
            peak(Spring::STIFF) <= 1.0 + 1e-4,
            "stiff {}",
            peak(Spring::STIFF)
        );
        assert!(peak(Spring::new(2.0, 2.0)) <= 1.0 + 1e-4, "over-damped");
    }

    #[test]
    fn stagger_delays_each_item_and_all_finish() {
        let n = 5;
        // At the very start only the first item has begun.
        assert!(stagger(0, n, 0.5, 0.1) > 0.0);
        assert_eq!(stagger(4, n, 0.5, 0.1), 0.0);
        // Later items are never ahead of earlier ones.
        for step in 0..=20 {
            let t = step as f32 / 20.0;
            for i in 1..n {
                assert!(stagger(i, n, 0.5, t) <= stagger(i - 1, n, 0.5, t) + 1e-6);
            }
        }
        // Everything has finished at t = 1.
        for i in 0..n {
            assert!(close(stagger(i, n, 0.5, 1.0), 1.0));
        }
        assert!(close(stagger(0, 1, 0.5, 0.4), 0.4));
    }

    #[test]
    fn shuffled_stagger_uses_every_slot_once() {
        let n = 8;
        let mut starts: Vec<f32> = (0..n)
            .map(|i| stagger_shuffled(i, n, 0.5, 0.25, 7))
            .collect();
        let again: Vec<f32> = (0..n)
            .map(|i| stagger_shuffled(i, n, 0.5, 0.25, 7))
            .collect();
        assert_eq!(starts, again, "same seed, same order");
        starts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let plain: Vec<f32> = {
            let mut v: Vec<f32> = (0..n).map(|i| stagger(i, n, 0.5, 0.25)).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v
        };
        assert_eq!(starts, plain, "a shuffle of the same set of delays");
    }

    #[test]
    fn loops_and_bumps() {
        assert!(
            close(ping_pong(0.0), 0.0) && close(ping_pong(0.5), 1.0) && close(ping_pong(1.0), 0.0)
        );
        assert!(close(ping_pong(1.25), ping_pong(0.25)));
        assert_eq!(bump(0.2, 0.4, 0.8, 0.1), 0.0);
        assert!(close(bump(0.2, 0.4, 0.8, 0.4), 1.0));
        assert_eq!(bump(0.2, 0.4, 0.8, 0.9), 0.0);
        assert!(close(inverse_lerp(2.0, 4.0, 3.0), 0.5));
    }
}
