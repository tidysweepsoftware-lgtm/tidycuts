//! Picture sizes, frame rates and the small formatting helpers the dialogs
//! share. Everything here is plain logic, so it is unit tested.

use std::path::{Path, PathBuf};
use tidycuts_core::FrameRate;

/// A named project size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizePreset {
    /// Long name, e.g. "Full HD 1080p".
    pub name: &'static str,
    /// Tile label, e.g. "1080p".
    pub short: &'static str,
    pub width: u32,
    pub height: u32,
}

const fn preset(name: &'static str, short: &'static str, width: u32, height: u32) -> SizePreset {
    SizePreset {
        name,
        short,
        width,
        height,
    }
}

/// Project sizes offered in Project Settings, with the usual aspect ratios.
pub const PROJECT_SIZES: [SizePreset; 8] = [
    preset("4K UHD", "4K", 3840, 2160),
    preset("2K QHD", "2K", 2560, 1440),
    preset("Full HD", "1080p", 1920, 1080),
    preset("HD", "720p", 1280, 720),
    preset("Vertical (phone)", "9:16", 1080, 1920),
    preset("Square", "1:1", 1080, 1080),
    preset("Portrait", "4:5", 1080, 1350),
    preset("Cinema widescreen", "21:9", 2560, 1080),
];

/// The preset with exactly this size.
pub fn preset_for(width: u32, height: u32) -> Option<usize> {
    PROJECT_SIZES
        .iter()
        .position(|p| p.width == width && p.height == height)
}

/// Frame rates offered for projects and exports.
pub const FRAME_RATES: [(FrameRate, &str); 10] = [
    (FrameRate::new(24000, 1001), "23.976 fps (film)"),
    (FrameRate::new(24, 1), "24 fps (cinema)"),
    (FrameRate::new(25, 1), "25 fps (PAL)"),
    (FrameRate::new(30000, 1001), "29.97 fps (NTSC)"),
    (FrameRate::new(30, 1), "30 fps"),
    (FrameRate::new(48, 1), "48 fps"),
    (FrameRate::new(50, 1), "50 fps"),
    (FrameRate::new(60000, 1001), "59.94 fps"),
    (FrameRate::new(60, 1), "60 fps"),
    (FrameRate::new(120, 1), "120 fps"),
];

/// Frame rates that suit animated GIFs.
pub const GIF_RATES: [(FrameRate, &str); 5] = [
    (FrameRate::new(10, 1), "10 fps"),
    (FrameRate::new(12, 1), "12 fps"),
    (FrameRate::new(15, 1), "15 fps"),
    (FrameRate::new(20, 1), "20 fps"),
    (FrameRate::new(25, 1), "25 fps"),
];

/// Whether two frame rates are the same speed (25/1 and 50/2 are).
pub fn same_rate(a: FrameRate, b: FrameRate) -> bool {
    (a.as_f64() - b.as_f64()).abs() < 0.001
}

/// A short label for a frame rate, e.g. "29.97 fps" or "25 fps".
pub fn rate_label(r: FrameRate) -> String {
    let v = r.as_f64();
    if (v - v.round()).abs() < 0.005 {
        format!("{} fps", v.round() as i64)
    } else {
        let t = format!("{v:.3}");
        format!("{} fps", t.trim_end_matches('0'))
    }
}

/// The everyday name of a picture size by its shorter side, like "4K" or
/// "1080p".
pub fn size_name(width: u32, height: u32) -> Option<&'static str> {
    match width.min(height) {
        4320 => Some("8K"),
        2160 => Some("4K"),
        1440 => Some("2K"),
        1080 => Some("1080p"),
        720 => Some("720p"),
        540 => Some("540p"),
        480 => Some("480p"),
        360 => Some("360p"),
        _ => None,
    }
}

/// "3840×2160 · 4K" or just "1000×700" for unnamed sizes.
pub fn size_label(width: u32, height: u32) -> String {
    match size_name(width, height) {
        Some(n) => format!("{width}×{height} · {n}"),
        None => format!("{width}×{height}"),
    }
}

fn even(v: u32) -> u32 {
    v.max(2) & !1
}

/// The same shape scaled so its shorter side is `short` pixels (even sizes,
/// as video encoders need).
pub fn scale_to_short(width: u32, height: u32, short: u32) -> (u32, u32) {
    let s = width.min(height).max(1) as f64;
    let k = short as f64 / s;
    (
        even((width as f64 * k).round() as u32),
        even((height as f64 * k).round() as u32),
    )
}

/// The size a video of `width`×`height` gets when brought down to `short`
/// lines (1080 for "1080p"), keeping its shape: 3840×2160 becomes
/// 1920×1080 and a vertical 2160×3840 becomes 1080×1920. Never enlarges.
pub fn downscale(width: u32, height: u32, short: u32) -> (u32, u32) {
    if width.min(height) <= short {
        (even(width), even(height))
    } else {
        scale_to_short(width, height, short)
    }
}

/// Sizes to offer when exporting a project of `width`×`height`: the
/// project's own size first, then common sizes of the same shape.
pub fn export_sizes(width: u32, height: u32) -> Vec<(u32, u32)> {
    let mut v = vec![(width, height)];
    for short in [2160, 1440, 1080, 720, 540, 480, 360] {
        let s = scale_to_short(width, height, short);
        if !v.contains(&s) {
            v.push(s);
        }
    }
    v
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// An aspect ratio label such as "16:9", "9:16", "21:9" or "1.85:1".
pub fn aspect_name(width: u32, height: u32) -> String {
    if width == 0 || height == 0 {
        return "—".into();
    }
    let r = width as f64 / height as f64;
    const KNOWN: [(u32, u32); 12] = [
        (16, 9),
        (9, 16),
        (4, 3),
        (3, 4),
        (1, 1),
        (4, 5),
        (5, 4),
        (3, 2),
        (2, 3),
        (21, 9),
        (9, 21),
        (2, 1),
    ];
    for (a, b) in KNOWN {
        let k = a as f64 / b as f64;
        if ((r - k) / k).abs() < 0.02 {
            return format!("{a}:{b}");
        }
    }
    let g = gcd(width, height).max(1);
    let (a, b) = (width / g, height / g);
    if a <= 32 && b <= 32 {
        format!("{a}:{b}")
    } else if r >= 1.0 {
        format!("{r:.2}:1")
    } else {
        format!("1:{:.2}", 1.0 / r)
    }
}

/// File sizes like "640 KB", "12.4 MB" or "1.25 GB".
pub fn fmt_bytes(bytes: u64) -> String {
    let b = bytes as f64;
    if b < 1e6 {
        format!("{:.0} KB", (b / 1e3).max(1.0))
    } else if b < 1e9 {
        format!("{:.1} MB", b / 1e6)
    } else {
        format!("{:.2} GB", b / 1e9)
    }
}

/// A duration like "0:42", "3:05" or "1:02:03".
pub fn fmt_clock(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// A timeline position with hundredths, like "0:05.40" or "1:02:03.00".
pub fn fmt_position(secs: f64) -> String {
    let cs = (secs.max(0.0) * 100.0).round() as u64;
    let (whole, frac) = (cs / 100, cs % 100);
    if whole >= 3600 {
        format!(
            "{}:{:02}:{:02}.{frac:02}",
            whole / 3600,
            (whole / 60) % 60,
            whole % 60
        )
    } else {
        format!("{}:{:02}.{frac:02}", whole / 60, whole % 60)
    }
}

/// Reads "1:02:03.5", "2:30", "75" or "12.5" as seconds.
pub fn parse_position(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.trim().split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut total = 0.0;
    for (i, p) in parts.iter().enumerate() {
        let v: f64 = p.trim().parse().ok()?;
        if v < 0.0 || (i + 1 < parts.len() && v.fract() != 0.0) {
            return None;
        }
        total = total * 60.0 + v;
    }
    Some(total)
}

/// Makes a name safe to use as a file name on every system.
pub fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "Untitled".into()
    } else {
        trimmed.to_string()
    }
}

/// `folder/name.ext`, or `folder/name (2).ext` and so on when taken.
pub fn unique_path(folder: &Path, name: &str, ext: &str) -> PathBuf {
    let first = folder.join(format!("{name}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..1000)
        .map(|n| folder.join(format!("{name} ({n}).{ext}")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// The name part of a path chosen by `unique_path`.
pub fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_found_by_size() {
        assert_eq!(preset_for(1920, 1080), Some(2));
        assert_eq!(preset_for(1080, 1920), Some(4));
        assert_eq!(preset_for(1000, 1000), None);
        for p in PROJECT_SIZES {
            assert_eq!(p.width % 2, 0);
            assert_eq!(p.height % 2, 0);
            assert!(!aspect_name(p.width, p.height).is_empty());
        }
    }

    #[test]
    fn downscaling_keeps_the_shape() {
        assert_eq!(downscale(3840, 2160, 1080), (1920, 1080));
        assert_eq!(downscale(2160, 3840, 1080), (1080, 1920));
        assert_eq!(downscale(4096, 2160, 1080), (2048, 1080));
        assert_eq!(downscale(2880, 2160, 1080), (1440, 1080));
        assert_eq!(downscale(5120, 2160, 1080), (2560, 1080));
        // Smaller videos are never enlarged.
        assert_eq!(downscale(1280, 720, 1080), (1280, 720));
        // Sizes stay even for encoders.
        let (w, h) = downscale(3840, 1644, 1080);
        assert_eq!((w % 2, h % 2), (0, 0));
        assert_eq!(h, 1080);
    }

    #[test]
    fn export_sizes_follow_the_project_shape() {
        let s = export_sizes(1920, 1080);
        assert_eq!(s[0], (1920, 1080));
        assert!(s.contains(&(3840, 2160)));
        assert!(s.contains(&(1280, 720)));
        assert_eq!(s.iter().filter(|x| **x == (1920, 1080)).count(), 1);
        let v = export_sizes(1080, 1920);
        assert!(v.contains(&(720, 1280)));
        assert!(v.contains(&(2160, 3840)));
    }

    #[test]
    fn sizes_and_aspects_have_friendly_names() {
        assert_eq!(size_name(3840, 2160), Some("4K"));
        assert_eq!(size_name(1080, 1920), Some("1080p"));
        assert_eq!(size_name(1000, 700), None);
        assert_eq!(size_label(3840, 2160), "3840×2160 · 4K");
        assert_eq!(aspect_name(1920, 1080), "16:9");
        assert_eq!(aspect_name(1080, 1920), "9:16");
        assert_eq!(aspect_name(1080, 1350), "4:5");
        assert_eq!(aspect_name(2560, 1080), "21:9");
        assert_eq!(aspect_name(1000, 1000), "1:1");
        assert_eq!(aspect_name(1998, 1080), "1.85:1");
    }

    #[test]
    fn frame_rates_compare_by_speed() {
        assert!(same_rate(FrameRate::new(50, 2), FrameRate::new(25, 1)));
        assert!(!same_rate(
            FrameRate::new(30000, 1001),
            FrameRate::new(30, 1)
        ));
        assert_eq!(rate_label(FrameRate::new(25, 1)), "25 fps");
        assert_eq!(rate_label(FrameRate::new(30000, 1001)), "29.97 fps");
        assert_eq!(rate_label(FrameRate::new(24000, 1001)), "23.976 fps");
    }

    #[test]
    fn positions_round_trip() {
        assert_eq!(fmt_position(5.4), "0:05.40");
        assert_eq!(fmt_position(3723.0), "1:02:03.00");
        assert_eq!(parse_position("0:05.40"), Some(5.4));
        assert_eq!(parse_position("1:02:03"), Some(3723.0));
        assert_eq!(parse_position("75"), Some(75.0));
        assert_eq!(parse_position("1.5:20"), None);
        assert_eq!(parse_position("abc"), None);
        assert_eq!(fmt_clock(65.0), "1:05");
        assert_eq!(fmt_clock(3725.0), "1:02:05");
    }

    #[test]
    fn bytes_and_names_format_nicely() {
        assert_eq!(fmt_bytes(640_000), "640 KB");
        assert_eq!(fmt_bytes(12_400_000), "12.4 MB");
        assert_eq!(fmt_bytes(1_250_000_000), "1.25 GB");
        assert_eq!(sanitize_file_name("My: video?"), "My_ video_");
        assert_eq!(sanitize_file_name("  ..  "), "Untitled");
    }

    #[test]
    fn unique_paths_skip_taken_names() {
        let dir = std::env::temp_dir().join(format!("tidycuts-unique-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(unique_path(&dir, "a", "mp4"), dir.join("a.mp4"));
        std::fs::write(dir.join("a.mp4"), b"x").unwrap();
        assert_eq!(unique_path(&dir, "a", "mp4"), dir.join("a (2).mp4"));
        assert_eq!(stem_of(&dir.join("a (2).mp4")), "a (2)");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
