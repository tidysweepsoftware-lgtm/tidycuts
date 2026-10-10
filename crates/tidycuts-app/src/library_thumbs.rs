//! Pictures for the catalog tiles in the library: each title, transition,
//! effect, filter, sticker and template drawn by the real compositor on a
//! procedural sample picture (no stock photos ship), plus a short looping
//! animation while the pointer rests on a tile, like most libraries.
//!
//! A worker thread renders them with its own compositor on the editor's
//! graphics device. Only tiles that are drawn (visible) get queued, newest
//! first, and queued tiles that scrolled away are dropped. Results become
//! egui textures keyed by entry and pixel size; hover frames are rendered
//! only for the hovered tile. Both caches are bounded. Until a picture is
//! ready the tile shows its placeholder icon.

mod scene;
mod worker;

use crate::app::TidyCutsApp;
use crate::library::{CatalogItem, LibraryTab};
use crate::theme;
use egui::load::SizedTexture;
use egui::{Align2, CornerRadius, FontId, Id, Rect, TextureHandle, TextureOptions};
use scene::Key;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use worker::{AnimJob, Done, Jobs, Px, Shared};

/// Most tile stills kept as textures (about 80 KB each at 192x108).
const MAX_STILLS: usize = 320;
/// Most hover loops kept (up to 30 frames each).
const MAX_LOOPS: usize = 6;

/// Paints a catalog tile's picture into `rect`.
pub fn draw(
    app: &mut TidyCutsApp,
    ui: &mut egui::Ui,
    tab: LibraryTab,
    item: &CatalogItem,
    rect: Rect,
    hovered: bool,
) {
    let (Some(key), Some(engine)) = (Key::of(&item.drag), app.engine.as_ref()) else {
        placeholder(ui, tab, rect);
        return;
    };
    let ctx = ui.ctx().clone();
    let id = Id::new("library-tile-pictures");
    let cell = match ctx.data(|d| d.get_temp::<Arc<Mutex<Tiles>>>(id)) {
        Some(c) => c,
        None => {
            let c = Arc::new(Mutex::new(Tiles::new(&ctx, engine.renderer.clone())));
            ctx.data_mut(|d| d.insert_temp(id, c.clone()));
            c
        }
    };
    let Ok(mut tiles) = cell.lock() else {
        placeholder(ui, tab, rect);
        return;
    };
    tiles.begin_pass(&ctx);
    let px = bucket(rect, ctx.pixels_per_point());
    let now = ui.input(|i| i.time);
    match tiles.picture(&ctx, key, px, hovered, now) {
        Some(tex) => {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(6), theme::PANEL_BG_2);
            let fit = crate::library::fit_rect(rect, tex.size_vec2());
            egui::Image::from_texture(SizedTexture::new(tex.id(), fit.size()))
                .corner_radius(CornerRadius::same(6))
                .paint_at(ui, fit);
        }
        None => placeholder(ui, tab, rect),
    }
}

/// A rendered picture of a catalog entry, if one is ready (for the drag
/// ghost).
pub fn texture_for(ctx: &egui::Context, item: &CatalogItem) -> Option<TextureHandle> {
    let key = Key::of(&item.drag)?;
    let cell = ctx.data(|d| d.get_temp::<Arc<Mutex<Tiles>>>(Id::new("library-tile-pictures")))?;
    let tiles = cell.lock().ok()?;
    tiles
        .stills
        .iter()
        .filter(|((k, _), _)| *k == key)
        .max_by_key(|((_, px), _)| px.0)
        .map(|(_, s)| s.tex.clone())
}

fn placeholder(ui: &egui::Ui, tab: LibraryTab, rect: Rect) {
    ui.painter()
        .rect_filled(rect, CornerRadius::same(6), theme::PANEL_BG_2);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        tab.icon(),
        FontId::proportional(22.0),
        theme::TEXT_FAINT,
    );
}

/// Picture size for a tile: a few fixed 16:9 sizes, so resizing the tiles
/// doesn't re-render everything at every step.
fn bucket(rect: Rect, ppp: f32) -> Px {
    let w = rect.width() * ppp;
    let bw = if w <= 200.0 {
        192
    } else if w <= 272.0 {
        256
    } else if w <= 400.0 {
        384
    } else {
        512
    };
    (bw, bw * 9 / 16)
}

struct Still {
    tex: TextureHandle,
    used: u64,
}

/// A tile's hover loop.
struct Loop {
    frames: Vec<Option<TextureHandle>>,
    /// Seconds per frame.
    interval: f32,
    /// Doesn't move: the still is shown instead.
    still_only: bool,
    /// The hover its frames were last asked for.
    asked: u64,
    used: u64,
}

struct Hover {
    key: Key,
    px: Px,
    since: f64,
    id: u64,
}

/// The tile picture cache, kept in egui's memory.
struct Tiles {
    shared: Arc<Shared>,
    done: Receiver<Done>,
    stills: HashMap<(Key, Px), Still>,
    /// Stills queued or being rendered.
    pending: HashSet<(Key, Px)>,
    failed: HashSet<Key>,
    loops: HashMap<(Key, Px), Loop>,
    hover: Option<Hover>,
    /// Some tile reported the pointer on it during this pass.
    hovered_now: bool,
    /// Tiles drawn during this pass and the one before.
    seen_now: HashSet<(Key, Px)>,
    seen_before: HashSet<(Key, Px)>,
    pass: u64,
    tick: u64,
}

impl Tiles {
    fn new(ctx: &egui::Context, renderer: Arc<tidycuts_render::Renderer>) -> Tiles {
        let shared = Arc::new(Shared {
            jobs: Mutex::new(Jobs::default()),
            cv: Condvar::new(),
            hover: Default::default(),
        });
        let (tx, rx) = channel();
        worker::spawn(renderer, shared.clone(), tx, ctx.clone());
        Tiles {
            shared,
            done: rx,
            stills: HashMap::new(),
            pending: HashSet::new(),
            failed: HashSet::new(),
            loops: HashMap::new(),
            hover: None,
            hovered_now: false,
            seen_now: HashSet::new(),
            seen_before: HashSet::new(),
            pass: u64::MAX,
            tick: 0,
        }
    }

    /// Once per pass: takes finished pictures, forgets queued tiles that
    /// are no longer on screen and ends a hover that moved off the tiles.
    fn begin_pass(&mut self, ctx: &egui::Context) {
        let pass = ctx.cumulative_pass_nr();
        if pass == self.pass {
            return;
        }
        let first = self.pass == u64::MAX;
        self.pass = pass;
        self.seen_before = std::mem::take(&mut self.seen_now);
        if !first {
            if !self.hovered_now && self.hover.take().is_some() {
                self.shared.hover.fetch_add(1, Ordering::Relaxed);
            }
            let seen = &self.seen_before;
            let mut jobs = self.shared.jobs.lock().unwrap();
            let pending = &mut self.pending;
            jobs.stills.retain(|j| {
                let keep = seen.contains(j);
                if !keep {
                    pending.remove(j);
                }
                keep
            });
        }
        self.hovered_now = false;
        self.poll(ctx);
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(done) = self.done.try_recv() {
            match done {
                Done::Still { key, px, image } => {
                    self.pending.remove(&(key, px));
                    self.tick += 1;
                    let tex = ctx.load_texture(
                        format!("library-tile-{key:?}"),
                        image,
                        TextureOptions::LINEAR,
                    );
                    self.stills.insert(
                        (key, px),
                        Still {
                            tex,
                            used: self.tick,
                        },
                    );
                }
                Done::Frame {
                    key,
                    px,
                    index,
                    image,
                } => {
                    if let Some(slot) = self
                        .loops
                        .get_mut(&(key, px))
                        .and_then(|l| l.frames.get_mut(index))
                    {
                        *slot = Some(ctx.load_texture(
                            format!("library-tile-{key:?}-{index}"),
                            image,
                            TextureOptions::LINEAR,
                        ));
                    }
                }
                Done::Static { key, px } => {
                    if let Some(l) = self.loops.get_mut(&(key, px)) {
                        l.still_only = true;
                        l.frames.clear();
                    }
                }
                Done::Failed { key, px } => {
                    self.pending.remove(&(key, px));
                    self.failed.insert(key);
                }
            }
        }
        self.evict();
    }

    fn evict(&mut self) {
        if self.stills.len() > MAX_STILLS {
            let mut ages: Vec<u64> = self.stills.values().map(|s| s.used).collect();
            ages.sort_unstable();
            let cutoff = ages[self.stills.len() - MAX_STILLS * 3 / 4];
            self.stills.retain(|_, s| s.used >= cutoff);
        }
        if self.loops.len() > MAX_LOOPS {
            let hovered = self.hover.as_ref().map(|h| (h.key, h.px));
            let oldest = self
                .loops
                .iter()
                .filter(|(k, _)| Some(**k) != hovered)
                .min_by_key(|(_, l)| l.used)
                .map(|(k, _)| *k);
            if let Some(k) = oldest {
                self.loops.remove(&k);
            }
        }
    }

    /// The texture to show for a tile now, queueing whatever is missing.
    fn picture(
        &mut self,
        ctx: &egui::Context,
        key: Key,
        px: Px,
        hovered: bool,
        now: f64,
    ) -> Option<TextureHandle> {
        self.tick += 1;
        self.seen_now.insert((key, px));
        if self.failed.contains(&key) {
            return None;
        }
        if hovered {
            self.hovered_now = true;
            if let Some(t) = self.hover_frame(ctx, key, px, now) {
                return Some(t);
            }
        }
        if let Some(s) = self.stills.get_mut(&(key, px)) {
            s.used = self.tick;
            return Some(s.tex.clone());
        }
        if self.pending.insert((key, px)) {
            let mut jobs = self.shared.jobs.lock().unwrap();
            jobs.stills.push_back((key, px));
            self.shared.cv.notify_one();
        }
        // Another size of the same picture while the right one renders.
        self.stills
            .iter()
            .find(|((k, _), _)| *k == key)
            .map(|(_, s)| s.tex.clone())
    }

    /// The current frame of a hovered tile's loop, asking for the frames
    /// it doesn't have yet.
    fn hover_frame(
        &mut self,
        ctx: &egui::Context,
        key: Key,
        px: Px,
        now: f64,
    ) -> Option<TextureHandle> {
        if self
            .hover
            .as_ref()
            .is_none_or(|h| (h.key, h.px) != (key, px))
        {
            let id = self.shared.hover.fetch_add(1, Ordering::Relaxed) + 1;
            self.hover = Some(Hover {
                key,
                px,
                since: now,
                id,
            });
        }
        let (hover_id, since) = self.hover.as_ref().map(|h| (h.id, h.since))?;
        let tick = self.tick;
        let lp = self.loops.entry((key, px)).or_insert_with(|| {
            let plan = scene::hover_loop(key);
            Loop {
                still_only: plan.times.is_empty(),
                frames: vec![None; plan.times.len()],
                interval: plan.interval.max(0.02),
                asked: 0,
                used: tick,
            }
        });
        lp.used = tick;
        if lp.still_only {
            return None;
        }
        let n = lp.frames.len();
        if lp.asked != hover_id && lp.frames.iter().any(Option::is_none) {
            // Start and middle first, so a loop that doesn't move is
            // caught after two frames.
            let mid = n / 2;
            let order = std::iter::once(0)
                .chain(std::iter::once(mid))
                .chain((1..n).filter(|i| *i != mid))
                .filter(|i| lp.frames[*i].is_none())
                .collect();
            lp.asked = hover_id;
            let mut jobs = self.shared.jobs.lock().unwrap();
            jobs.anim = Some(AnimJob {
                key,
                px,
                frames: order,
                hover: hover_id,
            });
            self.shared.cv.notify_one();
        }
        ctx.request_repaint_after(Duration::from_secs_f32(lp.interval));
        let at = ((now - since).max(0.0) / lp.interval as f64) as usize % n;
        // The frame due, or the latest one before it that's ready.
        lp.frames[..=at].iter().rev().flatten().next().cloned()
    }
}
