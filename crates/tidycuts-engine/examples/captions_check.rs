//! Prints the caption lines found in a media file's sound:
//! `captions_check media.mp4 [language]` (default English). Needs the
//! whisper.cpp program and the Whisper model (see `whispercpp::available`).
//! With `TIDYCUTS_PRINT_WORDS` set it prints every word with its times.

use std::sync::atomic::AtomicBool;
use tidycuts_engine::captions::{group_captions, Layout};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let language = args.get(2).map_or("en", String::as_str);
    let media = tidycuts_media::probe(std::path::Path::new(&args[1])).expect("probe");
    let t = std::time::Instant::now();
    let mut last = -1.0;
    let words = tidycuts_engine::whispercpp::transcribe(
        &media.path,
        0,
        (0.0, media.duration.as_secs_f64()),
        language,
        &AtomicBool::new(false),
        |f| {
            if f - last > 0.1 || f >= 1.0 {
                eprintln!(
                    "  {:.0}% after {:.0}s",
                    f * 100.0,
                    t.elapsed().as_secs_f32()
                );
                last = f;
            }
        },
    )
    .expect("transcribe");
    eprintln!(
        "{:.0}s of sound in {:.1}s",
        media.duration.as_secs_f64(),
        t.elapsed().as_secs_f32()
    );
    if std::env::var_os("TIDYCUTS_PRINT_WORDS").is_some() {
        for w in &words {
            println!("WORD {:.3} {:.3} {}", w.start, w.end, w.text);
        }
        return;
    }
    for c in group_captions(&words, Layout::default()) {
        println!(
            "{:7.2} - {:7.2}  {}",
            c.start,
            c.end,
            c.text.replace('\n', " / ")
        );
    }
}
