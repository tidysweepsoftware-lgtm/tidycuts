//! A clip's audio settings as an ffmpeg filter chain: speed, pitch, noise
//! removal, hum and wind removal, de-essing, equalizer and voice effects.
//! Volume, fades, panning, ducking and loudness evening happen in the mixer
//! because they follow the timeline.

use tidycuts_core::{AudioSettings, Clip};

/// Voice effects: (id, name).
pub static VOICE_EFFECTS: &[(&str, &str)] = &[
    ("", "None"),
    ("robot", "Robot"),
    ("chipmunk", "Chipmunk"),
    ("deep", "Deep Voice"),
    ("child", "Child"),
    ("monster", "Monster"),
    ("radio", "Radio"),
    ("telephone", "Telephone"),
    ("megaphone", "Megaphone"),
    ("echo", "Echo"),
    ("hall", "Big Hall"),
    ("underwater", "Underwater"),
    ("alien", "Alien"),
    ("chorus", "Chorus"),
    ("flanger", "Flanger"),
];

/// Equalizer band centres in Hz.
pub const EQ_BANDS: [f64; 10] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

/// Equalizer presets: (name, gains in dB).
pub static EQ_PRESETS: &[(&str, [f64; 10])] = &[
    ("Default", [0.0; 10]),
    (
        "Bass Boost",
        [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        "Treble Boost",
        [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0],
    ),
    (
        "Vocal",
        [-3.0, -2.0, -1.0, 1.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0],
    ),
    ("Pop", [-1.0, 1.0, 3.0, 4.0, 3.0, 0.0, -1.0, -1.0, 0.0, 1.0]),
    ("Rock", [4.0, 3.0, 2.0, 0.0, -1.0, -1.0, 1.0, 3.0, 4.0, 4.0]),
    (
        "Classical",
        [3.0, 2.0, 1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 2.0, 3.0],
    ),
    ("Jazz", [3.0, 2.0, 1.0, 2.0, -1.0, -1.0, 0.0, 1.0, 2.0, 3.0]),
    (
        "Electronic",
        [5.0, 4.0, 1.0, 0.0, -2.0, 2.0, 1.0, 1.0, 4.0, 5.0],
    ),
    (
        "Hip-Hop",
        [5.0, 4.0, 1.0, 3.0, -1.0, -1.0, 1.0, -1.0, 2.0, 3.0],
    ),
    (
        "Acoustic",
        [4.0, 4.0, 3.0, 1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 2.0],
    ),
    (
        "Podcast",
        [-6.0, -3.0, 0.0, 2.0, 3.0, 3.0, 2.0, 1.0, 0.0, -2.0],
    ),
    (
        "Soft",
        [0.0, 1.0, 2.0, 1.0, 0.0, -1.0, -2.0, -3.0, -3.0, -4.0],
    ),
    (
        "Loudness",
        [6.0, 4.0, 0.0, 0.0, -2.0, 0.0, -1.0, -3.0, 5.0, 1.0],
    ),
];

fn tempo_chain(mut factor: f64, out: &mut Vec<String>) {
    // atempo takes 0.5..=100 per instance; chain for slower speeds.
    while factor < 0.5 {
        out.push("atempo=0.5".into());
        factor /= 0.5;
    }
    if (factor - 1.0).abs() > 1e-6 {
        out.push(format!("atempo={factor:.6}"));
    }
}

/// Shifts pitch by `ratio` without changing length.
fn pitch_chain(ratio: f64, sr: u32, out: &mut Vec<String>) {
    out.push(format!("aresample={sr}"));
    out.push(format!("asetrate={:.3}", sr as f64 * ratio));
    out.push(format!("aresample={sr}"));
    tempo_chain(1.0 / ratio, out);
}

fn voice_chain(id: &str, sr: u32, out: &mut Vec<String>) {
    let semis = |s: f64| 2f64.powf(s / 12.0);
    match id {
        "robot" => out.push(
            "afftfilt=real='hypot(re,im)*sin(0)':imag='hypot(re,im)*cos(0)':win_size=512:overlap=0.75".into(),
        ),
        "chipmunk" => pitch_chain(semis(7.0), sr, out),
        "deep" => pitch_chain(semis(-5.0), sr, out),
        "child" => pitch_chain(semis(4.0), sr, out),
        "monster" => {
            pitch_chain(semis(-9.0), sr, out);
            out.push("aecho=0.8:0.6:60:0.3".into());
        }
        "radio" => out.push("highpass=f=300,lowpass=f=3000,acompressor=threshold=0.1:ratio=4:makeup=2".into()),
        "telephone" => out.push("highpass=f=400,lowpass=f=3400,volume=1.3".into()),
        "megaphone" => out.push("highpass=f=500,lowpass=f=4000,acrusher=bits=8:mix=0.25,volume=1.4".into()),
        "echo" => out.push("aecho=0.8:0.8:250:0.4".into()),
        "hall" => out.push("aecho=0.8:0.9:400|800:0.35|0.25".into()),
        "underwater" => out.push("lowpass=f=600,vibrato=f=1.5:d=0.3".into()),
        "alien" => {
            pitch_chain(semis(3.0), sr, out);
            out.push("vibrato=f=7:d=0.5".into());
        }
        "chorus" => out.push("chorus=0.7:0.9:55:0.4:0.25:2".into()),
        "flanger" => out.push("flanger".into()),
        _ => {}
    }
}

/// Filters for a clip's sound, applied to the source before it reaches the
/// mixer. The output runs at timeline speed (speed changes included).
pub fn clip_filters(clip: &Clip, sr: u32) -> Vec<String> {
    let a: &AudioSettings = &clip.audio;
    let mut f = Vec::new();
    // Clean-up first, while the sound is still natural.
    if a.wind_removal {
        f.push("highpass=f=90".into());
    }
    if a.hum_removal == 50 || a.hum_removal == 60 {
        let base = a.hum_removal as f64;
        for h in 1..=4 {
            f.push(format!(
                "bandreject=f={}:width_type=q:width=30",
                base * h as f64
            ));
        }
    }
    if a.denoise > 0.0 {
        let nr = 6.0 + a.denoise.clamp(0.0, 100.0) * 0.3;
        f.push(format!("afftdn=nr={nr:.1}:nf=-50:tn=1"));
    }
    if a.de_ess {
        f.push("deesser=i=0.6".into());
    }
    for (g, band) in a.eq.iter().zip(EQ_BANDS) {
        if g.abs() > 0.01 {
            f.push(format!(
                "equalizer=f={band}:t=o:w=1:g={:.2}",
                g.clamp(-12.0, 12.0)
            ));
        }
    }
    if a.pitch.abs() > 0.01 {
        pitch_chain(2f64.powf(a.pitch.clamp(-12.0, 12.0) / 12.0), sr, &mut f);
    }
    if !a.voice.is_empty() {
        voice_chain(&a.voice, sr, &mut f);
    }
    if clip.reverse {
        f.push("areverse".into());
    }
    let speed = clip.speed;
    if (speed - 1.0).abs() > 1e-6 {
        if a.keep_pitch {
            tempo_chain(speed, &mut f);
        } else {
            f.push(format!("aresample={sr}"));
            f.push(format!("asetrate={:.3}", sr as f64 * speed));
            f.push(format!("aresample={sr}"));
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tidycuts_core::{MediaItem, MediaKind, Time};

    fn clip() -> Clip {
        let m = MediaItem::new(
            PathBuf::from("/a.wav"),
            MediaKind::Audio,
            Time::from_secs_f64(5.0),
        );
        Clip::new(&m, Time::ZERO)
    }

    #[test]
    fn plain_clips_need_no_filters() {
        assert!(clip_filters(&clip(), 48000).is_empty());
    }

    #[test]
    fn slow_speed_chains_atempo() {
        let mut c = clip();
        c.speed = 0.25;
        let f = clip_filters(&c, 48000);
        assert_eq!(f, vec!["atempo=0.5", "atempo=0.500000"]);
    }

    #[test]
    fn clean_up_comes_before_effects() {
        let mut c = clip();
        c.audio.denoise = 50.0;
        c.audio.voice = "echo".into();
        c.audio.eq[0] = 3.0;
        let f = clip_filters(&c, 48000).join(",");
        assert!(f.find("afftdn").unwrap() < f.find("equalizer").unwrap());
        assert!(f.find("equalizer").unwrap() < f.find("aecho").unwrap());
    }

    #[test]
    fn presets_have_ten_bands() {
        for (name, g) in EQ_PRESETS {
            assert_eq!(g.len(), 10, "{name}");
        }
    }
}
