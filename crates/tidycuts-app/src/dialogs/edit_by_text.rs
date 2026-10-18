//! Edit by Text: the words of a clip, as text. Cross out words and the matching
//! pieces of the video are cut out and the gaps closed.

use super::{body, footer, header, primary, scroll_middle, secondary, window};
use crate::app::TidyCutsApp;
use crate::{theme, widgets};
use egui::{Color32, RichText, Sense};
use egui_phosphor::regular as ph;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tidycuts_core::ClipId;
use tidycuts_engine::captions::Word;
use tidycuts_engine::whispercpp;

enum Job {
    Ready,
    Working(f64),
    Done(Result<Vec<Word>, String>),
}

pub struct EditByTextDialog {
    clip: ClipId,
    ready: bool,
    needs_model: bool,
    /// The language, an index into `whispercpp::LANGUAGES`.
    language: usize,
    words: Vec<Word>,
    cut: Vec<bool>,
    /// The word last clicked, so Shift-click can cross out a stretch.
    anchor: Option<usize>,
    error: Option<String>,
    job: Arc<Mutex<Job>>,
    cancel: Arc<AtomicBool>,
}

/// Small sounds people want out ("um", "uh").
fn is_filler(text: &str) -> bool {
    let w: String = text
        .chars()
        .filter(|c| c.is_alphabetic())
        .collect::<String>()
        .to_lowercase();
    matches!(
        w.as_str(),
        "um" | "umm" | "uh" | "uhh" | "uhm" | "erm" | "er" | "ah" | "hmm" | "hm" | "mm" | "mhm"
    )
}

/// The stretches of the file (seconds) to cut for the crossed-out words: each run
/// of crossed-out words is one stretch, with a hair of room either side so a
/// neighbouring word is not clipped.
fn ranges_to_cut(words: &[Word], cut: &[bool], clip: (f64, f64)) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if !cut[i] {
            i += 1;
            continue;
        }
        let mut j = i;
        while j + 1 < words.len() && cut[j + 1] {
            j += 1;
        }
        // Up to the middle of the pause on each side, but never more than a fifth of a second.
        let before = if i > 0 {
            ((words[i].start - words[i - 1].end) / 2.0).clamp(0.0, 0.2)
        } else {
            0.1
        };
        let after = if j + 1 < words.len() {
            ((words[j + 1].start - words[j].end) / 2.0).clamp(0.0, 0.2)
        } else {
            0.1
        };
        let a = (words[i].start - before).max(clip.0);
        let b = (words[j].end + after).min(clip.1);
        if b > a {
            out.push((a, b));
        }
        i = j + 1;
    }
    out
}

impl EditByTextDialog {
    pub fn new(clip: ClipId) -> EditByTextDialog {
        EditByTextDialog {
            clip,
            ready: whispercpp::available(),
            needs_model: whispercpp::needs_model(),
            language: 1,
            words: Vec::new(),
            cut: Vec::new(),
            anchor: None,
            error: None,
            job: Arc::new(Mutex::new(Job::Ready)),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    fn listen(&mut self, app: &TidyCutsApp, ctx: &egui::Context) {
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
        self.error = None;
        *self.job.lock().unwrap() = Job::Working(0.0);
        self.cancel.store(false, Ordering::Relaxed);
        let (job, cancel, ctx) = (self.job.clone(), self.cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let progress = |f: f64| {
                *job.lock().unwrap() = Job::Working(f);
                ctx.request_repaint();
            };
            let words = whispercpp::transcribe(&path, stream, range, language, &cancel, progress);
            *job.lock().unwrap() = Job::Done(words);
            ctx.request_repaint();
        });
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        let mut close = false;
        let mut start = false;
        let mut apply = false;
        let mut mark_fillers = false;
        let mut clear = false;
        // A finished listening fills the text.
        let done = {
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
        match done {
            Some(Ok(words)) if words.is_empty() => {
                self.error = Some("No speech was found in this clip.".into());
            }
            Some(Ok(words)) => {
                self.cut = vec![false; words.len()];
                self.words = words;
                self.anchor = None;
            }
            Some(Err(e)) => self.error = Some(e),
            None => {}
        }
        let working = match *self.job.lock().unwrap() {
            Job::Working(f) => Some(f),
            _ => None,
        };
        let crossed = self.cut.iter().filter(|c| **c).count();
        let (_, keys) = window(ctx, "edit-by-text", 720.0, |ui| {
            close |= header(
                ui,
                ph::TEXT_AA,
                "Edit by Text",
                "Cross out words to cut them from the video",
            );
            scroll_middle(ui, |ui| {
                body(ui, |ui| {
                    if !self.ready {
                        widgets::hint(
                            ui,
                            "The speech program is not installed, so the words cannot be found.",
                        );
                        return;
                    }
                    if self.needs_model && self.words.is_empty() {
                        widgets::hint(
                            ui,
                            "The speech model is needed first. Open Titles > Auto Captions once to download it (574 MB), then come back.",
                        );
                        return;
                    }
                    if self.words.is_empty() {
                        ui.add_enabled_ui(working.is_none(), |ui| {
                            let options: Vec<(usize, &str)> = whispercpp::LANGUAGES
                                .iter()
                                .enumerate()
                                .map(|(i, l)| (i, l.1))
                                .collect();
                            widgets::labeled_row(ui, "Language", 90.0, |ui| {
                                widgets::choice(ui, "ebt-language", &mut self.language, &options);
                            });
                        });
                        ui.add_space(6.0);
                        widgets::hint(
                            ui,
                            "TidyCuts listens to the clip on this computer and writes down every word. Then you can cross words out.",
                        );
                        if let Some(f) = working {
                            ui.add_space(8.0);
                            ui.add(
                                egui::ProgressBar::new(f as f32)
                                    .text(format!("Listening… {:.0}%", f * 100.0)),
                            );
                        }
                    } else {
                        widgets::hint(
                            ui,
                            "Click a word to cross it out (or bring it back). Shift-click crosses out everything between two clicks.",
                        );
                        ui.add_space(6.0);
                        let words = &self.words;
                        let cut = &mut self.cut;
                        let anchor = &mut self.anchor;
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(5.0, 6.0);
                            for (i, w) in words.iter().enumerate() {
                                let mut t = RichText::new(w.text.trim()).size(15.0);
                                if cut[i] {
                                    t = t.strikethrough().color(theme::DANGER);
                                } else {
                                    t = t.color(Color32::WHITE);
                                }
                                let r = ui
                                    .add(egui::Label::new(t).sense(Sense::click()))
                                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                                if r.clicked() {
                                    let shift = ui.input(|i| i.modifiers.shift);
                                    match (*anchor, shift) {
                                        (Some(a), true) => {
                                            let (lo, hi) = (a.min(i), a.max(i));
                                            let on = !cut[i];
                                            cut[lo..=hi].iter_mut().for_each(|c| *c = on);
                                        }
                                        _ => cut[i] = !cut[i],
                                    }
                                    *anchor = Some(i);
                                }
                            }
                        });
                    }
                    if let Some(e) = &self.error {
                        ui.add_space(8.0);
                        ui.label(RichText::new(e).color(theme::DANGER));
                    }
                })
            });
            footer(
                ui,
                |ui| {
                    if !self.words.is_empty() {
                        if secondary(ui, "Mark ums and uhs").clicked() {
                            mark_fillers = true;
                        }
                        if secondary(ui, "Clear marks").clicked() {
                            clear = true;
                        }
                    }
                },
                |ui| {
                    if secondary(ui, "Close").clicked() {
                        close = true;
                    }
                    if self.words.is_empty() {
                        let go = working.is_none() && self.ready && !self.needs_model;
                        if primary(ui, &format!("{}  Listen to the clip", ph::EAR), go).clicked()
                            && go
                        {
                            start = true;
                        }
                    } else {
                        let label = if crossed == 0 {
                            "Cut the crossed-out words".to_string()
                        } else {
                            format!(
                                "Cut {crossed} crossed-out word{}",
                                if crossed == 1 { "" } else { "s" }
                            )
                        };
                        if primary(ui, &format!("{}  {label}", ph::SCISSORS), crossed > 0).clicked()
                            && crossed > 0
                        {
                            apply = true;
                        }
                    }
                },
            );
        });
        if mark_fillers {
            for (c, w) in self.cut.iter_mut().zip(&self.words) {
                if is_filler(&w.text) {
                    *c = true;
                }
            }
        }
        if clear {
            self.cut.iter_mut().for_each(|c| *c = false);
        }
        if start {
            self.listen(app, ctx);
        }
        if apply {
            let clip = app
                .project
                .clip(self.clip)
                .map(|(_, c)| (c.source_in.as_secs_f64(), c.source_out.as_secs_f64()));
            match clip {
                Some(range) => {
                    let ranges = ranges_to_cut(&self.words, &self.cut, range);
                    match app.remove_silences(self.clip, ranges) {
                        Ok(_) => {
                            app.notify(
                                &format!(
                                    "Cut {crossed} word{} from the video. Undo (Ctrl+Z) brings them back.",
                                    if crossed == 1 { "" } else { "s" }
                                ),
                                false,
                            );
                            return false;
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
                None => self.error = Some("The clip is gone.".into()),
            }
        }
        if close || keys.escape {
            self.cancel.store(true, Ordering::Relaxed);
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(t: &str, a: f64, b: f64) -> Word {
        Word {
            text: t.into(),
            start: a,
            end: b,
        }
    }

    #[test]
    fn crossed_out_neighbours_become_one_cut_with_a_little_room() {
        let words = vec![
            word("so", 0.0, 0.3),
            word("um", 0.5, 0.8),
            word("well", 0.85, 1.2),
            word("hello", 2.0, 2.4),
        ];
        let cut = vec![false, true, true, false];
        let r = ranges_to_cut(&words, &cut, (0.0, 3.0));
        assert_eq!(r.len(), 1);
        // From just before "um" to just after "well", not into "so" or "hello".
        assert!(r[0].0 > 0.3 && r[0].0 <= 0.5, "{r:?}");
        assert!(r[0].1 >= 1.2 && r[0].1 < 2.0, "{r:?}");
    }

    #[test]
    fn separate_words_are_separate_cuts_and_stay_inside_the_clip() {
        let words = vec![
            word("a", 0.0, 0.2),
            word("b", 1.0, 1.2),
            word("c", 2.0, 2.2),
        ];
        let r = ranges_to_cut(&words, &[true, false, true], (0.05, 2.25));
        assert_eq!(r.len(), 2);
        assert!(r[0].0 >= 0.05 && r[1].1 <= 2.25, "{r:?}");
    }

    #[test]
    fn ums_and_uhs_are_found() {
        assert!(is_filler("Um,"));
        assert!(is_filler("uh"));
        assert!(!is_filler("umbrella"));
    }
}
