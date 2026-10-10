//! Single pictures of the timeline: one frame drawn at a chosen size and
//! saved as a PNG (the Snapshot button), or returned as PNG bytes (what
//! Claude sees when it looks at the project).

use crate::frames::StreamFrames;
use crate::levels::AudioLevels;
use crate::plan::Planner;
use crate::statics::Statics;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tidycuts_core::{MediaItem, Project, Time};
use tidycuts_render::{Compositor, Grade, Renderer};

/// A frame as 8-bit RGBA pixels.
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Draws the project at `at` exactly as export would, but `width` pixels
/// wide (the height follows the project's shape). `None` uses the
/// project's own size.
pub fn render(
    project: &Project,
    at: Time,
    width: Option<u32>,
    renderer: Arc<Renderer>,
    statics: &Statics,
    auto_grade: &dyn Fn(&MediaItem) -> Option<Grade>,
) -> Picture {
    let (pw, ph) = (
        project.settings.width.max(2) as f64,
        project.settings.height.max(2) as f64,
    );
    let scale = width.map_or(1.0, |w| w.max(16) as f64 / pw);
    let (w, h) = (
        ((pw * scale).round() as u32).max(2) & !1,
        ((ph * scale).round() as u32).max(2) & !1,
    );
    let mut comp = Compositor::new(renderer);
    let target = comp.make_target(w, h, wgpu::TextureFormat::Rgba8Unorm, true);
    let mut frames = StreamFrames::new(Duration::from_secs(20), true, true);
    // Visualizers wait for their sound to decode rather than show silence.
    let levels = AudioLevels::default();
    levels.set_blocking(true);
    let mut planner = Planner {
        project,
        out_scale: scale as f32,
        frames: &mut frames,
        statics,
        auto_grade,
        levels: &levels,
        show_empty_cells: false,
        used_statics: Vec::new(),
    };
    let f = planner.build(at);
    frames.end_frame();
    for (k, img) in &f.statics {
        if !comp.has_static(*k) {
            comp.store_static(*k, img);
        }
    }
    let rgba = comp.render_to_cpu(&f.frame, &target);
    Picture {
        width: w,
        height: h,
        rgba,
    }
}

/// Draws the project at `at` at its own size and writes it to `path` as a PNG.
pub fn snapshot(
    project: &Project,
    at: Time,
    renderer: Arc<Renderer>,
    statics: &Statics,
    auto_grade: &dyn Fn(&MediaItem) -> Option<Grade>,
    path: &Path,
) -> Result<(), String> {
    let pic = render(project, at, None, renderer, statics, auto_grade);
    write_png(path, pic.width, pic.height, &pic.rgba)
}

/// Writes 8-bit RGBA pixels as a PNG, making the folder if needed.
pub fn write_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = std::fs::File::create(path)
        .map_err(|e| format!("Could not create {}: {e}", path.display()))?;
    encode_png(std::io::BufWriter::new(file), w, h, rgba)
}

/// The PNG file for 8-bit RGBA pixels, in memory.
pub fn png_bytes(w: u32, h: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    encode_png(&mut out, w, h, rgba)?;
    Ok(out)
}

fn encode_png<W: Write>(out: W, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let mut enc = png::Encoder::new(out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(rgba)
        .map_err(|e| format!("Could not write the picture: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_bytes_start_with_the_png_signature_and_hold_the_size() {
        let px = vec![255u8; 4 * 3 * 2];
        let b = png_bytes(3, 2, &px).unwrap();
        assert_eq!(&b[..8], b"\x89PNG\r\n\x1a\n");
        // IHDR: width and height right after the signature and chunk header.
        assert_eq!(u32::from_be_bytes(b[16..20].try_into().unwrap()), 3);
        assert_eq!(u32::from_be_bytes(b[20..24].try_into().unwrap()), 2);
    }
}
