//! Visualizer tab: the look of an audio visualizer and which sound it shows.

use super::kit::{self, Sec};
use super::Panel;
use crate::widgets;
use egui::Ui;
use tidycuts_core::{Clip, ClipId, Generator, MediaKind, VisualizerSpec, VISUALIZER_STYLES};

pub fn show(p: &mut Panel, ui: &mut Ui, c: &mut Clip) {
    let sources = sound_sources(p);
    let Some(Generator::Visualizer(spec)) = &mut c.generator else {
        return;
    };
    let out = Sec::new("visualizer", "Visualizer")
        .reset(*spec != VisualizerSpec::default())
        .show(ui, |ui| {
            let labels: Vec<&str> = VISUALIZER_STYLES.iter().map(|(_, l)| *l).collect();
            let sel = VISUALIZER_STYLES
                .iter()
                .position(|(id, _)| *id == spec.style);
            if let Some(i) = kit::chips_row(ui, "Style", &labels, sel) {
                spec.style = VISUALIZER_STYLES[i].0.to_string();
            }
            let mut bars = f64::from(spec.bars);
            if kit::slider_row(ui, "Bars", &mut bars, 8.0..=64.0, 0, "", 48.0) {
                spec.bars = bars.round().clamp(8.0, 64.0) as u32;
            }
            kit::slider_row(
                ui,
                "Sensitivity",
                &mut spec.sensitivity,
                10.0..=300.0,
                0,
                "%",
                100.0,
            );
            kit::slider_row(
                ui,
                "Thickness",
                &mut spec.thickness,
                10.0..=100.0,
                0,
                "%",
                65.0,
            );
            kit::toggle_row(ui, "Mirror", &mut spec.mirror);
            widgets::color_row(ui, "Color", &mut spec.color);
            widgets::color_row(ui, "Second color", &mut spec.color2);
            let refs: Vec<(Option<ClipId>, &str)> =
                sources.iter().map(|(id, n)| (*id, n.as_str())).collect();
            widgets::choice_row(ui, "Sound", &mut spec.source, &refs);
            kit::row_hint(
                ui,
                "Shows everything you hear, or the sound of one clip. The size and place \
                 come from the Video tab.",
            );
        });
    if out.reset {
        *spec = VisualizerSpec::default();
    }
}

/// "Everything you hear" and every clip that has sound.
fn sound_sources(p: &Panel) -> Vec<(Option<ClipId>, String)> {
    let project = &p.app.project;
    let mut out = vec![(None, "Everything you hear".to_string())];
    for t in &project.tracks {
        for c in &t.clips {
            let has_sound = c
                .media_id
                .and_then(|m| project.media(m))
                .is_some_and(|m| m.has_audio || m.kind == MediaKind::Audio);
            if has_sound {
                out.push((Some(c.id), crate::timeline::clip_name(p.app, c)));
            }
        }
    }
    out
}

pub fn reset(c: &mut Clip) {
    c.generator = Some(Generator::Visualizer(VisualizerSpec::default()));
}
