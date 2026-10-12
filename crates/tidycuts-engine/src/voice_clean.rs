//! Voice clean-up: a neural network tells the voice from everything else and
//! turns the rest down. Fans, hiss, traffic and room noise drop away while the
//! voice stays whole. Two engines do it: Studio Voice (`studio_voice`, the
//! best, used when its files are installed) and RNNoise (small, built in, used
//! otherwise).
//!
//! The clean-up runs ahead of time on the whole sound of a file and the result
//! is kept as a plain sound file in the cache, so playback and export just read
//! it like any other sound. How much of the cleaned sound is used (the clip's
//! `voice_clean` amount) is baked into the file in ten steps, so moving the
//! slider makes a new file only when it crosses a step.

use crate::studio_voice::{self, Studio, StudioStream};
use nnnoiseless::DenoiseState;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tidycuts_media::Tools;

/// RNNoise works at exactly 48 kHz.
pub const SAMPLE_RATE: u32 = 48_000;
const FRAME: usize = DenoiseState::FRAME_SIZE;
/// RNNoise works on 16-bit-sized numbers.
const SCALE: f32 = 32768.0;
/// Bumped when the processing changes, so old cache files are not reused.
const VERSION: u32 = 1;
/// The amount (1..=100) is rounded to this many steps.
pub const STEPS: u32 = 10;
/// Longest sound that is cleaned (a WAV file stops at 4 GB).
const MAX_SECONDS: f64 = 3.0 * 3600.0;

/// Cleans a stream of stereo sound, a chunk at a time.
pub struct Cleaner {
    states: [Box<DenoiseState<'static>>; 2],
    /// Samples waiting for a whole frame, per channel.
    pending: [Vec<f32>; 2],
    /// The previous frame as it came in, to mix with the cleaned frame that
    /// RNNoise gives one frame later.
    dry: [[f32; FRAME]; 2],
    started: bool,
    mix: f32,
    /// Stereo frames given to `push` so far, and handed back so far.
    pushed: usize,
    emitted: usize,
}

impl Cleaner {
    /// `mix` is how much of the cleaned sound to use, 0..=1.
    pub fn new(mix: f32) -> Cleaner {
        Cleaner {
            states: [DenoiseState::new(), DenoiseState::new()],
            pending: [Vec::new(), Vec::new()],
            dry: [[0.0; FRAME]; 2],
            started: false,
            mix: mix.clamp(0.0, 1.0),
            pushed: 0,
            emitted: 0,
        }
    }

    /// Feeds interleaved stereo samples and appends the cleaned ones that are
    /// ready to `out` (interleaved too). The sound comes out a little behind
    /// what went in; `finish` gives the rest.
    pub fn push(&mut self, interleaved: &[f32], out: &mut Vec<f32>) {
        for pair in interleaved.as_chunks::<2>().0 {
            self.pending[0].push(pair[0]);
            self.pending[1].push(pair[1]);
            self.pushed += 1;
        }
        let mut done = 0;
        while self.pending[0].len() - done >= FRAME {
            let l: Vec<f32> = self.pending[0][done..done + FRAME].to_vec();
            let r: Vec<f32> = self.pending[1][done..done + FRAME].to_vec();
            self.frame(&l, &r, out);
            done += FRAME;
        }
        self.pending[0].drain(..done);
        self.pending[1].drain(..done);
    }

    /// Flushes what is left. After this, exactly as many sample frames have come
    /// out (over all calls) as went in.
    pub fn finish(&mut self, out: &mut Vec<f32>) {
        let start = out.len();
        let before = self.emitted;
        let allowed = self.pushed.saturating_sub(before);
        let rest = self.pending[0].len();
        if rest > 0 {
            let mut l = std::mem::take(&mut self.pending[0]);
            let mut r = std::mem::take(&mut self.pending[1]);
            l.resize(FRAME, 0.0);
            r.resize(FRAME, 0.0);
            self.frame(&l, &r, out);
        }
        // One more frame of silence pushes the last real frame through.
        if self.started {
            let z = [0.0f32; FRAME];
            self.frame(&z, &z, out);
        }
        out.truncate(start + allowed * 2);
        self.emitted = before + out.len().saturating_sub(start) / 2;
    }

    fn frame(&mut self, l: &[f32], r: &[f32], out: &mut Vec<f32>) {
        let mut wet = [[0.0f32; FRAME]; 2];
        for (ch, input) in [l, r].into_iter().enumerate() {
            let mut scaled = [0.0f32; FRAME];
            for (s, x) in scaled.iter_mut().zip(input) {
                *s = if x.is_finite() { *x * SCALE } else { 0.0 };
            }
            self.states[ch].process_frame(&mut wet[ch], &scaled);
        }
        if self.started {
            let clean = |w: f32| {
                let w = w / SCALE;
                if w.is_finite() {
                    w.clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            };
            for (((&wl, &wr), &dl), &dr) in wet[0]
                .iter()
                .zip(&wet[1])
                .zip(&self.dry[0])
                .zip(&self.dry[1])
            {
                out.push(dl + (clean(wl) - dl) * self.mix);
                out.push(dr + (clean(wr) - dr) * self.mix);
            }
            self.emitted += FRAME;
        }
        // The first frame RNNoise gives back is a fade-in of nothing.
        self.started = true;
        self.dry = [
            <[f32; FRAME]>::try_from(l).unwrap_or([0.0; FRAME]),
            <[f32; FRAME]>::try_from(r).unwrap_or([0.0; FRAME]),
        ];
    }
}

/// Cleans a whole stereo sound in memory (tests and small jobs).
pub fn clean_stereo(interleaved: &[f32], mix: f32) -> Vec<f32> {
    let mut c = Cleaner::new(mix);
    let mut out = Vec::with_capacity(interleaved.len() + FRAME * 2);
    c.push(interleaved, &mut out);
    c.finish(&mut out);
    out
}

/// The engine that cleans, so both can be used the same way.
enum Engine {
    Rnnoise(Box<Cleaner>),
    Studio(Box<StudioStream>),
}

impl Engine {
    fn new(studio: bool, mix: f32) -> Engine {
        if studio {
            match Studio::load() {
                Ok(s) => return Engine::Studio(Box::new(StudioStream::new(s, 1.0, mix))),
                Err(e) => {
                    log::warn!("Studio Voice is not available ({e}); using the built-in cleaner")
                }
            }
        }
        Engine::Rnnoise(Box::new(Cleaner::new(mix)))
    }

    fn push(&mut self, samples: &[f32], out: &mut Vec<f32>) -> Result<(), String> {
        match self {
            Engine::Rnnoise(c) => {
                c.push(samples, out);
                Ok(())
            }
            Engine::Studio(s) => {
                out.extend(s.push(samples)?);
                Ok(())
            }
        }
    }

    fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), String> {
        match self {
            Engine::Rnnoise(c) => {
                c.finish(out);
                Ok(())
            }
            Engine::Studio(s) => {
                out.extend(s.finish()?);
                Ok(())
            }
        }
    }
}

/// Which engine a new clean-up uses, for the screen.
pub fn engine_name() -> &'static str {
    if studio_voice::available() {
        "Studio Voice"
    } else {
        "built-in cleaner"
    }
}

// ---- the cache ------------------------------------------------------------

static CACHE: OnceLock<PathBuf> = OnceLock::new();

/// Tells the engine where its cache folder is (once, at start-up).
pub fn set_cache_dir(dir: PathBuf) {
    let _ = CACHE.set(dir);
}

fn voice_dir() -> PathBuf {
    CACHE
        .get()
        .cloned()
        .unwrap_or_else(|| std::env::temp_dir().join("TidyCuts").join("cache"))
        .join("voice")
}

/// Deletes every cleaned sound (View > Delete Preview Files).
pub fn clear_cache() {
    let _ = std::fs::remove_dir_all(voice_dir());
    if let Ok(mut m) = ready().lock() {
        m.clear();
    }
}

/// The step (1..=STEPS) that an amount of 1..=100 falls in.
pub fn step_of(amount: f64) -> u32 {
    ((amount / 100.0 * STEPS as f64).round() as u32).clamp(1, STEPS)
}

/// How much of the cleaned sound a step uses. The slider counts decibels of
/// noise taken away (0.3 dB per point, 30 dB at the top), because that is how
/// the ear hears it; a straight 0..1 mix would do very little until the end.
pub fn mix_of_step(step: u32) -> f32 {
    if step >= STEPS {
        return 1.0;
    }
    let amount = step as f32 / STEPS as f32 * 100.0;
    1.0 - 10f32.powf(-amount * 0.3 / 20.0)
}

fn cache_file(media: &Path, stream: u32, step: u32) -> PathBuf {
    let studio = studio_voice::available();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    media.hash(&mut h);
    if let Ok(m) = std::fs::metadata(media) {
        m.len().hash(&mut h);
        if let Ok(t) = m.modified() {
            t.hash(&mut h);
        }
    }
    VERSION.hash(&mut h);
    studio.hash(&mut h);
    let tag = if studio { 's' } else { 'r' };
    voice_dir().join(format!("{:016x}-{stream}-{step}{tag}.wav", h.finish()))
}

enum Job {
    Running(Arc<AtomicU32>),
    Failed,
}

type Key = (PathBuf, u32, u32);

fn jobs() -> &'static Mutex<HashMap<Key, Job>> {
    static J: OnceLock<Mutex<HashMap<Key, Job>>> = OnceLock::new();
    J.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ready() -> &'static Mutex<HashMap<Key, PathBuf>> {
    static R: OnceLock<Mutex<HashMap<Key, PathBuf>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Where a sound's clean-up stands.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// The cleaned sound is ready to play.
    Ready(PathBuf),
    /// Being made: how far along, 0..1.
    Working(f32),
    /// It could not be made (the original sound is used).
    Failed,
}

/// Asks for the cleaned sound of `media`'s audio stream at `amount` (1..=100).
/// Starts making it in the background if it does not exist yet. With `block`
/// (export) it waits until it is done.
pub fn request(media: &Path, seconds: f64, stream: u32, amount: f64, block: bool) -> Status {
    let step = step_of(amount);
    let key: Key = (media.to_path_buf(), stream, step);
    if let Some(p) = ready().lock().unwrap().get(&key) {
        return Status::Ready(p.clone());
    }
    let file = cache_file(media, stream, step);
    if file.exists() {
        ready().lock().unwrap().insert(key, file.clone());
        return Status::Ready(file);
    }
    let progress = {
        let mut j = jobs().lock().unwrap();
        match j.get(&key) {
            Some(Job::Failed) => return Status::Failed,
            Some(Job::Running(p)) => {
                if !block {
                    return Status::Working(p.load(Ordering::Relaxed) as f32 / 1000.0);
                }
                None
            }
            None => {
                let p = Arc::new(AtomicU32::new(0));
                j.insert(key.clone(), Job::Running(p.clone()));
                Some(p)
            }
        }
    };
    match progress {
        Some(p) if block => run(&key, &file, seconds, &p),
        Some(p) => {
            let (key2, file2) = (key.clone(), file.clone());
            let spawned = std::thread::Builder::new()
                .name("tidycuts-voice-clean".into())
                .spawn(move || {
                    run(&key2, &file2, seconds, &p);
                });
            if spawned.is_err() {
                jobs().lock().unwrap().insert(key, Job::Failed);
                return Status::Failed;
            }
            Status::Working(0.0)
        }
        // Someone else is making it and we must wait for the result.
        None => loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
            match jobs().lock().unwrap().get(&key) {
                Some(Job::Running(_)) => {}
                Some(Job::Failed) => return Status::Failed,
                None => {
                    return match ready().lock().unwrap().get(&key) {
                        Some(p) => Status::Ready(p.clone()),
                        None => Status::Failed,
                    }
                }
            }
        },
    }
}

/// Makes the file, then records the result.
fn run(key: &Key, file: &Path, seconds: f64, progress: &AtomicU32) -> Status {
    let result = make(&key.0, key.1, key.2, file, seconds, progress);
    let mut j = jobs().lock().unwrap();
    match result {
        Ok(()) => {
            j.remove(key);
            ready()
                .lock()
                .unwrap()
                .insert(key.clone(), file.to_path_buf());
            Status::Ready(file.to_path_buf())
        }
        Err(e) => {
            log::warn!("voice clean-up of {} failed: {e}", key.0.display());
            j.insert(key.clone(), Job::Failed);
            Status::Failed
        }
    }
}

/// Decodes the whole sound, cleans it and writes it as a float WAV file.
fn make(
    media: &Path,
    stream: u32,
    step: u32,
    file: &Path,
    seconds: f64,
    progress: &AtomicU32,
) -> Result<(), String> {
    if seconds > MAX_SECONDS {
        return Err("sound too long".into());
    }
    let tools = Tools::get().map_err(|e| e.to_string())?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let part = file.with_extension("part");
    let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    write_header(&mut out, 0).map_err(|e| e.to_string())?;

    let mut cmd = tools.ffmpeg_cmd();
    cmd.arg("-i").arg(media);
    cmd.args(["-vn", "-sn", "-dn", "-map", &format!("0:a:{stream}?")]);
    cmd.args([
        "-af",
        &format!("aresample={SAMPLE_RATE}:async=1,aformat=sample_fmts=flt:channel_layouts=stereo"),
        "-f",
        "f32le",
        "-ac",
        "2",
        "-ar",
        &SAMPLE_RATE.to_string(),
        "pipe:1",
    ]);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let mut child = crate::decode::KillOnDrop(cmd.spawn().map_err(|e| e.to_string())?);
    let mut pipe = child.0.stdout.take().ok_or("no pipe")?;

    let mut cleaner = Engine::new(studio_voice::available(), mix_of_step(step));
    let total_est = (seconds * SAMPLE_RATE as f64).max(1.0);
    let mut bytes = vec![0u8; 1 << 16];
    let mut carry: Vec<u8> = Vec::new();
    let mut produced: Vec<f32> = Vec::new();
    let mut frames_in = 0usize;
    let mut frames_written = 0usize;
    loop {
        let n = pipe.read(&mut bytes).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        carry.extend_from_slice(&bytes[..n]);
        // Whole stereo frames only (8 bytes each).
        let whole = carry.len() / 8 * 8;
        let samples: Vec<f32> = carry[..whole]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        carry.drain(..whole);
        frames_in += samples.len() / 2;
        produced.clear();
        cleaner.push(&samples, &mut produced)?;
        write_samples(&mut out, &produced).map_err(|e| e.to_string())?;
        frames_written += produced.len() / 2;
        progress.store(
            ((frames_in as f64 / total_est).min(0.999) * 1000.0) as u32,
            Ordering::Relaxed,
        );
        if frames_in as f64 > MAX_SECONDS * SAMPLE_RATE as f64 {
            return Err("sound too long".into());
        }
    }
    if frames_in == 0 {
        return Err("no sound".into());
    }
    produced.clear();
    cleaner.finish(&mut produced)?;
    write_samples(&mut out, &produced).map_err(|e| e.to_string())?;
    frames_written += produced.len() / 2;
    out.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    write_header(&mut out, frames_written as u64).map_err(|e| e.to_string())?;
    drop(out);
    std::fs::rename(&part, file).map_err(|e| e.to_string())?;
    progress.store(1000, Ordering::Relaxed);
    Ok(())
}

fn write_samples(out: &mut std::fs::File, s: &[f32]) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(s.len() * 4);
    for v in s {
        buf.extend_from_slice(&v.to_le_bytes());
    }
    out.write_all(&buf)
}

/// A 32-bit float stereo WAV header for `frames` frames.
fn write_header(out: &mut std::fs::File, frames: u64) -> std::io::Result<()> {
    let data = (frames * 8).min(u32::MAX as u64 - 64) as u32;
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    h.extend_from_slice(&2u16.to_le_bytes());
    h.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    h.extend_from_slice(&(SAMPLE_RATE * 8).to_le_bytes());
    h.extend_from_slice(&8u16.to_le_bytes());
    h.extend_from_slice(&32u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data.to_le_bytes());
    out.write_all(&h)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tone that wobbles like a voice, plus steady noise.
    fn voicey(frames: usize, noise: f32) -> (Vec<f32>, Vec<f32>) {
        let mut seed = 12345u32;
        let mut rnd = move || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        };
        let mut clean = Vec::with_capacity(frames * 2);
        let mut noisy = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let t = i as f32 / SAMPLE_RATE as f32;
            let env = (0.5 + 0.5 * (t * 3.0 * std::f32::consts::TAU).sin()).powi(2);
            let v = 0.3
                * env
                * ((t * 140.0 * std::f32::consts::TAU).sin()
                    + 0.5 * (t * 280.0 * std::f32::consts::TAU).sin()
                    + 0.3 * (t * 1200.0 * std::f32::consts::TAU).sin());
            let n = noise * rnd();
            clean.extend([v, v]);
            noisy.extend([v + n, v + n]);
        }
        (clean, noisy)
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }

    #[test]
    fn output_is_as_long_as_the_input_and_finite() {
        for frames in [0usize, 1, 479, 480, 481, 48_000, 100_003] {
            let (_, noisy) = voicey(frames, 0.05);
            let out = clean_stereo(&noisy, 1.0);
            assert_eq!(out.len(), noisy.len(), "{frames} frames");
            assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0001));
        }
    }

    #[test]
    fn loud_input_cannot_blow_up() {
        let loud: Vec<f32> = (0..96_000)
            .map(|i| if i % 7 == 0 { 20.0 } else { -20.0 })
            .collect();
        let out = clean_stereo(&loud, 1.0);
        assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0001));
    }

    #[test]
    fn zero_amount_leaves_the_sound_alone() {
        let (_, noisy) = voicey(48_000, 0.05);
        let out = clean_stereo(&noisy, 0.0);
        let diff: f32 = out
            .iter()
            .zip(&noisy)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(diff < 1e-4, "{diff}");
    }

    #[test]
    fn steady_noise_between_words_is_turned_down() {
        // Two seconds of noise only, then a voice-like tone with the same noise.
        let (_, mut noisy) = voicey(48_000 * 4, 0.05);
        let mut seed = 99u32;
        for s in noisy.iter_mut().take(48_000 * 2 * 2) {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *s = 0.05 * ((seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0);
        }
        let out = clean_stereo(&noisy, 1.0);
        let before = rms(&noisy[48_000..48_000 * 2 * 2]);
        let after = rms(&out[48_000..48_000 * 2 * 2]);
        assert!(after < before * 0.35, "noise {before} -> {after}");
    }

    #[test]
    fn the_sound_is_not_shifted_in_time() {
        // A click train must come out where it went in (within a few samples).
        let frames = 48_000;
        let mut x = vec![0.0f32; frames * 2];
        for k in 0..20 {
            let at = 4_000 + k * 2_000;
            for j in 0..40 {
                let v = 0.5 * (j as f32 * 0.5).sin();
                x[(at + j) * 2] = v;
                x[(at + j) * 2 + 1] = v;
            }
        }
        let out = clean_stereo(&x, 0.0);
        assert_eq!(x, out);
        // And with the network in the path the time alignment is the same.
        // The pitch glides so the signal does not repeat (a plain tone would
        // match itself at every period).
        let n = 48_000usize;
        let mut phase = 0.0f32;
        let mut clean = Vec::with_capacity(n * 2);
        for i in 0..n {
            let t = i as f32 / SAMPLE_RATE as f32;
            phase += (130.0 + 90.0 * t + 25.0 * (t * 7.0).sin()) / SAMPLE_RATE as f32;
            let env = (0.55 + 0.45 * (t * 4.1 * std::f32::consts::TAU).sin()).powi(2);
            let mut v = 0.0;
            for h in 1..=12 {
                v += (phase * h as f32 * std::f32::consts::TAU).sin() / h as f32;
            }
            let v = 0.3 * env * v;
            clean.extend([v, v]);
        }
        let out = clean_stereo(&clean, 1.0);
        let l: Vec<f32> = clean.iter().step_by(2).copied().collect();
        let o: Vec<f32> = out.iter().step_by(2).copied().collect();
        let (mut best, mut best_lag) = (f32::MIN, 0i32);
        for lag in -700i32..=700 {
            let mut acc = 0.0;
            for i in 4_000..40_000usize {
                acc += l[i] * o[(i as i32 + lag) as usize];
            }
            if acc > best {
                best = acc;
                best_lag = lag;
            }
        }
        assert!(best_lag.abs() <= 8, "lag {best_lag}");
    }

    #[test]
    fn mix_grows_with_the_step_and_ends_at_full() {
        let mixes: Vec<f32> = (1..=STEPS).map(mix_of_step).collect();
        assert!(mixes.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(*mixes.last().unwrap(), 1.0);
        // Half way takes about 15 dB of noise away.
        let half = 20.0 * (1.0 - mix_of_step(5)).log10();
        assert!((half + 15.0).abs() < 0.1, "{half}");
    }

    #[test]
    fn steps_cover_the_range() {
        assert_eq!(step_of(1.0), 1);
        assert_eq!(step_of(55.0), 6);
        assert_eq!(step_of(100.0), STEPS);
        assert_eq!(step_of(400.0), STEPS);
    }
}
