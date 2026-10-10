//! Audio output through the system's default device.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Samples waiting to be played, shared between decoder threads and the
/// device callback.
#[derive(Default)]
pub struct AudioQueue {
    samples: Mutex<VecDeque<f32>>,
    /// Frames (one sample per channel) handed to the device since the last clear.
    played: AtomicU64,
    paused: AtomicBool,
}

impl AudioQueue {
    pub fn push(&self, data: &[f32]) {
        self.samples.lock().unwrap().extend(data.iter().copied());
    }

    /// Interleaved samples still waiting to play.
    pub fn buffered(&self) -> usize {
        self.samples.lock().unwrap().len()
    }

    pub fn clear(&self) {
        self.samples.lock().unwrap().clear();
        self.played.store(0, Ordering::SeqCst);
    }

    pub fn played_frames(&self) -> u64 {
        self.played.load(Ordering::SeqCst)
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
    }

    /// Fills a device buffer. Only frames that really came from the queue
    /// advance the clock, so video waits for audio when decoding falls behind.
    fn fill<T: cpal::Sample + cpal::FromSample<f32>>(&self, out: &mut [T], channels: usize) {
        let silence = T::from_sample(0.0f32);
        if self.paused.load(Ordering::SeqCst) {
            out.iter_mut().for_each(|s| *s = silence);
            return;
        }
        let mut q = self.samples.lock().unwrap();
        let available = q.len().min(out.len()) / channels * channels;
        for (dst, src) in out.iter_mut().zip(q.drain(..available)) {
            *dst = T::from_sample(src);
        }
        out[available..].iter_mut().for_each(|s| *s = silence);
        drop(q);
        self.played
            .fetch_add((available / channels) as u64, Ordering::SeqCst);
    }
}

/// An open output stream. Must stay on the thread that created it.
pub struct AudioOut {
    _stream: cpal::Stream,
    pub queue: Arc<AudioQueue>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioOut {
    /// Opens the default output device, or returns `None` when there isn't one
    /// (playback then runs on the wall clock with no sound).
    pub fn open() -> Option<AudioOut> {
        let device = cpal::default_host().default_output_device()?;
        let supported = device.default_output_config().ok()?;
        let format = supported.sample_format();
        let config = supported.config();
        let channels = config.channels as usize;
        let queue = Arc::new(AudioQueue::default());
        let err = |e| log::warn!("audio output error: {e}");
        let q = queue.clone();
        let stream = match format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                config,
                move |d: &mut [f32], _: &cpal::OutputCallbackInfo| q.fill(d, channels),
                err,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                config,
                move |d: &mut [i16], _: &cpal::OutputCallbackInfo| q.fill(d, channels),
                err,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_output_stream(
                config,
                move |d: &mut [u16], _: &cpal::OutputCallbackInfo| q.fill(d, channels),
                err,
                None,
            ),
            cpal::SampleFormat::I32 => device.build_output_stream(
                config,
                move |d: &mut [i32], _: &cpal::OutputCallbackInfo| q.fill(d, channels),
                err,
                None,
            ),
            other => {
                log::warn!("unsupported audio sample format {other:?}");
                return None;
            }
        }
        .map_err(|e| log::warn!("could not open audio output: {e}"))
        .ok()?;
        stream.play().ok()?;
        Some(AudioOut {
            _stream: stream,
            queue,
            sample_rate: config.sample_rate,
            channels: config.channels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_counts_only_real_frames() {
        let q = AudioQueue::default();
        q.push(&[0.5; 6]);
        let mut out = [1.0f32; 10];
        q.fill(&mut out, 2);
        assert_eq!(&out[..6], &[0.5; 6]);
        assert_eq!(&out[6..], &[0.0; 4]);
        assert_eq!(q.played_frames(), 3);
        q.set_paused(true);
        q.push(&[0.5; 4]);
        q.fill(&mut out, 2);
        assert_eq!(q.played_frames(), 3);
        assert_eq!(q.buffered(), 4);
    }
}
