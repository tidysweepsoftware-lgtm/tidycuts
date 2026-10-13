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
use tidycuts_engine::captions::{self, Caption, Layout};
use tidycuts_engine::whispercpp;

enum Job {
    /// Waiting for the person to press the button.
    Ready,
    /// Listening: how far along, 0..1.
    Working(f64),
    /// Downloading the speech model: how far along, 0..1.
    Downloading(f64),
    Downloaded(Result<(), String>),
    Done(Result<Vec<Caption>, String>),
}

pub struct CaptionsDialog {
    clip: ClipId,
    /// Whether Whisper (whisper.cpp) and its model are installed.
    ready: bool,
    /// The program is here but the speech model has not been downloaded yet.
    needs_model: bool,
    /// The language, as an index into `whispercpp::LANGUAGES`.
    language: usize,
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
            ready: whispercpp::available(),
            needs_model: whispercpp::needs_model(),
            language: 1,
            styles: app_captions::styles(),
            style: 0,
            lines: 1,
            job: Arc::new(Mutex::new(Job::Ready)),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    fn start(&mut self, app: &TidyCutsApp, ctx: &egui::Context) {
        if !self.ready {
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
        let language = whispercpp::LANGUAGES[self.language.min(whispercpp::LANGUAGES.len() - 1)].0;
        let layout = Layout {
            max_chars: [24, 32, 42][self.lines.min(2)],
            ..Layout::default()
        };
        *self.job.lock().unwrap() = Job::Working(0.0);
        self.cancel.store(false, Ordering::Relaxed);
        let (job, cancel, ctx) = (self.job.clone(), self.cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let progress = |f: f64| {
                *job.lock().unwrap() = Job::Working(f);
                ctx.request_repaint();
            };
            let words = whispercpp::transcribe(&path, stream, range, language, &cancel, progress);
            let result = words.map(|words| captions::group_captions(&words, layout));
            *job.lock().unwrap() = Job::Done(result);
            ctx.request_repaint();
        });
    }

    /// Downloads the speech model (once; the person pressed the button).
    fn download(&mut self, ctx: &egui::Context) {
        *self.job.lock().unwrap() = Job::Downloading(0.0);
        self.cancel.store(false, Ordering::Relaxed);
        let (job, cancel, ctx) = (self.job.clone(), self.cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let result = whispercpp::download_model(&cancel, |f| {
                *job.lock().unwrap() = Job::Downloading(f);
                ctx.request_repaint();
            });
            *job.lock().unwrap() = Job::Downloaded(result);
            ctx.request_repaint();
        });
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let mut close = false;
        let mut go = false;
        let mut get_model = false;
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
        let downloaded = {
            let mut j = self.job.lock().unwrap();
            if matches!(*j, Job::Downloaded(_)) {
                match std::mem::replace(&mut *j, Job::Ready) {
                    Job::Downloaded(r) => Some(r),
                    _ => None,
                }
            } else {
                None
            }
        };
        match downloaded {
            Some(Ok(())) => {
                self.ready = whispercpp::available();
                self.needs_model = whispercpp::needs_model();
            }
            Some(Err(e)) => error = Some(e),
            None => {}
        }
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
        let (working, downloading) = match *self.job.lock().unwrap() {
            Job::Working(f) => (Some(f), false),
            Job::Downloading(f) => (Some(f), true),
            _ => (None, false),
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
                    if !self.ready && !self.needs_model {
                        ui.add(
                            egui::Label::new(
                                RichText::new(
                                    "Captions need the speech files that come with TidyCuts (the \
                                     whisper-cpp and models folders next to the program). They \
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
                    field(ui, "Language", |ui| {
                        let options: Vec<(usize, &str)> = whispercpp::LANGUAGES
                            .iter()
                            .enumerate()
                            .map(|(i, l)| (i, l.1))
                            .collect();
                        ui.add_enabled_ui(working.is_none(), |ui| {
                            widgets::choice(ui, "captions-language", &mut self.language, &options);
                        });
                    });
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
                    if self.needs_model && working.is_none() {
                        ui.add(
                            egui::Label::new(
                                RichText::new(
                                    "Auto Captions needs a speech model (574 MB). It is \
                                     downloaded from the internet once, the first time; \
                                     after that everything works without a connection.",
                                )
                                .color(theme::TEXT_DIM),
                            )
                            .wrap(),
                        );
                    }
                    if let Some(f) = working {
                        let what = if downloading {
                            "Downloading"
                        } else {
                            "Listening"
                        };
                        ui.add(
                            egui::ProgressBar::new(f as f32)
                                .text(format!("{what}… {:.0}%", f * 100.0)),
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
                    if self.needs_model {
                        if primary(
                            ui,
                            &format!("{}  Download the speech model", ph::DOWNLOAD_SIMPLE),
                            working.is_none(),
                        )
                        .clicked()
                        {
                            get_model = true;
                        }
                    } else if primary(
                        ui,
                        &format!("{}  Create captions", ph::SUBTITLES),
                        self.ready && working.is_none(),
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
        if get_model {
            self.download(ctx);
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
