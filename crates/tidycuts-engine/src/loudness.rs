//! Loudness: how loud a sound is the way people hear it (ITU-R BS.1770 /
//! EBU R128, in LUFS). Two parts: a live "momentary" reading for the sound
//! meter, worked out here from the mixer's output, and a whole-file
//! measurement with ffmpeg that the Loudness window and the export's loudness
//! target use.

use std::collections::VecDeque;
use std::path::Path;

// ---- live reading ------------------------------------------------------------

/// A second-order filter section (direct form I).
#[derive(Clone, Copy, Default)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    fn run(&mut self, v: f64) -> f64 {
        let out = self.b[0] * v + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [v, self.x[0]];
        self.y = [out, self.y[0]];
        out
    }
}

/// The two "K-weighting" filters of BS.1770 at any sample rate: a shelf that
/// lifts the highs a little, then a high-pass at 38 Hz.
fn k_weighting(fs: f64) -> [Biquad; 2] {
    let shelf = {
        // The form used by libebur128; it reproduces the standard's 48 kHz table.
        let (f0, g, q) = (1681.974450955533, 3.999843853973347, 0.7071752369554196);
        let k = (std::f64::consts::PI * f0 / fs).tan();
        let vh = 10f64.powf(g / 20.0);
        let vb = vh.powf(0.4996667741545416);
        let a0 = 1.0 + k / q + k * k;
        Biquad {
            b: [
                (vh + vb * k / q + k * k) / a0,
                2.0 * (k * k - vh) / a0,
                (vh - vb * k / q + k * k) / a0,
            ],
            a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
            ..Default::default()
        }
    };
    let high_pass = {
        let (f0, q) = (38.13547087602444, 0.5003270373238773);
        let w0 = std::f64::consts::TAU * f0 / fs;
        let alpha = w0.sin() / (2.0 * q);
        let c = w0.cos();
        let a0 = 1.0 + alpha;
        Biquad {
            b: [(1.0 + c) / 2.0 / a0, -(1.0 + c) / a0, (1.0 + c) / 2.0 / a0],
            a: [-2.0 * c / a0, (1.0 - alpha) / a0],
            ..Default::default()
        }
    };
    [shelf, high_pass]
}

/// Momentary loudness (the last 400 ms) of a stereo stream, block by block.
pub struct Momentary {
    filters: [[Biquad; 2]; 2],
    /// Sum of squares of the weighted samples (both channels) and the number
    /// of sample frames, per block.
    blocks: VecDeque<(f64, usize)>,
    window: usize,
    sum: f64,
    count: usize,
}

impl Momentary {
    pub fn new(sample_rate: u32) -> Momentary {
        let f = k_weighting(sample_rate as f64);
        Momentary {
            filters: [f, f],
            blocks: VecDeque::new(),
            window: (sample_rate as f64 * 0.4) as usize,
            sum: 0.0,
            count: 0,
        }
    }

    /// Feeds interleaved stereo samples.
    pub fn push(&mut self, interleaved: &[f32]) {
        let mut acc = 0.0;
        let mut n = 0;
        for pair in interleaved.as_chunks::<2>().0 {
            for (ch, s) in pair.iter().enumerate() {
                let mut v = *s as f64;
                for f in &mut self.filters[ch] {
                    v = f.run(v);
                }
                acc += v * v;
            }
            n += 1;
        }
        if n == 0 {
            return;
        }
        self.blocks.push_back((acc, n));
        self.sum += acc;
        self.count += n;
        while self.count > self.window + self.blocks.front().map_or(0, |b| b.1) {
            let (s, c) = self.blocks.pop_front().unwrap();
            self.sum -= s;
            self.count -= c;
        }
    }

    /// The reading in LUFS, or `None` when it is quieter than -70.
    pub fn lufs(&self) -> Option<f32> {
        if self.count == 0 {
            return None;
        }
        // Mean square per channel, the two channels added (both weigh 1).
        let power = self.sum / self.count as f64;
        if power <= 0.0 {
            return None;
        }
        let l = -0.691 + 10.0 * power.log10();
        (l > -70.0).then_some(l as f32)
    }

    /// Forgets everything (after a jump in the timeline).
    pub fn reset(&mut self) {
        self.blocks.clear();
        self.sum = 0.0;
        self.count = 0;
    }
}

// ---- whole-file measurement -----------------------------------------------

/// How loud a finished sound is.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Integrated loudness, LUFS.
    pub integrated: f64,
    /// Highest peak between samples, dBTP.
    pub true_peak: f64,
    /// Loudness range, LU.
    pub range: f64,
}

/// Measures a file after the optional filters `pre` (loudnorm only reads).
fn run_loudnorm(path: &Path, pre: Option<&str>) -> Result<Report, String> {
    let tools = tidycuts_media::Tools::get().map_err(|e| e.to_string())?;
    // Plain command: the shared one turns the log level down, and loudnorm
    // prints its numbers at the info level.
    let mut cmd = std::process::Command::new(&tools.ffmpeg);
    cmd.args(["-hide_banner", "-nostdin", "-i"]).arg(path);
    cmd.args([
        "-af",
        &format!(
            "{}loudnorm=I=-14:TP=-1.5:LRA=11:print_format=json",
            pre.map(|p| format!("{p},")).unwrap_or_default()
        ),
        "-f",
        "null",
        "-",
    ]);
    cmd.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    parse_report(&String::from_utf8_lossy(&out.stderr))
}

/// Reads the numbers loudnorm prints at the end.
pub fn parse_report(text: &str) -> Result<Report, String> {
    let start = text.rfind('{').ok_or("no loudness numbers")?;
    let end = text.rfind('}').ok_or("no loudness numbers")?;
    let json: serde_json::Value =
        serde_json::from_str(&text[start..=end]).map_err(|e| e.to_string())?;
    let num = |k: &str| -> Result<f64, String> {
        json.get(k)
            .and_then(|v| v.as_str().and_then(|s| s.parse().ok()).or(v.as_f64()))
            .ok_or_else(|| format!("missing {k}"))
    };
    Ok(Report {
        integrated: num("input_i")?,
        true_peak: num("input_tp")?,
        range: num("input_lra")?,
    })
}

/// Measures a sound file.
pub fn measure_file(path: &Path) -> Result<Report, String> {
    run_loudnorm(path, None)
}

/// A gain followed by a limiter that keeps peaks around -1.5 dB.
fn gain_and_limit(gain_db: f64) -> String {
    format!("volume={gain_db:.2}dB,alimiter=limit=0.84:attack=3:release=50:level=0")
}

/// The ffmpeg filter that brings a sound file to `target` LUFS. The sound is
/// measured, a gain is worked out and a limiter keeps the peaks from
/// distorting; measuring through that chain corrects the difference the
/// limiter makes. One steady gain: no pumping.
pub fn normalize_filter(path: &Path, target: f64) -> Result<String, String> {
    let r = run_loudnorm(path, None)?;
    if r.integrated < -69.0 {
        // Silence: nothing to do.
        return Ok("anull".into());
    }
    let mut gain = target - r.integrated;
    // The limiter changes the loudness a little (a lot when a few loud peaks
    // carry most of it), so measure through the chain and correct, a few times.
    for _ in 0..3 {
        let after = run_loudnorm(path, Some(&gain_and_limit(gain)))?;
        let err = target - after.integrated;
        if err.abs() < 0.3 || after.integrated < -69.0 {
            break;
        }
        gain += err.clamp(-12.0, 12.0);
    }
    Ok(gain_and_limit(gain))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, amp: f32, secs: f32, sr: u32) -> Vec<f32> {
        let n = (secs * sr as f32) as usize;
        let mut v = Vec::with_capacity(n * 2);
        for i in 0..n {
            let s = amp * (std::f32::consts::TAU * freq * i as f32 / sr as f32).sin();
            v.extend([s, s]);
        }
        v
    }

    #[test]
    fn the_filters_match_the_published_48k_coefficients() {
        let [shelf, hp] = k_weighting(48_000.0);
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        // BS.1770-4, tables 1 and 2.
        assert!(near(shelf.b[0], 1.53512485958697));
        assert!(near(shelf.b[1], -2.69169618940638));
        assert!(near(shelf.b[2], 1.19839281085285));
        assert!(near(shelf.a[0], -1.69065929318241));
        assert!(near(shelf.a[1], 0.73248077421585));
        assert!(near(hp.a[0], -1.99004745483398));
        assert!(near(hp.a[1], 0.99007225036621));
    }

    #[test]
    fn a_1khz_tone_reads_as_the_standard_says() {
        // A full-scale stereo 1 kHz sine: mean square 0.5 per channel, two
        // channels, and the weighting adds about 0.7 dB at 1 kHz:
        // -0.691 + 10 log10(1.0) + 0.7 = about 0 LUFS.
        let mut m = Momentary::new(48_000);
        m.push(&tone(1000.0, 1.0, 1.0, 48_000));
        let l = m.lufs().unwrap();
        assert!(l.abs() < 0.3, "{l}");
    }

    #[test]
    fn halving_the_level_lowers_it_by_six_decibels() {
        let mut a = Momentary::new(44_100);
        let mut b = Momentary::new(44_100);
        a.push(&tone(1000.0, 0.5, 1.0, 44_100));
        b.push(&tone(1000.0, 0.25, 1.0, 44_100));
        let d = a.lufs().unwrap() - b.lufs().unwrap();
        assert!((d - 6.02).abs() < 0.1, "{d}");
    }

    #[test]
    fn silence_has_no_reading() {
        let mut m = Momentary::new(48_000);
        m.push(&vec![0.0; 96_000]);
        assert_eq!(m.lufs(), None);
        let mut q = Momentary::new(48_000);
        q.push(&tone(1000.0, 0.00001, 1.0, 48_000));
        assert_eq!(q.lufs(), None);
    }

    #[test]
    fn only_the_last_400_ms_count() {
        let mut m = Momentary::new(48_000);
        m.push(&tone(1000.0, 0.8, 1.0, 48_000));
        m.push(&tone(1000.0, 0.01, 0.6, 48_000));
        // Now only quiet sound is inside the window.
        assert!(m.lufs().unwrap() < -35.0);
    }

    #[test]
    fn it_reads_the_numbers_loudnorm_prints() {
        let text = r#"[Parsed_loudnorm_0 @ 0x1]
{
	"input_i" : "-23.40",
	"input_tp" : "-4.20",
	"input_lra" : "5.10",
	"input_thresh" : "-33.70",
	"output_i" : "-14.20",
	"target_offset" : "0.10"
}"#;
        let r = parse_report(text).unwrap();
        assert_eq!(r.integrated, -23.4);
        assert_eq!(r.true_peak, -4.2);
        assert_eq!(r.range, 5.1);
        assert!(parse_report("nothing").is_err());
    }
}
