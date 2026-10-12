//! The Image Thumbnail Generator: type an idea (or pick a photo to put into a
//! drawn scene) and get four pictures to choose from. They are drawn on this
//! computer. The first time, a window explains the download of the models,
//! offers the two sizes, and downloads only when the person says so.

use super::{body, footer, header, primary, scroll_middle, secondary, window};
use crate::app::TidyCutsApp;
use crate::{theme, widgets};
use egui::{Color32, CornerRadius, RichText, Sense, Stroke};
use egui_phosphor::regular as ph;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tidycuts_engine::imagegen::{self, Placement, Quality, Request, Side, GLOWS, STYLES};

use super::sizes::fmt_bytes;

/// The shapes offered: (name, width, height). The thumbnail maker fits the
/// picture to its own shape afterwards.
const SHAPES: [(&str, u32, u32); 3] = [
    ("Widescreen", 1280, 720),
    ("Square", 1024, 1024),
    ("Vertical", 720, 1280),
];

/// How many pictures one press makes.
const PICTURES: u32 = 4;

enum Job {
    Idle,
    /// Downloading models: what it is doing and how far along.
    Downloading(String, f64),
    Downloaded(Result<(), String>),
    /// Making pictures: how far along.
    Making(f64),
    Made(Result<Vec<PathBuf>, String>),
}

#[derive(PartialEq, Clone, Copy)]
enum Screen {
    /// Choose, download, switch and remove the models.
    Models,
    Make,
}

pub struct ImageGenDialog {
    screen: Screen,
    /// Nothing was downloaded yet: the window opens as the first-time question.
    first_use: bool,
    /// The size of the models used for the next pictures.
    quality: Quality,
    /// The size picked in the models screen.
    picked: Quality,
    /// 0 from an idea, 1 a photo in a drawn scene.
    mode: usize,
    idea: String,
    style: usize,
    shape: usize,
    /// A photo of a person to draw into the picture (mode 0).
    person: Option<PathBuf>,
    /// Pictures whose look the new ones copy (up to three).
    inspiration: Vec<PathBuf>,
    /// The photo to place into the scene (mode 1).
    photo: Option<PathBuf>,
    remove_white: bool,
    side: usize,
    glow: usize,
    results: Vec<PathBuf>,
    textures: Vec<Option<egui::TextureHandle>>,
    selected: Option<usize>,
    job: Arc<Mutex<Job>>,
    cancel: Arc<AtomicBool>,
    error: Option<String>,
    work_dir: PathBuf,
}

/// Reads a PNG into a picture for the screen.
fn load_png(path: &Path) -> Option<egui::ColorImage> {
    let file = std::fs::File::open(path).ok()?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(file));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buf[..w * h * 4].to_vec(),
        png::ColorType::Rgb => buf[..w * h * 3]
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        _ => return None,
    };
    Some(egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba))
}

fn pick_photo(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter("Pictures", &["png", "jpg", "jpeg", "webp", "bmp"])
        .pick_file()
}

fn gb(bytes: u64) -> String {
    fmt_bytes(bytes)
}

impl ImageGenDialog {
    /// `models`: open on the models screen (from Preferences or the Manage button).
    pub fn new(app: &TidyCutsApp, models: bool) -> Self {
        let mut quality = Quality::from_id(&app.prefs.imagegen_models);
        if !imagegen::installed(quality) {
            if let Some(q) = Quality::ALL.into_iter().find(|q| imagegen::installed(*q)) {
                quality = q;
            }
        }
        let any = Quality::ALL.into_iter().any(imagegen::installed);
        ImageGenDialog {
            screen: if any && !models {
                Screen::Make
            } else {
                Screen::Models
            },
            first_use: !any,
            quality,
            picked: if any { quality } else { Quality::Standard },
            mode: 0,
            idea: String::new(),
            style: 0,
            shape: 0,
            person: None,
            inspiration: Vec::new(),
            photo: None,
            remove_white: true,
            side: 2,
            glow: 0,
            results: Vec::new(),
            textures: Vec::new(),
            selected: None,
            job: Arc::new(Mutex::new(Job::Idle)),
            cancel: Arc::new(AtomicBool::new(false)),
            error: None,
            work_dir: app.prefs.cache_dir().join("imagegen"),
        }
    }

    fn busy(&self) -> bool {
        matches!(
            *self.job.lock().unwrap(),
            Job::Downloading(..) | Job::Making(_)
        )
    }

    /// Downloads a size of the models (the person pressed its button).
    fn download(&mut self, q: Quality, ctx: &egui::Context) {
        self.error = None;
        self.cancel.store(false, Ordering::Relaxed);
        *self.job.lock().unwrap() = Job::Downloading("Starting".into(), 0.0);
        let (job, cancel, ctx) = (self.job.clone(), self.cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let r = imagegen::download(q, &cancel, |stage, f| {
                *job.lock().unwrap() = Job::Downloading(stage.to_string(), f);
                ctx.request_repaint();
            });
            *job.lock().unwrap() = Job::Downloaded(r);
            ctx.request_repaint();
        });
    }

    fn make(&mut self, ctx: &egui::Context) {
        let (_, w, h) = SHAPES[self.shape.min(SHAPES.len() - 1)];
        let person = (self.mode == 0).then(|| self.person.clone()).flatten();
        let mut references: Vec<PathBuf> = person.iter().cloned().collect();
        references.extend(self.inspiration.iter().cloned());
        let req = Request {
            prompt: imagegen::with_references(
                &imagegen::full_prompt(&self.idea, self.style),
                person.is_some(),
                self.inspiration.len(),
            ),
            width: w,
            height: h,
            count: PICTURES,
            seed: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_millis() as u64 % 1_000_000_000),
            references,
            quality: self.quality,
        };
        let photo = (self.mode == 1).then(|| self.photo.clone()).flatten();
        let placement = Placement {
            remove_white: self.remove_white,
            side: [Side::Left, Side::Centre, Side::Right][self.side.min(2)],
            glow: GLOWS[self.glow.min(GLOWS.len() - 1)].1,
        };
        let dir = self.work_dir.clone();
        self.error = None;
        self.selected = None;
        self.cancel.store(false, Ordering::Relaxed);
        *self.job.lock().unwrap() = Job::Making(0.0);
        let (job, cancel, ctx) = (self.job.clone(), self.cancel.clone(), ctx.clone());
        std::thread::spawn(move || {
            let set = |f: f64| {
                *job.lock().unwrap() = Job::Making(f);
                ctx.request_repaint();
            };
            let result = (|| {
                let scale = if photo.is_some() { 0.85 } else { 1.0 };
                let pics = imagegen::generate(&req, &dir, &cancel, |f| set(f * scale))?;
                let Some(photo) = photo else {
                    return Ok(pics);
                };
                let mut out = Vec::new();
                for (i, p) in pics.iter().enumerate() {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("Cancelled.".to_string());
                    }
                    let o = dir.join(format!("scene_{i}.png"));
                    imagegen::place_photo(p, &photo, &o, req.width, req.height, &placement)?;
                    out.push(o);
                    set(0.85 + 0.15 * (i + 1) as f64 / pics.len() as f64);
                }
                Ok(out)
            })();
            *job.lock().unwrap() = Job::Made(result);
            ctx.request_repaint();
        });
    }

    /// Takes in what a worker thread has finished.
    fn poll(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) {
        let finished = {
            let mut j = self.job.lock().unwrap();
            if matches!(*j, Job::Downloaded(_) | Job::Made(_)) {
                std::mem::replace(&mut *j, Job::Idle)
            } else {
                Job::Idle
            }
        };
        match finished {
            Job::Downloaded(Ok(())) => {
                self.first_use = false;
                self.quality = self.picked;
                self.screen = Screen::Make;
                app.prefs.imagegen_models = self.quality.id().to_string();
                app.prefs.save();
                app.notify("The image models are ready.", false);
            }
            Job::Downloaded(Err(e)) if e != "Cancelled." => self.error = Some(e),
            Job::Made(Ok(files)) => {
                self.textures = files
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        load_png(f).map(|img| {
                            ctx.load_texture(
                                format!("imagegen-{i}-{}", f.display()),
                                img,
                                egui::TextureOptions::LINEAR,
                            )
                        })
                    })
                    .collect();
                self.selected = (!files.is_empty()).then_some(0);
                self.results = files;
            }
            Job::Made(Err(e)) if e != "Cancelled." => self.error = Some(e),
            _ => {}
        }
    }

    /// Copies the chosen picture next to the project's other pictures.
    fn keep_selected(&self, app: &TidyCutsApp) -> Result<PathBuf, String> {
        let src = self
            .selected
            .and_then(|i| self.results.get(i))
            .ok_or("Choose a picture first.")?;
        let dir = app.stills_dir();
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dest = crate::thumbnail::free_name(&dir, "Generated picture", "png");
        std::fs::copy(src, &dest).map_err(|e| e.to_string())?;
        Ok(dest)
    }

    /// Draws the window. Returns false once it has closed.
    pub fn show(&mut self, app: &mut TidyCutsApp, ctx: &egui::Context) -> bool {
        self.poll(app, ctx);
        let mut close = false;
        let mut use_quality: Option<Quality> = None;
        let mut get: Option<Quality> = None;
        let mut remove: Option<Quality> = None;
        let mut go_models = false;
        let mut generate = false;
        let mut add_words = false;
        let mut save = false;
        let busy = self.busy();
        let stop = self.cancel.clone();
        let (_, keys) = window(ctx, "imagegen", 940.0, |ui| {
            close |= header(
                ui,
                ph::SPARKLE,
                "Image Thumbnail Generator",
                if self.screen == Screen::Models {
                    "Choose the models that draw the pictures"
                } else {
                    "Describe your thumbnail and get pictures to choose from"
                },
            );
            scroll_middle(ui, |ui| {
                body(ui, |ui| match self.screen {
                    Screen::Models => {
                        self.models_screen(ui, &mut use_quality, &mut get, &mut remove)
                    }
                    Screen::Make => self.make_screen(ui, ctx, &mut go_models),
                })
            });
            footer(
                ui,
                |ui| {
                    if busy && secondary(ui, "Stop").clicked() {
                        stop.store(true, Ordering::Relaxed);
                    }
                },
                |ui| {
                    if self.screen == Screen::Models {
                        if secondary(ui, if self.first_use { "Not now" } else { "Back" }).clicked()
                        {
                            if self.first_use || !Quality::ALL.into_iter().any(imagegen::installed)
                            {
                                close = true;
                            } else {
                                self.screen = Screen::Make;
                            }
                        }
                    } else {
                        if secondary(ui, "Close").clicked() {
                            close = true;
                        }
                        let have = self.selected.is_some() && !busy;
                        if secondary(ui, &format!("{}  Save picture…", ph::DOWNLOAD_SIMPLE))
                            .clicked()
                            && have
                        {
                            save = true;
                        }
                        if ui
                            .add_enabled(
                                have,
                                theme::outline_button(format!("{}  Add words…", ph::TEXT_T))
                                    .min_size(egui::vec2(120.0, 34.0)),
                            )
                            .clicked()
                        {
                            add_words = true;
                        }
                        let ready = !busy && self.can_make();
                        if primary(
                            ui,
                            &format!("{}  Make {PICTURES} pictures", ph::SPARKLE),
                            ready,
                        )
                        .clicked()
                        {
                            generate = true;
                        }
                    }
                },
            );
        });
        if let Some(q) = use_quality {
            self.quality = q;
            self.picked = q;
            app.prefs.imagegen_models = q.id().to_string();
            app.prefs.save();
            self.screen = Screen::Make;
        }
        if let Some(q) = get {
            self.picked = q;
            self.download(q, ctx);
        }
        if let Some(q) = remove {
            match imagegen::remove(q) {
                Ok(()) => {
                    if self.quality == q {
                        if let Some(o) = Quality::ALL.into_iter().find(|o| imagegen::installed(*o))
                        {
                            self.quality = o;
                        }
                    }
                }
                Err(e) => self.error = Some(e),
            }
        }
        if go_models {
            self.screen = Screen::Models;
        }
        if generate {
            self.make(ctx);
        }
        if save {
            if let Some(src) = self.selected.and_then(|i| self.results.get(i)) {
                let name =
                    crate::thumbnail::free_name(&app.stills_dir(), "Generated picture", "png");
                if let Some(dest) = rfd::FileDialog::new()
                    .set_title("Save the picture")
                    .set_directory(app.stills_dir())
                    .set_file_name(
                        name.file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    )
                    .add_filter("PNG picture", &["png"])
                    .save_file()
                {
                    match std::fs::copy(src, &dest) {
                        Ok(_) => app.notify("Saved the picture.", false),
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
            }
        }
        if add_words {
            match self
                .keep_selected(app)
                .and_then(|p| super::thumbnail::ThumbnailDialog::with_picture(app, &p))
            {
                Ok(d) => {
                    app.dialogs.thumbnail = Some(d);
                    return false;
                }
                Err(e) => self.error = Some(e),
            }
        }
        let done = close || keys.escape;
        if done {
            self.cancel.store(true, Ordering::Relaxed);
        }
        !done
    }

    fn can_make(&self) -> bool {
        imagegen::available(self.quality)
            && !self.idea.trim().is_empty()
            && (self.mode == 0 || self.photo.is_some())
    }

    // ---- the models screen ----------------------------------------------------

    fn models_screen(
        &mut self,
        ui: &mut egui::Ui,
        use_quality: &mut Option<Quality>,
        get: &mut Option<Quality>,
        remove: &mut Option<Quality>,
    ) {
        let busy = self.busy();
        if self.first_use {
            ui.add(
                egui::Label::new(
                    RichText::new("A one-time download is needed")
                        .size(16.0)
                        .color(Color32::WHITE),
                )
                .wrap(),
            );
        }
        ui.add(
            egui::Label::new(
                RichText::new(
                    "The generator draws pictures on your computer, for free, with no account. \
                     It needs model files, which are downloaded from the internet once. After \
                     that it works without a connection, and your ideas and photos never leave \
                     this computer. You can download both sizes and pick one for each thumbnail.",
                )
                .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
        if imagegen::program_file().is_none() {
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "The image program that comes with TidyCuts (the imagegen folder next to \
                         the program) was not found, so pictures cannot be made here.",
                    )
                    .color(theme::WARNING),
                )
                .wrap(),
            );
        }
        ui.add_space(6.0);
        let job = match &*self.job.lock().unwrap() {
            Job::Downloading(s, f) => Some((s.clone(), *f)),
            _ => None,
        };
        for q in Quality::ALL {
            let installed = imagegen::installed(q);
            let current = installed && q == self.quality && !self.first_use;
            let is_downloading = job.is_some() && self.picked == q;
            egui::Frame::new()
                .fill(if self.picked == q {
                    theme::ACCENT_BG
                } else {
                    theme::PANEL_BG_2
                })
                .stroke(Stroke::new(
                    1.0,
                    if self.picked == q {
                        theme::ACCENT
                    } else {
                        theme::BORDER
                    },
                ))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::same(14))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width() - 190.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(q.name()).size(15.0).color(Color32::WHITE));
                                ui.label(
                                    RichText::new(gb(q.bytes()))
                                        .size(13.0)
                                        .color(theme::TEXT_DIM),
                                );
                                if installed {
                                    ui.label(
                                        RichText::new(format!(
                                            "{}  {}",
                                            ph::CHECK_CIRCLE,
                                            if current {
                                                "Downloaded, in use"
                                            } else {
                                                "Downloaded"
                                            }
                                        ))
                                        .size(12.5)
                                        .color(theme::ACCENT),
                                    );
                                }
                            });
                            ui.add(
                                egui::Label::new(
                                    RichText::new(q.blurb()).size(12.5).color(theme::TEXT_DIM),
                                )
                                .wrap(),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if installed {
                                if !current
                                    && ui
                                        .add_enabled(
                                            !busy,
                                            theme::accent_button("Use this one")
                                                .min_size(egui::vec2(120.0, 32.0)),
                                        )
                                        .clicked()
                                {
                                    *use_quality = Some(q);
                                }
                                if ui
                                    .add_enabled(
                                        !busy,
                                        theme::outline_button(ph::TRASH)
                                            .min_size(egui::vec2(36.0, 32.0)),
                                    )
                                    .on_hover_text("Delete these model files to free the space")
                                    .clicked()
                                {
                                    *remove = Some(q);
                                }
                            } else if ui
                                .add_enabled(
                                    !busy && imagegen::program_file().is_some(),
                                    theme::accent_button(&format!(
                                        "{}  Download ({})",
                                        ph::DOWNLOAD_SIMPLE,
                                        gb(q.bytes())
                                    ))
                                    .min_size(egui::vec2(170.0, 34.0)),
                                )
                                .clicked()
                            {
                                *get = Some(q);
                            }
                        });
                    });
                    if is_downloading {
                        if let Some((stage, f)) = &job {
                            ui.add_space(6.0);
                            ui.add(
                                egui::ProgressBar::new(*f as f32)
                                    .text(format!("{stage}… {:.0}%", f * 100.0))
                                    .animate(true),
                            );
                            ui.ctx()
                                .request_repaint_after(std::time::Duration::from_millis(200));
                        }
                    }
                });
        }
        widgets::hint(
            ui,
            &format!(
                "The files come from huggingface.co and are kept in {}. Each download is checked \
                 for damage. You need that much free disk space, and a graphics card.",
                imagegen::models_dir().map_or("the TidyCuts settings folder".into(), |p| p
                    .display()
                    .to_string())
            ),
        );
        if let Some(e) = &self.error {
            ui.add(egui::Label::new(RichText::new(e).color(theme::WARNING)).wrap());
        }
    }

    // ---- the making screen ----------------------------------------------------

    fn make_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, go_models: &mut bool) {
        let busy = self.busy();
        let making = match *self.job.lock().unwrap() {
            Job::Making(f) => Some(f),
            _ => None,
        };
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 20.0;
            // The pictures.
            ui.vertical(|ui| {
                ui.set_width(560.0);
                self.grid(ui, making);
            });
            // The controls.
            ui.vertical(|ui| {
                ui.set_width(300.0);
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 9.0);
                ui.add_enabled_ui(!busy, |ui| self.controls(ui, ctx, go_models));
            });
        });
        if let Some(e) = &self.error {
            ui.add(egui::Label::new(RichText::new(e).color(theme::WARNING)).wrap());
        }
    }

    fn grid(&mut self, ui: &mut egui::Ui, making: Option<f64>) {
        let cell = egui::vec2(275.0, 275.0 * 9.0 / 16.0 + 40.0);
        let slots = PICTURES as usize;
        for row in 0..slots / 2 {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
                for col in 0..2 {
                    let i = row * 2 + col;
                    let (rect, resp) = ui.allocate_exact_size(cell, Sense::click());
                    let inner =
                        egui::Rect::from_min_size(rect.min, egui::vec2(cell.x, cell.y - 40.0));
                    ui.painter().rect_filled(
                        inner,
                        CornerRadius::same(6),
                        Color32::from_rgb(0x10, 0x12, 0x15),
                    );
                    let tex = self.textures.get(i).and_then(|t| t.as_ref());
                    if let Some(t) = tex {
                        let size = t.size_vec2();
                        let k = (inner.width() / size.x).min(inner.height() / size.y);
                        let r = egui::Rect::from_center_size(inner.center(), size * k);
                        ui.painter().image(
                            t.id(),
                            r,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    } else {
                        let text = if making.is_some() { "Drawing…" } else { "" };
                        ui.painter().text(
                            inner.center(),
                            egui::Align2::CENTER_CENTER,
                            text,
                            egui::FontId::proportional(13.0),
                            theme::TEXT_DIM,
                        );
                    }
                    if self.selected == Some(i) && tex.is_some() {
                        ui.painter().rect_stroke(
                            inner,
                            CornerRadius::same(6),
                            Stroke::new(2.5, theme::ACCENT),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if resp.clicked() && tex.is_some() {
                        self.selected = Some(i);
                    }
                }
            });
        }
        if let Some(f) = making {
            ui.add(
                egui::ProgressBar::new(f as f32)
                    .text(format!("Drawing… {:.0}%", f * 100.0))
                    .animate(true),
            );
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        } else if self.results.is_empty() {
            widgets::hint(
                ui,
                "Four pictures appear here, about half a minute after you press Make. Click the one you like.",
            );
        }
    }

    /// The pictures whose look the new ones copy.
    fn inspiration_rows(&mut self, ui: &mut egui::Ui) {
        let mut remove = None;
        widgets::labeled_row(ui, "Inspiration", 70.0, |ui| {
            let room = self.inspiration.len() < imagegen::MAX_INSPIRATION;
            if ui
                .add_enabled(
                    room,
                    theme::outline_button(format!("{}  Add a picture…", ph::PLUS)),
                )
                .clicked()
            {
                if let Some(p) = pick_photo("A picture whose look you like") {
                    self.inspiration.push(p);
                }
            }
        });
        for (i, p) in self.inspiration.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.add_space(78.0);
                let name = p
                    .file_name()
                    .map_or(String::new(), |n| n.to_string_lossy().into_owned());
                ui.label(RichText::new(name).size(12.5).color(theme::TEXT));
                if widgets::icon_button(ui, ph::X, "Take this picture out").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.inspiration.remove(i);
        }
        if !self.inspiration.is_empty() {
            widgets::hint(
                ui,
                "The new pictures copy the colors, lighting and mood of these, not what is in them.",
            );
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context, go_models: &mut bool) {
        // The model: both sizes can be downloaded, and one is picked per thumbnail.
        widgets::group_title(ui, "Model");
        let installed: Vec<Quality> = Quality::ALL
            .into_iter()
            .filter(|q| imagegen::installed(*q))
            .collect();
        ui.horizontal(|ui| {
            let options: Vec<(usize, &str)> = installed
                .iter()
                .enumerate()
                .map(|(i, q)| (i, q.name()))
                .collect();
            let mut at = installed
                .iter()
                .position(|q| *q == self.quality)
                .unwrap_or(0);
            if widgets::choice(ui, "imagegen-model", &mut at, &options) {
                if let Some(q) = installed.get(at) {
                    self.quality = *q;
                }
            }
            if ui
                .add(theme::outline_button("Manage…"))
                .on_hover_text("Download the other size, or delete one")
                .clicked()
            {
                *go_models = true;
            }
        });
        widgets::group_title(ui, "What to make");
        widgets::segmented(ui, &mut self.mode, &["From an idea", "My photo in a scene"]);
        if self.mode == 1 {
            widgets::hint(
                ui,
                "Your real photo stays exact (a product, say). The scene around it is drawn from your idea.",
            );
        }
        ui.add(
            egui::TextEdit::multiline(&mut self.idea)
                .hint_text(if self.mode == 0 {
                    "Describe your thumbnail, e.g. a shocked man pointing at a glowing red button"
                } else {
                    "Describe the scene behind it, e.g. a jet cockpit at sunset"
                })
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        widgets::labeled_row(ui, "Style", 70.0, |ui| {
            let options: Vec<(usize, &str)> =
                STYLES.iter().enumerate().map(|(i, s)| (i, s.1)).collect();
            widgets::choice(ui, "imagegen-style", &mut self.style, &options);
        });
        let names: Vec<&str> = SHAPES.iter().map(|s| s.0).collect();
        widgets::labeled_row(ui, "Shape", 70.0, |ui| {
            widgets::segmented(ui, &mut self.shape, &names);
        });
        if self.mode == 0 {
            widgets::labeled_row(ui, "Person", 70.0, |ui| {
                let label = self
                    .person
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map_or("Optional: a photo".to_string(), |n| {
                        n.to_string_lossy().into_owned()
                    });
                if ui.add(theme::outline_button(label)).clicked() {
                    if let Some(p) = pick_photo("A photo of the person to include") {
                        self.person = Some(p);
                    }
                }
                if self.person.is_some()
                    && widgets::icon_button(ui, ph::X, "Take the person out").clicked()
                {
                    self.person = None;
                }
            });
            if self.person.is_some() {
                widgets::hint(ui, "The face comes out similar, not identical.");
            }
        }
        self.inspiration_rows(ui);
        if self.mode == 1 {
            widgets::labeled_row(ui, "Photo", 70.0, |ui| {
                let label = self
                    .photo
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map_or("Choose a photo…".to_string(), |n| {
                        n.to_string_lossy().into_owned()
                    });
                if ui.add(theme::outline_button(label)).clicked() {
                    if let Some(p) = pick_photo("The photo to place in the scene") {
                        self.photo = Some(p);
                    }
                }
            });
            super::switch(
                ui,
                &mut self.remove_white,
                "It has a plain white background",
            );
            widgets::labeled_row(ui, "Place it", 70.0, |ui| {
                widgets::segmented(ui, &mut self.side, &["Left", "Centre", "Right"]);
            });
            widgets::labeled_row(ui, "Glow", 70.0, |ui| {
                let options: Vec<(usize, &str)> =
                    GLOWS.iter().enumerate().map(|(i, g)| (i, g.0)).collect();
                widgets::choice(ui, "imagegen-glow", &mut self.glow, &options);
            });
        }
        widgets::hint(
            ui,
            "Pictures have no words on them. Press Add words to put your title on in clear letters.",
        );
    }
}
