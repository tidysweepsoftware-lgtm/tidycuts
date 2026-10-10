//! The thread that renders library tiles with its own compositor on the
//! editor's graphics device, so the window never waits for a thumbnail.

use super::scene::{scene, sweep_position, Key, Pic, Scene, Split};
use eframe::wgpu;
use egui::ColorImage;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use tidycuts_core::{Clip, MediaItem, Project, Time};
use tidycuts_engine::levels::AudioLevels;
use tidycuts_engine::{FrameProvider, Planner, Statics};
use tidycuts_render::{Compositor, Pixels, Renderer, RgbaImage, Target};

/// Picture size in pixels.
pub type Px = (u32, u32);

/// Frames of one tile's hover loop to render.
pub struct AnimJob {
    pub key: Key,
    pub px: Px,
    /// Frame indices, in the order to render them.
    pub frames: Vec<usize>,
    /// The hover this is for; stale jobs stop early.
    pub hover: u64,
}

#[derive(Default)]
pub struct Jobs {
    /// Tile stills in the order they were first drawn. Tiles that scroll
    /// out of view are taken out, so this is always what's on screen.
    pub stills: VecDeque<(Key, Px)>,
    /// The hovered tile's loop; replaces any earlier one.
    pub anim: Option<AnimJob>,
}

pub struct Shared {
    pub jobs: Mutex<Jobs>,
    pub cv: Condvar,
    /// Bumped whenever the hovered tile changes.
    pub hover: AtomicU64,
}

pub enum Done {
    Still {
        key: Key,
        px: Px,
        image: ColorImage,
    },
    Frame {
        key: Key,
        px: Px,
        index: usize,
        image: ColorImage,
    },
    /// The loop doesn't change over time; show the still instead.
    Static {
        key: Key,
        px: Px,
    },
    /// Nothing can be drawn for this entry.
    Failed {
        key: Key,
        px: Px,
    },
}

enum Job {
    Still(Key, Px),
    Anim(AnimJob),
}

/// Sample pictures handed to the planner in place of decoded video.
struct Pics<'a>(&'a mut HashMap<(Pic, u32, u32), Arc<RgbaImage>>);

impl FrameProvider for Pics<'_> {
    fn frame(&mut self, _clip: &Clip, media: &MediaItem, _at: f64, size: Px) -> Option<Pixels> {
        let pic = Pic::from_path(&media.path)?;
        if self.0.len() > 24 {
            self.0.clear();
        }
        let img = self
            .0
            .entry((pic, size.0, size.1))
            .or_insert_with(|| Arc::new(pic.draw(size.0, size.1)))
            .clone();
        Some(Pixels::Rgba(img))
    }
}

/// A compositor with what it needs to draw tile projects.
pub struct Studio {
    comp: Compositor,
    statics: Statics,
    targets: HashMap<Px, Target>,
    pics: HashMap<(Pic, u32, u32), Arc<RgbaImage>>,
    /// Made-up moving levels, so visualizer pictures show something.
    levels: AudioLevels,
}

impl Studio {
    pub fn new(renderer: Arc<Renderer>) -> Studio {
        Studio {
            comp: Compositor::new(renderer),
            statics: Statics::default(),
            targets: HashMap::new(),
            pics: HashMap::new(),
            levels: {
                let l = AudioLevels::default();
                l.set_demo(true);
                l
            },
        }
    }

    /// Draws `project` at `at` as `px` RGBA pixels (opaque, sRGB).
    pub fn render(&mut self, project: &Project, at: Time, px: Px) -> Vec<u8> {
        let auto = |_: &MediaItem| None;
        let mut pics = Pics(&mut self.pics);
        let mut planner = Planner {
            project,
            out_scale: px.0 as f32 / project.settings.width.max(1) as f32,
            frames: &mut pics,
            statics: &self.statics,
            auto_grade: &auto,
            levels: &self.levels,
            show_empty_cells: true,
            used_statics: Vec::new(),
        };
        let prepared = planner.build(at);
        for (k, img) in &prepared.statics {
            if !self.comp.has_static(*k) {
                self.comp.store_static(*k, img);
            }
        }
        if self.targets.len() > 4 {
            self.targets.clear();
        }
        let comp = &self.comp;
        let target = self
            .targets
            .entry(px)
            .or_insert_with(|| comp.make_target(px.0, px.1, wgpu::TextureFormat::Rgba8Unorm, true));
        self.comp.render_to_cpu(&prepared.frame, target)
    }
}

fn image(px: Px, rgba: &[u8]) -> ColorImage {
    ColorImage::from_rgba_unmultiplied([px.0 as usize, px.1 as usize], rgba)
}

/// Whether two renders look the same.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.abs_diff(*y) <= 2)
}

/// The two renders side by side: `before` left of the line at `pos` (0..1
/// from the left), `after` right of it, with a white line and a round
/// handle with arrows on it.
fn split_view(before: &[u8], after: &[u8], px: Px, pos: f32) -> Vec<u8> {
    let (w, h) = (px.0 as usize, px.1 as usize);
    let sx = pos * w as f32;
    let mut out = vec![255u8; w * h * 4];
    let line = (w as f32 / 110.0).max(1.5);
    let r = h as f32 * 0.085;
    let cy = h as f32 * 0.8;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let fx = x as f32 + 0.5;
            let src = if fx < sx { before } else { after };
            let mut c = [src[i] as f32, src[i + 1] as f32, src[i + 2] as f32];
            let d = (fx - sx).abs();
            // A soft shadow, then the line itself.
            let shadow = ((line * 2.2 - d) / (line * 1.7)).clamp(0.0, 1.0) * 0.28;
            let line_a = (line * 0.5 + 0.5 - d).clamp(0.0, 1.0);
            // The handle.
            let (dx, dy) = (fx - sx, y as f32 + 0.5 - cy);
            let dist = (dx * dx + dy * dy).sqrt();
            let disc = (r + 0.5 - dist).clamp(0.0, 1.0);
            let ring_shadow = ((r * 1.35 - dist) / (r * 0.35)).clamp(0.0, 1.0) * 0.3;
            for v in &mut c {
                *v *= 1.0 - shadow.max(ring_shadow);
                *v += (255.0 - *v) * line_a.max(disc * 0.95);
            }
            // Dark arrows pointing left and right.
            let arrow = |dir: f32| {
                let u = (dx * dir - r * 0.18) / (r * 0.42);
                (0.0..=1.0).contains(&u) && dy.abs() <= r * 0.34 * (1.0 - u)
            };
            if disc > 0.5 && (arrow(1.0) || arrow(-1.0)) {
                c.fill(70.0);
            }
            out[i] = c[0] as u8;
            out[i + 1] = c[1] as u8;
            out[i + 2] = c[2] as u8;
        }
    }
    out
}

/// The tile's still picture: the result, or the before/after split.
fn render_still(studio: &mut Studio, s: &Scene, px: Px) -> Vec<u8> {
    let mut rgba = studio.render(&s.project, s.still, px);
    if !s.blend.is_empty() {
        // Several moments of a motion laid over each other, so the tile
        // shows the movement without playing it.
        let mut sum: Vec<f32> = rgba.iter().map(|v| *v as f32).collect();
        for at in &s.blend {
            let more = studio.render(&s.project, *at, px);
            for (a, b) in sum.iter_mut().zip(&more) {
                *a += *b as f32;
            }
        }
        let n = (s.blend.len() + 1) as f32;
        for (o, a) in rgba.iter_mut().zip(&sum) {
            *o = (*a / n) as u8;
        }
    }
    match &s.split {
        Some(Split {
            plain,
            still: Some(pos),
            ..
        }) => {
            let before = studio.render(plain, s.still, px);
            split_view(&before, &rgba, px, *pos)
        }
        _ => rgba,
    }
}

fn next_job(shared: &Shared) -> Job {
    let mut jobs = shared.jobs.lock().unwrap();
    loop {
        if let Some(a) = jobs.anim.take() {
            return Job::Anim(a);
        }
        if let Some((k, px)) = jobs.stills.pop_front() {
            return Job::Still(k, px);
        }
        jobs = shared.cv.wait(jobs).unwrap();
    }
}

/// Renders one job, sending each picture as it's done. Returns false once
/// nobody listens any more.
fn run(studio: &mut Studio, shared: &Shared, job: Job, tx: &Sender<Done>) -> bool {
    match job {
        Job::Still(key, px) => {
            let done = match scene(key) {
                Some(s) => Done::Still {
                    key,
                    px,
                    image: image(px, &render_still(studio, &s, px)),
                },
                None => Done::Failed { key, px },
            };
            tx.send(done).is_ok()
        }
        Job::Anim(job) => {
            let Some(Scene {
                project,
                frames,
                split,
                ..
            }) = scene(job.key)
            else {
                return tx
                    .send(Done::Failed {
                        key: job.key,
                        px: job.px,
                    })
                    .is_ok();
            };
            let total = frames.len();
            let send = |index: usize, rgba: &[u8]| {
                tx.send(Done::Frame {
                    key: job.key,
                    px: job.px,
                    index,
                    image: image(job.px, rgba),
                })
                .is_ok()
            };
            // A before/after sweep draws the plain picture once; an effect
            // that turns out not to move gets one too.
            let mut sweep = split.as_ref().is_some_and(|s| s.sweep);
            let mut before: Option<Vec<u8>> = None;
            let mut swept =
                |studio: &mut Studio, split: &Split, at: Time, index: usize, fx: &[u8]| {
                    let b = before.get_or_insert_with(|| studio.render(&split.plain, at, job.px));
                    split_view(b, fx, job.px, sweep_position(index, total))
                };
            // The first two frames rendered are the loop's start and middle;
            // when they match, the effect doesn't move.
            let check = !sweep && job.frames.len() == frames.len() && frames.len() > 2;
            let mut first: Option<(usize, Vec<u8>)> = None;
            for (n, &index) in job.frames.iter().enumerate() {
                if shared.hover.load(Ordering::Relaxed) != job.hover {
                    break;
                }
                let Some(&at) = frames.get(index) else {
                    continue;
                };
                let rgba = studio.render(&project, at, job.px);
                if check && n == 0 {
                    first = Some((index, rgba.clone()));
                } else if check && n == 1 && first.as_ref().is_some_and(|(_, f)| same(f, &rgba)) {
                    let Some(split) = &split else {
                        return tx
                            .send(Done::Static {
                                key: job.key,
                                px: job.px,
                            })
                            .is_ok();
                    };
                    sweep = true;
                    // Redo the first frame as part of the sweep.
                    if let Some((i0, f0)) = first.take() {
                        let img = swept(studio, split, frames[i0], i0, &f0);
                        if !send(i0, &img) {
                            return false;
                        }
                    }
                }
                let out = match (&split, sweep) {
                    (Some(split), true) => swept(studio, split, at, index, &rgba),
                    _ => rgba,
                };
                if !send(index, &out) {
                    return false;
                }
            }
            true
        }
    }
}

/// Starts the rendering thread.
pub fn spawn(renderer: Arc<Renderer>, shared: Arc<Shared>, tx: Sender<Done>, ctx: egui::Context) {
    let spawned = std::thread::Builder::new()
        .name("library-thumbs".into())
        .spawn(move || {
            let mut studio = Studio::new(renderer.clone());
            loop {
                let job = next_job(&shared);
                let what = match &job {
                    Job::Still(k, px) | Job::Anim(AnimJob { key: k, px, .. }) => (*k, *px),
                };
                let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(&mut studio, &shared, job, &tx)
                }));
                ctx.request_repaint();
                match ok {
                    Ok(true) => {}
                    Ok(false) => return,
                    Err(_) => {
                        log::warn!("library picture of {:?} failed to render", what.0);
                        studio = Studio::new(renderer.clone());
                        if tx
                            .send(Done::Failed {
                                key: what.0,
                                px: what.1,
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                }
            }
        });
    if let Err(e) = spawned {
        log::warn!("could not start the library picture thread: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{catalog, LibraryTab};

    /// Writes contact sheets of every tile (still, then hover frames 25%
    /// and 75% through the loop) as PPM files:
    /// `TILE_SHEETS=dir cargo test -p tidycuts-app tile_sheets -- --ignored`.
    #[test]
    #[ignore]
    fn tile_sheets() {
        let Ok(dir) = std::env::var("TILE_SHEETS") else {
            return;
        };
        let renderer = tidycuts_render::gpu::headless().expect("gpu");
        let mut studio = Studio::new(renderer);
        let w: u32 = std::env::var("TILE_W")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(256);
        let px: Px = (w, w * 9 / 16);
        let only = std::env::var("TILE_ONLY").unwrap_or_default();
        let gap = 4u32;
        for tab in [
            LibraryTab::Titles,
            LibraryTab::Transitions,
            LibraryTab::Effects,
            LibraryTab::Filters,
            LibraryTab::Stickers,
            LibraryTab::Layouts,
            LibraryTab::Templates,
        ] {
            let items: Vec<_> = catalog(tab)
                .into_iter()
                .filter(|i| only.is_empty() || only.split(',').any(|o| i.name.contains(o)))
                .collect();
            if items.is_empty() {
                continue;
            }
            let cols = 3u32;
            let rows = items.len() as u32;
            let (sw, sh) = (cols * (px.0 + gap), rows * (px.1 + gap));
            let mut sheet = vec![20u8; (sw * sh * 3) as usize];
            let t0 = std::time::Instant::now();
            let mut n = 0;
            for (r, item) in items.iter().enumerate() {
                let key = Key::of(&item.drag).unwrap();
                let s = scene(key).unwrap();
                let mut moments = vec![None];
                for f in [0.25, 0.75] {
                    let i = (s.frames.len() as f64 * f) as usize;
                    if i < s.frames.len() {
                        moments.push(Some(i));
                    }
                }
                for (c, frame) in moments.into_iter().enumerate() {
                    let rgba = match frame {
                        None => render_still(&mut studio, &s, px),
                        Some(i) => {
                            let fx = studio.render(&s.project, s.frames[i], px);
                            match &s.split {
                                Some(sp) if sp.sweep => {
                                    let before = studio.render(&sp.plain, s.frames[i], px);
                                    split_view(&before, &fx, px, sweep_position(i, s.frames.len()))
                                }
                                _ => fx,
                            }
                        }
                    };
                    n += 1;
                    let (x0, y0) = (c as u32 * (px.0 + gap), r as u32 * (px.1 + gap));
                    for y in 0..px.1 {
                        for x in 0..px.0 {
                            let si = ((y * px.0 + x) * 4) as usize;
                            let di = (((y0 + y) * sw + x0 + x) * 3) as usize;
                            sheet[di..di + 3].copy_from_slice(&rgba[si..si + 3]);
                        }
                    }
                }
            }
            println!(
                "{}: {} renders, {:.1} ms each",
                tab.label(),
                n,
                t0.elapsed().as_secs_f64() * 1000.0 / n.max(1) as f64
            );
            let mut out = format!("P6\n{sw} {sh}\n255\n").into_bytes();
            out.extend_from_slice(&sheet);
            std::fs::write(format!("{dir}/{}.ppm", tab.label().to_lowercase()), out).unwrap();
        }
    }
}
