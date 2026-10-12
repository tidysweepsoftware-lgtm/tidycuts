//! Prints where there is speech in a raw mono f32le 48 kHz file:
//! `speech_ranges in.raw`. Used to check the voice detector behind Auto-Duck.

use tidycuts_engine::autoduck::{speech_ranges, voice_chance};

fn main() {
    let path = std::env::args().nth(1).expect("a raw file");
    let bytes = std::fs::read(path).expect("read");
    let samples: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    let chances = voice_chance(&samples);
    for (s, e) in speech_ranges(&chances, 0.6, 0.35, 0.35, 0.3) {
        println!("{s:.2} {e:.2}");
    }
}
