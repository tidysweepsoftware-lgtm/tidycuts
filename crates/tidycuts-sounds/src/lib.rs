//! TidyCuts' built-in, royalty-free sound library.
//!
//! Every sound effect and music bed here is synthesised from code at run
//! time: no samples, no downloads, no models. Rendering is deterministic
//! (all noise is seeded from the sound's id), so the same id always produces
//! the same audio, and [`ensure_file`] can cache renders on disk keyed by
//! [`VERSION`].
//!
//! ```no_run
//! let dir = std::env::temp_dir().join("tidycuts-sounds");
//! for def in tidycuts_sounds::catalog() {
//!     let wav = tidycuts_sounds::ensure_file(def.id, &dir).unwrap();
//!     println!("{} ({}, {:.1} s) -> {}", def.name, def.category, def.secs, wav.display());
//! }
//! ```

mod drums;
mod dsp;
mod inst;
mod master;
mod music;
mod sfx;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use dsp::{frames, hash_str, Ctx, Rng, Stereo};
use master::{Edges, Level};

/// Bump whenever any sound changes, so cached files are re-rendered.
pub const VERSION: u32 = 1;

/// Sample rate used by [`ensure_file`].
pub const FILE_SAMPLE_RATE: u32 = 48_000;

/// Whether an entry is a one-shot/ambience effect or a music bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundKind {
    Effect,
    Music,
}

/// A catalog entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoundDef {
    /// Stable identifier (also the cache file stem).
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Display category (e.g. "Impacts", "Chill").
    pub category: &'static str,
    /// Exact length in seconds.
    pub secs: f32,
    pub kind: SoundKind,
}

type Gen = fn(&mut Ctx) -> Stereo;

macro_rules! sounds {
    ($( $id:literal, $name:literal, $cat:literal, $secs:literal, $kind:ident, $level:expr, $gen:path; )*) => {
        static CATALOG: &[SoundDef] = &[
            $( SoundDef { id: $id, name: $name, category: $cat, secs: $secs, kind: SoundKind::$kind }, )*
        ];
        fn generator(id: &str) -> Option<(Gen, Level)> {
            match id {
                $( $id => Some(($gen as Gen, $level)), )*
                _ => None,
            }
        }
    };
}

const FX: Level = Level::Fx(-13.0);
const BIG: Level = Level::Fx(-11.0);
const AMB: Level = Level::Lufs(-24.0);
const MUSIC: Level = Level::Lufs(-14.0);

sounds! {
    // id, name, category, seconds, kind, level, generator
    "soft_whoosh", "Soft Whoosh", "Whoosh & Transitions", 1.2, Effect, FX, sfx::soft_whoosh;
    "fast_whoosh", "Fast Whoosh", "Whoosh & Transitions", 0.6, Effect, FX, sfx::fast_whoosh;
    "swoosh_up", "Swoosh Up", "Whoosh & Transitions", 1.0, Effect, FX, sfx::swoosh_up;
    "swoosh_down", "Swoosh Down", "Whoosh & Transitions", 1.0, Effect, FX, sfx::swoosh_down;
    "swish", "Swish", "Whoosh & Transitions", 0.45, Effect, FX, sfx::swish;
    "riser", "Riser", "Whoosh & Transitions", 3.0, Effect, FX, sfx::riser;
    "downlifter", "Downlifter", "Whoosh & Transitions", 2.5, Effect, FX, sfx::downlifter;
    "reverse_cymbal", "Reverse Cymbal", "Whoosh & Transitions", 2.0, Effect, FX, sfx::reverse_cymbal;
    "glitch", "Glitch", "Whoosh & Transitions", 0.8, Effect, FX, sfx::glitch;
    "magic_sparkle", "Magic Sparkle", "Whoosh & Transitions", 2.0, Effect, FX, sfx::magic_sparkle;

    "cinematic_boom", "Cinematic Boom", "Impacts", 4.0, Effect, BIG, sfx::cinematic_boom;
    "sub_drop", "Sub Drop", "Impacts", 3.0, Effect, BIG, sfx::sub_drop;
    "hit", "Hit", "Impacts", 1.5, Effect, BIG, sfx::hit;
    "punch", "Punch", "Impacts", 0.6, Effect, BIG, sfx::punch;
    "thud", "Thud", "Impacts", 0.8, Effect, BIG, sfx::thud;
    "glass_tink", "Glass Tink", "Impacts", 1.5, Effect, FX, sfx::glass_tink;
    "metal_clang", "Metal Clang", "Impacts", 2.5, Effect, BIG, sfx::metal_clang;
    "braam", "Cinematic Braam", "Impacts", 3.5, Effect, BIG, sfx::braam;

    "pop", "Pop", "Pops & Clicks", 0.3, Effect, FX, sfx::pop;
    "bubble_pop", "Bubble Pop", "Pops & Clicks", 0.45, Effect, FX, sfx::bubble_pop;
    "click", "Click", "Pops & Clicks", 0.15, Effect, FX, sfx::click;
    "mouse_click", "Mouse Click", "Pops & Clicks", 0.25, Effect, FX, sfx::mouse_click;
    "tick", "Tick", "Pops & Clicks", 0.12, Effect, FX, sfx::tick;
    "toggle", "Toggle Switch", "Pops & Clicks", 0.3, Effect, FX, sfx::toggle;
    "swipe", "Swipe", "Pops & Clicks", 0.35, Effect, FX, sfx::swipe;
    "keyboard_typing", "Keyboard Typing", "Pops & Clicks", 3.0, Effect, FX, sfx::keyboard_typing;
    "typewriter", "Typewriter", "Pops & Clicks", 4.0, Effect, FX, sfx::typewriter;
    "camera_shutter", "Camera Shutter", "Pops & Clicks", 0.5, Effect, FX, sfx::camera_shutter;

    "ding", "Ding", "Notifications", 1.5, Effect, FX, sfx::ding;
    "chime", "Chime", "Notifications", 2.5, Effect, FX, sfx::chime;
    "bell", "Bell", "Notifications", 3.0, Effect, FX, sfx::bell;
    "success", "Success", "Notifications", 1.5, Effect, FX, sfx::success;
    "error_buzz", "Error Buzz", "Notifications", 0.8, Effect, FX, sfx::error_buzz;
    "notification", "Notification", "Notifications", 1.2, Effect, FX, sfx::notification;
    "message_pop", "Message Pop", "Notifications", 0.6, Effect, FX, sfx::message_pop;
    "coin", "Coin", "Notifications", 0.8, Effect, FX, sfx::coin;
    "level_up", "Level Up", "Notifications", 1.5, Effect, FX, sfx::level_up;

    "beep", "Beep", "Countdown & Timers", 0.4, Effect, FX, sfx::beep;
    "countdown", "Countdown 3-2-1", "Countdown & Timers", 4.0, Effect, FX, sfx::countdown;
    "clock_ticking", "Clock Ticking", "Countdown & Timers", 5.0, Effect, FX, sfx::clock_ticking;
    "alarm", "Alarm", "Countdown & Timers", 3.0, Effect, FX, sfx::alarm;

    "boing", "Boing", "Cartoon & Comedy", 1.0, Effect, FX, sfx::boing;
    "slide_whistle_up", "Slide Whistle Up", "Cartoon & Comedy", 1.0, Effect, FX, sfx::slide_whistle_up;
    "slide_whistle_down", "Slide Whistle Down", "Cartoon & Comedy", 1.0, Effect, FX, sfx::slide_whistle_down;
    "rimshot", "Rimshot (Ba-Dum-Tss)", "Cartoon & Comedy", 2.0, Effect, FX, sfx::rimshot;
    "record_scratch", "Record Scratch", "Cartoon & Comedy", 0.8, Effect, FX, sfx::record_scratch;
    "laser", "Laser", "Cartoon & Comedy", 0.6, Effect, FX, sfx::laser;
    "sad_trombone", "Sad Trombone", "Cartoon & Comedy", 3.0, Effect, FX, sfx::sad_trombone;

    "kick", "Kick", "Drums", 0.8, Effect, FX, sfx::kick;
    "snare", "Snare", "Drums", 0.6, Effect, FX, sfx::snare;
    "clap", "Clap", "Drums", 0.6, Effect, FX, sfx::clap;
    "hihat", "Hi-Hat", "Drums", 0.3, Effect, FX, sfx::hihat;
    "open_hihat", "Open Hi-Hat", "Drums", 0.8, Effect, FX, sfx::open_hihat;
    "crash", "Crash Cymbal", "Drums", 3.0, Effect, FX, sfx::crash;

    "rain", "Rain", "Ambience", 10.0, Effect, AMB, sfx::rain;
    "wind", "Wind", "Ambience", 10.0, Effect, AMB, sfx::wind;
    "ocean_waves", "Ocean Waves", "Ambience", 12.0, Effect, AMB, sfx::ocean_waves;
    "fire_crackle", "Fire Crackle", "Ambience", 10.0, Effect, AMB, sfx::fire_crackle;
    "room_tone", "Room Tone", "Ambience", 10.0, Effect, Level::Lufs(-42.0), sfx::room_tone;
    "heartbeat", "Heartbeat", "Ambience", 10.0, Effect, Level::Lufs(-20.0), sfx::heartbeat;
    "forest_birds", "Forest Birds", "Ambience", 12.0, Effect, AMB, sfx::forest_birds;

    "lofi_chill", "Lo-fi Chill", "Chill", 42.0, Music, MUSIC, music::lofi_chill;
    "acoustic_sunshine", "Acoustic Sunshine", "Chill", 50.0, Music, MUSIC, music::acoustic_sunshine;
    "upbeat_pop", "Upbeat Pop", "Upbeat", 45.0, Music, MUSIC, music::upbeat_pop;
    "corporate_inspire", "Corporate Inspire", "Upbeat", 49.0, Music, MUSIC, music::corporate_inspire;
    "funky_groove", "Funky Groove", "Upbeat", 45.0, Music, MUSIC, music::funky_groove;
    "cinematic_pulse", "Cinematic Pulse", "Cinematic", 45.0, Music, MUSIC, music::cinematic_pulse;
    "retro_synthwave", "Retro Synthwave", "Cinematic", 52.0, Music, MUSIC, music::retro_synthwave;
    "ambient_dreams", "Ambient Dreams", "Ambient", 48.0, Music, MUSIC, music::ambient_dreams;
    "calm_piano", "Calm Piano", "Ambient", 49.0, Music, MUSIC, music::calm_piano;
}

/// All sounds, in display order.
pub fn catalog() -> &'static [SoundDef] {
    CATALOG
}

/// Look up a sound by id.
pub fn get(id: &str) -> Option<&'static SoundDef> {
    CATALOG.iter().find(|d| d.id == id)
}

/// The categories of one kind, in display order.
pub fn categories(kind: SoundKind) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for d in CATALOG.iter().filter(|d| d.kind == kind) {
        if !out.contains(&d.category) {
            out.push(d.category);
        }
    }
    out
}

/// Render a sound as interleaved stereo `f32` samples in -1..=1 at
/// `sample_rate` (clamped to 8–192 kHz). Returns `None` for unknown ids.
/// The result has exactly `round(secs * sample_rate)` frames, peaks below
/// -1 dBFS and fades in and out without clicks.
pub fn render(id: &str, sample_rate: u32) -> Option<Vec<f32>> {
    let def = get(id)?;
    let (gen, level) = generator(def.id)?;
    let sr = sample_rate.clamp(8_000, 192_000) as f32;
    let n = frames(def.secs, sr);
    let mut ctx = Ctx {
        sr,
        n,
        rng: Rng::new(hash_str(def.id)),
    };
    let mut st = gen(&mut ctx);
    st.resize(n);
    let edges = match (def.kind, def.category) {
        (SoundKind::Music, _) => Edges::Music,
        (_, "Ambience") => Edges::Loop,
        _ => Edges::Shot,
    };
    Some(master::finish(st, sr, level, edges))
}

/// Make sure `dir/<id>-v<VERSION>.wav` exists (16-bit stereo, 48 kHz),
/// rendering and writing it if needed, and return its path. Files are written
/// to a temporary name and renamed, so a half-written file is never reused.
pub fn ensure_file(id: &str, dir: &Path) -> io::Result<PathBuf> {
    let def = get(id).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, format!("unknown sound id `{id}`"))
    })?;
    let path = dir.join(format!("{}-v{}.wav", def.id, VERSION));
    if fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() > 44) {
        return Ok(path);
    }
    fs::create_dir_all(dir)?;
    let samples = render(def.id, FILE_SAMPLE_RATE)
        .ok_or_else(|| io::Error::other(format!("failed to render `{id}`")))?;
    let bytes = master::wav_bytes(&samples, FILE_SAMPLE_RATE, hash_str(def.id));
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let tmp = dir.join(format!(
        ".{}-v{}.{}-{}.tmp",
        def.id,
        VERSION,
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&tmp, bytes)?;
    if let Err(e) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        // Another writer may have won the race; that file is just as good.
        if !path.is_file() {
            return Err(e);
        }
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::{Osc, Stereo};

    #[test]
    fn loudness_of_a_reference_tone() {
        // A 0 dBFS 1 kHz sine in one channel reads -3.01 LUFS (BS.1770), so a
        // -20 dBFS sine in both channels reads -20 LUFS.
        let sr = 48_000.0;
        let mut o = Osc::default();
        let mono: Vec<f32> = (0..frames(3.0, sr))
            .map(|_| 0.1 * o.sine(1000.0, sr))
            .collect();
        let st = Stereo {
            l: mono.clone(),
            r: mono,
        };
        let l = master::integrated_lufs(&st, sr);
        assert!((l + 20.0).abs() < 0.3, "{l}");
    }

    #[test]
    fn limiter_respects_ceiling() {
        let sr = 48_000.0;
        let mut st = Stereo::new(48_000);
        for i in 0..st.len() {
            let x = (i as f32 * 0.05).sin() * if i % 5000 < 50 { 3.0 } else { 0.5 };
            st.l[i] = x;
            st.r[i] = -x;
        }
        master::limit(&mut st, sr, 0.8);
        assert!(st.peak() <= 0.8 + 1e-6);
    }
}
