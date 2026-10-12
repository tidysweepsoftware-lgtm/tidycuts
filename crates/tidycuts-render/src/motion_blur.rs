//! Motion blur for assets that are drawn one frame at a time (stickers,
//! titles): draw the frame at several moments inside the camera's "shutter"
//! time and average them. Fast things smear along their path, slow things
//! stay sharp, like a real exposure.
//!
//! (Footage and the GPU effects have their own directional, zoom and spin
//! blurs; this is for drawn assets that have none.)

use crate::RgbaImage;

/// The moments to draw: `samples` of them spread evenly over the shutter,
/// ending at `t`, none earlier than `floor`. The last one is always `t`
/// itself, so the sharpest copy is the current frame.
pub fn sample_times(t: f32, shutter: f32, samples: u32, floor: f32) -> Vec<f32> {
    let n = samples.max(1);
    if n == 1 || shutter <= 0.0 {
        return vec![t];
    }
    (0..n)
        .map(|k| {
            let back = shutter * (n - 1 - k) as f32 / (n - 1) as f32;
            (t - back).max(floor.min(t))
        })
        .collect()
}

/// The average of premultiplied RGBA8 frames of the same size.
pub fn average(frames: &[Vec<u8>]) -> Vec<u8> {
    let Some(first) = frames.first() else {
        return Vec::new();
    };
    if frames.len() == 1 {
        return first.clone();
    }
    let n = frames.len() as u32;
    (0..first.len())
        .map(|i| {
            let sum: u32 = frames.iter().map(|f| f[i] as u32).sum();
            ((sum + n / 2) / n) as u8
        })
        .collect()
}

/// Draws with `render` at every sample time and returns the average.
/// `floor` is the earliest time to look back to: pass the time the motion
/// restarted, so a cut is not smeared into the frame before it.
pub fn accumulate(
    t: f32,
    shutter: f32,
    samples: u32,
    floor: f32,
    mut render: impl FnMut(f32) -> RgbaImage,
) -> RgbaImage {
    let times = sample_times(t, shutter, samples, floor);
    let mut frames: Vec<RgbaImage> = times.iter().map(|&s| render(s)).collect();
    if frames.len() == 1 {
        return frames.remove(0);
    }
    let mut out = frames.remove(0);
    let mut all: Vec<Vec<u8>> = vec![std::mem::take(&mut out.pixels)];
    all.extend(frames.into_iter().map(|f| f.pixels));
    out.pixels = average(&all);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square_at(x: u32) -> RgbaImage {
        let (w, h) = (40u32, 8u32);
        let mut img = RgbaImage::new(w, h);
        img.premultiplied = true;
        for y in 0..h {
            for dx in 0..4 {
                let i = ((y * w + x + dx) * 4) as usize;
                img.pixels[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        img
    }

    fn lit(img: &RgbaImage) -> usize {
        img.pixels.chunks(4).filter(|p| p[3] > 0).count()
    }

    #[test]
    fn sample_times_cover_the_shutter_and_end_on_now() {
        let t = sample_times(1.0, 0.1, 5, 0.0);
        assert_eq!(t.len(), 5);
        assert!((t[0] - 0.9).abs() < 1e-6 && (t[4] - 1.0).abs() < 1e-6);
        assert!(t.windows(2).all(|w| w[1] > w[0]));
        assert_eq!(
            sample_times(1.0, 0.0, 5, 0.0),
            vec![1.0],
            "no shutter, no blur"
        );
        assert_eq!(sample_times(1.0, 0.1, 1, 0.0), vec![1.0]);
    }

    #[test]
    fn it_never_looks_back_past_the_floor() {
        // Just after a cut at t = 2.0, nothing from before 2.0 is used.
        let t = sample_times(2.01, 0.1, 5, 2.0);
        assert!(t.iter().all(|&s| s >= 2.0 - 1e-6), "{t:?}");
        assert_eq!(*t.last().unwrap(), 2.01);
        // A floor in the future never pushes the sample past now.
        assert!(sample_times(1.0, 0.1, 3, 5.0).iter().all(|&s| s <= 1.0));
    }

    #[test]
    fn averaging_blends_and_keeps_identical_frames() {
        let a = vec![0u8, 100, 200, 255];
        let b = vec![100u8, 100, 0, 255];
        assert_eq!(average(&[a.clone(), b]), vec![50, 100, 100, 255]);
        assert_eq!(average(&[a.clone(), a.clone(), a.clone()]), a);
        assert!(average(&[]).is_empty());
    }

    #[test]
    fn a_moving_square_smears_and_a_still_one_does_not() {
        // The square moves 20 px per second.
        let moving = |t: f32| square_at((t * 20.0).round() as u32);
        let sharp = moving(1.0);
        let blurred = accumulate(1.0, 0.5, 6, 0.0, moving);
        assert!(
            lit(&blurred) > lit(&sharp),
            "{} vs {}",
            lit(&blurred),
            lit(&sharp)
        );
        let peak = blurred.pixels.chunks(4).map(|p| p[3]).max().unwrap();
        assert!(
            peak > 0 && blurred.pixels.chunks(4).any(|p| p[3] > 0 && p[3] < 255),
            "soft edges"
        );
        // Something that never moves comes out exactly the same.
        let still = accumulate(1.0, 0.5, 6, 0.0, |_| square_at(10));
        assert_eq!(still.pixels, square_at(10).pixels);
        // No shutter: identical to a single draw.
        let none = accumulate(1.0, 0.0, 6, 0.0, moving);
        assert_eq!(none.pixels, sharp.pixels);
    }
}
