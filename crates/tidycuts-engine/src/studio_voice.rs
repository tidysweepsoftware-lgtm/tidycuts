//! Studio Voice: the best voice clean-up TidyCuts has. A trained network
//! (MossFormer2, from ClearerVoice-Studio, Apache-2.0) looks at a few seconds
//! of speech at a time and decides, for every frequency and every instant,
//! how much of it is the voice. Everything else is turned down. It beat the
//! smaller engines in our tests on every kind of noise (see CHANGELOG).
//!
//! The network runs through ONNX Runtime, loaded from a file next to the
//! program. The feature preparation and the sound reconstruction around it
//! are written here, to match what the model was trained with:
//! 48 kHz sound, 40 ms Hamming windows every 8 ms, a 60-band mel filter bank
//! plus its two deltas in, a mask out that multiplies the spectrum.
//!
//! The sound is cut into 4-second windows that overlap by one second; each
//! window's middle three seconds are kept, so there are no seams.

use ort::session::Session;
use ort::value::TensorRef;
use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, Weak};

pub const SAMPLE_RATE: usize = 48_000;
/// One window of sound the network sees: 4 s.
pub const WINDOW: usize = 4 * SAMPLE_RATE;
/// Windows start every 3 s.
pub const STRIDE: usize = 3 * SAMPLE_RATE;
/// Each window's first and last half second are not trusted (little context).
const GIVE_UP: usize = (WINDOW - STRIDE) / 2;
const WIN_LEN: usize = 1920;
const HOP: usize = 384;
const BINS: usize = WIN_LEN / 2 + 1;
const MELS: usize = 60;
/// Frames in one window: (192000 - 1920) / 384 + 1.
pub const FRAMES: usize = (WINDOW - WIN_LEN) / HOP + 1;
/// The model works on 16-bit-sized numbers.
const SCALE: f32 = 32768.0;
const MODEL_NAME: &str = "mossformer2_se_48k.onnx";

// ---- finding the files ----------------------------------------------------

pub(crate) fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|p| p.to_path_buf())
}

/// Folders searched for the files that come with the program, nearest first:
/// next to the program, and where a Mac app keeps them.
pub(crate) fn search_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = exe_dir() {
        v.push(d.clone());
        if let Some(up) = d.parent() {
            v.push(up.join("Resources"));
            v.push(up.join("Frameworks"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        v.push(cwd);
    }
    v
}

/// The network file, if it is there (`TIDYCUTS_VOICE_MODEL` overrides).
pub fn model_file() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("TIDYCUTS_VOICE_MODEL") {
        let p = PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    search_dirs()
        .into_iter()
        .map(|d| d.join("models").join(MODEL_NAME))
        .find(|p| p.is_file())
}

/// The ONNX Runtime library, if it is there (`TIDYCUTS_ORT` overrides).
pub fn runtime_file() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("TIDYCUTS_ORT") {
        let p = PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    let name = if cfg!(target_os = "windows") {
        "onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    };
    search_dirs()
        .into_iter()
        .flat_map(|d| [d.join("onnxruntime").join(name), d.join(name)])
        .find(|p| p.is_file())
}

/// Whether Studio Voice can run on this computer.
pub fn available() -> bool {
    model_file().is_some() && runtime_file().is_some()
}

// ---- the signal processing around the network -------------------------------

fn hamming(n: usize) -> Vec<f32> {
    // Not periodic: the same as the model's training code.
    (0..n)
        .map(|i| 0.54 - 0.46 * (std::f32::consts::TAU * i as f32 / (n - 1) as f32).cos())
        .collect()
}

fn mel_scale(f: f64) -> f64 {
    1127.0 * (1.0 + f / 700.0).ln()
}

/// Kaldi-style triangular filters: `MELS` rows over the power spectrum of a
/// 2048-point FFT (1025 bins), from 20 Hz to the top of the band.
fn mel_filters() -> Vec<Vec<f32>> {
    let fft = 2048usize;
    let sr = SAMPLE_RATE as f64;
    let (low, high) = (20.0, sr / 2.0);
    let (mel_low, mel_high) = (mel_scale(low), mel_scale(high));
    let delta = (mel_high - mel_low) / (MELS + 1) as f64;
    let bin_width = sr / fft as f64;
    (0..MELS)
        .map(|b| {
            let left = mel_low + b as f64 * delta;
            let center = mel_low + (b + 1) as f64 * delta;
            let right = mel_low + (b + 2) as f64 * delta;
            let mut row = vec![0.0f32; fft / 2 + 1];
            for (k, v) in row.iter_mut().enumerate().take(fft / 2) {
                let mel = mel_scale(bin_width * k as f64);
                let up = (mel - left) / (center - left);
                let down = (right - mel) / (right - center);
                *v = up.min(down).max(0.0) as f32;
            }
            row
        })
        .collect()
}

/// A small, repeatable source of normally distributed numbers.
struct Gauss(u64);

impl Gauss {
    fn next_uniform(&mut self) -> f32 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let v = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
        ((v >> 40) as f32 + 0.5) / (1u32 << 24) as f32
    }

    fn next(&mut self) -> f32 {
        let (a, b) = (self.next_uniform(), self.next_uniform());
        (-2.0 * a.ln()).sqrt() * (std::f32::consts::TAU * b).cos()
    }
}

/// Everything that is the same for every window.
struct Dsp {
    window: Vec<f32>,
    mel: Vec<Vec<f32>>,
    fbank_fft: Arc<dyn RealToComplex<f32>>,
    fwd: Arc<dyn RealToComplex<f32>>,
    inv: Arc<dyn ComplexToReal<f32>>,
    /// 1 / sum of squared window over the frames that touch each sample.
    envelope: Vec<f32>,
}

impl Dsp {
    fn new() -> Dsp {
        let mut planner = RealFftPlanner::<f32>::new();
        let window = hamming(WIN_LEN);
        let mut envelope = vec![0.0f32; WINDOW];
        for t in 0..FRAMES {
            for (i, w) in window.iter().enumerate() {
                envelope[t * HOP + i] += w * w;
            }
        }
        for e in &mut envelope {
            *e = if *e > 1e-11 { 1.0 / *e } else { 0.0 };
        }
        Dsp {
            window,
            mel: mel_filters(),
            fbank_fft: planner.plan_fft_forward(2048),
            fwd: planner.plan_fft_forward(WIN_LEN),
            inv: planner.plan_fft_inverse(WIN_LEN),
            envelope,
        }
    }

    /// The network's input for one window of sound already scaled by 32768:
    /// `FRAMES` rows of 60 log-mel values, their delta and delta-delta.
    fn features(&self, x: &[f32]) -> Vec<f32> {
        self.features_with(x, 1.0)
    }

    /// `dither` is the size of the whisper of noise added before the filters
    /// (the model was trained with 1.0).
    fn features_with(&self, x: &[f32], dither: f32) -> Vec<f32> {
        debug_assert_eq!(x.len(), WINDOW);
        let mut gauss = Gauss(0x9E37_79B9_7F4A_7C15);
        let mut fbank = vec![0.0f32; FRAMES * MELS];
        let mut frame = vec![0.0f32; 2048];
        let mut spec = vec![Complex32::new(0.0, 0.0); 1025];
        let mut scratch = self.fbank_fft.make_scratch_vec();
        for t in 0..FRAMES {
            let src = &x[t * HOP..t * HOP + WIN_LEN];
            // Dither: a whisper of noise so silence has a floor.
            let mut f: Vec<f32> = src.iter().map(|v| v + dither * gauss.next()).collect();
            let mean = f.iter().sum::<f32>() / WIN_LEN as f32;
            f.iter_mut().for_each(|v| *v -= mean);
            // Pre-emphasis (the first sample is mirrored).
            let mut pre = vec![0.0f32; WIN_LEN];
            pre[0] = f[0] - 0.97 * f[0];
            for i in 1..WIN_LEN {
                pre[i] = f[i] - 0.97 * f[i - 1];
            }
            frame.iter_mut().for_each(|v| *v = 0.0);
            for i in 0..WIN_LEN {
                frame[i] = pre[i] * self.window[i];
            }
            let _ = self
                .fbank_fft
                .process_with_scratch(&mut frame, &mut spec, &mut scratch);
            let power: Vec<f32> = spec.iter().map(|c| c.re * c.re + c.im * c.im).collect();
            for (m, row) in self.mel.iter().enumerate() {
                let e: f32 = row.iter().zip(&power).map(|(a, b)| a * b).sum();
                fbank[t * MELS + m] = e.max(f32::EPSILON).ln();
            }
        }
        let d1 = deltas(&fbank);
        let d2 = deltas(&d1);
        let mut out = Vec::with_capacity(FRAMES * MELS * 3);
        for t in 0..FRAMES {
            out.extend_from_slice(&fbank[t * MELS..(t + 1) * MELS]);
            out.extend_from_slice(&d1[t * MELS..(t + 1) * MELS]);
            out.extend_from_slice(&d2[t * MELS..(t + 1) * MELS]);
        }
        out
    }

    /// Multiplies the window's spectrum by the mask and turns it back into
    /// sound. `mask` is `FRAMES` rows of `BINS` values.
    fn apply_mask(&self, x: &[f32], mask: &[f32]) -> Vec<f32> {
        let mut out = vec![0.0f32; WINDOW];
        let mut frame = vec![0.0f32; WIN_LEN];
        let mut spec = vec![Complex32::new(0.0, 0.0); BINS];
        let mut scratch_f = self.fwd.make_scratch_vec();
        let mut scratch_i = self.inv.make_scratch_vec();
        for t in 0..FRAMES {
            for i in 0..WIN_LEN {
                frame[i] = x[t * HOP + i] * self.window[i];
            }
            let _ = self
                .fwd
                .process_with_scratch(&mut frame, &mut spec, &mut scratch_f);
            for (k, c) in spec.iter_mut().enumerate() {
                *c *= mask[t * BINS + k];
            }
            // The inverse wants real DC and Nyquist parts.
            spec[0].im = 0.0;
            spec[BINS - 1].im = 0.0;
            let _ = self
                .inv
                .process_with_scratch(&mut spec, &mut frame, &mut scratch_i);
            for i in 0..WIN_LEN {
                out[t * HOP + i] += frame[i] / WIN_LEN as f32 * self.window[i];
            }
        }
        for (o, e) in out.iter_mut().zip(&self.envelope) {
            *o *= e;
        }
        out
    }
}

/// Regression slope over five frames (the same as the training code's
/// `compute_deltas`), with the ends repeated.
fn deltas(x: &[f32]) -> Vec<f32> {
    let frames = x.len() / MELS;
    let at = |t: isize, m: usize| x[(t.clamp(0, frames as isize - 1) as usize) * MELS + m];
    let mut out = vec![0.0f32; x.len()];
    for t in 0..frames {
        for m in 0..MELS {
            let mut acc = 0.0;
            for k in 1..=2isize {
                acc += k as f32 * (at(t as isize + k, m) - at(t as isize - k, m));
            }
            out[t * MELS + m] = acc / 10.0;
        }
    }
    out
}

// ---- the network ----------------------------------------------------------

/// A loaded network. Cheap to share; make one per clean-up job.
pub struct Studio {
    session: Mutex<Session>,
    dsp: Dsp,
}

fn runtime_ready(lib: &std::path::Path) -> Result<(), String> {
    static INIT: OnceLock<Result<(), String>> = OnceLock::new();
    INIT.get_or_init(|| {
        let builder = ort::init_from(lib).map_err(|e| format!("onnxruntime: {e}"))?;
        builder.commit();
        Ok(())
    })
    .clone()
}

fn shared() -> &'static Mutex<Weak<Studio>> {
    static S: OnceLock<Mutex<Weak<Studio>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Weak::new()))
}

impl Studio {
    /// The loaded network, shared by everything that is using it now.
    pub fn load() -> Result<Arc<Studio>, String> {
        let mut slot = shared().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = slot.upgrade() {
            return Ok(s);
        }
        let model = model_file().ok_or("the voice model is not installed")?;
        let lib = runtime_file().ok_or("ONNX Runtime is not installed")?;
        runtime_ready(&lib)?;
        let threads = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).clamp(1, 8))
            .unwrap_or(2);
        let load = || -> ort::Result<Session> {
            let mut builder = Session::builder()?
                .with_intra_threads(threads)
                .map_err(|e| ort::Error::new(e.to_string()))?;
            builder.commit_from_file(&model)
        };
        let session = load().map_err(|e| format!("loading the voice model: {e}"))?;
        let studio = Arc::new(Studio {
            session: Mutex::new(session),
            dsp: Dsp::new(),
        });
        *slot = Arc::downgrade(&studio);
        Ok(studio)
    }

    /// Cleans one window of sound (exactly `WINDOW` samples, -1..1 scale).
    /// `gain` lifts a quiet recording into the range the network was trained
    /// on and is taken off again afterwards.
    pub fn enhance_window(&self, x: &[f32], gain: f32) -> Result<Vec<f32>, String> {
        let scaled: Vec<f32> = x.iter().map(|v| v * gain * SCALE).collect();
        let feats = self.dsp.features(&scaled);
        let mask: Vec<f32> = {
            let mut session = self.session.lock().unwrap_or_else(|e| e.into_inner());
            let input = TensorRef::from_array_view(([1usize, FRAMES, MELS * 3], &*feats))
                .map_err(|e| e.to_string())?;
            let outputs = session
                .run(ort::inputs!["fbank" => input])
                .map_err(|e| e.to_string())?;
            let (_, data) = outputs["mask"]
                .try_extract_tensor::<f32>()
                .map_err(|e| e.to_string())?;
            data.to_vec()
        };
        if mask.len() != FRAMES * BINS {
            return Err(format!("the voice model gave {} values", mask.len()));
        }
        let out = self.dsp.apply_mask(&scaled, &mask);
        let back = 1.0 / (gain * SCALE);
        Ok(out
            .into_iter()
            .map(|v| {
                let v = v * back;
                if v.is_finite() {
                    v.clamp(-1.5, 1.5)
                } else {
                    0.0
                }
            })
            .collect())
    }
}

/// Cleans stereo sound as it arrives, a window at a time. Windows where both
/// channels are the same (a mono recording) are cleaned once.
pub struct StudioStream {
    studio: Arc<Studio>,
    gain: f32,
    mix: f32,
    /// Input not yet finished, per channel; `base` is where it starts.
    input: [Vec<f32>; 2],
    /// Cleaned sound for the same stretch (filled by whole windows).
    wet: [Vec<f32>; 2],
    base: usize,
    /// How far (absolute sample) the cleaned sound is final.
    done: usize,
    total: usize,
    next_window: usize,
    /// How far (absolute sample) the sound has been handed out.
    emitted: usize,
}

impl StudioStream {
    /// `mix` is how much of the cleaned sound to use, 0..=1.
    pub fn new(studio: Arc<Studio>, gain: f32, mix: f32) -> StudioStream {
        StudioStream {
            studio,
            gain,
            mix: mix.clamp(0.0, 1.0),
            input: [Vec::new(), Vec::new()],
            wet: [Vec::new(), Vec::new()],
            base: 0,
            done: 0,
            total: 0,
            next_window: 0,
            emitted: 0,
        }
    }

    fn run_window(&mut self, start: usize, last: bool) -> Result<(), String> {
        let mut win = [vec![0.0f32; WINDOW], vec![0.0f32; WINDOW]];
        let from = start - self.base;
        for (w, input) in win.iter_mut().zip(&self.input) {
            let have = input.len().saturating_sub(from).min(WINDOW);
            w[..have].copy_from_slice(&input[from..from + have]);
        }
        let same = win[0] == win[1];
        let a = self.studio.enhance_window(&win[0], self.gain)?;
        let b = if same {
            a.clone()
        } else {
            self.studio.enhance_window(&win[1], self.gain)?
        };
        // Which part of this window is final.
        let (from, to) = if last {
            (if start == 0 { 0 } else { GIVE_UP }, WINDOW)
        } else {
            (if start == 0 { 0 } else { GIVE_UP }, WINDOW - GIVE_UP)
        };
        for (ch, out) in [a, b].into_iter().enumerate() {
            let need = start + to - self.base;
            if self.wet[ch].len() < need {
                self.wet[ch].resize(need, 0.0);
            }
            let at = start - self.base;
            self.wet[ch][at + from..at + to].copy_from_slice(&out[from..to]);
        }
        self.done = start + to;
        Ok(())
    }

    /// Takes the sound that has arrived and returns the cleaned sound that is
    /// ready (interleaved stereo), mixed with the original.
    pub fn push(&mut self, interleaved: &[f32]) -> Result<Vec<f32>, String> {
        for pair in interleaved.as_chunks::<2>().0 {
            self.input[0].push(pair[0]);
            self.input[1].push(pair[1]);
        }
        self.total += interleaved.len() / 2;
        // A window can run when all of its sound is here.
        while self.next_window * STRIDE + WINDOW <= self.total {
            self.run_window(self.next_window * STRIDE, false)?;
            self.next_window += 1;
        }
        Ok(self.drain(self.done.min(self.total)))
    }

    /// The end of the sound: cleans what is left.
    pub fn finish(&mut self) -> Result<Vec<f32>, String> {
        if self.total > 0 && self.done < self.total {
            self.run_window(self.next_window * STRIDE, true)?;
        }
        Ok(self.drain(self.total))
    }

    /// Mixes and hands out the final sound up to absolute sample `upto`, and
    /// forgets sound that no later window needs.
    fn drain(&mut self, upto: usize) -> Vec<f32> {
        let mut out = Vec::new();
        if upto > self.emitted {
            out.reserve((upto - self.emitted) * 2);
            for i in self.emitted..upto {
                for ch in 0..2 {
                    let dry = self.input[ch][i - self.base];
                    let wet = self.wet[ch][i - self.base];
                    out.push(dry + (wet - dry) * self.mix);
                }
            }
            self.emitted = upto;
        }
        // Later windows start at next_window * STRIDE; earlier sound that has
        // also been handed out is not needed any more.
        let keep_from = (self.next_window * STRIDE).min(self.emitted);
        if keep_from > self.base {
            let cut = keep_from - self.base;
            for ch in 0..2 {
                self.input[ch].drain(..cut.min(self.input[ch].len()));
                self.wet[ch].drain(..cut.min(self.wet[ch].len()));
            }
            self.base = keep_from;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(dir: &std::path::Path, name: &str) -> Vec<f32> {
        std::fs::read(dir.join(name))
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect()
    }

    fn max_diff(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len());
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f32::max)
    }

    #[test]
    fn deltas_of_a_ramp_are_its_slope() {
        let x: Vec<f32> = (0..20)
            .flat_map(|t| std::iter::repeat_n(t as f32 * 2.0, MELS))
            .collect();
        let d = deltas(&x);
        // Away from the ends the slope is exactly 2 per frame.
        assert!((d[10 * MELS] - 2.0).abs() < 1e-5);
        // The ends repeat the first and last frame, so the slope shrinks.
        assert!(d[0] < 2.0 && d[0] > 0.0);
    }

    #[test]
    fn mel_filters_cover_the_band_without_gaps() {
        let m = mel_filters();
        assert_eq!(m.len(), MELS);
        assert!(m.iter().all(|r| r.len() == 1025));
        // Every row has some weight, and weights are between 0 and 1.
        assert!(m.iter().all(|r| r.iter().sum::<f32>() > 0.0));
        assert!(m.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
    }

    #[test]
    fn the_window_count_matches_the_model() {
        assert_eq!(FRAMES, 496);
        assert_eq!(BINS, 961);
    }

    /// Compares with numbers made by the model's own Python code. The files
    /// are not in the repository (they are 1 MB each); point `TIDYCUTS_VECTORS`
    /// at a folder with `audio.bin`, `feat.bin`, `mask.bin` and `out.bin`.
    #[test]
    fn matches_the_reference_implementation() {
        let Some(dir) = std::env::var_os("TIDYCUTS_VECTORS") else {
            eprintln!("TIDYCUTS_VECTORS not set; skipping");
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        let (audio, feat, mask, out) = (
            read(&dir, "audio.bin"),
            read(&dir, "feat.bin"),
            read(&dir, "mask.bin"),
            read(&dir, "out.bin"),
        );
        let dsp = Dsp::new();
        let mine = dsp.features_with(&audio, 0.0);
        let d = max_diff(&mine, &feat);
        assert!(d < 2e-3, "features differ by {d}");
        let rebuilt = dsp.apply_mask(&audio, &mask);
        let scale = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let d = max_diff(&rebuilt, &out);
        assert!(d < scale * 1e-3, "sound differs by {d} (peak {scale})");
    }

    /// Needs the model and ONNX Runtime (`TIDYCUTS_VOICE_MODEL`, `TIDYCUTS_ORT`);
    /// skipped when they are not there.
    #[test]
    fn streaming_gives_the_same_sound_however_it_is_cut_up() {
        if !available() {
            eprintln!("Studio Voice files not found; skipping");
            return;
        }
        let studio = Studio::load().unwrap();
        // About 11 seconds: three windows, with a stereo difference in the middle.
        let n = 11 * SAMPLE_RATE + 1234;
        let mut x = Vec::with_capacity(n * 2);
        let mut phase = 0.0f32;
        for i in 0..n {
            let t = i as f32 / SAMPLE_RATE as f32;
            phase += (140.0 + 60.0 * (t * 0.7).sin()) / SAMPLE_RATE as f32;
            let v = 0.2 * (phase * std::f32::consts::TAU).sin() * (0.5 + 0.5 * (t * 3.0).sin());
            let side = if (4.0..6.0).contains(&t) {
                0.05 * (t * 900.0).sin()
            } else {
                0.0
            };
            x.extend([v + side, v - side]);
        }
        let run = |chunk: usize| {
            let mut s = StudioStream::new(studio.clone(), 1.0, 1.0);
            let mut out = Vec::new();
            for c in x.chunks(chunk) {
                out.extend(s.push(c).unwrap());
            }
            out.extend(s.finish().unwrap());
            out
        };
        let a = run(2 * 96_000);
        let b = run(2 * 7_001);
        assert_eq!(a.len(), x.len());
        assert_eq!(a, b);
        assert!(a.iter().all(|v| v.is_finite()));
        // No jump where one window hands over to the next (3.5 s and 6.5 s).
        for edge in [3 * SAMPLE_RATE + GIVE_UP, 6 * SAMPLE_RATE + GIVE_UP] {
            let step = (a[edge * 2] - a[(edge - 1) * 2]).abs();
            let typical = a[(edge - 200) * 2..edge * 2]
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0f32, f32::max);
            assert!(step <= typical * 4.0 + 1e-3, "jump {step} at {edge}");
        }
    }
}
