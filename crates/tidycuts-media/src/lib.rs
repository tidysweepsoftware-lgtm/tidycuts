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
#[cfg(windows)]
mod sapi;
pub mod speech;

pub use ffmpeg::{below_normal, FfmpegError, Tools};

/// The program that downloads files (curl, which comes with Windows 10 and 11, macOS and most
/// Linux systems). On Windows it is taken from the system folder by its full path, so it is
/// found even when the program's own search path is short (as in a packaged app).
pub fn curl() -> std::process::Command {
    #[cfg(windows)]
    let program = std::env::var_os("SystemRoot")
        .map(|r| {
            std::path::PathBuf::from(r)
                .join("System32")
                .join("curl.exe")
        })
        .filter(|p| p.is_file())
        .unwrap_or_else(|| "curl".into());
    #[cfg(not(windows))]
    let program = std::path::PathBuf::from("curl");
    let mut cmd = std::process::Command::new(program);
    ffmpeg::no_window(&mut cmd);
    cmd
}
pub use frames::{fit_within, grab_frame, RgbaFrame};
pub use player::{Player, Segment, SegmentKind};
pub use probe::probe;
