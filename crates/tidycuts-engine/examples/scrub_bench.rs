//! Times the scrub path: `cargo run --release -p tidycuts-engine --example scrub_bench -- video.mp4 cache_dir`.
//! Builds the scrub cache, then sweeps across the video like a dragged playhead.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tidycuts_engine::frames::FrameServer;
use tidycuts_engine::scrub::ScrubCache;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let media = tidycuts_media::probe(std::path::Path::new(&args[1])).unwrap();
    let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
    let scrub = ScrubCache::new(std::path::PathBuf::from(&args[2]), 3, notify.clone());
    let server = FrameServer::new(scrub.clone(), 256 << 20, 2, notify);
    if args.len() > 3 {
        let t = Instant::now();
        let first = server.proxy_frame(&media, 1.0);
        println!(
            "first proxy picture (reads the table too): {:.1} ms, got one: {}",
            t.elapsed().as_secs_f64() * 1e3,
            first.is_some()
        );
        let dur = media.duration.as_secs_f64();
        let mut worst: f64 = 0.0;
        let mut sum = 0.0;
        for i in 0..200 {
            let at = (i as f64 * 0.6180339887).fract() * dur;
            let t = Instant::now();
            let got = server.proxy_frame(&media, at);
            let ms = t.elapsed().as_secs_f64() * 1e3;
            let _ = &got;
            worst = worst.max(ms);
            sum += ms;
        }
        println!(
            "200 random jumps: mean {:.1} ms, worst {worst:.1} ms",
            sum / 200.0
        );
        jumps(&media, &args[3]);
        return;
    }
    let cache = scrub.get(&media).unwrap();
    let started = Instant::now();
    while !cache.is_complete() {
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("cache complete after {:?}", started.elapsed());
    let dur = media.duration.as_secs_f64();
    let mut times = Vec::new();
    for pass in 0..2 {
        times.clear();
        for i in 0..300 {
            let at = (i as f64 / 300.0) * dur;
            let t = Instant::now();
            let got = server.scrub_frame(&media, at, 2.0);
            times.push((t.elapsed().as_secs_f64() * 1000.0, got.is_some()));
        }
        let max = times.iter().map(|t| t.0).fold(0.0, f64::max);
        let slow = times.iter().filter(|t| t.0 > 10.0).count();
        let mean = times.iter().map(|t| t.0).sum::<f64>() / times.len() as f64;
        println!(
            "pass {pass}: 300 frames, mean {mean:.2} ms, max {max:.1} ms, {slow} over 10 ms, none missing: {}",
            times.iter().all(|t| t.1)
        );
    }
}

/// Jumps to places that are not cached yet: how long until a picture is there.
#[allow(dead_code)]
pub fn jumps(media: &tidycuts_core::MediaItem, dir: &str) {
    let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
    let scrub = ScrubCache::new(std::path::PathBuf::from(dir), 3, notify.clone());
    let server = FrameServer::new(scrub.clone(), 256 << 20, 2, notify);
    let dur = media.duration.as_secs_f64();
    let _ = server.scrub_frame(media, 0.0, 0.25);
    std::thread::sleep(Duration::from_millis(500));
    for k in [0.83, 0.31, 0.62, 0.12, 0.95, 0.47] {
        let at = dur * k;
        let t = Instant::now();
        loop {
            if server.scrub_frame(media, at, 0.25).is_some() {
                break;
            }
            if t.elapsed() > Duration::from_secs(10) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        println!(
            "jump to {at:5.1}s: picture after {:.0} ms",
            t.elapsed().as_secs_f64() * 1000.0
        );
        std::thread::sleep(Duration::from_millis(400));
    }
}
