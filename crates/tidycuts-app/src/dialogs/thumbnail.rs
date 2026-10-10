//! "Make a thumbnail": pick a picture (a video frame, a photo, or the
//! timeline), type the words, choose how they look, and save a PNG or JPG.
//! The picture is drawn on a worker thread and the preview follows every
//! change a moment after it is made.

use super::{body, footer, header, primary, secondary, window};
use crate::app::TidyCutsApp;
use crate::thumbnail::{self as thumb, Across, Down, Settings, CANVASES, LOOKS};
use crate::{theme, widgets};
use egui::{Color32, CornerRadius, RichText, Sense};
use egui_phosphor::regular as ph;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};
use tidycuts_core::{MediaId, MediaItem, MediaKind, Time};
use tidycuts_engine::snapshot::Picture;

/// Where the picture comes from.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Source {
    /// What the timeline shows at a moment.
    Timeline,
    Media(MediaId),
}

/// What a prepared picture was made from.
type BaseKey = (Source, i64);

struct Done {
    /// The picture the thumbnail is made on, when the job had to make it.
    base: Option<(BaseKey, MediaItem)>,
    picture: Result<Picture, String>,
}

pub struct ThumbnailDialog {
    s: Settings,
    source: Source,
    /// Videos and photos of the library.
    media: Vec<(MediaId, String, MediaKind, f64)>,
    timeline_secs: f64,
    base: Option<(BaseKey, MediaItem)>,
    preview: Option<egui::TextureHandle>,
    /// What the newest job was asked for.
    requested: Option<(Settings, Source)>,
    changed_at: Instant,
    working: bool,
    error: Option<String>,
    tx: Sender<Done>,
    rx: Receiver<Done>,
    saving: Option<Receiver<Result<PathBuf, String>>>,
    /// Temporary pictures made from the timeline.
    scratch: PathBuf,
}

impl ThumbnailDialog {
    pub fn new(app: &TidyCutsApp) -> Self {
        let (tx, rx) = channel();
        let media: Vec<_> = app
            .project
            .media
            .iter()
            .filter(|m| matches!(m.kind, MediaKind::Video | MediaKind::Image))
            .map(|m| (m.id, m.name.clone(), m.kind, m.duration.as_secs_f64()))
            .collect();
        let timeline_secs = app.project.duration().as_secs_f64();
        let source = if timeline_secs > 0.0 {
            Source::Timeline
        } else {
            media
                .first()
                .map_or(Source::Timeline, |m| Source::Media(m.0))
        };
        let scratch = app.prefs.cache_dir().join("thumbnails");
        let _ = std::fs::remove_dir_all(&scratch);
        let _ = std::fs::create_dir_all(&scratch);
        let mut d = ThumbnailDialog {
            s: Settings::default(),
            source,
            media,
            timeline_secs,
            base: None,
            preview: None,
            requested: None,
            changed_at: Instant::now() - Duration::from_secs(1),
            working: false,
            error: None,
            tx,
            rx,
            saving: None,
            scratch,
        };
        d.s.at = d.first_moment(app.playhead().as_secs_f64());
        d
    }

    /// A good starting moment for the current source.
    fn first_moment(&self, playhead: f64) -> f64 {
        match self.source {
            Source::Timeline => playhead.min(self.timeline_secs),
            Source::Media(id) => self.media.iter().find(|m| m.0 == id).map_or(0.0, |m| {
                if m.2 == MediaKind::Video {
                    (m.3 * 0.1).min(2.0)
                } else {
                    0.0
                }
            }),
        }
    }

    /// The length the "moment" slider covers, or none for a photo.
    fn span(&self) -> Option<f64> {
        match self.source {
            Source::Timeline => Some(self.timeline_secs),
            Source::Media(id) => self
                .media
                .iter()
                .find(|m| m.0 == id && m.2 == MediaKind::Video)
                .map(|m| m.3),
        }
    }

    fn key(&self) -> BaseKey {
        let ms = match self.source {
            Source::Timeline => (self.s.at * 1000.0).round() as i64,
            Source::Media(_) => 0,
        };
        (self.source, ms)
    }

    fn source_name(&self) -> String {
        match self.source {
            Source::Timeline => "The timeline".into(),
            Source::Media(id) => self
                .media
                .iter()
                .find(|m| m.0 == id)
                .map_or("Choose…".into(), |m| m.1.clone()),
        }
    }

    /// Takes in what the worker threads have finished.
    fn poll(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        while let Ok(done) = self.rx.try_recv() {
            self.working = false;
            if let Some((key, item)) = done.base {
                self.forget_base_file();
                self.base = Some((key, item));
            }
            match done.picture {
                Ok(pic) => {
                    let img = egui::ColorImage::from_rgba_unmultiplied(
                        [pic.width as usize, pic.height as usize],
                        &pic.rgba,
                    );
                    self.preview = Some(ctx.load_texture(
                        "thumbnail-preview",
                        img,
                        egui::TextureOptions::LINEAR,
                    ));
                    self.error = None;
                }
                Err(e) => {
                    log::warn!("thumbnail preview failed: {e}");
                    self.error = Some(e);
                }
            }
        }
        if let Some(rx) = &self.saving {
            match rx.try_recv() {
                Ok(Ok(path)) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    app.notify(&format!("Saved {name}"), false);
                    return false;
                }
                Ok(Err(e)) => {
                    app.notify(&e, true);
                    self.saving = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(_) => self.saving = None,
            }
        }
        true
    }

    /// Deletes the temporary picture of the timeline, if one is kept.
    fn forget_base_file(&mut self) {
        if let Some(((Source::Timeline, _), item)) = &self.base {
            let _ = std::fs::remove_file(&item.path);
        }
    }

    /// Starts drawing the preview if the settings changed and the last
    /// drawing is done.
    fn refresh(&mut self, app: &TidyCutsApp, ctx: &egui::Context) {
        let wanted = (self.s.clone(), self.source);
        if self.requested.as_ref() == Some(&wanted) {
            return;
        }
        if self.working {
            return;
        }
        let wait = Duration::from_millis(140);
        if self.changed_at.elapsed() < wait {
            ctx.request_repaint_after(wait);
            return;
        }
        let Some(engine) = &app.engine else {
            self.error = Some("Thumbnails need a graphics device.".into());
            self.requested = Some(wanted);
            return;
        };
        // A library picture needs no preparing; the timeline is flattened by the worker.
        let key = self.key();
        let have = self
            .base
            .as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, m)| m.clone());
        let base_for_job = match (have, self.source) {
            (Some(m), _) => Some(m),
            (None, Source::Media(id)) => {
                let item = app.project.media(id).cloned();
                if let Some(item) = &item {
                    self.base = Some((key, item.clone()));
                }
                item
            }
            (None, Source::Timeline) => None,
        };
        let timeline = base_for_job.is_none().then(|| app.project.clone());
        let at = Time::from_secs_f64(self.s.at);
        let width = CANVASES[self.s.canvas.min(CANVASES.len() - 1)]
            .width
            .min(800);
        let (renderer, analysis) = (engine.renderer.clone(), engine.analysis.clone());
        let (tx, scratch, repaint) = (self.tx.clone(), self.scratch.clone(), ctx.clone());
        let settings = self.s.clone();
        self.requested = Some(wanted.clone());
        self.working = true;
        std::thread::Builder::new()
            .name("tidycuts-thumbnail".into())
            .spawn(move || {
                let mut new_base = None;
                let base = match (base_for_job, timeline) {
                    (Some(m), _) => Ok(m),
                    (None, Some(tl)) => {
                        thumb::picture_of_timeline(&tl, at, &scratch, renderer.clone(), &analysis)
                            .inspect(|m| new_base = Some((key, m.clone())))
                    }
                    (None, None) => Err("There is nothing to make a picture from.".to_string()),
                };
                let picture = base.map(|b| {
                    let project = thumb::build(&settings, &b);
                    thumb::render(&project, Some(width), renderer, &analysis)
                });
                let _ = tx.send(Done {
                    base: new_base,
                    picture,
                });
                repaint.request_repaint();
            })
            .expect("spawn thumbnail thread");
    }

    /// Asks where to save, then draws the picture at full size and writes it.
    fn save(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) {
        let Some(engine) = &app.engine else { return };
        let Some((_, base)) = self.base.clone() else {
            return;
        };
        let dir = app.stills_dir();
        let name = thumb::free_name(&dir, "Thumbnail", "png")
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Thumbnail.png".into());
        let _ = std::fs::create_dir_all(&dir);
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save the thumbnail")
            .set_directory(&dir)
            .set_file_name(name)
            .add_filter("PNG picture", &["png"])
            .add_filter("JPG picture", &["jpg", "jpeg"])
            .save_file()
        else {
            return;
        };
        let (renderer, analysis) = (engine.renderer.clone(), engine.analysis.clone());
        let settings = self.s.clone();
        let repaint = ctx.clone();
        let (ready, answer) = channel();
        self.saving = Some(answer);
        std::thread::Builder::new()
            .name("tidycuts-thumbnail-save".into())
            .spawn(move || {
                let project = thumb::build(&settings, &base);
                let pic = thumb::render(&project, None, renderer, &analysis);
                let _ = ready.send(thumb::save(&pic, &path).map(|()| path));
                repaint.request_repaint();
            })
            .expect("spawn thumbnail save thread");
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        if !self.poll(app, ctx) {
            return false;
        }
        self.refresh(app, ctx);
        let before = (self.s.clone(), self.source);
        let nothing = self.media.is_empty() && self.timeline_secs <= 0.0;
        let saving = self.saving.is_some();
        let can_save = !nothing && self.base.is_some() && self.preview.is_some() && !saving;
        let mut go = false;
        let mut cancel = false;
        let (_, keys) = window(ctx, "thumbnail", 960.0, |ui| {
            cancel |= header(
                ui,
                ph::IMAGE_SQUARE,
                "Make a thumbnail",
                "A picture with big, clear words on it, ready to use as a video cover",
            );
            super::scroll_middle(ui, |ui| {
                body(ui, |ui| {
                    if nothing {
                        ui.add_space(30.0);
                        ui.label(
                            RichText::new("Add a video or a picture to the library first.")
                                .size(15.0)
                                .color(theme::TEXT_DIM),
                        );
                        ui.add_space(30.0);
                        return;
                    }
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 20.0;
                        self.preview_area(ui);
                        ui.vertical(|ui| {
                            ui.set_width(318.0);
                            self.controls(ui);
                        });
                    });
                })
            });
            footer(
                ui,
                |ui| {
                    widgets::hint(ui, "Saved as a PNG, or a JPG if the name ends in .jpg");
                },
                |ui| {
                    let label = if saving {
                        format!("{}  Saving…", ph::DOWNLOAD_SIMPLE)
                    } else {
                        format!("{}  Save picture…", ph::DOWNLOAD_SIMPLE)
                    };
                    if primary(ui, &label, can_save).clicked() {
                        go = true;
                    }
                    if secondary(ui, "Close").clicked() {
                        cancel = true;
                    }
                },
            );
        });
        if (self.s.clone(), self.source) != before {
            self.changed_at = Instant::now();
        }
        if go {
            self.save(app, ctx);
        }
        !(cancel || keys.escape)
    }

    fn preview_area(&mut self, ui: &mut egui::Ui) {
        let area = egui::vec2(590.0, 360.0);
        let (rect, _) = ui.allocate_exact_size(area, Sense::hover());
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(8),
            Color32::from_rgb(0x10, 0x12, 0x15),
        );
        if let Some(tex) = &self.preview {
            let size = tex.size_vec2();
            let k = (area.x / size.x).min(area.y / size.y);
            let r = egui::Rect::from_center_size(rect.center(), size * k);
            ui.painter().image(
                tex.id(),
                r,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let note = match (&self.error, self.working, &self.preview) {
            (Some(e), _, _) => Some((e.as_str(), theme::WARNING)),
            (None, true, None) => Some(("Drawing…", theme::TEXT_DIM)),
            _ => None,
        };
        if let Some((text, color)) = note {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(14.0),
                color,
            );
        }
        if self.working && self.preview.is_some() {
            ui.painter().text(
                rect.right_top() + egui::vec2(-12.0, 10.0),
                egui::Align2::RIGHT_TOP,
                "Updating…",
                egui::FontId::proportional(12.0),
                theme::TEXT_DIM,
            );
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 9.0);
        // The picture.
        widgets::group_title(ui, "Picture");
        let mut pick = self.source;
        widgets::labeled_row(ui, "From", 70.0, |ui| {
            let mut options = vec![(Source::Timeline, "The timeline".to_string())];
            options.extend(self.media.iter().map(|m| (Source::Media(m.0), m.1.clone())));
            egui::ComboBox::from_id_salt("thumbnail-source")
                .selected_text(self.source_name())
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (v, label) in &options {
                        ui.selectable_value(&mut pick, *v, label);
                    }
                });
        });
        if pick != self.source {
            self.source = pick;
            self.s.at = self.first_moment(0.0);
        }
        if let Some(span) = self.span() {
            widgets::labeled_row(ui, "Moment", 70.0, |ui| {
                widgets::slider(ui, &mut self.s.at, 0.0..=span.max(0.1), 1, " s");
            });
        }
        let mut canvas = self.s.canvas;
        widgets::labeled_row(ui, "Shape", 70.0, |ui| {
            widgets::segmented(ui, &mut canvas, &["Widescreen", "Square", "Vertical"]);
        });
        self.s.canvas = canvas;
        widgets::labeled_row(ui, "Punch", 70.0, |ui| {
            widgets::slider(ui, &mut self.s.punch, 0.0..=100.0, 0, "");
        });

        // The words.
        ui.add_space(4.0);
        widgets::group_title(ui, "Words");
        ui.add(
            egui::TextEdit::multiline(&mut self.s.text)
                .hint_text("Type the words for your thumbnail…")
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        let mut look = self.s.look;
        widgets::labeled_row(ui, "Style", 70.0, |ui| {
            egui::ComboBox::from_id_salt("thumbnail-look")
                .selected_text(LOOKS[look.min(LOOKS.len() - 1)].name)
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (i, l) in LOOKS.iter().enumerate() {
                        ui.selectable_value(&mut look, i, l.name);
                    }
                });
        });
        self.s.look = look;
        widgets::labeled_row(ui, "Size", 70.0, |ui| {
            widgets::slider(ui, &mut self.s.size, 40.0..=160.0, 0, "%");
        });
        let mut across = match self.s.across {
            Across::Left => 0,
            Across::Centre => 1,
            Across::Right => 2,
        };
        widgets::labeled_row(ui, "Across", 70.0, |ui| {
            widgets::segmented(ui, &mut across, &["Left", "Centre", "Right"]);
        });
        self.s.across = [Across::Left, Across::Centre, Across::Right][across];
        let mut down = match self.s.down {
            Down::Top => 0,
            Down::Middle => 1,
            Down::Bottom => 2,
        };
        widgets::labeled_row(ui, "Down", 70.0, |ui| {
            widgets::segmented(ui, &mut down, &["Top", "Middle", "Bottom"]);
        });
        self.s.down = [Down::Top, Down::Middle, Down::Bottom][down];
        widgets::labeled_row(ui, "Dark fade", 70.0, |ui| {
            widgets::slider(ui, &mut self.s.shade, 0.0..=100.0, 0, "");
        });
    }
}

impl Drop for ThumbnailDialog {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.scratch);
    }
}
