//! Downloads a file with `curl`, writing it from inside TidyCuts.
//!
//! curl only fetches the bytes; TidyCuts itself writes the file. That way the file is
//! made by the same program that later reads it (which matters in the Microsoft Store
//! package, where a separate program can see a different "view" of the settings folder),
//! and a full disk or a blocked folder is reported as what it is.

use std::io::{Read, Write};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How many times a broken connection is picked up again before giving up.
const TRIES: u32 = 6;

/// Downloads `url` into `part`, carrying on from what `part` already holds. `progress` gets
/// the number of bytes the file holds now. Returns `Err("Cancelled.")` when `cancel` is set.
/// The result is not checked here: the caller compares size and checksum.
pub fn download_resume(
    url: &str,
    part: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<(), String> {
    let mut last = String::new();
    for attempt in 0..TRIES {
        if attempt > 0 {
            // Wait a little before trying again, staying ready to be cancelled.
            for _ in 0..12 {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled.".into());
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        }
        match one_try(url, part, cancel, &mut progress) {
            Ok(()) => return Ok(()),
            Err(e) if e == "Cancelled." => return Err(e),
            Err(e) => {
                // Problems with the file or the disk will not go away by trying again.
                if e.starts_with("Could not write") {
                    return Err(e);
                }
                last = e;
            }
        }
    }
    Err(last)
}

fn one_try(
    url: &str,
    part: &Path,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(u64),
) -> Result<(), String> {
    let start = std::fs::metadata(part).map_or(0, |m| m.len());
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(part)
        .map_err(|e| format!("Could not write {} ({e}).", part.display()))?;

    let mut cmd = crate::curl();
    cmd.args(["-L", "--fail", "-sS", "--connect-timeout", "30"]);
    if start > 0 {
        cmd.arg("-r").arg(format!("{start}-"));
    }
    cmd.arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    let mut child = cmd
        .spawn()
        .map_err(|_| "Could not start the program that downloads files (curl).".to_string())?;
    let mut out = child.stdout.take().ok_or("Could not read the download.")?;

    let written = Arc::new(AtomicU64::new(start));
    let failure: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let (w, f, path) = (written.clone(), failure.clone(), part.to_path_buf());
    let writer = std::thread::spawn(move || {
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            match out.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Err(e) = file.write_all(&buf[..n]) {
                        *f.lock().unwrap() =
                            Some(format!("Could not write {} ({e}).", path.display()));
                        break;
                    }
                    w.fetch_add(n as u64, Ordering::Relaxed);
                }
                Err(e) => {
                    *f.lock().unwrap() = Some(format!("The download was interrupted ({e})."));
                    break;
                }
            }
        }
        let _ = file.flush();
    });

    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = writer.join();
            return Err("Cancelled.".into());
        }
        // A write that failed ends the download at once.
        if failure.lock().unwrap().is_some() {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) => {
                progress(written.load(Ordering::Relaxed));
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    let _ = writer.join();
    progress(written.load(Ordering::Relaxed));
    if let Some(e) = failure.lock().unwrap().take() {
        return Err(e);
    }
    if status.is_some_and(|s| s.success()) {
        return Ok(());
    }
    let mut msg = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut msg);
    }
    Err(msg.trim().to_string())
}
