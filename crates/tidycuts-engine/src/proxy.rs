//! Proxy files: small copies of big videos that decode fast, so editing
//! stays smooth. They are intra-only (every frame stands alone), which makes
//! jumping around instant, and are used only for the preview; export always
//! reads the original.

use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use tidycuts_core::MediaItem;
use tidycuts_media::Tools;

/// The height proxies are made at (never larger than the original).
pub const PROXY_HEIGHT: u32 = 540;

/// Where the proxy of `media` goes inside `cache_dir`: named after the file
/// and when it last changed, so an edited original gets a new proxy.
pub fn proxy_path(cache_dir: &Path, media: &MediaItem) -> PathBuf {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    media.path.hash(&mut h);
    if let Ok(m) = std::fs::metadata(&media.path) {
        m.len().hash(&mut h);
        if let Ok(t) = m.modified() {
            t.hash(&mut h);
        }
    }
    cache_dir
        .join("proxies")
        .join(format!("{:016x}.mov", h.finish()))
}

/// The ffmpeg arguments that make the proxy (after `-i <original>`).
fn proxy_args(media: &MediaItem) -> Vec<String> {
    let height = media.height.clamp(2, PROXY_HEIGHT) & !1;
    vec![
        "-an".into(),
        "-vf".into(),
        format!("scale=-2:{height}:flags=bilinear,format=yuvj420p"),
        "-c:v".into(),
        "mjpeg".into(),
        "-q:v".into(),
        "5".into(),
        // Keep every frame at its own time, so the proxy lines up exactly.
        "-fps_mode".into(),
        "passthrough".into(),
    ]
}

/// Makes the proxy of `media` at `out`. `progress` gets 0..1; setting
/// `cancel` stops it and removes the partial file.
pub fn make_proxy(
    tools: &Tools,
    media: &MediaItem,
    out: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f64),
) -> Result<(), String> {
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Could not make the proxy folder: {e}"))?;
    }
    // Written under another name first, so a half-made file is never used.
    let part = out.with_extension("part.mov");
    let mut cmd = tools.ffmpeg_cmd();
    cmd.arg("-y").arg("-i").arg(&media.path);
    cmd.args(proxy_args(media));
    cmd.args(["-progress", "pipe:1", "-nostats"]).arg(&part);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    // Below normal priority (and no window): making a proxy must not slow down the
    // decoding for the picture you are looking at.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000 | 0x0000_4000);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start ffmpeg: {e}"))?;
    let total = media.duration.as_secs_f64().max(0.1);
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                break;
            }
            if let Some(us) = line.strip_prefix("out_time_us=") {
                if let Ok(us) = us.trim().parse::<f64>() {
                    progress((us / 1e6 / total).clamp(0.0, 1.0));
                }
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("ffmpeg stopped unexpectedly: {e}"))?;
    if cancel.load(Ordering::Relaxed) {
        let _ = std::fs::remove_file(&part);
        return Err("Cancelled.".into());
    }
    if !output.status.success() {
        let _ = std::fs::remove_file(&part);
        let text = String::from_utf8_lossy(&output.stderr);
        let last = text
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        return Err(format!("Could not make the proxy: {last}"));
    }
    std::fs::rename(&part, out).map_err(|e| format!("Could not save the proxy: {e}"))?;
    progress(1.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tidycuts_core::{MediaKind, Time};

    fn media(path: &Path, w: u32, h: u32) -> MediaItem {
        let mut m = MediaItem::new(
            path.to_path_buf(),
            MediaKind::Video,
            Time::from_secs_f64(2.0),
        );
        (m.width, m.height) = (w, h);
        m
    }

    #[test]
    fn big_videos_want_proxies_and_small_ones_do_not() {
        let p = Path::new("/a.mp4");
        assert!(media(p, 3840, 2160).wants_proxy());
        assert!(!media(p, 1920, 1080).wants_proxy());
        assert!(!media(p, 1280, 720).wants_proxy());
        let mut m = media(p, 3840, 2160);
        m.proxy = Some("/p.mov".into());
        assert!(!m.wants_proxy());
    }

    #[test]
    fn proxies_are_small_and_the_preview_prefers_them() {
        let args = proxy_args(&media(Path::new("/a.mp4"), 3840, 2160));
        assert!(args.iter().any(|a| a.contains("scale=-2:540")));
        let tiny = proxy_args(&media(Path::new("/a.mp4"), 640, 360));
        assert!(tiny.iter().any(|a| a.contains("scale=-2:360")));
    }

    #[test]
    fn ffmpeg_makes_a_playable_proxy() {
        let Ok(tools) = Tools::get() else {
            eprintln!("no ffmpeg here; skipping");
            return;
        };
        let dir = std::env::temp_dir().join(format!("tidycuts-proxy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("big.mp4");
        let ok = tools
            .ffmpeg_cmd()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=1920x1080:rate=25:duration=2",
            ])
            .args(["-c:v", "mpeg4"])
            .arg(&src)
            .status()
            .unwrap()
            .success();
        assert!(ok, "could not make the source");
        let m = media(&src, 1920, 1080);
        let out = proxy_path(&dir, &m);
        let mut last = 0.0;
        make_proxy(tools, &m, &out, &AtomicBool::new(false), |p| last = p).unwrap();
        assert!(out.is_file() && last >= 1.0);
        // The proxy is 540 high, 25 frames a second, two seconds long.
        let probe = tools
            .ffprobe_cmd()
            .args([
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=height,nb_frames,codec_name",
                "-of",
                "csv=p=0",
            ])
            .arg(&out)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&probe.stdout).trim().to_string();
        assert!(
            text.contains("mjpeg") && text.contains("540") && text.contains("50"),
            "{text}"
        );
        // With the proxy on, the preview reads it; the original stays the original.
        let mut with = m.clone();
        with.proxy = Some(out.clone());
        assert_eq!(with.preview_path(), out.as_path());
        assert_eq!(with.path, src);
        // A cancelled run leaves nothing behind.
        let other = dir.join("other.mov");
        let stop = AtomicBool::new(true);
        assert!(make_proxy(tools, &m, &other, &stop, |_| {}).is_err());
        assert!(!other.exists() && !other.with_extension("part.mov").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
