//! Prints the words and caption lines found in a media file's sound:
//! `captions_check media.wav [en|multi]`. Needs the speech model (see
//! `captions::model_dir`) and ONNX Runtime.

use std::sync::atomic::AtomicBool;
use tidycuts_engine::captions::{group_captions, transcribe, Layout, Model};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let model = match args.get(2).map(String::as_str) {
        Some("multi") => Model::Multilingual,
        _ => Model::English,
    };
    let media = tidycuts_media::probe(std::path::Path::new(&args[1])).expect("probe");
    let t = std::time::Instant::now();
    let words = transcribe(
        &media.path,
        0,
        (0.0, media.duration.as_secs_f64()),
        model,
        &AtomicBool::new(false),
        |_| {},
    )
    .expect("transcribe");
    eprintln!(
        "{:.0}s of sound in {:.1}s",
        media.duration.as_secs_f64(),
        t.elapsed().as_secs_f32()
    );
    for c in group_captions(&words, Layout::default()) {
        println!(
            "{:7.2} - {:7.2}  {}",
            c.start,
            c.end,
            c.text.replace('\n', " / ")
        );
    }
}
