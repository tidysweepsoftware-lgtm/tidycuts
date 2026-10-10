//! Turns the project at one moment into a [`Frame`] for the compositor:
//! which clips show, in what order, with every setting, keyframe,
//! animation, transition and effect resolved to plain numbers.

use crate::frames::{decode_size, FrameProvider};
use crate::levels::AudioLevels;
use crate::statics::Statics;
use std::sync::Arc;
use tidycuts_core::{
    BackgroundFill, Cell, Clip, ColorAdjust, Generator, MediaItem, MediaKind, Project, Time,
    TrackKind, TrackMatte, Upscale,
};
use tidycuts_render::catalog::animations::{self, Motion, Phase};
use tidycuts_render::catalog::{effects, filters, stickers, transitions};
use tidycuts_render::{
    AdjustItem, CellParams, EnhanceParams, Frame, Fx, Grade, Item, Layer, MaskParams, MatteItem,
    RgbaImage, ShadowParams, Source, TransitionItem, VisualizerParams,
};

/// A frame plus the self-drawn pictures it uses, which the renderer uploads
/// before drawing if the GPU doesn't have them yet.
pub struct PreparedFrame {
    pub time: Time,
    pub frame: Frame,
    pub statics: Vec<(u64, Arc<RgbaImage>)>,
}

/// Everything the planner needs besides the project.
pub struct Planner<'a> {
    pub project: &'a Project,
    /// Output pixels per project pixel (preview size / project size).
    pub out_scale: f32,
    pub frames: &'a mut dyn FrameProvider,
    pub statics: &'a Statics,
    /// Automatic levels for a media file (Video > Auto Enhance), if known.
    pub auto_grade: &'a dyn Fn(&MediaItem) -> Option<Grade>,
    /// Sound levels for audio visualizers.
    pub levels: &'a AudioLevels,
    /// Draw a panel for empty split-screen cells (on screen, so there is
    /// something to drop media on); exports leave them out.
    pub show_empty_cells: bool,
    /// Pictures made while building the frame, so the caller can keep them.
    pub used_statics: Vec<u64>,
}

fn f(v: f64) -> f32 {
    v as f32
}

/// A split-screen cell's outline and picture area in project pixels.
fn cell_params(cell: &Cell, canvas: [f32; 2]) -> CellParams {
    let shape = cell.shape(f64::from(canvas[0]), f64::from(canvas[1]));
    let q = |p: [[f64; 2]; 4]| p.map(|v| [v[0] as f32, v[1] as f32]);
    CellParams {
        outer: q(shape.outer),
        inner: q(shape.inner),
        radius: shape.radius as f32,
        border: cell.border_color,
    }
}

/// Grade from the Color tab sliders (-100..100, hue in degrees).
pub fn color_grade(c: &ColorAdjust, lt: Time) -> Option<Grade> {
    let v = |a: &tidycuts_core::Animatable| f(a.value_at(lt));
    let g = Grade {
        exposure: v(&c.exposure) / 100.0,
        brightness: v(&c.brightness) / 100.0,
        contrast: v(&c.contrast) / 100.0,
        highlights: v(&c.highlights) / 100.0,
        shadows: v(&c.shadows) / 100.0,
        whites: v(&c.whites) / 100.0,
        blacks: v(&c.blacks) / 100.0,
        saturation: v(&c.saturation) / 100.0,
        vibrance: v(&c.vibrance) / 100.0,
        temperature: v(&c.temperature) / 100.0,
        tint: v(&c.tint) / 100.0,
        hue: v(&c.hue),
        vignette: v(&c.vignette) / 100.0,
        looks: tidycuts_render::looks::for_color(c, lt),
        ..Default::default()
    };
    (!g.is_identity()).then_some(g)
}

fn rotate(v: [f32; 2], deg: f32) -> [f32; 2] {
    let (s, c) = deg.to_radians().sin_cos();
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
}

/// Where a clip's picture sits, before effects.
struct Placement {
    center: [f32; 2],
    base: [f32; 2],
    crop: [f32; 4],
    scale: [f32; 2],
    rotation: f32,
    opacity: f32,
}

impl Planner<'_> {
    fn canvas(&self) -> [f32; 2] {
        [
            self.project.settings.width as f32,
            self.project.settings.height as f32,
        ]
    }

    /// Size factor for content designed on a 1080p frame (title sizes,
    /// sticker sizes, motion distances).
    fn k(&self) -> f32 {
        let c = self.canvas();
        c[0].min(c[1]) / 1080.0
    }

    fn fps(&self) -> f64 {
        self.project.settings.frame_rate.as_f64().max(1.0)
    }

    fn target_size(&self) -> (u32, u32) {
        let c = self.canvas();
        (
            (c[0] * self.out_scale).round().max(2.0) as u32,
            (c[1] * self.out_scale).round().max(2.0) as u32,
        )
    }

    /// Builds the frame at `t`.
    pub fn build(&mut self, t: Time) -> PreparedFrame {
        self.used_statics.clear();
        let project = self.project;
        let s = &project.settings;
        let bg = s.background;
        // Bottom video track first.
        let video: Vec<usize> = (0..project.tracks.len())
            .rev()
            .filter(|i| project.tracks[*i].kind == TrackKind::Video)
            .collect();
        let mut per_track: Vec<(Option<&Clip>, Vec<Item>)> = Vec::new();
        for &ti in &video {
            let track = &project.tracks[ti];
            if track.hidden {
                per_track.push((None, Vec::new()));
                continue;
            }
            per_track.push(self.track_items(&track.clips, t));
        }
        let mut items = Vec::new();
        let mut skip_next = false;
        for i in 0..per_track.len() {
            if skip_next {
                skip_next = false;
                continue;
            }
            let (clip, track_items) = std::mem::take(&mut per_track[i]);
            let matte_mode = clip
                .map(|c| c.video.track_matte)
                .unwrap_or(TrackMatte::None);
            if matte_mode != TrackMatte::None && i + 1 < per_track.len() {
                let above = std::mem::take(&mut per_track[i + 1].1);
                let matte = above.into_iter().find_map(|it| match it {
                    Item::Layer(l) => Some(l),
                    _ => None,
                });
                let mut layers: Vec<Layer> = Vec::new();
                for it in track_items {
                    match it {
                        Item::Layer(l) => layers.push(l),
                        other => items.push(other),
                    }
                }
                if let (Some(matte), Some(layer)) = (matte, layers.pop()) {
                    for l in layers {
                        items.push(Item::Layer(l));
                    }
                    let mode = match matte_mode {
                        TrackMatte::Alpha => 1,
                        TrackMatte::AlphaInverted => 2,
                        TrackMatte::Luma => 3,
                        _ => 4,
                    };
                    items.push(Item::Matte(MatteItem { layer, matte, mode }));
                } else {
                    items.extend(layers.into_iter().map(Item::Layer));
                }
                skip_next = true;
                continue;
            }
            items.extend(track_items);
        }
        let statics = self
            .used_statics
            .iter()
            .filter_map(|k| self.statics.get(*k).map(|img| (*k, img)))
            .collect();
        PreparedFrame {
            time: t,
            frame: Frame {
                canvas: self.canvas(),
                background: [bg[0], bg[1], bg[2], 1.0],
                items,
            },
            statics,
        }
    }

    /// What one track shows at `t`, and the clip responsible.
    fn track_items<'c>(&mut self, clips: &'c [Clip], t: Time) -> (Option<&'c Clip>, Vec<Item>) {
        let visible = |c: &&Clip| !c.disabled;
        // A transition centred on a cut between two touching clips.
        for (i, b) in clips.iter().enumerate().filter(|(_, c)| visible(c)) {
            let Some(tr) = &b.transition_in else {
                continue;
            };
            let prev = i
                .checked_sub(1)
                .map(|j| &clips[j])
                .filter(|a| !a.disabled && (a.end() - b.start).abs() < Time::from_millis(2));
            let Some(a) = prev else {
                continue;
            };
            let d = tr.duration.min(a.duration()).min(b.duration());
            let half = Time(d.0 / 2);
            if t >= b.start - half && t < b.start + half && d > Time::ZERO {
                let progress = (t - (b.start - half)).as_secs_f64() / d.as_secs_f64();
                let item = self.transition(tr, Some(a), Some(b), t, progress as f32);
                return (Some(b), vec![item]);
            }
        }
        let Some(c) = clips.iter().filter(visible).find(|c| c.contains(t)) else {
            return (None, Vec::new());
        };
        // Transitions from or to nothing, inside the clip.
        let lt = c.local(t);
        if let Some(tr) = &c.transition_in {
            let prev_touching = clips.iter().any(|a| {
                a.id != c.id && !a.disabled && (a.end() - c.start).abs() < Time::from_millis(2)
            });
            let d = tr.duration.min(c.duration());
            if !prev_touching && lt < d && d > Time::ZERO {
                let p = lt.as_secs_f64() / d.as_secs_f64();
                return (
                    Some(c),
                    vec![self.transition(tr, None, Some(c), t, p as f32)],
                );
            }
        }
        if let Some(tr) = &c.transition_out {
            let d = tr.duration.min(c.duration());
            let left = c.end() - t;
            if left < d && d > Time::ZERO {
                let p = 1.0 - left.as_secs_f64() / d.as_secs_f64();
                return (
                    Some(c),
                    vec![self.transition(tr, Some(c), None, t, p as f32)],
                );
            }
        }
        if c.is_adjustment() {
            return (Some(c), self.adjustment(c, t));
        }
        (Some(c), self.clip_layers(c, t))
    }

    fn transition(
        &mut self,
        tr: &tidycuts_core::Transition,
        from: Option<&Clip>,
        to: Option<&Clip>,
        t: Time,
        progress: f32,
    ) -> Item {
        let def = transitions::get(&tr.kind).or_else(|| transitions::get("dissolve"));
        let mut params = [[0.0f32; 4]; 4];
        let mut color = [0.0, 0.0, 0.0, 1.0];
        let mut kind = 1;
        if let Some(d) = def {
            params[0] = d.params;
            color = d.color;
            kind = d.shader;
        }
        for (i, key) in ["p0", "p1", "p2", "p3"].iter().enumerate() {
            if let Some(v) = tr.params.get(*key) {
                params[0][i] = *v as f32;
            }
        }
        if let (Some(r), Some(g), Some(b)) = (
            tr.params.get("color_r"),
            tr.params.get("color_g"),
            tr.params.get("color_b"),
        ) {
            color = [*r as f32, *g as f32, *b as f32, 1.0];
        }
        // Each side's own background fill or shadow would double up; keep
        // just the picture.
        let mut side = |c: Option<&Clip>| -> Option<Layer> {
            let c = c?;
            if c.is_adjustment() {
                return None;
            }
            self.clip_layers(c, t)
                .into_iter()
                .rev()
                .find_map(|i| match i {
                    Item::Layer(l) => Some(l),
                    _ => None,
                })
        };
        let from = side(from);
        let to = side(to);
        Item::Transition(TransitionItem {
            from,
            to,
            kind,
            progress: progress.clamp(0.0, 1.0),
            params,
            color,
            time: t.as_secs_f64() as f32,
        })
    }

    fn adjustment(&mut self, c: &Clip, t: Time) -> Vec<Item> {
        let lt = c.local(t).max(Time::ZERO).min(c.duration());
        let opacity = f(c.transform.opacity.value_at(lt) / 100.0).clamp(0.0, 1.0);
        let time = lt.as_secs_f64() as f32;
        let (grades, fx, _motion) = self.effects_of(c, lt);
        let mut out = vec![Item::Adjust(AdjustItem {
            grade: color_grade(&c.color, lt),
            effects: fx,
            opacity,
            time,
        })];
        for g in grades {
            out.push(Item::Adjust(AdjustItem {
                grade: Some(g),
                effects: Vec::new(),
                opacity,
                time,
            }));
        }
        out
    }

    /// Filters (as grades), shader effects and the motion of motion effects.
    fn effects_of(&self, c: &Clip, lt: Time) -> (Vec<Grade>, Vec<Fx>, Motion) {
        let mut grades = Vec::new();
        let mut fx = Vec::new();
        let mut motion = Motion::NONE;
        let time = lt.as_secs_f64() as f32;
        let k = self.k();
        for e in c.effects.iter().filter(|e| e.enabled) {
            if filters::is_filter(&e.kind) {
                if let Some(def) = filters::get(&e.kind) {
                    let mut g = (def.grade)();
                    let strength = e
                        .params
                        .get("strength")
                        .map(|a| f(a.value_at(lt)))
                        .unwrap_or(filters::STRENGTH.default);
                    g.strength *= strength.clamp(0.0, 1.0);
                    grades.push(g);
                }
                continue;
            }
            let Some(def) = effects::get(&e.kind) else {
                continue;
            };
            let value = |key: &str| -> f32 {
                e.params
                    .get(key)
                    .map(|a| f(a.value_at(lt)))
                    .unwrap_or_else(|| effects::default_value(def, key))
            };
            match def.kind {
                effects::EffectKind::Shader { id, passes } => {
                    let (params, colors) = effects::pack(def, value);
                    fx.push(Fx {
                        shader: id,
                        passes,
                        params,
                        colors,
                        time,
                    });
                }
                effects::EffectKind::Motion => {
                    let mut m = effects::motion(def.id, time, value);
                    m.dx *= k;
                    m.dy *= k;
                    motion = motion.then(m);
                }
            }
        }
        (grades, fx, motion)
    }

    /// Motion from the clip's in/out animations at local time `lt`.
    fn animation_motion(&self, c: &Clip, lt: Time) -> (Motion, f32) {
        let canvas = self.canvas();
        let mut m = Motion::NONE;
        let mut reveal = 1.0;
        let len = c.duration();
        if let Some(a) = &c.anim_in {
            let d = a.duration.min(len);
            if lt < d && d > Time::ZERO {
                let p = (lt.as_secs_f64() / d.as_secs_f64()) as f32;
                m = m.then(animations::evaluate(&a.kind, Phase::In, p, canvas));
                reveal = p;
            }
        }
        if let Some(a) = &c.anim_out {
            let d = a.duration.min(len);
            let start = len - d;
            if lt >= start && d > Time::ZERO {
                let p = ((lt - start).as_secs_f64() / d.as_secs_f64()) as f32;
                m = m.then(animations::evaluate(&a.kind, Phase::Out, p, canvas));
            }
        }
        (m, reveal)
    }

    fn placement(&self, c: &Clip, lt: Time, base: [f32; 2], motion: &Motion) -> Placement {
        let canvas = self.canvas();
        let tr = &c.transform;
        let v = |a: &tidycuts_core::Animatable| f(a.value_at(lt));
        let crop = c.crop.clamped();
        // The clip's own crop, then the motion's (wipes) inside it.
        let (l, t, r, b) = (f(crop.left), f(crop.top), f(crop.right), f(crop.bottom));
        let (w0, h0) = ((1.0 - l - r).max(0.001), (1.0 - t - b).max(0.001));
        let mc = motion.crop;
        let u0 = l + w0 * mc[0];
        let u1 = l + w0 * (1.0 - mc[2]);
        let v0 = t + h0 * mc[1];
        let v1 = t + h0 * (1.0 - mc[3]);
        let crop_uv = [u0, v0, u1.max(u0 + 0.0001), v1.max(v0 + 0.0001)];
        let visible = [
            base[0] * (crop_uv[2] - crop_uv[0]),
            base[1] * (crop_uv[3] - crop_uv[1]),
        ];
        let offset = [
            ((u0 + u1) * 0.5 - 0.5) * base[0],
            ((v0 + v1) * 0.5 - 0.5) * base[1],
        ];
        let scale = [
            v(&tr.scale_x) / 100.0 * motion.scale * motion.stretch[0],
            v(&tr.scale_y) / 100.0 * motion.scale * motion.stretch[1],
        ];
        let rotation = v(&tr.rotation) + motion.rotation;
        let anchor = [f(c.video.anchor[0]), f(c.video.anchor[1])];
        let pos = [
            canvas[0] * 0.5 + v(&tr.position_x) + motion.dx,
            canvas[1] * 0.5 + v(&tr.position_y) + motion.dy,
        ];
        let rel = [
            (offset[0] - anchor[0]) * scale[0],
            (offset[1] - anchor[1]) * scale[1],
        ];
        let rr = rotate(rel, rotation);
        Placement {
            center: [pos[0] + anchor[0] + rr[0], pos[1] + anchor[1] + rr[1]],
            base: visible,
            crop: crop_uv,
            scale,
            rotation,
            opacity: (v(&tr.opacity) / 100.0 * motion.opacity).clamp(0.0, 1.0),
        }
    }

    /// The picture of a visual clip, plus its background fill if any.
    fn clip_layers(&mut self, c: &Clip, t: Time) -> Vec<Item> {
        let project = self.project;
        let canvas = self.canvas();
        let k = self.k();
        let lt = c.local(t).max(Time::ZERO).min(c.duration());
        let time = lt.as_secs_f64() as f32;
        let (anim, reveal) = self.animation_motion(c, lt);
        let (grades, mut fx, fx_motion) = self.effects_of(c, lt);
        let mut motion = anim.then(fx_motion);

        let media = c.media_id.and_then(|m| project.media(m));
        let (source, base, decoded) = match (&c.generator, media) {
            (Some(Generator::Title(spec)), _) => {
                let sx = f(c.transform.scale_x.value_at(lt))
                    .abs()
                    .max(f(c.transform.scale_y.value_at(lt)).abs())
                    / 100.0;
                let q = ((sx * motion.scale).max(0.1) * 4.0).ceil() / 4.0;
                let raster_scale = (k * self.out_scale * q.min(4.0)).max(0.05);
                let chars = spec.text.chars().count();
                let visible = (motion.reveal < 0.999)
                    .then(|| (motion.reveal * chars as f32).round() as usize);
                let (key, size) =
                    self.statics
                        .title(spec, raster_scale, reveal, visible, motion.letters);
                self.used_statics.push(key);
                (Source::Cached(key), [size[0] * k, size[1] * k], None)
            }
            (Some(Generator::Sticker(spec)), _) => {
                let def = stickers::get(&spec.kind);
                let aspect = def.map(|d| d.aspect).unwrap_or(1.0);
                let h = stickers::STICKER_HEIGHT * k;
                let sy = f(c.transform.scale_y.value_at(lt))
                    .abs()
                    .max(f(c.transform.scale_x.value_at(lt)).abs())
                    / 100.0;
                let px = (h * self.out_scale * sy.max(0.05)).max(8.0);
                // Round up to steps so resizing doesn't redraw every frame.
                let px = ((px / 16.0).ceil() * 16.0).min(4096.0) as u32;
                let loop_motion = if spec.motion.is_empty() {
                    def.map(|d| d.motion).unwrap_or("none")
                } else {
                    spec.motion.as_str()
                };
                motion = motion.then(animations::sticker_motion(loop_motion, time, h));
                let key = self.statics.sticker(spec, px, time);
                self.used_statics.push(key);
                (Source::Cached(key), [h * aspect, h], None)
            }
            (Some(Generator::Solid(color)), _) => (Source::Solid(*color), canvas, None),
            (Some(Generator::Gradient { a, b, angle }), _) => {
                (Source::Gradient(*a, *b, *angle), canvas, None)
            }
            (Some(Generator::Placeholder), _) => {
                // An empty split-screen cell: a dark panel to drop media on,
                // but only where someone is editing; an export has none.
                if !self.show_empty_cells {
                    return Vec::new();
                }
                (Source::Solid([0.13, 0.15, 0.19, 1.0]), canvas, None)
            }
            (Some(Generator::Visualizer(spec)), _) => {
                let size = spec.base_size();
                let source = Source::Visualizer(VisualizerParams {
                    style: spec.style_index(),
                    levels: self.levels.bars(project, c, spec, t),
                    colors: [spec.color, spec.color2],
                    mirror: spec.mirror,
                    thickness: f(spec.thickness / 100.0).clamp(0.05, 1.0),
                });
                (source, [f(size[0]) * k, f(size[1]) * k], None)
            }
            (Some(Generator::Adjustment), _) => return Vec::new(),
            (None, Some(m)) if m.kind != MediaKind::Audio => {
                let fit =
                    (canvas[0] / m.width.max(1) as f32).min(canvas[1] / m.height.max(1) as f32);
                let base = [m.width.max(1) as f32 * fit, m.height.max(1) as f32 * fit];
                let (tw, th) = self.target_size();
                let sc = f(c.transform.scale_x.value_at(lt))
                    .abs()
                    .max(f(c.transform.scale_y.value_at(lt)).abs())
                    / 100.0;
                let boost = if sc > 1.25 { 2 } else { 1 };
                let size = decode_size(m, (tw * boost, th * boost));
                let src_t = c.source_time(t).as_secs_f64().max(0.0);
                match self.frames.frame(c, m, src_t, size) {
                    Some(p) => (Source::Pixels(p), base, Some((m, size))),
                    None => return Vec::new(),
                }
            }
            _ => return Vec::new(),
        };

        // Split-screen: the picture covers its cell's box instead of
        // fitting the frame, and is placed relative to the box's centre.
        let cell_box = c.video.cell.as_ref().map(|cell| {
            let (centre, size) = cell.frame_box(f64::from(canvas[0]), f64::from(canvas[1]));
            ([f(centre[0]), f(centre[1])], [f(size[0]), f(size[1])], cell)
        });
        let base = match (&cell_box, media, &c.generator) {
            (Some((_, _, cell)), Some(m), None) if m.kind != MediaKind::Audio => {
                let s = cell.cover(
                    f64::from(canvas[0]),
                    f64::from(canvas[1]),
                    f64::from(m.width.max(1)),
                    f64::from(m.height.max(1)),
                );
                [f(s[0]), f(s[1])]
            }
            (
                Some((_, size, _)),
                _,
                Some(Generator::Solid(_) | Generator::Gradient { .. } | Generator::Placeholder),
            ) => *size,
            _ => base,
        };

        // Lens correction runs before the clip's other effects.
        if c.video.lens_correction != 0.0 {
            if let Some(def) = effects::get("distort.lens") {
                if let effects::EffectKind::Shader { id, passes } = def.kind {
                    let amount = -f(c.video.lens_correction) / 100.0;
                    let (params, colors) = effects::pack(def, |key| {
                        if key == "amount" {
                            amount
                        } else {
                            effects::default_value(def, key)
                        }
                    });
                    fx.insert(
                        0,
                        Fx {
                            shader: id,
                            passes,
                            params,
                            colors,
                            time,
                        },
                    );
                }
            }
        }
        if motion.blur > 0.001 {
            fx.push(Fx {
                shader: 1,
                passes: 2,
                params: [[motion.blur, 0.0, 0.0, 0.0], [0.0; 4], [0.0; 4], [0.0; 4]],
                colors: [[0.0; 4]; 2],
                time,
            });
        }

        let mut place = self.placement(c, lt, base, &motion);
        if let Some((centre, _, _)) = &cell_box {
            place.center[0] += centre[0];
            place.center[1] += centre[1];
        }
        // Motion blur: blur along how far the clip moves in half a frame.
        if c.video.motion_blur > 0.0 {
            let dt = Time::from_secs_f64(0.5 / self.fps());
            let before_lt = if lt >= dt { lt - dt } else { Time::ZERO };
            let (anim_b, _) = self.animation_motion(c, before_lt);
            let (_, _, fxm_b) = self.effects_of(c, before_lt);
            let prev = self.placement(c, before_lt, base, &anim_b.then(fxm_b));
            let d = [
                place.center[0] - prev.center[0],
                place.center[1] - prev.center[1],
            ];
            let dist = (d[0] * d[0] + d[1] * d[1]).sqrt() * f(c.video.motion_blur) / 100.0 * 2.0;
            if dist > 1.0 {
                let ext_y = (place.base[1] * place.scale[1].abs()).max(1.0);
                let ang = d[1].atan2(d[0]).to_degrees() - place.rotation;
                fx.push(Fx {
                    shader: 4,
                    passes: 1,
                    params: [
                        [(dist / ext_y / 0.08).min(4.0), ang, 0.0, 0.0],
                        [0.0; 4],
                        [0.0; 4],
                        [0.0; 4],
                    ],
                    colors: [[0.0; 4]; 2],
                    time,
                });
            }
        }

        let mut grade_list = Vec::new();
        if let Some(m) = media {
            if c.video.auto_enhance > 0.0 {
                if let Some(mut g) = (self.auto_grade)(m) {
                    g.strength *= f(c.video.auto_enhance / 100.0).clamp(0.0, 1.0);
                    grade_list.push(g);
                }
            }
        }
        grade_list.extend(grades);
        let e = &c.enhance;
        let block_size = decoded
            .map(|(m, size)| 8.0 * size.0 as f32 / m.width.max(1) as f32)
            .unwrap_or(8.0);
        let mask = c.video.mask.as_ref().map(|m| {
            let v = |a: &tidycuts_core::Animatable| f(a.value_at(lt));
            MaskParams {
                shape: m.shape.index(),
                center: [v(&m.x), v(&m.y)],
                size: [v(&m.width), v(&m.height)],
                rotation: v(&m.rotation),
                roundness: v(&m.roundness) / 100.0,
                feather: v(&m.feather) / 100.0 * 0.25,
                invert: m.invert,
            }
        });
        let sh = &c.video.shadow;
        let shadow = sh.enabled.then(|| {
            let a = f(sh.angle).to_radians();
            ShadowParams {
                color: [
                    sh.color[0],
                    sh.color[1],
                    sh.color[2],
                    f(sh.opacity / 100.0).clamp(0.0, 1.0),
                ],
                offset: [-a.cos() * f(sh.distance) * k, a.sin() * f(sh.distance) * k],
                blur: f(sh.blur) / 100.0 * 60.0 * k,
            }
        });
        let layer = Layer {
            source: source.clone(),
            base_size: place.base,
            crop: place.crop,
            center: place.center,
            scale: place.scale,
            rotation: place.rotation,
            flip: [c.transform.flip_horizontal, c.transform.flip_vertical],
            opacity: place.opacity,
            blend: c.blend.index(),
            corner_radius: f(c.video.corner_radius) * k,
            enhance: EnhanceParams {
                denoise: f(e.denoise / 100.0),
                deblock: f(e.deblock / 100.0),
                sharpen: f(e.sharpen / 100.0),
                upscale: match e.upscale {
                    Upscale::Standard => 0,
                    Upscale::Sharp => 1,
                    Upscale::Enhanced => 2,
                },
                block_size,
            },
            grade: color_grade(&c.color, lt),
            filters: grade_list,
            effects: fx,
            mask,
            shadow,
            time,
            cell: c.video.cell.as_ref().map(|cell| cell_params(cell, canvas)),
        };

        let mut out = Vec::new();
        let bgf = &c.video.background;
        match bgf.fill {
            BackgroundFill::None => {}
            BackgroundFill::Color => {
                let mut l = Layer::full(Source::Solid(bgf.color), canvas);
                l.opacity = place.opacity;
                out.push(Item::Layer(l));
            }
            BackgroundFill::Blur => {
                // An enlarged, blurred copy of the clip filling the frame.
                let cover = (canvas[0] / base[0].max(1.0)).max(canvas[1] / base[1].max(1.0));
                let mut l = Layer::full(source, [base[0] * cover, base[1] * cover]);
                l.center = [canvas[0] * 0.5, canvas[1] * 0.5];
                l.opacity = place.opacity;
                l.time = time;
                l.grade = Some(Grade {
                    exposure: -0.15,
                    ..Default::default()
                });
                l.effects.push(Fx {
                    shader: 1,
                    passes: 2,
                    params: [
                        [0.2 + f(bgf.blur / 100.0) * 1.2, 0.0, 0.0, 0.0],
                        [0.0; 4],
                        [0.0; 4],
                        [0.0; 4],
                    ],
                    colors: [[0.0; 4]; 2],
                    time,
                });
                out.push(Item::Layer(l));
            }
        }
        out.push(Item::Layer(layer));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tidycuts_core::{Animatable, Transition};
    use tidycuts_render::Pixels;

    /// Hands out a tiny gray picture for any request and records the
    /// source times asked for.
    struct Fake(Vec<f64>);

    impl FrameProvider for Fake {
        fn frame(&mut self, _c: &Clip, _m: &MediaItem, s: f64, size: (u32, u32)) -> Option<Pixels> {
            self.0.push(s);
            Some(Pixels::Rgba(Arc::new(RgbaImage::new(
                size.0.max(2),
                size.1.max(2),
            ))))
        }
    }

    fn secs(x: f64) -> Time {
        Time::from_secs_f64(x)
    }

    fn project() -> (Project, tidycuts_core::MediaId) {
        let mut p = Project::new("t");
        let mut m = MediaItem::new(PathBuf::from("/v.mp4"), MediaKind::Video, secs(10.0));
        m.width = 1280;
        m.height = 720;
        let id = p.add_media(m);
        (p, id)
    }

    fn plan(p: &Project, t: f64) -> (PreparedFrame, Vec<f64>) {
        plan_with(p, t, true)
    }

    fn plan_with(p: &Project, t: f64, show_empty_cells: bool) -> (PreparedFrame, Vec<f64>) {
        let statics = Statics::default();
        let mut fake = Fake(Vec::new());
        let mut pl = Planner {
            project: p,
            out_scale: 0.5,
            frames: &mut fake,
            statics: &statics,
            auto_grade: &|_| None,
            levels: &AudioLevels::default(),
            show_empty_cells,
            used_statics: Vec::new(),
        };
        let f = pl.build(secs(t));
        (f, fake.0)
    }

    #[test]
    fn a_clip_fills_the_frame_and_maps_time() {
        let (mut p, m) = project();
        let c = p.append_clip(m).unwrap();
        p.clip_mut(c).unwrap().source_in = secs(2.0);
        let (f, asked) = plan(&p, 1.0);
        assert_eq!(f.frame.items.len(), 1);
        let Item::Layer(l) = &f.frame.items[0] else {
            panic!()
        };
        assert_eq!(l.base_size, [1920.0, 1080.0]);
        assert_eq!(l.center, [960.0, 540.0]);
        assert!((asked[0] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn transitions_are_centred_on_the_cut() {
        let (mut p, m) = project();
        let a = p.append_clip(m).unwrap();
        let b = p.append_clip(m).unwrap();
        let cut = p.clip(b).unwrap().1.start.as_secs_f64();
        p.clip_mut(b).unwrap().transition_in = Some(Transition::new("dissolve", secs(1.0)));
        let (f, _) = plan(&p, cut - 0.25);
        let Item::Transition(tr) = &f.frame.items[0] else {
            panic!("no transition")
        };
        assert!((tr.progress - 0.25).abs() < 1e-3);
        assert!(tr.from.is_some() && tr.to.is_some());
        let (f, _) = plan(&p, cut + 0.6);
        assert!(matches!(f.frame.items[0], Item::Layer(_)));
        let _ = a;
    }

    #[test]
    fn disabled_and_hidden_clips_do_not_show() {
        let (mut p, m) = project();
        let c = p.append_clip(m).unwrap();
        p.clip_mut(c).unwrap().disabled = true;
        assert!(plan(&p, 1.0).0.frame.items.is_empty());
        p.clip_mut(c).unwrap().disabled = false;
        p.tracks.iter_mut().for_each(|t| t.hidden = true);
        assert!(plan(&p, 1.0).0.frame.items.is_empty());
    }

    #[test]
    fn rotation_happens_around_the_anchor() {
        let (mut p, m) = project();
        let c = p.append_clip(m).unwrap();
        {
            let clip = p.clip_mut(c).unwrap();
            clip.video.anchor = [100.0, 0.0];
            clip.transform.rotation = Animatable::constant(180.0);
        }
        let (f, _) = plan(&p, 1.0);
        let Item::Layer(l) = &f.frame.items[0] else {
            panic!()
        };
        // Rotating half a turn around a point 100 px right of centre moves
        // the picture's centre 200 px right.
        assert!((l.center[0] - 1160.0).abs() < 0.01, "{:?}", l.center);
        assert!((l.center[1] - 540.0).abs() < 0.01);
    }

    #[test]
    fn cropping_keeps_the_rest_in_place() {
        let (mut p, m) = project();
        let c = p.append_clip(m).unwrap();
        p.clip_mut(c).unwrap().crop.left = 0.5;
        let (f, _) = plan(&p, 1.0);
        let Item::Layer(l) = &f.frame.items[0] else {
            panic!()
        };
        assert_eq!(l.base_size, [960.0, 1080.0]);
        assert!((l.center[0] - 1440.0).abs() < 0.01);
        assert_eq!(l.crop[0], 0.5);
    }

    #[test]
    fn track_matte_uses_the_track_above() {
        let (mut p, m) = project();
        let c = p.append_clip(m).unwrap();
        p.clip_mut(c).unwrap().video.track_matte = TrackMatte::Luma;
        let v2 = p.add_track(TrackKind::Video).unwrap();
        p.add_generated(Generator::Solid([1.0; 4]), secs(5.0), v2, Time::ZERO)
            .unwrap();
        let (f, _) = plan(&p, 1.0);
        assert_eq!(f.frame.items.len(), 1);
        assert!(matches!(&f.frame.items[0], Item::Matte(m) if m.mode == 3));
    }

    #[test]
    fn reversed_clips_play_backwards() {
        let (mut p, m) = project();
        let c = p.append_clip(m).unwrap();
        p.clip_mut(c).unwrap().reverse = true;
        let (_, asked) = plan(&p, 1.0);
        assert!((asked[0] - 9.0).abs() < 1e-6);
    }

    #[test]
    fn clips_in_a_cell_cover_its_box_and_carry_its_outline() {
        let (mut p, m) = project();
        let v2 = p.add_track(TrackKind::Video).unwrap();
        let c = p.add_clip(m, v2, Time::ZERO).unwrap();
        p.clip_mut(c).unwrap().video.cell = Some(tidycuts_core::Cell::rect(0.5, 0.0, 1.0, 1.0));
        let (f, _) = plan(&p, 1.0);
        let Item::Layer(l) = &f.frame.items[0] else {
            panic!()
        };
        // The 16:9 picture grows to cover the 960x1080 box and sits in it.
        assert!(l.base_size[1] >= 1079.9 && l.base_size[0] >= 960.0);
        assert!((l.center[0] - 1440.0).abs() < 0.01, "{:?}", l.center);
        let cell = l.cell.expect("cell");
        assert_eq!(cell.outer[0], [960.0, 0.0]);
        assert_eq!(cell.outer[2], [1920.0, 1080.0]);
    }

    #[test]
    fn an_export_leaves_empty_cells_out() {
        let (mut p, _) = project();
        let v2 = p.add_track(TrackKind::Video).unwrap();
        let cell = p
            .add_generated(Generator::Placeholder, secs(5.0), v2, Time::ZERO)
            .unwrap();
        p.clip_mut(cell).unwrap().video.cell = Some(tidycuts_core::Cell::rect(0.0, 0.0, 0.5, 0.5));
        assert_eq!(plan_with(&p, 1.0, true).0.frame.items.len(), 1);
        assert_eq!(plan_with(&p, 1.0, false).0.frame.items.len(), 0);
    }

    #[test]
    fn empty_cells_show_a_panel_and_visualizers_get_levels() {
        let (mut p, _) = project();
        let v2 = p.add_track(TrackKind::Video).unwrap();
        let cell = p
            .add_generated(Generator::Placeholder, secs(5.0), v2, Time::ZERO)
            .unwrap();
        p.clip_mut(cell).unwrap().video.cell = Some(tidycuts_core::Cell::rect(0.0, 0.0, 0.5, 0.5));
        let v3 = p.add_track(TrackKind::Video).unwrap();
        let spec = tidycuts_core::VisualizerSpec::default();
        p.add_generated(Generator::Visualizer(spec), secs(5.0), v3, Time::ZERO)
            .unwrap();
        let (f, _) = plan(&p, 1.0);
        assert_eq!(f.frame.items.len(), 2);
        let layers: Vec<&Layer> = f
            .frame
            .items
            .iter()
            .map(|i| match i {
                Item::Layer(l) => l,
                _ => panic!(),
            })
            .collect();
        assert!(matches!(layers[0].source, Source::Solid(_)));
        assert_eq!(layers[0].base_size, [960.0, 540.0]);
        let Source::Visualizer(v) = &layers[1].source else {
            panic!("visualizer source")
        };
        assert_eq!(v.levels.len(), 48);
    }
}
