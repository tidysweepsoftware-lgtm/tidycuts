//! Render every built-in sound to WAV files for inspection.
//!
//! ```text
//! cargo run --release -p tidycuts-sounds --example sheet -- <out-dir> [id-filter ...]
//! ```
//!
//! Files are written with `ensure_file`, so they are named `<id>-v<VERSION>.wav`.
//! Existing files are deleted first so every sound is rendered fresh and timed.

use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "tidycuts-sounds-out".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create output dir");

    let total = Instant::now();
    let mut audio_secs = 0.0f32;
    let mut count = 0;
    println!(
        "{:<20} {:<22} {:>6} {:>9}",
        "id", "category", "secs", "render"
    );
    for def in tidycuts_sounds::catalog() {
        if !filters.is_empty() && !filters.iter().any(|f| def.id.contains(f.as_str())) {
            continue;
        }
        let path = dir.join(format!("{}-v{}.wav", def.id, tidycuts_sounds::VERSION));
        let _ = std::fs::remove_file(&path);
        let t = Instant::now();
        tidycuts_sounds::ensure_file(def.id, &dir).expect("render sound");
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        println!(
            "{:<20} {:<22} {:>6.2} {:>7.0}ms",
            def.id, def.category, def.secs, ms
        );
        audio_secs += def.secs;
        count += 1;
    }
    let el = total.elapsed().as_secs_f32();
    println!(
        "{count} sounds, {audio_secs:.1} s of audio rendered and written in {el:.2} s ({:.0}x real time)",
        audio_secs / el.max(1e-6)
    );
}
