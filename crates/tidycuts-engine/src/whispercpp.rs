//! Automatic captions with Whisper, run by whisper.cpp.
//!
//! Whisper (OpenAI, MIT licence; the large-v3-turbo model, 99 languages) is
//! the most accurate speech recogniser that can be used freely and offline.
//! It is run by the small `whisper-cli` program from the whisper.cpp project
//! (MIT licence) that comes with TidyCuts, the same way ffmpeg is.
//!
//! Whisper alone has two weaknesses that this module covers:
//! * its timing wanders across silence (one word can last for many seconds),
//!   and it likes to invent text out of noise. So it is given only the
//!   stretches that contain a voice, with the silence cut out, and the times
//!   it answers with are mapped back to the clip;
//! * very quiet stretches next to loud ones are easily lost, so each stretch
//!   is lifted a little (never into clipping) before Whisper hears it.
//!
//! Its sentences are then spread over the time the voice is actually there
//! and handed to the caption layout.

use crate::captions::{is_made_up, read_sound, spoken_text, voice_spans, Word, SAMPLE_RATE};
use crate::decode::KillOnDrop;
use crate::studio_voice::search_dirs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};

/// File names of the model and the program.
const MODEL_FILE: &str = "ggml-large-v3-turbo-q5_0.bin";
#[cfg(windows)]
const PROGRAM: &str = "whisper-cli.exe";
#[cfg(not(windows))]
const PROGRAM: &str = "whisper-cli";

/// Voice stretches closer than this many seconds stay together as they were.
const KEEP_GAP: f64 = 1.0;
/// The silence put between stretches that were cut apart.
const JOINT_SECS: f64 = 0.4;
/// Whisper segments that it is less sure of than this are not used.
const MIN_CONFIDENCE: f64 = 0.35;

/// The languages offered: (code Whisper takes, name shown).
pub const LANGUAGES: &[(&str, &str)] = &[
    ("auto", "Detect the language"),
    ("en", "English"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("ru", "Russian"),
    ("uk", "Ukrainian"),
    ("tr", "Turkish"),
    ("ar", "Arabic"),
    ("hi", "Hindi"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("zh", "Chinese"),
    ("bg", "Bulgarian"),
];

/// The folder holding the whisper.cpp program, if it is installed
/// (`TIDYCUTS_WHISPER_DIR` overrides).
pub fn program_file() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("TIDYCUTS_WHISPER_DIR") {
        let p = PathBuf::from(d).join(PROGRAM);
        return p.is_file().then_some(p);
    }
    search_dirs()
        .into_iter()
        .flat_map(|d| [d.join("whisper-cpp").join(PROGRAM), d.join(PROGRAM)])
        .find(|p| p.is_file())
}

/// The user's own settings folder for TidyCuts, where downloaded models are
/// kept so they survive updates and need no permission.
pub(crate) fn settings_dir() -> Option<PathBuf> {
    let home = |var: &str| std::env::var_os(var).map(PathBuf::from);
    Some(if cfg!(windows) {
        home("APPDATA")?.join("TidyCuts")
    } else if cfg!(target_os = "macos") {
        home("HOME")?.join("Library/Application Support/TidyCuts")
    } else {
        home("XDG_CONFIG_HOME")
            .or_else(|| home("HOME").map(|h| h.join(".config")))?
            .join("tidycuts")
    })
}

/// Where a model downloaded from inside TidyCuts is kept.
pub fn downloaded_model_path() -> Option<PathBuf> {
    Some(
        settings_dir()?
            .join("models")
            .join("asr-whisper-cpp")
            .join(MODEL_FILE),
    )
}

/// The model file, if it is there: next to the program, or downloaded
/// (`TIDYCUTS_WHISPER_MODEL` overrides).
pub fn model_file() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("TIDYCUTS_WHISPER_MODEL") {
        let p = PathBuf::from(p);
        return p.is_file().then_some(p);
    }
    search_dirs()
        .into_iter()
        .map(|d| d.join("models").join("asr-whisper-cpp").join(MODEL_FILE))
        .chain(downloaded_model_path())
        .find(|p| p.is_file())
}

/// Whether captions can be made now: the program and the model are both here.
pub fn available() -> bool {
    program_file().is_some() && model_file().is_some()
}

/// Whether the program is here, so that only the model is missing.
pub fn needs_model() -> bool {
    program_file().is_some() && model_file().is_none()
}

/// Where the model can be downloaded from, first choice first.
const MODEL_URLS: [&str; 2] = [
    "https://github.com/tidysweepsoftware-lgtm/tidycuts/releases/download/caption-model-1/ggml-large-v3-turbo-q5_0.bin",
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
];
/// The model's exact size and checksum, so a broken download is noticed.
pub const MODEL_BYTES: u64 = 574_041_195;
const MODEL_SHA256: &str = "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2";

/// The SHA-256 checksum of a file, as lowercase hex.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut block = |b: &[u8]| {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    };
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut pending: Vec<u8> = Vec::new();
    let mut total: u64 = 0;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        pending.extend_from_slice(&buf[..n]);
        let whole = pending.len() / 64 * 64;
        for b in pending[..whole].as_chunks::<64>().0 {
            block(b);
        }
        pending.drain(..whole);
    }
    pending.push(0x80);
    while pending.len() % 64 != 56 {
        pending.push(0);
    }
    pending.extend_from_slice(&(total * 8).to_be_bytes());
    for b in pending.as_chunks::<64>().0 {
        block(b);
    }
    Ok(h.iter().map(|x| format!("{x:08x}")).collect())
}

/// Downloads the model into the user's settings folder, checking its size
/// and checksum. `progress` gets 0..1. The download only happens when this is
/// called (the person pressed the button).
pub fn download_model(cancel: &AtomicBool, mut progress: impl FnMut(f64)) -> Result<(), String> {
    let dest = downloaded_model_path().ok_or("There is no place to keep the model.")?;
    let folder = dest
        .parent()
        .ok_or("There is no place to keep the model.")?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let part = dest.with_extension("part");
    let mut last_error = String::from("The download did not start.");
    for (n, url) in MODEL_URLS.into_iter().enumerate() {
        // The first address carries on from what an earlier try got (a download that was
        // interrupted); the second starts afresh.
        if n > 0 {
            let _ = std::fs::remove_file(&part);
        }
        let have = std::fs::metadata(&part).map_or(0, |m| m.len());
        if have != MODEL_BYTES {
            let res = tidycuts_media::download_resume(url, &part, cancel, |got| {
                progress((got as f64 / MODEL_BYTES as f64).clamp(0.0, 0.99));
            });
            match res {
                Ok(()) => {}
                Err(e) if e == "Cancelled." => {
                    let _ = std::fs::remove_file(&part);
                    return Err(e);
                }
                Err(msg) => {
                    last_error = if msg.contains("resolve host")
                        || msg.contains("Could not connect")
                    {
                        "There is no internet connection to download the speech model. Connect and press Download again: it carries on from where it stopped.".to_string()
                    } else if msg.starts_with("Could not write") {
                        msg
                    } else {
                        format!(
                            "Could not download the speech model ({}). Press Download again to carry on.",
                            msg.trim()
                        )
                    };
                    continue;
                }
            }
        }
        let size = std::fs::metadata(&part).map_or(0, |m| m.len());
        if size != MODEL_BYTES {
            last_error =
                "The downloaded speech model is incomplete. Press Download again to carry on."
                    .into();
            continue;
        }
        if sha256_file(&part).map_err(|e| e.to_string())? != MODEL_SHA256 {
            let _ = std::fs::remove_file(&part);
            last_error = "The downloaded speech model is damaged (its checksum is wrong).".into();
            continue;
        }
        std::fs::rename(&part, &dest).map_err(|e| e.to_string())?;
        progress(1.0);
        return Ok(());
    }
    let _ = std::fs::remove_file(&part);
    Err(last_error)
}

/// A stretch of the clip that is heard, and where it sits in the shortened
/// sound Whisper is given (all in seconds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stretch {
    /// Where it starts in the clip.
    pub clip_from: f64,
    pub len: f64,
    /// Where it starts in the shortened sound.
    pub short_from: f64,
    /// How much voiced time comes before it (the lengths of the stretches
    /// before added up, without the silence between them).
    pub voiced_from: f64,
}

/// A short sound with only the voiced stretches of `samples` (each lifted a
/// little if it is quiet), and where each of them came from.
pub fn only_voice(samples: &[f32]) -> (Vec<f32>, Vec<Stretch>) {
    let rate = SAMPLE_RATE as f64;
    let spans = voice_spans(samples, 3600.0);
    // Stretches with a short pause between them stay as they were.
    let mut groups: Vec<(usize, usize)> = Vec::new();
    for p in spans {
        match groups.last_mut() {
            Some(g) if ((p.from.saturating_sub(g.1)) as f64 / rate) < KEEP_GAP => {
                g.1 = g.1.max(p.to)
            }
            _ => groups.push((p.from, p.to)),
        }
    }
    let joint = vec![0f32; (JOINT_SECS * rate) as usize];
    let mut out: Vec<f32> = Vec::new();
    let mut table = Vec::new();
    for (a, b) in groups {
        let seg = &samples[a..b];
        let rms = (seg.iter().map(|v| v * v).sum::<f32>() / seg.len().max(1) as f32)
            .sqrt()
            .max(1e-9);
        let peak = seg.iter().fold(1e-9f32, |m, v| m.max(v.abs()));
        // A gentle lift for a quiet stretch, never so far that it clips.
        let gain = (0.05 / rms).min(0.7 / peak).clamp(1.0, 30.0);
        table.push(Stretch {
            clip_from: a as f64 / rate,
            len: seg.len() as f64 / rate,
            short_from: out.len() as f64 / rate,
            voiced_from: table.iter().map(|s: &Stretch| s.len).sum(),
        });
        out.extend(seg.iter().map(|v| v * gain));
        out.extend_from_slice(&joint);
    }
    (out, table)
}

/// A time in the shortened sound as a time in the clip.
pub fn to_clip_time(t: f64, table: &[Stretch]) -> f64 {
    match table.iter().rev().find(|s| t >= s.short_from) {
        Some(s) => s.clip_from + (t - s.short_from).min(s.len),
        None => table.first().map_or(t, |s| s.clip_from),
    }
}

/// How much voiced time has passed at `t` in the shortened sound.
fn voiced_time(t: f64, table: &[Stretch]) -> f64 {
    match table.iter().rev().find(|s| t >= s.short_from) {
        Some(s) => s.voiced_from + (t - s.short_from).clamp(0.0, s.len),
        None => 0.0,
    }
}

/// The time in the clip of a moment `v` of voiced time, and which stretch it
/// is in.
fn clip_time_of_voiced(v: f64, table: &[Stretch]) -> (f64, usize) {
    let i = table
        .iter()
        .rposition(|s| v >= s.voiced_from)
        .unwrap_or_default();
    match table.get(i) {
        Some(s) => (s.clip_from + (v - s.voiced_from).clamp(0.0, s.len), i),
        None => (0.0, 0),
    }
}

/// Writes 16 kHz mono 16-bit sound as a WAV file.
fn write_wav(path: &Path, samples: &[f32]) -> std::io::Result<()> {
    let data_len = (samples.len() * 2) as u32;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&(SAMPLE_RATE as u32).to_le_bytes())?;
    f.write_all(&(SAMPLE_RATE as u32 * 2).to_le_bytes())?; // bytes per second
    f.write_all(&2u16.to_le_bytes())?; // bytes per frame
    f.write_all(&16u16.to_le_bytes())?; // bits
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for s in samples {
        f.write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
    }
    f.flush()
}

/// A word and when it begins in the shortened sound.
#[derive(Debug, Clone, PartialEq)]
pub struct TimedWord {
    pub text: String,
    pub start: f64,
}

/// One sentence Whisper wrote: its text, when it starts and ends in the
/// shortened sound, how sure the network was, and (when it could tell) when
/// each word begins.
#[derive(Debug, Clone, PartialEq)]
pub struct Sentence {
    pub text: String,
    pub from: f64,
    pub to: f64,
    pub confidence: f64,
    pub words: Option<Vec<TimedWord>>,
}

/// The words of a sentence with the time each begins, from the alignment
/// times (`t_dtw`, hundredths of a second) whisper.cpp gives every piece of a
/// word. `None` when they are missing.
fn timed_words(sentence: &serde_json::Value) -> Option<Vec<TimedWord>> {
    let tokens = sentence.get("tokens")?.as_array()?;
    let mut words: Vec<(String, f64)> = Vec::new();
    for t in tokens {
        let text = t.get("text").and_then(|x| x.as_str()).unwrap_or("");
        if text.starts_with("[_") {
            continue;
        }
        let at = t.get("t_dtw").and_then(|x| x.as_f64()).unwrap_or(-1.0);
        match words.last_mut() {
            // A piece with no space before it continues the word.
            Some(last) if !text.starts_with(' ') => {
                last.0.push_str(text);
                if last.1 < 0.0 {
                    last.1 = at / 100.0;
                }
            }
            _ => words.push((
                text.trim().to_string(),
                if at < 0.0 { -1.0 } else { at / 100.0 },
            )),
        }
    }
    words.retain(|w| !w.0.is_empty());
    if words.is_empty() || words.iter().any(|w| w.1 < 0.0) {
        return None;
    }
    // Times only go forwards.
    let mut prev = 0.0f64;
    Some(
        words
            .into_iter()
            .map(|(text, start)| {
                prev = prev.max(start);
                TimedWord { text, start: prev }
            })
            .collect(),
    )
}

/// Leaves out what Whisper describes instead of what is said: "(sighs)",
/// "[music]", "*laughs*", also over several words.
fn spoken_words(words: &[TimedWord]) -> Vec<TimedWord> {
    let mut depth = 0usize;
    let mut starred = false;
    let mut out = Vec::new();
    for w in words {
        let mut kept = String::new();
        for c in w.text.chars() {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = depth.saturating_sub(1),
                '*' => starred = !starred,
                '♪' | '♫' => {}
                _ if depth == 0 && !starred => kept.push(c),
                _ => {}
            }
        }
        if kept.chars().any(char::is_alphanumeric) {
            out.push(TimedWord {
                text: kept,
                start: w.start,
            });
        }
    }
    out
}

/// Reads whisper-cli's JSON output.
pub fn read_sentences(json: &str) -> Result<Vec<Sentence>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let items = v
        .get("transcription")
        .and_then(|t| t.as_array())
        .ok_or("The speech program gave an answer that could not be read.")?;
    let mut out = Vec::new();
    for it in items {
        let text = it.get("text").and_then(|t| t.as_str()).unwrap_or("");
        let off = it.get("offsets");
        let ms = |k: &str| {
            off.and_then(|o| o.get(k))
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0)
                / 1000.0
        };
        let probs: Vec<f64> = it
            .get("tokens")
            .and_then(|t| t.as_array())
            .map(|ts| {
                ts.iter()
                    .filter(|t| {
                        !t.get("text")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .starts_with("[_")
                    })
                    .filter_map(|t| t.get("p").and_then(|p| p.as_f64()))
                    .collect()
            })
            .unwrap_or_default();
        let confidence = if probs.is_empty() {
            0.0
        } else {
            probs.iter().sum::<f64>() / probs.len() as f64
        };
        out.push(Sentence {
            text: text.to_string(),
            from: ms("from"),
            to: ms("to"),
            confidence,
            words: timed_words(it),
        });
    }
    Ok(out)
}

/// How long a word takes to say, by its length, when only its start is known.
fn word_length(text: &str) -> f64 {
    (0.08 + 0.06 * text.chars().count() as f64).clamp(0.15, 0.9)
}

/// Gives words with known start times their place in the clip. A word ends
/// where the next begins, or after the time it takes to say it, and never
/// runs on past the stretch of voice it started in.
fn place_words(words: &[TimedWord], sentence_end: f64, table: &[Stretch]) -> Vec<Word> {
    words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let own = word_length(&w.text);
            let end = match words.get(i + 1) {
                Some(next) => next.start.min(w.start + own.max(0.3)).max(w.start + 0.05),
                None => (w.start + own).min(sentence_end.max(w.start + 0.15)),
            };
            let at = table
                .iter()
                .rposition(|s| w.start >= s.short_from)
                .unwrap_or(0);
            let start = to_clip_time(w.start, table);
            let mut stop = to_clip_time(end, table);
            // Not into the next stretch, nor past the end of its own.
            if let Some(t) = table.get(at) {
                stop = stop.min(t.clip_from + t.len);
            }
            Word {
                text: w.text.clone(),
                start,
                end: stop.max(start + 0.05),
            }
        })
        .collect()
}

/// Turns Whisper's sentences into timed words in the clip: noise it made up
/// is dropped, and the words of each sentence are spread over its time.
pub fn words_from_sentences(sentences: &[Sentence], table: &[Stretch]) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    let mut last_text = String::new();
    let mut repeats = 0;
    for s in sentences {
        if s.confidence < MIN_CONFIDENCE {
            continue;
        }
        let text = spoken_text(&s.text);
        if text.is_empty() || is_made_up(&text) {
            continue;
        }
        // The same sentence again and again is the network stuck.
        if text == last_text {
            repeats += 1;
            if repeats >= 2 {
                continue;
            }
        } else {
            repeats = 0;
            last_text = text.clone();
        }
        if let Some(timed) = &s.words {
            let timed = spoken_words(timed);
            if !timed.is_empty() {
                words.extend(place_words(&timed, s.to, table));
                continue;
            }
        }
        let list: Vec<&str> = text.split_whitespace().collect();
        let total: f64 = list.iter().map(|w| w.chars().count() as f64 + 1.0).sum();
        if list.is_empty() || s.to <= s.from || total <= 0.0 {
            continue;
        }
        // The words share the time the voice is there, not the silence cut
        // between the stretches.
        let (v0, v1) = (voiced_time(s.from, table), voiced_time(s.to, table));
        if v1 <= v0 {
            continue;
        }
        let mut v = v0;
        for w in list {
            let d = (v1 - v0) * (w.chars().count() as f64 + 1.0) / total;
            let (a, i) = clip_time_of_voiced(v, table);
            let (mut b, j) = clip_time_of_voiced((v + d - 1e-6).max(v), table);
            if j != i {
                // A word that runs into the next stretch ends where its own does.
                b = table[i].clip_from + table[i].len;
            }
            words.push(Word {
                text: w.to_string(),
                start: a,
                end: b.max(a + 0.05),
            });
            v += d;
        }
    }
    words
}

/// Listens to `range` of a media file with Whisper and returns the words
/// with their times (in the media file's own time).
pub fn transcribe(
    path: &Path,
    stream: u32,
    range: (f64, f64),
    language: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f64),
) -> Result<Vec<Word>, String> {
    let program = program_file().ok_or("The speech program is not installed.")?;
    let model = model_file().ok_or("The speech model is not installed.")?;
    progress(0.0);
    let samples = read_sound(path, stream, range)?;
    if samples.is_empty() {
        return Err("This clip has no sound to listen to.".into());
    }
    let (short, table) = only_voice(&samples);
    if table.is_empty() {
        return Ok(Vec::new());
    }
    progress(0.03);
    let tmp = std::env::temp_dir().join(format!("tidycuts-captions-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let wav = tmp.join("voice.wav");
    write_wav(&wav, &short).map_err(|e| e.to_string())?;
    let base = tmp.join("out");
    let threads = std::thread::available_parallelism()
        .map(|n| (n.get() / 2).clamp(2, 16))
        .unwrap_or(4);
    let mut cmd = std::process::Command::new(&program);
    cmd.arg("-m")
        .arg(&model)
        .arg("-f")
        .arg(&wav)
        .args(["-l", language])
        .args(["-t", &threads.to_string()])
        // Each sentence on its own: no memory of the one before, which is what
        // makes Whisper repeat itself.
        .args(["-mc", "0"])
        // Word times (they need Whisper's flash attention switched off).
        .args(["--dtw", "large.v3.turbo", "-nfa"])
        .arg("-pp")
        .arg("-np")
        .arg("-ojf")
        .arg("-of")
        .arg(&base)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = KillOnDrop(
        cmd.spawn()
            .map_err(|e| format!("Could not start the speech program: {e}"))?,
    );
    let stderr = child.0.stderr.take().ok_or("no output")?;
    let mut failure = String::new();
    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
        if cancel.load(Ordering::Relaxed) {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err("Cancelled.".into());
        }
        if let Some(pct) = line.split("progress =").nth(1) {
            if let Ok(p) = pct.trim().trim_end_matches('%').trim().parse::<f64>() {
                progress(0.05 + 0.95 * (p / 100.0).clamp(0.0, 1.0));
            }
        } else if line.to_lowercase().contains("error") {
            failure = line;
        }
    }
    let status = child.0.wait().map_err(|e| e.to_string())?;
    let json = std::fs::read_to_string(base.with_extension("json"));
    let _ = std::fs::remove_dir_all(&tmp);
    let json = match (status.success(), json) {
        (true, Ok(j)) => j,
        _ => {
            return Err(if failure.is_empty() {
                "The speech program stopped before it was done.".into()
            } else {
                format!("The speech program failed: {failure}")
            })
        }
    };
    let sentences = read_sentences(&json)?;
    let mut words = words_from_sentences(&sentences, &table);
    for w in &mut words {
        w.start += range.0;
        w.end += range.0;
    }
    progress(1.0);
    Ok(words)
}

#[cfg(test)]
mod tests {
    #[test]
    fn sha256_matches_the_known_answers() {
        let dir = std::env::temp_dir().join("tidycuts-sha-test");
        std::fs::create_dir_all(&dir).unwrap();
        let check = |bytes: &[u8], want: &str| {
            let f = dir.join("x.bin");
            std::fs::write(&f, bytes).unwrap();
            assert_eq!(sha256_file(&f).unwrap(), want);
        };
        check(
            b"",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
        check(
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        );
        // Longer than one block, and a multiple of the block size.
        check(
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        );
        check(
            &[b'a'; 64],
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_downloaded_model_has_a_place_in_the_users_settings_folder() {
        let p = downloaded_model_path().unwrap();
        assert!(p.ends_with("models/asr-whisper-cpp/ggml-large-v3-turbo-q5_0.bin"));
        assert!(p.to_string_lossy().to_lowercase().contains("tidycuts"));
    }

    use super::*;

    fn timed(text: &str, start: f64) -> TimedWord {
        TimedWord {
            text: text.into(),
            start,
        }
    }

    #[test]
    fn word_times_come_from_the_alignment_times_of_the_pieces() {
        let json = r#"{"transcription":[{"offsets":{"from":0,"to":3000},"text":" Massage them.","tokens":[
            {"text":"[_BEG_]","p":0.9,"t_dtw":-1},
            {"text":" mass","p":0.5,"t_dtw":36},{"text":"age","p":0.9,"t_dtw":52},
            {"text":" them","p":0.8,"t_dtw":70},{"text":".","p":0.5,"t_dtw":90}]}]}"#;
        let s = read_sentences(json).unwrap();
        let w = s[0].words.clone().unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(
            (w[0].text.as_str(), w[1].text.as_str()),
            ("massage", "them.")
        );
        assert!((w[0].start - 0.36).abs() < 1e-9 && (w[1].start - 0.70).abs() < 1e-9);
        // Missing alignment times mean no word times.
        let none = json
            .replace("\"t_dtw\":70", "\"t_dtw\":-1")
            .replace("\"t_dtw\":90", "\"t_dtw\":-1");
        assert!(read_sentences(&none).unwrap()[0].words.is_none());
    }

    #[test]
    fn words_with_start_times_get_clip_times_and_never_run_into_the_next_stretch() {
        let t = table();
        // "slow" starts 0.2 s before the first stretch ends and is long;
        // "next" is in the second stretch.
        let mut sen = sentence(" one slow next", 0.0, 6.0, 0.9);
        sen.words = Some(vec![
            timed("one", 0.5),
            timed("slow", 3.8),
            timed("next", 4.6),
        ]);
        let w = words_from_sentences(&[sen], &t);
        assert_eq!(w.len(), 3);
        assert!((w[0].start - 10.5).abs() < 1e-9);
        assert!((w[1].start - 13.8).abs() < 1e-9);
        // "slow" ends with its stretch (14.0), not in the 30 s that follow.
        assert!(w[1].end <= 14.0 + 1e-9, "{:?}", w[1]);
        assert!((w[2].start - 30.2).abs() < 1e-9);
        assert!(w.windows(2).all(|p| p[0].start <= p[1].start));
    }

    #[test]
    fn described_sounds_are_dropped_from_timed_words_even_over_several_words() {
        let words = vec![
            timed("(heavy", 0.1),
            timed("breathing)", 0.5),
            timed("Okay.", 1.0),
            timed("*laughs*", 1.5),
            timed("yes", 2.0),
        ];
        let kept: Vec<String> = spoken_words(&words).into_iter().map(|w| w.text).collect();
        assert_eq!(kept, ["Okay.", "yes"]);
    }

    fn sentence(text: &str, from: f64, to: f64, confidence: f64) -> Sentence {
        Sentence {
            text: text.into(),
            from,
            to,
            confidence,
            words: None,
        }
    }

    fn table() -> Vec<Stretch> {
        // Two stretches: clip 10..14 (4 s) then clip 30..32 (2 s).
        vec![
            Stretch {
                clip_from: 10.0,
                len: 4.0,
                short_from: 0.0,
                voiced_from: 0.0,
            },
            Stretch {
                clip_from: 30.0,
                len: 2.0,
                short_from: 4.4,
                voiced_from: 4.0,
            },
        ]
    }

    #[test]
    fn times_in_the_shortened_sound_go_back_to_the_clip() {
        let t = table();
        assert!((to_clip_time(0.0, &t) - 10.0).abs() < 1e-9);
        assert!((to_clip_time(3.0, &t) - 13.0).abs() < 1e-9);
        // The silence put between stretches lands at the end of the one before.
        assert!((to_clip_time(4.2, &t) - 14.0).abs() < 1e-9);
        assert!((to_clip_time(4.4, &t) - 30.0).abs() < 1e-9);
        assert!((to_clip_time(5.9, &t) - 31.5).abs() < 1e-9);
        assert!((to_clip_time(99.0, &t) - 32.0).abs() < 1e-9);
    }

    #[test]
    fn a_sentence_that_spans_two_stretches_does_not_stretch_over_the_silence_between() {
        let t = table();
        // One sentence from 2 s into the first stretch to 1 s into the second.
        let s = vec![sentence(" one two three four five six", 2.0, 5.4, 0.9)];
        let w = words_from_sentences(&s, &t);
        assert_eq!(w.len(), 6);
        // No word lasts longer than its words' share of the 3 s of voice.
        assert!(w.iter().all(|w| w.end - w.start < 1.2), "{w:?}");
        // The words in the first stretch are there (12..14), the rest in the second.
        assert!(w[0].start >= 12.0 - 1e-6 && w[0].end <= 14.0 + 1e-6);
        assert!(w[5].start >= 30.0 - 1e-6);
        assert!(w.windows(2).all(|p| p[0].start <= p[1].start + 1e-9));
    }

    #[test]
    fn only_the_voiced_stretches_are_kept_and_quiet_ones_are_lifted() {
        // 12 s of room tone with a quiet 1.5 s tone burst at 2 s and a louder
        // one at 8 s.
        let n = SAMPLE_RATE * 12;
        let mut seed = 7u32;
        let mut noise = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            ((seed >> 8) as f32 / 8388608.0) - 1.0
        };
        let mut x: Vec<f32> = (0..n).map(|_| noise() * 0.0005).collect();
        for (at, amp) in [(2.0f64, 0.01f32), (8.0, 0.2)] {
            let (a, b) = (
                (at * SAMPLE_RATE as f64) as usize,
                ((at + 1.5) * SAMPLE_RATE as f64) as usize,
            );
            for (i, v) in x.iter_mut().enumerate().take(b).skip(a) {
                *v += amp * (i as f32 * 0.2).sin();
            }
        }
        let (short, table) = only_voice(&x);
        assert_eq!(table.len(), 2, "{table:?}");
        // Far shorter than the clip, but holding both stretches.
        assert!(short.len() < SAMPLE_RATE * 6, "{}", short.len());
        assert!((table[0].clip_from - 2.0).abs() < 0.4 && (table[1].clip_from - 8.0).abs() < 0.4);
        // The quiet stretch was lifted (its peak is well above 0.01) and
        // nothing clips.
        let from = (table[0].short_from * SAMPLE_RATE as f64) as usize;
        let peak = short[from..from + SAMPLE_RATE]
            .iter()
            .fold(0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.03 && peak <= 0.7 + 1e-3, "{peak}");
        assert!(short.iter().all(|v| v.abs() <= 1.0));
    }

    #[test]
    fn whispers_json_is_read() {
        let json = r#"{"transcription":[
            {"offsets":{"from":1000,"to":3500},"text":" Hello there.","tokens":[
              {"text":"[_BEG_]","p":0.9},{"text":" Hello","p":0.8},{"text":" there","p":0.6},{"text":".","p":0.4}]}]}"#;
        let s = read_sentences(json).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text.trim(), "Hello there.");
        assert!((s[0].from - 1.0).abs() < 1e-9 && (s[0].to - 3.5).abs() < 1e-9);
        assert!((s[0].confidence - 0.6).abs() < 1e-9);
        assert!(read_sentences("{}").is_err());
    }

    #[test]
    fn words_get_clip_times_and_noise_made_up_by_whisper_is_dropped() {
        let t = table();
        let sentences = vec![
            sentence(" I want to know.", 0.0, 2.0, 0.8),
            sentence(" (sighs)", 2.0, 3.0, 0.9),
            sentence(" That's the end of the video.", 3.0, 4.0, 0.9),
            sentence(" Mumble mumble.", 4.4, 5.0, 0.1),
            sentence(" Okay.", 4.4, 5.4, 0.7),
        ];
        let w = words_from_sentences(&sentences, &t);
        let text: Vec<&str> = w.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(text, ["I", "want", "to", "know.", "Okay."]);
        // The first sentence starts at the clip's 10 s, the last at 30 s.
        assert!((w[0].start - 10.0).abs() < 1e-9);
        assert!((w[4].start - 30.0).abs() < 1e-9 && w[4].end > 30.5);
        assert!(w.windows(2).all(|p| p[0].start <= p[1].start));
    }

    #[test]
    fn a_sentence_repeated_over_and_over_is_kept_only_a_few_times() {
        let t = table();
        let s: Vec<Sentence> = (0..6)
            .map(|i| sentence(" I'm sorry.", i as f64 * 0.5, i as f64 * 0.5 + 0.5, 0.9))
            .collect();
        let w = words_from_sentences(&s, &t);
        assert_eq!(w.len(), 4); // two sentences of two words
    }

    #[test]
    fn the_wav_header_is_right() {
        let dir = std::env::temp_dir().join("tidycuts-wav-test");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.wav");
        write_wav(&f, &[0.0, 0.5, -0.5, 1.0]).unwrap();
        let b = std::fs::read(&f).unwrap();
        assert_eq!(&b[..4], b"RIFF");
        assert_eq!(&b[8..16], b"WAVEfmt ");
        assert_eq!(u32::from_le_bytes(b[24..28].try_into().unwrap()), 16000);
        assert_eq!(u32::from_le_bytes(b[40..44].try_into().unwrap()), 8);
        assert_eq!(i16::from_le_bytes(b[46..48].try_into().unwrap()), 16383);
        assert_eq!(b.len(), 44 + 8);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
