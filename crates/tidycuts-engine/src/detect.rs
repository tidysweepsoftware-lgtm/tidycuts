//! Finding things in a clip's sound and picture: silent stretches, scene
//! changes and the beat. Silences and scenes use ffmpeg's own detectors;
//! the beat is tracked from the sound itself.
//!
//! Every function works on a stretch of the source file (`range`, in
//! seconds) and returns times in source seconds.

use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use tidycuts_media::Tools;

/// Silent stretches (start, end) in source seconds: quieter than `noise_db`
/// (for example -35) for at least `min_secs`.
pub fn silences(
    tools: &Tools,
    path: &Path,
    range: (f64, f64),
    noise_db: f64,
    min_secs: f64,
) -> Result<Vec<(f64, f64)>, String> {
    let out = run_ffmpeg_log(
        tools,
        path,
        range,
        &[
            "-vn",
            "-af",
            &format!("silencedetect=noise={noise_db}dB:d={min_secs}"),
        ],
    )?;
    Ok(parse_silences(&out, range))
}

/// Moments (source seconds) where the picture changes scene. `threshold` is
/// 0..1; 0.3 suits most footage and lower finds more.
pub fn scene_changes(
    tools: &Tools,
    path: &Path,
    range: (f64, f64),
    threshold: f64,
) -> Result<Vec<f64>, String> {
    let out = run_ffmpeg_log(
        tools,
        path,
        range,
        &[
            "-an",
            "-vf",
            &format!("select='gt(scene,{threshold})',showinfo"),
        ],
    )?;
    Ok(parse_scene_times(&out, range.0))
}

/// Beat times (source seconds) of the sound in `range`.
pub fn beats(tools: &Tools, path: &Path, range: (f64, f64)) -> Result<Vec<f64>, String> {
    const RATE: u32 = 11_025;
    let samples = decode_mono(tools, path, range, RATE)?;
    if samples.is_empty() {
        return Err("This clip has no sound to find a beat in.".into());
    }
    Ok(track_beats(&samples, RATE)
        .into_iter()
        .map(|t| t + range.0)
        .collect())
}

/// The sound of `range` of a file as mono samples at `rate`.
pub(crate) fn decode_mono(
    tools: &Tools,
    path: &Path,
    range: (f64, f64),
    rate: u32,
) -> Result<Vec<f32>, String> {
    let mut cmd = tools.ffmpeg_cmd();
    cmd.args(["-ss", &format!("{:.3}", range.0), "-t"]);
    cmd.arg(format!("{:.3}", (range.1 - range.0).max(0.1)));
    cmd.arg("-i").arg(path);
    cmd.args([
        "-vn",
        "-ac",
        "1",
        "-ar",
        &rate.to_string(),
        "-f",
        "f32le",
        "pipe:1",
    ]);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start ffmpeg: {e}"))?;
    let mut bytes = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        out.read_to_end(&mut bytes)
            .map_err(|e| format!("Could not read the sound: {e}"))?;
    }
    let _ = child.wait();
    let (samples, _) = bytes.as_chunks::<4>();
    Ok(samples.iter().map(|b| f32::from_le_bytes(*b)).collect())
}

/// Runs ffmpeg over `range` of the file with a detector filter and returns
/// what it logged (the detectors report on the log).
fn run_ffmpeg_log(
    tools: &Tools,
    path: &Path,
    range: (f64, f64),
    filter_args: &[&str],
) -> Result<String, String> {
    let mut cmd = tools.ffmpeg_cmd();
    // The detectors write at the "info" level, which the tools hide.
    cmd.args(["-loglevel", "info", "-ss", &format!("{:.3}", range.0), "-t"]);
    cmd.arg(format!("{:.3}", (range.1 - range.0).max(0.1)));
    cmd.arg("-i").arg(path);
    cmd.args(filter_args);
    cmd.args(["-f", "null", "-"]);
    cmd.stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    let out = cmd
        .output()
        .map_err(|e| format!("Could not start ffmpeg: {e}"))?;
    if !out.status.success() {
        let text = String::from_utf8_lossy(&out.stderr);
        let last = text
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        return Err(format!("ffmpeg could not analyse the file: {last}"));
    }
    Ok(String::from_utf8_lossy(&out.stderr).into_owned())
}

/// The value after `key` on a log line ("silence_start: 1.5" → 1.5).
fn after(line: &str, key: &str) -> Option<f64> {
    let rest = line[line.find(key)? + key.len()..].trim_start();
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e'))
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Reads silencedetect's log into (start, end) pairs in source seconds. The
/// log's times start at zero for the analysed stretch; a silence that never
/// ends runs to the end of the stretch.
pub fn parse_silences(log: &str, range: (f64, f64)) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut open: Option<f64> = None;
    for line in log.lines() {
        if let Some(s) = after(line, "silence_start:") {
            open = Some(s.max(0.0));
        } else if let Some(e) = after(line, "silence_end:") {
            // A silence that began before the stretch has no start line.
            let s = open.take().unwrap_or(0.0);
            out.push((s + range.0, e + range.0));
        }
    }
    if let Some(s) = open {
        out.push((s + range.0, range.1));
    }
    out.retain(|(a, b)| b > a);
    out
}

/// Reads the `pts_time:` of every frame showinfo logged (the frames the
/// scene filter let through), offset to source seconds.
pub fn parse_scene_times(log: &str, offset: f64) -> Vec<f64> {
    let mut out: Vec<f64> = log
        .lines()
        .filter(|l| l.contains("Parsed_showinfo"))
        .filter_map(|l| after(l, "pts_time:"))
        .map(|t| t + offset)
        .collect();
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < 0.2);
    out
}

// ---- beat tracking -----------------------------------------------------------

const WINDOW: usize = 1024;
const HOP: usize = 256;

/// How strongly the sound "hits" at each step of `HOP` samples: the rise of
/// the spectrum between neighbouring windows.
fn onset_strength(samples: &[f32]) -> Vec<f32> {
    let bins = 160; // up to about 1.7 kHz at 11 kHz: where drums and attacks live
    let hann: Vec<f32> = (0..WINDOW)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / WINDOW as f32).cos())
        .collect();
    let mut prev = vec![0f32; bins];
    let mut out = Vec::new();
    let mut start = 0;
    while start + WINDOW <= samples.len() {
        let mut re: Vec<f32> = (0..WINDOW).map(|i| samples[start + i] * hann[i]).collect();
        let mut im = vec![0f32; WINDOW];
        crate::levels::fft(&mut re, &mut im);
        let mut flux = 0.0;
        for k in 1..bins {
            let mag = (re[k] * re[k] + im[k] * im[k]).sqrt();
            let log = (1.0 + 20.0 * mag).ln();
            flux += (log - prev[k]).max(0.0);
            prev[k] = log;
        }
        out.push(flux);
        start += HOP;
    }
    // Take away the slow trend and scale to a steady size.
    let n = out.len().max(1) as f32;
    let mean = out.iter().sum::<f32>() / n;
    let sd = (out.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n).sqrt();
    if sd < 1e-4 {
        return Vec::new();
    }
    out.iter().map(|v| ((v - mean) / sd).max(0.0)).collect()
}

/// The most likely gap between beats, in steps, from the onsets' own
/// rhythm (60 to 180 beats a minute, leaning towards 120).
fn beat_period(onset: &[f32], steps_per_sec: f32) -> Option<f32> {
    let lo = (steps_per_sec * 60.0 / 180.0).round() as usize;
    let hi = (steps_per_sec * 60.0 / 60.0).round() as usize;
    if onset.len() < hi * 3 {
        return None;
    }
    let mut best: Option<(f32, f32)> = None;
    for lag in lo..=hi {
        let sum: f32 = (lag..onset.len()).map(|i| onset[i] * onset[i - lag]).sum();
        let bpm = 60.0 * steps_per_sec / lag as f32;
        // A gentle pull towards 120 breaks ties between a beat and its double.
        let prior = (-0.5 * ((bpm / 120.0).ln() / 0.9).powi(2)).exp();
        let score = sum / (onset.len() - lag) as f32 * prior;
        if best.is_none_or(|(_, s)| score > s) {
            best = Some((lag as f32, score));
        }
    }
    best.map(|(lag, _)| lag)
}

/// Beat times in seconds. Picks a tempo from the sound's rhythm, then finds
/// the chain of onsets that fit it best (dynamic programming, after Ellis,
/// "Beat Tracking by Dynamic Programming").
pub fn track_beats(samples: &[f32], rate: u32) -> Vec<f64> {
    let steps_per_sec = rate as f32 / HOP as f32;
    let onset = onset_strength(samples);
    let Some(period) = beat_period(&onset, steps_per_sec) else {
        return Vec::new();
    };
    let n = onset.len();
    let tightness = 60.0f32;
    let mut score = onset.clone();
    let mut back: Vec<Option<usize>> = vec![None; n];
    let (lo, hi) = ((period * 0.5) as usize, (period * 2.0) as usize);
    for t in 0..n {
        if t < lo.max(1) {
            continue;
        }
        let mut best: Option<(f32, usize)> = None;
        for gap in lo.max(1)..=hi.min(t) {
            let penalty = tightness * (gap as f32 / period).ln().powi(2);
            let s = score[t - gap] - penalty;
            if best.is_none_or(|(b, _)| s > b) {
                best = Some((s, t - gap));
            }
        }
        if let Some((s, from)) = best {
            // Starting fresh costs nothing: a chain needn't begin at zero.
            if s > 0.0 {
                score[t] = onset[t] + s;
                back[t] = Some(from);
            }
        }
    }
    // The chain ends at the best score near the end.
    let tail = n.saturating_sub(period as usize * 2);
    let Some(mut t) = (tail..n).max_by(|a, b| score[*a].total_cmp(&score[*b])) else {
        return Vec::new();
    };
    let mut steps = vec![t];
    while let Some(prev) = back[t] {
        steps.push(prev);
        t = prev;
    }
    steps.reverse();
    steps
        .into_iter()
        .map(|s| (s * HOP + WINDOW * 3 / 4) as f64 / f64::from(rate))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_log_becomes_pairs_in_source_time() {
        let log = "\
[silencedetect @ 0x1] silence_start: 1.5\n\
[silencedetect @ 0x1] silence_end: 2.75 | silence_duration: 1.25\n\
[silencedetect @ 0x1] silence_start: 6\n";
        let found = parse_silences(log, (10.0, 20.0));
        // Times are measured from the start of the stretch; the last one runs on.
        assert_eq!(found, vec![(11.5, 12.75), (16.0, 20.0)]);
        // A silence already going when the stretch began has no start line.
        let found = parse_silences("silence_end: 0.8 | silence_duration: 0.8", (0.0, 5.0));
        assert_eq!(found, vec![(0.0, 0.8)]);
    }

    #[test]
    fn scene_log_becomes_times() {
        let log = "\
[Parsed_showinfo_1 @ 0x1] n:   0 pts:  96000 pts_time:4 duration: 1\n\
[Parsed_showinfo_1 @ 0x1] n:   1 pts: 200000 pts_time:8.333 duration: 1\n\
[Parsed_showinfo_1 @ 0x1] n:   2 pts: 200100 pts_time:8.35 duration: 1\n\
other line pts_time:99\n";
        assert_eq!(parse_scene_times(log, 100.0), vec![104.0, 108.333]);
    }

    fn clicks(bpm: f64, secs: f64, rate: u32) -> Vec<f32> {
        let mut s = vec![0f32; (secs * f64::from(rate)) as usize];
        let gap = 60.0 / bpm;
        let mut t = 0.3;
        while t < secs - 0.1 {
            let at = (t * f64::from(rate)) as usize;
            // A short burst of noise-like tone: a drum hit.
            for k in 0..300 {
                let env = (-(k as f32) / 60.0).exp();
                if let Some(v) = s.get_mut(at + k) {
                    *v += env * (k as f32 * 1.7).sin() * 0.8;
                }
            }
            t += gap;
        }
        s
    }

    #[test]
    fn a_steady_beat_is_found_on_the_hits() {
        for bpm in [100.0, 120.0, 140.0] {
            let rate = 11_025;
            let samples = clicks(bpm, 16.0, rate);
            let beats = track_beats(&samples, rate);
            assert!(beats.len() >= 15, "{bpm} bpm: only {} beats", beats.len());
            let gap = 60.0 / bpm;
            // Nearly every beat sits within 60 ms of a real hit.
            let near = beats
                .iter()
                .filter(|b| {
                    let k = ((**b - 0.3) / gap).round();
                    (**b - (0.3 + k * gap)).abs() < 0.06
                })
                .count();
            assert!(
                near as f64 >= beats.len() as f64 * 0.9,
                "{bpm} bpm: {near} of {} on the hits",
                beats.len()
            );
        }
    }

    #[test]
    fn silence_and_noise_have_no_beat() {
        assert!(track_beats(&vec![0.0; 11_025 * 10], 11_025).is_empty());
        assert!(track_beats(&[], 11_025).is_empty());
    }

    #[test]
    fn ffmpeg_finds_silences_and_scenes() {
        let Ok(tools) = Tools::get() else {
            eprintln!("no ffmpeg here; skipping");
            return;
        };
        let dir = std::env::temp_dir().join(format!("tidycuts-detect-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 2 s of tone, 2 s of silence, 2 s of tone; the picture goes
        // red, then blue at 3 s.
        let file = dir.join("t.mkv");
        let status = tools
            .ffmpeg_cmd()
            .args(["-y", "-f", "lavfi", "-i", "color=c=red:s=64x64:d=3:r=25"])
            .args(["-f", "lavfi", "-i", "color=c=blue:s=64x64:d=3:r=25"])
            .args(["-f", "lavfi", "-i"])
            .arg("aevalsrc='if(between(t,2,4),0,0.5*sin(2*PI*440*t))':d=6:s=22050")
            .args(["-filter_complex", "[0:v][1:v]concat=n=2:v=1:a=0[v]"])
            .args([
                "-map",
                "[v]",
                "-map",
                "2:a",
                "-c:v",
                "mpeg4",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&file)
            .status()
            .unwrap();
        assert!(status.success(), "could not make the test file");
        let s = silences(tools, &file, (0.0, 6.0), -35.0, 0.5).unwrap();
        assert_eq!(s.len(), 1, "{s:?}");
        assert!(
            (s[0].0 - 2.0).abs() < 0.15 && (s[0].1 - 4.0).abs() < 0.15,
            "{s:?}"
        );
        let c = scene_changes(tools, &file, (0.0, 6.0), 0.3).unwrap();
        assert_eq!(c.len(), 1, "{c:?}");
        assert!((c[0] - 3.0).abs() < 0.2, "{c:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
