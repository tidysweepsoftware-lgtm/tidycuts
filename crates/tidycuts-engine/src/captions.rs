//! Automatic captions: speech turned into timed text, offline.
//!
//! A speech-recognition network (NVIDIA NeMo FastConformer, CC-BY-4.0, run
//! through ONNX Runtime) reads 16 kHz sound as 80-band log-mel features and
//! says, for every 80 ms, which piece of a word it hears or "nothing". Reading
//! those answers gives the words and when they were said; the words are then
//! grouped into caption lines.
//!
//! Two networks can be installed: an English one (more accurate) and one
//! for ten European languages (English, German, French, Spanish, Italian,
//! Polish, Russian, Ukrainian, Belarusian, Croatian).

use crate::studio_voice::{ensure_runtime, search_dirs};
use ort::session::Session;
use ort::value::Tensor;
use realfft::num_complex::Complex32;
use realfft::{RealFftPlanner, RealToComplex};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub const SAMPLE_RATE: usize = 16_000;
const N_FFT: usize = 512;
const WIN: usize = 400;
const HOP: usize = 160;
const MELS: usize = 80;
/// The network answers once per 8 feature frames: 80 ms.
const SUBSAMPLING: usize = 8;
const FRAME_SECS: f64 = (HOP * SUBSAMPLING) as f64 / SAMPLE_RATE as f64;
/// Sound is read in pieces of about this many seconds, cut at quiet moments.
const CHUNK_SECS: f64 = 20.0;
/// Longest sound that is transcribed.
const MAX_SECS: f64 = 4.0 * 3600.0;
const WORD_MARK: char = '\u{2581}';

/// Which network to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    English,
    Multilingual,
}

impl Model {
    pub fn label(self) -> &'static str {
        match self {
            Model::English => "English",
            Model::Multilingual => {
                "Other languages (German, French, Spanish, Italian, Polish, Russian, Ukrainian…)"
            }
        }
    }

    fn folder(self) -> &'static str {
        match self {
            Model::English => "asr-en",
            Model::Multilingual => "asr-multi",
        }
    }
}

/// The folder holding a network's files, if it is installed.
pub fn model_dir(m: Model) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(match m {
        Model::English => "TIDYCUTS_ASR_EN",
        Model::Multilingual => "TIDYCUTS_ASR_MULTI",
    }) {
        let p = PathBuf::from(p);
        return p.join("model.int8.onnx").is_file().then_some(p);
    }
    search_dirs()
        .into_iter()
        .map(|d| d.join("models").join(m.folder()))
        .find(|d| d.join("model.int8.onnx").is_file() && d.join("tokens.txt").is_file())
}

/// The networks that can be used now (nothing when ONNX Runtime is missing).
pub fn available_models() -> Vec<Model> {
    if crate::studio_voice::runtime_file().is_none() {
        return Vec::new();
    }
    [Model::English, Model::Multilingual]
        .into_iter()
        .filter(|m| model_dir(*m).is_some())
        .collect()
}

// ---- features -----------------------------------------------------------------

fn hz_to_mel(f: f64) -> f64 {
    // The "Slaney" scale: linear to 1 kHz, then logarithmic.
    let (f_sp, min_log_hz) = (200.0 / 3.0, 1000.0);
    let min_log_mel = min_log_hz / f_sp;
    let logstep = (6.4f64).ln() / 27.0;
    if f >= min_log_hz {
        min_log_mel + (f / min_log_hz).ln() / logstep
    } else {
        f / f_sp
    }
}

fn mel_to_hz(m: f64) -> f64 {
    let (f_sp, min_log_hz) = (200.0 / 3.0, 1000.0);
    let min_log_mel = min_log_hz / f_sp;
    let logstep = (6.4f64).ln() / 27.0;
    if m >= min_log_mel {
        min_log_hz * (logstep * (m - min_log_mel)).exp()
    } else {
        f_sp * m
    }
}

/// `MELS` triangular filters over the `N_FFT / 2 + 1` bins, area-normalised.
fn mel_filters() -> Vec<Vec<f32>> {
    let bins = N_FFT / 2 + 1;
    let fmax = SAMPLE_RATE as f64 / 2.0;
    let (mlo, mhi) = (hz_to_mel(0.0), hz_to_mel(fmax));
    let points: Vec<f64> = (0..MELS + 2)
        .map(|i| mel_to_hz(mlo + (mhi - mlo) * i as f64 / (MELS + 1) as f64))
        .collect();
    let freqs: Vec<f64> = (0..bins)
        .map(|k| k as f64 * SAMPLE_RATE as f64 / N_FFT as f64)
        .collect();
    (0..MELS)
        .map(|m| {
            let (lo, mid, hi) = (points[m], points[m + 1], points[m + 2]);
            let norm = 2.0 / (hi - lo);
            freqs
                .iter()
                .map(|f| {
                    let up = (f - lo) / (mid - lo);
                    let down = (hi - f) / (hi - mid);
                    (up.min(down).max(0.0) * norm) as f32
                })
                .collect()
        })
        .collect()
}

/// 80 log-mel rows by `frames` columns for 16 kHz sound, each row scaled to
/// zero mean and unit spread over the piece. Returned row by row.
pub fn features(samples: &[f32]) -> (Vec<f32>, usize) {
    let pad = N_FFT / 2;
    let mut x = vec![0.0f32; samples.len() + 2 * pad];
    // Pre-emphasis.
    for (i, s) in samples.iter().enumerate() {
        let prev = if i == 0 { 0.0 } else { samples[i - 1] };
        x[pad + i] = if i == 0 { *s } else { *s - 0.97 * prev };
    }
    let frames = 1 + samples.len() / HOP;
    let mut planner = RealFftPlanner::<f32>::new();
    let fft: Arc<dyn RealToComplex<f32>> = planner.plan_fft_forward(N_FFT);
    // A periodic Hann window of 400 samples, centred in the 512.
    let mut window = vec![0.0f32; N_FFT];
    let left = (N_FFT - WIN) / 2;
    for i in 0..WIN {
        window[left + i] = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / WIN as f32).cos();
    }
    let mel = mel_filters();
    let mut out = vec![0.0f32; MELS * frames];
    let mut frame = vec![0.0f32; N_FFT];
    let mut spec = vec![Complex32::new(0.0, 0.0); N_FFT / 2 + 1];
    let mut scratch = fft.make_scratch_vec();
    for t in 0..frames {
        let src = &x[t * HOP..t * HOP + N_FFT];
        for i in 0..N_FFT {
            frame[i] = src[i] * window[i];
        }
        let _ = fft.process_with_scratch(&mut frame, &mut spec, &mut scratch);
        let power: Vec<f32> = spec.iter().map(|c| c.re * c.re + c.im * c.im).collect();
        for (m, row) in mel.iter().enumerate() {
            let e: f32 = row.iter().zip(&power).map(|(a, b)| a * b).sum();
            out[m * frames + t] = (e + 2f32.powi(-24)).ln();
        }
    }
    // Per-row normalisation (sample standard deviation).
    for m in 0..MELS {
        let row = &mut out[m * frames..(m + 1) * frames];
        let mean = row.iter().sum::<f32>() / frames as f32;
        let var =
            row.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / (frames.max(2) - 1) as f32;
        let sd = var.sqrt() + 1e-5;
        for v in row.iter_mut() {
            *v = (*v - mean) / sd;
        }
    }
    (out, frames)
}

// ---- reading the network's answers ------------------------------------------

/// A word and when it was said, in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

/// Reads greedy CTC output: for every step, the id the network found most
/// likely. `blank` is the "nothing" id. Returns words with times relative to
/// the start of the piece.
pub fn words_from_ids(ids: &[usize], tokens: &[String], blank: usize) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    let mut prev = usize::MAX;
    // The step where the last piece of the current word was heard.
    let mut last_step = 0usize;
    for (step, &id) in ids.iter().enumerate() {
        if id != prev && id != blank && id < tokens.len() {
            let piece = &tokens[id];
            let starts_word = piece.starts_with(WORD_MARK);
            let text = piece.trim_start_matches(WORD_MARK);
            // Punctuation and word endings join the word before.
            let joins = !starts_word && !words.is_empty();
            if joins {
                let w = words.last_mut().unwrap();
                w.text.push_str(text);
                w.end = (step + 1) as f64 * FRAME_SECS;
            } else if !text.is_empty() {
                words.push(Word {
                    text: text.to_string(),
                    start: step as f64 * FRAME_SECS,
                    end: (step + 1) as f64 * FRAME_SECS,
                });
            }
            last_step = step;
        }
        prev = id;
    }
    let _ = last_step;
    // A word ends a little after its last piece, but not after the next word begins.
    for i in 0..words.len() {
        let limit = words.get(i + 1).map_or(f64::MAX, |n| n.start);
        words[i].end = (words[i].end + 0.12)
            .min(limit)
            .max(words[i].start + FRAME_SECS);
    }
    words
}

// ---- the recogniser ---------------------------------------------------------

pub struct Recognizer {
    session: Mutex<Session>,
    tokens: Vec<String>,
    blank: usize,
}

impl Recognizer {
    pub fn load(model: Model) -> Result<Recognizer, String> {
        let dir = model_dir(model).ok_or("the speech model is not installed")?;
        ensure_runtime()?;
        let tokens: Vec<String> = std::fs::read_to_string(dir.join("tokens.txt"))
            .map_err(|e| e.to_string())?
            .lines()
            .map(|l| l.rsplit_once(' ').map_or(l, |(t, _)| t).to_string())
            .collect();
        let threads = std::thread::available_parallelism()
            .map(|n| (n.get() / 2).clamp(1, 8))
            .unwrap_or(2);
        let load = || -> ort::Result<Session> {
            let mut b = Session::builder()?
                .with_intra_threads(threads)
                .map_err(|e| ort::Error::new(e.to_string()))?;
            b.commit_from_file(dir.join("model.int8.onnx"))
        };
        let session = load().map_err(|e| format!("loading the speech model: {e}"))?;
        Ok(Recognizer {
            session: Mutex::new(session),
            tokens,
            blank: usize::MAX,
        })
    }

    /// The words in a piece of 16 kHz sound (times from the piece's start).
    pub fn transcribe_piece(&self, samples: &[f32]) -> Result<Vec<Word>, String> {
        if samples.len() < SAMPLE_RATE / 4 {
            return Ok(Vec::new());
        }
        let (feats, frames) = features(samples);
        let input =
            Tensor::from_array(([1usize, MELS, frames], feats)).map_err(|e| e.to_string())?;
        let length =
            Tensor::from_array(([1usize], vec![frames as i64])).map_err(|e| e.to_string())?;
        let mut session = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let outputs = session
            .run(ort::inputs!["audio_signal" => input, "length" => length])
            .map_err(|e| e.to_string())?;
        let (shape, data) = outputs["logprobs"]
            .try_extract_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        let steps = shape[1] as usize;
        let classes = shape[2] as usize;
        let blank = if self.blank == usize::MAX {
            classes - 1
        } else {
            self.blank
        };
        let ids: Vec<usize> = (0..steps)
            .map(|t| {
                let row = &data[t * classes..(t + 1) * classes];
                row.iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .map_or(blank, |(i, _)| i)
            })
            .collect();
        Ok(words_from_ids(&ids, &self.tokens, blank))
    }
}

/// Cuts sound into pieces of about `CHUNK_SECS`, each ending at the quietest
/// moment near where it would end. Returns sample ranges.
pub fn pieces(samples: &[f32]) -> Vec<(usize, usize)> {
    let frame = SAMPLE_RATE / 50; // 20 ms
    let rms: Vec<f32> = samples
        .chunks(frame)
        .map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt())
        .collect();
    let target = (CHUNK_SECS * 50.0) as usize;
    let reach = 5 * 50;
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < rms.len() {
        if rms.len() - start <= target + reach {
            out.push((start * frame, samples.len()));
            break;
        }
        let from = start + target - reach;
        let to = start + target + reach;
        let cut = (from..to)
            .min_by(|a, b| rms[*a].total_cmp(&rms[*b]))
            .unwrap_or(start + target);
        out.push((start * frame, cut * frame));
        start = cut;
    }
    out
}

/// Reads 16 kHz mono sound from a media file.
fn read_sound(path: &Path, stream: u32, range: (f64, f64)) -> Result<Vec<f32>, String> {
    let tools = tidycuts_media::Tools::get().map_err(|e| e.to_string())?;
    let mut cmd = tools.ffmpeg_cmd();
    cmd.args(["-nostdin", "-ss", &format!("{:.6}", range.0.max(0.0))]);
    cmd.args([
        "-t",
        &format!("{:.6}", (range.1 - range.0).clamp(0.1, MAX_SECS)),
    ]);
    cmd.arg("-i").arg(path);
    cmd.args([
        "-vn",
        "-sn",
        "-dn",
        "-map",
        &format!("0:a:{stream}?"),
        "-ac",
        "1",
        "-ar",
        "16000",
        "-f",
        "f32le",
        "pipe:1",
    ]);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let mut child = crate::decode::KillOnDrop(cmd.spawn().map_err(|e| e.to_string())?);
    let mut bytes = Vec::new();
    child
        .0
        .stdout
        .take()
        .ok_or("no pipe")?
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect())
}

/// Finds the words spoken in `range` (source seconds) of a media file's sound.
/// Times in the answer are source seconds. `progress` gets 0..1.
pub fn transcribe(
    path: &Path,
    stream: u32,
    range: (f64, f64),
    model: Model,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f64),
) -> Result<Vec<Word>, String> {
    let rec = Recognizer::load(model)?;
    let samples = read_sound(path, stream, range)?;
    if samples.is_empty() {
        return Err("This clip has no sound to listen to.".into());
    }
    let mut words = Vec::new();
    let pieces = pieces(&samples);
    for (i, (a, b)) in pieces.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled.".into());
        }
        let piece = &samples[*a..*b];
        // Skip stretches that are nearly silent.
        let rms = (piece.iter().map(|v| v * v).sum::<f32>() / piece.len().max(1) as f32).sqrt();
        if rms > 1e-4 {
            let offset = range.0 + *a as f64 / SAMPLE_RATE as f64;
            for mut w in rec.transcribe_piece(piece)? {
                w.start += offset;
                w.end += offset;
                words.push(w);
            }
        }
        progress((i + 1) as f64 / pieces.len() as f64);
    }
    Ok(words)
}

// ---- caption lines ------------------------------------------------------------

/// One caption: the text for the screen and when to show it.
#[derive(Debug, Clone, PartialEq)]
pub struct Caption {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

/// How words are grouped into captions.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    /// Most characters on one line.
    pub max_chars: usize,
    /// Longest time one caption stays up.
    pub max_secs: f64,
    /// A pause this long starts a new caption.
    pub pause: f64,
}

impl Default for Layout {
    fn default() -> Layout {
        Layout {
            max_chars: 32,
            max_secs: 5.0,
            pause: 0.7,
        }
    }
}

fn ends_sentence(w: &str) -> bool {
    w.ends_with(['.', '?', '!', '…'])
}

/// Splits a long line at the space nearest its middle, so two lines are even.
fn balance(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars * 6 / 10 {
        return text.to_string();
    }
    let mid = text.chars().count() / 2;
    let split = text
        .char_indices()
        .filter(|(_, c)| *c == ' ')
        .min_by_key(|(i, _)| (text[..*i].chars().count() as isize - mid as isize).abs());
    match split {
        Some((i, _)) => format!("{}\n{}", &text[..i], &text[i + 1..]),
        None => text.to_string(),
    }
}

/// Groups words into caption lines with comfortable timing.
pub fn group_captions(words: &[Word], layout: Layout) -> Vec<Caption> {
    // Lines hold up to two rows of `max_chars`.
    let line_limit = layout.max_chars * 2 - 4;
    let mut groups: Vec<Vec<&Word>> = Vec::new();
    for w in words {
        let new_group = match groups.last() {
            None => true,
            Some(g) => {
                let chars: usize = g.iter().map(|w| w.text.chars().count() + 1).sum();
                let first = g[0];
                let last = g[g.len() - 1];
                w.start - last.end > layout.pause
                    || chars + w.text.chars().count() > line_limit
                    || w.end - first.start > layout.max_secs
                    || (ends_sentence(&last.text) && chars >= 14)
                    || (last.text.ends_with(',') && chars >= 24)
            }
        };
        if new_group {
            groups.push(vec![w]);
        } else {
            groups.last_mut().unwrap().push(w);
        }
    }
    let mut captions: Vec<Caption> = groups
        .iter()
        .map(|g| {
            let text = g
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            Caption {
                text: balance(&text, layout.max_chars),
                start: (g[0].start - 0.05).max(0.0),
                end: g[g.len() - 1].end + 0.25,
            }
        })
        .collect();
    // A caption stays long enough to read and never runs into the next.
    for i in 0..captions.len() {
        let min_end = captions[i].start + 0.9;
        if captions[i].end < min_end {
            captions[i].end = min_end;
        }
        if let Some(next_start) = captions.get(i + 1).map(|c| c.start) {
            if captions[i].end > next_start - 0.02 {
                captions[i].end = (next_start - 0.02).max(captions[i].start + 0.3);
            }
        }
    }
    captions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, start: f64, end: f64) -> Word {
        Word {
            text: text.into(),
            start,
            end,
        }
    }

    #[test]
    fn pieces_of_words_become_words_with_times() {
        let tokens: Vec<String> = ["<unk>", "\u{2581}hel", "lo", "\u{2581}world", ".", "<blk>"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // blank, hel, hel (repeat), lo, blank, blank, world, ., blank
        let ids = [5, 1, 1, 2, 5, 5, 3, 4, 5];
        let words = words_from_ids(&ids, &tokens, 5);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "hello");
        assert_eq!(words[1].text, "world.");
        assert!((words[0].start - 0.08).abs() < 1e-9);
        assert!((words[1].start - 6.0 * 0.08).abs() < 1e-9);
        // A word ends before the next begins.
        assert!(words[0].end <= words[1].start);
        assert!(words[0].end > words[0].start);
    }

    #[test]
    fn a_stretch_of_blanks_gives_no_words() {
        let tokens = vec!["a".to_string(), "<blk>".to_string()];
        assert!(words_from_ids(&[1, 1, 1], &tokens, 1).is_empty());
    }

    #[test]
    fn captions_break_at_pauses_and_sentences() {
        let words = vec![
            w("Welcome", 0.0, 0.4),
            w("back.", 0.5, 0.9),
            w("Today", 1.0, 1.4),
            w("we", 1.5, 1.6),
            w("edit.", 1.7, 2.1),
            w("Then", 4.0, 4.3), // after a long pause
        ];
        let c = group_captions(&words, Layout::default());
        assert_eq!(c.len(), 3, "{c:?}");
        assert_eq!(c[0].text, "Welcome back.");
        assert_eq!(c[1].text, "Today we edit.");
        assert_eq!(c[2].text, "Then");
        // Timing is ordered, never overlaps and each stays up long enough.
        for pair in c.windows(2) {
            assert!(pair[0].end < pair[1].start);
        }
        assert!(c.iter().all(|c| c.end - c.start >= 0.3));
    }

    #[test]
    fn long_sentences_split_into_readable_lines() {
        let text = "this is a rather long sentence that keeps going and going without any stop at all until it must be cut";
        let mut words = Vec::new();
        for (i, t) in text.split(' ').enumerate() {
            words.push(w(t, i as f64 * 0.3, i as f64 * 0.3 + 0.25));
        }
        let c = group_captions(&words, Layout::default());
        assert!(c.len() >= 2, "{c:?}");
        for cap in &c {
            for line in cap.text.lines() {
                assert!(line.chars().count() <= 36, "{line}");
            }
            assert!(cap.text.lines().count() <= 2);
        }
    }

    #[test]
    fn mel_filters_follow_the_slaney_scale() {
        let m = mel_filters();
        assert_eq!(m.len(), MELS);
        // Below 1 kHz the scale is linear in hertz, so the first filter is
        // centred at the second mel point (about 49.5 Hz): bin 1 is 31.25 Hz.
        assert!(m[0][1] > 0.0 && m[0][0] >= 0.0);
        // Every filter has some weight, and the top one reaches the last bin.
        assert!(m.iter().all(|r| r.iter().sum::<f32>() > 0.0));
        assert!(m[MELS - 1][N_FFT / 2 - 1] > 0.0);
        // The scale conversion round-trips.
        for f in [0.0, 300.0, 999.0, 1000.0, 4000.0, 8000.0] {
            assert!((mel_to_hz(hz_to_mel(f)) - f).abs() < 1e-6);
        }
    }

    #[test]
    fn features_have_zero_mean_per_row() {
        let tone: Vec<f32> = (0..16_000)
            .map(|i| 0.3 * (std::f32::consts::TAU * 440.0 * i as f32 / 16_000.0).sin())
            .collect();
        let (f, frames) = features(&tone);
        assert_eq!(frames, 101);
        for m in 0..MELS {
            let mean = f[m * frames..(m + 1) * frames].iter().sum::<f32>() / frames as f32;
            assert!(mean.abs() < 1e-3, "row {m}: {mean}");
        }
    }

    #[test]
    fn pieces_cut_at_the_quiet_moments() {
        // 50 s of loud sound with a quiet gap at 21 s.
        let mut s: Vec<f32> = (0..50 * SAMPLE_RATE)
            .map(|i| 0.5 * (i as f32 * 0.05).sin())
            .collect();
        for v in &mut s[21 * SAMPLE_RATE..21 * SAMPLE_RATE + 8_000] {
            *v = 0.0;
        }
        let p = pieces(&s);
        assert!(p.len() >= 2);
        let cut = p[0].1 as f64 / SAMPLE_RATE as f64;
        assert!((cut - 21.0).abs() < 0.6, "{cut}");
        // Together the pieces cover everything.
        assert_eq!(p[0].0, 0);
        assert_eq!(p.last().unwrap().1, s.len());
        assert!(p.windows(2).all(|w| w[0].1 == w[1].0));
    }

    /// Compares with numbers made by the original Python code
    /// (`TIDYCUTS_ASR_VECTORS` = a folder with `audio.bin` and `feat.bin`).
    #[test]
    fn features_match_the_reference_implementation() {
        let Some(dir) = std::env::var_os("TIDYCUTS_ASR_VECTORS") else {
            eprintln!("TIDYCUTS_ASR_VECTORS not set; skipping");
            return;
        };
        let dir = PathBuf::from(dir);
        let read = |n: &str| -> Vec<f32> {
            std::fs::read(dir.join(n))
                .unwrap()
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect()
        };
        let (audio, want) = (read("audio.bin"), read("feat.bin"));
        let (got, frames) = features(&audio);
        assert_eq!(got.len(), want.len(), "frames {frames}");
        let d = got
            .iter()
            .zip(&want)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(d < 5e-3, "features differ by {d}");
    }

    /// Needs the speech model and ONNX Runtime (see `model_dir`); the
    /// recording is a few seconds of speech (`TIDYCUTS_ASR_WAV`, raw f32
    /// 16 kHz) and `TIDYCUTS_ASR_TEXT` the words it should contain.
    #[test]
    fn it_hears_the_words() {
        let (Some(wav), Some(text)) = (
            std::env::var_os("TIDYCUTS_ASR_WAV"),
            std::env::var("TIDYCUTS_ASR_TEXT").ok(),
        ) else {
            eprintln!("no test recording; skipping");
            return;
        };
        if available_models().is_empty() {
            eprintln!("no speech model; skipping");
            return;
        }
        let samples: Vec<f32> = std::fs::read(wav)
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        let model = available_models()[0];
        let rec = Recognizer::load(model).unwrap();
        let words = rec.transcribe_piece(&samples).unwrap();
        let heard: Vec<String> = words
            .iter()
            .map(|w| {
                w.text
                    .to_lowercase()
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect()
            })
            .collect();
        let wanted: Vec<String> = text
            .to_lowercase()
            .split_whitespace()
            .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect())
            .collect();
        let hits = wanted.iter().filter(|w| heard.contains(w)).count();
        assert!(
            hits * 10 >= wanted.len() * 8,
            "heard {heard:?}, wanted {wanted:?}"
        );
    }
}
