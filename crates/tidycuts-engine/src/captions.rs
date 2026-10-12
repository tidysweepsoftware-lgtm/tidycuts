//! Automatic captions: the words that were said, and how they become caption
//! lines.
//!
//! The listening itself is done by Whisper, run by whisper.cpp (see
//! `whispercpp`). This module holds what is shared: the timed words, the
//! cutting of sound into stretches that contain a voice, the clean-up of the
//! text, and the grouping of words into lines that are comfortable to read.

use std::io::Read;
use std::path::Path;
use std::process::Stdio;

pub const SAMPLE_RATE: usize = 16_000;
/// Sound is read in pieces of about this many seconds, cut at quiet moments.
const CHUNK_SECS: f64 = 20.0;
/// Longest sound that is transcribed.
const MAX_SECS: f64 = 4.0 * 3600.0;

/// A word and when it was said, in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

/// What Whisper wrote, without the descriptions of sounds it adds in brackets
/// ("(sighs)", "[music]") or the music notes: only what was said.
pub(crate) fn spoken_text(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    let mut starred = false;
    for c in text.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '*' => starred = !starred,
            '♪' | '♫' => {}
            _ if depth == 0 && !starred => out.push(c),
            _ => {}
        }
    }
    let t = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = t
        .trim_matches(|c: char| c == '-' || c == '–' || c == ' ')
        .to_string();
    if t.chars().any(char::is_alphanumeric) {
        t
    } else {
        String::new()
    }
}

/// Lines Whisper is known to make up out of noise (it learned them from the
/// ends of videos), which are never what was said in a clip.
pub(crate) fn is_made_up(text: &str) -> bool {
    let t: String = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect();
    [
        "thank you for watching",
        "thanks for watching",
        "thats the end of the video",
        "that is the end of the video",
        "please subscribe",
        "like and subscribe",
        "see you in the next video",
        "see you next time",
        "subtitles by",
        "amaraorg",
    ]
    .iter()
    .any(|p| t.contains(p))
}

/// Cuts sound into pieces of about `CHUNK_SECS`, each ending at the quietest
/// moment near where it would end. Returns sample ranges.
pub fn pieces(samples: &[f32]) -> Vec<(usize, usize)> {
    pieces_of(samples, CHUNK_SECS)
}

/// Like `pieces`, aiming for `secs` seconds each.
fn pieces_of(samples: &[f32], secs: f64) -> Vec<(usize, usize)> {
    let frame = SAMPLE_RATE / 50; // 20 ms
    let rms: Vec<f32> = samples
        .chunks(frame)
        .map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt())
        .collect();
    let target = (secs * 50.0) as usize;
    let reach = 5 * 50;
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < rms.len() {
        if rms.len() - start <= target + reach {
            out.push((start * frame, samples.len()));
            break;
        }
        let from = start + target - reach;
        let to = start + target + reach;
        let cut = (from..to)
            .min_by(|a, b| rms[*a].total_cmp(&rms[*b]))
            .unwrap_or(start + target);
        out.push((start * frame, cut * frame));
        start = cut;
    }
    out
}

/// Cuts sound into pieces that each hold one stretch of speech, with a
/// little quiet around it, and returns their sample ranges.
///
/// The recognizer rescales what it hears over the whole piece, so a quiet
/// line inside a long piece of room tone is averaged away and never heard.
/// Cutting tightly around each stretch of voice fixes that. "Speech" here is
/// anything clearly louder than the room tone around it (the tone is tracked
/// along the clip), so a soft voice counts just as much as a loud one.
/// Stretches longer than `max_secs` are cut at their quietest moments.
pub fn voice_pieces(samples: &[f32], max_secs: f64) -> Vec<(usize, usize)> {
    voice_spans(samples, max_secs)
        .into_iter()
        .map(|p| (p.from, p.to))
        .collect()
}

/// A piece of sound to listen to, and where the voice is inside it (sample
/// positions in the whole sound).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub from: usize,
    pub to: usize,
    pub voice_from: usize,
    pub voice_to: usize,
}

/// Like `voice_pieces`, also telling where the voice itself starts and ends
/// inside each piece (without the quiet kept around it).
pub fn voice_spans(samples: &[f32], max_secs: f64) -> Vec<Piece> {
    const ON_DB: f32 = 9.0;
    const OFF_DB: f32 = 5.0;
    const MIN_DB: f32 = -75.0;
    const GAP_SECS: f64 = 0.35;
    const MIN_SECS: f64 = 0.12;
    const PAD_SECS: f64 = 0.2;
    let frame = SAMPLE_RATE / 50; // 20 ms
    let n = samples.len() / frame;
    if n == 0 {
        return Vec::new();
    }
    let db: Vec<f32> = (0..n)
        .map(|i| {
            let f = &samples[i * frame..(i + 1) * frame];
            10.0 * (f.iter().map(|v| v * v).sum::<f32>() / frame as f32 + 1e-12).log10()
        })
        .collect();
    // The room tone: the quiet end of each second, smoothed over the seconds
    // around it.
    let block = 50;
    let tone: Vec<f32> = db
        .chunks(block)
        .map(|c| {
            let mut v = c.to_vec();
            v.sort_by(|a, b| a.total_cmp(b));
            v[(v.len() as f32 * 0.15) as usize]
        })
        .collect();
    // Never more than a little above the quietest part of the whole clip, so
    // a long unbroken stretch of speech does not become its own "room tone".
    let global = {
        let mut v = db.clone();
        v.sort_by(|a, b| a.total_cmp(b));
        v[(v.len() as f32 * 0.05) as usize]
    };
    let floor_at = |i: usize| -> f32 {
        let b = i / block;
        let lo = b.saturating_sub(2);
        let hi = (b + 3).min(tone.len());
        let mut v = tone[lo..hi].to_vec();
        v.sort_by(|a, b| a.total_cmp(b));
        v[v.len() / 2].min(global + 14.0)
    };
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    for (i, d) in db.iter().enumerate() {
        let floor = floor_at(i);
        match start {
            None if *d > floor + ON_DB && *d > MIN_DB => start = Some(i),
            Some(s) if *d < floor + OFF_DB => {
                ranges.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        ranges.push((s, n));
    }
    if ranges.is_empty() {
        // Nothing stood out. If there is sound at all, listen to all of it in
        // ordinary pieces rather than report silence.
        let mean = db.iter().sum::<f32>() / db.len() as f32;
        return if mean > -62.0 {
            pieces_of(samples, max_secs)
                .into_iter()
                .map(|(from, to)| Piece {
                    from,
                    to,
                    voice_from: from,
                    voice_to: to,
                })
                .collect()
        } else {
            Vec::new()
        };
    }
    let per_sec = 50.0;
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for r in ranges {
        match merged.last_mut() {
            Some(last) if (r.0 - last.1) as f64 / per_sec < GAP_SECS => last.1 = r.1,
            _ => merged.push(r),
        }
    }
    let pad = (PAD_SECS * SAMPLE_RATE as f64) as usize;
    let mut out: Vec<Piece> = Vec::new();
    for (s, e) in merged {
        if ((e - s) as f64 / per_sec) < MIN_SECS {
            continue;
        }
        let a = (s * frame)
            .saturating_sub(pad)
            .max(out.last().map_or(0, |l| l.to));
        let b = (e * frame + pad).min(samples.len());
        let (va, vb) = ((s * frame).max(a), (e * frame).min(b));
        // Long stretches are cut at their quietest moments.
        if (b - a) as f64 / SAMPLE_RATE as f64 > max_secs * 1.2 {
            for (x, y) in pieces_of(&samples[a..b], max_secs) {
                let (from, to) = (a + x, a + y);
                out.push(Piece {
                    from,
                    to,
                    voice_from: va.clamp(from, to),
                    voice_to: vb.clamp(from, to),
                });
            }
        } else {
            out.push(Piece {
                from: a,
                to: b,
                voice_from: va,
                voice_to: vb,
            });
        }
    }
    out
}

/// Reads 16 kHz mono sound from a media file.
pub(crate) fn read_sound(path: &Path, stream: u32, range: (f64, f64)) -> Result<Vec<f32>, String> {
    let tools = tidycuts_media::Tools::get().map_err(|e| e.to_string())?;
    let mut cmd = tools.ffmpeg_cmd();
    cmd.args(["-nostdin", "-ss", &format!("{:.6}", range.0.max(0.0))]);
    cmd.args([
        "-t",
        &format!("{:.6}", (range.1 - range.0).clamp(0.1, MAX_SECS)),
    ]);
    cmd.arg("-i").arg(path);
    cmd.args([
        "-vn",
        "-sn",
        "-dn",
        "-map",
        &format!("0:a:{stream}?"),
        "-ac",
        "1",
        "-ar",
        "16000",
        "-f",
        "f32le",
        "pipe:1",
    ]);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    let mut child = crate::decode::KillOnDrop(cmd.spawn().map_err(|e| e.to_string())?);
    let mut bytes = Vec::new();
    child
        .0
        .stdout
        .take()
        .ok_or("no pipe")?
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect())
}

/// One caption: the text for the screen and when to show it.
#[derive(Debug, Clone, PartialEq)]
pub struct Caption {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

/// How words are grouped into captions.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    /// Most characters on one line.
    pub max_chars: usize,
    /// Longest time one caption stays up.
    pub max_secs: f64,
    /// A pause this long starts a new caption.
    pub pause: f64,
}

impl Default for Layout {
    fn default() -> Layout {
        Layout {
            max_chars: 32,
            max_secs: 5.0,
            pause: 0.7,
        }
    }
}

fn ends_sentence(w: &str) -> bool {
    w.ends_with(['.', '?', '!', '…'])
}

/// Splits a long line at the space nearest its middle, so two lines are even.
fn balance(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars * 6 / 10 {
        return text.to_string();
    }
    let mid = text.chars().count() / 2;
    let split = text
        .char_indices()
        .filter(|(_, c)| *c == ' ')
        .min_by_key(|(i, _)| (text[..*i].chars().count() as isize - mid as isize).abs());
    match split {
        Some((i, _)) => format!("{}\n{}", &text[..i], &text[i + 1..]),
        None => text.to_string(),
    }
}

/// Groups words into caption lines with comfortable timing.
pub fn group_captions(words: &[Word], layout: Layout) -> Vec<Caption> {
    // Lines hold up to two rows of `max_chars`.
    let line_limit = layout.max_chars * 2 - 4;
    let mut groups: Vec<Vec<&Word>> = Vec::new();
    for w in words {
        let new_group = match groups.last() {
            None => true,
            Some(g) => {
                let chars: usize = g.iter().map(|w| w.text.chars().count() + 1).sum();
                let first = g[0];
                let last = g[g.len() - 1];
                w.start - last.end > layout.pause
                    || chars + w.text.chars().count() > line_limit
                    || w.end - first.start > layout.max_secs
                    || (ends_sentence(&last.text) && chars >= 14)
                    || (last.text.ends_with(',') && chars >= 24)
            }
        };
        if new_group {
            groups.push(vec![w]);
        } else {
            groups.last_mut().unwrap().push(w);
        }
    }
    let mut captions: Vec<Caption> = groups
        .iter()
        .map(|g| {
            let text = g
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            Caption {
                text: balance(&text, layout.max_chars),
                start: (g[0].start - 0.05).max(0.0),
                end: g[g.len() - 1].end + 0.1,
            }
        })
        .collect();
    // A sentence starts with a capital letter, whatever the network wrote.
    let mut sentence_start = true;
    for c in &mut captions {
        if sentence_start {
            let mut chars = c.text.chars();
            if let Some(first) = chars.next() {
                c.text = first.to_uppercase().collect::<String>() + chars.as_str();
            }
        }
        sentence_start = c.text.trim_end().ends_with(['.', '?', '!', '…']);
    }
    // A caption stays long enough to read and never runs into the next.
    for i in 0..captions.len() {
        let min_end = captions[i].start + 0.6;
        if captions[i].end < min_end {
            captions[i].end = min_end;
        }
        if let Some(next_start) = captions.get(i + 1).map(|c| c.start) {
            if captions[i].end > next_start - 0.02 {
                captions[i].end = (next_start - 0.02).max(captions[i].start + 0.3);
            }
        }
    }
    captions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room_with_bursts(secs: f64, bursts: &[(f64, f64, f32)]) -> Vec<f32> {
        let n = (secs * SAMPLE_RATE as f64) as usize;
        let mut seed = 12345u32;
        let mut noise = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            ((seed >> 8) as f32 / 8388608.0) - 1.0
        };
        let mut x: Vec<f32> = (0..n).map(|_| noise() * 0.001).collect();
        for (at, len, db) in bursts {
            let amp = 0.001 * 10f32.powf(db / 20.0);
            let (a, b) = (
                (at * SAMPLE_RATE as f64) as usize,
                ((at + len) * SAMPLE_RATE as f64) as usize,
            );
            for (i, v) in x.iter_mut().enumerate().take(b.min(n)).skip(a) {
                *v += amp * (i as f32 * 0.21).sin();
            }
        }
        x
    }

    fn w(text: &str, start: f64, end: f64) -> Word {
        Word {
            text: text.into(),
            start,
            end,
        }
    }

    #[test]
    fn lines_whisper_makes_up_from_noise_are_dropped() {
        assert!(is_made_up("That's the end of the video."));
        assert!(is_made_up("Thanks for watching!"));
        assert!(!is_made_up("I like the end of the road."));
        assert!(!is_made_up("Thank you."));
    }

    #[test]
    fn only_what_was_said_is_kept_from_whispers_text() {
        assert_eq!(
            spoken_text(" Do you want me to take it off?"),
            "Do you want me to take it off?"
        );
        assert_eq!(spoken_text("(sighs)"), "");
        assert_eq!(spoken_text("[BLANK_AUDIO]"), "");
        assert_eq!(spoken_text("(laughs) Okay. (sighs)"), "Okay.");
        assert_eq!(spoken_text("♪ la la ♪"), "la la");
        assert_eq!(spoken_text(" - "), "");
        assert_eq!(spoken_text("*mumbles* Okay"), "Okay");
    }

    #[test]
    fn each_stretch_of_voice_gets_its_own_tight_piece_even_a_quiet_one() {
        // A loud line, then a line 15 dB quieter (still well above the room tone).
        let x = room_with_bursts(12.0, &[(1.0, 1.5, 30.0), (6.0, 1.0, 15.0)]);
        let p = voice_pieces(&x, 12.0);
        assert_eq!(p.len(), 2, "{p:?}");
        let secs = |r: (usize, usize)| (r.1 - r.0) as f64 / SAMPLE_RATE as f64;
        // Tight: the burst and a little around it, not the whole 12 seconds.
        assert!(secs(p[0]) > 1.5 && secs(p[0]) < 2.4, "{}", secs(p[0]));
        assert!(secs(p[1]) > 1.0 && secs(p[1]) < 1.9, "{}", secs(p[1]));
        assert!(p[1].0 > p[0].1);
    }

    #[test]
    fn tiny_blips_and_plain_room_tone_make_no_pieces() {
        let x = room_with_bursts(8.0, &[(2.0, 0.05, 30.0)]);
        assert!(voice_pieces(&x, 12.0).is_empty());
        assert!(voice_pieces(&room_with_bursts(5.0, &[]), 12.0).is_empty());
        assert!(voice_pieces(&[], 12.0).is_empty());
    }

    #[test]
    fn a_short_pause_does_not_split_a_sentence_but_a_long_one_does() {
        let x = room_with_bursts(
            10.0,
            &[(1.0, 1.0, 25.0), (2.2, 1.0, 25.0), (6.0, 1.0, 25.0)],
        );
        // 0.2 s between the first two: one piece. Then a long gap: another.
        assert_eq!(voice_pieces(&x, 12.0).len(), 2);
    }

    #[test]
    fn very_long_speech_is_cut_into_pieces_that_are_not_too_long() {
        let x = room_with_bursts(40.0, &[(1.0, 36.0, 25.0)]);
        let p = voice_pieces(&x, 12.0);
        assert!(p.len() >= 3, "{p:?}");
        assert!(p
            .iter()
            .all(|r| (r.1 - r.0) as f64 / SAMPLE_RATE as f64 <= 17.0));
        // Together they cover the speech without gaps or overlaps.
        assert!(p.windows(2).all(|w| w[0].1 <= w[1].0));
    }

    #[test]
    fn captions_break_at_pauses_and_sentences() {
        let words = vec![
            w("Welcome", 0.0, 0.4),
            w("back.", 0.5, 0.9),
            w("Today", 1.0, 1.4),
            w("we", 1.5, 1.6),
            w("edit.", 1.7, 2.1),
            w("Then", 4.0, 4.3), // after a long pause
        ];
        let c = group_captions(&words, Layout::default());
        assert_eq!(c.len(), 3, "{c:?}");
        assert_eq!(c[0].text, "Welcome back.");
        assert_eq!(c[1].text, "Today we edit.");
        assert_eq!(c[2].text, "Then");
        // Timing is ordered, never overlaps and each stays up long enough.
        for pair in c.windows(2) {
            assert!(pair[0].end < pair[1].start);
        }
        assert!(c.iter().all(|c| c.end - c.start >= 0.3));
    }

    #[test]
    fn a_sentence_starts_with_a_capital() {
        let words = vec![
            w("welcome", 0.0, 0.4),
            w("back.", 0.5, 0.9),
            w("today", 2.5, 2.9),
        ];
        let c = group_captions(&words, Layout::default());
        assert_eq!(c[0].text, "Welcome back.");
        assert_eq!(c[1].text, "Today");
    }

    #[test]
    fn long_sentences_split_into_readable_lines() {
        let text = "this is a rather long sentence that keeps going and going without any stop at all until it must be cut";
        let mut words = Vec::new();
        for (i, t) in text.split(' ').enumerate() {
            words.push(w(t, i as f64 * 0.3, i as f64 * 0.3 + 0.25));
        }
        let c = group_captions(&words, Layout::default());
        assert!(c.len() >= 2, "{c:?}");
        for cap in &c {
            for line in cap.text.lines() {
                assert!(line.chars().count() <= 36, "{line}");
            }
            assert!(cap.text.lines().count() <= 2);
        }
    }

    #[test]
    fn pieces_cut_at_the_quiet_moments() {
        // 50 s of loud sound with a quiet gap at 21 s.
        let mut s: Vec<f32> = (0..50 * SAMPLE_RATE)
            .map(|i| 0.5 * (i as f32 * 0.05).sin())
            .collect();
        for v in &mut s[21 * SAMPLE_RATE..21 * SAMPLE_RATE + 8_000] {
            *v = 0.0;
        }
        let p = pieces(&s);
        assert!(p.len() >= 2);
        let cut = p[0].1 as f64 / SAMPLE_RATE as f64;
        assert!((cut - 21.0).abs() < 0.6, "{cut}");
        // Together the pieces cover everything.
        assert_eq!(p[0].0, 0);
        assert_eq!(p.last().unwrap().1, s.len());
        assert!(p.windows(2).all(|w| w[0].1 == w[1].0));
    }
}
