//! The built-in sound effects and music beds in the Audio tab (from
//! `tidycuts-sounds`). Each one is synthesised to a WAV file in the settings
//! folder the first time it's previewed or used, and from then on it's
//! ordinary audio media. Projects keep the file's path, so a project opened
//! on another computer (or after the files were cleared) gets them rebuilt.

use crate::app::TidyCutsApp;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use tidycuts_core::{MediaId, MediaItem, MediaKind, Project, Time, TrackId};

use tidycuts_sounds::SoundDef;

/// Where the rendered sound files live.
pub fn sounds_dir() -> PathBuf {
    crate::prefs::config_dir().join("sounds")
}

/// The catalog id of a built-in sound's file (`<id>-v<N>.wav` in a
/// `sounds` folder), wherever that folder is.
pub fn sound_id_of(path: &Path) -> Option<&'static str> {
    // Saved projects may come from another system, so split on both kinds
    // of separator.
    let s = path.to_string_lossy().replace('\\', "/");
    let mut parts = s.rsplit('/');
    let stem = parts.next()?.strip_suffix(".wav")?;
    if parts.next()? != "sounds" {
        return None;
    }
    let (id, version) = stem.rsplit_once("-v")?;
    if version.is_empty() || !version.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    tidycuts_sounds::get(id).map(|d| d.id)
}

/// The media item for a sound file that's already on disk.
fn media_for(def: &SoundDef, path: PathBuf) -> MediaItem {
    let mut m = MediaItem::new(path, MediaKind::Audio, Time::from_secs_f64(def.secs as f64));
    m.name = def.name.to_string();
    m.has_audio = true;
    m.audio_streams = 1;
    m
}

/// What to do with a sound once its file is ready.
enum Then {
    Preview,
    Add(Option<(TrackId, Time)>),
}

/// Sounds rendered so far and the ones being rendered.
pub struct SoundState {
    tx: Sender<(&'static str, Result<PathBuf, String>)>,
    rx: Receiver<(&'static str, Result<PathBuf, String>)>,
    /// Ready to use, by sound id.
    ready: HashMap<&'static str, MediaItem>,
    /// Being rendered, with what was asked for meanwhile.
    pending: HashMap<&'static str, Vec<Then>>,
    failed: HashSet<&'static str>,
    ctx: egui::Context,
}

impl SoundState {
    pub fn new(ctx: &egui::Context) -> Self {
        let (tx, rx) = channel();
        SoundState {
            tx,
            rx,
            ctx: ctx.clone(),
            ready: HashMap::new(),
            pending: HashMap::new(),
            failed: HashSet::new(),
        }
    }

    pub fn is_pending(&self, id: &str) -> bool {
        self.pending.contains_key(id)
    }

    /// The media item of a sound that has been rendered.
    pub fn media(&self, id: &str) -> Option<&MediaItem> {
        self.ready.get(id)
    }

    /// A rendered sound's media item by media id (for the source preview).
    pub fn media_by_id(&self, id: MediaId) -> Option<&MediaItem> {
        self.ready.values().find(|m| m.id == id)
    }

    fn request(&mut self, id: &'static str, then: Then) {
        if let Some(waiting) = self.pending.get_mut(id) {
            // A second click while it renders replaces the first ask.
            waiting.retain(|t| !matches!((t, &then), (Then::Preview, Then::Preview)));
            waiting.push(then);
            return;
        }
        self.failed.remove(id);
        self.pending.insert(id, vec![then]);
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let result = tidycuts_sounds::ensure_file(id, &sounds_dir()).map_err(|e| e.to_string());
            let _ = tx.send((id, result));
            ctx.request_repaint();
        });
    }
}

impl TidyCutsApp {
    /// Plays a built-in sound in the preview, rendering it first if needed.
    pub fn preview_sound(&mut self, id: &'static str) {
        match self.sounds.ready.get(id).map(|m| m.id) {
            Some(m) => {
                if self.preview.source_media == Some(m) && self.is_playing() {
                    self.pause();
                } else {
                    self.show_source(m);
                    self.toggle_play();
                }
            }
            None => self.sounds.request(id, Then::Preview),
        }
    }

    /// Puts a built-in sound on the timeline: at `at`, or at the playhead
    /// on a free audio track.
    pub fn add_sound(&mut self, id: &'static str, at: Option<(TrackId, Time)>) {
        match self.sounds.ready.get(id).cloned() {
            Some(m) => self.place_sound(m, at),
            None => self.sounds.request(id, Then::Add(at)),
        }
    }

    fn place_sound(&mut self, m: MediaItem, at: Option<(TrackId, Time)>) {
        // The project may already hold this sound (under another id when
        // it came from a saved project).
        let media = self
            .project
            .media
            .iter()
            .find(|p| p.path == m.path)
            .map(|p| p.id)
            .unwrap_or(m.id);
        self.add_media_to_timeline(media, at);
    }

    /// Takes finished renders and does what was asked for them.
    pub fn poll_sounds(&mut self) {
        while let Ok((id, result)) = self.sounds.rx.try_recv() {
            let waiting = self.sounds.pending.remove(id).unwrap_or_default();
            let path = match result {
                Ok(p) => p,
                Err(e) => {
                    log::warn!("couldn't make the sound {id}: {e}");
                    self.sounds.failed.insert(id);
                    self.notify("That sound couldn't be prepared.", true);
                    continue;
                }
            };
            let Some(def) = tidycuts_sounds::get(id) else {
                continue;
            };
            let m = media_for(def, path);
            self.sounds.ready.insert(id, m.clone());
            for then in waiting {
                match then {
                    Then::Preview => {
                        self.show_source(m.id);
                        self.toggle_play();
                    }
                    Then::Add(at) => self.place_sound(m.clone(), at),
                }
            }
        }
    }
}

/// Points a loaded project's built-in sounds at this computer's sound
/// folder, rebuilding any files that are missing.
pub fn restore_sounds(project: &mut Project) {
    let dir = sounds_dir();
    for m in &mut project.media {
        if m.path.is_file() {
            continue;
        }
        let Some(id) = sound_id_of(&m.path) else {
            continue;
        };
        match tidycuts_sounds::ensure_file(id, &dir) {
            Ok(p) => m.path = p,
            Err(e) => log::warn!("couldn't rebuild the sound {id}: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_sound_files_from_any_computer() {
        let id = tidycuts_sounds::catalog()[0].id;
        let p = PathBuf::from(format!("/somewhere/else/sounds/{id}-v1.wav"));
        assert_eq!(sound_id_of(&p), Some(id));
        let p = PathBuf::from(format!("C:\\Users\\x\\sounds\\{id}-v12.wav"));
        assert_eq!(sound_id_of(&p), Some(id));
        assert_eq!(sound_id_of(Path::new("/music/sounds/mysong-v1.wav")), None);
        let p = PathBuf::from(format!("/home/me/Music/{id}-v1.wav"));
        assert_eq!(sound_id_of(&p), None, "only files in a sounds folder");
        let p = PathBuf::from(format!("/x/sounds/{id}.wav"));
        assert_eq!(sound_id_of(&p), None);
    }

    #[test]
    fn media_items_match_the_catalog() {
        let def = tidycuts_sounds::catalog()[0];
        let m = media_for(&def, PathBuf::from("/x/sounds/a-v1.wav"));
        assert_eq!(m.kind, MediaKind::Audio);
        assert_eq!(m.name, def.name);
        assert!(m.has_audio);
        assert!((m.duration.as_secs_f64() - def.secs as f64).abs() < 1e-3);
    }
}
