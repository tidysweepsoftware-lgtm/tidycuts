//! Auto Reframe: where to point a tall (or square) crop of a wide picture so it
//! follows what matters.
//!
//! The picture is looked at a few times a second at a small size. Each column
//! of it gets a score from how much detail it holds (edges, which find faces,
//! people and objects) and how much it moves between looks (the subject moves,
//! the wall does not). The crop window that holds the most score wins, and the
//! path of the windows is then calmed down: it ignores small differences,
//! moves at a limited speed and never jitters, like a camera operator's pan.

use std::io::Read;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use tidycuts_core::MediaItem;
use tidycuts_media::Tools;

/// Size of the pictures looked at (columns by rows) and how many looks a second.
pub const W: usize = 192;
pub const H: usize = 108;
pub const LOOKS_PER_SEC: f64 = 4.0;

/// The score of every column of a picture: detail, plus movement against the
/// picture before it (when there is one).
fn column_scores(frame: &[u8], before: Option<&[u8]>) -> Vec<f32> {
    let mut detail = vec![0.0f32; W];
    let mut motion = vec![0.0f32; W];
    for y in 0..H - 1 {
        for x in 0..W - 1 {
            let i = y * W + x;
            let p = frame[i] as i32;
            detail[x] += ((p - frame[i + 1] as i32).abs() + (p - frame[i + W] as i32).abs()) as f32;
            if let Some(b) = before {
                motion[x] += (p - b[i] as i32).abs() as f32;
            }
        }
    }
    let norm = |v: &mut Vec<f32>| {
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        if mean > 1e-3 {
            v.iter_mut().for_each(|x| *x /= mean);
        }
    };
    norm(&mut detail);
    norm(&mut motion);
    detail
        .iter()
        .zip(&motion)
        .map(|(d, m)| d + 2.0 * m)
        .collect()
}

/// The centre (0 to 1 across the picture) of the window `width` columns wide
/// that holds the most score, with a small lean towards the middle so that a
/// picture with nothing special stays centred.
fn best_window(scores: &[f32], width: usize) -> f32 {
    let width = width.clamp(1, W);
    let mut prefix = vec![0.0f32; W + 1];
    for (i, s) in scores.iter().enumerate() {
        prefix[i + 1] = prefix[i] + s;
    }
    let mut best = (f32::MIN, W / 2);
    for left in 0..=(W - width) {
        // (a small base score per column lets the lean towards the middle break ties)
        let sum = prefix[left + width] - prefix[left] + 0.05 * width as f32;
        let centre = left as f32 + width as f32 / 2.0;
        let lean = 1.0 - 0.1 * ((centre - W as f32 / 2.0).abs() / (W as f32 / 2.0));
        let v = sum * lean;
        if v > best.0 {
            best = (v, left + width / 2);
        }
    }
    best.1 as f32 / W as f32
}

/// Calms a path of centres: the middle of five, then a dead zone and a speed limit.
fn calm(raw: &[f32]) -> Vec<f32> {
    if raw.is_empty() {
        return Vec::new();
    }
    let med: Vec<f32> = (0..raw.len())
        .map(|i| {
            let a = i.saturating_sub(2);
            let b = (i + 3).min(raw.len());
            let mut w = raw[a..b].to_vec();
            w.sort_by(|x, y| x.total_cmp(y));
            w[w.len() / 2]
        })
        .collect();
    let mut out = Vec::with_capacity(med.len());
    let mut now = med[0];
    for target in med {
        let d = target - now;
        if d.abs() > 0.05 {
            now += (d * 0.5).clamp(-0.06, 0.06);
        }
        out.push(now);
    }
    out
}

/// The path of crop centres for pictures looked at in turn. `aspect` is the
/// crop's width over its height (9/16 for a phone picture) against the
/// pictures' own height; the window covers that share of their width.
pub fn centres_from_frames(frames: &[Vec<u8>], aspect: f32, picture_aspect: f32) -> Vec<f32> {
    let window = (aspect / picture_aspect.max(0.1)).clamp(0.05, 1.0);
    let width = (W as f32 * window).round() as usize;
    let mut raw = Vec::with_capacity(frames.len());
    for (i, f) in frames.iter().enumerate() {
        let scores = column_scores(f, i.checked_sub(1).map(|j| frames[j].as_slice()));
        raw.push(best_window(&scores, width));
    }
    calm(&raw)
}

/// Looks at `len` seconds of a video from `from` and returns the crop centre
/// (0 to 1 across) for each look, as (source seconds, centre). `None` when the
/// pictures could not be read or it was stopped.
pub fn analyse(
    media: &MediaItem,
    from: f64,
    len: f64,
    aspect: f32,
    cancel: &AtomicBool,
) -> Option<Vec<(f64, f32)>> {
    let tools = Tools::get().ok()?;
    let mut cmd = tools.ffmpeg_cmd();
    tidycuts_media::below_normal(&mut cmd);
    if from > 0.0 {
        cmd.args(["-ss", &format!("{from:.3}")]);
    }
    cmd.args(["-t", &format!("{len:.3}")]);
    cmd.arg("-i").arg(media.preview_path());
    cmd.args([
        "-an",
        "-sn",
        "-vf",
        &format!("fps={LOOKS_PER_SEC},scale={W}:{H}:flags=area,format=gray"),
        "-f",
        "rawvideo",
        "-pix_fmt",
        "gray",
        "pipe:1",
    ]);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let mut child = cmd.spawn().ok()?;
    let mut out = child.stdout.take()?;
    let mut frames: Vec<Vec<u8>> = Vec::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            return None;
        }
        let mut buf = vec![0u8; W * H];
        let mut got = 0;
        while got < buf.len() {
            match out.read(&mut buf[got..]) {
                Ok(0) | Err(_) => break,
                Ok(n) => got += n,
            }
        }
        if got < buf.len() {
            break;
        }
        frames.push(buf);
    }
    let _ = child.wait();
    if frames.is_empty() {
        return None;
    }
    let picture_aspect = media.width.max(1) as f32 / media.height.max(1) as f32;
    let centres = centres_from_frames(&frames, aspect, picture_aspect);
    Some(
        centres
            .into_iter()
            .enumerate()
            .map(|(i, c)| (from + i as f64 / LOOKS_PER_SEC, c))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grey picture with a patterned block `size` columns wide starting at column `at`.
    fn picture(at: usize, size: usize, shift: u8) -> Vec<u8> {
        let mut f = vec![60u8; W * H];
        for y in 0..H {
            for x in at..(at + size).min(W) {
                f[y * W + x] = if (x + y) % 2 == 0 {
                    220
                } else {
                    90u8.saturating_add(shift)
                };
            }
        }
        f
    }

    #[test]
    fn the_crop_follows_a_subject_that_moves_across_the_picture() {
        // A subject at the left for two seconds, then it walks to the right.
        let mut frames = Vec::new();
        for _ in 0..10 {
            frames.push(picture(20, 24, 0));
        }
        for k in 0..16 {
            frames.push(picture(20 + k * 8, 24, k as u8));
        }
        for _ in 0..10 {
            frames.push(picture(148, 24, 0));
        }
        let c = centres_from_frames(&frames, 9.0 / 16.0, 16.0 / 9.0);
        assert_eq!(c.len(), frames.len());
        let (first, last) = (c[3], c[c.len() - 1]);
        assert!(first < 0.4, "starts on the left subject: {first}");
        assert!(last > 0.6, "ends on the right subject: {last}");
        // It never jumps: the path moves a little at a time.
        let jump = c
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f32::max);
        assert!(jump <= 0.061, "largest step {jump}");
    }

    #[test]
    fn a_picture_with_nothing_special_stays_centred() {
        let frames: Vec<Vec<u8>> = (0..8).map(|_| vec![100u8; W * H]).collect();
        let c = centres_from_frames(&frames, 9.0 / 16.0, 16.0 / 9.0);
        assert!(c.iter().all(|v| (v - 0.5).abs() < 0.08), "{c:?}");
    }
}
