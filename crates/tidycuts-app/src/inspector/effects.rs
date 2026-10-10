//! Effects tab: the clip's effects and filters in order, each with an
//! on/off switch, reorder and remove buttons and its catalog parameters as
//! keyframable sliders, toggles, choices and colors.

use super::kit::{self, Sec};
use super::{Kind, Panel};
use crate::widgets;
use egui::Ui;
use egui_phosphor::regular as ph;
use tidycuts_core::{Animatable, Clip, Effect, Time};
use tidycuts_render::catalog::{categories, effects, filters, ColorDef, ParamDef, ParamUi};

/// What the catalog says about an effect kind.
struct Def {
    name: &'static str,
    category: &'static str,
    filter: bool,
    params: &'static [ParamDef],
    colors: &'static [ColorDef],
}

fn lookup(kind: &str) -> Option<Def> {
    if let Some(f) = filters::get(kind) {
        return Some(Def {
            name: f.name,
            category: f.category,
            filter: true,
            params: std::slice::from_ref(&filters::STRENGTH),
            colors: &[],
        });
    }
    effects::get(kind).map(|e| Def {
        name: e.name,
        category: e.category,
        filter: false,
        params: e.params,
        colors: e.colors,
    })
}

pub fn show(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    add_menus(ui, c);
    ui.add_space(8.0);
    if c.effects.is_empty() {
        kit::empty_state(
            ui,
            ph::SPARKLE,
            "No effects yet",
            if p.kind == Kind::Adjustment {
                "Add an effect or filter; it applies to every track below this layer."
            } else {
                "Add one with the buttons above, or drag one from the Effects or \
                 Filters library onto the clip."
            },
        );
        return;
    }
    let local = p.local;
    let n = c.effects.len();
    let mut remove = None;
    let mut swap = None;
    for (i, e) in c.effects.iter_mut().enumerate() {
        let def = lookup(&e.kind);
        let title = def
            .as_ref()
            .map(|d| d.name.to_string())
            .unwrap_or_else(|| format!("Unknown ({})", e.kind));
        let mut on = e.enabled;
        let out = Sec::new(("effect", e.id), &title)
            .switch(&mut on)
            .reset(!e.params.is_empty())
            .button(ph::TRASH, "Remove", true)
            .button(ph::ARROW_DOWN, "Move down (applied later)", i + 1 < n)
            .button(ph::ARROW_UP, "Move up (applied earlier)", i > 0)
            .show(ui, |ui| match &def {
                Some(d) => {
                    kit::hint(
                        ui,
                        &format!(
                            "{} · {}",
                            if d.filter { "Filter" } else { "Effect" },
                            d.category
                        ),
                    );
                    if d.params.is_empty() && d.colors.is_empty() {
                        kit::hint(ui, "This effect has no settings.");
                    }
                    params(ui, e, d, local);
                }
                None => kit::hint(
                    ui,
                    "This effect isn't in this version's catalog, so it's skipped.",
                ),
            });
        if out.switched {
            e.enabled = on;
        }
        if out.reset {
            e.params.clear();
        }
        match out.button {
            Some(0) => remove = Some(i),
            Some(1) => swap = Some((i, i + 1)),
            Some(2) => swap = Some((i - 1, i)),
            _ => {}
        }
    }
    if let Some(i) = remove {
        c.effects.remove(i);
    }
    if let Some((a, b)) = swap {
        if b < c.effects.len() {
            c.effects.swap(a, b);
        }
    }
    ui.add_space(6.0);
    kit::hint(
        ui,
        "Effects apply from top to bottom. Switch one off to compare without losing \
         its settings.",
    );
}

/// "Add Effect" and "Add Filter" menus, by category.
fn add_menus(ui: &mut Ui, c: &mut Clip) {
    ui.horizontal(|ui| {
        ui.menu_button(format!("{}  Add Effect", ph::PLUS), |ui| {
            for cat in categories(effects::EFFECTS, |e| e.category) {
                ui.menu_button(cat, |ui| {
                    for e in effects::EFFECTS.iter().filter(|e| e.category == cat) {
                        if ui.button(e.name).clicked() {
                            c.effects.push(Effect::new(e.id));
                            ui.close();
                        }
                    }
                });
            }
        });
        ui.menu_button(format!("{}  Add Filter", ph::PLUS), |ui| {
            for cat in categories(filters::FILTERS, |f| f.category) {
                ui.menu_button(cat, |ui| {
                    for f in filters::FILTERS.iter().filter(|f| f.category == cat) {
                        if ui.button(f.name).clicked() {
                            c.effects.push(Effect::new(f.id));
                            ui.close();
                        }
                    }
                });
            }
        });
    });
}

/// The current value of a parameter (its default when unset).
fn get(e: &Effect, key: &str, default: f64) -> Animatable {
    e.params
        .get(key)
        .cloned()
        .unwrap_or_else(|| Animatable::constant(default))
}

/// Stores a parameter, dropping it when it's back to a constant default.
fn put(e: &mut Effect, key: &str, a: Animatable, default: f64) {
    if !a.is_animated() && (a.value - default).abs() < 1e-9 {
        e.params.remove(key);
    } else {
        e.params.insert(key.to_string(), a);
    }
}

fn params(ui: &mut Ui, e: &mut Effect, d: &Def, local: Time) {
    for p in d.params {
        let default = p.default as f64;
        let (min, max) = (p.min as f64, p.max as f64);
        let span = (max - min).max(1e-9);
        let mut a = get(e, p.key, default);
        let changed = match p.ui {
            ParamUi::Percent => kit::anim_row_mapped(
                ui,
                p.label,
                &mut a,
                local,
                |v| (v - min) / span * 100.0,
                |v| min + v / 100.0 * span,
                0.0..=100.0,
                0,
                "%",
                (default - min) / span * 100.0,
            ),
            ParamUi::Value(unit) => kit::anim_row(
                ui,
                p.label,
                &mut a,
                local,
                min..=max,
                kit::decimals_for(span),
                unit,
                default,
            ),
            ParamUi::Toggle => {
                let mut on = a.value_at(local) > 0.5;
                if kit::toggle_row(ui, p.label, &mut on) {
                    a = Animatable::constant(if on { 1.0 } else { 0.0 });
                    true
                } else {
                    false
                }
            }
            ParamUi::Choice(options) => {
                let mut idx = (a.value_at(local).round().max(0.0) as usize).min(options.len() - 1);
                let opts: Vec<(usize, &str)> = options.iter().copied().enumerate().collect();
                if widgets::choice_row(ui, p.label, &mut idx, &opts) {
                    a = Animatable::constant(idx as f64);
                    true
                } else {
                    false
                }
            }
        };
        if changed {
            put(e, p.key, a, default);
        }
    }
    for cd in d.colors {
        let keys = ["_r", "_g", "_b"].map(|s| format!("{}{s}", cd.key));
        let mut col = [0.0f32, 0.0, 0.0, 1.0];
        for (i, k) in keys.iter().enumerate() {
            col[i] = get(e, k, cd.default[i] as f64).value_at(local) as f32;
        }
        if widgets::color_row(ui, cd.label, &mut col) {
            for (i, k) in keys.iter().enumerate() {
                put(
                    e,
                    k,
                    Animatable::constant(col[i] as f64),
                    cd.default[i] as f64,
                );
            }
        }
    }
}
