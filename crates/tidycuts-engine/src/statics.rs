//! Pictures TidyCuts draws itself (titles and stickers), rasterized once at
//! the size they're shown and reused until they change.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use tidycuts_core::{StickerSpec, TitleSpec};
use tidycuts_render::catalog::animations::LetterKind;
use tidycuts_render::catalog::stickers;
use tidycuts_render::{text, RgbaImage};

struct Entry {
    image: Arc<RgbaImage>,
    size: [f32; 2],
    used: u64,
}

#[derive(Default)]
pub struct Statics {
    map: Mutex<(HashMap<u64, Entry>, u64)>,
}

fn key_of(parts: impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    parts.hash(&mut h);
    h.finish()
}

impl Statics {
    pub fn get(&self, key: u64) -> Option<Arc<RgbaImage>> {
        self.map
            .lock()
            .unwrap()
            .0
            .get(&key)
            .map(|e| e.image.clone())
    }

    fn insert(&self, key: u64, image: RgbaImage, size: [f32; 2]) {
        let mut m = self.map.lock().unwrap();
        m.1 += 1;
        let now = m.1;
        m.0.insert(
            key,
            Entry {
                image: Arc::new(image),
                size,
                used: now,
            },
        );
        // Keep the cache to a few hundred pictures.
        if m.0.len() > 400 {
            let mut ages: Vec<u64> = m.0.values().map(|e| e.used).collect();
            ages.sort_unstable();
            let cutoff = ages[ages.len() - 300];
            m.0.retain(|_, e| e.used >= cutoff);
        }
    }

    fn touch(&self, key: u64) -> Option<[f32; 2]> {
        let mut m = self.map.lock().unwrap();
        m.1 += 1;
        let now = m.1;
        let e = m.0.get_mut(&key)?;
        e.used = now;
        Some(e.size)
    }

    /// A title rendered at `scale` output pixels per title pixel. Returns
    /// the cache key and the title's size in title pixels.
    pub fn title(
        &self,
        spec: &TitleSpec,
        scale: f32,
        reveal: f32,
        visible: Option<usize>,
        letters: Option<(LetterKind, f32)>,
    ) -> (u64, [f32; 2]) {
        let json = serde_json::to_string(spec).unwrap_or_default();
        let reveal_q = (reveal.clamp(0.0, 1.0) * 60.0).round() as u32;
        // Letter animations are drawn in 60 steps like the rest.
        let letters = letters.map(|(k, p)| (k, (p.clamp(0.0, 1.0) * 60.0).round() as u32));
        let key = key_of((
            "title",
            json,
            scale.to_bits(),
            reveal_q,
            visible,
            letters.map(|(k, q)| (k as u8, q)),
        ));
        if let Some(size) = self.touch(key) {
            return (key, size);
        }
        let r = text::render_title_anim(
            spec,
            scale,
            reveal_q as f32 / 60.0,
            visible,
            letters.map(|(k, q)| (k, q as f32 / 60.0)),
        );
        let size = r.size;
        self.insert(key, r.image, size);
        (key, size)
    }

    /// A sticker `height_px` output pixels tall. Animated stickers are keyed
    /// by time (at 30 fps).
    pub fn sticker(&self, spec: &StickerSpec, height_px: u32, t: f32) -> u64 {
        let animated = stickers::get(&spec.kind).is_some_and(|d| d.animated);
        let frame = if animated {
            (t * 30.0).floor() as i64
        } else {
            0
        };
        let json = serde_json::to_string(spec).unwrap_or_default();
        let key = key_of(("sticker", json, height_px, frame));
        if self.touch(key).is_some() {
            return key;
        }
        let img = stickers::render(spec, height_px, frame as f32 / 30.0);
        let size = [img.width as f32, img.height as f32];
        self.insert(key, img, size);
        key
    }
}
