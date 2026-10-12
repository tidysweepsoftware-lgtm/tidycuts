//! Prints how many items each library has: in all, and the new ones the
//! library lists by default (the old designs are behind a switch).
//!
//! `cargo run --release -p tidycuts-render --example library_counts`

use tidycuts_render::catalog::{effects, filters, legacy, stickers, titles, transitions};

fn main() {
    let t = titles::TITLES.len();
    let tn = titles::TITLES
        .iter()
        .filter(|x| !legacy::title(x.id, x.category))
        .count();
    let tr = transitions::TRANSITIONS.len();
    let trn = transitions::TRANSITIONS
        .iter()
        .filter(|x| !legacy::transition(x.id, x.category))
        .count();
    let e = effects::EFFECTS.len();
    let en = effects::EFFECTS
        .iter()
        .filter(|x| !legacy::effect(x.id, x.category))
        .count();
    let f = filters::FILTERS.len();
    let fnn = filters::FILTERS
        .iter()
        .filter(|x| !legacy::filter(x.id))
        .count();
    let s = stickers::STICKERS.len();
    let sn = stickers::STICKERS
        .iter()
        .filter(|x| !legacy::sticker(x.category))
        .count();
    println!("titles      {tn} listed of {t}");
    println!("transitions {trn} listed of {tr}");
    println!("effects     {en} listed of {e}");
    println!("filters     {fnn} listed of {f}");
    println!("stickers    {sn} listed of {s}");
    println!("fonts       {}", tidycuts_render::fonts::FONTS.len());
}
