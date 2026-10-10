//! Media I/O for TidyCuts.
//!
//! Decoding and encoding go through the `ffmpeg` and `ffprobe` command-line
//! tools rather than linking FFmpeg's libraries. That keeps the Windows and
//! macOS builds simple: the app ships next to (or finds) the two programs and
//! talks to them over pipes.

pub mod audio;
pub mod ffmpeg;
pub mod frames;
pub mod player;
pub mod probe;
pub mod speech;

pub use ffmpeg::{FfmpegError, Tools};
pub use frames::{fit_within, grab_frame, RgbaFrame};
pub use player::{Player, Segment, SegmentKind};
pub use probe::probe;
