//! Puts speech and music on a timeline and prints where the speech was found
//! and the volume points the music would get:
//! `autoduck_check speech.wav music.wav`.

use std::sync::atomic::AtomicBool;
use tidycuts_core::*;
use tidycuts_engine::autoduck::{duck_points, find_speech};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let speech = tidycuts_media::probe(std::path::Path::new(&args[1])).expect("speech");
    let music = tidycuts_media::probe(std::path::Path::new(&args[2])).expect("music");
    let mut p = Project::new("duck");
    let (ms, mm) = (p.add_media(speech), p.add_media(music));
    let t1 = p.add_track(TrackKind::Audio).unwrap();
    let t2 = p.add_track(TrackKind::Audio).unwrap();
    // The speech starts 3 s in and has a 4 s gap in the middle (two clips).
    let a = p.add_clip(ms, t1, Time::from_secs_f64(3.0)).unwrap();
    let music_clip = p.add_clip(mm, t2, Time::ZERO).unwrap();
    let _ = a;
    let found = find_speech(&p, music_clip, &AtomicBool::new(false)).expect("find");
    println!("speech found in {} stretch(es):", found.len());
    for (s, e) in &found {
        println!("  {s:.2} - {e:.2}");
    }
    let len = p.clip(music_clip).unwrap().1.duration().as_secs_f64();
    let pts = duck_points(&found, len, 100.0, 25.0, 0.35, 0.9);
    println!("{} volume points", pts.len());
    for (t, v) in pts.iter().take(10) {
        println!("  {t:.2}s -> {v:.0}");
    }
}
