//! Which library items are the old designs. The library hides them by
//! default (a switch brings them back) and shows the new ones. Nothing is
//! removed: every id still works in saved projects.
//!
//! Old designs that have no replacement yet (the basic fades, wipes and
//! slides, blur, split screen and subtitles) stay visible.

use super::{effects, filters, transitions};

/// Title categories whose old presets are hidden.
const TITLE_CATEGORIES: [&str; 6] = [
    "Basic",
    "Modern",
    "Lower Thirds",
    "Callouts",
    "Openers",
    "Social",
];

/// Old animated captions are hidden; the new ones start with these.
const NEW_CAPTIONS: [&str; 7] = [
    "captions.pop_",
    "captions.bold_",
    "captions.sport_",
    "captions.news_",
    "captions.box_",
    "captions.soft_",
    "captions.mono_",
];

const TRANSITION_CATEGORIES: [&str; 8] = [
    "3D",
    "Blur",
    "Glitch",
    "Split",
    "Warp",
    "Light Leak",
    "Classic Shapes",
    "Zoom",
];

const EFFECT_CATEGORIES: [&str; 9] = [
    "Trendy",
    "Shake",
    "Zoom",
    "Distortion",
    "Glitch",
    "Retro",
    "Light",
    "Particles",
    "Overlay",
];

/// Sticker categories of the old set (all hidden).
const STICKER_CATEGORIES: [&str; 10] = [
    "Arrows",
    "Shapes",
    "Emoji",
    "Symbols",
    "Social Media",
    "Speech Bubbles",
    "Lines",
    "Badges",
    "Nature",
    "Sparkles",
];

pub fn title(id: &str, category: &str) -> bool {
    if category == "Animated Captions" {
        return !NEW_CAPTIONS.iter().any(|p| id.starts_with(p));
    }
    if category == "End Credits" {
        return !id.starts_with("endcredits.");
    }
    TITLE_CATEGORIES.contains(&category)
}

pub fn transition(id: &str, category: &str) -> bool {
    let first_new = transitions::TRANSITIONS
        .iter()
        .position(|t| t.id == "mg.bars_left")
        .unwrap_or(usize::MAX);
    let at = transitions::TRANSITIONS.iter().position(|t| t.id == id);
    at.is_some_and(|i| i < first_new) && TRANSITION_CATEGORIES.contains(&category)
}

pub fn effect(id: &str, category: &str) -> bool {
    if id == "trendy.shockwave" {
        return false;
    }
    let first_new = effects::EFFECTS
        .iter()
        .position(|e| e.id == "shake.sideways")
        .unwrap_or(usize::MAX);
    let at = effects::EFFECTS.iter().position(|e| e.id == id);
    at.is_some_and(|i| i < first_new) && EFFECT_CATEGORIES.contains(&category)
}

/// Every filter before the first of the new ones.
pub fn filter(id: &str) -> bool {
    let first_new = filters::FILTERS
        .iter()
        .position(|f| f.id == "filter.genre_noir_rain")
        .unwrap_or(usize::MAX);
    filters::FILTERS
        .iter()
        .position(|f| f.id == id)
        .is_some_and(|i| i < first_new)
}

pub fn sticker(category: &str) -> bool {
    STICKER_CATEGORIES.contains(&category)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{categories, stickers, titles};

    #[test]
    fn something_new_is_left_in_every_library() {
        let t = titles::TITLES
            .iter()
            .filter(|t| !title(t.id, t.category))
            .count();
        let tr = transitions::TRANSITIONS
            .iter()
            .filter(|t| !transition(t.id, t.category))
            .count();
        let e = effects::EFFECTS
            .iter()
            .filter(|e| !effect(e.id, e.category))
            .count();
        let f = filters::FILTERS.iter().filter(|f| !filter(f.id)).count();
        let s = stickers::STICKERS
            .iter()
            .filter(|s| !sticker(s.category))
            .count();
        assert!(
            t >= 150 && tr >= 90 && e >= 110 && f >= 96 && s >= 190,
            "{t} {tr} {e} {f} {s}"
        );
    }

    #[test]
    fn the_basics_stay_visible_and_old_looks_go() {
        assert!(!transition("dissolve", "Fade & Dissolve"));
        assert!(!effect("blur.gaussian", "Blur"));
        assert!(title("credits.the_end", "End Credits"));
        assert!(!title("endcredits.the_end", "End Credits"));
        assert!(!title("subtitles.plain", "Subtitles"));
        assert!(title("basic.title", "Basic"));
        assert!(filter("filter.soft_neutral"));
        assert!(!filter("filter.genre_noir_rain"));
        assert!(sticker("Emoji") && !sticker("Countdown"));
        assert!(!effect("trendy.shockwave", "Trendy"));
    }

    #[test]
    fn every_category_keeps_something_or_is_old_only() {
        // The library drops categories that end up empty, so the lists above
        // must name real categories.
        for c in TITLE_CATEGORIES {
            assert!(
                categories(titles::TITLES, |t| t.category).contains(&c),
                "{c}"
            );
        }
        for c in EFFECT_CATEGORIES {
            assert!(
                categories(effects::EFFECTS, |t| t.category).contains(&c),
                "{c}"
            );
        }
        for c in TRANSITION_CATEGORIES {
            assert!(
                categories(transitions::TRANSITIONS, |t| t.category).contains(&c),
                "{c}"
            );
        }
        for c in STICKER_CATEGORIES {
            assert!(
                categories(stickers::STICKERS, |t| t.category).contains(&c),
                "{c}"
            );
        }
    }
}
