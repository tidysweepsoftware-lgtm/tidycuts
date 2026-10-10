//! Title rendering: lays out and rasterizes text with ab_glyph outlines
//! drawn by tiny-skia, plus the decorations of each title style.
//!
//! The picture is centred on the text block (the centre of the image is
//! the centre of the text) with room on every side for outlines, shadows,
//! glows and decorations, so a title never jumps or gets clipped while its
//! decorations animate in or its characters type on.

mod layout;
mod raster;
mod styles;

use crate::catalog::animations::LetterKind;
use crate::RgbaImage;
use layout::{Face, Layout, Params};
use styles::{Effects, Fill, Op, Rgba};
use tidycuts_core::TitleSpec;
use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LinearGradient, Mask, Paint, Path, PathBuilder, Pixmap,
    PixmapPaint, Point, Rect, Shader, SpreadMode, Stroke, Transform,
};

/// A rendered title.
pub struct TitleRaster {
    pub image: RgbaImage,
    /// Size in project pixels (the image is `size * scale` pixels).
    pub size: [f32; 2],
}

/// Title decoration styles: (id, name).
pub static STYLES: &[(&str, &str)] = &[
    ("plain", "Plain"),
    ("underline", "Underline"),
    ("highlight", "Highlighter"),
    ("box", "Box"),
    ("outline_box", "Frame"),
    ("pill", "Pill"),
    ("lower_bar", "Bar Lower Third"),
    ("lower_slant", "Slanted Lower Third"),
    ("callout", "Callout"),
    ("brackets", "Brackets"),
    ("double_line", "Double Line"),
    ("neon", "Neon"),
    ("long_shadow", "Long Shadow"),
    ("gradient", "Gradient"),
    ("stroke_only", "Hollow"),
    ("split", "Split"),
    ("glitch", "Glitch"),
    ("retro_3d", "Retro 3D"),
    ("subtitle", "Subtitle"),
    ("tag", "Tag"),
    ("glass", "Glass Card"),
    ("notification", "Message Card"),
    ("stack", "Poster Stack"),
    ("refined", "Refined"),
    ("editorial", "Editorial"),
    ("gold", "Gold Foil"),
    ("quote", "Quote"),
];

/// Largest picture side in pixels; bigger titles are rendered at a lower
/// resolution (same size in project pixels).
const MAX_SIDE: u32 = 8192;

/// Renders `spec` at `scale` output pixels per project pixel. `reveal`
/// (0..1) animates the style's decorations in; `visible_chars` limits the
/// characters drawn (typewriter animation).
pub fn render_title(
    spec: &TitleSpec,
    scale: f32,
    reveal: f32,
    visible_chars: Option<usize>,
) -> TitleRaster {
    render_title_anim(spec, scale, reveal, visible_chars, None)
}

/// [`render_title`] with a letter-by-letter or word-by-word animation:
/// the kind and how far along it is (0..1).
pub fn render_title_anim(
    spec: &TitleSpec,
    scale: f32,
    reveal: f32,
    visible_chars: Option<usize>,
    letters: Option<(LetterKind, f32)>,
) -> TitleRaster {
    let mut scale = if scale.is_finite() && scale > 0.0 {
        scale.clamp(1e-3, 64.0)
    } else {
        1.0
    };
    let reveal = if reveal.is_finite() {
        reveal.clamp(0.0, 1.0)
    } else {
        1.0
    };
    for _ in 0..4 {
        match render_at(spec, scale, reveal, visible_chars, letters) {
            Ok(r) => return r,
            Err(side) => scale *= (MAX_SIDE as f32 - 64.0) / side,
        }
    }
    empty(scale)
}

fn empty(scale: f32) -> TitleRaster {
    TitleRaster {
        image: RgbaImage::new(4, 4),
        size: [4.0 / scale, 4.0 / scale],
    }
}

fn color(c: Rgba) -> Color {
    Color::from_rgba(
        c[0].clamp(0.0, 1.0),
        c[1].clamp(0.0, 1.0),
        c[2].clamp(0.0, 1.0),
        c[3].clamp(0.0, 1.0),
    )
    .unwrap_or(Color::TRANSPARENT)
}

fn solid(c: Rgba) -> Paint<'static> {
    Paint {
        shader: Shader::SolidColor(color(c)),
        anti_alias: true,
        ..Default::default()
    }
}

fn stroke(width: f32, cap: LineCap) -> Stroke {
    Stroke {
        width,
        line_cap: cap,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    }
}

fn grow(r: Rect, by: f32) -> Rect {
    Rect::from_ltrb(r.left() - by, r.top() - by, r.right() + by, r.bottom() + by).unwrap_or(r)
}

fn moved(r: Rect, dx: f32, dy: f32) -> Rect {
    Rect::from_ltrb(r.left() + dx, r.top() + dy, r.right() + dx, r.bottom() + dy).unwrap_or(r)
}

/// Shadow (offset x, offset y, blur radius) in output pixels.
fn shadow_geometry(spec: &TitleSpec, em: f32) -> Option<(f32, f32, f32)> {
    let k = (spec.shadow / 100.0).clamp(0.0, 1.0);
    if k.is_nan() || k <= 0.0 || spec.shadow_color[3] <= 0.0 {
        return None;
    }
    let off = em * (0.025 + 0.05 * k);
    Some((off * 0.75, off, em * (0.02 + 0.08 * k)))
}

/// Everything about one render that doesn't depend on the canvas.
struct Prepared {
    lay: Layout,
    em: f32,
    bold: f32,
    outline: f32,
}

fn render_at(
    spec: &TitleSpec,
    scale: f32,
    reveal: f32,
    visible: Option<usize>,
    letters: Option<(LetterKind, f32)>,
) -> Result<TitleRaster, f32> {
    if spec.text.trim().is_empty() {
        return Ok(empty(scale));
    }
    let size = if spec.size.is_finite() {
        spec.size.clamp(1.0, 1.0e5)
    } else {
        96.0
    };
    let em = size * scale;
    let primary = Face::load(&spec.font);
    let fallback = Face::fallback_for(&spec.font);
    let style = spec.style.as_str();
    let conf = |i: usize, n: usize| styles::line_conf(style, i, n);
    let params = Params {
        em,
        align: spec.align,
        letter_spacing: if spec.letter_spacing.is_finite() {
            spec.letter_spacing.clamp(-0.5, 5.0)
        } else {
            0.0
        },
        line_spacing: if spec.line_spacing.is_finite() {
            spec.line_spacing.clamp(0.2, 10.0)
        } else {
            1.15
        },
        italic: spec.italic,
        line_conf: &conf,
    };
    let lay = layout::layout(&spec.text, &primary, &fallback, &params);

    let bold = if spec.bold { 0.045 * em } else { 0.0 };
    let outline = if spec.outline.is_finite() {
        (spec.outline * scale).clamp(0.0, em)
    } else {
        0.0
    };
    let fx_full = styles::effects(spec, em, 1.0);
    let hollow = match fx_full.fill {
        Fill::Hollow(w) => w,
        _ => 0.0,
    };
    let stroke_half = (outline + bold * 0.5).max(hollow * 0.5 + outline);
    let prep = Prepared {
        lay,
        em,
        bold,
        outline,
    };

    // Measure everything at its fully revealed state.
    let block = Rect::from_xywh(0.0, 0.0, prep.lay.width.max(1.0), prep.lay.height).unwrap();
    let mut all = block;
    if let Some(ink) = prep.lay.ink {
        // Room for the text itself moving (split halves, glitch slices)
        // before the effects derived from it.
        let moving = fx_full.margin + fx_full.glitch.as_ref().map_or(0.0, |g| g.split + g.slice);
        let ink = grow(ink, stroke_half + 1.0 + moving);
        all = layout::union(all, ink);
        if let Some((dx, dy, blur)) = shadow_geometry(spec, em) {
            all = layout::union(all, grow(moved(ink, dx, dy), blur * 3.0));
        }
        for g in &fx_full.glows {
            all = layout::union(all, grow(ink, g.radius * 3.0));
        }
        if let Some(x) = &fx_full.extrude {
            all = layout::union(all, moved(ink, x.len, x.len));
        }
    }
    if spec.background {
        if let Some(bg) = background_path(&prep) {
            all = layout::union(all, bg.bounds());
        }
    }
    for op in styles::decorations(spec, &prep.lay, em, 1.0) {
        let b = op.path.bounds();
        all = layout::union(
            all,
            match op.stroke {
                Some((w, _)) => grow(b, w),
                None => b,
            },
        );
    }
    let (cx, cy) = (block.width() * 0.5, block.height() * 0.5);
    let half_w = (cx - all.left()).max(all.right() - cx) + 2.0;
    let half_h = (cy - all.top()).max(all.bottom() - cy) + 2.0;
    let (iw, ih) = ((2.0 * half_w).ceil(), (2.0 * half_h).ceil());
    if iw > MAX_SIDE as f32 || ih > MAX_SIDE as f32 {
        return Err(iw.max(ih));
    }
    let (iw, ih) = (iw.max(1.0) as u32, ih.max(1.0) as u32);
    let origin = Transform::from_translate(iw as f32 * 0.5 - cx, ih as f32 * 0.5 - cy);

    let mut canvas = Pixmap::new(iw, ih).unwrap();
    if spec.background {
        if let Some(bg) = background_path(&prep) {
            canvas.fill_path(
                &bg,
                &solid(spec.background_color),
                FillRule::Winding,
                origin,
                None,
            );
        }
    }
    let ops = styles::decorations(spec, &prep.lay, em, reveal);
    draw_ops(&mut canvas, &ops, false, origin);

    let fx = styles::effects(spec, em, reveal);
    let layered = shadow_geometry(spec, em).is_some()
        || !fx.glows.is_empty()
        || fx.extrude.is_some()
        || fx.glitch.is_some();
    if layered {
        let mut text = Pixmap::new(iw, ih).unwrap();
        draw_text(&mut text, spec, &prep, &fx, visible, letters, origin);
        compose_effects(&mut canvas, &text, spec, &prep, &fx, origin);
    } else {
        draw_text(&mut canvas, spec, &prep, &fx, visible, letters, origin);
    }
    draw_ops(&mut canvas, &ops, true, origin);

    Ok(TitleRaster {
        size: [iw as f32 / scale, ih as f32 / scale],
        image: RgbaImage {
            width: iw,
            height: ih,
            pixels: canvas.take(),
            premultiplied: true,
        },
    })
}

fn background_path(p: &Prepared) -> Option<Path> {
    let em = p.em;
    raster::rrect(
        -0.4 * em,
        -0.24 * em,
        p.lay.width + 0.8 * em,
        p.lay.height + 0.48 * em,
        0.14 * em,
    )
}

fn draw_ops(canvas: &mut Pixmap, ops: &[Op], front: bool, origin: Transform) {
    for op in ops.iter().filter(|o| o.front == front) {
        let paint = solid(op.color);
        match op.stroke {
            Some((w, cap)) => {
                let mut s = stroke(w, cap);
                s.line_join = styles::JOIN;
                canvas.stroke_path(&op.path, &paint, &s, origin, None);
            }
            None => canvas.fill_path(&op.path, &paint, FillRule::Winding, origin, None),
        }
    }
}

/// The visible glyphs of each line as one path per line.
fn line_paths(lay: &Layout, visible: Option<usize>) -> Vec<Option<Path>> {
    lay.lines
        .iter()
        .map(|l| {
            let mut pb = PathBuilder::new();
            for g in &lay.glyphs[l.glyphs.clone()] {
                if visible.is_none_or(|n| g.char_idx < n) {
                    pb.push_path(&g.path);
                }
            }
            pb.finish()
        })
        .collect()
}

fn vertical_gradient(y0: f32, y1: f32, c0: Rgba, c1: Rgba) -> Shader<'static> {
    LinearGradient::new(
        Point::from_xy(0.0, y0),
        Point::from_xy(0.0, y1.max(y0 + 0.5)),
        vec![
            GradientStop::new(0.0, color(c0)),
            GradientStop::new(1.0, color(c1)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(color(c0)))
}

/// How one glyph is moved and shown by a letter animation.
struct GlyphFx {
    dx: f32,
    dy: f32,
    scale: f32,
    alpha: f32,
    /// Painted in the accent colour (the karaoke highlight).
    accent: bool,
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn ease_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let (c1, c3) = (1.70158f32, 2.70158f32);
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Which word each character belongs to (spaces and line breaks belong
/// to none).
fn word_of_chars(text: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let (mut word, mut inside) = (0usize, false);
    for ch in text.chars() {
        if ch.is_whitespace() {
            if inside {
                word += 1;
                inside = false;
            }
            out.push(word);
        } else {
            inside = true;
            out.push(word);
        }
    }
    out
}

/// The motion of one glyph at `progress`. `unit` of `units` is its place in
/// the order the animation visits (letters or words); `centre` is the middle
/// of the glyph and `line_cx` the middle of its line, in block pixels.
fn glyph_fx(
    kind: LetterKind,
    progress: f32,
    unit: usize,
    units: usize,
    centre: (f32, f32),
    line_cx: f32,
    em: f32,
) -> GlyphFx {
    let p = progress.clamp(0.0, 1.0);
    let stagger = 0.55;
    let last = units.saturating_sub(1).max(1) as f32;
    let q = ((p - stagger * unit as f32 / last) / (1.0 - stagger)).clamp(0.0, 1.0);
    let mut g = GlyphFx {
        dx: 0.0,
        dy: 0.0,
        scale: 1.0,
        alpha: 1.0,
        accent: false,
    };
    match kind {
        LetterKind::Rise => {
            g.dy = (1.0 - ease_out(q)) * 0.7 * em;
            g.alpha = (q * 2.0).min(1.0);
        }
        LetterKind::Pop | LetterKind::WordsPop => {
            g.scale = ease_back(q).max(0.0);
            g.alpha = (q * 3.0).min(1.0);
        }
        LetterKind::Tracking => {
            let e = ease_out(p);
            g.dx = (centre.0 - line_cx) * (1.0 - e) * 0.9;
            g.alpha = (p * 2.5).min(1.0);
        }
        LetterKind::Karaoke => {
            // Words already sung are lit; the one being sung swells.
            let at = p * units as f32;
            g.accent = (unit as f32) < at;
            let bump = (1.0 - (at - unit as f32 - 0.5).abs() * 2.0).clamp(0.0, 1.0);
            g.scale = 1.0 + 0.1 * bump;
        }
    }
    g
}

/// Draws the letters: outline under the fill, then the fill (thickened by
/// a same-color stroke when bold). With a letter animation every glyph is
/// drawn on its own, moved and faded by its place in the animation.
fn draw_text(
    px: &mut Pixmap,
    spec: &TitleSpec,
    p: &Prepared,
    fx: &Effects,
    visible: Option<usize>,
    letters: Option<(LetterKind, f32)>,
    origin: Transform,
) {
    let paths = line_paths(&p.lay, visible);
    let (w, h) = (px.width(), px.height());
    // Split style: masks for the top and bottom halves of every line.
    let split_masks = if let Fill::Split { .. } = fx.fill {
        let mut top = Mask::new(w, h).unwrap();
        let mut bottom = Mask::new(w, h).unwrap();
        let mut tb = PathBuilder::new();
        let mut bb = PathBuilder::new();
        for l in &p.lay.lines {
            let mid = l.baseline - l.cap() * 0.5;
            let gap = 0.025 * l.em;
            let reach = 0.6 * l.em;
            let x0 = -p.lay.width - 4.0 * p.em;
            let x1 = 2.0 * p.lay.width + 4.0 * p.em;
            if let Some(r) = Rect::from_ltrb(x0, l.top - reach, x1, mid - gap) {
                tb.push_rect(r);
            }
            if let Some(r) = Rect::from_ltrb(x0, mid + gap, x1, l.bottom + reach) {
                bb.push_rect(r);
            }
        }
        if let Some(path) = tb.finish() {
            top.fill_path(&path, FillRule::Winding, true, origin);
        }
        if let Some(path) = bb.finish() {
            bottom.fill_path(&path, FillRule::Winding, true, origin);
        }
        Some((top, bottom))
    } else {
        None
    };

    // For letter animations: the unit (letter or word) of every glyph.
    let words = word_of_chars(&spec.text);
    let by_word = matches!(
        letters,
        Some((LetterKind::WordsPop | LetterKind::Karaoke, _))
    );
    let units_of: Vec<usize> = if letters.is_some() {
        let mut rank = 0usize;
        let mut prev = usize::MAX;
        p.lay
            .glyphs
            .iter()
            .map(|g| {
                if by_word {
                    words.get(g.char_idx).copied().unwrap_or(0)
                } else {
                    if g.char_idx != prev {
                        rank += 1;
                    }
                    prev = g.char_idx;
                    rank - 1
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let unit_count = units_of.iter().copied().max().map_or(1, |m| m + 1);
    let mut scratch: Option<Pixmap> = None;

    for (i, (l, line_path)) in p.lay.lines.iter().zip(&paths).enumerate() {
        // What to draw for this line: its whole path, or one entry per glyph.
        let mut items: Vec<(Path, Option<GlyphFx>)> = Vec::new();
        match letters {
            None => {
                if let Some(path) = line_path {
                    items.push((path.clone(), None));
                }
            }
            Some((kind, progress)) => {
                let line_cx = l.x + l.width * 0.5;
                for gi in l.glyphs.clone() {
                    let g = &p.lay.glyphs[gi];
                    if visible.is_some_and(|n| g.char_idx >= n) {
                        continue;
                    }
                    let b = g.path.bounds();
                    let centre = ((b.left() + b.right()) * 0.5, (b.top() + b.bottom()) * 0.5);
                    let fx = glyph_fx(
                        kind,
                        progress,
                        units_of[gi],
                        unit_count,
                        centre,
                        line_cx,
                        l.em,
                    );
                    items.push((g.path.clone(), Some(fx)));
                }
            }
        }
        let base = styles::line_color(spec, &p.lay, i);
        let bold = p.bold * l.em / p.em;
        let outline_w = 2.0 * p.outline + bold;
        for (path, gfx) in &items {
            let alpha = gfx.as_ref().map_or(1.0, |g| g.alpha);
            if alpha <= 0.003 {
                continue;
            }
            // The glyph's own move, about its centre, on top of the title's.
            let t = match gfx {
                Some(g) => {
                    let b = path.bounds();
                    let (cx, cy) = ((b.left() + b.right()) * 0.5, (b.top() + b.bottom()) * 0.5);
                    origin.pre_concat(
                        Transform::from_translate(cx + g.dx, cy + g.dy)
                            .pre_scale(g.scale, g.scale)
                            .pre_translate(-cx, -cy),
                    )
                }
                None => origin,
            };
            let base = if gfx.as_ref().is_some_and(|g| g.accent) {
                spec.accent
            } else {
                base
            };
            let paint_glyph = |target: &mut Pixmap| {
                let draw =
                    |px: &mut Pixmap, fill: Shader<'static>, t: Transform, mask: Option<&Mask>| {
                        if p.outline > 0.0 && spec.outline_color[3] > 0.0 {
                            px.stroke_path(
                                path,
                                &solid(spec.outline_color),
                                &stroke(outline_w, LineCap::Round),
                                t,
                                mask,
                            );
                        }
                        let paint = Paint {
                            shader: fill,
                            anti_alias: true,
                            ..Default::default()
                        };
                        if let Fill::Hollow(hw) = fx.fill {
                            px.stroke_path(
                                path,
                                &paint,
                                &stroke(hw * l.em / p.em + bold, LineCap::Round),
                                t,
                                mask,
                            );
                            return;
                        }
                        if bold > 0.0 {
                            px.stroke_path(path, &paint, &stroke(bold, LineCap::Round), t, mask);
                        }
                        px.fill_path(path, &paint, FillRule::Winding, t, mask);
                    };
                match fx.fill {
                    Fill::Solid | Fill::Hollow(_) => {
                        draw(target, Shader::SolidColor(color(base)), t, None)
                    }
                    Fill::Gradient(to) => {
                        let g = vertical_gradient(l.top, l.baseline + 0.1 * l.em, base, to);
                        // The gradient is in block space; the shader takes the
                        // same transform as the path.
                        draw(target, g, t, None)
                    }
                    Fill::Split { offset } => {
                        let (top, bottom) = split_masks.as_ref().unwrap();
                        draw(
                            target,
                            Shader::SolidColor(color(base)),
                            t.pre_translate(-offset, 0.0),
                            Some(top),
                        );
                        draw(
                            target,
                            Shader::SolidColor(color(spec.accent)),
                            t.pre_translate(offset, 0.0),
                            Some(bottom),
                        );
                    }
                }
            };
            if alpha < 0.999 {
                let sc = scratch.get_or_insert_with(|| Pixmap::new(w, h).unwrap());
                sc.fill(Color::TRANSPARENT);
                paint_glyph(sc);
                px.draw_pixmap(
                    0,
                    0,
                    sc.as_ref(),
                    &PixmapPaint {
                        opacity: alpha,
                        ..Default::default()
                    },
                    Transform::identity(),
                    None,
                );
            } else {
                paint_glyph(px);
            }
        }
    }
}

/// Shadow, extrusion, glow and glitch copies from the text's alpha, then
/// the text itself.
fn compose_effects(
    canvas: &mut Pixmap,
    text: &Pixmap,
    spec: &TitleSpec,
    p: &Prepared,
    fx: &Effects,
    origin: Transform,
) {
    let (w, h) = (canvas.width() as usize, canvas.height() as usize);
    let alpha = raster::alpha_of(text.data());
    let dst = canvas.data_mut();
    if let Some((dx, dy, radius)) = shadow_geometry(spec, p.em) {
        let a = raster::blurred(&alpha, w, h, radius);
        let k = (spec.shadow / 100.0).clamp(0.0, 1.0);
        let c = spec.shadow_color;
        raster::paint_mask(
            dst,
            w,
            h,
            &a,
            dx.round() as i32,
            dy.round() as i32,
            [c[0], c[1], c[2], c[3] * (0.6 + 0.4 * k)],
            false,
        );
    }
    if let Some(x) = &fx.extrude {
        let len = x.len.round() as usize;
        if len > 0 {
            let (cov, dist) = raster::extrude(&alpha, w, h, len);
            raster::paint_extrusion(dst, &cov, &dist, len, x.near, x.far, x.fade);
        }
    }
    for g in &fx.glows {
        if g.color[3] <= 0.0 {
            continue;
        }
        let mut a = raster::blurred(&alpha, w, h, g.radius);
        let boost = (g.boost * 256.0) as u32;
        for v in a.iter_mut() {
            *v = ((*v as u32 * boost) >> 8).min(255) as u8;
        }
        raster::paint_mask(dst, w, h, &a, 0, 0, g.color, true);
    }
    if let Some(g) = &fx.glitch {
        let d = g.split.round() as i32;
        raster::paint_mask(dst, w, h, &alpha, -d, 0, [0.0, 0.9, 1.0, 0.9], true);
        raster::paint_mask(dst, w, h, &alpha, d, 0, [1.0, 0.1, 0.35, 0.9], true);
        // Sideways slices across the block.
        let oy = origin.ty;
        let bh = p.lay.height;
        let bands = [(0.12, 0.22, 1.0), (0.48, 0.56, -1.0), (0.74, 0.8, 0.6)];
        let slice = g.slice;
        raster::over_rows(dst, text.data(), w, h, |y| {
            let fy = (y as f32 - oy) / bh;
            bands
                .iter()
                .find(|b| fy >= b.0 && fy < b.1)
                .map_or(0, |b| (slice * b.2).round() as i32)
        });
        return;
    }
    raster::over_rows(dst, text.data(), w, h, |_| 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(text: &str) -> TitleSpec {
        TitleSpec {
            text: text.into(),
            ..Default::default()
        }
    }

    fn coverage(r: &TitleRaster) -> usize {
        r.image
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .count()
    }

    #[test]
    fn empty_text_is_small_and_transparent() {
        for t in ["", "   ", "\n"] {
            let r = render_title(&spec(t), 1.0, 1.0, None);
            assert!(r.image.width <= 8 && r.image.height <= 8);
            assert!(r.image.pixels.iter().all(|&v| v == 0));
        }
    }

    #[test]
    fn size_matches_image_over_scale() {
        let r = render_title(&spec("Hello"), 0.5, 1.0, None);
        assert_eq!(
            r.image.pixels.len(),
            (r.image.width * r.image.height * 4) as usize
        );
        assert!((r.size[0] * 0.5 - r.image.width as f32).abs() < 0.01);
        assert!((r.size[1] * 0.5 - r.image.height as f32).abs() < 0.01);
        assert!(coverage(&r) > 100);
    }

    #[test]
    fn multi_line_is_taller_and_aligned() {
        let one = render_title(&spec("Line"), 1.0, 1.0, None);
        let two = render_title(&spec("Line\nLine two"), 1.0, 1.0, None);
        assert!(two.image.height > one.image.height + 60);
        for align in [
            tidycuts_core::TextAlign::Left,
            tidycuts_core::TextAlign::Right,
        ] {
            let s = TitleSpec {
                align,
                ..spec("A\nWider line")
            };
            let r = render_title(&s, 1.0, 1.0, None);
            assert_eq!(r.image.width, two.image.width.max(r.image.width));
            assert!(coverage(&r) > 100);
        }
    }

    #[test]
    fn typewriter_keeps_the_layout() {
        let s = TitleSpec {
            style: "box".into(),
            ..spec("Typing\nalong")
        };
        let full = render_title(&s, 1.0, 1.0, None);
        let none = render_title(&s, 1.0, 1.0, Some(0));
        let half = render_title(&s, 1.0, 1.0, Some(4));
        for r in [&none, &half] {
            assert_eq!(
                (r.image.width, r.image.height),
                (full.image.width, full.image.height)
            );
        }
        // With no characters only the box remains.
        assert!(coverage(&none) > 0);
        assert_ne!(none.image.pixels, half.image.pixels);
        assert_ne!(half.image.pixels, full.image.pixels);
        let plain = render_title(&spec("Typing"), 1.0, 1.0, Some(0));
        assert_eq!(coverage(&plain), 0);
    }

    #[test]
    fn huge_titles_are_clamped() {
        let s = TitleSpec {
            size: 20_000.0,
            ..spec("Enormous title")
        };
        let r = render_title(&s, 2.0, 1.0, None);
        assert!(r.image.width <= MAX_SIDE && r.image.height <= MAX_SIDE);
        assert!(r.size[0] > 20_000.0);
        assert!(coverage(&r) > 1000);
    }

    /// How many pixels are mostly opaque.
    fn ink(r: &TitleRaster) -> usize {
        r.image.pixels.chunks(4).filter(|p| p[3] > 128).count()
    }

    #[test]
    fn letter_animations_start_empty_and_end_like_the_plain_title() {
        let s = spec("Hello wide world");
        let plain = ink(&render_title(&s, 1.0, 1.0, None));
        assert!(plain > 0);
        for kind in [
            LetterKind::Rise,
            LetterKind::Pop,
            LetterKind::WordsPop,
            LetterKind::Tracking,
        ] {
            let start = ink(&render_title_anim(&s, 1.0, 1.0, None, Some((kind, 0.0))));
            let mid = ink(&render_title_anim(&s, 1.0, 1.0, None, Some((kind, 0.5))));
            let end = ink(&render_title_anim(&s, 1.0, 1.0, None, Some((kind, 1.0))));
            assert!(
                start < plain / 20,
                "{kind:?} starts with {start} of {plain}"
            );
            assert!(mid > start && mid < end + plain / 10, "{kind:?} mid {mid}");
            let off = end.abs_diff(plain) as f32 / plain as f32;
            assert!(off < 0.06, "{kind:?} ends at {end}, plain {plain}");
        }
    }

    #[test]
    fn karaoke_lights_the_words_one_by_one() {
        let mut s = spec("one two three");
        s.color = [1.0, 1.0, 1.0, 1.0];
        s.accent = [1.0, 0.0, 0.0, 1.0];
        let lit = |p: f32| {
            let r = render_title_anim(&s, 1.0, 1.0, None, Some((LetterKind::Karaoke, p)));
            // Count the strongly red pixels (the highlight).
            r.image
                .pixels
                .chunks(4)
                .filter(|px| px[3] > 128 && px[0] > 200 && px[1] < 80)
                .count()
        };
        let (none, some, all) = (lit(0.0), lit(0.5), lit(1.0));
        assert!(none == 0, "nothing is lit at the start: {none}");
        assert!(some > 0 && some < all, "some {some}, all {all}");
    }

    #[test]
    fn reveal_ends_and_every_style() {
        for (id, _) in STYLES {
            let s = TitleSpec {
                style: id.to_string(),
                text: "Style Test\nSecond line".into(),
                shadow: 40.0,
                outline: 3.0,
                ..Default::default()
            };
            let r0 = render_title(&s, 0.5, 0.0, None);
            let r1 = render_title(&s, 0.5, 1.0, None);
            let rh = render_title(&s, 0.5, 0.5, Some(5));
            assert_eq!(
                (r0.image.width, r0.image.height),
                (r1.image.width, r1.image.height),
                "{id}"
            );
            assert_eq!(
                (rh.image.width, rh.image.height),
                (r1.image.width, r1.image.height),
                "{id}"
            );
            assert!(coverage(&r1) > 500, "{id}");
            // Nothing touches the image border (no clipping).
            let (w, h) = (r1.image.width as usize, r1.image.height as usize);
            for (img, name) in [(&r0, "r0"), (&r1, "r1"), (&rh, "rh")] {
                let px = &img.image.pixels;
                let edge = (0..w).any(|x| px[x * 4 + 3] > 8 || px[((h - 1) * w + x) * 4 + 3] > 8)
                    || (0..h).any(|y| px[y * w * 4 + 3] > 8 || px[(y * w + w - 1) * 4 + 3] > 8);
                assert!(!edge, "{id} {name} touches the border");
            }
        }
    }

    #[test]
    fn odd_values_do_not_panic() {
        let s = TitleSpec {
            size: f32::NAN,
            letter_spacing: f32::INFINITY,
            line_spacing: -3.0,
            outline: 1e9,
            shadow: 1e9,
            italic: true,
            bold: true,
            background: true,
            text: "Ünïcode ✓ Кирилица 中\n\n\tTabs".into(),
            ..Default::default()
        };
        let r = render_title(&s, f32::NAN, f32::NAN, Some(usize::MAX));
        assert!(coverage(&r) > 0);
        let tiny = render_title(&spec("x"), 0.001, 1.0, None);
        assert!(tiny.image.width >= 1);
    }
}
