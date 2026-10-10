//! End-to-end checks against the real ffmpeg binaries. Skipped (with a
//! message) when ffmpeg isn't installed.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use tidycuts_core::{FrameRate, MediaKind, Time};
use tidycuts_media::{grab_frame, probe, Player, Segment, SegmentKind, Tools};

fn tools_or_skip() -> Option<&'static Tools> {
    match Tools::get() {
        Ok(t) => Some(t),
        Err(e) => {
            eprintln!("skipping: {e}");
            None
        }
    }
}

fn make_clip(dir: &Path) -> PathBuf {
    let out = dir.join("test.mp4");
    let status = Command::new(&Tools::get().unwrap().ffmpeg)
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=320x240:rate=25:duration=2",
        ])
        .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=2"])
        .args([
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());
    out
}

fn make_png(dir: &Path) -> PathBuf {
    let out = dir.join("still.png");
    let status = Command::new(&Tools::get().unwrap().ffmpeg)
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:size=64x48",
            "-frames:v",
            "1",
        ])
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());
    out
}

fn temp_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("tidycuts-media-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn probes_and_grabs_frames() {
    if tools_or_skip().is_none() {
        return;
    }
    let dir = temp_dir();
    let video = make_clip(&dir);
    let m = probe(&video).unwrap();
    assert_eq!(m.kind, MediaKind::Video);
    assert!(m.has_audio);
    assert_eq!((m.width, m.height), (320, 240));
    assert_eq!(m.frame_rate, Some(FrameRate::new(25, 1)));
    assert!((m.duration.as_secs_f64() - 2.0).abs() < 0.1);

    let f = grab_frame(&video, 1.0, (m.width, m.height), 160, 160).unwrap();
    assert_eq!((f.width, f.height), (160, 120));
    assert_eq!(f.pixels.len(), 160 * 120 * 4);
    // Asking for a frame past the end falls back to the last one.
    assert!(grab_frame(&video, 30.0, (m.width, m.height), 160, 160).is_ok());

    let png = make_png(&dir);
    let still = probe(&png).unwrap();
    assert_eq!(still.kind, MediaKind::Image);
    let f = grab_frame(&png, 0.0, (still.width, still.height), 100, 100).unwrap();
    assert_eq!((f.width, f.height), (64, 48));
    let px = &f.pixels[..3];
    assert!(
        px[0] > 240 && px[1] < 16 && px[2] < 16,
        "expected red, got {px:?}"
    );
}

#[test]
fn player_runs_through_a_sequence() {
    if tools_or_skip().is_none() {
        return;
    }
    let dir = temp_dir();
    let video = make_clip(&dir);
    let png = make_png(&dir);
    let seg = |path: &PathBuf, kind, start: f64, dur: f64, size| Segment {
        path: path.clone(),
        kind,
        start: Time::from_secs_f64(start),
        source_in: Time::ZERO,
        duration: Time::from_secs_f64(dur),
        size,
        has_audio: kind == SegmentKind::Video,
    };
    let segs = vec![
        seg(&video, SegmentKind::Video, 0.0, 1.0, (320, 240)),
        seg(&png, SegmentKind::Image, 1.0, 0.5, (64, 48)),
    ];
    let mut p = Player::new();
    p.set_max_size(160, 120);
    p.load(
        segs.clone(),
        segs,
        Time::from_secs_f64(1.5),
        FrameRate::new(25, 1),
    );
    p.play();
    let started = Instant::now();
    let mut changes = 0;
    while p.is_playing() && started.elapsed() < Duration::from_secs(10) {
        if p.tick() {
            changes += 1;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!p.is_playing(), "playback should stop at the end");
    assert_eq!(p.position(), Time::from_secs_f64(1.5));
    assert!(changes >= 10, "expected many frames, got {changes}");
}
