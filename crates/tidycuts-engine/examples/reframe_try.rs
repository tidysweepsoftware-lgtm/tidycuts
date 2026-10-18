//! Tries Auto Reframe on a video: `cargo run --release -p tidycuts-engine --example reframe_try -- video.mp4`.
fn main() {
    let path = std::env::args().nth(1).expect("a video file");
    let media = tidycuts_media::probe(std::path::Path::new(&path)).unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let t = std::time::Instant::now();
    let r = tidycuts_engine::reframe::analyse(
        &media,
        0.0,
        media.duration.as_secs_f64(),
        9.0 / 16.0,
        &cancel,
    );
    println!(
        "analysed {:.1} s of video in {:?}",
        media.duration.as_secs_f64(),
        t.elapsed()
    );
    match r {
        Some(v) => {
            println!(
                "{} looks; first centres: {:?}",
                v.len(),
                v.iter()
                    .take(12)
                    .map(|x| (x.1 * 100.0).round() / 100.0)
                    .collect::<Vec<_>>()
            );
        }
        None => println!("no result"),
    }
}
