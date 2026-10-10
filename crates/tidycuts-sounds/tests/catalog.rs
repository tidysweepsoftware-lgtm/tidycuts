use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

use tidycuts_sounds::{catalog, categories, ensure_file, get, render, SoundKind, VERSION};

const SR: u32 = 48_000;

#[test]
fn catalog_is_consistent() {
    let mut ids = HashSet::new();
    for def in catalog() {
        assert!(ids.insert(def.id), "duplicate id {}", def.id);
        assert!(
            def.id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "id {} should be snake_case",
            def.id
        );
        assert_eq!(get(def.id), Some(def));
        assert!(!def.name.is_empty() && !def.category.is_empty());
        match def.kind {
            SoundKind::Music => assert!((30.0..=60.0).contains(&def.secs), "{}", def.id),
            SoundKind::Effect if def.category == "Ambience" => {
                assert!((8.0..=12.0).contains(&def.secs), "{}", def.id)
            }
            SoundKind::Effect => assert!((0.1..=6.0).contains(&def.secs), "{}", def.id),
        }
    }
    let effects = catalog()
        .iter()
        .filter(|d| d.kind == SoundKind::Effect)
        .count();
    let music = catalog()
        .iter()
        .filter(|d| d.kind == SoundKind::Music)
        .count();
    assert!((45..=62).contains(&effects), "{effects} effects");
    assert!((6..=10).contains(&music), "{music} music beds");

    let fx = categories(SoundKind::Effect);
    assert_eq!(fx.first(), Some(&"Whoosh & Transitions"));
    assert!(fx.contains(&"Ambience") && fx.contains(&"Drums"));
    let mu = categories(SoundKind::Music);
    for c in ["Chill", "Upbeat", "Cinematic", "Ambient"] {
        assert!(mu.contains(&c), "missing music category {c}");
    }
    assert!(fx.iter().all(|c| !mu.contains(c)));
    assert!(get("no_such_sound").is_none());
    assert!(render("no_such_sound", SR).is_none());
}

fn check(id: &str, v: &[f32], secs: f32, sr: u32) {
    let frames = (secs * sr as f32).round() as usize;
    assert_eq!(v.len(), frames * 2, "{id}: wrong length");
    assert!(v.iter().all(|x| x.is_finite()), "{id}: NaN or inf");
    let peak = v.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(peak <= 0.9, "{id}: peak {peak}");
    let rms = (v.iter().map(|x| (x * x) as f64).sum::<f64>() / v.len() as f64).sqrt();
    // not silent; room tone is deliberately quiet (-42 LUFS), so the bar is low
    assert!(
        peak > 0.01 && rms > 1e-3,
        "{id}: too quiet (peak {peak}, rms {rms})"
    );
    // edges: the first and last half millisecond are (nearly) silent
    let edge = (sr as usize / 2000) * 2;
    let head = v[..edge].iter().fold(0.0f32, |m, x| m.max(x.abs()));
    let tail = v[v.len() - edge..]
        .iter()
        .fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(head < 0.02, "{id}: starts with a click ({head})");
    assert!(tail < 0.02, "{id}: ends with a click ({tail})");
    assert!(v[0].abs() < 1e-3 && v[1].abs() < 1e-3, "{id}: first sample");
    assert!(
        v[v.len() - 1].abs() < 1e-3 && v[v.len() - 2].abs() < 1e-3,
        "{id}: last sample"
    );
}

#[test]
fn every_sound_renders_cleanly() {
    let defs = catalog();
    // render in a few threads; the catalog is large
    let chunks: Vec<_> = defs.chunks(defs.len().div_ceil(4)).collect();
    std::thread::scope(|s| {
        for chunk in chunks {
            s.spawn(move || {
                for def in chunk {
                    let v = render(def.id, SR).unwrap();
                    check(def.id, &v, def.secs, SR);
                }
            });
        }
    });
}

#[test]
fn rendering_is_deterministic() {
    for id in [
        "soft_whoosh",
        "keyboard_typing",
        "rain",
        "glitch",
        "lofi_chill",
    ] {
        let a = render(id, SR).unwrap();
        let b = render(id, SR).unwrap();
        assert!(a == b, "{id} differs between renders");
    }
}

#[test]
fn other_sample_rates() {
    for (id, sr) in [("pop", 44_100), ("riser", 22_050), ("chime", 96_000)] {
        let def = get(id).unwrap();
        let v = render(id, sr).unwrap();
        check(id, &v, def.secs, sr);
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tidycuts-sounds-test-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn ensure_file_writes_and_reuses_a_wav() {
    let dir = temp_dir("wav");
    let path = ensure_file("coin", &dir).unwrap();
    assert_eq!(path, dir.join(format!("coin-v{VERSION}.wav")));
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..16], b"WAVEfmt ");
    assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 2, "channels");
    assert_eq!(
        u32::from_le_bytes(bytes[24..28].try_into().unwrap()),
        48_000
    );
    assert_eq!(u16::from_le_bytes([bytes[34], bytes[35]]), 16, "bits");
    let frames = (get("coin").unwrap().secs * 48_000.0).round() as usize;
    assert_eq!(bytes.len(), 44 + frames * 4);

    // cached: a second call returns the same file without rewriting it
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();
    let again = ensure_file("coin", &dir).unwrap();
    assert_eq!(again, path);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before
    );
    // no temp files left behind
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");

    assert!(ensure_file("no_such_sound", &dir).is_err());

    // ffprobe agrees, when it is installed
    match Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_name,sample_rate,channels,duration",
        ])
        .args(["-of", "default=noprint_wrappers=1"])
        .arg(&path)
        .output()
    {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            assert!(text.contains("codec_name=pcm_s16le"), "{text}");
            assert!(text.contains("sample_rate=48000"), "{text}");
            assert!(text.contains("channels=2"), "{text}");
            assert!(text.contains("duration=0.800000"), "{text}");
        }
        _ => eprintln!("ffprobe not available; skipped the ffprobe check"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
