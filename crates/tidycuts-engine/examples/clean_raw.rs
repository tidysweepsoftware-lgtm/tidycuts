//! Cleans a raw stereo f32le 48 kHz file with the voice clean-up:
//! `clean_raw in.raw out.raw [mix 0..1]`. Used to measure the clean-up on test
//! recordings.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (input, output) = (&args[1], &args[2]);
    let mix: f32 = args.get(3).and_then(|m| m.parse().ok()).unwrap_or(1.0);
    let bytes = std::fs::read(input).expect("read input");
    let samples: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    let out = tidycuts_engine::voice_clean::clean_stereo(&samples, mix);
    let mut raw = Vec::with_capacity(out.len() * 4);
    for v in out {
        raw.extend_from_slice(&v.to_le_bytes());
    }
    std::fs::write(output, raw).expect("write output");
}
