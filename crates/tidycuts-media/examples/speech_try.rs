//! Tries the system voices: `cargo run --release -p tidycuts-media --example speech_try`.
fn main() {
    let t = std::time::Instant::now();
    let v = tidycuts_media::speech::voices();
    println!(
        "{} voices in {:?}: {:?}",
        v.len(),
        t.elapsed(),
        v.iter().map(|x| x.name.clone()).collect::<Vec<_>>()
    );
    if let Some(first) = v.first() {
        let out = std::env::temp_dir().join("tc_speech_try.wav");
        let t = std::time::Instant::now();
        let r = tidycuts_media::speech::speak("This is a voiceover test.", &first.id, 1.0, &out);
        println!(
            "speak: {r:?} in {:?}, {} bytes",
            t.elapsed(),
            std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0)
        );
    }
}
