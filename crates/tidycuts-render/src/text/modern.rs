//! Modern title looks: the text is drawn white as usual, turned into a
//! distance map and lit by the `sdf` kit, which gives crisp rims, glows,
//! stacked 3D depth and bevelled, glossy surfaces at any size.
//!
//! Each look takes its colours from the title: `color` is the face and
//! `accent` the side, rim or glow.

use super::raster;
use crate::sdf::{self, Bevel, Extrude, Field, Fill, Glow, Look, Outline, Shadow};
use tidycuts_core::TitleSpec;
use tiny_skia::Pixmap;

type C = [f32; 4];

fn tint(c: C, k: f32) -> C {
    [
        c[0] + (1.0 - c[0]) * k,
        c[1] + (1.0 - c[1]) * k,
        c[2] + (1.0 - c[2]) * k,
        c[3],
    ]
}

fn shade(c: C, k: f32) -> C {
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}

fn with_alpha(c: C, a: f32) -> C {
    [c[0], c[1], c[2], c[3] * a]
}

/// The look of `spec.style`, sized for a font of `em` pixels, or `None` if
/// the style is not one of the modern ones.
pub fn look_for(spec: &TitleSpec, em: f32) -> Option<Look> {
    let (face, accent) = (spec.color, spec.accent);
    let white = [1.0, 1.0, 1.0, 1.0];
    Some(match spec.style.as_str() {
        "gold_bevel" => Look {
            fill: Fill {
                top: tint(face, 0.55),
                bottom: shade(face, 0.8),
            },
            outline: Some(Outline {
                color: shade(accent, 0.3),
                width: 0.03 * em,
            }),
            glow: None,
            shadow: Some(Shadow {
                color: [0.0, 0.0, 0.0, 0.55],
                offset: (0.07 * em, 0.15 * em),
                blur: 0.06 * em,
            }),
            extrude: Some(Extrude {
                color: shade(accent, 0.5),
                depth: 0.11 * em,
                dir: (0.45, 1.0),
            }),
            bevel: Some(Bevel {
                width: 0.08 * em,
                height: 0.08 * em,
                light: (-0.55, -0.85),
                highlight: tint(face, 0.8),
                shade: shade(accent, 0.5),
                shine: 0.95,
                sharpness: 40.0,
            }),
        },
        "neon_tube" => Look {
            fill: Fill {
                top: tint(face, 0.85),
                bottom: tint(face, 0.7),
            },
            outline: Some(Outline {
                color: accent,
                width: 0.045 * em,
            }),
            glow: Some(Glow {
                color: accent,
                radius: 0.13 * em,
                strength: 0.95,
            }),
            shadow: None,
            extrude: None,
            bevel: None,
        },
        "pop_3d" => Look {
            fill: Fill {
                top: tint(face, 0.4),
                bottom: shade(face, 0.9),
            },
            outline: Some(Outline {
                color: white,
                width: 0.055 * em,
            }),
            glow: None,
            shadow: Some(Shadow {
                color: [0.0, 0.0, 0.0, 0.5],
                offset: (0.03 * em, 0.13 * em),
                blur: 0.08 * em,
            }),
            extrude: Some(Extrude {
                color: accent,
                depth: 0.16 * em,
                dir: (0.2, 1.0),
            }),
            bevel: Some(Bevel {
                width: 0.13 * em,
                height: 0.12 * em,
                light: (-0.4, -0.9),
                highlight: tint(face, 0.85),
                shade: shade(face, 0.45),
                shine: 0.9,
                sharpness: 60.0,
            }),
        },
        "glass_soft" => Look {
            fill: Fill {
                top: with_alpha(white, 0.95),
                bottom: with_alpha(tint(face, 0.3), 0.85),
            },
            outline: Some(Outline {
                color: with_alpha(white, 0.35),
                width: 0.025 * em,
            }),
            glow: Some(Glow {
                color: tint(accent, 0.5),
                radius: 0.18 * em,
                strength: 0.35,
            }),
            shadow: Some(Shadow {
                color: [0.0, 0.0, 0.1, 0.4],
                offset: (0.0, 0.1 * em),
                blur: 0.1 * em,
            }),
            extrude: None,
            bevel: Some(Bevel {
                width: 0.07 * em,
                height: 0.06 * em,
                light: (-0.5, -0.9),
                highlight: white,
                shade: shade(tint(face, 0.3), 0.7),
                shine: 0.8,
                sharpness: 80.0,
            }),
        },
        // The quiet family: no bevel, rim or colour burst. Just good type,
        // a soft shadow and, at most, a whisper of gradient or glow.
        "clean_soft" => Look {
            fill: Fill {
                top: face,
                bottom: shade(face, 0.9),
            },
            outline: None,
            glow: None,
            shadow: Some(Shadow {
                color: [0.0, 0.0, 0.0, 0.32],
                offset: (0.0, 0.05 * em),
                blur: 0.07 * em,
            }),
            extrude: None,
            bevel: None,
        },
        "clean_gradient" => Look {
            fill: Fill {
                top: face,
                bottom: accent,
            },
            outline: None,
            glow: Some(Glow {
                color: accent,
                radius: 0.3 * em,
                strength: 0.16,
            }),
            shadow: Some(Shadow {
                color: [0.0, 0.0, 0.0, 0.25],
                offset: (0.0, 0.04 * em),
                blur: 0.08 * em,
            }),
            extrude: None,
            bevel: None,
        },
        "clean_glow" => Look {
            fill: Fill {
                top: face,
                bottom: shade(face, 0.93),
            },
            outline: None,
            glow: Some(Glow {
                color: tint(accent, 0.55),
                radius: 0.34 * em,
                strength: 0.3,
            }),
            shadow: None,
            extrude: None,
            bevel: None,
        },
        _ => return None,
    })
}

/// Paints `look` onto a title. `opaque` is the text with every letter at
/// full strength (so the shape is steady); `faded` is the same text with
/// the letter animation's fades applied, if there is one. Where letters are
/// fading in, the lit result fades with them.
pub fn paint(opaque: &Pixmap, faded: Option<&Pixmap>, look: &Look) -> Vec<u8> {
    let (w, h) = (opaque.width() as usize, opaque.height() as usize);
    let shape = raster::alpha_of(opaque.data());
    let field = Field::from_alpha(&shape, opaque.width(), opaque.height());
    let mut img = sdf::render(&field, look);
    if let Some(faded) = faded {
        // How faded the nearby letters are: the faded mask against the full
        // one, both softened so the fade carries into glows and shadows.
        let radius = (look.reach() * 0.5).max(2.0);
        let soft_full = raster::blurred(&shape, w, h, radius);
        let soft_faded = raster::blurred(&raster::alpha_of(faded.data()), w, h, radius);
        for (i, px) in img.pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let full = soft_full[i] as f32;
            if full < 1.0 {
                continue;
            }
            let k = (soft_faded[i] as f32 / full).clamp(0.0, 1.0);
            for v in px.iter_mut() {
                *v = (*v as f32 * k + 0.5) as u8;
            }
        }
    }
    img.pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_modern_style_has_a_look() {
        // The ids are listed (with their names) in `text::STYLES`.
        let mut spec = TitleSpec::default();
        for id in [
            "gold_bevel",
            "neon_tube",
            "pop_3d",
            "glass_soft",
            "clean_soft",
            "clean_gradient",
            "clean_glow",
        ] {
            spec.style = id.into();
            assert!(look_for(&spec, 100.0).is_some(), "{id}");
            assert!(
                crate::text::STYLES.iter().any(|(s, _)| *s == id),
                "{id} is not in the style list"
            );
        }
        spec.style = "plain".into();
        assert!(look_for(&spec, 100.0).is_none());
    }

    #[test]
    fn looks_scale_with_the_font_size() {
        let spec = TitleSpec {
            style: "pop_3d".into(),
            ..Default::default()
        };
        let small = look_for(&spec, 50.0).unwrap().reach();
        let big = look_for(&spec, 200.0).unwrap().reach();
        assert!((big / small - 4.0).abs() < 0.05, "{small} -> {big}");
    }
}
