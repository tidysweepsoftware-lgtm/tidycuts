//! Keeps the image program running in the background, so the models are loaded
//! into the graphics card once instead of at every press of "Make pictures".
//!
//! The program is `sd-server` (stable-diffusion.cpp). It listens only on this
//! computer (127.0.0.1) on a free port. The first picture still waits while the
//! models load (which can start early, see [`warm_up`]); every later one starts
//! drawing at once. After a few idle minutes it is stopped, giving the graphics
//! card back. If it cannot be started, [`generate`] says so and the caller
//! falls back to running the one-shot `sd-cli` program as before.

use crate::decode::KillOnDrop;
use crate::imagegen::{models_dir, program_file, Quality, Request};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

#[cfg(windows)]
const SERVER: &str = "sd-server.exe";
#[cfg(not(windows))]
const SERVER: &str = "sd-server";

/// How long the program may sit unused before it is stopped.
pub const IDLE_LIMIT: Duration = Duration::from_secs(5 * 60);
/// How long the models may take to load before giving up.
const START_LIMIT: Duration = Duration::from_secs(8 * 60);

/// What the program says it is doing right now, from its own messages.
#[derive(Debug, Default)]
pub(crate) struct Live {
    text_done: bool,
    /// Drawing passes that finished (a refined picture has two).
    passes_done: u32,
    /// The step of the pass being drawn, 0..1.
    step: f64,
    decoding: bool,
}

impl Live {
    fn feed(&mut self, line: &str) {
        if line.contains("get_learned_condition completed") {
            self.text_done = true;
        } else if line.contains("sampling completed") {
            self.passes_done += 1;
            self.step = 0.0;
        } else if line.contains("decoding") && line.contains("latent") || line.contains("decoded") {
            self.decoding = true;
        } else if let Some(bar) = line.rsplit('|').next().filter(|_| line.contains('|')) {
            if let Some((k, n)) = bar
                .split_whitespace()
                .next()
                .and_then(|t| t.split_once('/'))
                .and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?)))
            {
                if n > 0 {
                    self.step = f64::from(k) / f64::from(n);
                }
            }
        }
    }

    /// How far one picture is, 0..1 (`passes`: 2 when it is drawn and then refined).
    fn fraction(&self, passes: u32) -> f64 {
        if self.decoding {
            return 0.93;
        }
        let text = if self.text_done { 0.1 } else { 0.0 };
        let drawn = (f64::from(self.passes_done) + self.step) / f64::from(passes.max(1));
        (text + 0.8 * drawn.min(1.0)).min(0.92)
    }
}

struct Running {
    live: Arc<Mutex<Live>>,
    child: KillOnDrop,
    port: u16,
    quality: Quality,
    used: Instant,
}

static SERVICE: Mutex<Option<Running>> = Mutex::new(None);
static WATCHER: Once = Once::new();
/// The seconds one picture took last time (in thousandths), to show progress.
static SECS_PER_PICTURE: AtomicU64 = AtomicU64::new(6_000);

/// Why a request to the background program did not give pictures.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// The program could not be started or reached: run it the one-shot way.
    Unavailable(String),
    /// The person stopped it.
    Cancelled,
    /// It ran and said it could not make the picture.
    Failed(String),
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

fn free_port() -> Option<u16> {
    let l = std::net::TcpListener::bind("127.0.0.1:0").ok()?;
    l.local_addr().ok().map(|a| a.port())
}

/// The server program next to the one-shot program.
fn server_file() -> Option<PathBuf> {
    let p = program_file()?.parent()?.join(SERVER);
    p.is_file().then_some(p)
}

/// One call to the background program, with curl (which TidyCuts already uses).
fn call(method: &str, url: &str, body: Option<&Path>, secs: u32) -> Result<Value, String> {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sS", "-m", &secs.to_string(), "-X", method])
        .args(["-H", "Content-Type: application/json"]);
    if let Some(b) = body {
        cmd.arg("--data-binary").arg(format!("@{}", b.display()));
    }
    cmd.arg(url).stdin(Stdio::null()).stderr(Stdio::piped());
    no_window(&mut cmd);
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("unreadable answer: {e}"))
}

// ---- base64 (for the pictures that go in and come out) ----------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(crate) fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub(crate) fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0);
    for b in text.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\r' | b'\n' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ---- starting and stopping ---------------------------------------------------

fn start(q: Quality, cancel: &AtomicBool) -> Result<Running, Error> {
    let server = server_file().ok_or_else(|| Error::Unavailable("no sd-server program".into()))?;
    let dir = models_dir().ok_or_else(|| Error::Unavailable("no models folder".into()))?;
    if !crate::imagegen::installed(q) {
        return Err(Error::Unavailable("models not downloaded".into()));
    }
    let port = free_port().ok_or_else(|| Error::Unavailable("no free port".into()))?;
    let f = q.files();
    let mut cmd = std::process::Command::new(&server);
    cmd.arg("--diffusion-model")
        .arg(dir.join(f[0].name))
        .arg("--llm")
        .arg(dir.join(f[1].name))
        .arg("--vae")
        .arg(dir.join(f[2].name))
        .args(["--diffusion-fa", "--cfg-scale", "1.0"])
        .args([
            "--listen-ip",
            "127.0.0.1",
            "--listen-port",
            &port.to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let Some(d) = server.parent() {
        cmd.current_dir(d);
    }
    no_window(&mut cmd);
    let mut child = KillOnDrop(
        cmd.spawn()
            .map_err(|e| Error::Unavailable(format!("could not start sd-server: {e}")))?,
    );
    // Its messages say how far a picture is; read them on a thread of their own.
    let live = Arc::new(Mutex::new(Live::default()));
    if let Some(mut err) = child.0.stderr.take() {
        let live = live.clone();
        std::thread::spawn(move || {
            let (mut buf, mut line) = ([0u8; 4096], Vec::new());
            loop {
                let n = match err.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                for &b in &buf[..n] {
                    if b == b'\r' || b == b'\n' {
                        if !line.is_empty() {
                            let text = String::from_utf8_lossy(&line).into_owned();
                            live.lock().unwrap_or_else(|e| e.into_inner()).feed(&text);
                            line.clear();
                        }
                    } else {
                        line.push(b);
                    }
                }
            }
        });
    }
    let began = Instant::now();
    let url = format!("http://127.0.0.1:{port}/sdcpp/v1/capabilities");
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        if let Ok(Some(s)) = child.0.try_wait() {
            return Err(Error::Unavailable(format!("sd-server stopped ({s})")));
        }
        if began.elapsed() > START_LIMIT {
            return Err(Error::Unavailable("sd-server took too long to load".into()));
        }
        if call("GET", &url, None, 3).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    watch();
    Ok(Running {
        live,
        child,
        port,
        quality: q,
        used: Instant::now(),
    })
}

/// A helper that stops the program after it has been idle for a while.
fn watch() {
    WATCHER.call_once(|| {
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_secs(15));
            stop_if_idle(IDLE_LIMIT);
        });
    });
}

/// Stops the program if it has not been used for `limit`.
pub fn stop_if_idle(limit: Duration) {
    if let Ok(mut s) = SERVICE.try_lock() {
        if s.as_ref().is_some_and(|r| r.used.elapsed() > limit) {
            *s = None;
        }
    }
}

/// Stops the program now (the app is closing, or the models were deleted).
pub fn shutdown() {
    *SERVICE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Whether the program is running with the models loaded.
pub fn is_running() -> bool {
    SERVICE.try_lock().is_ok_and(|s| s.is_some())
}

/// The port of the running program, starting it (and waiting while the
/// models load) when needed.
fn ensure(q: Quality, cancel: &AtomicBool) -> Result<(u16, Arc<Mutex<Live>>), Error> {
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        // Someone else may be starting it: wait for the lock, but stay stoppable.
        let Ok(mut guard) = SERVICE.try_lock() else {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        };
        if let Some(r) = guard.as_mut() {
            let alive = matches!(r.child.0.try_wait(), Ok(None));
            if alive && r.quality == q {
                r.used = Instant::now();
                return Ok((r.port, r.live.clone()));
            }
            *guard = None;
        }
        let r = start(q, cancel)?;
        let out = (r.port, r.live.clone());
        *guard = Some(r);
        return Ok(out);
    }
}

/// Starts loading the models in the background (when the window opens), so
/// the first press is quick. Does nothing if it is already running.
pub fn warm_up(q: Quality) {
    if is_running() || server_file().is_none() || !crate::imagegen::installed(q) {
        return;
    }
    std::thread::spawn(move || {
        let never = AtomicBool::new(false);
        let _ = ensure(q, &never);
    });
}

// ---- making pictures ---------------------------------------------------------

/// The request body for one batch of pictures.
fn request_body(req: &Request) -> Result<Value, String> {
    let mut refs = Vec::new();
    for r in &req.references {
        let bytes = std::fs::read(r).map_err(|e| format!("Could not read {}: {e}", r.display()))?;
        refs.push(base64_encode(&bytes));
    }
    let (draw, refine) = crate::imagegen::draw_size(req.width, req.height);
    let mut body = json!({
        "prompt": req.prompt,
        "width": draw.0,
        "height": draw.1,
        "seed": req.seed,
        "batch_count": req.count.max(1),
        "ref_images": refs,
        "output_format": "png",
        "sample_params": {
            "sample_steps": 4,
            "guidance": { "txt_cfg": 1.0 },
        },
    });
    if refine {
        // Drawn at half size, then refined at the full size in tiles.
        body["hires"] = json!({
            "enabled": true,
            "upscaler": "Lanczos",
            "scale": 2.0,
            "target_width": req.width,
            "target_height": req.height,
            "steps": 4,
            "denoising_strength": 0.45,
        });
        body["vae_tiling_params"] = json!({ "enabled": true });
    }
    Ok(body)
}

/// Makes `req.count` pictures with the background program and writes them to
/// `out_dir` as `picture_0.png` ... (`picture.png` for one). They are drawn one
/// after the other: asking for four at once takes far longer than four single
/// pictures (measured: 98 s against 20 s).
pub fn generate(
    req: &Request,
    out_dir: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f64),
    on_picture: &mut dyn FnMut(usize, &Path),
) -> Result<Vec<PathBuf>, Error> {
    let (port, live) = ensure(req.quality, cancel)?;
    let passes = if crate::imagegen::draw_size(req.width, req.height).1 {
        2
    } else {
        1
    };
    std::fs::create_dir_all(out_dir).map_err(|e| Error::Failed(e.to_string()))?;
    let base = format!("http://127.0.0.1:{port}/sdcpp/v1");
    let count = req.count.max(1);
    let mut files = Vec::new();
    for i in 0..count {
        let mut one = req.clone();
        one.count = 1;
        one.seed = req.seed.wrapping_add(u64::from(i));
        let body = request_body(&one).map_err(Error::Failed)?;
        let body_file = out_dir.join("request.json");
        std::fs::File::create(&body_file)
            .and_then(|mut f| f.write_all(body.to_string().as_bytes()))
            .map_err(|e| Error::Failed(e.to_string()))?;
        *live.lock().unwrap_or_else(|e| e.into_inner()) = Live::default();
        let sent = Instant::now();
        let job = call("POST", &format!("{base}/img_gen"), Some(&body_file), 60)
            .map_err(Error::Unavailable)?;
        let _ = std::fs::remove_file(&body_file);
        let id = job
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                Error::Failed(format!(
                    "The image program did not accept the request: {job}"
                ))
            })?
            .to_string();
        let expected = SECS_PER_PICTURE.load(Ordering::Relaxed) as f64 / 1000.0 + 0.5;
        let bytes = loop {
            if cancel.load(Ordering::Relaxed) {
                let _ = call("POST", &format!("{base}/jobs/{id}/cancel"), None, 10);
                if passes == 2 {
                    // A refined (2K or 4K) picture runs for minutes and the program does not
                    // stop a picture in the middle: end it, and it starts again next time.
                    shutdown();
                }
                return Err(Error::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(150));
            let status =
                call("GET", &format!("{base}/jobs/{id}"), None, 30).map_err(Error::Unavailable)?;
            match status.get("status").and_then(Value::as_str) {
                Some("completed") => {
                    let b64 = status
                        .pointer("/result/images/0/b64_json")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            Error::Failed("The image program gave no picture.".into())
                        })?;
                    break base64_decode(b64)
                        .ok_or_else(|| Error::Failed("A picture could not be read.".into()))?;
                }
                Some("failed") => {
                    let msg = status
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown error");
                    return Err(Error::Failed(crate::imagegen::explain_failure(&[
                        msg.to_string()
                    ])));
                }
                Some("cancelled") => return Err(Error::Cancelled),
                _ => {
                    // The program's own step messages when they come, otherwise
                    // a guess from the time the last picture took: never stuck at 0.
                    let real = live
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .fraction(passes);
                    let guess = (sent.elapsed().as_secs_f64() / expected).min(0.9);
                    let f = if real > 0.0 { real } else { guess };
                    progress((f64::from(i) + f) / f64::from(count));
                }
            }
        };
        // Pictures after the first take the time of a picture, nothing more.
        if i > 0 {
            SECS_PER_PICTURE.store(
                (sent.elapsed().as_secs_f64() * 1000.0) as u64,
                Ordering::Relaxed,
            );
        }
        let path = if count == 1 {
            out_dir.join("picture.png")
        } else {
            out_dir.join(format!("picture_{i}.png"))
        };
        std::fs::write(&path, bytes).map_err(|e| Error::Failed(e.to_string()))?;
        on_picture(files.len(), &path);
        files.push(path);
        if let Some(r) = SERVICE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            r.used = Instant::now();
        }
        progress(f64::from(i + 1) / f64::from(count));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        for len in 0..40usize {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            let text = base64_encode(&data);
            assert_eq!(text.len() % 4, 0);
            assert_eq!(base64_decode(&text).unwrap(), data, "{len}");
        }
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_decode("TWFu\n").unwrap(), b"Man");
        assert!(base64_decode("***").is_none());
    }

    #[test]
    fn progress_follows_the_programs_own_messages() {
        let mut l = Live::default();
        assert_eq!(l.fraction(1), 0.0);
        l.feed("[I] get_learned_condition completed, taking 0.69s --- image.cpp:529");
        assert!((l.fraction(1) - 0.1).abs() < 1e-9);
        l.feed("  |=========================>                        | 2/4 - 5.15it/s");
        assert!((l.fraction(1) - 0.5).abs() < 1e-9, "{}", l.fraction(1));
        l.feed("[I] sampling completed, taking 6.85s --- image.cpp:899");
        assert!((l.fraction(1) - 0.9).abs() < 1e-9);
        // A refined picture is two passes: the first one is half of the drawing.
        assert!((l.fraction(2) - 0.5).abs() < 1e-9);
        l.feed("[I] decoding 1 latents --- image.cpp:554");
        assert!((l.fraction(2) - 0.93).abs() < 1e-9);
    }

    #[test]
    fn the_request_carries_the_idea_the_size_and_the_pictures() {
        let dir = std::env::temp_dir().join(format!("tidycuts-svc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pic = dir.join("look.png");
        std::fs::write(&pic, b"abc").unwrap();
        let req = Request {
            prompt: "a cat".into(),
            width: 1280,
            height: 725,
            count: 4,
            seed: 7,
            references: vec![pic],
            quality: Quality::Standard,
        };
        let v = request_body(&req).unwrap();
        assert_eq!(v["prompt"], "a cat");
        assert_eq!(v["height"], 720);
        assert!(v.get("hires").is_none());
        let big = request_body(&Request {
            width: 3840,
            height: 2160,
            ..req.clone()
        })
        .unwrap();
        assert_eq!(big["width"], 1920);
        assert_eq!(big["hires"]["target_width"], 3840);
        assert_eq!(big["vae_tiling_params"]["enabled"], true);
        assert_eq!(v["batch_count"], 4);
        assert_eq!(v["ref_images"][0], "YWJj");
        assert_eq!(v["sample_params"]["sample_steps"], 4);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn without_the_program_it_says_so_instead_of_hanging() {
        // No program installed in a test run: the caller then uses the one-shot way.
        let never = AtomicBool::new(false);
        let req = Request {
            prompt: "x".into(),
            width: 256,
            height: 256,
            count: 1,
            seed: 1,
            references: vec![],
            quality: Quality::High,
        };
        if server_file().is_none() {
            let r = generate(
                &req,
                &std::env::temp_dir().join("tidycuts-svc-none"),
                &never,
                |_| {},
                &mut |_, _| {},
            );
            assert!(matches!(r, Err(Error::Unavailable(_))));
        }
        stop_if_idle(Duration::ZERO);
    }
}
