//! Hosts the playback engine inside the window: the GPU compositor shares
//! egui's graphics device and draws the preview straight into a texture
//! egui shows, so frames never round-trip through the CPU.

use eframe::egui_wgpu::RenderState;
use eframe::wgpu;
use std::sync::Arc;
use tidycuts_core::{Clip, MediaId, MediaItem, MediaKind, Project, ProjectSettings, Time};
use tidycuts_engine::analysis::Analysis;
use tidycuts_engine::{
    FrameServer, Planner, Playback, PreparedFrame, PreviewFrames, ScrubCache, Statics,
};
use tidycuts_render::{Compositor, Grade, Renderer, Target};

/// What the preview texture currently shows, so unchanged frames aren't
/// drawn twice.
#[derive(Debug, Clone, PartialEq)]
struct Shown {
    revision: u64,
    time: Time,
    size: (u32, u32),
    source: Option<MediaId>,
}

pub struct Engine {
    rs: RenderState,
    pub renderer: Arc<Renderer>,
    pub compositor: Compositor,
    target: Option<Target>,
    /// The preview picture as an egui texture.
    pub texture: Option<egui::TextureId>,
    pub scrub: Arc<ScrubCache>,
    pub frames: FrameServer,
    pub statics: Arc<Statics>,
    pub analysis: Analysis,
    pub playback: Playback,
    /// Revision of the project the playback threads have.
    playback_revision: u64,
    /// Rendered previews of the timeline, played where they cover it.
    previews: Vec<tidycuts_engine::previews::Cached>,
    /// What the playback threads are playing is the timeline (not a source).
    playing_timeline: bool,
    shown: Option<Shown>,
    /// The last still preview used a stand-in picture somewhere.
    pub incomplete: bool,
    pub hwaccel: bool,
    /// Name of the graphics adapter, for the About box.
    pub adapter: String,
    /// Pictures drawn so far, and the one the scopes were last counted from.
    drawn: u64,
    scoped: Option<(u64, std::time::Instant)>,
    scopes: Option<Arc<tidycuts_engine::scopes::Scopes>>,
}

impl Engine {
    pub fn new(
        rs: &RenderState,
        ctx: &egui::Context,
        cache_dir: std::path::PathBuf,
        hwaccel: bool,
    ) -> Engine {
        let renderer = Renderer::new(rs.device.clone(), rs.queue.clone());
        let compositor = Compositor::new(renderer.clone());
        let repaint: Arc<dyn Fn() + Send + Sync> = {
            let ctx = ctx.clone();
            Arc::new(move || ctx.request_repaint())
        };
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        tidycuts_engine::voice_clean::set_cache_dir(cache_dir.clone());
        let scrub = ScrubCache::new(cache_dir, (cores / 3).clamp(1, 4), repaint.clone());
        let frames = FrameServer::new(
            scrub.clone(),
            768 << 20,
            (cores / 4).clamp(2, 4),
            repaint.clone(),
        );
        let statics = Arc::new(Statics::default());
        let analysis = Analysis::new(repaint.clone());
        let grades = {
            let a = analysis.clone();
            Arc::new(move |m: &MediaItem| a.auto_grade(m))
                as Arc<dyn Fn(&MediaItem) -> Option<Grade> + Send + Sync>
        };
        let playback = Playback::new(
            Arc::new(Project::default()),
            statics.clone(),
            analysis.loudness.clone(),
            grades,
            analysis.levels.clone(),
            repaint,
        );
        playback.set_hardware_decoding(hwaccel);
        let adapter = rs.adapter.get_info();
        tidycuts_render::gpu::note_adapter(&adapter);
        Engine {
            rs: rs.clone(),
            renderer,
            compositor,
            target: None,
            texture: None,
            scrub,
            frames,
            statics,
            analysis,
            playback,
            playback_revision: u64::MAX,
            previews: Vec::new(),
            playing_timeline: false,
            shown: None,
            incomplete: false,
            hwaccel,
            adapter: format!("{} ({:?})", adapter.name, adapter.backend),
            drawn: 0,
            scoped: None,
            scopes: None,
        }
    }

    pub fn set_hardware_decoding(&mut self, on: bool) {
        self.hwaccel = on;
        self.playback.set_hardware_decoding(on);
    }

    /// Makes sure the preview texture is `size` pixels.
    fn ensure_target(&mut self, size: (u32, u32)) {
        if self
            .target
            .as_ref()
            .is_some_and(|t| (t.width, t.height) == size)
        {
            return;
        }
        let target =
            self.compositor
                .make_target(size.0, size.1, wgpu::TextureFormat::Rgba8UnormSrgb, true);
        let mut r = self.rs.renderer.write();
        match self.texture {
            Some(id) => r.update_egui_texture_from_wgpu_texture(
                &self.rs.device,
                &target.display_view,
                wgpu::FilterMode::Linear,
                id,
            ),
            None => {
                self.texture = Some(r.register_native_texture(
                    &self.rs.device,
                    &target.display_view,
                    wgpu::FilterMode::Linear,
                ));
            }
        }
        drop(r);
        self.target = Some(target);
        self.shown = None;
    }

    /// The scopes of the picture in the preview. Counted again only when a
    /// new picture was drawn, and at most every 60 ms while playing.
    pub fn scopes(&mut self) -> Option<Arc<tidycuts_engine::scopes::Scopes>> {
        let target = self.target.as_ref()?;
        let fresh = match self.scoped {
            Some((n, at)) => {
                n != self.drawn
                    && (!self.playback.is_playing()
                        || at.elapsed() > std::time::Duration::from_millis(60))
            }
            None => true,
        };
        if fresh {
            let px = self.compositor.read_target(target);
            self.scopes = Some(Arc::new(tidycuts_engine::scopes::analyse(
                &px,
                target.width as usize,
                target.height as usize,
            )));
            self.scoped = Some((self.drawn, std::time::Instant::now()));
        }
        self.scopes.clone()
    }

    fn draw(&mut self, prepared: &PreparedFrame) {
        self.drawn += 1;
        for (k, img) in &prepared.statics {
            if !self.compositor.has_static(*k) {
                self.compositor.store_static(*k, img);
            }
        }
        if let Some(t) = &self.target {
            self.compositor.render(&prepared.frame, t);
        }
    }

    /// Draws `project` at `at` while paused or scrubbing. Cheap when nothing
    /// changed since the last call.
    pub fn show_still(
        &mut self,
        project: &Project,
        revision: u64,
        source: Option<MediaId>,
        at: Time,
        size: (u32, u32),
        scrubbing: bool,
    ) {
        self.ensure_target(size);
        let key = Shown {
            revision,
            time: at,
            size,
            source,
        };
        if self.shown.as_ref() == Some(&key) && !self.incomplete {
            return;
        }
        let mut provider = PreviewFrames {
            server: &self.frames,
            scrubbing,
            incomplete: false,
            hwaccel: self.hwaccel,
        };
        let grades = self.analysis.clone();
        let auto = move |m: &MediaItem| grades.auto_grade(m);
        let mut planner = Planner {
            project,
            out_scale: size.0 as f32 / project.settings.width.max(1) as f32,
            frames: &mut provider,
            statics: &self.statics,
            auto_grade: &auto,
            levels: &self.analysis.levels,
            show_empty_cells: true,
            used_statics: Vec::new(),
        };
        let prepared = planner.build(at);
        self.incomplete = provider.incomplete;
        self.draw(&prepared);
        self.shown = Some(key);
    }

    /// The saved previews to play where they cover the timeline.
    pub fn set_previews(&mut self, previews: Vec<tidycuts_engine::previews::Cached>) {
        self.previews = previews;
        if self.playing_timeline {
            self.playback.set_previews(self.previews.clone());
        }
    }

    /// Starts playing `project` from `at`. `timeline` is false when it is a
    /// single source clip: saved previews are about the timeline only.
    pub fn play(
        &mut self,
        project: &Project,
        revision: u64,
        at: Time,
        size: (u32, u32),
        timeline: bool,
    ) {
        self.playing_timeline = timeline;
        self.playback.set_previews(if timeline {
            self.previews.clone()
        } else {
            Vec::new()
        });
        self.sync_playback(project, revision);
        self.ensure_target(size);
        self.playback.set_output_size(size.0, size.1);
        self.playback.play(at);
    }

    /// Gives the playback threads the latest project after an edit.
    pub fn sync_playback(&mut self, project: &Project, revision: u64) {
        if self.playback_revision != revision {
            self.playback.set_project(Arc::new(project.clone()));
            self.playback_revision = revision;
        }
    }

    /// Shows the next due frame while playing. Returns false once playback
    /// has stopped by itself (end of the timeline).
    pub fn tick_playing(&mut self, size: (u32, u32)) -> bool {
        if !self.playback.is_playing() {
            return false;
        }
        self.playback.set_output_size(size.0, size.1);
        if let Some(f) = self.playback.poll() {
            // The compositor scales to the target, so frames planned before
            // a resize still fill it.
            self.ensure_target(size);
            self.draw(&f);
            self.shown = None;
        }
        true
    }

    pub fn is_playing(&self) -> bool {
        self.playback.is_playing()
    }

    pub fn position(&self) -> Time {
        self.playback.position()
    }

    pub fn pause(&mut self) -> Time {
        self.shown = None;
        self.playback.pause()
    }

    /// Forces the next still to be drawn again (after settings changes).
    pub fn invalidate(&mut self) {
        self.shown = None;
    }
}

/// A one-clip project for previewing a library item on its own.
pub fn source_project(media: &MediaItem, base: &ProjectSettings) -> Project {
    let mut p = Project::new("Source");
    let mut s = base.clone();
    if media.kind != MediaKind::Audio && media.width > 0 && media.height > 0 {
        s.width = media.width & !1;
        s.height = media.height & !1;
    }
    if let Some(fps) = media.frame_rate {
        s.frame_rate = fps;
    }
    s.magnetic = false;
    p.settings = s;
    p.add_media(media.clone());
    let clip = Clip::new(media, Time::ZERO);
    let kind = media.kind;
    let track = p.default_track_for(kind);
    if let Some(t) = track {
        if let Some(tr) = p.track_mut(t) {
            tr.clips.push(clip);
        }
    }
    p
}

/// Fits a picture of the project's aspect inside `avail` pixels, never
/// bigger than the project itself.
pub fn fit_size(project: &ProjectSettings, avail: (f32, f32), factor: f32) -> (u32, u32) {
    let (pw, ph) = (project.width.max(2) as f32, project.height.max(2) as f32);
    let s = (avail.0 / pw).min(avail.1 / ph).min(1.0) * factor;
    let w = ((pw * s).round() as u32).max(16) & !1;
    let h = ((ph * s).round() as u32).max(16) & !1;
    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn fit_keeps_aspect_and_caps_at_project_size() {
        let s = ProjectSettings::default();
        assert_eq!(fit_size(&s, (960.0, 1000.0), 1.0), (960, 540));
        assert_eq!(fit_size(&s, (4000.0, 4000.0), 1.0), (1920, 1080));
        assert_eq!(fit_size(&s, (960.0, 540.0), 0.5), (480, 270));
    }

    #[test]
    fn source_project_uses_the_media_shape() {
        let mut m = MediaItem::new(
            PathBuf::from("/v.mp4"),
            MediaKind::Video,
            Time::from_secs_f64(3.0),
        );
        m.width = 1080;
        m.height = 1920;
        let p = source_project(&m, &ProjectSettings::default());
        assert_eq!((p.settings.width, p.settings.height), (1080, 1920));
        assert_eq!(p.duration(), Time::from_secs_f64(3.0));
    }
}
