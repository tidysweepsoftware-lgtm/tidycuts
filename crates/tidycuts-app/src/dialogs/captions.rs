//! The Auto Captions window: listens to the selected clip and puts what is
//! said on the timeline as subtitle titles.

use super::{field, footer, header, primary, secondary, window};
use crate::app::TidyCutsApp;
use crate::{captions as app_captions, theme, widgets};
use egui::RichText;
use egui_phosphor::regular as ph;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tidycuts_core::ClipId;
use tidycuts_engine::captions::{self, Caption, Layout, Model};

enum Job {
    /// Waiting for the person to press the button.
    Ready,
    /// Listening: how far along, 0..1.
    Working(f64),
    Done(Result<Vec<Caption>, String>),
}

pub struct CaptionsDialog {
    clip: ClipId,
    models: Vec<Model>,
    model: usize,
    styles: Vec<(&'static str, &'static str)>,
    style: usize,
    /// 0 short, 1 medium, 2 long lines.
    lines: usize,
    job: Arc<Mutex<Job>>,
    cancel: Arc<AtomicBool>,
}

impl CaptionsDialog {
    pub fn new(clip: ClipId) -> CaptionsDialog {
        CaptionsDialog {
            clip,
            models: captions::available_models(),
            model: 0,
            styles: app_captions::styles(),
            style: 0,
            lines: 1,
            job: Arc::new(Mutex::new(Job::Ready)),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    fn start(&mut self, app: &TidyCutsApp, ctx: &egui::Context) {
        if self.models.is_empty() {
            return;
        }
        let Some((_, c)) = app.project.clip(self.clip) else {
            return;
        };
        let Some(media) = c.media_id.and_then(|m| app.project.media(m)) else {
            return;
        };
        let path = media.path.clone();
        let stream = c.audio.stream.min(media.audio_streams.saturating_sub(1));
        let range = (c.source_in.as_secs_f64(), c.source_out.as_secs_f64());
        let model = self.models[self.model.min(self.models.len() - 1)];
        let layout = Layout {
            max_chars: [24, 32, 42][self.lines.min(2)],
            ..Layout::default()
        };
        *self.job.lock().unwrap() = Job::Working(0.0);
        self.cancel.store(false, Ordering::Relaxed);
        let (job, cancel, ctx) = (self.job.clone(), self.cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = captions::transcribe(&path, stream, range, model, &cancel, |f| {
                *job.lock().unwrap() = Job::Working(f);
                ctx.request_repaint();
            })
            .map(|words| captions::group_captions(&words, layout));
            *job.lock().unwrap() = Job::Done(result);
            ctx.request_repaint();
        });
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let mut close = false;
        let mut go = false;
        // A finished search is applied to the timeline, then the window closes.
        let finished = {
            let mut j = self.job.lock().unwrap();
            if matches!(*j, Job::Done(_)) {
                match std::mem::replace(&mut *j, Job::Ready) {
                    Job::Done(r) => Some(r),
                    _ => None,
                }
            } else {
                None
            }
        };
        let mut error: Option<String> = None;
        if let Some(result) = finished {
            match result {
                Ok(caps) if caps.is_empty() => {
                    error = Some("No speech was found in this clip.".into());
                }
                Ok(caps) => {
                    let preset = self
                        .styles
                        .get(self.style)
                        .map_or("subtitle.classic", |s| s.0);
                    match app.add_captions(self.clip, &caps, preset) {
                        Some(n) if n > 0 => {
                            let plural = if n == 1 { "" } else { "s" };
                            app.notify(
                                &format!(
                                    "Added {n} caption{plural}. They are normal titles: click one to change its text."
                                ),
                                false,
                            );
                            return false;
                        }
                        _ => {
                            error =
                                Some("The captions could not be placed on the timeline.".into());
                        }
                    }
                }
                Err(e) => error = Some(e),
            }
        }
        let working = match *self.job.lock().unwrap() {
            Job::Working(f) => Some(f),
            _ => None,
        };
        let (_, keys) = window(ctx, "captions", 500.0, |ui| {
            if header(
                ui,
                ph::SUBTITLES,
                "Auto Captions",
                "Turn what is said into subtitles",
            ) {
                close = true;
            }
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(20, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 10.0);
                    if self.models.is_empty() {
                        ui.add(
                            egui::Label::new(
                                RichText::new(
                                    "Captions need the speech files that come with TidyCuts (the \
                                     models and onnxruntime folders next to the program). They \
                                     were not found here.",
                                )
                                .color(theme::WARNING),
                            )
                            .wrap(),
                        );
                        return;
                    }
                    let clip_name = app
                        .project
                        .clip(self.clip)
                        .map_or(String::new(), |(_, c)| clip_label(app, c));
                    field(ui, "Clip", |ui| {
                        ui.label(RichText::new(clip_name).color(theme::TEXT));
                    });
                    if self.models.len() > 1 {
                        field(ui, "Language", |ui| {
                            let labels: Vec<&str> = self
                                .models
                                .iter()
                                .map(|m| match m {
                                    Model::English => "English",
                                    Model::Multilingual => "Other languages",
                                })
                                .collect();
                            ui.add_enabled_ui(working.is_none(), |ui| {
                                widgets::segmented(ui, &mut self.model, &labels);
                            });
                        });
                    } else {
                        field(ui, "Language", |ui| {
                            ui.label(RichText::new(self.models[0].label()).color(theme::TEXT));
                        });
                    }
                    ui.add_enabled_ui(working.is_none(), |ui| {
                        field(ui, "Look", |ui| {
                            let names: Vec<&str> = self.styles.iter().map(|s| s.1).collect();
                            if !names.is_empty() {
                                widgets::segmented(ui, &mut self.style, &names);
                            }
                        });
                        field(ui, "Line length", |ui| {
                            widgets::segmented(ui, &mut self.lines, &["Short", "Medium", "Long"]);
                        });
                    });
                    ui.add(
                        egui::Label::new(
                            RichText::new(
                                "Everything is listened to on your computer. The captions are \
                                 normal title clips on a new track, so you can edit their text, \
                                 look and timing afterwards.",
                            )
                            .size(12.0)
                            .color(theme::TEXT_FAINT),
                        )
                        .wrap(),
                    );
                    if let Some(f) = working {
                        ui.add(
                            egui::ProgressBar::new(f as f32)
                                .text(format!("Listening… {:.0}%", f * 100.0))
                                .animate(true),
                        );
                        ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    }
                    if let Some(e) = &error {
                        ui.add(egui::Label::new(RichText::new(e).color(theme::WARNING)).wrap());
                    }
                });
            footer(
                ui,
                |ui| {
                    if working.is_some() && secondary(ui, "Stop").clicked() {
                        self.cancel.store(true, Ordering::Relaxed);
                    }
                },
                |ui| {
                    if secondary(ui, "Close").clicked() {
                        close = true;
                    }
                    if primary(
                        ui,
                        &format!("{}  Create captions", ph::SUBTITLES),
                        !self.models.is_empty() && working.is_none(),
                    )
                    .clicked()
                    {
                        go = true;
                    }
                },
            );
        });
        if go {
            self.start(app, ctx);
        }
        let done = close || keys.escape;
        if done {
            self.cancel.store(true, Ordering::Relaxed);
        }
        !done
    }
}

fn clip_label(app: &TidyCutsApp, c: &tidycuts_core::Clip) -> String {
    c.media_id
        .and_then(|m| app.project.media(m))
        .map_or("Clip".into(), |m| m.name.clone())
}
