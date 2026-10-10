//! What Claude can do: the tools the MCP server offers, run on the
//! window's thread against the same editing operations the buttons use, so
//! undo, unsaved-change tracking and the preview all work for Claude's
//! edits too. Times are in seconds on the timeline.

use crate::app::TidyCutsApp;
use crate::detection::Detection;
use crate::mcp::Job;
use crate::timeline::clip_name;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use tidycuts_core::{
    Clip, ClipId, Edge, Generator, MediaId, MediaKind, RampPreset, SpeedRamp, Time, Track, TrackId,
    TrackKind,
};
use tidycuts_engine::export::{Container, ExportSettings, Quality};
use tidycuts_mcp::{Tool, ToolResult};
use uuid::Uuid;

/// Shown to Claude when it connects.
pub const INSTRUCTIONS: &str = "\
This is a video editor. You are editing the project that is open in its window; the person \
can watch every change and undo it.

How to work:
1. Call get_project first. It lists the media (with ids), the tracks and every clip on them.
2. Make changes with the tools. Every id is the long id from get_project (a unique start of \
it, at least 6 characters, also works).
3. Look at the result with render_frame, and fix what is wrong. Don't assume an edit worked \
the way you meant: check.

Conventions: all times are seconds on the timeline (a clip's start and duration) unless a \
tool says 'source'. Tracks are listed top to bottom; a clip on a higher video track draws on \
top of the ones below. The first video track is the magnetic main track: cutting or deleting \
there closes the gap. A project has at most 10 video tracks and 5 audio tracks.

Use list_catalog to find the ids of titles, transitions, effects, filters, stickers, \
layouts, visualizers and LUTs. Use get_clip to see every setting of a clip, and update_clip \
to change them (a JSON merge patch). Everything can be undone with undo.";

pub fn tool_list() -> Vec<Tool> {
    let id = |what: &str| json!({"type": "string", "description": format!("The id of {what}.")});
    let secs = |what: &str| json!({"type": "number", "description": what});
    vec![
        tool(
            "get_project",
            "The whole project: settings, the media library with ids, every track with its \
             clips (id, kind, start, duration, source), markers, the playhead and what is selected.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "get_clip",
            "Every setting of one clip: transform, crop, color, effects, audio, animation, speed, text.",
            json!({"clip": id("the clip")}),
            &["clip"],
            true,
        ),
        tool(
            "list_catalog",
            "The ids you can use for titles, transitions, effects, filters, stickers, layouts, \
             visualizers, luts, animations or ramps.",
            json!({"kind": {"type": "string", "enum": CATALOGS}}),
            &["kind"],
            true,
        ),
        tool(
            "render_frame",
            "A picture of the finished video at one moment, exactly as it will export. Use it \
             to check your work.",
            json!({
                "time": secs("Seconds on the timeline. Default: the playhead."),
                "width": {"type": "integer", "description": "Picture width in pixels (default 960, at most 1920)."},
            }),
            &[],
            true,
        ),
        tool(
            "make_thumbnail",
            "Makes a thumbnail picture (PNG or JPG file) from a video frame, a photo or the \
             timeline, with big words on it, and shows it to you. Takes a few seconds.",
            json!({
                "text": {"type": "string", "description": "The words on the thumbnail."},
                "media": id("a library video or picture to use (default: the timeline at 'time')"),
                "time": secs("Which moment of the video or timeline (default: near the start, or the playhead)."),
                "style": {"type": "string", "enum": ["bold", "yellow", "pop", "clean", "box", "marker"], "description": "How the words look (default bold)."},
                "across": {"type": "string", "enum": ["left", "center", "right"], "description": "Where the words sit sideways (default left)."},
                "down": {"type": "string", "enum": ["top", "middle", "bottom"], "description": "Where the words sit up and down (default bottom)."},
                "shape": {"type": "string", "enum": ["widescreen", "square", "vertical"], "description": "The picture shape (default widescreen, 1280 x 720)."},
                "size": {"type": "number", "description": "Word size in percent, 40 to 160 (default 100)."},
                "punch": {"type": "number", "description": "Extra colour and contrast for the picture, 0 to 100 (default 40)."},
                "fade": {"type": "number", "description": "Dark fade behind the words, 0 to 100 (default 60)."},
                "path": {"type": "string", "description": "Where to save it, ending in .png or .jpg (default: a new file in the project's pictures folder)."},
            }),
            &[],
            false,
        ),
        tool(
            "import_media",
            "Adds video, audio or picture files from disk to the media library. Returns their ids.",
            json!({"paths": {"type": "array", "items": {"type": "string"}, "description": "Full file paths."}}),
            &["paths"],
            false,
        ),
        tool(
            "add_to_timeline",
            "Puts a library item on the timeline. Without track and start it goes on the end of \
             its usual track (video on the main track).",
            json!({
                "media": id("the media item"),
                "track": id("the track (optional)"),
                "start": secs("Where it starts (optional)."),
            }),
            &["media"],
            false,
        ),
        tool(
            "add_title",
            "Adds a title (text) on an overlay track. Pick a look with preset (see list_catalog titles).",
            json!({
                "text": {"type": "string", "description": "The words to show."},
                "preset": {"type": "string", "description": "A titles id from list_catalog."},
                "start": secs("Default: the playhead."),
                "duration": secs("Default: the preset's length."),
            }),
            &[],
            false,
        ),
        tool(
            "add_layout",
            "Lays out a split-screen layout: one empty cell per video track. Fill the cells with fill_cell.",
            json!({"layout": {"type": "string", "description": "A layouts id from list_catalog."}, "start": secs("Default: the playhead.")}),
            &["layout"],
            false,
        ),
        tool(
            "fill_cell",
            "Puts media into an empty split-screen cell.",
            json!({"clip": id("the empty cell"), "media": id("the media item")}),
            &["clip", "media"],
            false,
        ),
        tool(
            "add_visualizer",
            "Adds an audio visualizer (bars, line, circle...) that moves with the sound.",
            json!({"style": {"type": "string", "description": "A visualizers id from list_catalog."}, "start": secs("Default: the playhead.")}),
            &["style"],
            false,
        ),
        tool(
            "split_clip",
            "Cuts a clip in two at a moment on the timeline (the moment must be inside the clip).",
            json!({"clip": id("the clip"), "at": secs("Seconds on the timeline.")}),
            &["clip", "at"],
            false,
        ),
        tool(
            "delete_clips",
            "Deletes clips. With ripple, later clips on the track move left to close the gap.",
            json!({"clips": {"type": "array", "items": {"type": "string"}}, "ripple": {"type": "boolean"}}),
            &["clips"],
            false,
        ),
        tool(
            "move_clip",
            "Moves a clip to a new start time, optionally to another track of the same kind.",
            json!({"clip": id("the clip"), "start": secs("New start, seconds."), "track": id("another track (optional)")}),
            &["clip", "start"],
            false,
        ),
        tool(
            "trim_clip",
            "Drags one end of a clip to a moment on the timeline, hiding or revealing footage.",
            json!({"clip": id("the clip"), "edge": {"type": "string", "enum": ["start", "end"]}, "to": secs("Seconds on the timeline.")}),
            &["clip", "edge", "to"],
            false,
        ),
        tool(
            "update_clip",
            "Changes settings of a clip with a JSON merge patch over what get_clip shows, for \
             example {\"transform\":{\"scale_x\":{\"value\":150}}} or, for a title, \
             {\"generator\":{\"Title\":{\"text\":\"Hello\"}}}. Objects merge, null removes a \
             setting, arrays replace. Not allowed here: id, media_id, start, source_in, \
             source_out, speed, ramp (use move_clip, trim_clip and set_speed).",
            json!({"clip": id("the clip"), "patch": {"type": "object"}}),
            &["clip", "patch"],
            false,
        ),
        tool(
            "apply_effect",
            "Adds an effect or filter (see list_catalog) to clips. With no clips it becomes an \
             adjustment clip over everything below it.",
            json!({"effect": {"type": "string"}, "clips": {"type": "array", "items": {"type": "string"}}}),
            &["effect"],
            false,
        ),
        tool(
            "set_transition",
            "Sets the transition at the start (or, with at_end, the end) of a clip.",
            json!({"clip": id("the clip"), "transition": {"type": "string", "description": "A transitions id."}, "at_end": {"type": "boolean"}}),
            &["clip", "transition"],
            false,
        ),
        tool(
            "set_speed",
            "Changes a clip's speed: a steady speed (1 = normal, 0.5 = half) or a ramp preset \
             (see list_catalog ramps; 'none' removes a ramp). The clip's length changes and later \
             clips on its track move.",
            json!({"clip": id("the clip"), "speed": {"type": "number"}, "ramp": {"type": "string"}}),
            &["clip"],
            false,
        ),
        tool(
            "detach_audio",
            "Splits a video clip's sound onto its own audio track.",
            json!({"clip": id("the video clip")}),
            &["clip"],
            false,
        ),
        tool(
            "freeze_frame",
            "Holds the picture of a video clip at one moment for five seconds.",
            json!({"clip": id("the video clip"), "at": secs("Seconds on the timeline.")}),
            &["clip", "at"],
            false,
        ),
        tool(
            "set_mix",
            "The audio mixer: a track's volume (percent, 100 = unchanged, up to 200), mute and \
             solo, or the master volume when no track is given.",
            json!({
                "track": id("the track (leave out for the master volume)"),
                "volume": {"type": "number", "description": "Percent, 0 to 200."},
                "muted": {"type": "boolean"},
                "solo": {"type": "boolean", "description": "Only soloed tracks play while any is soloed."},
            }),
            &[],
            false,
        ),
        tool(
            "sync_by_audio",
            "Lines clips up by their sound (several cameras, or a camera and a recorder, that \
             heard the same thing). The earliest clip stays; the others move. Clips need sound \
             at normal speed.",
            json!({"clips": {"type": "array", "items": {"type": "string"}, "description": "Two or more clip ids."}}),
            &["clips"],
            false,
        ),
        tool(
            "make_proxy",
            "Starts making a small copy (proxy) of a big video so editing is smooth. It works in \
             the background; get_project shows the proxy once it is ready. Exports always use \
             the original.",
            json!({"media": id("the media item")}),
            &["media"],
            false,
        ),
        tool(
            "add_voiceover",
            "Turns text into speech with the computer's own voices and puts it on an audio \
             track. Takes a few seconds.",
            json!({
                "text": {"type": "string", "description": "What the voice says."},
                "voice": {"type": "string", "description": "A voice id from list_voices (optional)."},
                "speed": {"type": "number", "description": "0.5 to 2.0, 1 is normal."},
                "start": secs("Where it starts (default: the playhead)."),
            }),
            &["text"],
            false,
        ),
        tool(
            "list_voices",
            "The voices that add_voiceover can use on this computer.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "remove_silence",
            "Cuts the silent stretches out of a clip and closes the gaps. May take a few \
             seconds on a long clip.",
            json!({
                "clip": id("the clip"),
                "level_db": {"type": "number", "description": "Quieter than this counts as silence (default -35, range -60 to -15)."},
                "min_pause": secs("Pauses at least this long are cut (default 0.5)."),
            }),
            &["clip"],
            false,
        ),
        tool(
            "split_at_scenes",
            "Cuts a video clip wherever the picture changes scene. May take a few seconds on a \
             long clip.",
            json!({
                "clip": id("the clip"),
                "threshold": {"type": "number", "description": "0.05 to 0.9; lower finds more changes (default 0.3)."},
            }),
            &["clip"],
            false,
        ),
        tool(
            "add_beat_markers",
            "Puts a marker on every beat of a clip's sound.",
            json!({"clip": id("the clip")}),
            &["clip"],
            false,
        ),
        tool(
            "add_marker",
            "Puts a marker on the timeline ruler.",
            json!({"time": secs("Seconds."), "name": {"type": "string"}, "note": {"type": "string"}}),
            &["time"],
            false,
        ),
        tool(
            "set_playhead",
            "Moves the playhead.",
            json!({"time": secs("Seconds.")}),
            &["time"],
            false,
        ),
        tool("undo", "Undoes the last change.", json!({}), &[], false),
        tool("redo", "Redoes the change that was undone.", json!({}), &[], false),
        tool(
            "save_project",
            "Saves the project. Give a path the first time (a .tcproj file).",
            json!({"path": {"type": "string"}}),
            &[],
            false,
        ),
        tool(
            "open_project",
            "Opens a saved project. Refuses if there are unsaved changes unless discard_changes is true.",
            json!({"path": {"type": "string"}, "discard_changes": {"type": "boolean"}}),
            &["path"],
            false,
        ),
        tool(
            "export_video",
            "Renders the project to a file (the format follows the extension: .mp4, .mov, .gif, \
             .mp3...). It runs in the background; check export_status.",
            json!({
                "path": {"type": "string", "description": "Where to write the file."},
                "width": {"type": "integer"},
                "start": secs("Export only from here (optional)."),
                "end": secs("...to here (optional)."),
                "quality": {"type": "string", "enum": ["higher", "recommended", "lower"]},
            }),
            &["path"],
            false,
        ),
        tool(
            "export_status",
            "How the export is going, or how the last one ended.",
            json!({}),
            &[],
            true,
        ),
    ]
}

const CATALOGS: [&str; 10] = [
    "titles",
    "transitions",
    "effects",
    "filters",
    "stickers",
    "layouts",
    "visualizers",
    "luts",
    "animations",
    "ramps",
];

fn tool(name: &str, description: &str, props: Value, required: &[&str], read_only: bool) -> Tool {
    Tool {
        name: name.into(),
        description: description.into(),
        schema: json!({"type": "object", "properties": props, "required": required}),
        read_only,
    }
}

// ----- running ------------------------------------------------------------------

/// Runs one call on the window's thread and sends its answer.
pub fn run(app: &mut TidyCutsApp, job: Job) {
    let Job { tool, args, reply } = job;
    if tool == "render_frame" || tool == "make_thumbnail" {
        // Drawing takes a moment: a worker answers when it is done.
        let started = if tool == "render_frame" {
            render_frame(app, &Args(&args), reply.clone())
        } else {
            make_thumbnail(app, &Args(&args), reply.clone())
        };
        if let Err(e) = started {
            let _ = reply.send(ToolResult::error(e));
        }
        return;
    }
    let result = dispatch(app, &tool, &Args(&args)).unwrap_or_else(ToolResult::error);
    let _ = reply.send(result);
}

type Out = Result<ToolResult, String>;

fn dispatch(app: &mut TidyCutsApp, tool: &str, a: &Args) -> Out {
    match tool {
        "get_project" => Ok(ToolResult::json(&project_json(app))),
        "get_clip" => get_clip(app, a),
        "list_catalog" => list_catalog(a),
        "import_media" => import_media(app, a),
        "add_to_timeline" => add_to_timeline(app, a),
        "add_title" => add_title(app, a),
        "add_layout" => add_layout(app, a),
        "fill_cell" => fill_cell(app, a),
        "add_visualizer" => add_visualizer(app, a),
        "split_clip" => split_clip(app, a),
        "delete_clips" => delete_clips(app, a),
        "move_clip" => move_clip(app, a),
        "trim_clip" => trim_clip(app, a),
        "update_clip" => update_clip(app, a),
        "apply_effect" => apply_effect(app, a),
        "set_transition" => set_transition(app, a),
        "set_speed" => set_speed(app, a),
        "detach_audio" => detach_audio(app, a),
        "freeze_frame" => freeze_frame(app, a),
        "add_marker" => add_marker(app, a),
        "set_mix" => set_mix(app, a),
        "sync_by_audio" => sync_by_audio(app, a),
        "make_proxy" => make_proxy(app, a),
        "add_voiceover" => add_voiceover(app, a),
        "list_voices" => list_voices(),
        "remove_silence" => remove_silence(app, a),
        "split_at_scenes" => split_at_scenes(app, a),
        "add_beat_markers" => add_beat_markers(app, a),
        "set_playhead" => {
            let t = a.time("time")?;
            app.seek(t);
            Ok(done(format!("Playhead at {:.3} s.", secs(t))))
        }
        "undo" => {
            if !app.can_undo() {
                return Err("There is nothing to undo.".into());
            }
            app.undo();
            Ok(done("Undone."))
        }
        "redo" => {
            if !app.can_redo() {
                return Err("There is nothing to redo.".into());
            }
            app.redo();
            Ok(done("Redone."))
        }
        "save_project" => save_project(app, a),
        "open_project" => open_project(app, a),
        "export_video" => export_video(app, a),
        "export_status" => Ok(ToolResult::json(&export_status(app))),
        other => Err(format!("Unknown tool: {other}")),
    }
}

fn done(text: impl Into<String>) -> ToolResult {
    ToolResult::text(text)
}

// ----- arguments -----------------------------------------------------------------

struct Args<'a>(&'a Value);

impl Args<'_> {
    fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key).filter(|v| !v.is_null())
    }

    fn str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    fn need_str(&self, key: &str) -> Result<&str, String> {
        self.str(key)
            .ok_or_else(|| format!("'{key}' is needed (a string)."))
    }

    fn num(&self, key: &str) -> Result<Option<f64>, String> {
        match self.get(key) {
            None => Ok(None),
            Some(v) => v
                .as_f64()
                .filter(|n| n.is_finite())
                .map(Some)
                .ok_or_else(|| format!("'{key}' must be a number.")),
        }
    }

    fn need_num(&self, key: &str) -> Result<f64, String> {
        self.num(key)?
            .ok_or_else(|| format!("'{key}' is needed (a number)."))
    }

    fn bool(&self, key: &str) -> bool {
        self.get(key).and_then(Value::as_bool).unwrap_or(false)
    }

    fn time(&self, key: &str) -> Result<Time, String> {
        Ok(Time::from_secs_f64(self.need_num(key)?.max(0.0)))
    }

    fn opt_time(&self, key: &str) -> Result<Option<Time>, String> {
        Ok(self.num(key)?.map(|s| Time::from_secs_f64(s.max(0.0))))
    }

    fn strings(&self, key: &str) -> Result<Vec<&str>, String> {
        match self.get(key) {
            None => Ok(Vec::new()),
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| {
                    v.as_str()
                        .ok_or_else(|| format!("'{key}' must be a list of strings."))
                })
                .collect(),
            Some(_) => Err(format!("'{key}' must be a list of strings.")),
        }
    }
}

fn secs(t: Time) -> f64 {
    (t.as_secs_f64() * 1000.0).round() / 1000.0
}

/// An id given in full or as a unique start of one.
fn resolve(what: &str, given: &str, ids: impl Iterator<Item = Uuid>) -> Result<Uuid, String> {
    let given = given.trim().to_ascii_lowercase();
    let all: Vec<Uuid> = ids.collect();
    if let Ok(id) = Uuid::parse_str(&given) {
        return all
            .contains(&id)
            .then_some(id)
            .ok_or_else(|| format!("There is no {what} with id {given}. Call get_project."));
    }
    let simple = given.replace('-', "");
    let hits: Vec<Uuid> = all
        .into_iter()
        .filter(|id| simple.len() >= 6 && id.simple().to_string().starts_with(&simple))
        .collect();
    match hits.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!(
            "There is no {what} with id {given}. Call get_project to see the ids."
        )),
        _ => Err(format!(
            "'{given}' matches several {what}s; give more of the id."
        )),
    }
}

fn clip_id(app: &TidyCutsApp, given: &str) -> Result<ClipId, String> {
    resolve(
        "clip",
        given,
        app.project
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(|c| c.id)),
    )
}

fn track_id(app: &TidyCutsApp, given: &str) -> Result<TrackId, String> {
    resolve("track", given, app.project.tracks.iter().map(|t| t.id))
}

fn media_id(app: &TidyCutsApp, given: &str) -> Result<MediaId, String> {
    resolve("media item", given, app.project.media.iter().map(|m| m.id))
}

fn all_clip_ids(app: &TidyCutsApp) -> HashSet<ClipId> {
    app.project
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(|c| c.id))
        .collect()
}

/// A short description of the clips that exist now and didn't before.
fn created_since(app: &TidyCutsApp, before: &HashSet<ClipId>) -> Value {
    let mut out = Vec::new();
    for t in &app.project.tracks {
        for c in t.clips.iter().filter(|c| !before.contains(&c.id)) {
            out.push(clip_json(app, t, c));
        }
    }
    Value::Array(out)
}

// ----- describing the project ---------------------------------------------------------

fn kind_name(app: &TidyCutsApp, c: &Clip) -> &'static str {
    match &c.generator {
        Some(Generator::Title(_)) => "title",
        Some(Generator::Sticker(_)) => "sticker",
        Some(Generator::Solid(_)) | Some(Generator::Gradient { .. }) => "color",
        Some(Generator::Adjustment) => "adjustment",
        Some(Generator::Placeholder) => "empty_cell",
        Some(Generator::Visualizer(_)) => "visualizer",
        None => match c
            .media_id
            .and_then(|m| app.project.media(m))
            .map(|m| m.kind)
        {
            Some(MediaKind::Video) => "video",
            Some(MediaKind::Image) => "image",
            Some(MediaKind::Audio) => "audio",
            None => "unknown",
        },
    }
}

fn clip_json(app: &TidyCutsApp, track: &Track, c: &Clip) -> Value {
    let mut o = Map::new();
    o.insert("id".into(), json!(c.id));
    o.insert("name".into(), json!(clip_name(app, c)));
    o.insert("kind".into(), json!(kind_name(app, c)));
    o.insert("track".into(), json!(track.id));
    o.insert("start".into(), json!(secs(c.start)));
    o.insert("duration".into(), json!(secs(c.duration())));
    o.insert("end".into(), json!(secs(c.end())));
    if let Some(m) = c.media_id {
        o.insert("media".into(), json!(m));
        o.insert("source_in".into(), json!(secs(c.source_in)));
        o.insert("source_out".into(), json!(secs(c.source_out)));
    }
    if c.ramp.is_some() {
        o.insert("speed_ramp".into(), json!(true));
    } else if (c.speed - 1.0).abs() > 1e-9 {
        o.insert("speed".into(), json!(c.speed));
    }
    if c.reverse {
        o.insert("reverse".into(), json!(true));
    }
    if c.disabled {
        o.insert("disabled".into(), json!(true));
    }
    if let Some(t) = c.title() {
        o.insert("text".into(), json!(t.text));
    }
    if !c.effects.is_empty() {
        let names: Vec<&str> = c.effects.iter().map(|e| e.kind.as_str()).collect();
        o.insert("effects".into(), json!(names));
    }
    if c.video.cell.is_some() {
        o.insert("in_split_screen_cell".into(), json!(true));
    }
    Value::Object(o)
}

fn project_json(app: &TidyCutsApp) -> Value {
    let p = &app.project;
    let tracks: Vec<Value> = p
        .tracks
        .iter()
        .map(|t| {
            json!({
                "id": t.id,
                "name": t.name,
                "kind": match t.kind { TrackKind::Video => "video", TrackKind::Audio => "audio" },
                "main": p.main_track() == Some(t.id),
                "locked": t.locked,
                "hidden": t.hidden,
                "muted": t.muted,
                "solo": t.solo,
                "volume": t.volume,
                "clips": t.clips.iter().map(|c| clip_json(app, t, c)).collect::<Vec<_>>(),
            })
        })
        .collect();
    let media: Vec<Value> = p
        .media
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "name": m.path.file_name().map(|n| n.to_string_lossy().into_owned()),
                "kind": match m.kind { MediaKind::Video => "video", MediaKind::Audio => "audio", MediaKind::Image => "image" },
                "duration": secs(m.duration),
                "width": m.width,
                "height": m.height,
                "has_audio": m.has_audio,
                "path": m.path,
                "proxy": m.proxy,
            })
        })
        .collect();
    let markers: Vec<Value> = p
        .markers
        .iter()
        .map(|m| json!({"id": m.id, "time": secs(m.time), "name": m.name, "note": m.note}))
        .collect();
    json!({
        "name": p.name,
        "file": app.project_path,
        "unsaved_changes": app.dirty,
        "settings": serde_json::to_value(&p.settings).unwrap_or(Value::Null),
        "duration": secs(p.duration()),
        "master_volume": p.master_volume,
        "playhead": secs(app.playhead()),
        "selected_clips": app.selection,
        "tracks": tracks,
        "media": media,
        "markers": markers,
    })
}

fn get_clip(app: &TidyCutsApp, a: &Args) -> Out {
    let id = clip_id(app, a.need_str("clip")?)?;
    let (track, clip) = app.project.clip(id).ok_or("That clip is gone.")?;
    Ok(ToolResult::json(&json!({
        "track": track.id,
        "summary": clip_json(app, track, clip),
        "clip": serde_json::to_value(clip).map_err(|e| e.to_string())?,
    })))
}

fn list_catalog(a: &Args) -> Out {
    use tidycuts_render::catalog::{animations, effects, filters, stickers, titles, transitions};
    let list = |items: Vec<(&str, &str, &str)>| -> Value {
        Value::Array(
            items
                .into_iter()
                .map(|(id, name, cat)| json!({"id": id, "name": name, "category": cat}))
                .collect(),
        )
    };
    let kind = a.need_str("kind")?;
    let v = match kind {
        "titles" => list(
            titles::TITLES
                .iter()
                .map(|t| (t.id, t.name, t.category))
                .collect(),
        ),
        "transitions" => list(
            transitions::TRANSITIONS
                .iter()
                .map(|t| (t.id, t.name, t.category))
                .collect(),
        ),
        "effects" => list(
            effects::EFFECTS
                .iter()
                .map(|t| (t.id, t.name, t.category))
                .collect(),
        ),
        "filters" => list(
            filters::FILTERS
                .iter()
                .map(|t| (t.id, t.name, t.category))
                .collect(),
        ),
        "stickers" => list(
            stickers::STICKERS
                .iter()
                .map(|t| (t.id, t.name, t.category))
                .collect(),
        ),
        "layouts" => list(
            tidycuts_core::LAYOUTS
                .iter()
                .map(|l| (l.id, l.name, l.category))
                .collect(),
        ),
        "visualizers" => list(
            tidycuts_core::VISUALIZER_STYLES
                .iter()
                .map(|(id, name)| (*id, *name, "Audio visualizers"))
                .collect(),
        ),
        "luts" => list(
            tidycuts_render::lut::BUILT_IN
                .iter()
                .map(|l| (l.id, l.name, "Built-in looks"))
                .collect(),
        ),
        "animations" => list(
            animations::ANIMATIONS
                .iter()
                .map(|t| {
                    (
                        t.id,
                        t.name,
                        if t.titles_only {
                            "Titles only"
                        } else {
                            "Any clip"
                        },
                    )
                })
                .collect(),
        ),
        "ramps" => list(
            RampPreset::ALL
                .iter()
                .filter(|r| **r != RampPreset::Custom)
                .map(|r| (ramp_id(*r), r.label(), "Speed ramps"))
                .collect(),
        ),
        other => {
            return Err(format!(
                "Unknown catalog '{other}'. Choose one of: {}.",
                CATALOGS.join(", ")
            ))
        }
    };
    Ok(ToolResult::json(&v))
}

/// A ramp preset's id: its label in lowercase with underscores.
fn ramp_id(r: RampPreset) -> &'static str {
    match r {
        RampPreset::Custom => "custom",
        RampPreset::Montage => "montage",
        RampPreset::HeroMoment => "hero_moment",
        RampPreset::BulletTime => "bullet_time",
        RampPreset::Jumper => "jumper",
        RampPreset::FlashIn => "flash_in",
        RampPreset::FlashOut => "flash_out",
    }
}

// ----- editing --------------------------------------------------------------------

fn import_media(app: &mut TidyCutsApp, a: &Args) -> Out {
    let paths = a.strings("paths")?;
    if paths.is_empty() {
        return Err("'paths' needs at least one file.".into());
    }
    if paths.len() > 25 {
        return Err("Import at most 25 files at a time.".into());
    }
    if let Some(e) = &app.ffmpeg_error {
        return Err(format!(
            "Media can't be imported until FFmpeg is installed: {e}"
        ));
    }
    let mut results = Vec::new();
    let mut added = Vec::new();
    for p in paths {
        let path = PathBuf::from(p);
        if !path.is_file() {
            results.push(json!({"path": p, "error": "That file doesn't exist."}));
            continue;
        }
        if let Some(m) = app.project.media.iter().find(|m| m.path == path) {
            results.push(json!({"path": p, "media": m.id, "note": "already in the library"}));
            continue;
        }
        match tidycuts_media::probe(&path) {
            Ok(item) => {
                results.push(json!({
                    "path": p,
                    "media": item.id,
                    "kind": match item.kind { MediaKind::Video => "video", MediaKind::Audio => "audio", MediaKind::Image => "image" },
                    "duration": secs(item.duration),
                    "width": item.width,
                    "height": item.height,
                }));
                added.push(item);
            }
            Err(e) => {
                results.push(json!({"path": p, "error": format!("Can't use that file: {e}")}))
            }
        }
    }
    if !added.is_empty() {
        app.checkpoint();
        for item in added {
            app.add_imported_media(item);
        }
    }
    Ok(ToolResult::json(&Value::Array(results)))
}

fn add_to_timeline(app: &mut TidyCutsApp, a: &Args) -> Out {
    let media = media_id(app, a.need_str("media")?)?;
    let kind = app
        .project
        .media(media)
        .map(|m| m.kind)
        .ok_or("That media item is gone.")?;
    let start = a.opt_time("start")?;
    let track = a.str("track").map(|t| track_id(app, t)).transpose()?;
    let at = match (track, start) {
        (None, None) => None,
        (Some(t), s) => {
            let s = s.unwrap_or_else(|| app.project.track(t).map_or(Time::ZERO, Track::end));
            Some((t, s))
        }
        (None, Some(s)) => {
            let t = app
                .project
                .default_track_for(kind)
                .ok_or("There is no track for that kind of media.")?;
            Some((t, s))
        }
    };
    let before = all_clip_ids(app);
    app.add_media_to_timeline(media, at);
    let made = created_since(app, &before);
    if made.as_array().is_none_or(Vec::is_empty) {
        return Err(
            "The clip could not be placed (is the track locked or of the wrong kind?).".into(),
        );
    }
    let mut text = serde_json::to_string_pretty(&made).unwrap_or_default();
    if app.dialogs.is_modal_open() {
        text.push_str(
            "\n\nNote: the person is being asked about the project's size on screen; \
             their answer decides whether the project takes this video's size.",
        );
    }
    Ok(ToolResult::text(text))
}

fn add_title(app: &mut TidyCutsApp, a: &Args) -> Out {
    let preset = a.str("preset").unwrap_or("");
    let start = a.opt_time("start")?.unwrap_or_else(|| app.playhead());
    let len = a.opt_time("duration")?;
    let before = all_clip_ids(app);
    let id = app.add_title_with(preset, a.str("text"), start, len);
    id.ok_or("The title could not be added (all video tracks busy there?).")?;
    Ok(ToolResult::json(&created_since(app, &before)))
}

fn add_layout(app: &mut TidyCutsApp, a: &Args) -> Out {
    let id = a.need_str("layout")?;
    if tidycuts_core::layout(id).is_none() {
        return Err(format!("Unknown layout '{id}'. See list_catalog layouts."));
    }
    let start = a.opt_time("start")?;
    let before = all_clip_ids(app);
    app.add_layout(id, start);
    let made = created_since(app, &before);
    if made.as_array().is_none_or(Vec::is_empty) {
        return Err("The layout needs more free video tracks there.".into());
    }
    Ok(ToolResult::json(&made))
}

fn fill_cell(app: &mut TidyCutsApp, a: &Args) -> Out {
    let clip = clip_id(app, a.need_str("clip")?)?;
    let media = media_id(app, a.need_str("media")?)?;
    if !app.edit(|p| p.fill_cell(clip, media)) {
        return Err("That media can't go in this clip. It must be an empty cell on an unlocked track, and the media must have a picture.".into());
    }
    let (t, c) = app.project.clip(clip).ok_or("The clip is gone.")?;
    Ok(ToolResult::json(&clip_json(app, t, c)))
}

fn add_visualizer(app: &mut TidyCutsApp, a: &Args) -> Out {
    let style = a.need_str("style")?;
    if !tidycuts_core::VISUALIZER_STYLES
        .iter()
        .any(|(id, _)| *id == style)
    {
        return Err(format!(
            "Unknown style '{style}'. See list_catalog visualizers."
        ));
    }
    if let Some(s) = a.opt_time("start")? {
        app.seek(s);
    }
    let before = all_clip_ids(app);
    app.add_visualizer(style, None);
    let made = created_since(app, &before);
    if made.as_array().is_none_or(Vec::is_empty) {
        return Err("The visualizer could not be added (all video tracks busy there?).".into());
    }
    Ok(ToolResult::json(&made))
}

fn split_clip(app: &mut TidyCutsApp, a: &Args) -> Out {
    let id = clip_id(app, a.need_str("clip")?)?;
    let at = a.time("at")?;
    let (_, c) = app.project.clip(id).ok_or("That clip is gone.")?;
    if at <= c.start || at >= c.end() {
        return Err(format!(
            "{:.3} s is not inside the clip, which runs from {:.3} s to {:.3} s.",
            secs(at),
            secs(c.start),
            secs(c.end())
        ));
    }
    let before = all_clip_ids(app);
    if !app.edit(|p| p.split_clip(id, at).is_some()) {
        return Err("The clip could not be split (is its track locked?).".into());
    }
    Ok(ToolResult::json(&created_since(app, &before)))
}

fn delete_clips(app: &mut TidyCutsApp, a: &Args) -> Out {
    let given = a.strings("clips")?;
    if given.is_empty() {
        return Err("'clips' needs at least one clip.".into());
    }
    let ids: Vec<ClipId> = given
        .iter()
        .map(|g| clip_id(app, g))
        .collect::<Result<_, _>>()?;
    let before = all_clip_ids(app).len();
    app.selection = ids;
    app.delete_selection(a.bool("ripple"));
    let gone = before - all_clip_ids(app).len();
    if gone == 0 {
        return Err("Nothing was deleted (are the tracks locked?).".into());
    }
    Ok(done(format!("Deleted {gone} clip(s).")))
}

fn move_clip(app: &mut TidyCutsApp, a: &Args) -> Out {
    let id = clip_id(app, a.need_str("clip")?)?;
    let start = a.time("start")?;
    let (track, _) = app.project.clip(id).ok_or("That clip is gone.")?;
    let to = match a.str("track") {
        Some(t) => track_id(app, t)?,
        None => track.id,
    };
    if !app.edit(|p| p.move_clip(id, to, start)) {
        return Err(
            "The clip could not be moved (locked track, or a track of the other kind).".into(),
        );
    }
    let (t, c) = app.project.clip(id).ok_or("The clip is gone.")?;
    Ok(ToolResult::json(&clip_json(app, t, c)))
}

fn trim_clip(app: &mut TidyCutsApp, a: &Args) -> Out {
    let id = clip_id(app, a.need_str("clip")?)?;
    let edge = match a.need_str("edge")? {
        "start" => Edge::Start,
        "end" => Edge::End,
        other => return Err(format!("'edge' must be start or end, not '{other}'.")),
    };
    let to = a.time("to")?;
    if !app.edit(|p| p.trim_clip(id, edge, to).is_some()) {
        return Err(
            "The clip could not be trimmed (locked track, or there is no more footage that way)."
                .into(),
        );
    }
    let (t, c) = app.project.clip(id).ok_or("The clip is gone.")?;
    Ok(ToolResult::json(&clip_json(app, t, c)))
}

/// Merges a JSON merge patch into `dst`: objects merge, null removes.
fn merge(dst: &mut Value, patch: &Value) {
    match (dst, patch) {
        (Value::Object(d), Value::Object(p)) => {
            for (k, v) in p {
                if v.is_null() {
                    d.remove(k);
                } else {
                    merge(d.entry(k.clone()).or_insert(Value::Null), v);
                }
            }
        }
        (d, p) => *d = p.clone(),
    }
}

const PROTECTED: [(&str, &str); 7] = [
    ("id", "a clip's id can't change"),
    ("media_id", "use fill_cell or add a new clip"),
    ("start", "use move_clip"),
    ("source_in", "use trim_clip"),
    ("source_out", "use trim_clip"),
    ("speed", "use set_speed"),
    ("ramp", "use set_speed"),
];

fn update_clip(app: &mut TidyCutsApp, a: &Args) -> Out {
    let id = clip_id(app, a.need_str("clip")?)?;
    let patch = a.get("patch").ok_or("'patch' is needed (an object).")?;
    let Value::Object(fields) = patch else {
        return Err("'patch' must be an object.".into());
    };
    for (key, why) in PROTECTED {
        if fields.contains_key(key) {
            return Err(format!("'{key}' can't be changed here: {why}."));
        }
    }
    let (track, clip) = app.project.clip(id).ok_or("That clip is gone.")?;
    if track.locked {
        return Err("That clip's track is locked.".into());
    }
    let mut value = serde_json::to_value(clip).map_err(|e| e.to_string())?;
    merge(&mut value, patch);
    let new: Clip = serde_json::from_value(value)
        .map_err(|e| format!("That change doesn't fit the clip's settings: {e}"))?;
    if new.id != id {
        return Err("A clip's id can't change.".into());
    }
    let changed = app.edit(|p| match p.clip_mut(id) {
        Some(c) if *c != new => {
            *c = new;
            true
        }
        _ => false,
    });
    let (t, c) = app.project.clip(id).ok_or("The clip is gone.")?;
    let mut text = serde_json::to_string_pretty(&clip_json(app, t, c)).unwrap_or_default();
    if !changed {
        text = format!("Nothing changed (those values are already set).\n{text}");
    }
    Ok(ToolResult::text(text))
}

fn apply_effect(app: &mut TidyCutsApp, a: &Args) -> Out {
    use tidycuts_render::catalog::{effects, filters};
    let kind = a.need_str("effect")?;
    if effects::get(kind).is_none() && filters::get(kind).is_none() {
        return Err(format!(
            "Unknown effect '{kind}'. See list_catalog effects and filters."
        ));
    }
    let ids: Vec<ClipId> = a
        .strings("clips")?
        .iter()
        .map(|g| clip_id(app, g))
        .collect::<Result<_, _>>()?;
    let before = all_clip_ids(app);
    app.apply_effect(kind, &ids, None);
    let made = created_since(app, &before);
    if ids.is_empty() {
        return Ok(ToolResult::json(&made));
    }
    Ok(done(format!("Added {kind} to {} clip(s).", ids.len())))
}

fn set_transition(app: &mut TidyCutsApp, a: &Args) -> Out {
    use tidycuts_render::catalog::transitions;
    let id = clip_id(app, a.need_str("clip")?)?;
    let kind = a.need_str("transition")?;
    if transitions::get(kind).is_none() {
        return Err(format!(
            "Unknown transition '{kind}'. See list_catalog transitions."
        ));
    }
    if a.bool("at_end") {
        app.set_transition_out(id, kind);
    } else {
        app.set_transition(id, kind);
    }
    Ok(done(format!("Set the {kind} transition.")))
}

fn set_speed(app: &mut TidyCutsApp, a: &Args) -> Out {
    let id = clip_id(app, a.need_str("clip")?)?;
    let ramp = a.str("ramp");
    let speed = a.num("speed")?;
    match (speed, ramp) {
        (None, None) => return Err("Give a speed or a ramp.".into()),
        (Some(_), Some(_)) => return Err("Give a speed or a ramp, not both.".into()),
        (Some(s), None) => {
            if !(tidycuts_core::MIN_SPEED..=tidycuts_core::MAX_SPEED).contains(&s) {
                return Err(format!(
                    "Speed must be between {} and {}.",
                    tidycuts_core::MIN_SPEED,
                    tidycuts_core::MAX_SPEED
                ));
            }
            if !app.edit(|p| p.set_speed(id, s)) {
                return Err("The speed couldn't be changed (is the track locked?).".into());
            }
        }
        (None, Some(name)) => {
            let (_, c) = app.project.clip(id).ok_or("That clip is gone.")?;
            let new = if name == "none" {
                None
            } else {
                let preset = RampPreset::ALL
                    .iter()
                    .copied()
                    .find(|r| *r != RampPreset::Custom && ramp_id(*r) == name)
                    .ok_or_else(|| format!("Unknown ramp '{name}'. See list_catalog ramps."))?;
                let (from, to) = if c.reverse {
                    (c.source_out, c.source_in)
                } else {
                    (c.source_in, c.source_out)
                };
                Some(SpeedRamp::preset(preset, from, to))
            };
            if !app.edit(|p| p.set_speed_ramp(id, new)) {
                return Err("The speed couldn't be changed (is the track locked?).".into());
            }
        }
    }
    let (t, c) = app.project.clip(id).ok_or("The clip is gone.")?;
    Ok(ToolResult::json(&clip_json(app, t, c)))
}

fn detach_audio(app: &mut TidyCutsApp, a: &Args) -> Out {
    let id = clip_id(app, a.need_str("clip")?)?;
    if !app.can_detach(id) {
        return Err("That clip has no sound to detach.".into());
    }
    let before = all_clip_ids(app);
    app.selection = vec![id];
    app.detach_audio();
    Ok(ToolResult::json(&created_since(app, &before)))
}

fn freeze_frame(app: &mut TidyCutsApp, a: &Args) -> Out {
    let id = clip_id(app, a.need_str("clip")?)?;
    let at = a.time("at")?;
    app.selection = vec![id];
    app.seek(at);
    app.freeze_frame();
    Ok(done(
        "Freezing that frame: the still appears in a moment. Call get_project to see it.",
    ))
}

fn set_mix(app: &mut TidyCutsApp, a: &Args) -> Out {
    let volume = a
        .num("volume")?
        .map(|v| v.clamp(0.0, crate::mixer::MAX_VOLUME));
    let muted = a.get("muted").and_then(Value::as_bool);
    let solo = a.get("solo").and_then(Value::as_bool);
    let track = a.str("track").map(|t| track_id(app, t)).transpose()?;
    match track {
        None => {
            let v = volume.ok_or("Give a volume for the master, or a track to mix.")?;
            app.edit(|p| {
                p.master_volume = v;
                true
            });
            Ok(done(format!("Master volume set to {v:.0}%.")))
        }
        Some(id) => {
            if volume.is_none() && muted.is_none() && solo.is_none() {
                return Err("Give a volume, muted or solo.".into());
            }
            app.edit(|p| {
                let Some(t) = p.track_mut(id) else {
                    return false;
                };
                if let Some(v) = volume {
                    t.volume = v;
                }
                if let Some(m) = muted {
                    t.muted = m;
                }
                if let Some(s) = solo {
                    t.solo = s;
                }
                true
            });
            Ok(done("Mixer updated."))
        }
    }
}

fn sync_by_audio(app: &mut TidyCutsApp, a: &Args) -> Out {
    let ids: Vec<ClipId> = a
        .strings("clips")?
        .iter()
        .map(|g| clip_id(app, g))
        .collect::<Result<_, _>>()?;
    Ok(done(app.sync_now(&ids)?))
}

fn make_proxy(app: &mut TidyCutsApp, a: &Args) -> Out {
    let id = media_id(app, a.need_str("media")?)?;
    if app.make_proxy(id) {
        Ok(done("Making the proxy in the background."))
    } else {
        Err("A proxy can't be made for this one (it must be a video without a proxy yet).".into())
    }
}

fn add_voiceover(app: &mut TidyCutsApp, a: &Args) -> Out {
    let text = a.need_str("text")?;
    if text.chars().count() > 5000 {
        return Err("Keep a voiceover under 5000 characters; make several for more.".into());
    }
    let voice = a.str("voice").unwrap_or("");
    let speed = a.num("speed")?.unwrap_or(1.0).clamp(0.5, 2.0);
    let at = a.opt_time("start")?;
    Ok(done(app.voiceover_now(text, voice, speed, at)?))
}

fn list_voices() -> Out {
    let voices: Vec<Value> = tidycuts_media::speech::voices()
        .into_iter()
        .map(|v| json!({"id": v.id, "name": v.name, "language": v.language}))
        .collect();
    Ok(ToolResult::json(&Value::Array(voices)))
}

fn remove_silence(app: &mut TidyCutsApp, a: &Args) -> Out {
    let id = clip_id(app, a.need_str("clip")?)?;
    let noise_db = a.num("level_db")?.unwrap_or(-35.0).clamp(-60.0, -15.0);
    let min_secs = a.num("min_pause")?.unwrap_or(0.5).clamp(0.1, 10.0);
    let text = app.detect_now(id, Detection::Silences { noise_db, min_secs })?;
    Ok(done(text))
}

fn split_at_scenes(app: &mut TidyCutsApp, a: &Args) -> Out {
    let id = clip_id(app, a.need_str("clip")?)?;
    let threshold = a.num("threshold")?.unwrap_or(0.3).clamp(0.05, 0.9);
    let before = all_clip_ids(app);
    let text = app.detect_now(id, Detection::Scenes { threshold })?;
    Ok(ToolResult::text(format!(
        "{text}\n{}",
        serde_json::to_string_pretty(&created_since(app, &before)).unwrap_or_default()
    )))
}

fn add_beat_markers(app: &mut TidyCutsApp, a: &Args) -> Out {
    let id = clip_id(app, a.need_str("clip")?)?;
    let text = app.detect_now(id, Detection::Beats)?;
    Ok(done(text))
}

fn add_marker(app: &mut TidyCutsApp, a: &Args) -> Out {
    let at = a.time("time")?;
    let name = a.str("name").map(str::to_string);
    let note = a.str("note").map(str::to_string);
    let mut id = None;
    app.edit(|p| {
        let m = p.add_marker(at);
        if let Some(mk) = p.marker_mut(m) {
            if let Some(n) = &name {
                mk.name = n.clone();
            }
            if let Some(n) = &note {
                mk.note = n.clone();
            }
        }
        id = Some(m);
        true
    });
    Ok(done(format!(
        "Marker added: {}",
        id.map(|i| i.to_string()).unwrap_or_default()
    )))
}

fn save_project(app: &mut TidyCutsApp, a: &Args) -> Out {
    let path = match a.str("path") {
        Some(p) => PathBuf::from(p),
        None => app
            .project_path
            .clone()
            .ok_or("This project has no file yet: give a 'path' (ending in .tcproj).")?,
    };
    let path = if path.extension().is_none() {
        path.with_extension(tidycuts_core::PROJECT_EXTENSION)
    } else {
        path
    };
    if app.save_to(&path) {
        Ok(done(format!("Saved to {}.", path.display())))
    } else {
        Err("The project could not be saved.".into())
    }
}

fn open_project(app: &mut TidyCutsApp, a: &Args) -> Out {
    let path = PathBuf::from(a.need_str("path")?);
    if !path.is_file() {
        return Err("That file doesn't exist.".into());
    }
    if app.dirty && !a.bool("discard_changes") {
        return Err(
            "The open project has unsaved changes. Save it first, or pass discard_changes: true."
                .into(),
        );
    }
    app.open_project_at(&path);
    Ok(done(format!("Opened {}.", path.display())))
}

fn export_video(app: &mut TidyCutsApp, a: &Args) -> Out {
    if app
        .export_job
        .as_ref()
        .is_some_and(|j| j.finished.is_none())
    {
        return Err("An export is already running. Check export_status.".into());
    }
    if app.project.duration() == Time::ZERO {
        return Err("There is nothing on the timeline to export.".into());
    }
    let path = PathBuf::from(a.need_str("path")?);
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let container = match ext.as_str() {
        "mp4" => Container::Mp4,
        "mov" => Container::Mov,
        "mkv" => Container::Mkv,
        "webm" => Container::Webm,
        "gif" => Container::Gif,
        "mp3" => Container::Mp3,
        "wav" => Container::Wav,
        "m4a" => Container::M4a,
        _ => {
            return Err(
                "Use a path ending in .mp4, .mov, .mkv, .webm, .gif, .mp3, .wav or .m4a.".into(),
            )
        }
    };
    let mut s = ExportSettings::for_project(&app.project, path);
    s.container = container;
    if !container.video_codecs().contains(&s.codec) {
        if let Some(c) = container.video_codecs().first() {
            s.codec = *c;
        }
    }
    if let Some(w) = a.num("width")? {
        let w = (w.round() as u32).clamp(16, 7680) & !1;
        let h = (f64::from(w) * f64::from(s.height) / f64::from(s.width.max(1))).round() as u32;
        s.width = w;
        s.height = h.max(2) & !1;
    }
    s.quality = match a.str("quality").unwrap_or("recommended") {
        "higher" => Quality::Higher,
        "lower" => Quality::Lower,
        "recommended" => Quality::Recommended,
        other => return Err(format!("Unknown quality '{other}'.")),
    };
    s.hardware_encode = app.prefs.hardware_encode;
    s.hardware_decode = app.prefs.hardware_decode;
    if let (Some(from), Some(to)) = (a.opt_time("start")?, a.opt_time("end")?) {
        if to <= from {
            return Err("'end' must be after 'start'.".into());
        }
        s.range = Some((from, to));
    }
    let shown = s.path.clone();
    app.start_export(s);
    if app.export_job.is_none() {
        return Err("The export could not start.".into());
    }
    Ok(done(format!(
        "Export to {} started. Check export_status.",
        shown.display()
    )))
}

fn export_status(app: &TidyCutsApp) -> Value {
    let Some(job) = &app.export_job else {
        return json!({"state": "none", "message": "No export has been started."});
    };
    match &job.finished {
        Some((Ok(()), took)) => json!({
            "state": "finished",
            "file": job.settings.path,
            "seconds_taken": took.as_secs_f64().round(),
        }),
        Some((Err(e), _)) => json!({"state": "failed", "error": e}),
        None => {
            let p = job.progress.lock().map(|p| (p.stage, p.done, p.total)).ok();
            let percent = p
                .filter(|(_, _, total)| *total > 0)
                .map(|(_, done, total)| (done as f64 / total as f64 * 100.0).round());
            json!({
                "state": "running",
                "stage": p.map(|(s, _, _)| s),
                "percent": percent,
                "file": job.settings.path,
            })
        }
    }
}

// ----- looking ---------------------------------------------------------------------

fn make_thumbnail(
    app: &mut TidyCutsApp,
    a: &Args,
    reply: std::sync::mpsc::Sender<ToolResult>,
) -> Result<(), String> {
    use crate::thumbnail::{self as thumb, Across, Down};
    let engine = app
        .engine
        .as_ref()
        .ok_or("There is no graphics device, so nothing can be drawn.")?;
    let mut s = thumb::Settings {
        text: a.str("text").unwrap_or("").trim().to_string(),
        ..thumb::Settings::default()
    };
    if s.text.chars().count() > 200 {
        return Err("Keep the words under 200 characters.".into());
    }
    if let Some(name) = a.str("style") {
        s.look = thumb::look_named(name).ok_or_else(|| {
            let all: Vec<&str> = thumb::LOOKS.iter().map(|l| l.id).collect();
            format!(
                "Unknown style '{name}'. The styles are: {}.",
                all.join(", ")
            )
        })?;
    }
    if let Some(v) = a.str("across") {
        s.across = match v.to_lowercase().as_str() {
            "left" => Across::Left,
            "center" | "centre" => Across::Centre,
            "right" => Across::Right,
            _ => return Err("'across' must be left, center or right.".into()),
        };
    }
    if let Some(v) = a.str("down") {
        s.down = match v.to_lowercase().as_str() {
            "top" => Down::Top,
            "middle" => Down::Middle,
            "bottom" => Down::Bottom,
            _ => return Err("'down' must be top, middle or bottom.".into()),
        };
    }
    if let Some(v) = a.str("shape") {
        s.canvas = match v.to_lowercase().as_str() {
            "widescreen" => 0,
            "square" => 1,
            "vertical" => 2,
            _ => return Err("'shape' must be widescreen, square or vertical.".into()),
        };
    }
    s.size = a.num("size")?.unwrap_or(s.size).clamp(40.0, 160.0);
    s.punch = a.num("punch")?.unwrap_or(s.punch).clamp(0.0, 100.0);
    s.shade = a.num("fade")?.unwrap_or(s.shade).clamp(0.0, 100.0);

    let media = match a.str("media") {
        Some(m) => {
            let id = media_id(app, m)?;
            let item = app
                .project
                .media(id)
                .cloned()
                .ok_or("That media is gone.")?;
            if item.kind == tidycuts_core::MediaKind::Audio {
                return Err("That is a sound, not a picture or a video.".into());
            }
            Some(item)
        }
        None => None,
    };
    let timeline = match &media {
        Some(_) => None,
        None if app.project.duration() == Time::ZERO => {
            return Err("The timeline is empty; give a 'media' picture or video.".into())
        }
        None => Some(app.project.clone()),
    };
    let default_at = match &media {
        Some(m) => (m.duration.as_secs_f64() * 0.1).min(2.0),
        None => app.playhead().as_secs_f64(),
    };
    s.at = a.num("time")?.unwrap_or(default_at).max(0.0);

    let path = match a.str("path") {
        Some(p) => std::path::PathBuf::from(p),
        None => thumb::free_name(&app.stills_dir(), "Thumbnail", "png"),
    };
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !["png", "jpg", "jpeg"].contains(&ext.as_str()) {
        return Err("The file name must end in .png or .jpg.".into());
    }

    let (renderer, analysis) = (engine.renderer.clone(), engine.analysis.clone());
    let scratch = app
        .prefs
        .cache_dir()
        .join("thumbnails")
        .join(format!("mcp-{}", std::process::id()));
    std::thread::Builder::new()
        .name("tidycuts-mcp-thumbnail".into())
        .spawn(move || {
            let result = (|| -> Result<ToolResult, String> {
                let base = match (media, timeline) {
                    (Some(m), _) => m,
                    (None, Some(tl)) => {
                        let _ = std::fs::create_dir_all(&scratch);
                        thumb::picture_of_timeline(
                            &tl,
                            Time::from_secs_f64(s.at),
                            &scratch,
                            renderer.clone(),
                            &analysis,
                        )?
                    }
                    (None, None) => return Err("There is nothing to draw from.".into()),
                };
                let project = thumb::build(&s, &base);
                let full = thumb::render(&project, None, renderer.clone(), &analysis);
                thumb::save(&full, &path)?;
                let preview = if full.width > 960 {
                    thumb::render(&project, Some(960), renderer, &analysis)
                } else {
                    full
                };
                let png = tidycuts_engine::snapshot::png_bytes(
                    preview.width,
                    preview.height,
                    &preview.rgba,
                )?;
                Ok(ToolResult::text(format!(
                    "Saved the thumbnail to {} ({}x{}).",
                    path.display(),
                    canvas_size(s.canvas).0,
                    canvas_size(s.canvas).1
                ))
                .with_image(png, "image/png"))
            })();
            let _ = std::fs::remove_dir_all(&scratch);
            let _ = reply.send(result.unwrap_or_else(ToolResult::error));
        })
        .map_err(|e| format!("Could not start drawing: {e}"))?;
    Ok(())
}

fn canvas_size(i: usize) -> (u32, u32) {
    let c = &crate::thumbnail::CANVASES[i.min(crate::thumbnail::CANVASES.len() - 1)];
    (c.width, c.height)
}

fn render_frame(
    app: &mut TidyCutsApp,
    a: &Args,
    reply: std::sync::mpsc::Sender<ToolResult>,
) -> Result<(), String> {
    let engine = app
        .engine
        .as_ref()
        .ok_or("There is no graphics device, so nothing can be drawn.")?;
    let end = app.project.duration();
    if end == Time::ZERO {
        return Err("The timeline is empty: there is nothing to draw.".into());
    }
    let at = a
        .opt_time("time")?
        .unwrap_or_else(|| app.playhead())
        .min(end - Time::from_millis(1));
    let width = a
        .num("width")?
        .map_or(960, |w| (w.round() as u32).clamp(128, 1920));
    let project = app.project.clone();
    let renderer = engine.renderer.clone();
    let analysis = engine.analysis.clone();
    std::thread::Builder::new()
        .name("tidycuts-mcp-frame".into())
        .spawn(move || {
            let statics = tidycuts_engine::Statics::default();
            let auto = move |m: &tidycuts_core::MediaItem| analysis.auto_grade(m);
            let pic = tidycuts_engine::snapshot::render(
                &project,
                at,
                Some(width),
                renderer,
                &statics,
                &auto,
            );
            let result =
                match tidycuts_engine::snapshot::png_bytes(pic.width, pic.height, &pic.rgba) {
                    Ok(png) => ToolResult::text(format!(
                        "The project at {:.3} s, {}x{} pixels.",
                        secs(at),
                        pic.width,
                        pic.height
                    ))
                    .with_image(png, "image/png"),
                    Err(e) => ToolResult::error(e),
                };
            let _ = reply.send(result);
        })
        .map_err(|e| format!("Could not start drawing: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_patches_merge_objects_and_null_removes() {
        let mut v = json!({"a": {"b": 1, "c": 2}, "d": [1, 2], "e": 5});
        merge(
            &mut v,
            &json!({"a": {"b": 9, "x": null, "c": null}, "d": [3], "e": null}),
        );
        assert_eq!(v, json!({"a": {"b": 9}, "d": [3]}));
    }

    #[test]
    fn ids_resolve_in_full_or_by_unique_start() {
        let a = Uuid::parse_str("aaaaaaaa-0000-4000-8000-000000000001").unwrap();
        let b = Uuid::parse_str("aaaaaabb-0000-4000-8000-000000000002").unwrap();
        let ids = || [a, b].into_iter();
        assert_eq!(resolve("clip", &a.to_string(), ids()), Ok(a));
        assert_eq!(resolve("clip", "AAAAAABB", ids()), Ok(b));
        assert_eq!(resolve("clip", "aaaaaaaa", ids()), Ok(a));
        assert!(resolve("clip", "aaaa", ids()).is_err(), "too short");
        assert!(resolve("clip", "aaaaaa", ids()).is_err(), "two clips match");
        assert!(resolve("clip", "cccccc", ids()).is_err());
        assert!(resolve("clip", &Uuid::nil().to_string(), ids()).is_err());
    }

    #[test]
    fn every_tool_has_an_object_schema_and_a_unique_name() {
        let tools = tool_list();
        let names: HashSet<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names.len(), tools.len());
        for t in &tools {
            assert_eq!(t.schema["type"], "object", "{}", t.name);
            assert!(!t.description.is_empty());
            // Every required argument is a declared property.
            for r in t.schema["required"].as_array().unwrap() {
                assert!(
                    t.schema["properties"].get(r.as_str().unwrap()).is_some(),
                    "{}",
                    t.name
                );
            }
        }
        for name in [
            "get_project",
            "render_frame",
            "import_media",
            "add_to_timeline",
            "split_clip",
            "update_clip",
            "export_video",
            "undo",
        ] {
            assert!(names.contains(name), "{name}");
        }
    }

    #[test]
    fn ramp_ids_cover_every_preset_but_custom() {
        for r in RampPreset::ALL.iter().filter(|r| **r != RampPreset::Custom) {
            assert_eq!(ramp_id(*r), r.label().to_lowercase().replace(' ', "_"));
        }
    }
}
