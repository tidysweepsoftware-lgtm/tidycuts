//! Templates: plain color and gradient backgrounds plus multi-clip recipes
//! (intros, lower thirds, end screens, social posts…) built only from
//! TidyCuts' own content: backgrounds, titles, stickers, effects and
//! animations. Placing one puts every clip on the timeline as one undo
//! step, adding overlay tracks above the drop track as needed.

use crate::app::TidyCutsApp;
use tidycuts_core::{
    Animatable, Clip, ClipAnimation, ClipId, Color, Effect, Generator, Project, StickerSpec,
    TextAlign, Time, TitleSpec, TrackId, TrackKind, MAX_VIDEO_TRACKS,
};
use tidycuts_render::catalog::stickers;

/// A ready-made background or combination of clips.
pub struct Template {
    pub name: &'static str,
    pub category: &'static str,
    /// Length in seconds; 0 uses the still-image length from the
    /// preferences (plain backgrounds).
    pub length: f64,
    /// A moment that shows the template well, in seconds from its start
    /// (used for its library picture).
    pub poster: f64,
    /// Sits over the video below (lower thirds, reminders) instead of
    /// covering the frame, so it never goes on the main track.
    pub overlay: bool,
    /// The clips, one lane (track) per entry from the bottom up, with
    /// starts measured from the template's start.
    pub build: fn(&Canvas) -> Vec<Vec<Clip>>,
}

/// The frame a template is laid out for. Recipes are designed on a
/// 1080-pixel-high frame; [`Canvas::u`] scales design pixels to the
/// project and the edge anchors keep corner pieces inside any aspect.
pub struct Canvas {
    pub w: f64,
    pub h: f64,
    /// Seconds.
    pub len: f64,
}

impl Canvas {
    pub fn new(project: &Project, len: f64) -> Canvas {
        Canvas {
            w: project.settings.width.max(2) as f64,
            h: project.settings.height.max(2) as f64,
            len,
        }
    }

    fn k(&self) -> f64 {
        self.w.min(self.h) / 1080.0
    }

    /// Design pixels to project pixels.
    fn u(&self, v: f64) -> f64 {
        v * self.k()
    }

    /// Offset from the frame centre of a point `margin` design pixels in
    /// from the left edge.
    fn left(&self, margin: f64) -> f64 {
        -self.w * 0.5 + self.u(margin)
    }

    fn right(&self, margin: f64) -> f64 {
        self.w * 0.5 - self.u(margin)
    }

    fn top(&self, margin: f64) -> f64 {
        -self.h * 0.5 + self.u(margin)
    }

    fn bottom(&self, margin: f64) -> f64 {
        self.h * 0.5 - self.u(margin)
    }
}

pub static TEMPLATES: &[Template] = &[
    // ----- Intros -----
    Template {
        name: "Channel Intro",
        category: "Intros",
        length: 5.0,
        poster: 2.2,
        overlay: false,
        build: channel_intro,
    },
    Template {
        name: "Countdown 3-2-1",
        category: "Intros",
        length: 4.0,
        poster: 0.6,
        overlay: false,
        build: countdown,
    },
    Template {
        name: "Vlog Opener",
        category: "Intros",
        length: 5.0,
        poster: 2.4,
        overlay: false,
        build: vlog_opener,
    },
    Template {
        name: "Coming Soon",
        category: "Intros",
        length: 5.0,
        poster: 2.6,
        overlay: false,
        build: coming_soon,
    },
    // ----- Lower thirds -----
    Template {
        name: "Lower Third",
        category: "Lower Thirds",
        length: 6.0,
        poster: 2.5,
        overlay: true,
        build: lower_third,
    },
    Template {
        name: "Breaking News",
        category: "Lower Thirds",
        length: 6.0,
        poster: 2.5,
        overlay: true,
        build: breaking_news,
    },
    // ----- Social -----
    Template {
        name: "Social Post (Vertical)",
        category: "Social",
        length: 6.0,
        poster: 2.6,
        overlay: false,
        build: social_post,
    },
    Template {
        name: "Subscribe Reminder",
        category: "Social",
        length: 5.0,
        poster: 2.4,
        overlay: true,
        build: subscribe_reminder,
    },
    Template {
        name: "Social Handle",
        category: "Social",
        length: 5.0,
        poster: 2.0,
        overlay: true,
        build: social_handle,
    },
    // ----- End screens -----
    Template {
        name: "Subscribe End Screen",
        category: "End Screens",
        length: 8.0,
        poster: 2.5,
        overlay: false,
        build: end_screen,
    },
    // ----- Text -----
    Template {
        name: "Quote Card",
        category: "Text",
        length: 7.0,
        poster: 3.6,
        overlay: false,
        build: quote_card,
    },
    Template {
        name: "Chapter Title",
        category: "Text",
        length: 5.0,
        poster: 2.2,
        overlay: false,
        build: chapter_title,
    },
    // ----- Promo -----
    Template {
        name: "Sale Banner",
        category: "Promo",
        length: 5.0,
        poster: 2.0,
        overlay: false,
        build: sale_banner,
    },
    // ----- Backgrounds -----
    Template {
        name: "Black",
        category: "Backgrounds",
        length: 0.0,
        poster: 0.0,
        overlay: false,
        build: |cv| plain(cv, Generator::Solid([0.0, 0.0, 0.0, 1.0])),
    },
    Template {
        name: "White",
        category: "Backgrounds",
        length: 0.0,
        poster: 0.0,
        overlay: false,
        build: |cv| plain(cv, Generator::Solid([1.0, 1.0, 1.0, 1.0])),
    },
    Template {
        name: "Midnight",
        category: "Backgrounds",
        length: 0.0,
        poster: 0.0,
        overlay: false,
        build: |cv| plain(cv, Generator::Solid([0.07, 0.09, 0.16, 1.0])),
    },
    Template {
        name: "Mint",
        category: "Backgrounds",
        length: 0.0,
        poster: 0.0,
        overlay: false,
        build: |cv| plain(cv, Generator::Solid([0.18, 0.85, 0.69, 1.0])),
    },
    Template {
        name: "Sunset",
        category: "Backgrounds",
        length: 0.0,
        poster: 0.0,
        overlay: false,
        build: |cv| {
            plain(
                cv,
                Generator::Gradient {
                    a: [0.98, 0.45, 0.25, 1.0],
                    b: [0.55, 0.18, 0.55, 1.0],
                    angle: 135.0,
                },
            )
        },
    },
    Template {
        name: "Ocean",
        category: "Backgrounds",
        length: 0.0,
        poster: 0.0,
        overlay: false,
        build: |cv| {
            plain(
                cv,
                Generator::Gradient {
                    a: [0.1, 0.55, 0.85, 1.0],
                    b: [0.05, 0.15, 0.4, 1.0],
                    angle: 90.0,
                },
            )
        },
    },
    Template {
        name: "Peach",
        category: "Backgrounds",
        length: 0.0,
        poster: 0.0,
        overlay: false,
        build: |cv| plain(cv, gradient(0xffd3a5, 0xfd6585, 160.0)),
    },
    Template {
        name: "Aurora",
        category: "Backgrounds",
        length: 0.0,
        poster: 0.0,
        overlay: false,
        build: |cv| plain(cv, gradient(0x00c9a7, 0x4b2a91, 120.0)),
    },
];

pub fn get(index: usize) -> Option<&'static Template> {
    TEMPLATES.get(index)
}

// ----- building blocks -----------------------------------------------------

fn secs(s: f64) -> Time {
    Time::from_secs_f64(s)
}

/// An opaque color from 0xRRGGBB.
fn hex(rgb: u32) -> Color {
    [
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
        1.0,
    ]
}

fn gradient(a: u32, b: u32, angle: f32) -> Generator {
    Generator::Gradient {
        a: hex(a),
        b: hex(b),
        angle,
    }
}

/// A title style with the common settings filled in.
fn text(text: &str, font: &str, size: f32, color: u32) -> TitleSpec {
    TitleSpec {
        text: text.into(),
        font: font.into(),
        size,
        color: hex(color),
        ..TitleSpec::default()
    }
}

/// A clip being set up for a recipe.
struct Piece(Clip);

fn piece(generator: Generator, start: f64, end: f64) -> Piece {
    Piece(Clip::generated(generator, secs(start), secs(end - start)))
}

fn title(spec: TitleSpec, start: f64, end: f64) -> Piece {
    piece(Generator::Title(spec), start, end)
}

/// A catalog sticker in its own colors and motion.
fn sticker(kind: &str, start: f64, end: f64) -> Piece {
    let def = stickers::get(kind);
    let spec = StickerSpec {
        kind: kind.into(),
        color: def.map(|d| d.color).unwrap_or([1.0; 4]),
        color2: def.map(|d| d.color2).unwrap_or([1.0; 4]),
        motion: def.map(|d| d.motion).unwrap_or("none").into(),
    };
    piece(Generator::Sticker(spec), start, end)
}

/// A full-length background.
fn backdrop(cv: &Canvas, generator: Generator) -> Piece {
    piece(generator, 0.0, cv.len).named("Background")
}

fn plain(cv: &Canvas, generator: Generator) -> Vec<Vec<Clip>> {
    vec![vec![piece(generator, 0.0, cv.len).0]]
}

impl Piece {
    fn at(mut self, x: f64, y: f64) -> Self {
        self.0.transform.position_x = Animatable::constant(x);
        self.0.transform.position_y = Animatable::constant(y);
        self
    }

    /// Percent of the natural size.
    fn scale(mut self, pct: f64) -> Self {
        self.0.transform.scale_x = Animatable::constant(pct);
        self.0.transform.scale_y = Animatable::constant(pct);
        self
    }

    /// Makes a color clip a `w` x `h` rectangle (design pixels).
    fn rect(mut self, cv: &Canvas, w: f64, h: f64) -> Self {
        self.0.transform.scale_x = Animatable::constant(cv.u(w) / cv.w * 100.0);
        self.0.transform.scale_y = Animatable::constant(cv.u(h) / cv.h * 100.0);
        self
    }

    fn rotate(mut self, deg: f64) -> Self {
        self.0.transform.rotation = Animatable::constant(deg);
        self
    }

    fn opacity(mut self, pct: f64) -> Self {
        self.0.transform.opacity = Animatable::constant(pct);
        self
    }

    /// Rounded corners in design pixels (the planner scales them).
    fn corners(mut self, radius: f64) -> Self {
        self.0.video.corner_radius = radius;
        self
    }

    fn shadow(mut self) -> Self {
        let s = &mut self.0.video.shadow;
        s.enabled = true;
        s.opacity = 45.0;
        s.distance = 14.0;
        s.blur = 60.0;
        self
    }

    fn enter(mut self, kind: &str, d: f64) -> Self {
        self.0.anim_in = Some(ClipAnimation {
            kind: kind.into(),
            duration: secs(d),
        });
        self
    }

    fn exit(mut self, kind: &str, d: f64) -> Self {
        self.0.anim_out = Some(ClipAnimation {
            kind: kind.into(),
            duration: secs(d),
        });
        self
    }

    /// Adds a catalog effect or filter with some parameters changed.
    fn fx(mut self, kind: &str, params: &[(&str, f64)]) -> Self {
        let mut e = Effect::new(kind);
        for (k, v) in params {
            e.params.insert(k.to_string(), Animatable::constant(*v));
        }
        self.0.effects.push(e);
        self
    }

    /// Recolors a sticker.
    fn colors(mut self, a: u32, b: u32) -> Self {
        if let Some(Generator::Sticker(s)) = &mut self.0.generator {
            s.color = hex(a);
            s.color2 = hex(b);
        }
        self
    }

    fn motion(mut self, m: &str) -> Self {
        if let Some(Generator::Sticker(s)) = &mut self.0.generator {
            s.motion = m.into();
        }
        self
    }

    fn named(mut self, name: &str) -> Self {
        self.0.name = Some(name.into());
        self
    }

    /// Places a title so the left edge of its lettering is at `x`.
    fn left_at(self, cv: &Canvas, x: f64, y: f64) -> Self {
        let ink = match &self.0.generator {
            Some(Generator::Title(spec)) => ink_left(spec),
            _ => 0.0,
        };
        self.at(x + cv.u(ink), y)
    }

    fn done(self) -> Clip {
        self.0
    }
}

/// How far the drawn part of a title reaches left of its centre, in
/// design pixels (titles are centred on their position).
fn ink_left(spec: &TitleSpec) -> f64 {
    let scale = 0.25;
    let r = tidycuts_render::text::render_title(spec, scale, 1.0, None);
    let img = &r.image;
    let (w, h) = (img.width as usize, img.height as usize);
    let first = (0..w).find(|&x| (0..h).any(|y| img.pixels[(y * w + x) * 4 + 3] > 24));
    match first {
        Some(x) => (w as f64 * 0.5 - x as f64) / scale as f64,
        None => r.size[0] as f64 * 0.5,
    }
}

// ----- recipes ------------------------------------------------------------

fn channel_intro(cv: &Canvas) -> Vec<Vec<Clip>> {
    let end = cv.len;
    vec![
        vec![backdrop(cv, gradient(0x1b1046, 0xd63a73, 135.0))
            .fx("overlay.bokeh", &[("amount", 0.5), ("speed", 0.3)])
            .done()],
        vec![title(
            TitleSpec {
                letter_spacing: 0.04,
                shadow: 60.0,
                ..text("YOUR CHANNEL", "Poppins Black", 150.0, 0xffffff)
            },
            0.2,
            end,
        )
        .at(0.0, cv.u(-50.0))
        .enter("pop", 0.6)
        .exit("fade", 0.5)
        .done()],
        vec![title(
            TitleSpec {
                style: "pill".into(),
                accent: hex(0xff3d7f),
                ..text("New videos every week", "Poppins SemiBold", 50.0, 0xffffff)
            },
            0.8,
            end,
        )
        .at(0.0, cv.u(120.0))
        .enter("rise", 0.6)
        .exit("fade", 0.5)
        .done()],
        vec![sticker("sparkle.twinkle", 0.5, end)
            .at(cv.right(380.0), cv.top(260.0))
            .scale(70.0)
            .exit("fade", 0.5)
            .done()],
        vec![sticker("sparkle.stars", 0.9, end)
            .at(cv.left(380.0), cv.bottom(250.0))
            .scale(60.0)
            .exit("fade", 0.5)
            .done()],
    ]
}

fn countdown(cv: &Canvas) -> Vec<Vec<Clip>> {
    let number = |n: &str, at: f64| {
        title(
            TitleSpec {
                shadow: 50.0,
                ..text(n, "Anton", 220.0, 0xffffff)
            },
            at,
            at + 1.0,
        )
        .at(0.0, 0.0)
        .enter("zoom_out", 0.35)
        .exit("fade", 0.2)
        .done()
    };
    let go = title(
        TitleSpec {
            shadow: 50.0,
            ..text("GO!", "Anton", 240.0, 0xffe14d)
        },
        3.0,
        cv.len.max(3.5),
    )
    .enter("pop", 0.4)
    .exit("fade", 0.3)
    .done();
    vec![
        vec![backdrop(cv, gradient(0x0f172a, 0x1e3a5f, 90.0))
            .fx("overlay.vignette", &[("amount", 0.6)])
            .done()],
        vec![sticker("shape.ring", 0.0, 3.0)
            .colors(0x2ed8b0, 0xffffff)
            .motion("pulse")
            .scale(210.0)
            .done()],
        vec![number("3", 0.0), number("2", 1.0), number("1", 2.0), go],
        vec![sticker("sparkle.confetti", 3.0, cv.len.max(3.5))
            .scale(260.0)
            .done()],
    ]
}

fn vlog_opener(cv: &Canvas) -> Vec<Vec<Clip>> {
    let end = cv.len;
    vec![
        vec![backdrop(cv, gradient(0xffb347, 0xff5e62, 160.0))
            .fx("light.leak", &[("intensity", 0.5)])
            .done()],
        vec![title(
            TitleSpec {
                shadow: 45.0,
                ..text("Summer Vlog", "Lobster", 170.0, 0xffffff)
            },
            0.3,
            end,
        )
        .at(0.0, cv.u(-40.0))
        .enter("bounce", 1.0)
        .exit("fade", 0.5)
        .done()],
        vec![title(
            TitleSpec {
                style: "pill".into(),
                accent: hex(0x1d3557),
                letter_spacing: 0.08,
                ..text("DAY 01", "Poppins Bold", 60.0, 0xffffff)
            },
            0.9,
            end,
        )
        .at(0.0, cv.u(120.0))
        .enter("slide_up", 0.6)
        .exit("fade", 0.5)
        .done()],
        vec![sticker("emoji.cool", 1.2, end)
            .at(cv.right(330.0), cv.top(250.0))
            .scale(55.0)
            .rotate(12.0)
            .enter("pop", 0.5)
            .exit("fade", 0.4)
            .done()],
    ]
}

fn coming_soon(cv: &Canvas) -> Vec<Vec<Clip>> {
    let end = cv.len;
    vec![
        vec![backdrop(cv, Generator::Solid(hex(0x07070a)))
            .fx("light.leak", &[("intensity", 0.75), ("speed", 0.5)])
            .fx("retro.grain", &[("amount", 0.25)])
            .done()],
        vec![title(
            TitleSpec {
                letter_spacing: 0.3,
                shadow: 40.0,
                ..text("COMING SOON", "Bebas Neue", 190.0, 0xffffff)
            },
            0.4,
            end,
        )
        .at(0.0, cv.u(-20.0))
        .enter("blur", 1.4)
        .exit("fade", 0.8)
        .done()],
        vec![title(
            TitleSpec {
                letter_spacing: 0.5,
                ..text("06 . 21 . 2026", "Poppins SemiBold", 44.0, 0xf5e6c8)
            },
            1.6,
            end,
        )
        .at(0.0, cv.u(120.0))
        .enter("fade", 0.8)
        .exit("fade", 0.8)
        .done()],
    ]
}

fn lower_third(cv: &Canvas) -> Vec<Vec<Clip>> {
    let end = cv.len;
    let (x0, y) = (cv.left(110.0), cv.bottom(190.0));
    let (bar_w, bar_h) = (660.0, 150.0);
    vec![
        vec![piece(Generator::Solid(hex(0x0d1117)), 0.0, end)
            .named("Bar")
            .rect(cv, bar_w, bar_h)
            .at(x0 + cv.u(bar_w * 0.5), y)
            .opacity(88.0)
            .corners(10.0)
            .enter("wipe_right", 0.5)
            .exit("wipe_left", 0.4)
            .done()],
        vec![piece(Generator::Solid(hex(0x2ed8b0)), 0.1, end)
            .named("Accent")
            .rect(cv, 14.0, bar_h)
            .at(x0 + cv.u(7.0), y)
            .enter("wipe_down", 0.35)
            .exit("wipe_up", 0.3)
            .done()],
        vec![title(
            text("Name Surname", "Poppins Bold", 62.0, 0xffffff),
            0.35,
            end,
        )
        .left_at(cv, x0 + cv.u(44.0), y - cv.u(24.0))
        .enter("slide_right", 0.5)
        .exit("fade", 0.3)
        .done()],
        vec![title(
            text("Job Title  ·  Company", "Poppins", 36.0, 0x9fe8d6),
            0.6,
            end,
        )
        .left_at(cv, x0 + cv.u(46.0), y + cv.u(36.0))
        .enter("fade", 0.5)
        .exit("fade", 0.3)
        .done()],
    ]
}

fn breaking_news(cv: &Canvas) -> Vec<Vec<Clip>> {
    let end = cv.len;
    let y = cv.bottom(150.0);
    let full = cv.w / cv.k();
    let tag_w = 470.0;
    let x0 = cv.left(0.0);
    vec![
        vec![piece(Generator::Solid(hex(0x0b0f19)), 0.0, end)
            .named("Banner")
            .rect(cv, full, 110.0)
            .at(0.0, y)
            .opacity(92.0)
            .enter("wipe_right", 0.5)
            .exit("wipe_left", 0.4)
            .done()],
        vec![piece(Generator::Solid(hex(0xe11d2e)), 0.2, end)
            .named("Tag")
            .rect(cv, tag_w, 110.0)
            .at(x0 + cv.u(tag_w * 0.5), y)
            .enter("wipe_right", 0.4)
            .exit("wipe_left", 0.3)
            .done()],
        vec![title(
            TitleSpec {
                letter_spacing: 0.04,
                ..text("BREAKING NEWS", "Poppins Black", 40.0, 0xffffff)
            },
            0.45,
            end,
        )
        .at(x0 + cv.u(tag_w * 0.5), y)
        .enter("fade", 0.3)
        .exit("fade", 0.3)
        .done()],
        vec![title(
            text(
                "Your headline goes here",
                "Poppins SemiBold",
                46.0,
                0xffffff,
            ),
            0.7,
            end,
        )
        .left_at(cv, x0 + cv.u(tag_w + 40.0), y)
        .enter("slide_left", 0.5)
        .exit("fade", 0.3)
        .done()],
        vec![sticker("social.live", 0.9, end)
            .at(cv.right(170.0), y - cv.u(110.0))
            .scale(32.0)
            .enter("pop", 0.4)
            .exit("fade", 0.3)
            .done()],
    ]
}

fn social_post(cv: &Canvas) -> Vec<Vec<Clip>> {
    let end = cv.len;
    let (card_w, card_h) = (640.0, 860.0);
    vec![
        vec![backdrop(cv, gradient(0xa18cd1, 0xfbc2eb, 160.0))
            .fx("particles.bubbles", &[("amount", 0.35), ("speed", 0.3)])
            .done()],
        vec![piece(Generator::Solid(hex(0xffffff)), 0.0, end)
            .named("Card")
            .rect(cv, card_w, card_h)
            .corners(40.0)
            .shadow()
            .enter("rise", 0.6)
            .exit("fade", 0.4)
            .done()],
        vec![title(
            TitleSpec {
                style: "pill".into(),
                accent: hex(0xff3d7f),
                letter_spacing: 0.06,
                ..text("NEW POST", "Poppins Black", 44.0, 0xffffff)
            },
            0.4,
            end,
        )
        .at(0.0, cv.u(-300.0))
        .enter("pop", 0.5)
        .exit("fade", 0.4)
        .done()],
        vec![title(
            TitleSpec {
                line_spacing: 1.1,
                ..text(
                    "Your caption\ngoes right\nhere",
                    "Poppins Black",
                    76.0,
                    0x1f2937,
                )
            },
            0.6,
            end,
        )
        .at(0.0, cv.u(-40.0))
        .enter("rise", 0.6)
        .exit("fade", 0.4)
        .done()],
        vec![sticker("social.like", 1.0, end)
            .at(0.0, cv.u(215.0))
            .scale(42.0)
            .enter("pop", 0.5)
            .exit("fade", 0.4)
            .done()],
        vec![title(
            text("@yourhandle", "Poppins SemiBold", 40.0, 0x6b7280),
            1.2,
            end,
        )
        .at(0.0, cv.u(335.0))
        .enter("fade", 0.5)
        .exit("fade", 0.4)
        .done()],
    ]
}

fn subscribe_reminder(cv: &Canvas) -> Vec<Vec<Clip>> {
    let end = cv.len;
    let y = cv.bottom(190.0);
    vec![
        vec![sticker("social.subscribe", 0.0, end)
            .at(0.0, y)
            .scale(75.0)
            .motion("none")
            .enter("pop", 0.5)
            .exit("zoom_in", 0.4)
            .done()],
        vec![sticker("social.bell", 0.6, end)
            .at(cv.u(510.0), y)
            .scale(55.0)
            .motion("swing")
            .enter("pop", 0.4)
            .exit("fade", 0.3)
            .done()],
        vec![sticker("social.cursor_click", 1.0, end - 0.6)
            .at(cv.u(150.0), y + cv.u(70.0))
            .scale(50.0)
            .enter("slide_up", 0.5)
            .exit("fade", 0.3)
            .done()],
    ]
}

fn social_handle(cv: &Canvas) -> Vec<Vec<Clip>> {
    let end = cv.len;
    let y = cv.bottom(150.0);
    vec![
        vec![title(
            TitleSpec {
                style: "pill".into(),
                accent: hex(0xff3e8a),
                ..text("@yourhandle", "Poppins Bold", 64.0, 0xffffff)
            },
            0.0,
            end,
        )
        .at(cv.right(340.0), y)
        .enter("slide_left", 0.6)
        .exit("slide_right", 0.5)
        .done()],
        vec![sticker("social.follow", 0.5, end)
            .at(cv.right(340.0), y - cv.u(130.0))
            .scale(48.0)
            .enter("pop", 0.5)
            .exit("fade", 0.4)
            .done()],
    ]
}

fn end_screen(cv: &Canvas) -> Vec<Vec<Clip>> {
    let end = cv.len;
    vec![
        vec![backdrop(cv, gradient(0x141e30, 0x243b55, 135.0))
            .fx("overlay.vignette", &[("amount", 0.5)])
            .done()],
        vec![piece(Generator::Solid(hex(0xffffff)), 0.3, end)
            .named("Video placeholder")
            .rect(cv, 760.0, 428.0)
            .at(cv.u(-330.0), cv.u(60.0))
            .opacity(16.0)
            .corners(22.0)
            .enter("pop", 0.5)
            .exit("fade", 0.4)
            .done()],
        vec![title(
            TitleSpec {
                letter_spacing: 0.25,
                ..text("WATCH NEXT", "Poppins Bold", 40.0, 0xffffff)
            },
            0.6,
            end,
        )
        .at(cv.u(-330.0), cv.u(60.0))
        .opacity(70.0)
        .enter("fade", 0.5)
        .exit("fade", 0.4)
        .done()],
        vec![sticker("shape.ring", 0.5, end)
            .colors(0xffffff, 0xffffff)
            .motion("none")
            .at(cv.u(430.0), cv.u(20.0))
            .scale(95.0)
            .enter("pop", 0.5)
            .exit("fade", 0.4)
            .done()],
        vec![title(
            TitleSpec {
                style: "pill".into(),
                accent: hex(0xff1e1e),
                letter_spacing: 0.04,
                ..text("SUBSCRIBE", "Poppins Black", 52.0, 0xffffff)
            },
            0.8,
            end,
        )
        .at(cv.u(430.0), cv.u(250.0))
        .enter("pop", 0.5)
        .exit("fade", 0.4)
        .done()],
        vec![title(
            TitleSpec {
                shadow: 40.0,
                ..text("Thanks for watching!", "Pacifico", 96.0, 0xffffff)
            },
            0.0,
            end,
        )
        .at(0.0, cv.top(175.0))
        .enter("fade", 0.8)
        .exit("fade", 0.5)
        .done()],
    ]
}

fn quote_card(cv: &Canvas) -> Vec<Vec<Clip>> {
    let end = cv.len;
    vec![
        vec![backdrop(cv, gradient(0xf7f0e3, 0xe6d5b8, 90.0))
            .fx("overlay.vignette", &[("amount", 0.18)])
            .done()],
        vec![title(
            text("\u{201C}", "DM Serif Display", 360.0, 0xc9a46a),
            0.0,
            end,
        )
        .at(0.0, cv.u(-250.0))
        .enter("fade", 0.8)
        .exit("fade", 0.6)
        .done()],
        vec![title(
            TitleSpec {
                line_spacing: 1.2,
                ..text(
                    "The best way to predict\nthe future is to create it.",
                    "DM Serif Display",
                    78.0,
                    0x2b2b2b,
                )
            },
            0.5,
            end,
        )
        .at(0.0, cv.u(10.0))
        .enter("typewriter", 2.2)
        .exit("fade", 0.6)
        .done()],
        vec![title(
            TitleSpec {
                letter_spacing: 0.12,
                ..text("AUTHOR NAME", "Poppins SemiBold", 36.0, 0x8a6d3b)
            },
            2.8,
            end,
        )
        .at(0.0, cv.u(200.0))
        .enter("rise", 0.6)
        .exit("fade", 0.6)
        .done()],
    ]
}

fn chapter_title(cv: &Canvas) -> Vec<Vec<Clip>> {
    let end = cv.len;
    vec![
        vec![backdrop(cv, gradient(0x121212, 0x2a2a2a, 90.0))
            .fx("overlay.vignette", &[("amount", 0.8)])
            .fx("retro.grain", &[("amount", 0.3)])
            .done()],
        vec![title(
            TitleSpec {
                letter_spacing: 0.45,
                ..text("CHAPTER ONE", "Poppins SemiBold", 40.0, 0xe8c27a)
            },
            0.2,
            end,
        )
        .at(0.0, cv.u(-125.0))
        .enter("fade", 0.8)
        .exit("fade", 0.6)
        .done()],
        vec![sticker("line.divider", 0.5, end)
            .colors(0xe8c27a, 0xe8c27a)
            .motion("none")
            .at(0.0, cv.u(-62.0))
            .scale(45.0)
            .enter("wipe_right", 0.7)
            .exit("fade", 0.6)
            .done()],
        vec![title(
            TitleSpec {
                shadow: 40.0,
                ..text("The Beginning", "DM Serif Display", 130.0, 0xffffff)
            },
            0.6,
            end,
        )
        .at(0.0, cv.u(45.0))
        .enter("blur", 1.0)
        .exit("fade", 0.6)
        .done()],
    ]
}

fn sale_banner(cv: &Canvas) -> Vec<Vec<Clip>> {
    let end = cv.len;
    let (bx, by) = (cv.u(420.0), cv.u(-20.0));
    vec![
        vec![backdrop(cv, gradient(0xff416c, 0xff9a44, 45.0))
            .fx("particles.confetti", &[("amount", 0.35)])
            .done()],
        vec![sticker("shape.burst", 0.3, end)
            .colors(0xffe14d, 0xffffff)
            .motion("pulse")
            .at(bx, by)
            .scale(150.0)
            .enter("spin", 0.6)
            .exit("fade", 0.4)
            .done()],
        vec![title(
            TitleSpec {
                align: TextAlign::Center,
                line_spacing: 0.95,
                ..text("50%\nOFF", "Poppins Black", 96.0, 0xd6204e)
            },
            0.6,
            end,
        )
        .at(bx, by)
        .enter("pop", 0.5)
        .exit("fade", 0.4)
        .done()],
        vec![title(
            TitleSpec {
                shadow: 55.0,
                letter_spacing: 0.02,
                ..text("BIG SALE", "Anton", 210.0, 0xffffff)
            },
            0.0,
            end,
        )
        .at(cv.u(-260.0), cv.u(-60.0))
        .enter("slide_right", 0.6)
        .exit("fade", 0.4)
        .done()],
        vec![title(
            TitleSpec {
                style: "pill".into(),
                accent: hex(0x111111),
                letter_spacing: 0.08,
                ..text("SHOP NOW", "Poppins Bold", 48.0, 0xffffff)
            },
            0.8,
            end,
        )
        .at(cv.u(-260.0), cv.u(150.0))
        .enter("rise", 0.5)
        .exit("fade", 0.4)
        .done()],
    ]
}

// ----- placing ------------------------------------------------------------

/// Why a template couldn't be placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceError {
    Locked,
    /// Every video track is in use at that spot.
    NoRoom,
}

/// Seconds a template lasts in this project.
pub fn length(t: &Template, still_secs: f64) -> f64 {
    if t.length > 0.0 {
        t.length
    } else {
        still_secs.max(0.1)
    }
}

/// The span `lane` covers, relative to the template start.
fn lane_span(lane: &[Clip]) -> (Time, Time) {
    let s = lane.iter().map(|c| c.start).min().unwrap_or(Time::ZERO);
    let e = lane.iter().map(Clip::end).max().unwrap_or(Time::ZERO);
    (s, e)
}

fn track_index(p: &Project, id: TrackId) -> Option<usize> {
    p.tracks.iter().position(|t| t.id == id)
}

/// A video track above track `below` (an index) with room for `start..start+len`:
/// the one right above when it's free, else a new one right above, else
/// any free one higher up.
fn track_above(p: &mut Project, below: usize, start: Time, len: Time) -> Option<TrackId> {
    let usable = |p: &Project, i: usize| {
        let t = &p.tracks[i];
        t.kind == TrackKind::Video && !t.locked && t.is_free(start, len, None)
    };
    if below > 0 && usable(p, below - 1) {
        return Some(p.tracks[below - 1].id);
    }
    if let Some(id) = p.insert_track(TrackKind::Video, below) {
        return Some(id);
    }
    (0..below)
        .rev()
        .find(|&i| usable(p, i))
        .map(|i| p.tracks[i].id)
}

/// Places a template's clips: the bottom lane on `track` (or on a new top
/// video track when `None`) at `at`, every further lane on a track above
/// the one before, adding tracks as needed. On the magnetic main track the
/// bottom lane is inserted between clips like any clip, and the rest
/// follows it. On failure the project may be half changed; callers keep a
/// copy to restore.
pub fn place(
    p: &mut Project,
    t: &Template,
    len_secs: f64,
    track: Option<TrackId>,
    at: Time,
) -> Result<Vec<ClipId>, PlaceError> {
    let cv = Canvas::new(p, len_secs);
    let lanes: Vec<Vec<Clip>> = (t.build)(&cv)
        .into_iter()
        .filter(|l| !l.is_empty())
        .collect();
    let at = at.max(Time::ZERO);
    let Some(first) = lanes.first() else {
        return Ok(Vec::new());
    };
    let (s0, e0) = lane_span(first);
    let main = p.main_track();
    let base = match track {
        None => p
            .insert_track(TrackKind::Video, 0)
            .ok_or(PlaceError::NoRoom)?,
        Some(id) if t.overlay && Some(id) == main => {
            let i = track_index(p, id).ok_or(PlaceError::Locked)?;
            track_above(p, i, at + s0, e0 - s0).ok_or(PlaceError::NoRoom)?
        }
        Some(id) => id,
    };
    let base_track = p.track(base).ok_or(PlaceError::Locked)?;
    if base_track.locked || base_track.kind != TrackKind::Video {
        return Err(PlaceError::Locked);
    }
    let mut placed = Vec::new();
    // Bottom lane: where it lands decides where everything else goes.
    let origin = if p.is_magnetic(base) {
        let mut origin = None;
        for c in first.clone() {
            let rel = c.start;
            let want = origin.map(|o| o + rel).unwrap_or(at + rel);
            let id = p.place_clip(c, base, want).ok_or(PlaceError::Locked)?;
            if origin.is_none() {
                let got = p.clip(id).map(|(_, c)| c.start).unwrap_or(want);
                origin = Some((got - rel).max(Time::ZERO));
            }
            placed.push(id);
        }
        origin.unwrap_or(at)
    } else {
        let tr = p.track(base).ok_or(PlaceError::Locked)?;
        let origin = (tr.free_slot(at + s0, e0 - s0, None) - s0).max(Time::ZERO);
        for c in first.clone() {
            let rel = c.start;
            placed.push(
                p.place_clip(c, base, origin + rel)
                    .ok_or(PlaceError::Locked)?,
            );
        }
        origin
    };
    let mut below = base;
    for lane in &lanes[1..] {
        let (s, e) = lane_span(lane);
        let i = track_index(p, below).ok_or(PlaceError::Locked)?;
        let tr = track_above(p, i, origin + s, e - s).ok_or(PlaceError::NoRoom)?;
        for c in lane.clone() {
            let rel = c.start;
            placed.push(
                p.place_clip(c, tr, origin + rel)
                    .ok_or(PlaceError::Locked)?,
            );
        }
        below = tr;
    }
    Ok(placed)
}

/// A project showing just the template, for its library picture.
pub fn preview_project(t: &Template, still_secs: f64) -> Project {
    let mut p = Project::new("Template");
    p.settings.magnetic = false;
    let main = p.main_track();
    let _ = place(&mut p, t, length(t, still_secs), main, Time::ZERO);
    p
}

impl TidyCutsApp {
    /// How long template `index` lasts when added.
    pub fn template_length(&self, index: usize) -> Time {
        let secs = get(index)
            .map(|t| length(t, self.prefs.image_duration))
            .unwrap_or(self.prefs.image_duration);
        Time::from_secs_f64(secs)
    }

    /// Adds template `index` as one undo step: at `at` on a track (a new
    /// top track when the track is `None`), or at the playhead on the main
    /// track.
    pub fn add_template(&mut self, index: usize, at: Option<(Option<TrackId>, Time)>) {
        let Some(t) = get(index) else {
            return;
        };
        let (track, time) = match at {
            Some(x) => x,
            None => (self.project.main_track(), self.playhead()),
        };
        let len = length(t, self.prefs.image_duration);
        let before = self.project.clone();
        match place(&mut self.project, t, len, track, time) {
            Ok(ids) if !ids.is_empty() => {
                self.record(before);
                self.selection = ids;
            }
            Ok(_) => self.project = before,
            Err(e) => {
                self.project = before;
                let msg = match e {
                    PlaceError::Locked => "That track is locked.".to_string(),
                    PlaceError::NoRoom => format!(
                        "\u{201C}{}\u{201D} needs more free video tracks there \
                         (a project can have up to {MAX_VIDEO_TRACKS}).",
                        t.name
                    ),
                };
                self.notify(&msg, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tidycuts_render::catalog::{animations, effects, filters, titles};

    fn check_project(p: &Project) {
        assert!(p.track_count(TrackKind::Video) <= MAX_VIDEO_TRACKS);
        for tr in &p.tracks {
            for w in tr.clips.windows(2) {
                assert!(w[0].start <= w[1].start, "unsorted on {}", tr.name);
                assert!(
                    w[0].end() <= w[1].start,
                    "overlap on {}: {:?} {:?}",
                    tr.name,
                    w[0].generator,
                    w[1].generator
                );
            }
            for c in &tr.clips {
                assert_eq!(tr.kind, TrackKind::Video);
                assert!(c.duration() > Time::ZERO);
                assert!(c.start >= Time::ZERO);
                let g = c
                    .generator
                    .as_ref()
                    .expect("templates only make generated clips");
                match g {
                    Generator::Title(s) => {
                        assert!(!s.text.trim().is_empty());
                        assert!(
                            tidycuts_render::fonts::FONTS
                                .iter()
                                .any(|f| f.name == s.font),
                            "unknown font {}",
                            s.font
                        );
                        assert!(
                            tidycuts_render::text::STYLES
                                .iter()
                                .any(|(id, _)| *id == s.style),
                            "unknown style {}",
                            s.style
                        );
                    }
                    Generator::Sticker(s) => {
                        assert!(stickers::get(&s.kind).is_some(), "sticker {}", s.kind);
                        assert!(
                            animations::STICKER_MOTIONS
                                .iter()
                                .any(|(id, _)| *id == s.motion),
                            "motion {}",
                            s.motion
                        );
                    }
                    _ => {}
                }
                for e in &c.effects {
                    if filters::is_filter(&e.kind) {
                        assert!(filters::get(&e.kind).is_some(), "filter {}", e.kind);
                    } else {
                        let def =
                            effects::get(&e.kind).unwrap_or_else(|| panic!("effect {}", e.kind));
                        for k in e.params.keys() {
                            assert!(
                                def.params.iter().any(|p| p.key == k),
                                "{} has no parameter {k}",
                                e.kind
                            );
                        }
                    }
                }
                for a in [&c.anim_in, &c.anim_out].into_iter().flatten() {
                    assert!(animations::get(&a.kind).is_some(), "animation {}", a.kind);
                    assert!(a.duration <= c.duration());
                }
            }
        }
        let _ = titles::TITLES.len();
    }

    #[test]
    fn every_template_places_valid_clips_on_a_fresh_project() {
        for (i, t) in TEMPLATES.iter().enumerate() {
            let mut p = Project::default();
            let main = p.main_track();
            let ids = place(&mut p, t, length(t, 5.0), main, Time::from_secs_f64(2.0))
                .unwrap_or_else(|e| panic!("{} failed: {e:?}", t.name));
            let lanes = (t.build)(&Canvas::new(&p, length(t, 5.0)));
            let n: usize = lanes.iter().map(Vec::len).sum();
            assert_eq!(ids.len(), n, "{}", t.name);
            assert!(!lanes.is_empty(), "{}", t.name);
            assert!(lanes[0].iter().any(|c| c.start == Time::ZERO), "{}", t.name);
            for id in &ids {
                assert!(p.clip(*id).is_some(), "{}", t.name);
            }
            // Lanes stack upwards in order.
            let rows: Vec<usize> = lanes
                .iter()
                .scan(0usize, |k, l| {
                    let first = ids[*k];
                    *k += l.len();
                    Some(
                        p.tracks
                            .iter()
                            .position(|tr| tr.clips.iter().any(|c| c.id == first))
                            .unwrap(),
                    )
                })
                .collect();
            assert!(rows.windows(2).all(|w| w[1] < w[0]), "{}: {rows:?}", t.name);
            // Overlays stay off the main track; full-frame ones start it.
            let on_main = p
                .track(main.unwrap())
                .unwrap()
                .clips
                .iter()
                .any(|c| ids.contains(&c.id));
            assert_eq!(on_main, !t.overlay, "{}", t.name);
            check_project(&p);
            // The poster moment is inside the template.
            assert!(t.poster < length(t, 5.0), "{}", t.name);
            let _ = i;
        }
    }

    #[test]
    fn templates_stack_over_existing_clips_and_follow_the_magnetic_track() {
        let mut p = Project::default();
        let main = p.main_track().unwrap();
        p.add_generated(
            Generator::Solid([0.5; 4]),
            Time::from_secs_f64(10.0),
            main,
            Time::ZERO,
        )
        .unwrap();
        for t in TEMPLATES {
            let before = p.clone();
            match place(
                &mut p,
                t,
                length(t, 5.0),
                Some(main),
                Time::from_secs_f64(3.0),
            ) {
                Ok(_) => check_project(&p),
                // Running out of tracks is fine as long as nothing breaks.
                Err(PlaceError::NoRoom) => p = before,
                Err(e) => panic!("{}: {e:?}", t.name),
            }
        }
        check_project(&p);
    }

    #[test]
    fn a_drop_above_the_tracks_adds_a_new_top_track() {
        let mut p = Project::default();
        let t = TEMPLATES.iter().find(|t| t.name == "Lower Third").unwrap();
        let ids = place(&mut p, t, t.length, None, Time::from_secs_f64(1.5)).unwrap();
        check_project(&p);
        let first = p.clip(ids[0]).unwrap().1;
        assert_eq!(first.start, Time::from_secs_f64(1.5));
        // The main track stays empty.
        assert!(p.track(p.main_track().unwrap()).unwrap().clips.is_empty());
    }

    #[test]
    fn titles_are_left_aligned_by_their_lettering() {
        let spec = text("Name Surname", "Poppins Bold", 62.0, 0xffffff);
        let ink = ink_left(&spec);
        assert!(ink > 100.0 && ink < 600.0, "{ink}");
    }
}
