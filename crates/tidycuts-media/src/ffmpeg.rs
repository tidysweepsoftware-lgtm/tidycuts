use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

#[derive(Debug, thiserror::Error)]
pub enum FfmpegError {
    #[error("FFmpeg was not found. Put ffmpeg and ffprobe next to TidyCuts, install them on your PATH, or set TIDYCUTS_FFMPEG_DIR.")]
    NotFound,
    #[error("could not start {0}: {1}")]
    Spawn(&'static str, std::io::Error),
    #[error("{tool} failed: {message}")]
    Failed { tool: &'static str, message: String },
    #[error("unexpected output from ffprobe: {0}")]
    BadOutput(String),
}

/// Paths to the ffmpeg and ffprobe executables.
#[derive(Debug, Clone)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn works(path: &Path) -> bool {
    let mut cmd = Command::new(path);
    cmd.arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    no_window(&mut cmd);
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

impl Tools {
    /// Finds the tools once and caches the answer. Search order:
    /// `TIDYCUTS_FFMPEG_DIR`, the folder holding the TidyCuts executable (and
    /// its `ffmpeg` subfolder), common install folders, then `PATH`.
    pub fn get() -> Result<&'static Tools, FfmpegError> {
        static TOOLS: OnceLock<Option<Tools>> = OnceLock::new();
        TOOLS
            .get_or_init(Tools::locate)
            .as_ref()
            .ok_or(FfmpegError::NotFound)
    }

    fn locate() -> Option<Tools> {
        let mut dirs: Vec<PathBuf> = Vec::new();
        // The first name's variable still works.
        for var in ["TIDYCUTS_FFMPEG_DIR", "RUSTCUT_FFMPEG_DIR"] {
            if let Some(d) = std::env::var_os(var) {
                dirs.push(PathBuf::from(d));
            }
        }
        if let Some(d) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
        {
            dirs.push(d.join("ffmpeg"));
            // Inside a macOS .app bundle the binaries live in Contents/Resources.
            dirs.push(d.join("../Resources"));
            dirs.push(d);
        }
        for d in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
            dirs.push(PathBuf::from(d));
        }
        for dir in dirs {
            let ffmpeg = dir.join(exe("ffmpeg"));
            let ffprobe = dir.join(exe("ffprobe"));
            if ffmpeg.is_file() && ffprobe.is_file() && works(&ffmpeg) {
                return Some(Tools { ffmpeg, ffprobe });
            }
        }
        let on_path = Tools {
            ffmpeg: PathBuf::from(exe("ffmpeg")),
            ffprobe: PathBuf::from(exe("ffprobe")),
        };
        if works(&on_path.ffmpeg) && works(&on_path.ffprobe) {
            return Some(on_path);
        }
        None
    }

    pub fn ffmpeg_cmd(&self) -> Command {
        let mut cmd = Command::new(&self.ffmpeg);
        cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
        no_window(&mut cmd);
        cmd
    }

    pub fn ffprobe_cmd(&self) -> Command {
        let mut cmd = Command::new(&self.ffprobe);
        cmd.args(["-v", "error"]);
        no_window(&mut cmd);
        cmd
    }
}

/// Stops Windows from flashing a console window for every ffmpeg call.
pub(crate) fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Formats seconds for ffmpeg's `-ss` / `-t` arguments.
pub(crate) fn secs_arg(secs: f64) -> String {
    format!("{:.6}", secs.max(0.0))
}
