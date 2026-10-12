//! Renders the sound of one file through the real mixer, the way an export
//! does: `audio_render in.wav out.raw <preset>` (raw stereo f32le, 48 kHz).
//! Presets: none, enhance, clean, level, normalize, normalize_low, loud, old.

use std::time::Duration;
use tidycuts_core::*;
use tidycuts_engine::mixer::Mixer;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let media = tidycuts_media::probe(std::path::Path::new(&args[1])).expect("probe");
    let seconds = media.duration.as_secs_f64();
    let mut p = Project::new("audio");
    let m = p.add_media(media);
    let t = p.add_track(TrackKind::Audio).unwrap();
    let id = p.add_clip(m, t, Time::ZERO).unwrap();
    {
        let a = &mut p.clip_mut(id).unwrap().audio;
        match args.get(3).map(String::as_str).unwrap_or("none") {
            "enhance" => tidycuts_engine::audio_fx::enhance_speech(a),
            "clean" => a.voice_clean = 70.0,
            // What the old Noise Removal did at its 55 setting, for comparing.
            "old" => {
                a.denoise = 55.0;
                a.wind_removal = true;
            }
            "level" => {
                a.leveler = 50.0;
                a.limiter = true;
            }
            "normalize" => a.normalize = true,
            // Normalize at a lower level, to see it work on ordinary speech.
            "normalize_low" => {
                a.normalize = true;
                a.normalize_level = -26.0;
            }
            _ => {}
        }
    }
    // A very loud setting, to see the hearing guard work.
    if args.get(3).map(String::as_str) == Some("loud") {
        p.clip_mut(id).unwrap().volume = Animatable::constant(1600.0);
    }
    let sr = 48_000u32;
    let mut mixer = Mixer::new(sr, Duration::from_secs(30));
    // TIDYCUTS_GUARD=-10 turns the hearing guard on, as playback does.
    mixer.set_guard(
        std::env::var("TIDYCUTS_GUARD")
            .ok()
            .and_then(|g| g.parse().ok()),
    );
    let total = (seconds * sr as f64) as usize;
    let mut out = Vec::with_capacity(total * 8);
    let mut buf = vec![0f32; 4096 * 2];
    let mut done = 0;
    while done < total {
        let n = 4096.min(total - done);
        mixer.mix(
            &p,
            Time::from_secs_f64(done as f64 / sr as f64),
            &mut buf[..n * 2],
        );
        for s in &buf[..n * 2] {
            out.extend_from_slice(&s.to_le_bytes());
        }
        done += n;
    }
    std::fs::write(&args[2], out).expect("write");
}
