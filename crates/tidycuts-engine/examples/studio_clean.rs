//! Cleans a raw stereo f32le 48 kHz file with Studio Voice:
//! `studio_clean in.raw out.raw [mix 0..1] [gain]`. Needs the model and
//! ONNX Runtime (see `studio_voice::model_file`).

use std::time::Instant;
use tidycuts_engine::studio_voice::{Studio, StudioStream};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mix: f32 = args.get(3).and_then(|m| m.parse().ok()).unwrap_or(1.0);
    let gain: f32 = args.get(4).and_then(|m| m.parse().ok()).unwrap_or(1.0);
    let bytes = std::fs::read(&args[1]).expect("read input");
    let samples: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    let t = Instant::now();
    let studio = Studio::load().expect("load");
    println!("loaded in {:.1}s", t.elapsed().as_secs_f32());
    let mut stream = StudioStream::new(studio, gain, mix);
    let mut out = Vec::with_capacity(samples.len());
    let t = Instant::now();
    for chunk in samples.chunks(96_000) {
        out.extend(stream.push(chunk).expect("push"));
    }
    out.extend(stream.finish().expect("finish"));
    let secs = samples.len() as f32 / 96_000.0;
    println!(
        "{secs:.0}s of sound in {:.1}s ({:.1}x real time), {} samples out of {}",
        t.elapsed().as_secs_f32(),
        secs / t.elapsed().as_secs_f32(),
        out.len(),
        samples.len()
    );
    let mut raw = Vec::with_capacity(out.len() * 4);
    for v in out {
        raw.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(&args[2], raw).expect("write");
}
