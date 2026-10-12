//! One-off measurements done in the background: automatic levels and white
//! balance for "Auto Enhance".

use crate::decode::{decode_still, VideoParams};
use crate::levels::AudioLevels;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use tidycuts_core::{MediaItem, MediaKind};
use tidycuts_render::{Grade, Pixels};

#[derive(Clone, Default)]
pub struct Analysis {
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
        crate::stabilize::set_notify(notify.clone());
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
}

/// Samples a few frames and works out the colour balance, brightness,
/// contrast and colour strength that make the picture look right.
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
    let mut pixels: Vec<[f32; 3]> = Vec::new();
    for t in times {
        let Some(Pixels::Rgba(img)) = decode_still(&p, t) else {
            continue;
        };
        for px in img.pixels.as_chunks::<4>().0.iter() {
            // Transparent parts of a picture are not part of what is seen.
            if px[3] < 128 {
                continue;
            }
            pixels.push([
                px[0] as f32 / 255.0,
                px[1] as f32 / 255.0,
                px[2] as f32 / 255.0,
            ]);
        }
    }
    if pixels.is_empty() {
        return None;
    }
    Some(crate::autograde::grade_from_pixels(&pixels))
}
