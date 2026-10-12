//! Times making pictures: `imagegen_bench <runs> [width height]`. Uses
//! TIDYCUTS_IMAGEGEN_DIR / TIDYCUTS_IMAGEGEN_MODELS; set TIDYCUTS_IMAGEGEN_NO_SERVICE=1 to time
//! the one-shot program instead of the background one.

use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tidycuts_engine::imagegen::{self, Quality, Request};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let runs: u32 = a.get(1).and_then(|v| v.parse().ok()).unwrap_or(2);
    let (w, h) = (
        a.get(2).and_then(|v| v.parse().ok()).unwrap_or(1280),
        a.get(3).and_then(|v| v.parse().ok()).unwrap_or(720),
    );
    let out = std::env::temp_dir().join("tidycuts-imagegen-bench");
    let never = AtomicBool::new(false);
    for i in 0..runs {
        let req = Request {
            prompt: imagegen::full_prompt("a shocked man pointing at a glowing red button", 0),
            width: w,
            height: h,
            count: a.get(4).and_then(|v| v.parse().ok()).unwrap_or(4),
            seed: 100 + u64::from(i),
            references: vec![],
            quality: Quality::Standard,
        };
        let t = Instant::now();
        let mut first = None;
        let mut last = 99;
        let r = imagegen::generate(&req, &out, &never, |f| {
            if f > 0.0 && first.is_none() {
                first = Some(t.elapsed());
            }
            let tenth = (f * 10.0) as u32;
            if tenth != last {
                last = tenth;
                println!(
                    "   {:>3.0}% at {:.1} s",
                    f * 100.0,
                    t.elapsed().as_secs_f64()
                );
            }
        });
        println!(
            "run {}: {:?} in {:.1} s (first progress after {:?})",
            i + 1,
            r.map(|v| v.len()),
            t.elapsed().as_secs_f64(),
            first
        );
    }
    tidycuts_engine::imagegen_service::shutdown();
}
