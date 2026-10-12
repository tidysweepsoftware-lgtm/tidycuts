//! The Image Thumbnail Generator: makes pictures from a sentence, on this
//! computer, with no account and no internet once the models are downloaded.
//!
//! The pictures are drawn by FLUX.2 [klein] 4B (Black Forest Labs, Apache 2.0),
//! with Qwen3 4B (Alibaba, Apache 2.0) reading the sentence, run by the
//! `sd-cli` program of stable-diffusion.cpp (MIT) that comes with TidyCuts, the
//! same way ffmpeg and whisper.cpp are. The model files are big, so they are
//! downloaded once, when the person says so, in one of two sizes:
//! Standard (smaller, faster, needs about 5 GB of video memory) and High quality.
//!
//! A second way to make a thumbnail keeps a real photo exact (a product, say):
//! the picture is drawn without it and the photo, cut out of its plain
//! background, is placed in with a glow and a shadow. That part is ffmpeg.

use crate::decode::KillOnDrop;
use crate::studio_voice::search_dirs;
use crate::whispercpp::{settings_dir, sha256_file};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(windows)]
const PROGRAM: &str = "sd-cli.exe";
#[cfg(not(windows))]
const PROGRAM: &str = "sd-cli";

// ---- the model files ---------------------------------------------------------

/// One file of a model set: where it comes from, its exact size and checksum
/// (so a broken download is noticed).
#[derive(Debug, Clone, Copy)]
pub struct ModelFile {
    /// The name it is kept under.
    pub name: &'static str,
    pub url: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

const KLEIN_Q4: ModelFile = ModelFile {
    name: "flux-2-klein-4b-Q4_0.gguf",
    url:
        "https://huggingface.co/leejet/FLUX.2-klein-4B-GGUF/resolve/main/flux-2-klein-4b-Q4_0.gguf",
    bytes: 2_460_378_560,
    sha256: "d1023499ef3f2f82ff7c50e6778495195c1b6cc34835741778868428111f9ff4",
};
const KLEIN_Q8: ModelFile = ModelFile {
    name: "flux-2-klein-4b-Q8_0.gguf",
    url:
        "https://huggingface.co/leejet/FLUX.2-klein-4B-GGUF/resolve/main/flux-2-klein-4b-Q8_0.gguf",
    bytes: 4_300_629_440,
    sha256: "0bba6951258ec8f92d51114a8fa13e66828297bfff58a738f52729b3ef66fa28",
};
const QWEN_Q4: ModelFile = ModelFile {
    name: "Qwen3-4B-Q4_K_M.gguf",
    url: "https://huggingface.co/unsloth/Qwen3-4B-GGUF/resolve/main/Qwen3-4B-Q4_K_M.gguf",
    bytes: 2_497_281_312,
    sha256: "f6f851777709861056efcdad3af01da38b31223a3ba26e61a4f8bf3a2195813a",
};
const QWEN_Q8: ModelFile = ModelFile {
    name: "Qwen3-4B-Q8_0.gguf",
    url: "https://huggingface.co/unsloth/Qwen3-4B-GGUF/resolve/main/Qwen3-4B-Q8_0.gguf",
    bytes: 4_280_405_792,
    sha256: "eed555233267a33c7e8ee31682762cc7751b3f6d224039086e0e846f05fffa5d",
};
/// The picture decoder, from the same Apache 2.0 repository as the model.
const VAE: ModelFile = ModelFile {
    name: "flux2-klein-vae.safetensors",
    url: "https://huggingface.co/black-forest-labs/FLUX.2-klein-4B/resolve/main/vae/diffusion_pytorch_model.safetensors",
    bytes: 168_120_878,
    sha256: "ca70d2202afe6415bdbcb8793ba8cd99fd159cfe6192381504d6c4d3036e0f04",
};

/// The two sizes of the models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Standard,
    High,
}

impl Quality {
    pub const ALL: [Quality; 2] = [Quality::Standard, Quality::High];

    /// The name saved in the settings.
    pub fn id(self) -> &'static str {
        match self {
            Quality::Standard => "standard",
            Quality::High => "high",
        }
    }

    pub fn from_id(id: &str) -> Quality {
        if id == "high" {
            Quality::High
        } else {
            Quality::Standard
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Quality::Standard => "Standard",
            Quality::High => "High quality",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Quality::Standard => {
                "Smaller and faster. Looked just as good in our tests. Needs a graphics card with about 5 GB of memory."
            }
            Quality::High => {
                "The larger version of the same models: a little more detail, a bigger download, slower. Needs a graphics card with about 8 GB of memory."
            }
        }
    }

    pub fn files(self) -> [ModelFile; 3] {
        match self {
            Quality::Standard => [KLEIN_Q4, QWEN_Q4, VAE],
            Quality::High => [KLEIN_Q8, QWEN_Q8, VAE],
        }
    }

    /// The size of the download, in bytes.
    pub fn bytes(self) -> u64 {
        self.files().iter().map(|f| f.bytes).sum()
    }
}

/// The folder the model files are kept in (`TIDYCUTS_IMAGEGEN_MODELS` overrides).
pub fn models_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("TIDYCUTS_IMAGEGEN_MODELS") {
        return Some(PathBuf::from(d));
    }
    Some(settings_dir()?.join("models").join("imagegen"))
}

fn file_is_complete(dir: &Path, f: &ModelFile) -> bool {
    std::fs::metadata(dir.join(f.name)).is_ok_and(|m| m.len() == f.bytes)
}

/// Whether a size of the models is downloaded.
pub fn installed(q: Quality) -> bool {
    models_dir().is_some_and(|d| q.files().iter().all(|f| file_is_complete(&d, f)))
}

/// Deletes a size of the models (a file the other size also needs stays).
pub fn remove(q: Quality) -> Result<(), String> {
    let dir = models_dir().ok_or("There is no place the models are kept.")?;
    let other = if q == Quality::Standard {
        Quality::High
    } else {
        Quality::Standard
    };
    let keep: Vec<&str> = if installed(other) {
        other.files().iter().map(|f| f.name).collect()
    } else {
        Vec::new()
    };
    for f in q.files() {
        if !keep.contains(&f.name) {
            let _ = std::fs::remove_file(dir.join(f.name));
        }
        let _ = std::fs::remove_file(dir.join(format!("{}.part", f.name)));
    }
    Ok(())
}

/// The `sd-cli` program, if it is installed (`TIDYCUTS_IMAGEGEN_DIR` overrides
/// the folder).
pub fn program_file() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("TIDYCUTS_IMAGEGEN_DIR") {
        let p = PathBuf::from(d).join(PROGRAM);
        return p.is_file().then_some(p);
    }
    search_dirs()
        .into_iter()
        .flat_map(|d| [d.join("imagegen").join(PROGRAM), d.join(PROGRAM)])
        .find(|p| p.is_file())
}

/// Whether pictures can be made now with this size of the models.
pub fn available(q: Quality) -> bool {
    program_file().is_some() && installed(q)
}

// ---- downloading -------------------------------------------------------------

/// Downloads a size of the models into the settings folder, one file after
/// the other, checking each one's size and checksum. A download that was
/// interrupted carries on where it stopped. `progress` gets what it is doing
/// and 0..1. This only runs when the person said to download.
pub fn download(
    q: Quality,
    cancel: &AtomicBool,
    progress: impl FnMut(&str, f64),
) -> Result<(), String> {
    let dir = models_dir().ok_or("There is no place to keep the models.")?;
    download_files(&q.files(), &dir, cancel, progress)
}

fn download_files(
    files: &[ModelFile],
    dir: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(&str, f64),
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Could not make the models folder: {e}"))?;
    let missing: Vec<&ModelFile> = files.iter().filter(|f| !file_is_complete(dir, f)).collect();
    let total: u64 = missing.iter().map(|f| f.bytes).sum();
    let mut done_before = 0u64;
    for f in missing {
        let part = dir.join(format!("{}.part", f.name));
        let got = std::fs::metadata(&part).map_or(0, |m| m.len());
        if got > f.bytes {
            let _ = std::fs::remove_file(&part);
        }
        let mut attempt = 0;
        loop {
            let have = std::fs::metadata(&part).map_or(0, |m| m.len());
            if have == f.bytes {
                break;
            }
            attempt += 1;
            match run_curl(f, &part, cancel, |got| {
                progress(
                    "Downloading",
                    ((done_before + got) as f64 / total as f64).clamp(0.0, 0.99),
                );
            }) {
                Ok(()) => break,
                Err(e) if e == "Cancelled." => return Err(e),
                // A server that cannot carry on a half download: start over once.
                Err(_) if attempt == 1 && have > 0 => {
                    let _ = std::fs::remove_file(&part);
                }
                Err(e) => return Err(format!("Could not download {} ({e}).", f.name)),
            }
        }
        if std::fs::metadata(&part).map_or(0, |m| m.len()) != f.bytes {
            let _ = std::fs::remove_file(&part);
            return Err(format!("The download of {} is incomplete.", f.name));
        }
        progress(
            "Checking the download",
            ((done_before + f.bytes) as f64 / total as f64).clamp(0.0, 0.99),
        );
        if sha256_file(&part).map_err(|e| e.to_string())? != f.sha256 {
            let _ = std::fs::remove_file(&part);
            return Err(format!(
                "{} is damaged (its checksum is wrong). Please try again.",
                f.name
            ));
        }
        std::fs::rename(&part, dir.join(f.name)).map_err(|e| e.to_string())?;
        done_before += f.bytes;
    }
    progress("Done", 1.0);
    Ok(())
}

/// Runs curl for one file, carrying on from a partial file. `progress` gets
/// the bytes the part file holds.
fn run_curl(
    f: &ModelFile,
    part: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<(), String> {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-L", "--fail", "-sS", "--retry", "3", "-C", "-", "-o"])
        .arg(part)
        .arg(f.url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    no_window(&mut cmd);
    let mut child = KillOnDrop(
        cmd.spawn()
            .map_err(|_| "Could not start the program that downloads files (curl).".to_string())?,
    );
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled.".into());
        }
        match child.0.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                progress(std::fs::metadata(part).map_or(0, |m| m.len()));
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    if status.success() {
        return Ok(());
    }
    let mut msg = String::new();
    if let Some(mut e) = child.0.stderr.take() {
        let _ = e.read_to_string(&mut msg);
    }
    Err(msg.trim().to_string())
}

fn no_window(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

// ---- making pictures ---------------------------------------------------------

/// A look the pictures are drawn in: (id, name shown, words added to the idea).
pub const STYLES: &[(&str, &str, &str)] = &[
    (
        "bold",
        "Bold and bright",
        "bold, bright, high contrast, vivid saturated colors, sharp detail, eye-catching",
    ),
    (
        "cinematic",
        "Cinematic",
        "cinematic movie still, dramatic lighting, rich color grading, shallow depth of field",
    ),
    (
        "dark",
        "Dark and moody",
        "dark and moody, deep shadows, glowing accent light, high contrast",
    ),
    (
        "clean",
        "Clean and simple",
        "clean and simple, soft studio lighting, uncluttered background, bright colors",
    ),
    (
        "cartoon",
        "Cartoon",
        "colorful cartoon illustration, bold outlines, playful, vibrant",
    ),
];

/// What is asked of the model: the idea, the look, and the things it must not
/// draw by itself (it makes up words and logos if it is not told not to; the
/// words on a thumbnail are added afterwards, in letters that are always right).
pub fn full_prompt(idea: &str, style: usize) -> String {
    let look = STYLES.get(style).map_or("", |s| s.2);
    format!(
        "{}, {look}, no text, no letters, no words, no logos, no watermark",
        idea.trim().trim_end_matches(['.', ',']),
    )
}

/// The most inspiration pictures that can be given.
pub const MAX_INSPIRATION: usize = 3;

/// The prompt when pictures are given along with the idea: the first picture
/// may be a person to draw in, the rest are inspiration whose colours,
/// lighting and mood the new picture copies (said this firmly, because a
/// softer wording hardly changes the look).
pub fn with_references(prompt: &str, person: bool, inspiration: usize) -> String {
    let mut out = String::new();
    if inspiration > 0 {
        let which = if inspiration == 1 {
            "reference image"
        } else {
            "reference images"
        };
        let from = if person { "the other " } else { "the " };
        out.push_str(&format!(
            "Create a new image that copies the exact color palette, lighting, atmosphere and mood of {from}{which}. "
        ));
    }
    if person {
        out.push_str("The person from the first reference image appears in it. ");
    }
    if inspiration > 0 || person {
        out.push_str("New subject: ");
    }
    out.push_str(prompt);
    out
}

#[derive(Debug, Clone)]
pub struct Request {
    pub prompt: String,
    pub width: u32,
    pub height: u32,
    /// How many pictures.
    pub count: u32,
    pub seed: u64,
    /// Pictures the model looks at: a person to draw in (the face comes out
    /// similar, not identical) and/or inspiration for the look.
    pub references: Vec<PathBuf>,
    pub quality: Quality,
}

/// The program's command line.
fn build_args(req: &Request, models: &Path, out: &Path) -> Vec<OsString> {
    let q = req.quality.files();
    let mut a: Vec<OsString> = Vec::new();
    let mut push = |s: &str, v: OsString| {
        a.push(s.into());
        a.push(v);
    };
    push("--diffusion-model", models.join(q[0].name).into());
    push("--llm", models.join(q[1].name).into());
    push("--vae", models.join(q[2].name).into());
    push("-p", req.prompt.clone().into());
    push("--cfg-scale", "1.0".into());
    push("--steps", "4".into());
    // The model wants sizes in steps of 16.
    push("-W", (req.width / 16 * 16).max(256).to_string().into());
    push("-H", (req.height / 16 * 16).max(256).to_string().into());
    push("-b", req.count.max(1).to_string().into());
    push("-s", req.seed.to_string().into());
    for r in &req.references {
        push("-r", r.clone().into());
    }
    push("-o", out.into());
    a.push("--diffusion-fa".into());
    a
}

/// Follows what the program prints, to say how far along it is.
#[derive(Debug, Default)]
struct Progress {
    count: u32,
    image: u32,
    /// The step of the picture being drawn, 0..1.
    step: f64,
    decoded: u32,
}

impl Progress {
    /// Reads one piece of the program's output.
    fn feed(&mut self, line: &str) {
        let number_pair = |s: &str| -> Option<(u32, u32)> {
            let token = s.split_whitespace().next()?;
            let (a, b) = token.split_once('/')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        };
        if let Some(rest) = line.split("generating image: ").nth(1) {
            if let Some((i, n)) = number_pair(rest) {
                self.image = i;
                self.count = n;
                self.step = 0.0;
            }
        } else if line.contains("latent") && line.contains("decoded") {
            self.decoded += 1;
        } else if let Some(bar) = line.rsplit('|').next().filter(|_| line.contains('|')) {
            if let Some((k, b)) = number_pair(bar.trim_start()) {
                if b > 0 {
                    self.step = f64::from(k) / f64::from(b);
                }
            }
        }
    }

    /// How far along, 0..1: drawing takes most of the time, then decoding.
    fn fraction(&self) -> f64 {
        let n = f64::from(self.count.max(1));
        if self.decoded > 0 {
            return (0.85 + 0.15 * f64::from(self.decoded) / n).min(0.99);
        }
        if self.image == 0 {
            return 0.0;
        }
        (((f64::from(self.image) - 1.0) + self.step) / n * 0.85).min(0.85)
    }
}

/// Words that tell what went wrong, from the program's last lines.
fn explain_failure(tail: &[String]) -> String {
    let all = tail.join(" ").to_lowercase();
    if all.contains("out of device memory")
        || all.contains("out of memory")
        || all.contains("outofdevicememory")
        || all.contains("failed to allocate")
    {
        return "The graphics card ran out of memory. Try the Standard models, a smaller shape, or close other programs that use the graphics card.".into();
    }
    if all.contains("vulkan") && (all.contains("no device") || all.contains("not found")) {
        return "No graphics card that can make pictures was found.".into();
    }
    match tail.iter().rev().find(|l| {
        let l = l.to_lowercase();
        l.contains("error") || l.contains("failed")
    }) {
        Some(l) => format!("The image program stopped: {}", l.trim()),
        None => "The image program stopped without making a picture.".into(),
    }
}

/// Makes `req.count` pictures into `out_dir` and returns their files. `progress`
/// gets 0..1. Stops when `cancel` is set.
pub fn generate(
    req: &Request,
    out_dir: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f64),
) -> Result<Vec<PathBuf>, String> {
    let program =
        program_file().ok_or("The image program that comes with TidyCuts was not found.")?;
    let models = models_dir().ok_or("There is no place the models are kept.")?;
    if !installed(req.quality) {
        return Err("The image models are not downloaded yet.".into());
    }
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let out = out_dir.join("picture.png");
    // Pictures from an earlier run must not be taken for new ones.
    for i in 0..=req.count {
        let _ = std::fs::remove_file(out_dir.join(format!("picture_{i}.png")));
    }
    let _ = std::fs::remove_file(&out);
    let mut cmd = std::process::Command::new(&program);
    cmd.args(build_args(req, &models, &out))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    if let Some(dir) = program.parent() {
        cmd.current_dir(dir);
    }
    no_window(&mut cmd);
    let mut child = KillOnDrop(
        cmd.spawn()
            .map_err(|e| format!("Could not start the image program: {e}"))?,
    );
    // The program draws its progress with carriage returns; read it on its own
    // thread so that stopping is never held up.
    let state = Arc::new(Mutex::new((Progress::default(), Vec::<String>::new())));
    let reader = {
        let mut err = child
            .0
            .stderr
            .take()
            .ok_or("No output from the image program.")?;
        let state = state.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut line = Vec::new();
            loop {
                let n = match err.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                for &b in &buf[..n] {
                    if b == b'\r' || b == b'\n' {
                        if !line.is_empty() {
                            let text = String::from_utf8_lossy(&line).into_owned();
                            let mut s = state.lock().unwrap();
                            s.0.feed(&text);
                            if !text.contains('|') {
                                s.1.push(text);
                                if s.1.len() > 12 {
                                    s.1.remove(0);
                                }
                            }
                            line.clear();
                        }
                    } else {
                        line.push(b);
                    }
                }
            }
        })
    };
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled.".into());
        }
        match child.0.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {
                progress(state.lock().unwrap().0.fraction());
                std::thread::sleep(Duration::from_millis(120));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    let _ = reader.join();
    let files: Vec<PathBuf> = if req.count <= 1 {
        vec![out.clone()]
    } else {
        (0..req.count)
            .map(|i| out_dir.join(format!("picture_{i}.png")))
            .collect()
    };
    let made: Vec<PathBuf> = files.into_iter().filter(|p| p.is_file()).collect();
    if !status.success() || made.is_empty() {
        let tail = state.lock().unwrap().1.clone();
        return Err(explain_failure(&tail));
    }
    progress(1.0);
    Ok(made)
}

// ---- a real photo in a drawn scene -------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Centre,
    Right,
}

/// How a photo is placed into a picture.
#[derive(Debug, Clone, Copy)]
pub struct Placement {
    /// The photo has a plain white background to take out.
    pub remove_white: bool,
    pub side: Side,
    /// The colour of the glow behind it.
    pub glow: [u8; 3],
}

/// Glow colours to choose from: (name, colour).
pub const GLOWS: &[(&str, [u8; 3])] = &[
    ("Orange", [255, 110, 20]),
    ("Blue", [30, 140, 255]),
    ("Green", [40, 220, 90]),
    ("Pink", [255, 60, 170]),
    ("White", [255, 255, 255]),
];

/// Space kept around the photo while it is prepared, so its glow and shadow
/// are not cut off.
const PAD: u32 = 160;

/// The ffmpeg filter that places the photo (second input) into the picture
/// (first input): the picture is made a little darker and richer, the photo
/// loses its plain background, gets a glow and a soft shadow, and sits at the
/// side, as large as the picture allows.
fn placement_filter(width: u32, height: u32, p: &Placement) -> String {
    let (w, h) = (f64::from(width), f64::from(height));
    let (box_w, box_h) = ((w * 0.78).round() as u32, (h * 0.9).round() as u32);
    let key = if p.remove_white {
        ",colorkey=0xFFFFFF:0.13:0.08"
    } else {
        ""
    };
    let (x, y) = match p.side {
        Side::Left => (format!("0.02*W-{PAD}"), format!("H-h+{PAD}-0.03*H")),
        Side::Centre => ("(W-w)/2".to_string(), format!("H-h+{PAD}-0.03*H")),
        Side::Right => (format!("W-w+{PAD}-0.02*W"), format!("H-h+{PAD}-0.03*H")),
    };
    let [r, g, b] = p.glow;
    format!(
        "[0:v]scale={width}:{height},eq=brightness=-0.04:saturation=1.2,vignette=PI/6[bg];\
         [1:v]format=rgba{key},scale={box_w}:{box_h}:force_original_aspect_ratio=decrease,\
         pad=w=iw+{pad2}:h=ih+{pad2}:x={PAD}:y={PAD}:color=black@0[prod];\
         [prod]split=3[pa][pb][pc];\
         [pa]lutrgb=r={r}:g={g}:b={b},gblur=sigma=40[glow];\
         [pb]lutrgb=r=0:g=0:b=0,gblur=sigma=16[shadow];\
         [bg][glow]overlay=x={x}:y={y}:format=auto[b1];\
         [b1][shadow]overlay=x={x}+15:y={y}+30:format=auto[b2];\
         [b2][pc]overlay=x={x}:y={y}:format=auto",
        pad2 = PAD * 2,
    )
}

/// Places `photo` into `background` and writes the result as a picture.
pub fn place_photo(
    background: &Path,
    photo: &Path,
    out: &Path,
    width: u32,
    height: u32,
    p: &Placement,
) -> Result<(), String> {
    let tools = tidycuts_media::Tools::get().map_err(|e| e.to_string())?;
    let mut cmd = tools.ffmpeg_cmd();
    cmd.args(["-y", "-i"])
        .arg(background)
        .arg("-i")
        .arg(photo)
        .args(["-filter_complex", &placement_filter(width, height, p)])
        .args(["-frames:v", "1"])
        .arg(out);
    let r = cmd.output().map_err(|e| e.to_string())?;
    if r.status.success() && out.is_file() {
        Ok(())
    } else {
        Err(format!(
            "Could not place the photo: {}",
            String::from_utf8_lossy(&r.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_sizes_have_the_sizes_we_say() {
        assert_eq!(Quality::Standard.bytes(), 5_125_780_750);
        assert_eq!(Quality::High.bytes(), 8_749_156_110);
        // The decoder is shared, so it is downloaded once.
        assert_eq!(
            Quality::Standard.files()[2].name,
            Quality::High.files()[2].name
        );
        for q in Quality::ALL {
            assert_eq!(Quality::from_id(q.id()), q);
        }
        assert_eq!(Quality::from_id("nonsense"), Quality::Standard);
    }

    #[test]
    fn every_file_has_a_full_checksum_and_a_web_address() {
        for q in Quality::ALL {
            for f in q.files() {
                assert_eq!(f.sha256.len(), 64, "{}", f.name);
                assert!(f.sha256.chars().all(|c| c.is_ascii_hexdigit()));
                assert!(f.url.starts_with("https://huggingface.co/"));
                assert!(f.bytes > 100_000_000);
            }
        }
    }

    #[test]
    fn the_prompt_forbids_made_up_words() {
        let p = full_prompt("a shocked man pointing at a red button.", 0);
        assert!(p.starts_with("a shocked man pointing at a red button, bold"));
        assert!(p.contains("no text, no letters"));
        // An unknown look is just left out.
        assert!(full_prompt("a cat", 99).contains("no logos"));
    }

    #[test]
    fn pictures_given_with_the_idea_are_explained_to_the_model() {
        assert_eq!(with_references("a cat", false, 0), "a cat");
        let p = with_references("a cat", false, 1);
        assert!(p.starts_with("Create a new image that copies the exact color palette"));
        assert!(p.contains("of the reference image. New subject: a cat"));
        assert!(with_references("a cat", false, 2).contains("reference images"));
        let both = with_references("a cat", true, 2);
        assert!(both.contains("of the other reference images"));
        assert!(both.contains("The person from the first reference image"));
        assert_eq!(
            with_references("a cat", true, 0),
            "The person from the first reference image appears in it. New subject: a cat"
        );
    }

    #[test]
    fn the_command_line_uses_the_chosen_files_and_whole_steps_of_16() {
        let req = Request {
            prompt: "a cat".into(),
            width: 1280,
            height: 725,
            count: 4,
            seed: 9,
            references: vec![PathBuf::from("me.png"), PathBuf::from("look.png")],
            quality: Quality::High,
        };
        let a: Vec<String> = build_args(&req, Path::new("/m"), Path::new("/o/picture.png"))
            .iter()
            .map(|s| s.to_string_lossy().replace('\\', "/"))
            .collect();
        let at = |flag: &str| {
            let i = a.iter().position(|s| s == flag).expect(flag);
            a[i + 1].clone()
        };
        assert!(at("--diffusion-model").ends_with("flux-2-klein-4b-Q8_0.gguf"));
        assert!(at("--llm").ends_with("Qwen3-4B-Q8_0.gguf"));
        assert!(at("--vae").ends_with("flux2-klein-vae.safetensors"));
        assert_eq!(at("-W"), "1280");
        assert_eq!(at("-H"), "720");
        assert_eq!(at("-b"), "4");
        assert_eq!(at("-s"), "9");
        assert_eq!(at("-r"), "me.png");
        assert_eq!(a.iter().filter(|s| *s == "-r").count(), 2);
        assert_eq!(at("--steps"), "4");
    }

    #[test]
    fn progress_follows_what_the_program_prints() {
        let mut p = Progress::default();
        assert_eq!(p.fraction(), 0.0);
        p.feed("[I] generating image: 1/2 - seed 5 --- image.cpp:866");
        p.feed("  |=========================>                        | 2/4 - 5.15it/s");
        assert!(
            (p.fraction() - 0.5 / 2.0 * 0.85).abs() < 1e-9,
            "{}",
            p.fraction()
        );
        p.feed("[I] generating image: 2/2 - seed 6 --- image.cpp:866");
        p.feed("  |==================================================| 4/4 - 5.28it/s");
        assert!((p.fraction() - 0.85).abs() < 1e-9);
        p.feed("[I] latent 1 decoded, taking 1.10s --- image.cpp:620");
        assert!(p.fraction() > 0.85 && p.fraction() < 0.99);
        p.feed("[I] latent 2 decoded, taking 0.87s --- image.cpp:620");
        assert!((p.fraction() - 0.99).abs() < 1e-9);
        // Other lines change nothing.
        p.feed("[I] loading tensors completed, taking 1.45s");
        assert!((p.fraction() - 0.99).abs() < 1e-9);
    }

    #[test]
    fn failures_are_explained_in_plain_words() {
        let mem = vec!["ErrorOutOfDeviceMemory".to_string()];
        assert!(explain_failure(&mem).contains("ran out of memory"));
        let other = vec!["[E] something failed to load".to_string()];
        assert!(explain_failure(&other).contains("something failed to load"));
        assert!(explain_failure(&[]).contains("without making a picture"));
    }

    #[test]
    fn the_filter_puts_the_photo_on_the_chosen_side_and_keys_out_white_only_when_asked() {
        let mut p = Placement {
            remove_white: true,
            side: Side::Right,
            glow: [255, 110, 20],
        };
        let f = placement_filter(1280, 720, &p);
        assert!(f.contains("colorkey=0xFFFFFF"));
        assert!(f.contains("lutrgb=r=255:g=110:b=20"));
        assert!(f.contains("overlay=x=W-w+160-0.02*W"));
        assert!(f.contains("scale=998:648:force_original_aspect_ratio=decrease"));
        p.remove_white = false;
        p.side = Side::Left;
        let f = placement_filter(1280, 720, &p);
        assert!(!f.contains("colorkey"));
        assert!(f.contains("0.02*W-160"));
    }

    #[test]
    fn a_photo_is_really_placed_into_a_picture() {
        let Ok(tools) = tidycuts_media::Tools::get() else {
            eprintln!("no ffmpeg here; skipping");
            return;
        };
        let dir = std::env::temp_dir().join(format!("tidycuts-place-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (bg, photo, out) = (
            dir.join("bg.png"),
            dir.join("photo.png"),
            dir.join("out.png"),
        );
        // A blue picture, and a dark square on a white background.
        let ok = tools
            .ffmpeg_cmd()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=640x360:d=1",
                "-frames:v",
                "1",
            ])
            .arg(&bg)
            .status()
            .unwrap()
            .success()
            && tools
                .ffmpeg_cmd()
                .args(["-y", "-f", "lavfi", "-i", "color=c=white:s=300x300:d=1"])
                .args([
                    "-vf",
                    "drawbox=x=75:y=75:w=150:h=150:color=black:t=fill",
                    "-frames:v",
                    "1",
                ])
                .arg(&photo)
                .status()
                .unwrap()
                .success();
        assert!(ok);
        let p = Placement {
            remove_white: true,
            side: Side::Right,
            glow: [255, 110, 20],
        };
        place_photo(&bg, &photo, &out, 640, 360, &p).unwrap();
        // The result is a 640x360 picture whose right side holds the dark square
        // and whose left side is still the blue picture.
        let probe = tidycuts_media::probe(&out).unwrap();
        assert_eq!((probe.width, probe.height), (640, 360));
        let pixel = |x: u32, y: u32| -> [u8; 3] {
            let o = tools
                .ffmpeg_cmd()
                .args(["-i"])
                .arg(&out)
                .args([
                    "-vf",
                    &format!("crop=1:1:{x}:{y}"),
                    "-f",
                    "rawvideo",
                    "-pix_fmt",
                    "rgb24",
                    "-",
                ])
                .output()
                .unwrap();
            [o.stdout[0], o.stdout[1], o.stdout[2]]
        };
        let left = pixel(40, 180);
        assert!(
            left[2] > left[0] && left[2] > 100,
            "left should still be blue: {left:?}"
        );
        let dark = (0..360)
            .step_by(6)
            .flat_map(|y| (320..640).step_by(6).map(move |x| (x, y)))
            .filter(|&(x, y)| pixel(x, y).iter().all(|c| *c < 60))
            .count();
        assert!(
            dark > 20,
            "the dark square should be in the right half ({dark})"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn files_download_check_and_resume() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("tidycuts-dl-{}", std::process::id()));
        let src = dir.join("src");
        let dst = dir.join("dst");
        std::fs::create_dir_all(&src).unwrap();
        let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::File::create(src.join("a.bin"))
            .unwrap()
            .write_all(&data)
            .unwrap();
        let sha = sha256_file(&src.join("a.bin")).unwrap();
        let url: &'static str = Box::leak(
            format!(
                "file:///{}",
                src.join("a.bin").display().to_string().replace('\\', "/")
            )
            .into_boxed_str(),
        );
        let sha: &'static str = Box::leak(sha.into_boxed_str());
        let good = ModelFile {
            name: "a.bin",
            url,
            bytes: data.len() as u64,
            sha256: sha,
        };
        let cancel = AtomicBool::new(false);
        let mut last = 0.0;
        let mut stages = Vec::new();
        // A half-finished download from an earlier try is carried on.
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(dst.join("a.bin.part"), &data[..100_000]).unwrap();
        let r = download_files(&[good], &dst, &cancel, |s, f| {
            assert!(f >= last - 1e-9, "progress went backwards");
            last = f;
            if stages.last().map(String::as_str) != Some(s) {
                stages.push(s.to_string());
            }
        });
        assert_eq!(r, Ok(()));
        assert_eq!(std::fs::read(dst.join("a.bin")).unwrap(), data);
        assert!(!dst.join("a.bin.part").exists());
        assert_eq!(stages.last().unwrap(), "Done");
        assert!(stages.iter().any(|s| s == "Checking the download"));
        // A file that is already there is not fetched again.
        let r = download_files(&[good], &dst, &cancel, |s, _| assert_eq!(s, "Done"));
        assert_eq!(r, Ok(()));
        // A wrong checksum is refused and nothing is kept.
        let zeros: &'static str = "0".repeat(64).leak();
        let bad = ModelFile {
            sha256: zeros,
            ..good
        };
        let dst2 = dir.join("dst2");
        let r = download_files(&[bad], &dst2, &cancel, |_, _| {});
        assert!(r.unwrap_err().contains("damaged"));
        assert!(!dst2.join("a.bin").exists() && !dst2.join("a.bin.part").exists());
        // Stopping works.
        cancel.store(true, Ordering::Relaxed);
        let dst3 = dir.join("dst3");
        assert_eq!(
            download_files(&[good], &dst3, &cancel, |_, _| {}),
            Err("Cancelled.".into())
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
