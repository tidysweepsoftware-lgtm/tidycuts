//! Line layout: places glyphs with ab_glyph metrics (advances, kerning,
//! letter spacing) and turns their outlines into tiny-skia paths.
//!
//! Everything is in output pixels with the origin at the top-left of the
//! text block. The block's vertical extent is the "visual" line box: from
//! the cap height of the first line to a little under the baseline of the
//! last one, so boxes and bars hug the letters the same way whatever is
//! typed (no jumping while the user edits).

use ab_glyph::{Font, FontRef, GlyphId, OutlineCurve, Point, VariableFont};
use std::ops::Range;
use tidycuts_core::TextAlign;
use tiny_skia::{Path, PathBuilder, Rect};

/// tan(12°): the slant used for synthetic italics.
const ITALIC_SKEW: f32 = 0.2126;

/// A font with the metrics layout needs, all in ems.
pub struct Face {
    pub font: FontRef<'static>,
    upem: f32,
    ascent: f32,
    descent: f32,
    /// Height of a capital letter.
    pub cap: f32,
    /// How far the line box reaches under the baseline.
    pub under: f32,
}

impl Face {
    pub fn load(name: &str) -> Face {
        let font = FontRef::try_from_slice(crate::fonts::get(name).data)
            .or_else(|_| FontRef::try_from_slice(crate::fonts::get("Poppins Bold").data))
            .expect("embedded fonts parse");
        Face::new(font)
    }

    /// Inter (wide coverage: Latin, Cyrillic, Greek, symbols) at a weight
    /// close to `primary`, for characters the chosen font doesn't have.
    pub fn fallback_for(primary: &str) -> Face {
        let mut font =
            FontRef::try_from_slice(crate::fonts::inter()).expect("embedded fonts parse");
        font.set_variation(b"wght", weight_of(primary));
        font.set_variation(b"opsz", 32.0);
        Face::new(font)
    }

    fn new(font: FontRef<'static>) -> Face {
        let upem = font.units_per_em().unwrap_or(1000.0).max(1.0);
        let ascent = (font.ascent_unscaled() / upem).clamp(0.3, 3.0);
        let descent = (-font.descent_unscaled() / upem).clamp(0.0, 2.0);
        let ink = |c: char| {
            let id = font.glyph_id(c);
            if id.0 == 0 {
                None
            } else {
                font.outline(id).map(|o| o.bounds)
            }
        };
        // ab_glyph's outline bounds hold (x_min, y_max) in `min` and
        // (x_max, y_min) in `max`, y pointing up.
        let cap = ink('H')
            .map(|b| b.min.y / upem)
            .filter(|c| *c > 0.2)
            .unwrap_or(ascent * 0.72);
        let depth = ink('p')
            .map(|b| (-b.max.y / upem).max(0.0))
            .unwrap_or(descent * 0.6);
        Face {
            font,
            upem,
            ascent,
            descent,
            cap: cap.min(ascent),
            under: depth.min(descent.max(0.05)) * 0.6,
        }
    }

    fn has(&self, c: char) -> bool {
        self.font.glyph_id(c).0 != 0
    }

    /// Distance between baselines at 1 em, before `line_spacing`. Fonts
    /// disagree wildly about their line height (Poppins says 1.5 em, VT323
    /// 1.0), so it's kept within a range that suits titles.
    fn line_height(&self) -> f32 {
        (self.ascent + self.descent).clamp(1.0, 1.25)
    }

    fn ascent_share(&self) -> f32 {
        self.ascent / (self.ascent + self.descent).max(0.01)
    }
}

/// A rough weight for each embedded font, to match fallback glyphs to it.
fn weight_of(name: &str) -> f32 {
    let n = crate::fonts::get(name).name;
    match n {
        _ if n.ends_with("Black") => 900.0,
        _ if n.ends_with("SemiBold") => 600.0,
        _ if n.ends_with("Bold") => 700.0,
        "Poppins" | "Inter" | "Special Elite" | "VT323" | "Great Vibes" | "Press Start 2P" => 400.0,
        "DM Serif Display" | "Lobster" | "Pacifico" | "Righteous" | "Permanent Marker"
        | "Monoton" => 600.0,
        "Bebas Neue" => 700.0,
        _ => 800.0,
    }
}

/// How a style wants one line laid out.
#[derive(Clone, Copy, Debug)]
pub struct LineConf {
    /// Size relative to the title size.
    pub factor: f32,
    /// Extra space after this line, in ems of the title size.
    pub gap_after: f32,
    /// Added to the letter spacing (fraction of the line's size).
    pub extra_spacing: f32,
}

impl Default for LineConf {
    fn default() -> Self {
        LineConf {
            factor: 1.0,
            gap_after: 0.0,
            extra_spacing: 0.0,
        }
    }
}

pub struct Line {
    /// Font size of this line in output pixels.
    pub em: f32,
    /// Left edge and advance width.
    pub x: f32,
    pub width: f32,
    pub baseline: f32,
    /// Visual line box: cap height to a little under the baseline.
    pub top: f32,
    pub bottom: f32,
    /// Index range into `Layout::glyphs`.
    pub glyphs: Range<usize>,
}

impl Line {
    pub fn cap(&self) -> f32 {
        self.baseline - self.top
    }
}

pub struct Glyph {
    pub path: Path,
    /// Index of the character in the whole text (newlines count).
    pub char_idx: usize,
}

pub struct Layout {
    pub lines: Vec<Line>,
    pub glyphs: Vec<Glyph>,
    /// Size of the text block.
    pub width: f32,
    pub height: f32,
    /// Union of every glyph outline (all characters, visible or not).
    pub ink: Option<Rect>,
}

pub struct Params<'a> {
    pub em: f32,
    pub align: TextAlign,
    pub letter_spacing: f32,
    pub line_spacing: f32,
    pub italic: bool,
    pub line_conf: &'a dyn Fn(usize, usize) -> LineConf,
}

struct Placed {
    fallback: bool,
    id: GlyphId,
    x: f32,
    char_idx: usize,
}

pub fn layout(text: &str, primary: &Face, fallback: &Face, p: &Params) -> Layout {
    // Split into lines, keeping each character's index in the whole text.
    let mut rows: Vec<Vec<(usize, char)>> = vec![Vec::new()];
    for (i, c) in text.chars().enumerate() {
        match c {
            '\n' => rows.push(Vec::new()),
            '\r' => {}
            '\t' => rows.last_mut().unwrap().push((i, ' ')),
            _ => rows.last_mut().unwrap().push((i, c)),
        }
    }
    let n = rows.len();
    let confs: Vec<LineConf> = (0..n).map(|i| (p.line_conf)(i, n)).collect();

    // Horizontal placement.
    let mut placed: Vec<(Vec<Placed>, f32)> = Vec::with_capacity(n);
    for (row, conf) in rows.iter().zip(&confs) {
        let le = p.em * conf.factor;
        let spacing = (p.letter_spacing + conf.extra_spacing) * le;
        let mut pen = 0.0f32;
        let mut prev: Option<(bool, GlyphId)> = None;
        let mut out = Vec::with_capacity(row.len());
        for &(idx, c) in row {
            let use_fallback = !primary.has(c) && !c.is_whitespace() && fallback.has(c);
            let face = if use_fallback { fallback } else { primary };
            let ppu = le / face.upem;
            let id = face.font.glyph_id(c);
            if let Some((pf, pid)) = prev {
                if pf == use_fallback {
                    pen += face.font.kern_unscaled(pid, id) * ppu;
                }
            }
            out.push(Placed {
                fallback: use_fallback,
                id,
                x: pen,
                char_idx: idx,
            });
            pen += face.font.h_advance_unscaled(id) * ppu + spacing;
            prev = Some((use_fallback, id));
        }
        let width = if out.is_empty() {
            0.0
        } else {
            (pen - spacing).max(0.0)
        };
        placed.push((out, width));
    }

    // Vertical placement.
    let lh = primary.line_height();
    let share = primary.ascent_share();
    let mut baselines = Vec::with_capacity(n);
    let mut b = primary.cap * p.em * confs[0].factor;
    for i in 0..n {
        if i > 0 {
            let (e0, e1) = (p.em * confs[i - 1].factor, p.em * confs[i].factor);
            b += p.line_spacing * lh * (e0 * (1.0 - share) + e1 * share)
                + confs[i - 1].gap_after * p.em;
        }
        baselines.push(b);
    }
    let tops: Vec<f32> = (0..n)
        .map(|i| baselines[i] - primary.cap * p.em * confs[i].factor)
        .collect();
    let bottoms: Vec<f32> = (0..n)
        .map(|i| baselines[i] + primary.under * p.em * confs[i].factor)
        .collect();
    let min_top = tops.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_bottom = bottoms.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let width = placed.iter().map(|(_, w)| *w).fold(0.0, f32::max);

    let mut lines = Vec::with_capacity(n);
    let mut glyphs = Vec::new();
    let mut ink: Option<Rect> = None;
    for (i, (row, w)) in placed.into_iter().enumerate() {
        let le = p.em * confs[i].factor;
        let x = match p.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => (width - w) * 0.5,
            TextAlign::Right => width - w,
        };
        let baseline = baselines[i] - min_top;
        let start = glyphs.len();
        let skew = if p.italic { ITALIC_SKEW } else { 0.0 };
        // Slant around the middle of the capitals so the text stays centred
        // over its decorations.
        let pivot = -primary.cap * le * 0.5;
        for g in row {
            let face = if g.fallback { fallback } else { primary };
            let ppu = le / face.upem;
            if let Some(path) = glyph_path(face, g.id, ppu, x + g.x, baseline, skew, pivot) {
                let bb = path.bounds();
                ink = Some(match ink {
                    Some(r) => union(r, bb),
                    None => bb,
                });
                glyphs.push(Glyph {
                    path,
                    char_idx: g.char_idx,
                });
            }
        }
        lines.push(Line {
            em: le,
            x,
            width: w,
            baseline,
            top: tops[i] - min_top,
            bottom: bottoms[i] - min_top,
            glyphs: start..glyphs.len(),
        });
    }
    Layout {
        lines,
        glyphs,
        width,
        height: (max_bottom - min_top).max(1.0),
        ink,
    }
}

pub fn union(a: Rect, b: Rect) -> Rect {
    Rect::from_ltrb(
        a.left().min(b.left()),
        a.top().min(b.top()),
        a.right().max(b.right()),
        a.bottom().max(b.bottom()),
    )
    .unwrap_or(a)
}

/// The outline of one glyph placed with its origin at (`ox`, `oy`).
fn glyph_path(
    face: &Face,
    id: GlyphId,
    ppu: f32,
    ox: f32,
    oy: f32,
    skew: f32,
    pivot: f32,
) -> Option<Path> {
    let outline = face.font.outline(id)?;
    let map = |p: &Point| {
        let y = -p.y * ppu;
        (ox + p.x * ppu + skew * (pivot - y), oy + y)
    };
    let mut pb = PathBuilder::with_capacity(outline.curves.len() + 8, outline.curves.len() * 3 + 8);
    let mut last: Option<Point> = None;
    for curve in &outline.curves {
        let start = match curve {
            OutlineCurve::Line(a, _)
            | OutlineCurve::Quad(a, _, _)
            | OutlineCurve::Cubic(a, _, _, _) => *a,
        };
        let joined =
            last.is_some_and(|l| (l.x - start.x).abs() < 1e-3 && (l.y - start.y).abs() < 1e-3);
        if !joined {
            if last.is_some() {
                pb.close();
            }
            let (x, y) = map(&start);
            pb.move_to(x, y);
        }
        match curve {
            OutlineCurve::Line(_, b) => {
                let (x, y) = map(b);
                pb.line_to(x, y);
                last = Some(*b);
            }
            OutlineCurve::Quad(_, c, b) => {
                let (cx, cy) = map(c);
                let (x, y) = map(b);
                pb.quad_to(cx, cy, x, y);
                last = Some(*b);
            }
            OutlineCurve::Cubic(_, c1, c2, b) => {
                let (c1x, c1y) = map(c1);
                let (c2x, c2y) = map(c2);
                let (x, y) = map(b);
                pb.cubic_to(c1x, c1y, c2x, c2y, x, y);
                last = Some(*b);
            }
        }
    }
    if last.is_some() {
        pb.close();
    }
    pb.finish()
}
