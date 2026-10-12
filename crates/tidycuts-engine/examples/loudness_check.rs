//! Brings a sound file to a loudness and measures the result:
//! `loudness_check in.wav -14`. Checks the export's loudness target.

use std::path::Path;
use tidycuts_engine::loudness::{measure_file, normalize_filter};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = Path::new(&args[1]);
    let target: f64 = args.get(2).and_then(|t| t.parse().ok()).unwrap_or(-14.0);
    let before = measure_file(input).expect("measure");
    println!(
        "before: {:.1} LUFS, peak {:.1} dBTP, range {:.1} LU",
        before.integrated, before.true_peak, before.range
    );
    let filter = normalize_filter(input, target).expect("filter");
    let out = std::env::temp_dir().join("tidycuts-loudness-check.wav");
    let tools = tidycuts_media::Tools::get().expect("ffmpeg");
    let status = tools
        .ffmpeg_cmd()
        .args(["-nostdin", "-y", "-i"])
        .arg(input)
        .args(["-af", &filter, "-c:a", "pcm_s16le"])
        .arg(&out)
        .status()
        .expect("run ffmpeg");
    assert!(status.success());
    let after = measure_file(&out).expect("measure after");
    println!(
        "target {target}: after {:.1} LUFS, peak {:.1} dBTP",
        after.integrated, after.true_peak
    );
}
