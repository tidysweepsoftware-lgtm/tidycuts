//! One-off measurements done in the background: loudness for "Normalize"
//! and automatic levels and white balance for "Auto Enhance".

use crate::decode::{decode_still, VideoParams};
use crate::levels::AudioLevels;
use crate::mixer::{measure_loudness, LoudnessTable};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use tidycuts_core::{MediaItem, MediaKind};
use tidycuts_render::{Grade, Pixels};

#[derive(Clone, Default)]
pub struct Analysis {
    pub loudness: LoudnessTable,
    /// Sound levels for audio visualizers.
    pub levels: AudioLevels,
    grades: Arc<Mutex<HashMap<PathBuf, Grade>>>,
    started: Arc<Mutex<HashSet<(PathBuf, u32, bool)>>>,
    notify: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Analysis {
    pub fn new(notify: Arc<dyn Fn() + Send + Sync>) -> Analysis {
        let levels = AudioLevels::default();
        levels.set_notify(notify.clone());
        Analysis {
            levels,
            notify: Some(notify),
            ..Default::default()
        }
    }

    /// The auto-enhance grade for a file, starting the measurement if needed.
    pub fn auto_grade(&self, media: &MediaItem) -> Option<Grade> {
        if let Some(g) = self.grades.lock().unwrap().get(&media.path) {
            return Some(g.clone());
        }
        if media.kind == MediaKind::Audio
            || !self
                .started
                .lock()
                .unwrap()
                .insert((media.path.clone(), 0, true))
        {
            return None;
        }
        let m = media.clone();
        let grades = self.grades.clone();
        let notify = self.notify.clone();
        thread::spawn(move || {
            if let Some(g) = measure_levels(&m) {
                grades.lock().unwrap().insert(m.path.clone(), g);
                if let Some(n) = notify {
                    n();
                }
            }
        });
        None
    }

    /// Starts measuring loudness for a file's audio stream if not done yet.
    pub fn want_loudness(&self, media: &MediaItem, stream: u32) {
        let key = (media.path.clone(), stream);
        if self.loudness.lock().unwrap().contains_key(&key)
            || !self
                .started
                .lock()
                .unwrap()
                .insert((media.path.clone(), stream, false))
        {
            return;
        }
        let table = self.loudness.clone();
        thread::spawn(move || {
            let gain = measure_loudness(&key.0, key.1).unwrap_or(1.0);
            table.lock().unwrap().insert(key, gain);
        });
    }
}

/// Samples a few frames and works out levels and white balance that make
/// the picture use the full range with neutral grays.
fn measure_levels(media: &MediaItem) -> Option<Grade> {
    let mut p = VideoParams::for_media(media, 160, 90);
    p.alpha = true;
    p.hwaccel = false;
    let len = media.duration.as_secs_f64();
    let times: Vec<f64> = if media.kind == MediaKind::Image {
        vec![0.0]
    } else {
        (1..=6).map(|i| len * i as f64 / 7.0).collect()
    };
    let mut lumas: Vec<f32> = Vec::new();
    let mut sum = [0f64; 3];
    let mut n = 0f64;
    for t in times {
        let Some(Pixels::Rgba(img)) = decode_still(&p, t) else {
            continue;
        };
        for px in img.pixels.as_chunks::<4>().0.iter() {
            let (r, g, b) = (
                px[0] as f32 / 255.0,
                px[1] as f32 / 255.0,
                px[2] as f32 / 255.0,
            );
            lumas.push(0.2126 * r + 0.7152 * g + 0.0722 * b);
            sum[0] += r as f64;
            sum[1] += g as f64;
            sum[2] += b as f64;
            n += 1.0;
        }
    }
    if lumas.is_empty() {
        return None;
    }
    Some(levels_grade(
        &mut lumas,
        [
            (sum[0] / n) as f32,
            (sum[1] / n) as f32,
            (sum[2] / n) as f32,
        ],
    ))
}

fn levels_grade(lumas: &mut [f32], avg: [f32; 3]) -> Grade {
    lumas.sort_by(|a, b| a.total_cmp(b));
    let pick = |q: f32| lumas[((lumas.len() - 1) as f32 * q) as usize];
    let (lo, hi) = (pick(0.01), pick(0.99));
    // grade.wgsl: black point = -0.12 * blacks, white point = 1 - 0.18 * whites.
    // Pictures that already reach (nearly) black and white are left alone.
    let blacks = if lo < 0.03 {
        0.0
    } else {
        (-lo / 0.12).clamp(-1.0, 0.0)
    };
    let whites = if hi > 0.97 {
        0.0
    } else {
        ((1.0 - hi) / 0.18).clamp(0.0, 1.0)
    };
    let [r, g, b] = avg;
    let temperature = ((b - r) / (0.35 * (r + b).max(0.01))).clamp(-0.5, 0.5);
    let tint = (((r + b) * 0.5 / g.max(0.01) - 1.0) / 0.25).clamp(-0.5, 0.5);
    Grade {
        blacks,
        whites,
        temperature,
        tint,
        vibrance: 0.15,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dull_bluish_pictures_get_stretched_and_warmed() {
        let mut l: Vec<f32> = (0..100).map(|i| 0.2 + 0.5 * i as f32 / 99.0).collect();
        let g = levels_grade(&mut l, [0.4, 0.45, 0.55]);
        assert!(g.blacks < 0.0 && g.whites > 0.0);
        assert!(g.temperature > 0.0);
        let mut l: Vec<f32> = (0..100).map(|i| i as f32 / 99.0).collect();
        let g = levels_grade(&mut l, [0.5, 0.5, 0.5]);
        assert!(g.blacks.abs() < 0.01 && g.whites.abs() < 0.01 && g.temperature.abs() < 1e-6);
    }
}
