//! Movement computed on the CPU: in/out animations (Animation
//! tab), sticker loops and motion effects all produce a [`Motion`] that the
//! engine applies on top of the clip's own transform.

/// How a title's letters or words animate one by one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LetterKind {
    /// Letters rise into place one after another.
    Rise,
    /// Letters pop in one after another.
    Pop,
    /// Words pop in one after another.
    WordsPop,
    /// Letters start spread out and draw together.
    Tracking,
    /// A highlight moves through the words.
    Karaoke,
}

/// A change to a clip's placement for one moment. The neutral value is
/// [`Motion::NONE`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    /// Offset in project pixels (1080p-sized frame).
    pub dx: f32,
    pub dy: f32,
    /// Multiplies the scale.
    pub scale: f32,
    /// Extra non-uniform scale (multiplies x and y separately).
    pub stretch: [f32; 2],
    /// Added to the rotation, degrees.
    pub rotation: f32,
    /// Multiplies the opacity.
    pub opacity: f32,
    /// Extra blur, 0..1.
    pub blur: f32,
    /// Extra crop as fractions (left, top, right, bottom), for wipes.
    pub crop: [f32; 4],
    /// Fraction of a title's characters shown (typewriter), 1 = all.
    pub reveal: f32,
    /// A letter-by-letter or word-by-word animation and how far along it is.
    pub letters: Option<(LetterKind, f32)>,
}

impl Motion {
    pub const NONE: Motion = Motion {
        dx: 0.0,
        dy: 0.0,
        scale: 1.0,
        stretch: [1.0, 1.0],
        rotation: 0.0,
        opacity: 1.0,
        blur: 0.0,
        crop: [0.0; 4],
        reveal: 1.0,
        letters: None,
    };

    /// Combines two motions (offsets add, factors multiply).
    pub fn then(self, o: Motion) -> Motion {
        Motion {
            dx: self.dx + o.dx,
            dy: self.dy + o.dy,
            scale: self.scale * o.scale,
            stretch: [
                self.stretch[0] * o.stretch[0],
                self.stretch[1] * o.stretch[1],
            ],
            rotation: self.rotation + o.rotation,
            opacity: self.opacity * o.opacity,
            blur: (self.blur + o.blur).min(1.0),
            crop: [
                self.crop[0].max(o.crop[0]),
                self.crop[1].max(o.crop[1]),
                self.crop[2].max(o.crop[2]),
                self.crop[3].max(o.crop[3]),
            ],
            reveal: self.reveal.min(o.reveal),
            letters: o.letters.or(self.letters),
        }
    }
}

impl Default for Motion {
    fn default() -> Self {
        Motion::NONE
    }
}

/// Whether an animation plays as the clip appears or disappears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    In,
    Out,
}

#[derive(Debug, Clone, Copy)]
pub struct AnimDef {
    pub id: &'static str,
    pub name: &'static str,
    /// Only meaningful for titles (typewriter).
    pub titles_only: bool,
}

const fn anim(id: &'static str, name: &'static str) -> AnimDef {
    AnimDef {
        id,
        name,
        titles_only: false,
    }
}

/// In and out animations. Each one is mirrored for the out phase (a
/// "slide left" in comes from the right; out it leaves to the left).
pub static ANIMATIONS: &[AnimDef] = &[
    anim("fade", "Fade"),
    anim("zoom_in", "Zoom In"),
    anim("zoom_out", "Zoom Out"),
    anim("slide_left", "Slide Left"),
    anim("slide_right", "Slide Right"),
    anim("slide_up", "Slide Up"),
    anim("slide_down", "Slide Down"),
    anim("rise", "Rise"),
    anim("drop", "Drop"),
    anim("pop", "Pop"),
    anim("bounce", "Bounce"),
    anim("spin", "Spin"),
    anim("rotate", "Rotate"),
    anim("flip", "Flip"),
    anim("stretch", "Stretch"),
    anim("squeeze", "Squeeze"),
    anim("blur", "Blur"),
    anim("wipe_left", "Wipe Left"),
    anim("wipe_right", "Wipe Right"),
    anim("wipe_up", "Wipe Up"),
    anim("wipe_down", "Wipe Down"),
    anim("slam", "Slam"),
    anim("flicker", "Flicker On"),
    AnimDef {
        id: "typewriter",
        name: "Typewriter",
        titles_only: true,
    },
    AnimDef {
        id: "letters_rise",
        name: "Letters Rise",
        titles_only: true,
    },
    AnimDef {
        id: "letters_pop",
        name: "Letters Pop",
        titles_only: true,
    },
    AnimDef {
        id: "words_pop",
        name: "Words Pop",
        titles_only: true,
    },
    AnimDef {
        id: "tracking",
        name: "Tracking In",
        titles_only: true,
    },
    AnimDef {
        id: "karaoke",
        name: "Karaoke Highlight",
        titles_only: true,
    },
];

pub fn get(id: &str) -> Option<&'static AnimDef> {
    ANIMATIONS.iter().find(|a| a.id == id)
}

/// A light switching on: dark, a stutter, then steady.
fn flicker(r: f32) -> f32 {
    match r {
        r if r >= 0.55 => 1.0,
        r if r < 0.12 => 0.0,
        r if r < 0.22 => 0.85,
        r if r < 0.3 => 0.1,
        r if r < 0.42 => 1.0,
        r if r < 0.48 => 0.25,
        _ => 1.0,
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

fn ease_out_bounce(t: f32) -> f32 {
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

/// The motion of an in or out animation. `progress` runs 0..1 over the
/// animation: for `In`, 0 is fully hidden and 1 is in place; for `Out`, 0 is
/// in place and 1 is gone. `canvas` is the project size in pixels.
pub fn evaluate(id: &str, phase: Phase, progress: f32, canvas: [f32; 2]) -> Motion {
    let p = progress.clamp(0.0, 1.0);
    // `v` = how far the clip is from its resting state, 1 = fully away.
    let v = match phase {
        Phase::In => 1.0 - ease_out_cubic(p),
        Phase::Out => p * p,
    };
    let dir: f32 = match phase {
        Phase::In => 1.0,
        Phase::Out => -1.0,
    };
    let (w, h) = (canvas[0], canvas[1]);
    let mut m = Motion::NONE;
    match id {
        "fade" => m.opacity = 1.0 - v,
        // In: grows from small to full size. Out: keeps growing as it fades.
        "zoom_in" => {
            m.scale = 1.0 - 0.6 * v * dir;
            m.opacity = 1.0 - v;
        }
        // In: shrinks from large to full size. Out: keeps shrinking.
        "zoom_out" => {
            m.scale = 1.0 + 0.6 * v * dir;
            m.opacity = 1.0 - v;
        }
        "slide_left" => {
            m.dx = w * v * dir;
            m.opacity = 1.0 - v * 0.3;
        }
        "slide_right" => {
            m.dx = -w * v * dir;
            m.opacity = 1.0 - v * 0.3;
        }
        "slide_up" => {
            m.dy = h * v * dir;
            m.opacity = 1.0 - v * 0.3;
        }
        "slide_down" => {
            m.dy = -h * v * dir;
            m.opacity = 1.0 - v * 0.3;
        }
        "rise" => {
            m.dy = h * 0.12 * v;
            m.opacity = 1.0 - v;
        }
        "drop" => {
            m.dy = -h * 0.12 * v;
            m.opacity = 1.0 - v;
        }
        "pop" => {
            let s = match phase {
                Phase::In => ease_out_back(p),
                Phase::Out => ease_out_back(1.0 - p),
            };
            m.scale = s.max(0.0);
            m.opacity = (1.0 - v * 1.5).clamp(0.0, 1.0);
        }
        "bounce" => {
            let b = match phase {
                Phase::In => 1.0 - ease_out_bounce(p),
                Phase::Out => p * p,
            };
            m.dy = -h * 0.6 * b * dir;
        }
        "spin" => {
            m.rotation = 360.0 * v * dir;
            m.scale = 1.0 - 0.8 * v;
            m.opacity = 1.0 - v;
        }
        "rotate" => {
            m.rotation = -25.0 * v * dir;
            m.opacity = 1.0 - v;
        }
        "flip" => {
            m.stretch = [(1.0 - v).max(0.0), 1.0];
        }
        "stretch" => {
            m.stretch = [1.0 + 1.5 * v, (1.0 - v).max(0.02)];
            m.opacity = 1.0 - v;
        }
        "squeeze" => {
            m.stretch = [(1.0 - v).max(0.02), 1.0 + 0.5 * v];
            m.opacity = 1.0 - v;
        }
        "blur" => {
            m.blur = v;
            m.opacity = 1.0 - v * v;
        }
        "wipe_left" => m.crop[0] = v,
        "wipe_right" => m.crop[2] = v,
        "wipe_up" => m.crop[1] = v,
        "wipe_down" => m.crop[3] = v,
        "typewriter" => {
            m.reveal = match phase {
                Phase::In => p,
                Phase::Out => 1.0 - p,
            }
        }
        "slam" => {
            // Drops in from huge and blurred; leaves by swelling away.
            m.scale = match phase {
                Phase::In => 1.0 + 1.6 * v,
                Phase::Out => 1.0 + 0.6 * v,
            };
            m.blur = match phase {
                Phase::In => v * 0.8,
                Phase::Out => v * 0.5,
            };
            m.opacity = match phase {
                Phase::In => (p * 5.0).min(1.0),
                Phase::Out => 1.0 - v,
            };
        }
        "flicker" => {
            let q = match phase {
                Phase::In => p,
                Phase::Out => 1.0 - p,
            };
            m.opacity = flicker(q);
        }
        "letters_rise" | "letters_pop" | "words_pop" | "tracking" => {
            let kind = match id {
                "letters_rise" => LetterKind::Rise,
                "letters_pop" => LetterKind::Pop,
                "words_pop" => LetterKind::WordsPop,
                _ => LetterKind::Tracking,
            };
            m.letters = Some((
                kind,
                match phase {
                    Phase::In => p,
                    Phase::Out => 1.0 - p,
                },
            ));
        }
        "karaoke" if phase == Phase::In => m.letters = Some((LetterKind::Karaoke, p)),
        _ => {}
    }
    m
}

/// Looping sticker motions: (id, name).
pub static STICKER_MOTIONS: &[(&str, &str)] = &[
    ("none", "None"),
    ("bounce", "Bounce"),
    ("pulse", "Pulse"),
    ("spin", "Spin"),
    ("float", "Float"),
    ("wiggle", "Wiggle"),
    ("swing", "Swing"),
    ("heartbeat", "Heartbeat"),
    ("shake", "Shake"),
];

/// A sticker's looping motion `t` seconds into the clip. `size` is the
/// sticker height in project pixels.
pub fn sticker_motion(id: &str, t: f32, size: f32) -> Motion {
    let mut m = Motion::NONE;
    let tau = std::f32::consts::TAU;
    match id {
        "bounce" => {
            let ph = (t * 1.6).fract();
            m.dy = -size * 0.18 * (ph * std::f32::consts::PI).sin().abs();
            let squash = (1.0 - (ph * tau).cos()) * 0.5;
            m.stretch = [1.0 + 0.04 * (1.0 - squash), 1.0 - 0.04 * (1.0 - squash)];
        }
        "pulse" => m.scale = 1.0 + 0.08 * (t * tau * 1.2).sin(),
        "spin" => m.rotation = (t * 120.0) % 360.0,
        "float" => {
            m.dy = size * 0.06 * (t * tau * 0.5).sin();
            m.rotation = 3.0 * (t * tau * 0.25).sin();
        }
        "wiggle" => m.rotation = 8.0 * (t * tau * 2.0).sin(),
        "swing" => m.rotation = 15.0 * (t * tau * 0.8).sin(),
        "heartbeat" => {
            let ph = (t * 1.2).fract();
            let beat =
                (-((ph - 0.1) * 30.0).powi(2)).exp() + 0.6 * (-((ph - 0.3) * 30.0).powi(2)).exp();
            m.scale = 1.0 + 0.12 * beat;
        }
        "shake" => {
            m.dx = size * 0.03 * (t * 47.0).sin();
            m.dy = size * 0.02 * (t * 61.0).cos();
        }
        _ => {}
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animations_rest_at_their_ends() {
        let canvas = [1920.0, 1080.0];
        for a in ANIMATIONS {
            let rest_in = evaluate(a.id, Phase::In, 1.0, canvas);
            let rest_out = evaluate(a.id, Phase::Out, 0.0, canvas);
            for m in [rest_in, rest_out] {
                assert!(m.dx.abs() < 1e-3 && m.dy.abs() < 1e-3, "{}", a.id);
                assert!((m.scale - 1.0).abs() < 1e-3, "{} scale {}", a.id, m.scale);
                assert!((m.opacity - 1.0).abs() < 1e-3, "{}", a.id);
                assert!(m.crop.iter().all(|c| c.abs() < 1e-3), "{}", a.id);
                assert!((m.reveal - 1.0).abs() < 1e-3, "{}", a.id);
            }
        }
    }
}
