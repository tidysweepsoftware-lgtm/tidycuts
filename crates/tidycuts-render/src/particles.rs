//! A deterministic particle emitter for built-in assets: sparks, embers,
//! dust, confetti, bubbles, snow. Nothing is simulated step by step. Where a
//! particle is at time `t` is a formula, so any frame can be drawn on its own
//! and the effect loops perfectly every `life` seconds.

use crate::motion::smoothstep;
use crate::noise;
use std::f32::consts::PI;

/// One particle at one moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    /// Position in the asset's own units (usually 0..1 across its height).
    pub pos: (f32, f32),
    /// Size in the same units.
    pub size: f32,
    /// How far through its life it is, 0 (just born) to 1 (gone).
    pub age: f32,
    /// A ready-made opacity: fades in quickly, fades out over the second half.
    pub fade: f32,
    /// Stable index, for picking a colour or shape.
    pub id: u32,
    /// A stable random number in 0..1, for variety.
    pub rand: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Emitter {
    pub seed: u32,
    /// How many particles are alive at once, roughly.
    pub count: u32,
    /// Seconds a particle lives. The whole effect loops with this period.
    pub life: f32,
    pub origin: (f32, f32),
    /// Random offset of the birth point around the origin (half-width, half-height).
    pub origin_spread: (f32, f32),
    /// Main direction in radians. 0 is right, -PI/2 is up.
    pub angle: f32,
    /// Random deviation from that direction, in radians either way.
    pub spread: f32,
    /// Speed range, units per second.
    pub speed: (f32, f32),
    /// Constant acceleration, units per second squared. Positive y is down.
    pub gravity: (f32, f32),
    /// 0 = none. Higher slows particles sooner.
    pub drag: f32,
    pub size: (f32, f32),
    /// How much a swirling noise field pushes them as they age (units).
    pub turbulence: f32,
    /// Off (the default): every cycle is identical, so the effect loops
    /// perfectly. On: each respawn picks new random values, which never
    /// repeats but does not loop.
    pub fresh_each_cycle: bool,
}

impl Emitter {
    /// A reasonable starting point: a fountain going up from the bottom middle.
    pub const fn new(seed: u32) -> Self {
        Emitter {
            seed,
            count: 40,
            life: 2.0,
            origin: (0.5, 0.9),
            origin_spread: (0.05, 0.0),
            angle: -PI / 2.0,
            spread: 0.5,
            speed: (0.2, 0.5),
            gravity: (0.0, 0.0),
            drag: 0.0,
            size: (0.01, 0.03),
            turbulence: 0.0,
            fresh_each_cycle: false,
        }
    }

    /// Particle number `i` (0..count) at time `t` seconds.
    pub fn particle(&self, i: u32, t: f32) -> Particle {
        let h = |k: u32| noise_hash(self.seed, i, k);
        // Each particle is born at its own point inside the cycle, so there is
        // a steady stream and not a burst.
        let age = (t / self.life + h(0)).rem_euclid(1.0);
        let secs = age * self.life;
        // Which round of the cycle this is. Only used when every respawn
        // should land somewhere new.
        let round = if self.fresh_each_cycle {
            (t / self.life + h(0)).floor() as i32 as u32
        } else {
            0
        };
        let hr = |k: u32| noise_hash(self.seed ^ round.wrapping_mul(0x9E37_79B1), i, k);

        let dir = self.angle + (hr(1) * 2.0 - 1.0) * self.spread;
        let speed = self.speed.0 + (self.speed.1 - self.speed.0) * hr(2);
        let travel = if self.drag > 1e-4 {
            speed * (1.0 - (-self.drag * secs).exp()) / self.drag
        } else {
            speed * secs
        };
        let (gx, gy) = (
            0.5 * self.gravity.0 * secs * secs,
            0.5 * self.gravity.1 * secs * secs,
        );
        let bx = self.origin.0 + (hr(3) * 2.0 - 1.0) * self.origin_spread.0;
        let by = self.origin.1 + (hr(4) * 2.0 - 1.0) * self.origin_spread.1;
        let (mut x, mut y) = (bx + dir.cos() * travel + gx, by + dir.sin() * travel + gy);
        if self.turbulence > 0.0 {
            // The field is read at the particle's age, not at the clock, so
            // the swirl repeats with the cycle.
            let (cx, cy) = noise::curl(bx * 3.0, by * 3.0 + hr(5) * 10.0, age * 1.5, self.seed);
            x += cx * self.turbulence * age;
            y += cy * self.turbulence * age;
        }
        let size = self.size.0 + (self.size.1 - self.size.0) * hr(6);
        Particle {
            pos: (x, y),
            size,
            age,
            fade: smoothstep(0.0, 0.08, age) * (1.0 - smoothstep(0.5, 1.0, age)),
            id: i,
            rand: hr(7),
        }
    }

    /// Every particle at time `t`.
    pub fn particles(&self, t: f32) -> impl Iterator<Item = Particle> + '_ {
        (0..self.count).map(move |i| self.particle(i, t))
    }
}

fn noise_hash(seed: u32, i: u32, k: u32) -> f32 {
    noise::hash3(i as i32, k as i32, 0, seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_time_same_picture() {
        let e = Emitter::new(5);
        let a: Vec<Particle> = e.particles(0.8).collect();
        let b: Vec<Particle> = e.particles(0.8).collect();
        assert_eq!(a, b);
        assert_eq!(a.len(), 40);
    }

    #[test]
    fn it_loops_every_life() {
        let mut e = Emitter::new(11);
        e.gravity = (0.0, 0.3);
        e.drag = 0.8;
        e.turbulence = 0.1;
        for i in 0..e.count {
            let (a, b) = (e.particle(i, 0.37), e.particle(i, 0.37 + e.life));
            assert!(
                (a.pos.0 - b.pos.0).abs() < 1e-3 && (a.pos.1 - b.pos.1).abs() < 1e-3,
                "{a:?} {b:?}"
            );
            assert!((a.age - b.age).abs() < 1e-3);
        }
    }

    #[test]
    fn particles_are_born_unseen_and_die_unseen() {
        let e = Emitter::new(2);
        for i in 0..e.count {
            let mut seen = false;
            for step in 0..200 {
                let p = e.particle(i, step as f32 * e.life / 200.0);
                assert!((0.0..=1.0).contains(&p.fade) && (0.0..1.0).contains(&p.age));
                seen |= p.fade > 0.5;
                if p.age < 0.005 {
                    assert!(p.fade < 0.05, "born visible: {p:?}");
                }
            }
            assert!(seen, "particle {i} is never visible");
        }
    }

    #[test]
    fn stream_is_steady_not_a_burst() {
        let e = Emitter::new(8);
        // The number of visible particles stays similar over time.
        let counts: Vec<usize> = (0..20)
            .map(|k| e.particles(k as f32 * 0.1).filter(|p| p.fade > 0.2).count())
            .collect();
        let (lo, hi) = (*counts.iter().min().unwrap(), *counts.iter().max().unwrap());
        assert!(lo > 10 && hi - lo < 14, "counts {counts:?}");
    }

    #[test]
    fn direction_gravity_and_size_are_obeyed() {
        let mut e = Emitter::new(3);
        e.spread = 0.0;
        e.angle = 0.0; // to the right
        e.origin = (0.1, 0.5);
        e.origin_spread = (0.0, 0.0);
        e.speed = (0.4, 0.4);
        e.size = (0.02, 0.02);
        let p = e.particle(0, 0.5);
        assert!(p.pos.0 > 0.1 && (p.pos.1 - 0.5).abs() < 1e-4, "{p:?}");
        assert!((p.size - 0.02).abs() < 1e-6);
        e.gravity = (0.0, 1.0);
        let q = e.particle(0, 0.5);
        assert!(q.pos.1 > 0.5, "gravity pulls down: {q:?}");
        // Drag stops a particle short of where it would otherwise have got to.
        e.gravity = (0.0, 0.0);
        e.drag = 3.0;
        assert!(e.particle(0, 1.5).pos.0 < p.pos.0 + 0.4 * 1.0);
    }

    #[test]
    fn different_seeds_look_different() {
        let a = Emitter::new(1).particle(3, 0.4);
        let b = Emitter::new(2).particle(3, 0.4);
        assert_ne!(a.pos, b.pos);
    }
}
