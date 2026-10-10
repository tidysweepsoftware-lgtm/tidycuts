use crate::clip::{Clip, ClipId, Color};
use crate::marker::Marker;
use crate::time::{FrameRate, Time};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// File extension for saved projects.
pub const PROJECT_EXTENSION: &str = "tcproj";

/// Extensions projects had before the program was called TidyCuts; they
/// still open.
pub const LEGACY_PROJECT_EXTENSIONS: [&str; 1] = ["rcproj"];

/// Whether `path` names a project file, with the current or an older extension.
pub fn is_project_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        e.eq_ignore_ascii_case(PROJECT_EXTENSION)
            || LEGACY_PROJECT_EXTENSIONS
                .iter()
                .any(|l| e.eq_ignore_ascii_case(l))
    })
}

/// Bumped whenever the on-disk format changes incompatibly.
const FORMAT_VERSION: u32 = 2;

pub type MediaId = Uuid;
pub type TrackId = Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("could not read or write the project file: {0}")]
    Io(#[from] std::io::Error),
    #[error("the project file is damaged or not a TidyCuts project: {0}")]
    Format(#[from] serde_json::Error),
    #[error("this project was saved by a newer version of TidyCuts (format {0})")]
    TooNew(u32),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub sample_rate: u32,
    /// Keeps the main track (the lowest video track) free of gaps, like
    /// The magnetic timeline.
    #[serde(default = "default_true")]
    pub magnetic: bool,
    /// Shown wherever no clip covers the frame.
    #[serde(default = "default_background")]
    pub background: Color,
}

fn default_true() -> bool {
    true
}

fn default_background() -> Color {
    [0.0, 0.0, 0.0, 1.0]
}

impl Default for ProjectSettings {
    fn default() -> Self {
        ProjectSettings {
            width: 1920,
            height: 1080,
            frame_rate: FrameRate::new(25, 1),
            sample_rate: 48_000,
            magnetic: true,
            background: default_background(),
        }
    }
}

impl ProjectSettings {
    /// Aspect ratio label shown under the preview, e.g. "16:9".
    pub fn aspect_label(&self) -> String {
        fn gcd(a: u32, b: u32) -> u32 {
            if b == 0 {
                a
            } else {
                gcd(b, a % b)
            }
        }
        let g = gcd(self.width, self.height).max(1);
        format!("{}:{}", self.width / g, self.height / g)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
    Video,
    Audio,
    Image,
}

/// A file imported into the project's media library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: MediaId,
    pub path: PathBuf,
    pub name: String,
    pub kind: MediaKind,
    /// Natural length. Images use a default still duration.
    pub duration: Time,
    pub width: u32,
    pub height: u32,
    pub frame_rate: Option<FrameRate>,
    pub has_audio: bool,
    /// The picture has an alpha channel (PNG, ProRes 4444, WebM with alpha…).
    #[serde(default)]
    pub has_alpha: bool,
    /// YUV matrix of the video: `"bt709"`, `"bt601"` or `"bt2020"`.
    #[serde(default)]
    pub color_matrix: Option<String>,
    /// Full-range ("PC") levels instead of TV levels.
    #[serde(default)]
    pub full_range: bool,
    /// Number of audio streams (some recordings keep mic and game sound apart).
    #[serde(default)]
    pub audio_streams: u32,
    /// A smaller, fast-to-decode copy of the video used while editing. The
    /// original is always used for export.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<PathBuf>,
}

/// Whether editing uses proxy files (a preference; the file is still used
/// only if it exists).
pub static USE_PROXIES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

impl MediaItem {
    /// The file to decode for the preview: the proxy when there is one and
    /// proxies are on, otherwise the original.
    pub fn preview_path(&self) -> &std::path::Path {
        match &self.proxy {
            Some(p) if USE_PROXIES.load(std::sync::atomic::Ordering::Relaxed) && p.is_file() => p,
            _ => &self.path,
        }
    }

    /// A video big enough that editing is smoother with a proxy.
    pub fn wants_proxy(&self) -> bool {
        self.kind == MediaKind::Video
            && self.proxy.is_none()
            && u64::from(self.width) * u64::from(self.height) > 1920 * 1080 * 12 / 10
    }

    /// How long a still image lasts when dropped on the timeline.
    pub const DEFAULT_IMAGE_DURATION: Time = Time(5_000_000);

    pub fn new(path: PathBuf, kind: MediaKind, duration: Time) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        MediaItem {
            id: Uuid::new_v4(),
            path,
            name,
            kind,
            duration,
            width: 0,
            height: 0,
            frame_rate: None,
            has_audio: false,
            has_alpha: false,
            color_matrix: None,
            full_range: false,
            audio_streams: 0,
            proxy: None,
        }
    }
}

/// Most video tracks a project can have (for A-roll, B-roll, overlays).
pub const MAX_VIDEO_TRACKS: usize = 10;
/// Most audio tracks a project can have.
pub const MAX_AUDIO_TRACKS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub kind: TrackKind,
    pub name: String,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub hidden: bool,
    /// Plays alone (with any other soloed tracks): the rest are silent.
    #[serde(default)]
    pub solo: bool,
    /// The track's volume in the mixer, percent (100 leaves clips as they are).
    #[serde(default = "full_volume")]
    pub volume: f64,
    /// Kept sorted by `start`, never overlapping.
    pub clips: Vec<Clip>,
}

fn full_volume() -> f64 {
    100.0
}

fn is_full_volume(v: &f64) -> bool {
    (*v - 100.0).abs() < 1e-9
}

impl Track {
    pub fn new(kind: TrackKind, name: impl Into<String>) -> Self {
        Track {
            id: Uuid::new_v4(),
            kind,
            name: name.into(),
            locked: false,
            muted: false,
            hidden: false,
            solo: false,
            volume: 100.0,
            clips: Vec::new(),
        }
    }

    pub fn end(&self) -> Time {
        self.clips.iter().map(Clip::end).max().unwrap_or(Time::ZERO)
    }

    pub fn clip_at(&self, t: Time) -> Option<&Clip> {
        self.clips.iter().find(|c| c.contains(t))
    }

    /// Earliest start at or after `wanted` where a clip of `len` fits
    /// without overlapping anything already on the track.
    pub fn free_slot(&self, wanted: Time, len: Time, ignore: Option<ClipId>) -> Time {
        let mut start = wanted.max(Time::ZERO);
        loop {
            let clash = self
                .clips
                .iter()
                .filter(|c| Some(c.id) != ignore)
                .find(|c| start < c.end() && c.start < start + len);
            match clash {
                Some(c) => start = c.end(),
                None => return start,
            }
        }
    }
}

/// A whole editing project, saved as JSON with the `.rcproj` extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub format_version: u32,
    pub name: String,
    pub settings: ProjectSettings,
    pub media: Vec<MediaItem>,
    pub tracks: Vec<Track>,
    /// Markers on the ruler, kept sorted by time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Marker>,
    /// The mixer's master volume, percent.
    #[serde(default = "full_volume", skip_serializing_if = "is_full_volume")]
    pub master_volume: f64,
}

impl Default for Project {
    fn default() -> Self {
        Project::new("Untitled")
    }
}

impl Project {
    /// A new project with one video track, one audio track and an empty
    /// timeline.
    pub fn new(name: impl Into<String>) -> Self {
        Project {
            format_version: FORMAT_VERSION,
            name: name.into(),
            settings: ProjectSettings::default(),
            media: Vec::new(),
            tracks: vec![
                Track::new(TrackKind::Video, "Video 1"),
                Track::new(TrackKind::Audio, "Audio 1"),
            ],
            markers: Vec::new(),
            master_volume: 100.0,
        }
    }

    pub fn duration(&self) -> Time {
        self.tracks
            .iter()
            .map(Track::end)
            .max()
            .unwrap_or(Time::ZERO)
    }

    pub fn media_mut(&mut self, id: MediaId) -> Option<&mut MediaItem> {
        self.media.iter_mut().find(|m| m.id == id)
    }

    pub fn media(&self, id: MediaId) -> Option<&MediaItem> {
        self.media.iter().find(|m| m.id == id)
    }

    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    pub fn clip(&self, id: ClipId) -> Option<(&Track, &Clip)> {
        self.tracks
            .iter()
            .find_map(|t| t.clips.iter().find(|c| c.id == id).map(|c| (t, c)))
    }

    pub fn clip_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        self.tracks
            .iter_mut()
            .find_map(|t| t.clips.iter_mut().find(|c| c.id == id))
    }

    pub fn add_media(&mut self, item: MediaItem) -> MediaId {
        let id = item.id;
        self.media.push(item);
        id
    }

    /// Removes a media item and every clip that uses it.
    pub fn remove_media(&mut self, id: MediaId) {
        self.media.retain(|m| m.id != id);
        for t in &mut self.tracks {
            t.clips.retain(|c| c.media_id != Some(id));
        }
    }

    pub fn track_count(&self, kind: TrackKind) -> usize {
        self.tracks.iter().filter(|t| t.kind == kind).count()
    }

    /// Whether another track of `kind` fits under the limits
    /// ([`MAX_VIDEO_TRACKS`], [`MAX_AUDIO_TRACKS`]).
    pub fn can_add_track(&self, kind: TrackKind) -> bool {
        let max = match kind {
            TrackKind::Video => MAX_VIDEO_TRACKS,
            TrackKind::Audio => MAX_AUDIO_TRACKS,
        };
        self.track_count(kind) < max
    }

    /// Adds a track of `kind`, named "Video 2", "Audio 3" and so on.
    /// Video tracks go above existing video tracks; audio tracks go last.
    /// Returns `None` when the track limit is reached.
    pub fn add_track(&mut self, kind: TrackKind) -> Option<TrackId> {
        let at = match kind {
            TrackKind::Video => 0,
            TrackKind::Audio => self.tracks.len(),
        };
        self.insert_track(kind, at)
    }

    /// Adds a track of `kind` at position `index` in the track list (0 is
    /// the top). Video tracks always stay above audio tracks.
    pub fn insert_track(&mut self, kind: TrackKind, index: usize) -> Option<TrackId> {
        if !self.can_add_track(kind) {
            return None;
        }
        let label = match kind {
            TrackKind::Video => "Video",
            TrackKind::Audio => "Audio",
        };
        let mut n = self.track_count(kind) + 1;
        while self.tracks.iter().any(|t| t.name == format!("{label} {n}")) {
            n += 1;
        }
        let first_audio = self
            .tracks
            .iter()
            .position(|t| t.kind == TrackKind::Audio)
            .unwrap_or(self.tracks.len());
        let index = match kind {
            TrackKind::Video => index.min(first_audio),
            TrackKind::Audio => index.clamp(first_audio, self.tracks.len()),
        };
        let track = Track::new(kind, format!("{label} {n}"));
        let id = track.id;
        self.tracks.insert(index, track);
        Some(id)
    }

    /// Removes a track and its clips. The last video track can't be removed
    /// because it's the main track.
    pub fn remove_track(&mut self, id: TrackId) -> bool {
        let Some(i) = self.tracks.iter().position(|t| t.id == id) else {
            return false;
        };
        let kind = self.tracks[i].kind;
        if kind == TrackKind::Video && self.track_count(TrackKind::Video) <= 1 {
            return false;
        }
        self.tracks.remove(i);
        true
    }

    pub fn to_json(&self) -> Result<String, ProjectError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(json: &str) -> Result<Project, ProjectError> {
        let project: Project = serde_json::from_str(json)?;
        if project.format_version > FORMAT_VERSION {
            return Err(ProjectError::TooNew(project.format_version));
        }
        Ok(project)
    }

    pub fn save(&self, path: &Path) -> Result<(), ProjectError> {
        // Write to a temp file first so a crash can't leave a half-written project.
        let mut tmp_name = path
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default();
        tmp_name.push(".tmp");
        let tmp = path.with_file_name(tmp_name);
        std::fs::write(&tmp, self.to_json()?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Project, ProjectError> {
        Project::from_json(&std::fs::read_to_string(path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(secs: f64) -> MediaItem {
        let mut m = MediaItem::new(
            PathBuf::from("/clips/a.mp4"),
            MediaKind::Video,
            Time::from_secs_f64(secs),
        );
        m.has_audio = true;
        m
    }

    fn s(secs: f64) -> Time {
        Time::from_secs_f64(secs)
    }

    #[test]
    fn new_project_has_one_video_and_one_audio_track() {
        let p = Project::new("Untitled");
        assert_eq!(p.tracks.len(), 2);
        assert_eq!(p.tracks[0].name, "Video 1");
        assert_eq!(p.tracks[1].name, "Audio 1");
        assert_eq!(p.settings.aspect_label(), "16:9");
        assert!(p.settings.magnetic);
        assert_eq!(p.duration(), Time::ZERO);
    }

    #[test]
    fn removing_media_removes_its_clips() {
        let mut p = Project::default();
        let m = p.add_media(video(4.0));
        p.append_clip(m).unwrap();
        p.remove_media(m);
        assert!(p.media.is_empty());
        assert_eq!(p.duration(), Time::ZERO);
    }

    #[test]
    fn save_and_load_round_trip() {
        let mut p = Project::new("Trip");
        let m = p.add_media(video(4.0));
        let c = p.append_clip(m).unwrap();
        {
            let clip = p.clip_mut(c).unwrap();
            clip.transform.scale_x.set_keyframe(s(1.0), 150.0);
            clip.effects.push(crate::clip::Effect::new("blur.gaussian"));
            clip.crop.left = 0.1;
        }
        let t = p.add_track(TrackKind::Video).unwrap();
        p.add_generated(
            crate::clip::Generator::Title(Default::default()),
            s(3.0),
            t,
            s(0.5),
        )
        .unwrap();
        let dir = std::env::temp_dir().join(format!("tidycuts-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("trip.rcproj");
        p.save(&path).unwrap();
        let loaded = Project::load(&path).unwrap();
        assert_eq!(loaded, p);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn loads_step_one_projects() {
        let id = Uuid::new_v4();
        let json = format!(
            r#"{{"format_version":1,"name":"Old","settings":{{"width":1920,"height":1080,
            "frame_rate":{{"num":25,"den":1}},"sample_rate":48000}},
            "media":[{{"id":"{id}","path":"a.mp4","name":"a.mp4","kind":"Video","duration":4000000,
            "width":1920,"height":1080,"frame_rate":null,"has_audio":true}}],
            "tracks":[{{"id":"{t}","kind":"Video","name":"Video 1","clips":[{{"id":"{c}",
            "media_id":"{id}","start":0,"source_in":0,"source_out":4000000}}]}}]}}"#,
            t = Uuid::new_v4(),
            c = Uuid::new_v4()
        );
        let p = Project::from_json(&json).unwrap();
        let clip = &p.tracks[0].clips[0];
        assert_eq!(clip.media_id, Some(id));
        assert_eq!(clip.speed, 1.0);
        assert_eq!(clip.duration(), s(4.0));
        assert!(p.settings.magnetic);
    }

    #[test]
    fn refuses_newer_formats() {
        let p = Project {
            format_version: 999,
            ..Default::default()
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(matches!(
            Project::from_json(&json),
            Err(ProjectError::TooNew(999))
        ));
    }
}
