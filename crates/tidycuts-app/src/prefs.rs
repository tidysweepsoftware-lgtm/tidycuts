//! Preferences that outlive a project: defaults for new projects, import
//! behaviour, editing defaults and performance settings. Saved as JSON in
//! the user's config folder.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tidycuts_core::ProjectSettings;

/// Where the panels sit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutPreset {
    /// Library, preview and properties side by side across the top.
    #[default]
    Standard,
    /// The same turned around: properties, preview, library.
    Mirrored,
    /// Menus on the left: the library, and the properties of a clip opened
    /// with a double-click, take the wide left side next to the preview.
    LeftMenus,
}

impl LayoutPreset {
    pub const ALL: [LayoutPreset; 3] = [
        LayoutPreset::Standard,
        LayoutPreset::Mirrored,
        LayoutPreset::LeftMenus,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LayoutPreset::Standard => "Standard",
            LayoutPreset::Mirrored => "Mirrored",
            LayoutPreset::LeftMenus => "Left menus",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            LayoutPreset::Standard => "Library, preview, properties",
            LayoutPreset::Mirrored => "Properties, preview, library",
            LayoutPreset::LeftMenus => "Library and properties on the left, next to the preview",
        }
    }

    /// Whether the properties panel has a place of its own (the left-menus
    /// layout shows it in place of the library instead).
    pub fn has_dock(self) -> bool {
        self != LayoutPreset::LeftMenus
    }
}

/// What to do when imported video doesn't match the project's size or
/// frame rate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportSize {
    /// Ask every time.
    #[default]
    Ask,
    /// Change the project to the video's size and frame rate.
    MatchMedia,
    /// Keep the project's settings; the video is scaled to fit (e.g. 4K
    /// shown and exported at 1080p).
    KeepProject,
    /// Take the video's shape and frame rate, but bring anything bigger
    /// than 1080p down to 1080p.
    Downscale1080,
}

impl ImportSize {
    pub const ALL: [ImportSize; 4] = [
        ImportSize::Ask,
        ImportSize::MatchMedia,
        ImportSize::Downscale1080,
        ImportSize::KeepProject,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ImportSize::Ask => "Ask me every time",
            ImportSize::MatchMedia => "Match the project to the video",
            ImportSize::KeepProject => "Keep the project settings (scale the video)",
            ImportSize::Downscale1080 => "Match the video, but at most 1080p",
        }
    }
}

/// Preview resolution compared with the size it's shown at.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreviewQuality {
    /// Full while paused, half while playing large projects.
    #[default]
    Auto,
    Full,
    Half,
    Quarter,
}

impl PreviewQuality {
    pub const ALL: [PreviewQuality; 4] = [
        PreviewQuality::Auto,
        PreviewQuality::Full,
        PreviewQuality::Half,
        PreviewQuality::Quarter,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PreviewQuality::Auto => "Auto",
            PreviewQuality::Full => "Full Quality",
            PreviewQuality::Half => "1/2 Quality",
            PreviewQuality::Quarter => "1/4 Quality",
        }
    }

    /// Fraction of the on-screen size to render at.
    pub fn factor(self, playing: bool, pixels: u32) -> f32 {
        match self {
            PreviewQuality::Auto => {
                if playing && pixels > 1920 * 1080 {
                    0.5
                } else {
                    1.0
                }
            }
            PreviewQuality::Full => 1.0,
            PreviewQuality::Half => 0.5,
            PreviewQuality::Quarter => 0.25,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// Settings every new project starts with.
    pub project_defaults: ProjectSettings,
    pub import_size: ImportSize,
    /// Default lengths in seconds.
    pub image_duration: f64,
    pub title_duration: f64,
    pub sticker_duration: f64,
    pub transition_duration: f64,
    pub animation_duration: f64,
    pub snapping: bool,
    /// Keep following the playhead while playing.
    pub auto_scroll: bool,
    pub hardware_decode: bool,
    pub hardware_encode: bool,
    /// Edit with small proxy copies of big videos when there are some.
    pub use_proxies: bool,
    /// Make a proxy for every large video as it is imported.
    pub auto_proxies: bool,
    pub preview_quality: PreviewQuality,
    /// Where scrub frames are kept; `None` uses the system cache folder.
    pub cache_dir: Option<PathBuf>,
    /// Scrub cache size limit in gigabytes.
    pub cache_limit_gb: f64,
    /// Minutes between automatic backups; 0 turns them off.
    pub autosave_minutes: u32,
    pub export_folder: Option<PathBuf>,
    pub recent_projects: Vec<PathBuf>,
    /// Library width and top-area height as fractions of the window.
    pub layout: [f32; 2],
    /// The same for the left-menus layout, where the left side is wider.
    #[serde(alias = "layout_left_menus")]
    pub layout_left_menus: [f32; 2],
    /// Which arrangement of the panels is in use.
    pub layout_preset: LayoutPreset,
    /// Width of the properties panel in points (dragged by its edge).
    pub dock_w: f32,
    /// Ask before deleting a track that has clips on it.
    pub confirm_track_delete: bool,
    /// Let Claude connect and edit (a local server, off by default).
    pub mcp_enabled: bool,
    /// The port Claude connects to (another is used if it is taken).
    pub mcp_port: u16,
    /// The secret a connection must show.
    pub mcp_token: String,
    /// The welcome tour has been shown once.
    pub tour_done: bool,
    /// Hearing guard for playback: 0 off, 1 normal (-10 LUFS), 2 careful (-16 LUFS).
    pub hearing_guard: u8,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            project_defaults: ProjectSettings::default(),
            import_size: ImportSize::Ask,
            image_duration: 5.0,
            title_duration: 5.0,
            sticker_duration: 5.0,
            transition_duration: 1.0,
            animation_duration: 0.8,
            snapping: true,
            auto_scroll: true,
            hardware_decode: true,
            hardware_encode: true,
            use_proxies: true,
            auto_proxies: true,
            preview_quality: PreviewQuality::Auto,
            cache_dir: None,
            cache_limit_gb: 20.0,
            autosave_minutes: 5,
            export_folder: None,
            recent_projects: Vec::new(),
            layout: [0.496, 0.587],
            layout_left_menus: [0.5, 0.587],
            layout_preset: LayoutPreset::Standard,
            dock_w: 310.0,
            confirm_track_delete: true,
            mcp_enabled: false,
            mcp_port: 47861,
            mcp_token: String::new(),
            tour_done: false,
            hearing_guard: 1,
        }
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// The folder TidyCuts keeps its settings in.
pub fn config_dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".config")))
    };
    base.unwrap_or_else(std::env::temp_dir)
        .join(if cfg!(any(windows, target_os = "macos")) {
            "TidyCuts"
        } else {
            "tidycuts"
        })
}

/// The system cache folder for TidyCuts (scrub frames, waveforms).
pub fn default_cache_dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library/Caches"))
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".cache")))
    };
    base.unwrap_or_else(std::env::temp_dir)
        .join(if cfg!(any(windows, target_os = "macos")) {
            "TidyCuts"
        } else {
            "tidycuts"
        })
        .join("cache")
}

/// Moves the settings and cache folders from where the program kept them
/// under its first name (RustCut) to where it keeps them now, once, so
/// nothing is lost. Does nothing when the new folders already exist.
pub fn migrate_legacy_folders() {
    let legacy = if cfg!(any(windows, target_os = "macos")) {
        "RustCut"
    } else {
        "rustcut"
    };
    let old_config = config_dir().with_file_name(legacy);
    let cache_root = default_cache_dir()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let old_cache = cache_root.with_file_name(legacy);
    for (old, new) in [(old_config, config_dir()), (old_cache, cache_root)] {
        match move_if_absent(&old, &new) {
            Ok(true) => log::info!("moved {} to {}", old.display(), new.display()),
            Ok(false) => {}
            Err(e) => log::warn!("could not move {}: {e}", old.display()),
        }
    }
}

/// Renames the folder `old` to `new` when `old` exists and `new` doesn't.
fn move_if_absent(old: &Path, new: &Path) -> std::io::Result<bool> {
    if new.exists() || !old.is_dir() {
        return Ok(false);
    }
    std::fs::rename(old, new)?;
    Ok(true)
}

/// Where automatic backups of unsaved projects go.
pub fn autosave_dir() -> PathBuf {
    config_dir().join("autosave")
}

/// Backup copies of projects that were never saved or closed properly
/// (a crash or a power cut leaves them behind), newest first.
pub fn leftover_autosaves() -> Vec<(PathBuf, std::time::SystemTime)> {
    backups_in(&autosave_dir())
}

fn backups_in(dir: &Path) -> Vec<(PathBuf, std::time::SystemTime)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e == tidycuts_core::PROJECT_EXTENSION)
        })
        .filter_map(|p| {
            let t = p.metadata().ok()?.modified().ok()?;
            Some((p, t))
        })
        .collect();
    out.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
    out
}

/// The loudest level (LUFS) the hearing guard allows for a setting of the
/// preference, or `None` when it is off.
pub fn guard_ceiling(level: u8) -> Option<f32> {
    match level {
        0 => None,
        2 => Some(-16.0),
        _ => Some(-10.0),
    }
}

impl Prefs {
    /// The split fractions of the arrangement in use.
    pub fn fractions(&self) -> [f32; 2] {
        match self.layout_preset {
            LayoutPreset::LeftMenus => self.layout_left_menus,
            _ => self.layout,
        }
    }

    pub fn set_fractions(&mut self, f: [f32; 2]) {
        match self.layout_preset {
            LayoutPreset::LeftMenus => self.layout_left_menus = f,
            _ => self.layout = f,
        }
    }

    /// Puts the panels of the arrangement in use back to their first sizes.
    pub fn reset_layout(&mut self) {
        let d = Prefs::default();
        self.layout = d.layout;
        self.layout_left_menus = d.layout_left_menus;
        self.dock_w = d.dock_w;
    }

    fn path() -> PathBuf {
        config_dir().join("preferences.json")
    }

    pub fn load() -> Prefs {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    log::warn!("could not save preferences: {e}");
                }
            }
            Err(e) => log::warn!("could not save preferences: {e}"),
        }
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.cache_dir.clone().unwrap_or_else(default_cache_dir)
    }

    /// Puts a project at the top of the recent list.
    pub fn add_recent(&mut self, path: &Path) {
        self.recent_projects.retain(|p| p != path);
        self.recent_projects.insert(0, path.to_path_buf());
        self.recent_projects.truncate(10);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_preference_files_keep_working() {
        let p: Prefs = serde_json::from_str(r#"{"snapping": false}"#).unwrap();
        assert!(!p.snapping);
        assert_eq!(p.image_duration, 5.0);
        assert_eq!(p.import_size, ImportSize::Ask);
    }

    #[test]
    fn recent_list_moves_to_top_without_duplicates() {
        let mut p = Prefs::default();
        p.add_recent(Path::new("/a.rcproj"));
        p.add_recent(Path::new("/b.rcproj"));
        p.add_recent(Path::new("/a.rcproj"));
        assert_eq!(
            p.recent_projects,
            vec![PathBuf::from("/a.rcproj"), PathBuf::from("/b.rcproj")]
        );
    }

    #[test]
    fn old_folders_move_over_once_and_never_overwrite() {
        let root = std::env::temp_dir().join(format!("tidycuts-move-{}", uuid::Uuid::new_v4()));
        let (old, new) = (root.join("RustCut"), root.join("TidyCuts"));
        std::fs::create_dir_all(old.join("autosave")).unwrap();
        std::fs::write(old.join("preferences.json"), "{}").unwrap();
        assert!(move_if_absent(&old, &new).unwrap());
        assert!(new.join("preferences.json").is_file() && new.join("autosave").is_dir());
        assert!(!old.exists());
        // Nothing to move, and an existing new folder is left alone.
        assert!(!move_if_absent(&old, &new).unwrap());
        std::fs::create_dir_all(&old).unwrap();
        assert!(!move_if_absent(&old, &new).unwrap());
        assert!(old.is_dir());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn leftover_backups_are_listed_newest_first() {
        let dir = std::env::temp_dir().join(format!("tidycuts-backups-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(backups_in(&dir).is_empty());
        std::fs::write(dir.join("old (autosave).tcproj"), "{}").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        std::fs::write(dir.join("new (autosave).tcproj"), "{}").unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let found = backups_in(&dir);
        let names: Vec<_> = found
            .iter()
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["new (autosave).tcproj", "old (autosave).tcproj"]);
        assert!(backups_in(&dir.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
