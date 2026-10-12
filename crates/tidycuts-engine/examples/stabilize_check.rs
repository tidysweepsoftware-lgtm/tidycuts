//! Checks camera-shake removal on a clip made with known shake:
//! `stabilize_check shaky.mp4 [strength]`. The clip must have been made by
//! cropping a still picture with the movement defined in `shake()`.

use std::sync::atomic::AtomicBool;
use tidycuts_core::Time;
use tidycuts_engine::stabilize::{self, smooth};

/// The camera's true position in pixels of the 960-wide clip at time `t`.
fn shake(t: f64) -> (f64, f64) {
    let tau = std::f64::consts::TAU;
    (
        (220.0 + 14.0 * (tau * 1.3 * t).sin() + 9.0 * (tau * 3.1 * t + 1.0).sin()).round() - 220.0,
        (120.0 + 10.0 * (tau * 1.9 * t + 2.0).sin() + 6.0 * (tau * 4.3 * t).sin()).round() - 120.0,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let strength: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(50.0);
    let media = tidycuts_media::probe(std::path::Path::new(&args[1])).expect("probe");
    let range = (Time::ZERO, media.duration);
    let t0 = std::time::Instant::now();
    assert!(stabilize::finish(&media, range, &AtomicBool::new(false)));
    println!(
        "analysed {:.1}s of video in {:.1}s",
        media.duration.as_secs_f64(),
        t0.elapsed().as_secs_f64()
    );
    let m = stabilize::measured(&media, range).unwrap();
    let steps = m.steps();
    let fps = m.fps;
    let width = 960.0f64;
    // Recovered path (content moves opposite to the camera).
    let mut est = vec![(0.0f64, 0.0f64)];
    for s in steps {
        let l = *est.last().unwrap();
        est.push((
            l.0 + f64::from(s[0]) * width,
            l.1 + f64::from(s[1]) * width * 9.0 / 16.0,
        ));
    }
    let truth: Vec<(f64, f64)> = (0..est.len())
        .map(|i| {
            let (x, y) = shake(i as f64 / fps);
            let (x0, y0) = shake(0.0);
            (-(x - x0), -(y - y0))
        })
        .collect();
    let err = |a: &[(f64, f64)], b: &[(f64, f64)]| -> f64 {
        (a.iter()
            .zip(b)
            .map(|(p, q)| (p.0 - q.0).powi(2) + (p.1 - q.1).powi(2))
            .sum::<f64>()
            / a.len() as f64)
            .sqrt()
    };
    for i in 1..16 {
        println!(
            "step {i}: est ({:6.2},{:6.2})  true ({:6.2},{:6.2})",
            est[i].0 - est[i - 1].0,
            est[i].1 - est[i - 1].1,
            truth[i].0 - truth[i - 1].0,
            truth[i].1 - truth[i - 1].1
        );
    }
    println!(
        "path error: {:.2} px (shake size {:.1} px rms)",
        err(&est, &truth),
        err(&truth, &vec![(0.0, 0.0); truth.len()])
    );
    // Shake left after the correction (frame to frame jitter).
    let s = smooth(steps, fps, strength);
    // The true path with the correction applied.
    let after: Vec<(f64, f64)> = truth
        .iter()
        .enumerate()
        .map(|(i, p)| {
            (
                p.0 + f64::from(s.moves[i][0]) * width,
                p.1 + f64::from(s.moves[i][1]) * width * 9.0 / 16.0,
            )
        })
        .collect();
    let jit = |p: &[(f64, f64)]| -> f64 {
        (1..p.len() - 1)
            .map(|i| {
                (p[i].0 - 0.5 * (p[i - 1].0 + p[i + 1].0)).powi(2)
                    + (p[i].1 - 0.5 * (p[i - 1].1 + p[i + 1].1)).powi(2)
            })
            .sum::<f64>()
            .sqrt()
            / ((p.len() - 2) as f64).sqrt()
    };
    println!(
        "jitter before {:.2} px, after {:.2} px, zoom {:.3}",
        jit(&truth),
        jit(&after),
        s.zoom
    );
}
