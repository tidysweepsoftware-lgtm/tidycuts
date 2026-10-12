//! Camera-shake removal.
//!
//! The clip is watched once at low resolution. Between each pair of frames
//! the picture's movement (sideways, up and down, and a little turn) is
//! measured from many small blocks; blocks that disagree with the rest, such
//! as a person walking through the shot, are ignored. Adding the movements up
//! gives the camera's path. A smoothed copy of that path is what the camera
//! should have done, and each frame is moved by the difference. The picture is
//! zoomed in a little, by just enough to hide the edges the moves uncover.
//!
//! The analysis runs in the background the first time a clip's stabilization
//! is switched on and is kept until the program closes.

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use tidycuts_core::{MediaItem, Time};

/// Width the clip is analysed at.
const WIDTH: usize = 320;
/// Frames per second analysed at most.
const MAX_FPS: f64 = 30.0;

// ---- measuring movement between two frames -----------------------------------

/// A grey picture.
struct Gray {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Gray {
    /// Half the size, averaging 2x2.
    fn half(&self) -> Gray {
        let (w, h) = (self.w / 2, self.h / 2);
        let mut px = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                let i = 2 * y * self.w + 2 * x;
                let s = self.px[i] as u32
                    + self.px[i + 1] as u32
                    + self.px[i + self.w] as u32
                    + self.px[i + self.w + 1] as u32;
                px[y * w + x] = (s / 4) as u8;
            }
        }
        Gray { w, h, px }
    }
}

/// Sum of absolute differences between the `size` x `size` block of `a` at
/// (ax, ay) and of `b` at (bx, by).
fn sad(a: &Gray, ax: usize, ay: usize, b: &Gray, bx: usize, by: usize, size: usize) -> u32 {
    let mut s = 0u32;
    for row in 0..size {
        let ra = &a.px[(ay + row) * a.w + ax..][..size];
        let rb = &b.px[(by + row) * b.w + bx..][..size];
        for (p, q) in ra.iter().zip(rb) {
            s += (*p as i32 - *q as i32).unsigned_abs();
        }
    }
    s
}

/// How much a block varies (a flat block cannot be matched).
fn busyness(a: &Gray, x: usize, y: usize, size: usize) -> f32 {
    let mut sum = 0f32;
    let mut sq = 0f32;
    for row in 0..size {
        for p in &a.px[(y + row) * a.w + x..][..size] {
            let v = *p as f32;
            sum += v;
            sq += v * v;
        }
    }
    let n = (size * size) as f32;
    sq / n - (sum / n).powi(2)
}

struct Match {
    /// Whole-pixel offset of the best match.
    u: i32,
    v: i32,
    /// Sub-pixel correction, within +-0.5.
    fu: f32,
    fv: f32,
    best: u32,
    mean: f32,
}

/// Finds where the block of `prev` at (x, y) went in `cur`, looking within
/// `range` of the offset (ox, oy).
#[allow(clippy::too_many_arguments)]
fn match_block(
    prev: &Gray,
    cur: &Gray,
    x: usize,
    y: usize,
    size: usize,
    ox: i32,
    oy: i32,
    range: i32,
) -> Option<Match> {
    let w = 2 * range as usize + 1;
    let mut grid = vec![u32::MAX; w * w];
    let (mut best, mut bu, mut bv) = (u32::MAX, 0, 0);
    let (mut total, mut n) = (0f64, 0u32);
    for dv in -range..=range {
        for du in -range..=range {
            let (cx, cy) = (x as i32 + ox + du, y as i32 + oy + dv);
            if cx < 0 || cy < 0 || cx as usize + size > cur.w || cy as usize + size > cur.h {
                continue;
            }
            let s = sad(prev, x, y, cur, cx as usize, cy as usize, size);
            grid[(dv + range) as usize * w + (du + range) as usize] = s;
            total += s as f64;
            n += 1;
            if s < best {
                (best, bu, bv) = (s, ox + du, oy + dv);
            }
        }
    }
    if n < 9 || best == u32::MAX {
        return None;
    }
    let at = |du: i32, dv: i32| -> Option<f32> {
        let (a, b) = (du + range, dv + range);
        if a < 0 || b < 0 || a >= w as i32 || b >= w as i32 {
            return None;
        }
        let s = grid[b as usize * w + a as usize];
        (s != u32::MAX).then_some(s as f32)
    };
    let (lu, lv) = (bu - ox, bv - oy);
    let parabola = |m: Option<f32>, p: Option<f32>| -> f32 {
        match (m, p) {
            (Some(m), Some(p)) => {
                let c = at(lu, lv).unwrap_or(0.0);
                let d = m - 2.0 * c + p;
                if d > 1e-3 {
                    ((m - p) / (2.0 * d)).clamp(-0.5, 0.5)
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    };
    Some(Match {
        u: bu,
        v: bv,
        fu: parabola(at(lu - 1, lv), at(lu + 1, lv)),
        fv: parabola(at(lu, lv - 1), at(lu, lv + 1)),
        best,
        mean: (total / n as f64) as f32,
    })
}

/// One block's movement: where it is (from the picture's centre) and how far
/// it moved, in pixels of the analysed picture.
#[derive(Clone, Copy)]
struct Vector {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
}

/// Fits sideways, up-down and turn to the block movements, ignoring the ones
/// that do not agree with most of the others. Returns (dx, dy, turn in
/// radians) of the picture's centre, or `None` when there is too little to go
/// on.
fn fit(vectors: &[Vector]) -> Option<[f32; 3]> {
    if vectors.len() < 4 {
        return None;
    }
    let mut inlier = vec![true; vectors.len()];
    let mut model = [0f32; 3];
    for _ in 0..5 {
        let picked: Vec<&Vector> = vectors
            .iter()
            .zip(&inlier)
            .filter(|(_, i)| **i)
            .map(|(v, _)| v)
            .collect();
        let n = picked.len() as f32;
        if picked.len() < 3 {
            return None;
        }
        let mean = |f: fn(&Vector) -> f32| picked.iter().map(|v| f(v)).sum::<f32>() / n;
        let (mx, my, mdx, mdy) = (mean(|v| v.x), mean(|v| v.y), mean(|v| v.dx), mean(|v| v.dy));
        let (mut num, mut den) = (0f32, 0f32);
        for v in &picked {
            num += (v.x - mx) * (v.dy - mdy) - (v.y - my) * (v.dx - mdx);
            den += (v.x - mx).powi(2) + (v.y - my).powi(2);
        }
        let turn = if den > 1.0 { num / den } else { 0.0 };
        model = [mdx + turn * my, mdy - turn * mx, turn];
        let resid = |v: &Vector| {
            let (ex, ey) = (
                v.dx - (model[0] - model[2] * v.y),
                v.dy - (model[1] + model[2] * v.x),
            );
            (ex * ex + ey * ey).sqrt()
        };
        let mut r: Vec<f32> = picked.iter().map(|v| resid(v)).collect();
        r.sort_by(|a, b| a.total_cmp(b));
        let limit = (2.5 * r[r.len() / 2]).max(0.6);
        let next: Vec<bool> = vectors.iter().map(|v| resid(v) <= limit).collect();
        if next == inlier {
            break;
        }
        inlier = next;
    }
    let kept = inlier.iter().filter(|i| **i).count();
    (kept >= 4 && kept * 4 >= vectors.len()).then_some(model)
}

/// How far the picture moved between two frames of the same size: (dx, dy)
/// in pixels and the turn in radians (clockwise). `None` when it cannot be
/// told, for instance in a plain, empty picture.
fn estimate(prev: &Gray, cur: &Gray) -> Option<[f32; 3]> {
    let (hp, hc) = (prev.half(), cur.half());
    let mut vectors = Vec::new();
    let (bs, step_x, step_y, edge) = (12usize, 16usize, 14usize, 10usize);
    let mut y = edge;
    while y + bs + edge <= hp.h {
        let mut x = edge;
        while x + bs + edge <= hp.w {
            if busyness(&hp, x, y, bs) > 10.0 {
                if let Some(m) = match_block(&hp, &hc, x, y, bs, 0, 0, 8) {
                    // Blocks that look like many other places are not reliable.
                    if m.mean > m.best as f32 * 1.15 + 12.0 {
                        // Refine at full size around twice the half-size offset.
                        let (fx, fy) = (2 * x, 2 * y);
                        if let Some(f) = match_block(prev, cur, fx, fy, 2 * bs, 2 * m.u, 2 * m.v, 2)
                        {
                            let size = (2 * bs) as f32;
                            vectors.push(Vector {
                                x: fx as f32 + size / 2.0 - prev.w as f32 / 2.0,
                                y: fy as f32 + size / 2.0 - prev.h as f32 / 2.0,
                                dx: f.u as f32 + f.fu,
                                dy: f.v as f32 + f.fv,
                            });
                        }
                    }
                }
            }
            x += step_x;
        }
        y += step_y;
    }
    fit(&vectors)
}

// ---- the camera's path -------------------------------------------------------

/// What to do to each frame of a clip.
pub struct Smoothed {
    /// Per analysed frame: sideways and up-down as a fraction of the picture's
    /// width and height, and the turn in radians (clockwise).
    pub moves: Vec<[f32; 3]>,
    /// How much to zoom in to hide the edges.
    pub zoom: f32,
}

fn gaussian_smooth(path: &[[f32; 3]], sigma: f32) -> Vec<[f32; 3]> {
    let radius = (sigma * 3.0).ceil().max(1.0) as isize;
    let weights: Vec<f32> = (-radius..=radius)
        .map(|k| (-(k as f32).powi(2) / (2.0 * sigma * sigma)).exp())
        .collect();
    (0..path.len() as isize)
        .map(|i| {
            let mut acc = [0f32; 3];
            let mut total = 0f32;
            for (k, w) in (-radius..=radius).zip(&weights) {
                let j = i + k;
                if j < 0 || j >= path.len() as isize {
                    continue;
                }
                total += w;
                for c in 0..3 {
                    acc[c] += w * path[j as usize][c];
                }
            }
            acc.map(|v| v / total)
        })
        .collect()
}

/// Works out the moves for a clip from the measured steps.
/// `strength` is 0..100.
pub fn smooth(steps: &[[f32; 3]], fps: f64, strength: f64) -> Smoothed {
    let s = (strength / 100.0).clamp(0.0, 1.0) as f32;
    let mut path = vec![[0f32; 3]];
    for st in steps {
        let last = *path.last().unwrap();
        path.push([last[0] + st[0], last[1] + st[1], last[2] + st[2]]);
    }
    let sigma = ((0.15 + 1.35 * s) * fps as f32).max(1.0);
    let smooth = gaussian_smooth(&path, sigma);
    let margin = 0.03 + 0.05 * s;
    let turn_max = (1.5 + 2.0 * s).to_radians();
    let moves: Vec<[f32; 3]> = path
        .iter()
        .zip(&smooth)
        .map(|(p, q)| {
            [
                (q[0] - p[0]).clamp(-margin, margin),
                (q[1] - p[1]).clamp(-margin, margin),
                (q[2] - p[2]).clamp(-turn_max, turn_max),
            ]
        })
        .collect();
    let (mut shift, mut turn) = (0f32, 0f32);
    for m in &moves {
        shift = shift.max(m[0].abs()).max(m[1].abs());
        turn = turn.max(m[2].abs());
    }
    Smoothed {
        moves,
        zoom: (1.0 + 2.0 * shift + 1.2 * turn).min(1.3),
    }
}

/// What a frame needs, from the picture's own point of view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Correction {
    /// Move sideways, as a fraction of the picture's width (right is positive).
    pub dx: f32,
    /// Move down, as a fraction of the picture's height.
    pub dy: f32,
    /// Turn clockwise, in degrees.
    pub turn: f32,
    /// Zoom, 1 = none.
    pub zoom: f32,
}

impl Smoothed {
    /// The correction at `secs` seconds after the analysed part started.
    pub fn at(&self, secs: f64, fps: f64) -> Correction {
        let n = self.moves.len();
        let pos = (secs * fps).clamp(0.0, (n - 1) as f64);
        let (i, f) = (pos.floor() as usize, (pos - pos.floor()) as f32);
        let (a, b) = (self.moves[i], self.moves[(i + 1).min(n - 1)]);
        let l = |k: usize| a[k] + (b[k] - a[k]) * f;
        Correction {
            dx: l(0),
            dy: l(1),
            turn: l(2).to_degrees(),
            zoom: self.zoom,
        }
    }
}

// ---- analysing a clip in the background --------------------------------------

/// The measured steps of one part of a media file.
pub struct Measured {
    pub fps: f64,
    /// Source time of the first frame.
    pub start: f64,
    steps: Vec<[f32; 3]>,
    smoothed: Mutex<HashMap<u32, Arc<Smoothed>>>,
}

impl Measured {
    /// The measured movement between each pair of frames (sideways and
    /// up-down as a fraction of the picture, and the turn in radians).
    pub fn steps(&self) -> &[[f32; 3]] {
        &self.steps
    }

    fn smoothed(&self, strength: f64) -> Arc<Smoothed> {
        let key = strength.round().clamp(0.0, 100.0) as u32;
        self.smoothed
            .lock()
            .unwrap()
            .entry(key)
            .or_insert_with(|| Arc::new(smooth(&self.steps, self.fps, f64::from(key))))
            .clone()
    }
}

enum Slot {
    /// Progress in thousandths.
    Working(Arc<AtomicU32>),
    Ready(Arc<Measured>),
    Failed,
}

/// Where a clip's analysis stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    /// Fraction done, 0..1.
    Working(f32),
    Ready,
    Failed,
}

type Key = (PathBuf, i64, i64);

static SLOTS: LazyLock<Mutex<HashMap<Key, Slot>>> = LazyLock::new(Default::default);
static NOTIFY: OnceLock<Arc<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Called when an analysis finishes, so the picture can be redrawn.
pub fn set_notify(f: Arc<dyn Fn() + Send + Sync>) {
    let _ = NOTIFY.set(f);
}

fn key_of(media: &MediaItem, range: (Time, Time)) -> Key {
    (
        media.path.clone(),
        (range.0.as_secs_f64() * 1000.0).round() as i64,
        (range.1.as_secs_f64() * 1000.0).round() as i64,
    )
}

/// Starts the analysis of `range` of a media file if it has not been done,
/// and says how it is going.
pub fn request(media: &MediaItem, range: (Time, Time)) -> Status {
    let key = key_of(media, range);
    let mut slots = SLOTS.lock().unwrap();
    match slots.get(&key) {
        Some(Slot::Working(p)) => {
            return Status::Working(p.load(Ordering::Relaxed) as f32 / 1000.0)
        }
        Some(Slot::Ready(_)) => return Status::Ready,
        Some(Slot::Failed) => return Status::Failed,
        None => {}
    }
    let progress = Arc::new(AtomicU32::new(0));
    slots.insert(key.clone(), Slot::Working(progress.clone()));
    drop(slots);
    let (m, p) = (media.clone(), progress);
    let (a, b) = (range.0.as_secs_f64(), range.1.as_secs_f64());
    std::thread::spawn(move || {
        let result = analyse(&m, (a, b), &p);
        let slot = match result {
            Ok(m) => Slot::Ready(Arc::new(m)),
            Err(_) => Slot::Failed,
        };
        SLOTS.lock().unwrap().insert(key, slot);
        if let Some(n) = NOTIFY.get() {
            n();
        }
    });
    Status::Working(0.0)
}

/// Waits for the analysis to finish (used by export). Returns whether it
/// worked.
pub fn finish(media: &MediaItem, range: (Time, Time), cancel: &AtomicBool) -> bool {
    loop {
        match request(media, range) {
            Status::Ready => return true,
            Status::Failed => return false,
            Status::Working(_) => {
                if cancel.load(Ordering::Relaxed) {
                    return false;
                }
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
        }
    }
}

/// Puts ready-made measurements in place, for tests.
#[cfg(test)]
pub fn seed(media: &MediaItem, range: (Time, Time), fps: f64, steps: Vec<[f32; 3]>) {
    let slot = Slot::Ready(Arc::new(Measured {
        fps,
        start: range.0.as_secs_f64(),
        steps,
        smoothed: Mutex::default(),
    }));
    SLOTS.lock().unwrap().insert(key_of(media, range), slot);
}

/// The measurements for a clip, once the analysis is ready.
pub fn measured(media: &MediaItem, range: (Time, Time)) -> Option<Arc<Measured>> {
    match SLOTS.lock().unwrap().get(&key_of(media, range)) {
        Some(Slot::Ready(m)) => Some(m.clone()),
        _ => None,
    }
}

/// The correction for the picture at source time `src_t` of a clip showing
/// `range` of `media`, once the analysis is ready.
pub fn correction(
    media: &MediaItem,
    range: (Time, Time),
    strength: f64,
    src_t: f64,
) -> Option<Correction> {
    if strength <= 0.0 {
        return None;
    }
    if request(media, range) != Status::Ready {
        return None;
    }
    let measured = match SLOTS.lock().unwrap().get(&key_of(media, range)) {
        Some(Slot::Ready(m)) => m.clone(),
        _ => return None,
    };
    let smoothed = measured.smoothed(strength);
    Some(smoothed.at(src_t - measured.start, measured.fps))
}

/// Watches the clip's part with ffmpeg and measures every step.
fn analyse(media: &MediaItem, range: (f64, f64), progress: &AtomicU32) -> Result<Measured, String> {
    let tools = tidycuts_media::Tools::get().map_err(|e| e.to_string())?;
    let fps = media
        .frame_rate
        .map(|f| f.as_f64())
        .filter(|f| *f > 1.0)
        .unwrap_or(30.0)
        .min(MAX_FPS);
    let aspect = media.height.max(1) as f64 / media.width.max(1) as f64;
    let (w, h) = (
        WIDTH,
        (((WIDTH as f64 * aspect) / 2.0).round() as usize * 2).max(48),
    );
    let start = range.0.max(0.0);
    let secs = (range.1 - range.0).max(0.1);
    let mut cmd = tools.ffmpeg_cmd();
    cmd.args([
        "-nostdin",
        "-ss",
        &format!("{start:.6}"),
        "-t",
        &format!("{secs:.6}"),
    ]);
    cmd.arg("-i").arg(media.preview_path());
    cmd.args([
        "-an",
        "-sn",
        "-dn",
        "-vf",
        &format!("fps={fps:.6},scale={w}:{h}:flags=bilinear,format=gray"),
        "-f",
        "rawvideo",
        "-pix_fmt",
        "gray",
        "pipe:1",
    ]);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let mut child = crate::decode::KillOnDrop(cmd.spawn().map_err(|e| e.to_string())?);
    let mut out = child.0.stdout.take().ok_or("no pipe")?;
    let expected = (secs * fps).max(1.0);
    let mut steps: Vec<[f32; 3]> = Vec::new();
    let mut prev: Option<Gray> = None;
    let mut frames = 0u32;
    loop {
        let mut px = vec![0u8; w * h];
        if out.read_exact(&mut px).is_err() {
            break;
        }
        let cur = Gray { w, h, px };
        if let Some(p) = &prev {
            // A step that cannot be measured is taken as no movement.
            let m = estimate(p, &cur).unwrap_or([0.0; 3]);
            steps.push([m[0] / w as f32, m[1] / h as f32, m[2]]);
        }
        prev = Some(cur);
        frames += 1;
        progress.store(
            ((f64::from(frames) / expected).min(1.0) * 1000.0) as u32,
            Ordering::Relaxed,
        );
    }
    if steps.is_empty() {
        return Err("The clip has too few pictures to analyse.".into());
    }
    Ok(Measured {
        fps,
        start,
        steps,
        smoothed: Mutex::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A busy, smooth picture that can be sampled anywhere, so a moved or
    /// turned copy is exact.
    fn scene(x: f32, y: f32) -> f32 {
        let v = 128.0
            + 40.0 * (x * 0.11).sin() * (y * 0.07).cos()
            + 30.0 * ((x + 2.0 * y) * 0.05).sin()
            + 25.0 * ((x * 0.31).sin() + (y * 0.27).cos())
            + 20.0 * ((x - y) * 0.19).cos();
        v.clamp(0.0, 255.0)
    }

    /// The scene moved by (dx, dy) and turned `turn` radians clockwise about
    /// the picture's centre.
    fn frame(w: usize, h: usize, dx: f32, dy: f32, turn: f32) -> Gray {
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        let (s, c) = (-turn).sin_cos();
        let mut px = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                // Where this pixel came from.
                let (rx, ry) = (x as f32 - cx - dx, y as f32 - cy - dy);
                let (sx, sy) = (rx * c - ry * s + cx, rx * s + ry * c + cy);
                px[y * w + x] = scene(sx, sy).round() as u8;
            }
        }
        Gray { w, h, px }
    }

    #[test]
    fn sideways_and_up_down_movement_is_measured_to_a_fraction_of_a_pixel() {
        let a = frame(320, 180, 0.0, 0.0, 0.0);
        for (dx, dy) in [(3.4, -2.1), (-7.0, 5.5), (0.0, 0.0), (12.3, 9.1)] {
            let b = frame(320, 180, dx, dy, 0.0);
            let m = estimate(&a, &b).expect("measured");
            assert!((m[0] - dx).abs() < 0.3, "{dx},{dy} -> {m:?}");
            assert!((m[1] - dy).abs() < 0.3, "{dx},{dy} -> {m:?}");
            assert!(m[2].abs() < 0.003, "{dx},{dy} -> {m:?}");
        }
    }

    #[test]
    fn a_small_turn_is_measured() {
        let a = frame(320, 180, 0.0, 0.0, 0.0);
        let turn = 1.5f32.to_radians();
        let b = frame(320, 180, 1.0, -1.0, turn);
        let m = estimate(&a, &b).expect("measured");
        assert!((m[2] - turn).abs() < 0.004, "{m:?}");
        assert!(
            (m[0] - 1.0).abs() < 0.4 && (m[1] + 1.0).abs() < 0.4,
            "{m:?}"
        );
    }

    #[test]
    fn something_moving_through_the_shot_does_not_fool_it() {
        let a = frame(320, 180, 0.0, 0.0, 0.0);
        let mut b = frame(320, 180, 4.0, 2.0, 0.0);
        // A person-sized patch (about a fifth of the picture) moves the other way.
        let other = frame(320, 180, -9.0, 6.0, 0.0);
        for y in 40..150 {
            for x in 150..230 {
                b.px[y * 320 + x] = 255 - other.px[y * 320 + x];
            }
        }
        let m = estimate(&a, &b).expect("measured");
        assert!(
            (m[0] - 4.0).abs() < 0.4 && (m[1] - 2.0).abs() < 0.4,
            "{m:?}"
        );
    }

    #[test]
    fn an_empty_picture_gives_no_answer() {
        let flat = Gray {
            w: 320,
            h: 180,
            px: vec![90; 320 * 180],
        };
        assert!(estimate(&flat, &flat).is_none());
    }

    /// A deterministic jitter, quick and uneven, like a hand-held camera.
    fn jitter(i: usize) -> f32 {
        let t = i as f32;
        0.012 * (t * 1.7).sin() + 0.008 * (t * 2.9 + 1.0).sin() + 0.005 * (t * 0.9 + 2.0).cos()
    }

    #[test]
    fn shake_is_taken_out_and_a_steady_pan_is_left() {
        let n = 240;
        // Steps of a camera that drifts steadily right while shaking.
        let pan = 0.002f32;
        let position = |i: usize| pan * i as f32 + jitter(i);
        let steps: Vec<[f32; 3]> = (1..=n)
            .map(|i| [position(i) - position(i - 1), 0.0, 0.0])
            .collect();
        let s = smooth(&steps, 30.0, 80.0);
        // The stabilised path is the real path plus the correction.
        let stabilised: Vec<f32> = (0..=n).map(|i| position(i) + s.moves[i][0]).collect();
        let high_pass = |p: &dyn Fn(usize) -> f32| -> f32 {
            // How much each frame differs from the average of its neighbours.
            (10..n - 10)
                .map(|i| (p(i) - 0.5 * (p(i - 1) + p(i + 1))).powi(2))
                .sum::<f32>()
                / (n - 20) as f32
        };
        let before = high_pass(&position);
        let after = high_pass(&|i| stabilised[i]);
        assert!(after < before * 0.03, "{before} -> {after}");
        // The pan itself is still there: the picture ends up where the camera
        // pointed, give or take the margin.
        assert!((stabilised[n - 20] - position(n - 20)).abs() <= 0.08 + 1e-4);
        assert!(s.zoom >= 1.0 && s.zoom <= 1.3);
    }

    #[test]
    fn a_steady_clip_is_not_moved_or_zoomed() {
        let steps = vec![[0.0f32; 3]; 100];
        let s = smooth(&steps, 30.0, 100.0);
        assert!(s.moves.iter().all(|m| m.iter().all(|v| v.abs() < 1e-6)));
        assert!((s.zoom - 1.0).abs() < 1e-6);
    }

    #[test]
    fn higher_strength_smooths_more_and_the_lookup_interpolates() {
        let steps: Vec<[f32; 3]> = (1..=200)
            .map(|i| [jitter(i) - jitter(i - 1), 0.0, 0.0])
            .collect();
        let size = |strength: f64| -> f32 {
            smooth(&steps, 30.0, strength)
                .moves
                .iter()
                .map(|m| m[0].abs())
                .sum::<f32>()
        };
        assert!(size(100.0) > size(10.0));
        let s = smooth(&steps, 30.0, 50.0);
        let c = s.at(10.5 / 30.0, 30.0);
        let (a, b) = (s.moves[10][0], s.moves[11][0]);
        assert!((c.dx - 0.5 * (a + b)).abs() < 1e-6);
        // Past the end keeps the last value.
        assert_eq!(s.at(1000.0, 30.0).dx, s.moves[s.moves.len() - 1][0]);
    }
}
