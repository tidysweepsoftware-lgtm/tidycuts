//! Filters: color looks built from the grade pass. Applied to a clip as an
//! effect whose kind is the filter id, with one "strength" parameter.

use super::{ParamDef, Sample};
use crate::Grade;

#[derive(Debug, Clone, Copy)]
pub struct FilterDef {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub grade: fn() -> Grade,
    pub sample: Sample,
}

pub const STRENGTH: ParamDef = ParamDef::percent("strength", "Strength", 1.0);

const fn filter(
    id: &'static str,
    name: &'static str,
    category: &'static str,
    grade: fn() -> Grade,
    sample: Sample,
) -> FilterDef {
    FilterDef {
        id,
        name,
        category,
        grade,
        sample,
    }
}

use Sample::{Landscape, Meadow, Nap, Pet, Portrait, Sunset, Wildlife};

/// A gradient map: every colour is replaced by a point on a three-colour
/// gradient (dark, middle, light) chosen by how bright it is. It is done
/// with a small colour lookup table made here, once per gradient.
fn gradient_map(dark: [f32; 3], mid: [f32; 3], light: [f32; 3]) -> Grade {
    use crate::looks::Looks;
    use crate::lut::Lut3d;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};

    static CACHE: OnceLock<Mutex<HashMap<[u32; 9], Arc<Looks>>>> = OnceLock::new();
    let key = [
        dark[0].to_bits(),
        dark[1].to_bits(),
        dark[2].to_bits(),
        mid[0].to_bits(),
        mid[1].to_bits(),
        mid[2].to_bits(),
        light[0].to_bits(),
        light[1].to_bits(),
        light[2].to_bits(),
    ];
    let cache = CACHE.get_or_init(Default::default);
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    let looks = cache
        .entry(key)
        .or_insert_with(|| {
            let mix = |a: [f32; 3], b: [f32; 3], k: f32| {
                [
                    a[0] + (b[0] - a[0]) * k,
                    a[1] + (b[1] - a[1]) * k,
                    a[2] + (b[2] - a[2]) * k,
                ]
            };
            let lut = Lut3d::from_fn(17, |c| {
                let t = (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]).clamp(0.0, 1.0);
                if t < 0.5 {
                    mix(dark, mid, t * 2.0)
                } else {
                    mix(mid, light, (t - 0.5) * 2.0)
                }
            });
            Arc::new(Looks {
                tone: None,
                curves: false,
                hsl: false,
                lut: Some(Arc::new(lut)),
                lut_strength: 1.0,
            })
        })
        .clone();
    Grade {
        looks: Some(looks),
        ..Default::default()
    }
}

// Note on toned monochrome looks (Sepia, Cyanotype): `grade.wgsl` applies
// split toning before desaturating, so a tint can't survive full
// desaturation. These looks keep a little color and push strong tints.

pub static FILTERS: &[FilterDef] = &[
    // ---- Basic ----
    filter(
        "filter.vivid",
        "Vivid",
        "Basic",
        || Grade {
            contrast: 0.15,
            saturation: 0.35,
            vibrance: 0.25,
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.bright",
        "Bright",
        "Basic",
        || Grade {
            exposure: 0.12,
            brightness: 0.12,
            shadows: 0.25,
            vibrance: 0.1,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.crisp",
        "Crisp",
        "Basic",
        || Grade {
            contrast: 0.3,
            whites: 0.25,
            blacks: -0.25,
            vibrance: 0.15,
            ..Default::default()
        },
        Landscape,
    ),
    filter(
        "filter.soft",
        "Soft",
        "Basic",
        || Grade {
            contrast: -0.25,
            highlights: -0.2,
            saturation: -0.05,
            fade: 0.2,
            ..Default::default()
        },
        Pet,
    ),
    filter(
        "filter.punch",
        "Punch",
        "Basic",
        || Grade {
            contrast: 0.4,
            saturation: 0.2,
            shadows: -0.1,
            vignette: 0.25,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.muted",
        "Muted",
        "Basic",
        || Grade {
            contrast: -0.1,
            saturation: -0.45,
            fade: 0.12,
            ..Default::default()
        },
        Wildlife,
    ),
    // ---- Minimal ----
    filter(
        "filter.soft_neutral",
        "Soft Neutral",
        "Minimal",
        || Grade {
            exposure: 0.12,
            contrast: -0.12,
            saturation: -0.18,
            fade: 0.1,
            highlights: -0.1,
            shadows: 0.15,
            temperature: 0.02,
            ..Default::default()
        },
        Pet,
    ),
    filter(
        "filter.matte_calm",
        "Matte Calm",
        "Minimal",
        || Grade {
            fade: 0.22,
            contrast: -0.1,
            saturation: -0.22,
            temperature: 0.06,
            blacks: 0.1,
            highlight_tint: [1.0, 0.92, 0.82, 0.15],
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.airy_white",
        "Airy White",
        "Minimal",
        || Grade {
            exposure: 0.2,
            whites: 0.2,
            contrast: -0.15,
            saturation: -0.15,
            shadows: 0.2,
            fade: 0.08,
            ..Default::default()
        },
        Nap,
    ),
    filter(
        "filter.grey_space",
        "Grey Space",
        "Minimal",
        || Grade {
            saturation: -0.55,
            contrast: -0.08,
            fade: 0.15,
            shadows: 0.1,
            highlight_tint: [0.85, 0.9, 0.95, 0.12],
            ..Default::default()
        },
        Wildlife,
    ),
    filter(
        "filter.linen",
        "Linen",
        "Minimal",
        || Grade {
            temperature: 0.1,
            saturation: -0.28,
            exposure: 0.1,
            fade: 0.14,
            highlight_tint: [1.0, 0.95, 0.85, 0.2],
            contrast: -0.1,
            ..Default::default()
        },
        Pet,
    ),
    filter(
        "filter.studio_clean",
        "Studio Clean",
        "Minimal",
        || Grade {
            contrast: 0.05,
            saturation: -0.12,
            temperature: -0.02,
            exposure: 0.08,
            whites: 0.1,
            vibrance: 0.05,
            shadows: 0.1,
            ..Default::default()
        },
        Portrait,
    ),
    // ---- Pastel ----
    filter(
        "filter.hazy_pastel",
        "Hazy Pastel",
        "Pastel",
        || Grade {
            fade: 0.26,
            contrast: -0.18,
            saturation: -0.1,
            exposure: 0.1,
            highlight_tint: [1.0, 0.78, 0.88, 0.39],
            shadow_tint: [0.55, 0.7, 0.95, 0.2],
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.blush_dusk",
        "Blush Dusk",
        "Pastel",
        || Grade {
            temperature: 0.05,
            tint: 0.12,
            fade: 0.22,
            saturation: -0.08,
            contrast: -0.1,
            highlight_tint: [1.0, 0.7, 0.78, 0.42],
            shadow_tint: [0.6, 0.5, 0.75, 0.2],
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.lilac_haze",
        "Lilac Haze",
        "Pastel",
        || Grade {
            tint: 0.15,
            temperature: -0.05,
            fade: 0.24,
            saturation: -0.1,
            exposure: 0.08,
            highlight_tint: [0.9, 0.8, 1.0, 0.35],
            ..Default::default()
        },
        Meadow,
    ),
    filter(
        "filter.cotton_candy",
        "Cotton Candy",
        "Pastel",
        || Grade {
            saturation: 0.05,
            fade: 0.26,
            contrast: -0.15,
            exposure: 0.12,
            highlight_tint: [1.0, 0.75, 0.9, 0.42],
            shadow_tint: [0.5, 0.85, 0.95, 0.25],
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.peach_milk",
        "Peach Milk",
        "Pastel",
        || Grade {
            temperature: 0.2,
            tint: 0.05,
            fade: 0.21,
            saturation: -0.1,
            exposure: 0.1,
            contrast: -0.1,
            highlight_tint: [1.0, 0.8, 0.65, 0.39],
            ..Default::default()
        },
        Nap,
    ),
    filter(
        "filter.sky_dream",
        "Sky Dream",
        "Pastel",
        || Grade {
            temperature: -0.12,
            fade: 0.21,
            exposure: 0.1,
            saturation: -0.05,
            highlight_tint: [0.8, 0.9, 1.0, 0.35],
            shadow_tint: [0.5, 0.7, 0.95, 0.2],
            ..Default::default()
        },
        Landscape,
    ),
    // ---- Creator ----
    filter(
        "filter.creator_clean",
        "Creator Clean",
        "Creator",
        || Grade {
            contrast: 0.12,
            vibrance: 0.2,
            saturation: 0.05,
            whites: 0.12,
            shadows: 0.1,
            temperature: 0.03,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.punchy_vlog",
        "Punchy Vlog",
        "Creator",
        || Grade {
            contrast: 0.25,
            vibrance: 0.3,
            saturation: 0.1,
            shadows: 0.05,
            whites: 0.1,
            vignette: 0.1,
            ..Default::default()
        },
        Wildlife,
    ),
    filter(
        "filter.talking_head",
        "Talking Head",
        "Creator",
        || Grade {
            contrast: 0.08,
            shadows: 0.12,
            temperature: 0.05,
            vibrance: 0.1,
            highlights: -0.08,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.skin_first",
        "Skin First",
        "Creator",
        || Grade {
            saturation: -0.08,
            vibrance: -0.05,
            contrast: 0.05,
            temperature: 0.07,
            tint: 0.03,
            highlights: -0.05,
            shadows: 0.1,
            highlight_tint: [1.0, 0.88, 0.78, 0.12],
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.daylight_pop",
        "Daylight Pop",
        "Creator",
        || Grade {
            exposure: 0.08,
            contrast: 0.15,
            vibrance: 0.35,
            saturation: 0.08,
            whites: 0.15,
            temperature: -0.02,
            ..Default::default()
        },
        Landscape,
    ),
    filter(
        "filter.tech_cool",
        "Tech Cool",
        "Creator",
        || Grade {
            temperature: -0.1,
            contrast: 0.15,
            saturation: -0.05,
            whites: 0.1,
            shadow_tint: [0.2, 0.45, 0.7, 0.2],
            ..Default::default()
        },
        Pet,
    ),
    // ---- Lifestyle ----
    filter(
        "filter.soft_light",
        "Soft Light",
        "Lifestyle",
        || Grade {
            exposure: 0.15,
            contrast: -0.1,
            highlights: -0.15,
            shadows: 0.15,
            temperature: 0.06,
            fade: 0.08,
            saturation: -0.05,
            ..Default::default()
        },
        Nap,
    ),
    filter(
        "filter.cozy_home",
        "Cozy Home",
        "Lifestyle",
        || Grade {
            temperature: 0.2,
            saturation: -0.05,
            contrast: -0.05,
            shadows: 0.1,
            vignette: 0.12,
            highlight_tint: [1.0, 0.82, 0.6, 0.25],
            ..Default::default()
        },
        Pet,
    ),
    filter(
        "filter.cafe",
        "Cafe",
        "Lifestyle",
        || Grade {
            temperature: 0.15,
            contrast: 0.1,
            saturation: -0.12,
            fade: 0.1,
            shadow_tint: [0.45, 0.3, 0.2, 0.2],
            highlight_tint: [1.0, 0.9, 0.75, 0.2],
            ..Default::default()
        },
        Nap,
    ),
    filter(
        "filter.morning_window",
        "Morning Window",
        "Lifestyle",
        || Grade {
            exposure: 0.2,
            temperature: 0.08,
            whites: 0.2,
            contrast: -0.12,
            saturation: -0.1,
            fade: 0.08,
            ..Default::default()
        },
        Pet,
    ),
    filter(
        "filter.fresh",
        "Fresh",
        "Lifestyle",
        || Grade {
            temperature: -0.03,
            exposure: 0.1,
            vibrance: 0.2,
            tint: -0.05,
            whites: 0.12,
            contrast: 0.05,
            shadows: 0.1,
            ..Default::default()
        },
        Meadow,
    ),
    filter(
        "filter.daily_warm",
        "Daily Warm",
        "Lifestyle",
        || Grade {
            temperature: 0.12,
            contrast: 0.05,
            vibrance: 0.1,
            shadows: 0.1,
            fade: 0.05,
            ..Default::default()
        },
        Portrait,
    ),
    // ---- Cinematic ----
    filter(
        "filter.teal_orange",
        "Teal & Orange",
        "Cinematic",
        || Grade {
            contrast: 0.25,
            saturation: 0.15,
            shadow_tint: [0.0, 0.55, 0.65, 0.35],
            highlight_tint: [1.0, 0.62, 0.25, 0.3],
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.blockbuster",
        "Blockbuster",
        "Cinematic",
        || Grade {
            contrast: 0.35,
            blacks: -0.15,
            temperature: -0.15,
            saturation: 0.1,
            shadow_tint: [0.1, 0.25, 0.65, 0.35],
            highlight_tint: [1.0, 0.8, 0.45, 0.3],
            vignette: 0.35,
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.bleach_bypass",
        "Bleach Bypass",
        "Cinematic",
        || Grade {
            contrast: 0.45,
            saturation: -0.55,
            highlights: 0.1,
            blacks: -0.1,
            shadow_tint: [0.3, 0.45, 0.5, 0.15],
            ..Default::default()
        },
        Nap,
    ),
    filter(
        "filter.moody",
        "Moody",
        "Cinematic",
        || Grade {
            exposure: -0.15,
            contrast: 0.2,
            saturation: -0.3,
            shadow_tint: [0.15, 0.35, 0.4, 0.3],
            fade: 0.12,
            vignette: 0.45,
            ..Default::default()
        },
        Meadow,
    ),
    filter(
        "filter.matte",
        "Matte",
        "Cinematic",
        || Grade {
            contrast: -0.1,
            saturation: -0.1,
            fade: 0.45,
            highlight_tint: [1.0, 0.9, 0.8, 0.1],
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.thriller",
        "Thriller",
        "Cinematic",
        || Grade {
            contrast: 0.25,
            saturation: -0.35,
            tint: 0.2,
            shadow_tint: [0.15, 0.45, 0.35, 0.3],
            highlight_tint: [0.95, 0.9, 0.6, 0.15],
            vignette: 0.3,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.desert",
        "Desert",
        "Cinematic",
        || Grade {
            contrast: 0.25,
            temperature: 0.75,
            tint: -0.08,
            saturation: -0.05,
            highlight_tint: [1.0, 0.65, 0.3, 0.4],
            shadow_tint: [0.45, 0.25, 0.1, 0.25],
            ..Default::default()
        },
        Landscape,
    ),
    filter(
        "filter.day_for_night",
        "Day for Night",
        "Cinematic",
        || Grade {
            exposure: -0.6,
            contrast: 0.15,
            temperature: -0.6,
            saturation: -0.4,
            shadow_tint: [0.1, 0.2, 0.6, 0.3],
            vignette: 0.4,
            ..Default::default()
        },
        Pet,
    ),
    filter(
        "filter.neon_night",
        "Neon Night",
        "Cinematic",
        || Grade {
            contrast: 0.3,
            saturation: 0.3,
            temperature: -0.3,
            tint: -0.35,
            shadow_tint: [0.55, 0.1, 0.9, 0.3],
            highlight_tint: [0.1, 0.9, 1.0, 0.3],
            ..Default::default()
        },
        Sunset,
    ),
    // ---- Night ----
    filter(
        "filter.midnight",
        "Midnight",
        "Night",
        || Grade {
            exposure: -0.2,
            contrast: 0.2,
            temperature: -0.15,
            saturation: -0.1,
            vignette: 0.25,
            shadow_tint: [0.1, 0.2, 0.5, 0.3],
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.neon_rain",
        "Neon Rain",
        "Night",
        || Grade {
            contrast: 0.25,
            saturation: 0.2,
            exposure: -0.1,
            vignette: 0.2,
            shadow_tint: [0.1, 0.3, 0.7, 0.3],
            highlight_tint: [1.0, 0.3, 0.7, 0.25],
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.smoky",
        "Smoky",
        "Night",
        || Grade {
            contrast: 0.15,
            saturation: -0.35,
            exposure: -0.1,
            fade: 0.08,
            vignette: 0.25,
            shadow_tint: [0.3, 0.35, 0.4, 0.2],
            ..Default::default()
        },
        Pet,
    ),
    filter(
        "filter.cold_alley",
        "Cold Alley",
        "Night",
        || Grade {
            temperature: -0.2,
            contrast: 0.2,
            saturation: -0.15,
            exposure: -0.12,
            vignette: 0.2,
            shadow_tint: [0.05, 0.3, 0.4, 0.3],
            ..Default::default()
        },
        Landscape,
    ),
    filter(
        "filter.city_glow",
        "City Glow",
        "Night",
        || Grade {
            contrast: 0.2,
            saturation: 0.1,
            exposure: -0.08,
            vignette: 0.15,
            highlight_tint: [1.0, 0.7, 0.35, 0.3],
            shadow_tint: [0.1, 0.25, 0.5, 0.25],
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.low_light",
        "Low Light",
        "Night",
        || Grade {
            exposure: -0.25,
            shadows: 0.2,
            contrast: 0.1,
            saturation: -0.15,
            vignette: 0.3,
            ..Default::default()
        },
        Nap,
    ),
    // ---- Nature ----
    filter(
        "filter.earthy",
        "Earthy",
        "Nature",
        || Grade {
            temperature: 0.12,
            saturation: -0.15,
            contrast: 0.1,
            fade: 0.08,
            shadow_tint: [0.3, 0.25, 0.15, 0.2],
            ..Default::default()
        },
        Meadow,
    ),
    filter(
        "filter.botanical",
        "Botanical",
        "Nature",
        || Grade {
            saturation: -0.05,
            vibrance: 0.2,
            tint: -0.1,
            temperature: -0.02,
            contrast: 0.1,
            shadows: 0.1,
            ..Default::default()
        },
        Landscape,
    ),
    filter(
        "filter.moss",
        "Moss",
        "Nature",
        || Grade {
            tint: -0.15,
            saturation: -0.15,
            contrast: 0.15,
            exposure: -0.05,
            shadow_tint: [0.1, 0.25, 0.15, 0.25],
            ..Default::default()
        },
        Meadow,
    ),
    filter(
        "filter.forest_mist",
        "Forest Mist",
        "Nature",
        || Grade {
            fade: 0.18,
            contrast: -0.1,
            tint: -0.08,
            temperature: -0.04,
            saturation: -0.15,
            highlight_tint: [0.85, 0.95, 0.9, 0.2],
            ..Default::default()
        },
        Landscape,
    ),
    filter(
        "filter.wildflower",
        "Wildflower",
        "Nature",
        || Grade {
            vibrance: 0.3,
            saturation: 0.1,
            exposure: 0.05,
            contrast: 0.08,
            temperature: 0.04,
            highlights: -0.05,
            ..Default::default()
        },
        Wildlife,
    ),
    filter(
        "filter.dry_grass",
        "Dry Grass",
        "Nature",
        || Grade {
            temperature: 0.25,
            saturation: -0.1,
            contrast: 0.12,
            tint: 0.03,
            highlight_tint: [1.0, 0.85, 0.5, 0.25],
            ..Default::default()
        },
        Meadow,
    ),
    // ---- Travel ----
    filter(
        "filter.tropical",
        "Tropical",
        "Travel",
        || Grade {
            vibrance: 0.35,
            saturation: 0.1,
            temperature: 0.05,
            tint: -0.03,
            contrast: 0.12,
            shadow_tint: [0.0, 0.4, 0.5, 0.2],
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.sandy",
        "Sandy",
        "Travel",
        || Grade {
            temperature: 0.2,
            saturation: -0.1,
            exposure: 0.12,
            contrast: -0.05,
            whites: 0.15,
            fade: 0.08,
            ..Default::default()
        },
        Wildlife,
    ),
    filter(
        "filter.ocean_breeze",
        "Ocean Breeze",
        "Travel",
        || Grade {
            temperature: -0.12,
            vibrance: 0.25,
            exposure: 0.08,
            saturation: 0.03,
            highlight_tint: [0.8, 0.95, 1.0, 0.2],
            ..Default::default()
        },
        Landscape,
    ),
    filter(
        "filter.city_walk",
        "City Walk",
        "Travel",
        || Grade {
            contrast: 0.15,
            saturation: -0.15,
            vibrance: 0.1,
            temperature: 0.02,
            shadow_tint: [0.2, 0.3, 0.4, 0.15],
            ..Default::default()
        },
        Pet,
    ),
    filter(
        "filter.road_trip",
        "Road Trip",
        "Travel",
        || Grade {
            temperature: 0.15,
            contrast: 0.15,
            saturation: 0.05,
            fade: 0.1,
            vignette: 0.12,
            highlight_tint: [1.0, 0.8, 0.5, 0.25],
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.island_blue",
        "Island Blue",
        "Travel",
        || Grade {
            temperature: -0.08,
            vibrance: 0.3,
            saturation: 0.1,
            exposure: 0.05,
            shadow_tint: [0.0, 0.3, 0.6, 0.25],
            ..Default::default()
        },
        Landscape,
    ),
    // ---- Mono ----
    filter(
        "filter.bw",
        "Black & White",
        "Mono",
        || Grade {
            mono: true,
            contrast: 0.15,
            ..Default::default()
        },
        Wildlife,
    ),
    filter(
        "filter.noir",
        "Noir",
        "Mono",
        || Grade {
            mono: true,
            contrast: 0.5,
            vignette: 0.5,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.high_key",
        "High-Key",
        "Mono",
        || Grade {
            mono: true,
            exposure: 0.3,
            shadows: 0.3,
            contrast: 0.2,
            highlights: 0.1,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.silver",
        "Silver",
        "Mono",
        || Grade {
            mono: true,
            contrast: 0.25,
            highlights: -0.2,
            fade: 0.25,
            ..Default::default()
        },
        Nap,
    ),
    filter(
        "filter.grit",
        "Grit",
        "Mono",
        || Grade {
            mono: true,
            contrast: 0.6,
            blacks: -0.2,
            grain: 0.5,
            vignette: 0.3,
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.sepia",
        "Sepia",
        "Mono",
        || Grade {
            saturation: -0.8,
            temperature: 0.9,
            contrast: 0.05,
            shadow_tint: [1.0, 0.55, 0.2, 2.4],
            highlight_tint: [1.0, 0.72, 0.4, 3.0],
            fade: 0.12,
            vignette: 0.25,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.cyanotype",
        "Cyanotype",
        "Mono",
        || Grade {
            saturation: -0.75,
            temperature: -0.6,
            contrast: 0.15,
            shadow_tint: [0.0, 0.3, 1.0, 2.2],
            highlight_tint: [0.5, 0.75, 1.0, 1.4],
            fade: 0.1,
            ..Default::default()
        },
        Portrait,
    ),
    // ---- Vintage ----
    filter(
        "filter.faded",
        "Faded",
        "Vintage",
        || Grade {
            contrast: -0.15,
            saturation: -0.25,
            temperature: 0.1,
            fade: 0.5,
            ..Default::default()
        },
        Meadow,
    ),
    filter(
        "filter.seventies",
        "70s",
        "Vintage",
        || Grade {
            temperature: 0.55,
            tint: 0.05,
            fade: 0.3,
            highlight_tint: [1.0, 0.8, 0.35, 0.35],
            shadow_tint: [0.5, 0.3, 0.15, 0.3],
            vignette: 0.25,
            ..Default::default()
        },
        Landscape,
    ),
    filter(
        "filter.instant",
        "Instant",
        "Vintage",
        || Grade {
            contrast: -0.05,
            saturation: -0.1,
            temperature: 0.1,
            fade: 0.3,
            highlight_tint: [1.0, 0.95, 0.7, 0.3],
            shadow_tint: [0.15, 0.45, 0.5, 0.35],
            vignette: 0.2,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.toy_camera",
        "Toy Camera",
        "Vintage",
        || Grade {
            contrast: 0.45,
            saturation: 0.4,
            shadow_tint: [0.1, 0.3, 0.55, 0.3],
            highlight_tint: [1.0, 0.9, 0.5, 0.25],
            vignette: 0.85,
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.cross_process",
        "Cross Process",
        "Vintage",
        || Grade {
            contrast: 0.3,
            saturation: 0.25,
            tint: 0.15,
            highlight_tint: [0.9, 1.0, 0.4, 0.4],
            shadow_tint: [0.15, 0.3, 0.85, 0.4],
            ..Default::default()
        },
        Pet,
    ),
    filter(
        "filter.postcard",
        "Old Postcard",
        "Vintage",
        || Grade {
            saturation: -0.35,
            fade: 0.35,
            highlight_tint: [1.0, 0.9, 0.7, 0.35],
            shadow_tint: [0.4, 0.25, 0.55, 0.25],
            grain: 0.15,
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.dusty",
        "Dusty",
        "Vintage",
        || Grade {
            contrast: -0.1,
            saturation: -0.3,
            temperature: 0.25,
            fade: 0.25,
            grain: 0.35,
            vignette: 0.4,
            ..Default::default()
        },
        Portrait,
    ),
    // ---- Portrait ----
    filter(
        "filter.soft_skin",
        "Soft Skin",
        "Portrait",
        || Grade {
            contrast: -0.12,
            highlights: -0.1,
            shadows: 0.15,
            temperature: 0.08,
            saturation: -0.05,
            fade: 0.08,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.radiant",
        "Radiant",
        "Portrait",
        || Grade {
            exposure: 0.12,
            highlights: 0.15,
            contrast: -0.1,
            vibrance: 0.1,
            highlight_tint: [1.0, 0.85, 0.7, 0.25],
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.porcelain",
        "Porcelain",
        "Portrait",
        || Grade {
            exposure: 0.15,
            contrast: -0.05,
            shadows: 0.2,
            highlights: 0.1,
            temperature: -0.1,
            saturation: -0.25,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.peach",
        "Peach",
        "Portrait",
        || Grade {
            temperature: 0.12,
            tint: -0.15,
            saturation: -0.05,
            fade: 0.1,
            highlight_tint: [1.0, 0.72, 0.62, 0.3],
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.bronze",
        "Bronze",
        "Portrait",
        || Grade {
            contrast: 0.2,
            temperature: 0.3,
            saturation: 0.1,
            highlight_tint: [1.0, 0.7, 0.35, 0.3],
            shadow_tint: [0.45, 0.25, 0.1, 0.25],
            vignette: 0.2,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.pastel",
        "Pastel",
        "Portrait",
        || Grade {
            exposure: 0.1,
            contrast: -0.25,
            saturation: -0.15,
            vibrance: 0.2,
            fade: 0.35,
            highlight_tint: [1.0, 0.82, 0.95, 0.25],
            ..Default::default()
        },
        Wildlife,
    ),
    // ---- Warm ----
    filter(
        "filter.golden_hour",
        "Golden Hour",
        "Warm",
        || Grade {
            exposure: 0.05,
            temperature: 0.8,
            tint: -0.05,
            saturation: 0.15,
            highlight_tint: [1.0, 0.72, 0.25, 0.45],
            vignette: 0.15,
            ..Default::default()
        },
        Nap,
    ),
    filter(
        "filter.sunset",
        "Sunset",
        "Warm",
        || Grade {
            temperature: 0.5,
            tint: -0.15,
            saturation: 0.2,
            shadow_tint: [0.6, 0.2, 0.55, 0.3],
            highlight_tint: [1.0, 0.6, 0.3, 0.3],
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.autumn",
        "Autumn",
        "Warm",
        || Grade {
            hue: -6.0,
            temperature: 0.5,
            tint: -0.1,
            saturation: 0.3,
            contrast: 0.2,
            gain: [0.08, 0.0, -0.12],
            shadow_tint: [0.6, 0.3, 0.1, 0.35],
            highlight_tint: [1.0, 0.65, 0.2, 0.25],
            ..Default::default()
        },
        Meadow,
    ),
    filter(
        "filter.honey",
        "Honey",
        "Warm",
        || Grade {
            temperature: 0.25,
            contrast: 0.05,
            highlights: -0.1,
            saturation: -0.05,
            gain: [0.04, 0.02, -0.1],
            fade: 0.15,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.amber",
        "Amber",
        "Warm",
        || Grade {
            temperature: 0.6,
            contrast: 0.25,
            saturation: -0.15,
            vignette: 0.35,
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.summer",
        "Summer",
        "Warm",
        || Grade {
            exposure: 0.12,
            temperature: 0.3,
            vibrance: 0.35,
            saturation: 0.1,
            shadows: 0.15,
            ..Default::default()
        },
        Landscape,
    ),
    // ---- Cool ----
    filter(
        "filter.arctic",
        "Arctic",
        "Cool",
        || Grade {
            exposure: 0.1,
            temperature: -0.5,
            saturation: -0.25,
            highlights: 0.1,
            fade: 0.1,
            ..Default::default()
        },
        Pet,
    ),
    filter(
        "filter.blue_hour",
        "Blue Hour",
        "Cool",
        || Grade {
            exposure: -0.1,
            contrast: 0.1,
            temperature: -0.4,
            tint: -0.05,
            shadow_tint: [0.1, 0.2, 0.7, 0.3],
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.mint",
        "Mint",
        "Cool",
        || Grade {
            temperature: -0.15,
            tint: 0.2,
            saturation: -0.1,
            fade: 0.15,
            highlight_tint: [0.75, 1.0, 0.85, 0.25],
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.frost",
        "Frost",
        "Cool",
        || Grade {
            exposure: 0.15,
            contrast: -0.1,
            temperature: -0.3,
            saturation: -0.45,
            highlight_tint: [0.85, 0.95, 1.0, 0.3],
            ..Default::default()
        },
        Wildlife,
    ),
    filter(
        "filter.ocean",
        "Ocean",
        "Cool",
        || Grade {
            hue: -5.0,
            contrast: 0.15,
            temperature: -0.35,
            tint: 0.1,
            saturation: 0.15,
            shadow_tint: [0.0, 0.45, 0.55, 0.4],
            highlight_tint: [0.6, 0.95, 1.0, 0.2],
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.steel",
        "Steel",
        "Cool",
        || Grade {
            contrast: 0.35,
            blacks: -0.1,
            temperature: -0.25,
            saturation: -0.6,
            vignette: 0.25,
            ..Default::default()
        },
        Sunset,
    ),
    // ---- Film ----
    filter(
        "filter.warm_film",
        "Warm Film",
        "Film",
        || Grade {
            contrast: 0.15,
            temperature: 0.18,
            saturation: 0.1,
            highlight_tint: [1.0, 0.85, 0.55, 0.2],
            shadow_tint: [0.35, 0.2, 0.25, 0.12],
            fade: 0.08,
            grain: 0.2,
            ..Default::default()
        },
        Nap,
    ),
    filter(
        "filter.cool_film",
        "Cool Film",
        "Film",
        || Grade {
            contrast: 0.12,
            temperature: -0.08,
            tint: 0.1,
            saturation: 0.05,
            shadow_tint: [0.1, 0.4, 0.35, 0.2],
            highlight_tint: [0.95, 1.0, 0.9, 0.1],
            fade: 0.06,
            grain: 0.2,
            ..Default::default()
        },
        Meadow,
    ),
    filter(
        "filter.soft_film",
        "Soft Film",
        "Film",
        || Grade {
            contrast: -0.08,
            shadows: 0.1,
            temperature: 0.12,
            saturation: -0.1,
            highlight_tint: [1.0, 0.8, 0.7, 0.15],
            grain: 0.15,
            ..Default::default()
        },
        Portrait,
    ),
    filter(
        "filter.slide_film",
        "Slide Film",
        "Film",
        || Grade {
            contrast: 0.35,
            blacks: -0.1,
            saturation: 0.45,
            vibrance: 0.1,
            temperature: 0.05,
            ..Default::default()
        },
        Landscape,
    ),
    filter(
        "filter.tungsten",
        "Tungsten Night",
        "Film",
        || Grade {
            contrast: 0.15,
            highlights: 0.1,
            temperature: -0.35,
            tint: -0.1,
            shadow_tint: [0.1, 0.35, 0.5, 0.25],
            highlight_tint: [1.0, 0.4, 0.3, 0.15],
            grain: 0.3,
            ..Default::default()
        },
        Sunset,
    ),
    filter(
        "filter.expired",
        "Expired Film",
        "Film",
        || Grade {
            hue: 8.0,
            tint: 0.15,
            temperature: 0.15,
            saturation: -0.15,
            fade: 0.3,
            shadow_tint: [0.5, 0.2, 0.6, 0.3],
            highlight_tint: [0.9, 1.0, 0.6, 0.3],
            grain: 0.4,
            vignette: 0.3,
            ..Default::default()
        },
        Pet,
    ),
    filter(
        "filter.super8",
        "Super 8",
        "Film",
        || Grade {
            contrast: 0.2,
            temperature: 0.3,
            saturation: 0.1,
            fade: 0.2,
            highlight_tint: [1.0, 0.8, 0.5, 0.2],
            grain: 0.5,
            vignette: 0.55,
            ..Default::default()
        },
        Sunset,
    ),
    // ---- Genre, series, tones, stocks, city, maps, recovery, gaming, business, food ----
    filter(
        "filter.genre_noir_rain",
        "Noir Rain",
        "Genre",
        || Grade {
            saturation: -0.75,
            contrast: 0.4,
            exposure: -0.12,
            blacks: -0.2,
            shadow_tint: [0.1, 0.15, 0.3, 0.3],
            highlight_tint: [0.8, 0.9, 1.0, 0.2],
            vignette: 0.45,
            grain: 0.25,
            ..Default::default()
        },
        Sample::Night,
    ),
    filter(
        "filter.genre_scifi_cold",
        "Sci-Fi Cold",
        "Genre",
        || Grade {
            temperature: -0.3,
            tint: -0.1,
            contrast: 0.25,
            saturation: -0.1,
            shadow_tint: [0.0, 0.3, 0.55, 0.4],
            highlight_tint: [0.7, 0.95, 1.0, 0.25],
            vignette: 0.2,
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.genre_western_gold",
        "Western Gold",
        "Genre",
        || Grade {
            temperature: 0.35,
            saturation: 0.05,
            contrast: 0.22,
            fade: 0.08,
            shadow_tint: [0.45, 0.22, 0.05, 0.3],
            highlight_tint: [1.0, 0.8, 0.4, 0.35],
            vignette: 0.35,
            ..Default::default()
        },
        Sample::Sunset,
    ),
    filter(
        "filter.genre_war_drab",
        "War Drab",
        "Genre",
        || Grade {
            saturation: -0.45,
            contrast: 0.3,
            temperature: 0.05,
            tint: 0.1,
            shadow_tint: [0.2, 0.25, 0.15, 0.3],
            highlight_tint: [0.85, 0.8, 0.6, 0.2],
            grain: 0.35,
            vignette: 0.4,
            ..Default::default()
        },
        Sample::Meadow,
    ),
    filter(
        "filter.genre_horror_green",
        "Horror Green",
        "Genre",
        || Grade {
            saturation: -0.3,
            contrast: 0.3,
            tint: 0.35,
            exposure: -0.15,
            shadow_tint: [0.0, 0.25, 0.15, 0.4],
            highlight_tint: [0.6, 0.9, 0.7, 0.2],
            vignette: 0.5,
            ..Default::default()
        },
        Sample::Night,
    ),
    filter(
        "filter.genre_romance_glow",
        "Romance Glow",
        "Genre",
        || Grade {
            exposure: 0.12,
            contrast: -0.1,
            saturation: 0.1,
            temperature: 0.12,
            fade: 0.12,
            highlight_tint: [1.0, 0.75, 0.7, 0.3],
            shadow_tint: [0.5, 0.25, 0.35, 0.2],
            vignette: 0.15,
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.genre_drama_teal",
        "Drama Teal",
        "Genre",
        || Grade {
            contrast: 0.3,
            saturation: -0.05,
            temperature: -0.12,
            shadow_tint: [0.0, 0.4, 0.45, 0.35],
            highlight_tint: [1.0, 0.85, 0.7, 0.2],
            blacks: -0.1,
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.genre_action_steel",
        "Action Steel",
        "Genre",
        || Grade {
            contrast: 0.45,
            saturation: -0.25,
            temperature: -0.18,
            whites: 0.15,
            blacks: -0.2,
            shadow_tint: [0.1, 0.2, 0.35, 0.3],
            grain: 0.15,
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.genre_mystery_fog",
        "Mystery Fog",
        "Genre",
        || Grade {
            exposure: -0.05,
            contrast: -0.15,
            saturation: -0.35,
            fade: 0.2,
            temperature: -0.1,
            highlight_tint: [0.8, 0.88, 0.95, 0.25],
            vignette: 0.3,
            ..Default::default()
        },
        Sample::Wildlife,
    ),
    filter(
        "filter.genre_fantasy_bloom",
        "Fantasy Bloom",
        "Genre",
        || Grade {
            exposure: 0.1,
            saturation: 0.25,
            vibrance: 0.3,
            contrast: -0.05,
            fade: 0.1,
            highlight_tint: [1.0, 0.85, 1.0, 0.25],
            shadow_tint: [0.35, 0.2, 0.6, 0.25],
            ..Default::default()
        },
        Sample::Meadow,
    ),
    filter(
        "filter.genre_heist_night",
        "Heist Night",
        "Genre",
        || Grade {
            exposure: -0.25,
            contrast: 0.35,
            saturation: 0.1,
            temperature: -0.2,
            shadow_tint: [0.05, 0.1, 0.4, 0.4],
            highlight_tint: [1.0, 0.7, 0.3, 0.25],
            vignette: 0.4,
            ..Default::default()
        },
        Sample::Night,
    ),
    filter(
        "filter.genre_epic_dusk",
        "Epic Dusk",
        "Genre",
        || Grade {
            contrast: 0.3,
            saturation: 0.2,
            temperature: 0.12,
            tint: -0.1,
            shadow_tint: [0.2, 0.1, 0.45, 0.4],
            highlight_tint: [1.0, 0.55, 0.25, 0.4],
            vignette: 0.3,
            ..Default::default()
        },
        Sample::Sunset,
    ),
    filter(
        "filter.matte_01",
        "Soft Matte 1",
        "Soft Matte",
        || Grade {
            fade: 0.082,
            saturation: -0.065,
            temperature: -0.02,
            contrast: -0.058,
            blacks: 0.02,
            highlight_tint: [1.0, 0.93, 0.85, 0.12],
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.matte_02",
        "Soft Matte 2",
        "Soft Matte",
        || Grade {
            fade: 0.114,
            saturation: -0.1,
            temperature: 0.07,
            contrast: -0.076,
            blacks: 0.04,
            highlight_tint: [1.0, 0.93, 0.85, 0.14],
            ..Default::default()
        },
        Sample::Pet,
    ),
    filter(
        "filter.matte_03",
        "Soft Matte 3",
        "Soft Matte",
        || Grade {
            fade: 0.146,
            saturation: -0.135,
            temperature: -0.02,
            contrast: -0.094,
            blacks: 0.06,
            highlight_tint: [1.0, 0.93, 0.85, 0.16],
            ..Default::default()
        },
        Sample::Nap,
    ),
    filter(
        "filter.matte_04",
        "Soft Matte 4",
        "Soft Matte",
        || Grade {
            fade: 0.178,
            saturation: -0.17,
            temperature: 0.07,
            contrast: -0.112,
            blacks: 0.08,
            highlight_tint: [1.0, 0.93, 0.85, 0.18],
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.matte_05",
        "Soft Matte 5",
        "Soft Matte",
        || Grade {
            fade: 0.21,
            saturation: -0.205,
            temperature: -0.02,
            contrast: -0.13,
            blacks: 0.1,
            highlight_tint: [1.0, 0.93, 0.85, 0.2],
            ..Default::default()
        },
        Sample::Meadow,
    ),
    filter(
        "filter.matte_06",
        "Soft Matte 6",
        "Soft Matte",
        || Grade {
            fade: 0.242,
            saturation: -0.24,
            temperature: 0.07,
            contrast: -0.148,
            blacks: 0.12,
            highlight_tint: [1.0, 0.93, 0.85, 0.22],
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.matte_07",
        "Soft Matte 7",
        "Soft Matte",
        || Grade {
            fade: 0.274,
            saturation: -0.275,
            temperature: -0.02,
            contrast: -0.166,
            blacks: 0.14,
            highlight_tint: [1.0, 0.93, 0.85, 0.24],
            ..Default::default()
        },
        Sample::Pet,
    ),
    filter(
        "filter.matte_08",
        "Soft Matte 8",
        "Soft Matte",
        || Grade {
            fade: 0.306,
            saturation: -0.31,
            temperature: 0.07,
            contrast: -0.184,
            blacks: 0.16,
            highlight_tint: [1.0, 0.93, 0.85, 0.26],
            ..Default::default()
        },
        Sample::Nap,
    ),
    filter(
        "filter.skin_clear",
        "Clear Skin",
        "Skin Tones",
        || Grade {
            exposure: 0.08,
            contrast: -0.05,
            saturation: -0.08,
            highlights: -0.15,
            shadows: 0.12,
            temperature: 0.03,
            vibrance: 0.1,
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.skin_honey_glow",
        "Honey Glow",
        "Skin Tones",
        || Grade {
            exposure: 0.1,
            temperature: 0.14,
            saturation: 0.05,
            fade: 0.06,
            highlight_tint: [1.0, 0.82, 0.55, 0.25],
            contrast: -0.06,
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.skin_rose_cheek",
        "Rose Cheek",
        "Skin Tones",
        || Grade {
            exposure: 0.06,
            tint: 0.12,
            temperature: 0.05,
            saturation: 0.08,
            highlight_tint: [1.0, 0.8, 0.8, 0.2],
            shadow_tint: [0.55, 0.25, 0.3, 0.15],
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.skin_studio_beauty",
        "Studio Beauty",
        "Skin Tones",
        || Grade {
            contrast: 0.1,
            exposure: 0.05,
            highlights: -0.2,
            whites: 0.1,
            saturation: -0.04,
            vibrance: 0.12,
            shadows: 0.1,
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.skin_sun_kissed",
        "Sun-Kissed",
        "Skin Tones",
        || Grade {
            temperature: 0.22,
            saturation: 0.12,
            contrast: 0.08,
            highlight_tint: [1.0, 0.7, 0.4, 0.25],
            shadows: 0.08,
            ..Default::default()
        },
        Sample::Pet,
    ),
    filter(
        "filter.skin_cool_porcelain",
        "Cool Porcelain",
        "Skin Tones",
        || Grade {
            exposure: 0.12,
            temperature: -0.08,
            saturation: -0.15,
            fade: 0.06,
            highlight_tint: [0.92, 0.95, 1.0, 0.2],
            contrast: -0.08,
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.skin_warm_bronze",
        "Warm Bronze",
        "Skin Tones",
        || Grade {
            exposure: -0.04,
            temperature: 0.18,
            contrast: 0.15,
            saturation: 0.1,
            shadow_tint: [0.4, 0.2, 0.05, 0.25],
            highlight_tint: [1.0, 0.75, 0.4, 0.2],
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.skin_soft_focus",
        "Soft Focus Look",
        "Skin Tones",
        || Grade {
            contrast: -0.2,
            highlights: -0.25,
            fade: 0.1,
            exposure: 0.08,
            saturation: -0.05,
            vignette: 0.12,
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.stock_100_warm",
        "Stock 100 Warm",
        "Film Stocks",
        || Grade {
            temperature: 0.15,
            contrast: 0.12,
            saturation: 0.1,
            fade: 0.05,
            grain: 0.2,
            highlight_tint: [1.0, 0.85, 0.6, 0.15],
            ..Default::default()
        },
        Sample::Sunset,
    ),
    filter(
        "filter.stock_200_neutral",
        "Stock 200 Neutral",
        "Film Stocks",
        || Grade {
            contrast: 0.1,
            saturation: 0.02,
            fade: 0.04,
            grain: 0.25,
            vignette: 0.15,
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.stock_400_gritty",
        "Stock 400 Gritty",
        "Film Stocks",
        || Grade {
            contrast: 0.2,
            saturation: -0.1,
            grain: 0.6,
            fade: 0.06,
            shadow_tint: [0.1, 0.18, 0.2, 0.2],
            ..Default::default()
        },
        Sample::Meadow,
    ),
    filter(
        "filter.stock_slide_vivid",
        "Slide Vivid",
        "Film Stocks",
        || Grade {
            contrast: 0.3,
            saturation: 0.35,
            vibrance: 0.2,
            blacks: -0.1,
            vignette: 0.2,
            grain: 0.1,
            ..Default::default()
        },
        Sample::Sunset,
    ),
    filter(
        "filter.stock_negative_soft",
        "Negative Soft",
        "Film Stocks",
        || Grade {
            contrast: -0.12,
            saturation: -0.08,
            fade: 0.14,
            temperature: 0.05,
            grain: 0.3,
            shadows: 0.1,
            ..Default::default()
        },
        Sample::Pet,
    ),
    filter(
        "filter.stock_cross_cyan",
        "Cross Cyan",
        "Film Stocks",
        || Grade {
            contrast: 0.25,
            saturation: 0.2,
            tint: -0.15,
            shadow_tint: [0.0, 0.45, 0.55, 0.35],
            highlight_tint: [1.0, 0.95, 0.6, 0.3],
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.stock_bleach_light",
        "Bleach Light",
        "Film Stocks",
        || Grade {
            contrast: 0.28,
            saturation: -0.4,
            whites: 0.12,
            blacks: -0.12,
            grain: 0.3,
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.stock_instant_faded",
        "Instant Faded",
        "Film Stocks",
        || Grade {
            fade: 0.3,
            contrast: -0.1,
            saturation: -0.15,
            temperature: 0.1,
            tint: 0.1,
            vignette: 0.35,
            highlight_tint: [1.0, 0.9, 0.75, 0.25],
            ..Default::default()
        },
        Sample::Pet,
    ),
    filter(
        "filter.stock_cine35_teal",
        "Cine 35 Teal",
        "Film Stocks",
        || Grade {
            contrast: 0.2,
            temperature: -0.06,
            grain: 0.3,
            shadow_tint: [0.0, 0.35, 0.4, 0.3],
            highlight_tint: [1.0, 0.82, 0.6, 0.22],
            vignette: 0.2,
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.stock_cine16_gritty",
        "Cine 16 Gritty",
        "Film Stocks",
        || Grade {
            contrast: 0.22,
            saturation: -0.2,
            grain: 0.65,
            fade: 0.1,
            vignette: 0.4,
            temperature: 0.06,
            ..Default::default()
        },
        Sample::Wildlife,
    ),
    filter(
        "filter.stock_tungsten_warm",
        "Tungsten Warm",
        "Film Stocks",
        || Grade {
            temperature: 0.28,
            tint: 0.05,
            contrast: 0.15,
            exposure: -0.08,
            grain: 0.3,
            shadow_tint: [0.35, 0.15, 0.0, 0.3],
            ..Default::default()
        },
        Sample::Nap,
    ),
    filter(
        "filter.stock_daylight_classic",
        "Daylight Classic",
        "Film Stocks",
        || Grade {
            contrast: 0.12,
            saturation: 0.12,
            temperature: 0.02,
            grain: 0.2,
            fade: 0.03,
            vignette: 0.18,
            ..Default::default()
        },
        Sample::Meadow,
    ),
    filter(
        "filter.city_cyber_pink",
        "Cyber Pink",
        "City Night",
        || Grade {
            contrast: 0.3,
            saturation: 0.35,
            temperature: -0.2,
            tint: 0.3,
            shadow_tint: [0.4, 0.1, 0.7, 0.4],
            highlight_tint: [1.0, 0.35, 0.7, 0.3],
            vignette: 0.3,
            brightness: 0.12,
            ..Default::default()
        },
        Sample::Night,
    ),
    filter(
        "filter.city_neon_alley",
        "Neon Alley",
        "City Night",
        || Grade {
            contrast: 0.35,
            saturation: 0.3,
            temperature: -0.25,
            tint: -0.2,
            shadow_tint: [0.05, 0.2, 0.6, 0.4],
            highlight_tint: [0.2, 1.0, 0.9, 0.3],
            brightness: 0.12,
            ..Default::default()
        },
        Sample::Night,
    ),
    filter(
        "filter.city_blue_ice",
        "Blue Ice City",
        "City Night",
        || Grade {
            exposure: -0.1,
            contrast: 0.25,
            temperature: -0.4,
            saturation: 0.1,
            shadow_tint: [0.0, 0.2, 0.5, 0.45],
            highlight_tint: [0.7, 0.9, 1.0, 0.3],
            brightness: 0.12,
            ..Default::default()
        },
        Sample::Night,
    ),
    filter(
        "filter.city_lilac_dream",
        "Lilac Dream",
        "City Night",
        || Grade {
            exposure: -0.05,
            contrast: 0.15,
            temperature: -0.1,
            tint: 0.25,
            saturation: 0.15,
            shadow_tint: [0.35, 0.15, 0.55, 0.35],
            highlight_tint: [1.0, 0.7, 0.95, 0.25],
            brightness: 0.12,
            ..Default::default()
        },
        Sample::Night,
    ),
    filter(
        "filter.city_midnight_pulse",
        "Midnight Pulse",
        "City Night",
        || Grade {
            exposure: -0.2,
            contrast: 0.4,
            saturation: 0.2,
            temperature: -0.2,
            shadow_tint: [0.05, 0.05, 0.35, 0.5],
            highlight_tint: [1.0, 0.5, 0.35, 0.3],
            vignette: 0.4,
            brightness: 0.12,
            ..Default::default()
        },
        Sample::Night,
    ),
    filter(
        "filter.city_gold_tower",
        "Gold Tower",
        "City Night",
        || Grade {
            exposure: -0.12,
            contrast: 0.3,
            temperature: 0.25,
            saturation: 0.15,
            shadow_tint: [0.15, 0.1, 0.3, 0.35],
            highlight_tint: [1.0, 0.75, 0.3, 0.4],
            ..Default::default()
        },
        Sample::Sunset,
    ),
    filter(
        "filter.city_rain_neon",
        "Rain Neon",
        "City Night",
        || Grade {
            exposure: -0.15,
            contrast: 0.25,
            saturation: 0.2,
            tint: 0.1,
            temperature: -0.15,
            shadow_tint: [0.1, 0.1, 0.4, 0.4],
            highlight_tint: [1.0, 0.4, 0.6, 0.25],
            fade: 0.08,
            brightness: 0.12,
            ..Default::default()
        },
        Sample::Night,
    ),
    filter(
        "filter.city_sunset_skyline",
        "Sunset Skyline",
        "City Night",
        || Grade {
            contrast: 0.28,
            saturation: 0.25,
            temperature: 0.1,
            tint: 0.1,
            shadow_tint: [0.3, 0.1, 0.5, 0.4],
            highlight_tint: [1.0, 0.55, 0.3, 0.4],
            ..Default::default()
        },
        Sample::Sunset,
    ),
    filter(
        "filter.city_station_dusk",
        "Station Dusk",
        "City Night",
        || Grade {
            exposure: -0.15,
            contrast: 0.22,
            temperature: 0.05,
            tint: 0.15,
            shadow_tint: [0.2, 0.1, 0.4, 0.35],
            highlight_tint: [1.0, 0.6, 0.4, 0.3],
            fade: 0.06,
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.city_aurora_blue",
        "Aurora Blue",
        "City Night",
        || Grade {
            exposure: -0.1,
            saturation: 0.3,
            temperature: -0.3,
            tint: -0.3,
            contrast: 0.18,
            shadow_tint: [0.0, 0.35, 0.35, 0.4],
            highlight_tint: [0.5, 1.0, 0.8, 0.25],
            brightness: 0.12,
            ..Default::default()
        },
        Sample::Night,
    ),
    filter(
        "filter.map_violet_mirror",
        "Violet Mirror",
        "Colour Map",
        || gradient_map([0.08, 0.02, 0.2], [0.55, 0.2, 0.75], [1.0, 0.85, 0.97]),
        Sample::Portrait,
    ),
    filter(
        "filter.map_ember",
        "Ember Map",
        "Colour Map",
        || gradient_map([0.1, 0.0, 0.0], [0.8, 0.15, 0.05], [1.0, 0.88, 0.45]),
        Sample::Landscape,
    ),
    filter(
        "filter.map_ocean",
        "Ocean Map",
        "Colour Map",
        || gradient_map([0.0, 0.04, 0.18], [0.05, 0.45, 0.65], [0.8, 1.0, 0.95]),
        Sample::Landscape,
    ),
    filter(
        "filter.map_toxic",
        "Toxic Map",
        "Colour Map",
        || gradient_map([0.02, 0.08, 0.0], [0.35, 0.75, 0.05], [0.95, 1.0, 0.45]),
        Sample::Portrait,
    ),
    filter(
        "filter.map_rose",
        "Rose Map",
        "Colour Map",
        || gradient_map([0.15, 0.0, 0.1], [0.85, 0.25, 0.45], [1.0, 0.92, 0.92]),
        Sample::Portrait,
    ),
    filter(
        "filter.map_gold",
        "Gold Map",
        "Colour Map",
        || gradient_map([0.08, 0.04, 0.0], [0.65, 0.42, 0.05], [1.0, 0.93, 0.6]),
        Sample::Portrait,
    ),
    filter(
        "filter.map_ice",
        "Ice Map",
        "Colour Map",
        || gradient_map([0.0, 0.06, 0.2], [0.3, 0.6, 0.9], [0.95, 0.99, 1.0]),
        Sample::Landscape,
    ),
    filter(
        "filter.map_sunset",
        "Sunset Map",
        "Colour Map",
        || gradient_map([0.12, 0.0, 0.25], [0.9, 0.25, 0.35], [1.0, 0.85, 0.3]),
        Sample::Sunset,
    ),
    filter(
        "filter.map_infrared",
        "Infrared",
        "Colour Map",
        || gradient_map([0.05, 0.0, 0.15], [0.85, 0.1, 0.35], [1.0, 0.95, 0.9]),
        Sample::Meadow,
    ),
    filter(
        "filter.map_acid",
        "Acid Map",
        "Colour Map",
        || gradient_map([0.1, 0.0, 0.3], [0.6, 0.1, 0.8], [0.8, 1.0, 0.1]),
        Sample::Portrait,
    ),
    filter(
        "filter.map_mint",
        "Mint Map",
        "Colour Map",
        || gradient_map([0.0, 0.15, 0.15], [0.3, 0.75, 0.65], [0.95, 1.0, 0.93]),
        Sample::Pet,
    ),
    filter(
        "filter.map_blood_orange",
        "Blood Orange",
        "Colour Map",
        || gradient_map([0.12, 0.0, 0.02], [0.85, 0.18, 0.05], [1.0, 0.7, 0.2]),
        Sample::Sunset,
    ),
    filter(
        "filter.map_midnight",
        "Midnight Map",
        "Colour Map",
        || gradient_map([0.0, 0.0, 0.1], [0.2, 0.25, 0.65], [0.75, 0.85, 1.0]),
        Sample::Night,
    ),
    filter(
        "filter.map_candy",
        "Candy Map",
        "Colour Map",
        || gradient_map([0.1, 0.2, 0.65], [0.85, 0.45, 0.85], [1.0, 0.85, 0.75]),
        Sample::Pet,
    ),
    filter(
        "filter.log_01",
        "Flat Recovery 1",
        "Log Recovery",
        || Grade {
            contrast: 0.21,
            saturation: 0.195,
            temperature: -0.105,
            tint: -0.04,
            blacks: -0.07,
            whites: 0.08,
            vibrance: 0.1,
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.log_02",
        "Flat Recovery 2",
        "Log Recovery",
        || Grade {
            contrast: 0.24,
            saturation: 0.23,
            temperature: -0.07,
            tint: 0.04,
            blacks: -0.08,
            whites: 0.08,
            vibrance: 0.1,
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.log_03",
        "Flat Recovery 3",
        "Log Recovery",
        || Grade {
            contrast: 0.27,
            saturation: 0.265,
            temperature: -0.035,
            tint: -0.04,
            blacks: -0.09,
            whites: 0.08,
            vibrance: 0.1,
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.log_04",
        "Flat Recovery 4",
        "Log Recovery",
        || Grade {
            contrast: 0.3,
            saturation: 0.3,
            temperature: 0.0,
            tint: 0.04,
            blacks: -0.1,
            whites: 0.08,
            vibrance: 0.1,
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.log_05",
        "Flat Recovery 5",
        "Log Recovery",
        || Grade {
            contrast: 0.33,
            saturation: 0.335,
            temperature: 0.035,
            tint: -0.04,
            blacks: -0.11,
            whites: 0.08,
            vibrance: 0.1,
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.log_06",
        "Flat Recovery 6",
        "Log Recovery",
        || Grade {
            contrast: 0.36,
            saturation: 0.37,
            temperature: 0.07,
            tint: 0.04,
            blacks: -0.12,
            whites: 0.08,
            vibrance: 0.1,
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.log_07",
        "Flat Recovery 7",
        "Log Recovery",
        || Grade {
            contrast: 0.39,
            saturation: 0.405,
            temperature: 0.105,
            tint: -0.04,
            blacks: -0.13,
            whites: 0.08,
            vibrance: 0.1,
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.log_08",
        "Flat Recovery 8",
        "Log Recovery",
        || Grade {
            contrast: 0.42,
            saturation: 0.44,
            temperature: 0.14,
            tint: 0.04,
            blacks: -0.14,
            whites: 0.08,
            vibrance: 0.1,
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.game_arcade_pop",
        "Arcade Pop",
        "Gaming",
        || Grade {
            contrast: 0.3,
            saturation: 0.5,
            vibrance: 0.3,
            shadow_tint: [0.3, 0.0, 0.6, 0.25],
            highlight_tint: [1.0, 0.9, 0.2, 0.2],
            ..Default::default()
        },
        Sample::Sunset,
    ),
    filter(
        "filter.game_neon_grid",
        "Neon Grid",
        "Gaming",
        || Grade {
            contrast: 0.4,
            saturation: 0.4,
            temperature: -0.2,
            tint: 0.2,
            shadow_tint: [0.0, 0.1, 0.5, 0.4],
            highlight_tint: [1.0, 0.1, 0.8, 0.3],
            brightness: 0.12,
            ..Default::default()
        },
        Sample::Night,
    ),
    filter(
        "filter.game_toxic_green",
        "Toxic Green",
        "Gaming",
        || Grade {
            contrast: 0.35,
            saturation: 0.3,
            tint: -0.4,
            temperature: -0.05,
            shadow_tint: [0.0, 0.25, 0.05, 0.4],
            highlight_tint: [0.7, 1.0, 0.3, 0.3],
            ..Default::default()
        },
        Sample::Meadow,
    ),
    filter(
        "filter.game_plasma_purple",
        "Plasma Purple",
        "Gaming",
        || Grade {
            contrast: 0.3,
            saturation: 0.3,
            tint: 0.35,
            temperature: -0.25,
            shadow_tint: [0.4, 0.0, 0.7, 0.45],
            highlight_tint: [0.9, 0.5, 1.0, 0.3],
            brightness: 0.12,
            ..Default::default()
        },
        Sample::Night,
    ),
    filter(
        "filter.game_hot_lava",
        "Hot Lava",
        "Gaming",
        || Grade {
            contrast: 0.4,
            saturation: 0.35,
            temperature: 0.35,
            tint: 0.1,
            shadow_tint: [0.45, 0.0, 0.0, 0.4],
            highlight_tint: [1.0, 0.7, 0.1, 0.35],
            ..Default::default()
        },
        Sample::Sunset,
    ),
    filter(
        "filter.game_cold_steel",
        "Cold Steel Game",
        "Gaming",
        || Grade {
            contrast: 0.4,
            saturation: -0.2,
            temperature: -0.3,
            whites: 0.15,
            blacks: -0.2,
            shadow_tint: [0.0, 0.15, 0.35, 0.35],
            ..Default::default()
        },
        Sample::Landscape,
    ),
    filter(
        "filter.game_retro_console",
        "Retro Console",
        "Gaming",
        || Grade {
            contrast: 0.25,
            saturation: 0.35,
            fade: 0.1,
            shadow_tint: [0.3, 0.1, 0.5, 0.25],
            highlight_tint: [0.5, 1.0, 0.8, 0.2],
            ..Default::default()
        },
        Sample::Pet,
    ),
    filter(
        "filter.game_cyber_visor",
        "Cyber Visor",
        "Gaming",
        || Grade {
            contrast: 0.3,
            saturation: 0.2,
            temperature: -0.35,
            tint: -0.15,
            shadow_tint: [0.0, 0.35, 0.5, 0.45],
            highlight_tint: [0.4, 1.0, 1.0, 0.3],
            vignette: 0.45,
            ..Default::default()
        },
        Sample::Portrait,
    ),
    filter(
        "filter.biz_corporate_clean",
        "Corporate Clean",
        "Business",
        || Grade {
            contrast: 0.1,
            saturation: -0.08,
            exposure: 0.05,
            temperature: -0.03,
            whites: 0.08,
            vibrance: 0.08,
            ..Default::default()
        },
        Sample::City,
    ),
    filter(
        "filter.biz_boardroom",
        "Boardroom",
        "Business",
        || Grade {
            contrast: 0.15,
            saturation: -0.15,
            temperature: -0.05,
            shadows: 0.08,
            highlights: -0.1,
            vignette: 0.12,
            ..Default::default()
        },
        Sample::City,
    ),
    filter(
        "filter.biz_product_white",
        "Product White",
        "Business",
        || Grade {
            exposure: 0.2,
            whites: 0.2,
            contrast: 0.05,
            saturation: -0.05,
            shadows: 0.15,
            temperature: -0.02,
            ..Default::default()
        },
        Sample::City,
    ),
    filter(
        "filter.biz_warm_office",
        "Warm Office",
        "Business",
        || Grade {
            temperature: 0.1,
            exposure: 0.05,
            contrast: 0.08,
            saturation: -0.05,
            highlight_tint: [1.0, 0.92, 0.8, 0.15],
            ..Default::default()
        },
        Sample::City,
    ),
    filter(
        "filter.biz_cool_tech",
        "Cool Tech Office",
        "Business",
        || Grade {
            temperature: -0.15,
            contrast: 0.15,
            saturation: -0.1,
            tint: -0.05,
            shadow_tint: [0.0, 0.15, 0.3, 0.2],
            whites: 0.1,
            ..Default::default()
        },
        Sample::City,
    ),
    filter(
        "filter.biz_presentation_pop",
        "Presentation Pop",
        "Business",
        || Grade {
            contrast: 0.2,
            saturation: 0.2,
            vibrance: 0.2,
            exposure: 0.05,
            whites: 0.1,
            ..Default::default()
        },
        Sample::City,
    ),
    filter(
        "filter.biz_soft_studio",
        "Soft Studio Light",
        "Business",
        || Grade {
            contrast: -0.05,
            exposure: 0.1,
            highlights: -0.2,
            shadows: 0.2,
            saturation: -0.05,
            fade: 0.04,
            ..Default::default()
        },
        Sample::City,
    ),
    filter(
        "filter.biz_documentary",
        "Documentary Neutral",
        "Business",
        || Grade {
            contrast: 0.12,
            saturation: -0.18,
            fade: 0.05,
            grain: 0.1,
            vignette: 0.15,
            ..Default::default()
        },
        Sample::City,
    ),
    filter(
        "filter.food_fresh_plate",
        "Fresh Plate",
        "Food",
        || Grade {
            exposure: 0.1,
            saturation: 0.2,
            vibrance: 0.25,
            contrast: 0.1,
            temperature: 0.02,
            highlights: -0.1,
            ..Default::default()
        },
        Sample::Food,
    ),
    filter(
        "filter.food_warm_kitchen",
        "Warm Kitchen",
        "Food",
        || Grade {
            temperature: 0.2,
            exposure: 0.05,
            saturation: 0.12,
            contrast: 0.1,
            highlight_tint: [1.0, 0.85, 0.6, 0.2],
            ..Default::default()
        },
        Sample::Food,
    ),
    filter(
        "filter.food_brunch",
        "Brunch Light",
        "Food",
        || Grade {
            exposure: 0.15,
            temperature: 0.08,
            saturation: 0.1,
            fade: 0.05,
            contrast: -0.04,
            highlights: -0.12,
            ..Default::default()
        },
        Sample::Food,
    ),
    filter(
        "filter.food_bakery_gold",
        "Bakery Gold",
        "Food",
        || Grade {
            temperature: 0.3,
            saturation: 0.18,
            contrast: 0.15,
            shadow_tint: [0.35, 0.18, 0.05, 0.25],
            highlight_tint: [1.0, 0.8, 0.4, 0.25],
            ..Default::default()
        },
        Sample::Food,
    ),
    filter(
        "filter.food_street",
        "Street Food Glow",
        "Food",
        || Grade {
            exposure: -0.05,
            contrast: 0.25,
            saturation: 0.3,
            temperature: 0.15,
            shadow_tint: [0.3, 0.1, 0.0, 0.3],
            vignette: 0.3,
            ..Default::default()
        },
        Sample::Food,
    ),
    filter(
        "filter.food_fine_dining",
        "Fine Dining",
        "Food",
        || Grade {
            exposure: -0.1,
            contrast: 0.25,
            saturation: -0.05,
            temperature: 0.1,
            shadow_tint: [0.15, 0.08, 0.05, 0.35],
            highlight_tint: [1.0, 0.85, 0.65, 0.2],
            vignette: 0.35,
            ..Default::default()
        },
        Sample::Food,
    ),
    filter(
        "filter.food_green_market",
        "Green Market",
        "Food",
        || Grade {
            saturation: 0.25,
            vibrance: 0.3,
            temperature: -0.02,
            tint: -0.08,
            contrast: 0.12,
            exposure: 0.05,
            ..Default::default()
        },
        Sample::Food,
    ),
    filter(
        "filter.food_sweet_treat",
        "Sweet Treat",
        "Food",
        || Grade {
            exposure: 0.12,
            saturation: 0.22,
            temperature: 0.05,
            tint: 0.1,
            fade: 0.05,
            highlight_tint: [1.0, 0.82, 0.88, 0.2],
            ..Default::default()
        },
        Sample::Food,
    ),
];

pub fn get(id: &str) -> Option<&'static FilterDef> {
    FILTERS.iter().find(|f| f.id == id)
}

pub fn is_filter(kind: &str) -> bool {
    kind.starts_with("filter.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_are_distinct_and_well_formed() {
        let mut ids = std::collections::HashSet::new();
        for (i, f) in FILTERS.iter().enumerate() {
            assert!(ids.insert(f.id), "duplicate id {}", f.id);
            assert!(is_filter(f.id), "{}", f.id);
            assert!(
                f.id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_'),
                "{}",
                f.id
            );
            let g = (f.grade)();
            assert_eq!(
                g.strength, 1.0,
                "{} must leave strength to the engine",
                f.id
            );
            assert!(!g.is_identity(), "{} changes nothing", f.id);
            for o in &FILTERS[..i] {
                assert!((o.grade)() != g, "{} looks exactly like {}", f.id, o.id);
            }
        }
        for id in ["filter.teal_orange", "filter.noir"] {
            assert!(get(id).is_some(), "{id}");
        }
    }

    #[test]
    fn categories_follow_the_library_order() {
        let order = [
            "Basic",
            "Minimal",
            "Pastel",
            "Creator",
            "Lifestyle",
            "Cinematic",
            "Night",
            "Nature",
            "Travel",
            "Mono",
            "Vintage",
            "Portrait",
            "Warm",
            "Cool",
            "Film",
            "Genre",
            "Soft Matte",
            "Skin Tones",
            "Film Stocks",
            "City Night",
            "Colour Map",
            "Log Recovery",
            "Gaming",
            "Business",
            "Food",
        ];
        assert_eq!(super::super::categories(FILTERS, |f| f.category), order);
    }
}
