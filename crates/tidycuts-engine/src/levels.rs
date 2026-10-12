//! Sound levels for audio visualizers. Each file's sound is decoded once in
//! the background to 16 kHz mono. For any moment, the clips sounding then
//! are mixed over a short window and a small FFT turns the mix into 64
//! bands from deep bass to treble. The same moment always gives the same
//! levels, so scrubbing, playback and export agree.

use crate::decode::{AudioParams, AudioStream};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;
use tidycuts_core::{Clip, MediaItem, MediaKind, Project, Time, TrackKind, VisualizerSpec};

/// Bands the sound is split into.
pub const BANDS: usize = 64;
/// Sample rate of the decoded copies.
const RATE: u32 = 16_000;
/// Samples per analysis window (64 ms).
const WINDOW: usize = 1024;
const LOW_HZ: f64 = 45.0;
const HIGH_HZ: f64 = 7_600.0;
/// Bars fall back over a few frames instead of dropping at once: each
/// earlier window counts this much less.
const FALL: f32 = 0.07;
const HISTORY: usize = 5;

type Samples = Arc<Vec<i16>>;

enum Slot {
    Loading,
    Ready(Samples),
    Failed,
}

#[derive(Default)]
struct Inner {
    files: Mutex<HashMap<(PathBuf, u32), Slot>>,
    ready: Condvar,
    notify: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    generation: AtomicU64,
    blocking: AtomicBool,
    demo: AtomicBool,
}

/// The decoded sound of every file a visualizer has needed. Clones share
/// it.
#[derive(Clone, Default)]
pub struct AudioLevels {
    inner: Arc<Inner>,
}

impl AudioLevels {
    /// Called whenever a file finishes decoding (to redraw the preview).
    pub fn set_notify(&self, f: Arc<dyn Fn() + Send + Sync>) {
        *self.inner.notify.lock().unwrap() = Some(f);
    }

    /// Waits for files to decode instead of showing silence meanwhile (for
    /// export).
    pub fn set_blocking(&self, on: bool) {
        self.inner.blocking.store(on, Ordering::Relaxed);
    }

    /// Made-up levels that move with time, for library pictures.
    pub fn set_demo(&self, on: bool) {
        self.inner.demo.store(on, Ordering::Relaxed);
    }

    /// Goes up each time a file finishes decoding.
    pub fn generation(&self) -> u64 {
        self.inner.generation.load(Ordering::Relaxed)
    }

    /// A file's sound as 16 kHz mono, starting the decode if needed.
    fn samples(&self, media: &MediaItem, stream: u32) -> Option<Samples> {
        let key = (media.path.clone(), stream);
        let mut files = self.inner.files.lock().unwrap();
        if !files.contains_key(&key) {
            files.insert(key.clone(), Slot::Loading);
            let inner = self.inner.clone();
            let (path, s) = key.clone();
            std::thread::Builder::new()
                .name("tidycuts-levels".into())
                .spawn(move || {
                    let slot = match decode_mono(path.clone(), s) {
                        Some(v) => Slot::Ready(Arc::new(v)),
                        None => Slot::Failed,
                    };
                    inner.files.lock().unwrap().insert((path, s), slot);
                    inner.generation.fetch_add(1, Ordering::Relaxed);
                    inner.ready.notify_all();
                    if let Some(n) = inner.notify.lock().unwrap().clone() {
                        n();
                    }
                })
                .ok()?;
        }
        if self.inner.blocking.load(Ordering::Relaxed) {
            let deadline = std::time::Instant::now() + Duration::from_secs(120);
            while matches!(files.get(&key), Some(Slot::Loading)) {
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                if left.is_zero() {
                    break;
                }
                files = self.inner.ready.wait_timeout(files, left).unwrap().0;
            }
        }
        match files.get(&key) {
            Some(Slot::Ready(v)) => Some(v.clone()),
            _ => None,
        }
    }

    /// The bar heights (0..1) a visualizer clip shows at timeline time `t`.
    pub fn bars(&self, project: &Project, vis: &Clip, spec: &VisualizerSpec, t: Time) -> Vec<f32> {
        let bands = if self.inner.demo.load(Ordering::Relaxed) {
            demo_bands(vis.local(t).as_secs_f64())
        } else {
            self.bands(project, spec, t)
        };
        let n = spec.bars.clamp(1, BANDS as u32) as usize;
        let gain = (spec.sensitivity / 100.0).max(0.0) as f32;
        (0..n)
            .map(|j| {
                let (a, b) = (j * BANDS / n, ((j + 1) * BANDS / n).max(j * BANDS / n + 1));
                let v = bands[a..b.min(BANDS)].iter().fold(0f32, |m, x| m.max(*x));
                (v * gain).clamp(0.0, 1.0)
            })
            .collect()
    }

    /// How loud the sound is at timeline time `t`, for sound-reactive effects:
    /// (everything, bass, mid, treble), each 0..1. It follows the clip's own
    /// sound; a clip with none (a picture, a title) follows the whole mix.
    pub fn reactive(&self, project: &Project, clip: &Clip, t: Time) -> [f32; 4] {
        let bands = if self.inner.demo.load(Ordering::Relaxed) {
            demo_bands(clip.local(t).as_secs_f64())
        } else {
            let own = VisualizerSpec {
                source: Some(clip.id),
                ..VisualizerSpec::default()
            };
            let mut b = self.bands(project, &own, t);
            if b.iter().all(|v| *v < 0.001) {
                b = self.bands(project, &VisualizerSpec::default(), t);
            }
            b
        };
        summarise(&bands)
    }
    /// The 64 band levels of what plays at `t`: the whole mix, or one clip.
    pub fn bands(&self, project: &Project, spec: &VisualizerSpec, t: Time) -> [f32; BANDS] {
        let dt = Time::from_secs_f64(1.0 / 30.0);
        let reach = Time::from_secs_f64(WINDOW as f64 / RATE as f64) + dt.scale(HISTORY as f64);
        let sources: Vec<(&Clip, Samples)> = sounding(project, spec, t - reach, t + reach)
            .into_iter()
            .filter_map(|(c, m)| {
                let s = self.samples(m, c.audio.stream.min(m.audio_streams.saturating_sub(1)))?;
                Some((c, s))
            })
            .collect();
        let mut out = [0f32; BANDS];
        if sources.is_empty() {
            return out;
        }
        for k in 0..HISTORY {
            let at = t - dt.scale(k as f64);
            let raw = spectrum(&sources, at);
            for (o, v) in out.iter_mut().zip(raw) {
                *o = o.max(v - FALL * k as f32);
            }
        }
        out
    }
}

/// (Everything, bass, mid, treble) from the 64 bands: each the loudest band in
/// its range, and the first a weighted mix of the three.
fn summarise(bands: &[f32; BANDS]) -> [f32; 4] {
    let top = |a: usize, b: usize| bands[a..b].iter().fold(0f32, |m, x| m.max(*x));
    let (bass, mid, treble) = (top(0, 14), top(14, 40), top(40, BANDS));
    [
        (bass * 0.5 + mid * 0.3 + treble * 0.2).clamp(0.0, 1.0),
        bass,
        mid,
        treble,
    ]
}
/// Clips whose sound plays somewhere in `from..to`, with their media.
fn sounding<'a>(
    project: &'a Project,
    spec: &VisualizerSpec,
    from: Time,
    to: Time,
) -> Vec<(&'a Clip, &'a MediaItem)> {
    let has_sound = |kind: TrackKind, m: &MediaItem| match kind {
        TrackKind::Audio => m.kind == MediaKind::Audio || m.has_audio,
        TrackKind::Video => m.kind == MediaKind::Video && m.has_audio,
    };
    // One chosen clip, even when it's muted (its sound may be detached).
    if let Some(id) = spec.source {
        if let Some((track, c)) = project.clip(id) {
            let m = c.media_id.and_then(|m| project.media(m));
            return match m {
                Some(m) if has_sound(track.kind, m) => vec![(c, m)],
                _ => Vec::new(),
            };
        }
    }
    let mut out = Vec::new();
    for track in project.tracks.iter().filter(|t| !t.muted) {
        for c in &track.clips {
            if c.disabled || c.audio.muted || c.end() <= from || c.start >= to {
                continue;
            }
            if let Some(m) = c.media_id.and_then(|m| project.media(m)) {
                if has_sound(track.kind, m) {
                    out.push((c, m));
                }
            }
        }
    }
    out
}

/// Band levels (0..1) of the mix in the window centred on `at`.
fn spectrum(sources: &[(&Clip, Samples)], at: Time) -> [f32; BANDS] {
    let half = Time::from_secs_f64(WINDOW as f64 * 0.5 / RATE as f64);
    let w0 = at - half;
    let step = 1.0 / RATE as f64;
    let mut mix = vec![0f32; WINDOW];
    for (c, pcm) in sources {
        let first = ((c.start - w0).as_secs_f64() / step).ceil().max(0.0) as usize;
        let last = (((c.end() - w0).as_secs_f64() / step).floor().max(0.0) as usize).min(WINDOW);
        if first >= last {
            continue;
        }
        // Source position at both ends of the window, in between linearly.
        let s0 = c.source_time(w0).as_secs_f64() * RATE as f64;
        let s1 = c
            .source_time(w0 + Time::from_secs_f64((WINDOW - 1) as f64 * step))
            .as_secs_f64()
            * RATE as f64;
        let per = (s1 - s0) / (WINDOW - 1) as f64;
        let gain = (c.volume.value_at(c.local(at)) / 100.0).max(0.0) as f32 / 32768.0;
        for (i, m) in mix.iter_mut().enumerate().take(last).skip(first) {
            let idx = (s0 + per * i as f64).round();
            if idx >= 0.0 && (idx as usize) < pcm.len() {
                *m += pcm[idx as usize] as f32 * gain;
            }
        }
    }
    analyse(&mut mix)
}

/// Windowed FFT of `samples` (WINDOW long) into log-spaced band levels.
fn analyse(samples: &mut [f32]) -> [f32; BANDS] {
    let n = samples.len();
    let hann = hann();
    let mut re: Vec<f32> = samples.iter().zip(hann).map(|(s, w)| s * w).collect();
    let mut im = vec![0f32; n];
    fft(&mut re, &mut im);
    // Amplitude of a sine that would give each bin (the Hann window halves it).
    let mags: Vec<f32> = (0..=n / 2)
        .map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt() * 4.0 / n as f32)
        .collect();
    let bin_hz = RATE as f64 / n as f64;
    let at = |k: f64| -> f32 {
        let k = k.clamp(0.0, (mags.len() - 1) as f64);
        let i = k.floor() as usize;
        let f = (k - i as f64) as f32;
        let j = (i + 1).min(mags.len() - 1);
        mags[i] * (1.0 - f) + mags[j] * f
    };
    let ratio = (HIGH_HZ / LOW_HZ).powf(1.0 / BANDS as f64);
    let mut out = [0f32; BANDS];
    for (b, o) in out.iter_mut().enumerate() {
        let lo = LOW_HZ * ratio.powi(b as i32);
        let hi = lo * ratio;
        let (kl, kh) = (lo / bin_hz, hi / bin_hz);
        let mut m = at((kl + kh) * 0.5);
        let mut k = kl.ceil();
        while k <= kh {
            m = m.max(at(k));
            k += 1.0;
        }
        // Music has much less energy up high; tilt so the treble shows.
        let centre = (lo * hi).sqrt();
        let tilt = 3.0 * (centre / 150.0).log2().max(0.0);
        let db = 20.0 * (m.max(1e-7) as f64).log10() + tilt;
        *o = ((db + 62.0) / 52.0).clamp(0.0, 1.0) as f32;
    }
    // A touch of blur across neighbouring bands.
    let src = out;
    for (b, o) in out.iter_mut().enumerate() {
        let l = src[b.saturating_sub(1)];
        let r = src[(b + 1).min(BANDS - 1)];
        *o = 0.15 * l + 0.7 * src[b] + 0.15 * r;
    }
    out
}

fn hann() -> &'static [f32] {
    static W: OnceLock<Vec<f32>> = OnceLock::new();
    W.get_or_init(|| {
        (0..WINDOW)
            .map(|i| {
                let x = std::f32::consts::TAU * i as f32 / (WINDOW - 1) as f32;
                0.5 - 0.5 * x.cos()
            })
            .collect()
    })
}

/// In-place radix-2 FFT; the length must be a power of two.
pub(crate) fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -std::f64::consts::TAU / len as f64;
        let (ws, wc) = ang.sin_cos();
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f64, 0.0f64);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let (br, bi) = (re[b] as f64, im[b] as f64);
                let tr = br * cr - bi * ci;
                let ti = br * ci + bi * cr;
                re[b] = (re[a] as f64 - tr) as f32;
                im[b] = (im[a] as f64 - ti) as f32;
                re[a] = (re[a] as f64 + tr) as f32;
                im[a] = (im[a] as f64 + ti) as f32;
                let nr = cr * wc - ci * ws;
                ci = cr * ws + ci * wc;
                cr = nr;
            }
        }
        len <<= 1;
    }
}

/// Decodes a file's audio stream to 16 kHz mono.
fn decode_mono(path: PathBuf, stream: u32) -> Option<Vec<i16>> {
    let p = AudioParams {
        path,
        stream,
        sample_rate: RATE,
        filters: Vec::new(),
    };
    let mut s = AudioStream::open(&p, 0.0, None);
    let mut out: Vec<i16> = Vec::new();
    let mut buf = vec![0f32; 16_384];
    loop {
        let got = s.read(&mut buf, Duration::from_secs(10));
        for f in buf[..got].as_chunks::<2>().0.iter() {
            out.push(((f[0] + f[1]) * 0.5 * 32767.0).clamp(-32768.0, 32767.0) as i16);
        }
        if got < buf.len() && s.finished() {
            break;
        }
        if got == 0 && !s.finished() {
            // Stuck decoder.
            break;
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Lively made-up levels for library pictures: a full, arched spectrum with
/// a strong low end, a beat, and bands that move on their own.
fn demo_bands(t: f64) -> [f32; BANDS] {
    let beat = 0.7 + 0.3 * (t * std::f64::consts::PI * 2.0).sin().powi(2);
    let mut out = [0f32; BANDS];
    for (b, o) in out.iter_mut().enumerate() {
        let x = b as f64 / BANDS as f64;
        // Loud low end, a second hump in the mids, falling away to the top.
        let shape = 0.28
            + 0.5 * (-((x - 0.12) / 0.16).powi(2)).exp()
            + 0.38 * (-((x - 0.45) / 0.2).powi(2)).exp()
            + 0.14 * (x * 23.0).sin().abs();
        let wobble = 0.5 + 0.5 * (t * (3.1 + b as f64 * 0.17) + b as f64 * 0.9).sin().abs();
        let wobble = wobble.powf(0.8);
        *o = (shape * wobble * beat * 1.15).clamp(0.04, 1.0) as f32;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f64, amp: f32) -> Vec<f32> {
        (0..WINDOW)
            .map(|i| amp * (std::f64::consts::TAU * hz * i as f64 / RATE as f64).sin() as f32)
            .collect()
    }

    fn band_of(hz: f64) -> usize {
        ((hz / LOW_HZ).ln() / (HIGH_HZ / LOW_HZ).ln() * BANDS as f64) as usize
    }

    #[test]
    fn fft_finds_a_pure_tone() {
        let mut re: Vec<f32> = (0..64)
            .map(|i| (std::f32::consts::TAU * 5.0 * i as f32 / 64.0).cos())
            .collect();
        let mut im = vec![0f32; 64];
        fft(&mut re, &mut im);
        let peak = (0..32)
            .max_by(|a, b| re[*a].abs().total_cmp(&re[*b].abs()))
            .unwrap();
        assert_eq!(peak, 5);
        assert!((re[5] - 32.0).abs() < 1e-3);
    }

    #[test]
    fn a_tone_lights_its_own_band() {
        for hz in [100.0, 1000.0, 5000.0] {
            let bands = analyse(&mut tone(hz, 0.5));
            let peak = (0..BANDS)
                .max_by(|a, b| bands[*a].total_cmp(&bands[*b]))
                .unwrap();
            assert!(peak.abs_diff(band_of(hz)) <= 1, "{hz} Hz: band {peak}");
            assert!(bands[peak] > 0.6, "{hz} Hz: {}", bands[peak]);
            // Far-away bands stay low.
            let far = if peak > BANDS / 2 { 0 } else { BANDS - 1 };
            assert!(bands[far] < 0.3, "{hz} Hz: band {far} = {}", bands[far]);
        }
        let quiet = analyse(&mut vec![0f32; WINDOW]);
        assert!(quiet.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn bars_follow_the_clip_and_its_volume() {
        let mut p = Project::new("t");
        let mut m = MediaItem::new(
            PathBuf::from("/tone.wav"),
            MediaKind::Audio,
            Time::from_secs_f64(4.0),
        );
        m.has_audio = true;
        m.audio_streams = 1;
        let mid = p.add_media(m.clone());
        let a = p.append_clip(mid).unwrap();
        let levels = AudioLevels::default();
        // Pretend the file is decoded: a 440 Hz tone.
        let pcm: Vec<i16> = (0..RATE as usize * 4)
            .map(|i| {
                let x = std::f64::consts::TAU * 440.0 * i as f64 / RATE as f64;
                (x.sin() * 12000.0) as i16
            })
            .collect();
        levels
            .inner
            .files
            .lock()
            .unwrap()
            .insert((m.path.clone(), 0), Slot::Ready(Arc::new(pcm)));
        let spec = VisualizerSpec::default();
        let vis = Clip::generated(
            tidycuts_core::Generator::Visualizer(spec.clone()),
            Time::ZERO,
            Time::from_secs_f64(6.0),
        );
        let loud = levels.bands(&p, &spec, Time::from_secs_f64(1.0));
        assert!(loud[band_of(440.0)] > 0.5);
        // After the clip ends there's nothing.
        let after = levels.bands(&p, &spec, Time::from_secs_f64(5.0));
        assert!(after.iter().all(|v| *v == 0.0));
        // Muting the clip silences it.
        p.clip_mut(a).unwrap().audio.muted = true;
        let muted = levels.bands(&p, &spec, Time::from_secs_f64(1.0));
        assert!(muted.iter().all(|v| *v == 0.0));
        // ...unless the visualizer is set to show that clip.
        let one = VisualizerSpec {
            source: Some(a),
            ..spec.clone()
        };
        assert!(levels.bands(&p, &one, Time::from_secs_f64(1.0))[band_of(440.0)] > 0.5);
        let bars = levels.bars(&p, &vis, &one, Time::from_secs_f64(1.0));
        assert_eq!(bars.len(), spec.bars as usize);
        assert!(bars.iter().any(|b| *b > 0.5));
    }

    #[test]
    fn reactive_levels_move_with_time_and_stay_in_range() {
        let levels = AudioLevels::default();
        levels.set_demo(true);
        let project = Project::default();
        let clip = Clip::generated(
            tidycuts_core::Generator::Solid([0.0; 4]),
            Time::ZERO,
            Time::from_secs_f64(5.0),
        );
        let a = levels.reactive(&project, &clip, Time::from_secs_f64(0.1));
        let b = levels.reactive(&project, &clip, Time::from_secs_f64(0.9));
        assert_ne!(a, b, "the demo sound moves");
        for v in a.iter().chain(b.iter()) {
            assert!((0.0..=1.0).contains(v), "{v}");
        }
        // Without any sound, the levels are zero.
        let quiet = AudioLevels::default().reactive(&project, &clip, Time::ZERO);
        assert_eq!(quiet, [0.0; 4]);
    }
    #[test]
    fn demo_levels_move() {
        let a = demo_bands(0.0);
        let b = demo_bands(0.4);
        assert!(a.iter().zip(b).any(|(x, y)| (x - y).abs() > 0.1));
    }
}
