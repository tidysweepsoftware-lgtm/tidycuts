//! What each library item is good for, so the library can be filtered by the
//! kind of video being made: business, gaming, travel, reviews, vlogs,
//! education, food, fitness, real estate, music and events.
//!
//! A use is one bit of a `u16`. Items nobody narrowed down (the classic ones
//! and the general-purpose ones) are good for every use. The rules are by id
//! prefix (or category for filters) so a new item is covered by naming it well.

/// The uses, in bit order.
pub const NAMES: [&str; 11] = [
    "Business",
    "Gaming",
    "Travel",
    "Reviews",
    "Vlog",
    "Education",
    "Food",
    "Fitness",
    "Real estate",
    "Music",
    "Events",
];

/// Every use.
pub const ALL: u16 = (1 << NAMES.len()) - 1;

const BIZ: u16 = 1 << 0;
const GAME: u16 = 1 << 1;
const TRAVEL: u16 = 1 << 2;
const REVIEW: u16 = 1 << 3;
const VLOG: u16 = 1 << 4;
const EDU: u16 = 1 << 5;
const FOOD: u16 = 1 << 6;
const FIT: u16 = 1 << 7;
const HOME: u16 = 1 << 8;
const MUSIC: u16 = 1 << 9;
const EVENT: u16 = 1 << 10;

/// The first rule whose prefix starts `id`, else every use.
fn by_prefix(id: &str, rules: &[(&str, u16)]) -> u16 {
    rules
        .iter()
        .find(|(p, _)| id.starts_with(p))
        .map_or(ALL, |(_, m)| *m)
}

/// Whether `mask` includes use number `use_index`.
pub fn has(mask: u16, use_index: usize) -> bool {
    use_index < NAMES.len() && mask & (1 << use_index) != 0
}

/// The names of the uses in `mask`, space separated (for searching).
pub fn words(mask: u16) -> String {
    if mask == ALL {
        return String::new();
    }
    NAMES
        .iter()
        .enumerate()
        .filter(|(i, _)| has(mask, *i))
        .map(|(_, n)| *n)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn title(id: &str) -> u16 {
    by_prefix(
        id,
        &[
            ("endcredits.directed", VLOG | EVENT | MUSIC | BIZ),
            ("endcredits.music", MUSIC | VLOG | EVENT),
            ("endcredits.sport", FIT | GAME | EVENT),
            ("endcredits.game_over", GAME),
            (
                "endcredits.follow",
                VLOG | REVIEW | GAME | FOOD | FIT | MUSIC | TRAVEL,
            ),
            (
                "endcredits.watch_next",
                VLOG | REVIEW | GAME | FOOD | FIT | MUSIC | TRAVEL | EDU,
            ),
            ("endcredits.filmed_in", TRAVEL | VLOG | EVENT | HOME),
            ("endcredits.sponsor", REVIEW | VLOG | BIZ),
            ("endcredits.", ALL),
            (
                "captions.pop_",
                VLOG | GAME | REVIEW | FIT | FOOD | MUSIC | TRAVEL,
            ),
            ("captions.bold_", VLOG | REVIEW | EDU | BIZ | TRAVEL | FOOD),
            ("captions.sport_", FIT | GAME | EVENT),
            ("captions.news_", BIZ | EDU | REVIEW),
            ("captions.mono_", EDU | GAME),
            ("captions.soft_", TRAVEL | VLOG | EVENT | HOME),
            ("card.name_", BIZ | EDU | HOME | EVENT | REVIEW | VLOG),
            ("card.news_", BIZ | EDU | EVENT),
            ("card.location_", TRAVEL | VLOG | HOME | EVENT),
            ("card.date_", EVENT | VLOG | BIZ),
            ("card.listing_", HOME),
            ("card.price_", REVIEW | FOOD | HOME | BIZ),
            ("card.rating", REVIEW | FOOD | TRAVEL),
            ("card.review_", REVIEW),
            ("card.sponsor_", REVIEW | VLOG),
            ("card.step", EDU | FOOD | FIT | VLOG),
            ("card.chapter", EDU | VLOG | BIZ),
            ("card.tip_", EDU | FOOD | FIT | VLOG),
            (
                "card.",
                REVIEW | VLOG | GAME | EDU | FOOD | FIT | MUSIC | TRAVEL,
            ),
            ("clean.glow_", EVENT | HOME | MUSIC | BIZ),
            ("clean.grad_", EVENT | HOME | TRAVEL | FOOD | VLOG | BIZ),
            ("clean.sub_", ALL),
            ("clean.quote_", ALL),
            ("clean.", BIZ | EDU | TRAVEL | VLOG | REVIEW | HOME),
            ("gloss.gold_", EVENT | MUSIC | HOME | BIZ),
            ("gloss.silver_", BIZ | EVENT | MUSIC),
            ("gloss.rose_", EVENT | FOOD | VLOG),
            ("gloss.level_", GAME),
            ("gloss.", BIZ | REVIEW | EVENT | GAME | FOOD),
            ("neon.", MUSIC | GAME | EVENT),
            ("glass.summer", TRAVEL | VLOG | EVENT),
            ("glass.travel", TRAVEL | VLOG),
            ("glass.vlog", VLOG | REVIEW),
            ("glass.food", FOOD),
            ("glass.fit", FIT),
            ("glass.home", HOME | BIZ),
            ("glass.", VLOG | TRAVEL),
            ("script.engraved", HOME | EVENT | BIZ),
            ("script.editorial", BIZ | TRAVEL | VLOG),
            ("script.noir", MUSIC | EVENT),
            ("script.handwritten", VLOG | EDU | TRAVEL),
            ("script.brush", TRAVEL | VLOG | FIT),
            ("script.bon_appetit", FOOD),
            ("script.thanks", ALL),
            ("script.", EVENT | VLOG),
            ("tech.gear_review", GAME | REVIEW),
            ("tech.setup_tour", GAME | REVIEW | VLOG),
            ("tech.terminal", EDU | GAME),
            ("tech.code_lesson", EDU),
            ("tech.", GAME | MUSIC),
            ("sport.", FIT | EVENT),
            ("biz.open_house", HOME),
            ("biz.just_listed", HOME),
            ("biz.sold", HOME | BIZ),
            ("biz.venue", EVENT | HOME | BIZ),
            ("biz.agenda", BIZ | EDU),
            ("biz.speaker", BIZ | EDU | EVENT),
            ("biz.webinar", BIZ | EDU),
            ("biz.", BIZ),
            ("event.", EVENT | MUSIC),
        ],
    )
}

pub fn transition(id: &str) -> u16 {
    by_prefix(
        id,
        &[
            ("mg.", BIZ | EDU | REVIEW | VLOG | HOME | TRAVEL),
            ("brush.", TRAVEL | FOOD | EVENT | VLOG),
            ("paper.", VLOG | EDU | TRAVEL | FOOD),
            ("dust.", EVENT | MUSIC | TRAVEL | FIT),
            ("camera.shutter", TRAVEL | VLOG | REVIEW),
            ("camera.speed", GAME | FIT | MUSIC | VLOG),
            ("camera.freeze", TRAVEL | VLOG | EVENT),
            ("luxury.", HOME | EVENT | BIZ | FOOD),
            ("glass.frost", BIZ | HOME | REVIEW | EDU),
            ("glass.", GAME | MUSIC | EVENT),
            ("cards.", BIZ | EDU | REVIEW | HOME | TRAVEL),
            ("slabs.", BIZ | HOME | EVENT | REVIEW | EDU | TRAVEL),
            ("torus.", BIZ | EVENT | MUSIC | GAME | REVIEW | HOME),
            ("ice.", TRAVEL | GAME | MUSIC | EVENT | FIT),
            ("steps.", BIZ | EDU | REVIEW | HOME | VLOG | GAME),
            ("tiles3d.", BIZ | HOME | EVENT | REVIEW | TRAVEL | MUSIC),
        ],
    )
}

pub fn effect(id: &str) -> u16 {
    by_prefix(
        id,
        &[
            ("shake.", VLOG | GAME | MUSIC | FIT | EVENT),
            ("zoom.", VLOG | GAME | MUSIC | FIT | REVIEW),
            ("flicker.", MUSIC | GAME | EVENT | VLOG),
            ("glass.", EVENT | MUSIC | GAME),
            ("sparkle.", EVENT | MUSIC | HOME | FOOD | TRAVEL),
            ("grunge.", MUSIC | GAME | VLOG | REVIEW),
            ("frame.neon_", MUSIC | GAME | EVENT),
            ("celebration.", EVENT | MUSIC | FIT),
            ("react.", MUSIC | GAME | FIT | VLOG | EVENT),
            ("beat.", MUSIC | FIT | GAME | VLOG | EVENT),
            ("layout.", BIZ | HOME | TRAVEL | REVIEW | EDU),
        ],
    )
}

/// Filters are grouped by category.
pub fn filter(category: &str) -> u16 {
    match category {
        "Skin Tones" => VLOG | REVIEW | FIT | EVENT | BIZ,
        "Film Stocks" => TRAVEL | VLOG | MUSIC | EVENT,
        "City Night" => TRAVEL | MUSIC | EVENT | VLOG,
        "Log Recovery" => TRAVEL | BIZ | EVENT,
        "Gaming" => GAME,
        "Business" => BIZ | HOME | EDU,
        "Food" => FOOD,
        "Soft Matte" => VLOG | EVENT | HOME | TRAVEL | FOOD,
        _ => ALL,
    }
}

pub fn sticker(id: &str, category: &str) -> u16 {
    let by_id = by_prefix(
        id,
        &[
            ("burst.heart", VLOG | EVENT | MUSIC),
            ("burst.", GAME | MUSIC | EVENT | VLOG | FIT | REVIEW),
            ("party.", EVENT | MUSIC | VLOG | FIT | GAME),
            ("fire.orb", GAME | MUSIC | EDU),
            ("fire.lightning", GAME | MUSIC | FIT),
            ("fire.arc", GAME | MUSIC | EDU),
            ("fire.smoke", FOOD | GAME | MUSIC),
            ("fire.", GAME | MUSIC | FIT | FOOD | EVENT),
            ("word.new_", BIZ | REVIEW | FOOD | HOME),
            ("word.sale_", BIZ | REVIEW | FOOD | HOME),
            ("word.hot_", BIZ | REVIEW | FOOD | VLOG),
            ("word.best_", BIZ | REVIEW | FOOD | HOME),
            ("word.top_", BIZ | REVIEW | FOOD | GAME),
            ("word.free_", BIZ | REVIEW | FOOD | HOME | EVENT),
            ("word.live_", VLOG | GAME | MUSIC | EVENT),
            ("word.rec_", VLOG | GAME | MUSIC | EDU),
            ("word.win_", GAME | FIT | EVENT),
            ("word.goal_", GAME | FIT | EVENT),
            ("word.soon_", EVENT | MUSIC | BIZ | GAME),
            ("word.", VLOG | GAME | REVIEW | FIT | FOOD | EVENT),
            ("tag.rating", REVIEW | FOOD | TRAVEL | HOME),
            ("tag.pin_", TRAVEL | HOME | FOOD | VLOG | EVENT),
            ("tag.seal_", BIZ | REVIEW | EDU | HOME),
            ("tag.", BIZ | REVIEW | FOOD | HOME),
            ("hud.equalizer", MUSIC | VLOG),
            ("hud.sound_wave", MUSIC | VLOG | EDU),
            ("hud.loading", BIZ | EDU | GAME),
            ("hud.progress", BIZ | EDU | FIT | GAME),
            ("hud.", GAME | VLOG | MUSIC | BIZ),
        ],
    );
    if by_id != ALL || id.starts_with("callout.") || id.starts_with("mark.") {
        return by_id;
    }
    match category {
        "Nature" => TRAVEL | FOOD | FIT | VLOG,
        _ => ALL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{effects, filters, stickers, titles, transitions};

    fn count(masks: impl Iterator<Item = u16>, use_index: usize) -> usize {
        masks.filter(|m| has(*m, use_index)).count()
    }

    #[test]
    fn every_use_has_plenty_of_everything() {
        for (i, name) in NAMES.iter().enumerate() {
            let t = count(titles::TITLES.iter().map(|t| title(t.id)), i);
            let tr = count(transitions::TRANSITIONS.iter().map(|t| transition(t.id)), i);
            let e = count(effects::EFFECTS.iter().map(|t| effect(t.id)), i);
            let f = count(filters::FILTERS.iter().map(|t| filter(t.category)), i);
            let s = count(
                stickers::STICKERS.iter().map(|t| sticker(t.id, t.category)),
                i,
            );
            assert!(t >= 25, "{name}: {t} titles");
            assert!(tr >= 60, "{name}: {tr} transitions");
            assert!(e >= 60, "{name}: {e} effects");
            assert!(f >= 100, "{name}: {f} filters");
            assert!(s >= 70, "{name}: {s} stickers");
        }
    }

    #[test]
    fn narrow_items_are_narrow_and_classics_are_open() {
        assert_eq!(title("basic.title"), ALL);
        assert_eq!(title("biz.sold") & GAME, 0);
        assert_ne!(title("tech.game_over") & GAME, 0);
        assert_eq!(words(ALL), "");
        assert!(words(FOOD | FIT).contains("Fitness"));
        assert!(!has(ALL, 99));
    }

    #[test]
    fn the_new_work_is_tagged_not_left_open() {
        // Most of what was built for the new library names a use.
        let open = titles::TITLES
            .iter()
            .filter(|t| {
                t.id.split('.').next().is_some_and(|p| {
                    [
                        "card", "clean", "gloss", "neon", "glass", "script", "tech", "sport",
                        "biz", "event",
                    ]
                    .contains(&p)
                })
            })
            .filter(|t| title(t.id) == ALL)
            .count();
        assert!(open <= 4, "{open} new titles left open");
    }
}
