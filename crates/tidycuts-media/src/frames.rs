use crate::ffmpeg::{secs_arg, FfmpegError, Tools};
use std::path::Path;

/// One decoded picture, 8-bit RGBA, rows packed with no padding.
#[derive(Clone)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl std::fmt::Debug for RgbaFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RgbaFrame({}x{})", self.width, self.height)
    }
}

impl RgbaFrame {
    pub fn byte_len(width: u32, height: u32) -> usize {
        width as usize * height as usize * 4
    }
}

/// Largest even-sized box with the source's aspect ratio that fits inside
/// `max_w` x `max_h`. Falls back to the box itself when the source size is
/// unknown. Even sizes keep every ffmpeg pixel format happy.
pub fn fit_within(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let even = |v: f64| ((v.round() as u32) & !1).max(2);
    if src_w == 0 || src_h == 0 {
        return (even(max_w as f64), even(max_h as f64));
    }
    let scale = (max_w as f64 / src_w as f64)
        .min(max_h as f64 / src_h as f64)
        .min(1.0);
    (even(src_w as f64 * scale), even(src_h as f64 * scale))
}

/// The `-vf` scale filter that produces exactly `w` x `h`.
pub(crate) fn scale_filter(w: u32, h: u32) -> String {
    format!("scale={w}:{h}:flags=bilinear")
}

/// Decodes the single frame showing at `at_secs`, scaled to fit inside
/// `max_w` x `max_h`. Works for videos and still images.
pub fn grab_frame(
    path: &Path,
    at_secs: f64,
    src_size: (u32, u32),
    max_w: u32,
    max_h: u32,
) -> Result<RgbaFrame, FfmpegError> {
    let tools = Tools::get()?;
    let (w, h) = fit_within(src_size.0, src_size.1, max_w, max_h);
    let mut cmd = tools.ffmpeg_cmd();
    if at_secs > 0.0 {
        cmd.args(["-ss", &secs_arg(at_secs)]);
    }
    let out = cmd
        .arg("-i")
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-an",
            "-vf",
            &scale_filter(w, h),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .output()
        .map_err(|e| FfmpegError::Spawn("ffmpeg", e))?;
    let need = RgbaFrame::byte_len(w, h);
    if out.stdout.len() < need {
        // Seeking past the last frame yields nothing; retry from slightly earlier.
        if at_secs > 0.5 && out.status.success() {
            return grab_frame(path, at_secs - 0.5, src_size, max_w, max_h);
        }
        return Err(FfmpegError::Failed {
            tool: "ffmpeg",
            message: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    let mut pixels = out.stdout;
    pixels.truncate(need);
    Ok(RgbaFrame {
        width: w,
        height: h,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_keeping_aspect_and_even_sizes() {
        assert_eq!(fit_within(1920, 1080, 960, 960), (960, 540));
        assert_eq!(fit_within(1080, 1920, 960, 540), (304, 540));
        assert_eq!(fit_within(640, 360, 960, 540), (640, 360));
        assert_eq!(fit_within(0, 0, 321, 181), (320, 180));
    }
}
