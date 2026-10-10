//! Playing sound at a speed that changes (speed ramping).
//!
//! ffmpeg's tempo filter takes one fixed speed, so for a clip with a speed
//! ramp the mixer decodes the sound at normal speed and this stretches it
//! as it goes, following the clip's speed curve. Two ways, like constant
//! speed has: keeping the pitch (overlap-add with the splice point chosen
//! by correlation, so voices stay natural) or letting the pitch follow the
//! speed (resampling).

use tidycuts_core::SpeedRamp;

/// Grain length in frames and the hop between output grains (half of it, so
/// Hann windows add up to exactly 1).
const GRAIN: usize = 1536;
const HOP: usize = GRAIN / 2;
/// How far (frames) a grain may move to line up with the one before it.
const SEARCH: i64 = 256;
/// Frames compared when lining grains up.
const MATCH: usize = 256;
/// Frames asked of the source at a time.
const CHUNK: usize = 4096;

/// What a source gives back: how many samples it wrote (interleaved stereo)
/// and whether it has ended for good.
pub type Pull<'a> = &'a mut dyn FnMut(&mut [f32]) -> (usize, bool);

pub struct Stretcher {
    sample_rate: f64,
    ramp: SpeedRamp,
    /// Source time (seconds) of the first frame the source gives.
    base: f64,
    /// The source plays backwards from `base`, so source time falls.
    reverse: bool,
    keep_pitch: bool,
    /// Buffered source frames (interleaved stereo) from frame `offset` on.
    input: Vec<f32>,
    offset: usize,
    ended: bool,
    /// Where the next output comes from, in source frames.
    pos: f64,
    // Overlap-add state (keep pitch).
    tail: Vec<f32>,
    prev_start: Option<usize>,
    /// Finished output waiting to be handed out.
    ready: Vec<f32>,
}

impl Stretcher {
    /// `base` is the source time of the first frame the source will give,
    /// `reverse` whether source time falls as the source plays.
    pub fn new(
        ramp: SpeedRamp,
        base: f64,
        reverse: bool,
        keep_pitch: bool,
        sample_rate: u32,
    ) -> Stretcher {
        Stretcher {
            sample_rate: sample_rate as f64,
            ramp,
            base,
            reverse,
            keep_pitch,
            input: Vec::new(),
            offset: 0,
            ended: false,
            pos: 0.0,
            tail: Vec::new(),
            prev_start: None,
            ready: Vec::new(),
        }
    }

    /// The speed the ramp has at source frame `f` of the stream.
    fn speed_at(&self, f: f64) -> f64 {
        let secs = f / self.sample_rate;
        let t = if self.reverse {
            self.base - secs
        } else {
            self.base + secs
        };
        self.ramp.speed_at(t)
    }

    fn buffered_end(&self) -> usize {
        self.offset + self.input.len() / 2
    }

    /// Pulls source until frames up to `upto` are buffered (or it ends or
    /// runs dry). Returns whether they are.
    fn fill(&mut self, upto: usize, pull: Pull) -> bool {
        let mut chunk = vec![0f32; CHUNK * 2];
        while self.buffered_end() < upto && !self.ended {
            let (n, done) = pull(&mut chunk);
            self.input.extend_from_slice(&chunk[..n]);
            if done {
                self.ended = true;
            }
            if n == 0 && !done {
                break;
            }
        }
        self.buffered_end() >= upto || self.ended
    }

    /// A frame of the source (zero outside what exists).
    fn frame(&self, f: i64) -> [f32; 2] {
        if f < self.offset as i64 {
            return [0.0; 2];
        }
        let i = (f as usize - self.offset) * 2;
        if i + 1 < self.input.len() {
            [self.input[i], self.input[i + 1]]
        } else {
            [0.0; 2]
        }
    }

    fn forget_before(&mut self, frame: usize) {
        if frame > self.offset {
            let drop = ((frame - self.offset) * 2).min(self.input.len());
            self.input.drain(..drop);
            self.offset += drop / 2;
        }
    }

    /// Writes up to `out.len()` interleaved samples. Fewer means the source
    /// ran dry for now or has ended (see [`Stretcher::finished`]).
    pub fn read(&mut self, out: &mut [f32], pull: Pull) -> usize {
        if self.keep_pitch {
            self.read_ola(out, pull)
        } else {
            self.read_resample(out, pull)
        }
    }

    /// Whether everything has been played out.
    pub fn finished(&self) -> bool {
        self.ended && self.pos.max(0.0) as usize >= self.buffered_end() && self.ready.is_empty()
    }

    fn read_resample(&mut self, out: &mut [f32], pull: Pull) -> usize {
        let mut n = 0;
        while n + 2 <= out.len() {
            let i = self.pos.floor().max(0.0) as usize;
            self.fill(i + 2, pull);
            let end = self.buffered_end();
            if i >= end || (i + 1 >= end && !self.ended) {
                break;
            }
            let a = self.frame(i as i64);
            let b = self.frame(i as i64 + 1);
            let t = (self.pos - i as f64) as f32;
            out[n] = a[0] + (b[0] - a[0]) * t;
            out[n + 1] = a[1] + (b[1] - a[1]) * t;
            n += 2;
            self.pos += self.speed_at(self.pos);
            self.forget_before((self.pos.max(0.0) as usize).saturating_sub(2));
        }
        n
    }

    fn read_ola(&mut self, out: &mut [f32], pull: Pull) -> usize {
        let mut n = 0;
        loop {
            // Hand out what is finished.
            let take = (out.len() - n).min(self.ready.len());
            out[n..n + take].copy_from_slice(&self.ready[..take]);
            self.ready.drain(..take);
            n += take;
            if n == out.len() || !self.next_grain(pull) {
                break;
            }
        }
        n
    }

    /// Adds the next grain to the overlap-add tail. False when the source
    /// has nothing more for now.
    fn next_grain(&mut self, pull: Pull) -> bool {
        let nominal = self.pos.round().max(0.0) as usize;
        if self.ended && nominal >= self.buffered_end() {
            return false;
        }
        let need = nominal + GRAIN + SEARCH as usize + 1;
        if !self.fill(need, pull) && !self.ended {
            return false;
        }
        let start = match self.prev_start {
            None => nominal,
            Some(prev) => self.best_start(prev, nominal),
        };
        let first = self.prev_start.is_none();
        if self.tail.len() < GRAIN * 2 {
            self.tail.resize(GRAIN * 2, 0.0);
        }
        for i in 0..GRAIN {
            let x = self.frame((start + i) as i64);
            // Hann window; the very first grain starts at full level.
            let w = if first && i < HOP {
                1.0
            } else {
                0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / GRAIN as f32).cos()
            };
            self.tail[i * 2] += x[0] * w;
            self.tail[i * 2 + 1] += x[1] * w;
        }
        self.prev_start = Some(start);
        // The first hop of the tail is now complete.
        self.ready.extend(self.tail.drain(..HOP * 2));
        let speed = self.speed_at(self.pos);
        self.pos += HOP as f64 * speed;
        self.forget_before(
            start
                .min(self.pos.max(0.0) as usize)
                .saturating_sub(SEARCH as usize + GRAIN),
        );
        true
    }

    /// The start near `nominal` whose first frames best continue what the
    /// previous grain was playing into.
    fn best_start(&self, prev: usize, nominal: usize) -> usize {
        let target = prev + HOP;
        let mut best = (f32::MIN, nominal);
        for o in (-SEARCH..=SEARCH).step_by(2) {
            let s = nominal as i64 + o;
            if s < self.offset as i64 {
                continue;
            }
            let mut c = 0.0f32;
            let mut e = 1e-6f32;
            for i in (0..MATCH).step_by(4) {
                let a = self.frame((target + i) as i64);
                let b = self.frame(s + i as i64);
                c += (a[0] + a[1]) * (b[0] + b[1]);
                e += (b[0] + b[1]) * (b[0] + b[1]);
            }
            let score = c / e.sqrt();
            if score > best.0 {
                best = (score, s as usize);
            }
        }
        best.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tidycuts_core::{RampPreset, Time};

    const SR: u32 = 48_000;

    /// A source of a sine at `hz` lasting `secs`.
    fn sine(hz: f64, secs: f64) -> impl FnMut(&mut [f32]) -> (usize, bool) {
        let total = (secs * SR as f64) as usize;
        let mut at = 0usize;
        move |buf: &mut [f32]| {
            let frames = (buf.len() / 2).min(total - at);
            for i in 0..frames {
                let v = (std::f64::consts::TAU * hz * (at + i) as f64 / SR as f64).sin() as f32;
                buf[i * 2] = v * 0.5;
                buf[i * 2 + 1] = v * 0.5;
            }
            at += frames;
            (frames * 2, at >= total)
        }
    }

    fn constant(speed: f64) -> SpeedRamp {
        SpeedRamp {
            preset: RampPreset::Custom,
            points: vec![
                tidycuts_core::RampPoint {
                    at: Time::ZERO,
                    speed,
                },
                tidycuts_core::RampPoint {
                    at: Time::from_secs_f64(100.0),
                    speed,
                },
            ],
        }
    }

    /// Plays everything out and returns the left channel.
    fn drain(mut st: Stretcher, mut src: impl FnMut(&mut [f32]) -> (usize, bool)) -> Vec<f32> {
        let mut all = Vec::new();
        let mut buf = vec![0f32; 4800 * 2];
        loop {
            let n = st.read(&mut buf, &mut src);
            all.extend(buf[..n].as_chunks::<2>().0.iter().map(|f| f[0]));
            if n < buf.len() {
                break;
            }
        }
        all
    }

    /// Rising zero crossings per second.
    fn frequency(x: &[f32]) -> f64 {
        let ups = x.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        ups as f64 / (x.len() as f64 / SR as f64)
    }

    #[test]
    fn keeping_pitch_changes_length_not_frequency() {
        for speed in [0.5, 2.0, 3.0] {
            let st = Stretcher::new(constant(speed), 0.0, false, true, SR);
            let out = drain(st, sine(440.0, 4.0));
            let want = 4.0 / speed * SR as f64;
            assert!(
                (out.len() as f64 - want).abs() < want * 0.08 + 3000.0,
                "{speed}x: {} frames, wanted about {want}",
                out.len()
            );
            // Skip the ends, where the first and last grains are partial.
            let mid = &out[SR as usize / 10..out.len() - SR as usize / 10];
            let hz = frequency(mid);
            assert!((hz - 440.0).abs() < 25.0, "{speed}x: {hz} Hz");
        }
    }

    #[test]
    fn resampling_moves_the_pitch_with_the_speed() {
        let st = Stretcher::new(constant(2.0), 0.0, false, false, SR);
        let out = drain(st, sine(440.0, 4.0));
        assert!((out.len() as f64 - 2.0 * SR as f64).abs() < 400.0);
        let hz = frequency(&out[1000..out.len() - 1000]);
        assert!((hz - 880.0).abs() < 20.0, "{hz} Hz");
    }

    #[test]
    fn a_ramp_takes_as_long_as_the_ramp_says() {
        let ramp = SpeedRamp::preset(RampPreset::BulletTime, Time::ZERO, Time::from_secs_f64(6.0));
        let want = ramp.play_secs(0.0, 6.0) * SR as f64;
        for keep in [true, false] {
            let st = Stretcher::new(ramp.clone(), 0.0, false, keep, SR);
            let out = drain(st, sine(300.0, 6.0));
            assert!(
                (out.len() as f64 - want).abs() < want * 0.05 + 3000.0,
                "keep_pitch {keep}: {} vs {want}",
                out.len()
            );
            assert!(out.iter().all(|v| v.abs() <= 0.75), "no runaway gain");
        }
    }

    #[test]
    fn keeping_pitch_has_no_big_jumps_at_splices() {
        let st = Stretcher::new(constant(1.5), 0.0, false, true, SR);
        let out = drain(st, sine(200.0, 3.0));
        // A 200 Hz sine at 0.5 amplitude never moves more than ~0.03 per frame.
        let worst = out
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f32::max);
        assert!(worst < 0.09, "largest step {worst}");
    }

    #[test]
    fn reversed_clips_read_the_curve_backwards() {
        // Fast at the start of the source, slow at the end; played in
        // reverse from 4 s, the slow part comes first.
        let ramp = SpeedRamp {
            preset: RampPreset::Custom,
            points: vec![
                tidycuts_core::RampPoint {
                    at: Time::ZERO,
                    speed: 4.0,
                },
                tidycuts_core::RampPoint {
                    at: Time::from_secs_f64(4.0),
                    speed: 0.5,
                },
            ],
        };
        let want = ramp.play_secs(0.0, 4.0) * SR as f64;
        let st = Stretcher::new(ramp, 4.0, true, true, SR);
        let out = drain(st, sine(300.0, 4.0));
        assert!(
            (out.len() as f64 - want).abs() < want * 0.06 + 3000.0,
            "{} vs {want}",
            out.len()
        );
    }

    #[test]
    fn a_slow_source_is_waited_for_not_skipped() {
        // A source that gives a little at a time and sometimes nothing.
        let mut inner = sine(440.0, 2.0);
        let mut turn = 0;
        let src = move |buf: &mut [f32]| {
            turn += 1;
            if turn % 3 == 0 {
                (0, false)
            } else {
                let n = buf.len().min(900);
                inner(&mut buf[..n])
            }
        };
        let mut st = Stretcher::new(constant(1.0), 0.0, false, true, SR);
        let mut src = src;
        let mut got = 0;
        let mut buf = vec![0f32; 2048 * 2];
        for _ in 0..400 {
            got += st.read(&mut buf, &mut src);
        }
        assert!(got as f64 > 1.9 * SR as f64 * 2.0 * 0.9, "{got}");
    }
}
