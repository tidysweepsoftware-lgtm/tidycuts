//! Does starting FFmpeg freeze other threads? `cargo run --release -p tidycuts-engine --example spawn_stall -- video.mp4`
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tidycuts_engine::decode::{decode_still, VideoParams};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let media = tidycuts_media::probe(std::path::Path::new(&args[1])).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let s2 = stop.clone();
    // A thread that wants to run every 2 ms and notes the worst delay.
    let watcher = std::thread::spawn(move || {
        let mut worst = Duration::ZERO;
        let mut stalls = Vec::new();
        while !s2.load(Ordering::Relaxed) {
            let t = Instant::now();
            std::thread::sleep(Duration::from_millis(2));
            let late = t.elapsed().saturating_sub(Duration::from_millis(2));
            if late > Duration::from_millis(30) {
                stalls.push(late.as_millis());
            }
            worst = worst.max(late);
        }
        (worst, stalls)
    });
    std::thread::sleep(Duration::from_millis(300));
    for i in 0..8 {
        let params = VideoParams::for_media(&media, 960, 540);
        let t = Instant::now();
        let _ = decode_still(&params, 5.0 + i as f64 * 20.0);
        println!("still {i}: {} ms", t.elapsed().as_millis());
        std::thread::sleep(Duration::from_millis(400));
    }
    stop.store(true, Ordering::Relaxed);
    let (worst, stalls) = watcher.join().unwrap();
    println!(
        "worst delay of the watcher thread: {} ms; delays over 30 ms: {:?}",
        worst.as_millis(),
        stalls
    );
}
