//! Mixes every sounding clip into one stereo stream: one decoder per clip,
//! then volume keyframes, fades, panning, ducking, loudness evening, mutes
//! and a soft limiter.

use crate::audio_fx::clip_filters;
use crate::decode::{AudioParams, AudioStream};
use crate::loudness::Momentary;
use crate::stretch::Stretcher;
use crate::voice_clean;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tidycuts_core::{
    Clip, ClipId, MediaItem, MediaKind, Project, SpeedRamp, Time, TrackId, TrackKind,
};

/// Loudness corrections per (file, stream), measured in the background.
pub type LoudnessTable = Arc<Mutex<HashMap<(PathBuf, u32), f32>>>;

struct Voice {
    stream: AudioStream,
    /// Timeline time of the next sample the stream will give.
    next: Time,
    /// Settings the stream was opened with; a change reopens it.
    filters: Vec<String>,
    /// Follows the clip's speed ramp, if it has one (the stream then plays
    /// at normal speed and this changes its speed as it goes).
    stretch: Option<Stretcher>,
    ramp: Option<SpeedRamp>,
    keep_pitch: bool,
    source_in: Time,
    source_out: Time,
    start: Time,
    used: bool,
    /// Smoothed ducking gain.
    duck: f32,
    /// The voice-cleaned copy of the sound this stream reads, when there is one.
    cleaned: Option<PathBuf>,
}

/// A clip that makes sound, with the gains that apply to it.
struct Sounding<'a> {
    clip: &'a Clip,
    media: &'a MediaItem,
    track: TrackId,
    /// The track's mixer volume as a factor.
    track_gain: f32,
}

/// Peak levels (0..1 and beyond, left and right) of the last mixed block,
/// for the mixer's meters. The mixer writes them; the window reads them.
#[derive(Default)]
pub struct Meters {
    state: Mutex<MeterState>,
}

#[derive(Default, Clone)]
pub struct MeterState {
    pub tracks: HashMap<TrackId, [f32; 2]>,
    pub master: [f32; 2],
    /// Momentary loudness (the last 400 ms) of the mix, LUFS; `None` when
    /// it is quieter than -70.
    pub lufs: Option<f32>,
}

impl Meters {
    pub fn publish(&self, state: MeterState) {
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = state;
    }

    pub fn read(&self) -> MeterState {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Back to silence (playback stopped).
    pub fn clear(&self) {
        self.publish(MeterState::default());
    }
}

pub struct Mixer {
    pub sample_rate: u32,
    voices: HashMap<ClipId, Voice>,
    pub loudness: LoudnessTable,
    /// How long to wait for a late decoder: short when playing live, long
    /// when exporting.
    pub wait: Duration,
    scratch: Vec<f32>,
    /// Where to report levels, when something is watching.
    pub meters: Option<Arc<Meters>>,
    /// Files whose loudness is being measured in the background.
    measuring: HashSet<(PathBuf, u32)>,
    /// The live loudness reading of what comes out.
    momentary: Momentary,
}

/// Every clip that sounds at some point in `[from, to)`.
fn sounding<'a>(project: &'a Project, from: Time, to: Time) -> Vec<Sounding<'a>> {
    let mut out = Vec::new();
    // While any track is soloed, only soloed tracks play.
    let solo = project.tracks.iter().any(|t| t.solo);
    for track in &project.tracks {
        if track.muted || (solo && !track.solo) {
            continue;
        }
        for clip in &track.clips {
            if clip.disabled || clip.audio.muted || clip.end() <= from || clip.start >= to {
                continue;
            }
            let Some(media) = clip.media_id.and_then(|m| project.media(m)) else {
                continue;
            };
            let has_sound = match track.kind {
                TrackKind::Audio => media.kind == MediaKind::Audio || media.has_audio,
                TrackKind::Video => media.kind == MediaKind::Video && media.has_audio,
            };
            if has_sound {
                out.push(Sounding {
                    clip,
                    media,
                    track: track.id,
                    track_gain: (track.volume / 100.0).max(0.0) as f32,
                });
            }
        }
    }
    out
}

/// Fade-in/out gain at local time `lt` (equal-power curve).
fn fade_gain(clip: &Clip, lt: f64) -> f32 {
    let len = clip.duration().as_secs_f64();
    let mut g = 1.0f64;
    let fi = clip.audio.fade_in.as_secs_f64();
    if fi > 0.0 && lt < fi {
        g *= (lt / fi).clamp(0.0, 1.0);
    }
    let fo = clip.audio.fade_out.as_secs_f64();
    if fo > 0.0 && lt > len - fo {
        g *= ((len - lt) / fo).clamp(0.0, 1.0);
    }
    (g * std::f64::consts::FRAC_PI_2).sin() as f32
}

fn soft_limit(x: f32) -> f32 {
    // Linear up to 0.8, then eases into 1.0 instead of hard clipping.
    let a = x.abs();
    if a <= 0.8 {
        x
    } else {
        let over = a - 0.8;
        (0.8 + 0.2 * (over / 0.2).tanh()).copysign(x)
    }
}

impl Mixer {
    pub fn new(sample_rate: u32, wait: Duration, loudness: LoudnessTable) -> Mixer {
        Mixer {
            sample_rate,
            voices: HashMap::new(),
            loudness,
            wait,
            scratch: Vec::new(),
            meters: None,
            measuring: HashSet::new(),
            momentary: Momentary::new(sample_rate),
        }
    }

    /// Whether this mixer is rendering a file (it may wait for slow work)
    /// rather than playing live.
    fn exporting(&self) -> bool {
        self.wait >= Duration::from_secs(10)
    }

    /// The voice-cleaned copy of a clip's sound, when clean-up is on and the
    /// copy is ready. While it is being made the original plays; when it is
    /// done the stream is reopened on it. Exports wait for it.
    fn cleaned_source(&self, clip: &Clip, media: &MediaItem) -> Option<PathBuf> {
        if clip.audio.voice_clean <= 0.0 {
            return None;
        }
        let stream = clip.audio.stream.min(media.audio_streams.saturating_sub(1));
        match voice_clean::request(
            &media.path,
            media.duration.as_secs_f64(),
            stream,
            clip.audio.voice_clean,
            self.exporting(),
        ) {
            voice_clean::Status::Ready(p) => Some(p),
            _ => None,
        }
    }

    /// The gain that brings a file to the standard loudness. A render
    /// measures it on the spot; live playback starts the measurement in the
    /// background and plays at the file's own level until it is done.
    fn normalize_gain(&mut self, path: &std::path::Path, stream: u32) -> f32 {
        let key = (path.to_path_buf(), stream);
        if let Some(g) = self.loudness.lock().unwrap().get(&key) {
            return *g;
        }
        if self.exporting() {
            let g = measure_loudness(path, stream).unwrap_or(1.0);
            self.loudness.lock().unwrap().insert(key, g);
            return g;
        }
        if self.measuring.insert(key.clone()) {
            let table = self.loudness.clone();
            std::thread::spawn(move || {
                let g = measure_loudness(&key.0, key.1).unwrap_or(1.0);
                table.lock().unwrap().insert(key, g);
            });
        }
        1.0
    }

    /// Forgets every decoder (after a seek).
    pub fn reset(&mut self) {
        self.voices.clear();
        self.momentary.reset();
    }

    fn open(&self, clip: &Clip, media: &MediaItem, at: Time) -> Voice {
        let filters = clip_filters(clip, self.sample_rate);
        let src = clip.source_time(at).as_secs_f64();
        let cleaned = self.cleaned_source(clip, media);
        let params = AudioParams {
            path: cleaned.clone().unwrap_or_else(|| media.path.clone()),
            // The cleaned copy holds just the one stream.
            stream: if cleaned.is_some() {
                0
            } else {
                clip.audio.stream.min(media.audio_streams.saturating_sub(1))
            },
            sample_rate: self.sample_rate,
            filters: filters.clone(),
        };
        let stream = if clip.reverse {
            // Play the source backwards from `src` down to the clip's in point.
            let begin = clip.source_in.as_secs_f64();
            AudioStream::open(&params, begin, Some((src - begin).max(0.0)))
        } else {
            let left = (clip.source_out.as_secs_f64() - src).max(0.0);
            AudioStream::open(&params, src, Some(left))
        };
        let stretch = clip.ramp.as_ref().map(|r| {
            Stretcher::new(
                r.clone(),
                src,
                clip.reverse,
                clip.audio.keep_pitch,
                self.sample_rate,
            )
        });
        Voice {
            stream,
            next: at,
            filters,
            stretch,
            ramp: clip.ramp.clone(),
            keep_pitch: clip.audio.keep_pitch,
            source_in: clip.source_in,
            source_out: clip.source_out,
            start: clip.start,
            used: true,
            duck: 1.0,
            cleaned,
        }
    }

    /// Mixes `out.len() / 2` stereo frames starting at timeline time `at`.
    pub fn mix(&mut self, project: &Project, at: Time, out: &mut [f32]) {
        out.iter_mut().for_each(|s| *s = 0.0);
        let frames = out.len() / 2;
        if frames == 0 {
            return;
        }
        let sr = self.sample_rate as f64;
        let block_len = Time::from_secs_f64(frames as f64 / sr);
        let end = at + block_len;
        let clips = sounding(project, at, end);
        let mut track_peaks: HashMap<TrackId, [f32; 2]> = HashMap::new();
        for v in self.voices.values_mut() {
            v.used = false;
        }
        // Clips with ducking set lower everything else while they play.
        let duckers: Vec<(ClipId, Time, Time, f32)> = clips
            .iter()
            .filter(|s| s.clip.audio.ducking > 0.0)
            .map(|s| {
                (
                    s.clip.id,
                    s.clip.start,
                    s.clip.end(),
                    (s.clip.audio.ducking / 100.0) as f32,
                )
            })
            .collect();
        for s in &clips {
            let c = s.clip;
            let filters = clip_filters(c, self.sample_rate);
            let cleaned = self.cleaned_source(c, s.media);
            let stale = self.voices.get(&c.id).is_none_or(|v| {
                v.filters != filters
                    || v.cleaned != cleaned
                    || v.ramp != c.ramp
                    || v.keep_pitch != c.audio.keep_pitch
                    || v.source_in != c.source_in
                    || v.source_out != c.source_out
                    || v.start != c.start
                    || (v.next - at.max(c.start)).abs() > Time::from_millis(30)
            });
            if stale {
                let voice = self.open(c, s.media, at.max(c.start));
                self.voices.insert(c.id, voice);
            }
            let norm = if c.audio.normalize {
                self.normalize_gain(&s.media.path, c.audio.stream)
            } else {
                1.0
            };
            let wait = self.wait;
            let voice = self.voices.get_mut(&c.id).unwrap();
            voice.used = true;
            // Where in this block the clip starts and stops.
            let first = ((c.start - at).as_secs_f64() * sr).round().max(0.0) as usize;
            let last = (((c.end() - at).as_secs_f64() * sr).round().max(0.0) as usize).min(frames);
            if first >= last {
                continue;
            }
            let n = last - first;
            self.scratch.resize(n * 2, 0.0);
            let got = match &mut voice.stretch {
                Some(st) => {
                    let stream = &mut voice.stream;
                    st.read(&mut self.scratch[..n * 2], &mut |buf| {
                        let got = stream.read(buf, wait);
                        (got, got < buf.len() && stream.finished())
                    }) / 2
                }
                None => voice.stream.read(&mut self.scratch[..n * 2], wait) / 2,
            };
            self.scratch[got * 2..].iter_mut().for_each(|x| *x = 0.0);
            voice.next = at + Time::from_secs_f64(last as f64 / sr);

            let pan = (c.audio.pan / 100.0).clamp(-1.0, 1.0) as f32;
            let (pl, pr) = ((1.0 - pan).min(1.0), (1.0 + pan).min(1.0));
            let ducked = duckers
                .iter()
                .filter(|(id, s0, e0, _)| *id != c.id && *s0 < end && *e0 > at)
                .map(|d| 1.0 - d.3)
                .fold(1.0f32, f32::min);
            // Gains are worked out every 64 samples and ramped between.
            let step = 64;
            let mut i = 0;
            let duck_from = voice.duck;
            while i < n {
                let j = (i + step).min(n);
                let t_lt = (at + Time::from_secs_f64((first + i) as f64 / sr)) - c.start;
                let vol = (c.volume.value_at(t_lt) / 100.0).max(0.0) as f32;
                let g = vol * fade_gain(c, t_lt.as_secs_f64()) * norm * s.track_gain;
                let duck = duck_from + (ducked - duck_from) * (j as f32 / n as f32);
                for k in i..j {
                    let (l, r) = (self.scratch[k * 2], self.scratch[k * 2 + 1]);
                    let (ol, or) = (l * g * duck * pl, r * g * duck * pr);
                    out[(first + k) * 2] += ol;
                    out[(first + k) * 2 + 1] += or;
                    let peak = track_peaks.entry(s.track).or_insert([0.0; 2]);
                    peak[0] = peak[0].max(ol.abs());
                    peak[1] = peak[1].max(or.abs());
                }
                i = j;
            }
            voice.duck = ducked;
        }
        self.voices.retain(|_, v| v.used);
        let master = (project.master_volume / 100.0).max(0.0) as f32;
        let mut peak = [0f32; 2];
        for (i, s) in out.iter_mut().enumerate() {
            *s = soft_limit(*s * master);
            peak[i % 2] = peak[i % 2].max(s.abs());
        }
        self.momentary.push(out);
        if let Some(m) = &self.meters {
            m.publish(MeterState {
                tracks: track_peaks,
                master: peak,
                lufs: self.momentary.lufs(),
            });
        }
    }
}

/// Measures a file's loudness and returns the gain that brings it to -14
/// LUFS (the level streaming sites use), clamped to a sane range.
pub fn measure_loudness(path: &std::path::Path, stream: u32) -> Option<f32> {
    let tools = tidycuts_media::Tools::get().ok()?;
    let mut cmd = std::process::Command::new(&tools.ffmpeg);
    cmd.args(["-hide_banner", "-nostdin", "-i"]).arg(path);
    cmd.args([
        "-map",
        &format!("0:a:{stream}?"),
        "-af",
        "loudnorm=I=-14:TP=-1:print_format=json",
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
    let out = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&out.stderr);
    let start = text.rfind('{')?;
    let json: serde_json::Value = serde_json::from_str(&text[start..text.rfind('}')? + 1]).ok()?;
    let input_i: f32 = json.get("input_i")?.as_str()?.parse().ok()?;
    if !input_i.is_finite() || input_i < -70.0 {
        return None;
    }
    Some(10f32.powf((-14.0 - input_i) / 20.0).clamp(0.1, 8.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_is_transparent_for_quiet_sound() {
        assert_eq!(soft_limit(0.5), 0.5);
        assert!(soft_limit(3.0) <= 1.0 && soft_limit(3.0) > 0.95);
        assert!(soft_limit(-3.0) >= -1.0);
    }

    #[test]
    fn fades_reach_silence_at_the_ends() {
        let m = MediaItem::new(
            PathBuf::from("/a.wav"),
            MediaKind::Audio,
            Time::from_secs_f64(4.0),
        );
        let mut c = Clip::new(&m, Time::ZERO);
        c.audio.fade_in = Time::from_secs_f64(1.0);
        c.audio.fade_out = Time::from_secs_f64(1.0);
        assert!(fade_gain(&c, 0.0) < 1e-6);
        assert!((fade_gain(&c, 2.0) - 1.0).abs() < 1e-6);
        assert!(fade_gain(&c, 3.999) < 0.01);
    }

    /// A project with one second of a 440 Hz tone (amplitude 0.4) on each of
    /// two audio tracks, or `None` without ffmpeg.
    fn two_tracks() -> Option<(Project, std::path::PathBuf)> {
        let tools = tidycuts_media::Tools::get().ok()?;
        let dir = std::env::temp_dir().join(format!("tidycuts-mix-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok()?;
        let wav = dir.join("tone.wav");
        let ok = tools
            .ffmpeg_cmd()
            .args(["-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=2"])
            // The sine source is 0.125 loud; 3.2 times that is 0.4.
            .args(["-af", "volume=3.2", "-ar", "48000", "-ac", "2"])
            .arg(&wav)
            .status()
            .ok()?
            .success();
        if !ok {
            return None;
        }
        let mut p = Project::new("mix");
        let mut m = MediaItem::new(wav.clone(), MediaKind::Audio, Time::from_secs_f64(2.0));
        m.has_audio = true;
        let id = p.add_media(m);
        let a = p
            .audio_track_for(Time::ZERO, Time::from_secs_f64(2.0))
            .unwrap();
        p.add_clip(id, a, Time::ZERO).unwrap();
        let b = p.add_track(TrackKind::Audio).unwrap();
        p.add_clip(id, b, Time::ZERO).unwrap();
        Some((p, dir))
    }

    /// The loudest sample of a second block mixed from the start.
    fn loudest(p: &Project) -> (f32, MeterState) {
        let meters = Arc::new(Meters::default());
        let mut mixer = Mixer::new(48_000, Duration::from_secs(5), Default::default());
        mixer.meters = Some(meters.clone());
        let mut out = vec![0f32; 4800 * 2];
        // The first block starts the decoders; the second is real sound.
        mixer.mix(p, Time::ZERO, &mut out);
        mixer.mix(p, Time::from_secs_f64(0.1), &mut out);
        (out.iter().fold(0f32, |m, v| m.max(v.abs())), meters.read())
    }

    #[test]
    fn track_volume_solo_mute_and_master_shape_the_mix() {
        let Some((mut p, dir)) = two_tracks() else {
            eprintln!("no ffmpeg here; skipping");
            return;
        };
        let ids: Vec<TrackId> = p
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Audio)
            .map(|t| t.id)
            .collect();
        // One track alone is the yardstick; two in step are twice as loud.
        p.track_mut(ids[0]).unwrap().solo = true;
        let (one, state) = loudest(&p);
        assert!(one > 0.2, "one {one}");
        assert!(
            !state.tracks.contains_key(&ids[1]),
            "solo plays only that track"
        );
        p.track_mut(ids[0]).unwrap().solo = false;
        let (both, state) = loudest(&p);
        assert!((both / one - 2.0).abs() < 0.1, "both {both} vs one {one}");
        assert_eq!(state.tracks.len(), 2);
        assert!(state.master[0] > one * 1.8);
        // Halving one track's volume: 1 + 0.5.
        p.track_mut(ids[1]).unwrap().volume = 50.0;
        let (half, state) = loudest(&p);
        assert!((half / one - 1.5).abs() < 0.1, "half {half} vs one {one}");
        assert!(state.tracks[&ids[1]][0] < state.tracks[&ids[0]][0] * 0.6);
        // A muted track is silent; the master volume scales everything.
        p.track_mut(ids[1]).unwrap().muted = true;
        p.master_volume = 50.0;
        let (quiet, _) = loudest(&p);
        assert!(
            (quiet / one - 0.5).abs() < 0.05,
            "quiet {quiet} vs one {one}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
