//! Voiceovers from typed text, spoken by the computer's own voices (see
//! `tidycuts_media::speech`). The sound file is made on a worker thread and
//! then put on an audio track at the playhead.

use crate::app::TidyCutsApp;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use tidycuts_core::{MediaItem, Time};
use tidycuts_media::speech;

pub type Done = Result<MediaItem, String>;

/// A short name for a voiceover: its first few words.
pub fn name_for(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().take(5).collect();
    let mut name = words.join(" ");
    if text.split_whitespace().count() > 5 {
        name.push('…');
    }
    format!("Voiceover: {name}")
}

/// Makes the voiceover file and reads it as media. Takes a while: call it
/// from a worker thread.
pub fn make(cache_dir: PathBuf, text: &str, voice: &str, speed: f64) -> Done {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (text, voice, (speed * 100.0) as i64).hash(&mut h);
    let out =
        cache_dir
            .join("voiceovers")
            .join(format!("{:016x}.{}", h.finish(), speech::extension()));
    if !out.is_file() {
        speech::speak(text, voice, speed, &out)?;
    }
    let mut item = tidycuts_media::probe(&out).map_err(|e| e.to_string())?;
    item.name = name_for(text);
    Ok(item)
}

impl TidyCutsApp {
    /// Starts a voiceover in the background; it appears on the timeline when
    /// it is ready.
    pub fn start_voiceover(&mut self, text: String, voice: String, speed: f64) {
        let tx = self.voice_tx.clone();
        let ctx = self.ctx.clone();
        let dir = self.prefs.cache_dir();
        self.notify("Making the voiceover…", false);
        std::thread::spawn(move || {
            let _ = tx.send(make(dir, &text, &voice, speed));
            ctx.request_repaint();
        });
    }

    /// Makes a voiceover right now and puts it at `at` (the caller waits).
    /// For the assistant's tools.
    pub fn voiceover_now(
        &mut self,
        text: &str,
        voice: &str,
        speed: f64,
        at: Option<Time>,
    ) -> Result<String, String> {
        let item = make(self.prefs.cache_dir(), text, voice, speed)?;
        self.place_voiceover(item, at)
    }

    pub fn poll_voiceovers(&mut self) {
        while let Ok(done) = self.voice_rx.try_recv() {
            match done.and_then(|item| self.place_voiceover(item, None)) {
                Ok(text) => self.notify(&text, false),
                Err(e) => self.notify(&e, true),
            }
        }
    }

    /// Adds the voiceover to the library and an audio track at `at` (the
    /// playhead when `None`).
    fn place_voiceover(&mut self, item: MediaItem, at: Option<Time>) -> Result<String, String> {
        let (id, len) = (item.id, item.duration);
        self.checkpoint();
        self.add_imported_media(item);
        let at = at.unwrap_or_else(|| self.playhead());
        let track = self
            .project
            .audio_track_for(at, len)
            .ok_or("There is no free audio track for the voiceover.")?;
        self.add_media_to_timeline(id, Some((track, at)));
        Ok("The voiceover is on the timeline.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::name_for;

    #[test]
    fn names_use_the_first_words() {
        assert_eq!(name_for("Hello there"), "Voiceover: Hello there");
        assert_eq!(
            name_for("one two three four five six seven"),
            "Voiceover: one two three four five…"
        );
    }
}
