# TidyCuts development notes

Notes for anyone (or any AI assistant) picking TidyCuts up: what the project is,
what the owner asked for, how it is built and tested, and lessons learned along
the way. Written on 2026-10-10 for the 1.0.0 release.

## What TidyCuts is

A desktop video editor for Windows and macOS, written in Rust, with the look,
layout and ease of use of a mainstream consumer editor. It must have every
non-AI editing function and no AI features. Claude will drive it later
through an MCP server built into the app (step 4). See README.md for the
user-facing feature list. Do not name other companies' products in the app,
the docs or the code: the project is going to be open source.

## The owner's requirements (all still in force)

- Layout: library top left (about half the width), player top right,
  timeline full width along the bottom. Clip properties open on
  double-click, inside the library area. Dark, modern, roomy UI; the owner
  called a small properties panel "outdated".
- A magnetic main track: cutting or deleting closes the gap.
- Real built-in assets, made by TidyCuts itself (no third-party stock):
  titles, transitions, effects, filters, stickers, templates, music and
  sound effects.
- Fast scrubbing and GPU acceleration everywhere it helps.
- Up to 10 video tracks and 5 audio tracks (A-roll and B-roll).
- A scissors "cut" button on the playhead line.
- When the first video lands on the timeline and doesn't match the project,
  ask whether to keep its size (e.g. 4K) or use 1080p. Project Settings and
  Preferences hold the defaults.
- Detach audio ("very important"), audio fades, levels, noise removal and
  the full clip right-click menu (skip AI items).
- Ruler drag zooms, ruler click seeks, the playhead handle scrubs.
  Full-screen preview.
- Upscaling and enhancement (denoise, deblock, sharpen) as GPU shaders.
- A full set of export options.
- Step 5 (later) adds free, offline AI add-ons first (Kokoro/Piper voices,
  procedural music, a thumbnail maker); paid or cloud options only later.

## Roadmap

1. Foundation and layout: done.
2. Core editing, compositor, export: done.
3. Creative library (titles, transitions, effects, filters, stickers,
   templates, sounds, music): done.
4. Claude integration through an MCP server, plus Windows and Mac
   installers: done (installers untested until a release build runs).
5. Free offline AI add-ons: not started (voiceover with system voices is done).

All ten editing tools and step 4 are built; the UI refresh comes next.

## Where the work stands at 1.0.0

- **Newest round (before the Windows + Mac test build):**
  - **Research behind the modern look** (no product names here on purpose; the
    sources were vendor blogs, so treat them as hints): short-form edits lean
    on zoom punches, flashes, shake, RGB split, speed ramps, glitch and neon;
    calm effects suit longer videos; the cinematic stack is halation,
    diffusion, anamorphic streaks, fine grain, soft vignette and a little lens
    colour fringing; dated: star/heart wipes, page curls, lens flares dropped
    on a clip, flashy effects in every scene. Overlay libraries sell: glitch,
    leaks and flares, camcorder overlays, anime speed lines, dust and mist,
    sparks and embers, fluid colour. Text packs: minimal, kinetic, cinematic,
    gold/metallic, neon, glass, message cards, editorial, captions with word
    highlight (social, music video, cinematic, karaoke). Filter packs: minimal,
    pastel, lifestyle, nature, travel, night, creator; keep strength moderate.
    Everything was built as our own procedural versions; no third-party
    footage or presets are included.
  - New: 19 modern effects (`cinematic.*`, `trendy.*`, 3 particles; shaders
    74-90), 7 title styles + 19 presets + `LetterKind` letter/word animations
    (`text.rs` draws glyph by glyph with a scratch pixmap for alpha), 42
    filters, 9 transitions (shaders 45-48), Silence Detection toolbar button,
    export window thumbnail rendered from the project (`dialogs/export_preview.rs`).
  - **Lesson**: the effect shader's single `switch` broke past ~79 cases in
    lavapipe (case 79 wrong, 80+ black). `fs_effect` now calls three
    dispatchers. Keep any `switch` well under ~50 cases.
  - **Mac test (the owner, first try)**: the unsigned .app/.dmg was reported
    "damaged and can't be opened" by macOS. Now ad-hoc signed
    (`scripts/make-macos-app.sh`), verified in the workflow, with a read-me
    in the image. Real fix = Apple Developer ID signing + notarization
    (needs an Apple Developer account, about $99 a year): ask the owner. The
    runner is Apple Silicon only (`macos-latest`).
  - **Lesson**: minimizing on Windows gives a zero-size window; `App::ui`
    returns early under `MIN_DRAW` (480x320) so no panel maths runs.
  - Sample pictures are now the owner's own photographs (`assets/photos/*.jpg`,
    1280 px wide, decoded with zune-jpeg and averaged down by `sample::Photo`
    mip levels): lake at sunset, a swan, young swans, trees, two photos of his
    cat, and his cut-out portrait (`assets/portrait.png`) in front of a drawn
    bokeh or a green screen. `Sample::{Wildlife, Meadow, Pet, Nap}` pick the
    new pictures; filters and effects take them in turn, transition tiles use
    pairs (`scene::transition_pictures`).
  - Blur Area and Pixelate Area (`blur.area`, `blur.pixelate_area`, shaders
    72 and 73): ellipse, rectangle or rounded shape with position, size,
    feather and "everything else"; every setting can be keyframed, so the
    area can follow a moving face.
  - 58 new stickers (117 in all): Symbols, Nature, more social call-outs
    (generic buttons, no brand logos or names), five more emoji, badges,
    lines and animated sparkles. `Pen::inset` keeps wide outlines inside the
    box; the margin test (`every_sticker_renders_large_with_margin`) checks
    the shadow too.

- **Latest round (the owner's test feedback, all done and tested under Xvfb):**
  free lane above the top video track and below the last audio track
  (`timeline.rs`, `FREE_LANE_H`), tracks anchored at the bottom with the
  scroll sticking to it, drag the line under a track header to resize
  (`TimelineState::track_heights`, not saved with the project), titles,
  stickers, effects and visualizers always on their own track (`drop_plan`
  `overlay`), layout presets (`prefs::LayoutPreset`, View > Layout; the left-menus
  layout shows the properties under the library tabs via
  `TidyCutsApp::left_shows_props`), tiles fill each row
  (`library::fitted_tile`), dock edge draggable (`prefs.dock_w`), overflow
  fixes (the inspector logs "tab runs Npx past the panel" when a row is too
  wide), dialogs scroll when the window is short (`dialogs::scroll_middle`),
  relaxed double-click timing (`theme::apply`).
  **Colour bug found by measuring**: the live player darkened the picture
  because the preview target was shown through an sRGB view; it is shown as
  plain bytes now (`Target::display_view`). Test with SMPTE bars: 75% yellow
  must read (191,191,0) in the player.
  **GPU export**: `Compositor::submit_nv12`/`read_nv12` draw the picture as
  NV12 on the card (`fs_output_y`/`fs_output_uv` in `common.wgsl`; test
  `tests/nv12_gpu.rs`), with two sets of planes so one frame is read back
  while the next is drawn; ffmpeg gets `-pix_fmt nv12` and no `-vf` (ProRes and
  GIF keep RGBA). `pick_encoder` puts the encoder on the rendering card first
  (`tidycuts_render::gpu::adapter`), NVENC gets `-tune hq -spatial-aq 1
  -rc-lookahead 20` only where a probe accepts them. None of the hardware
  paths can run here (no GPU in the container): they need the owner's PC.
  **Colour wheels** are baked into the per-channel curve table
  (`looks::wheels_curve`), the window is `scopes.rs` (counts in
  `tidycuts-engine/src/scopes.rs`), **Silence Detection** is
  `dialogs/silence.rs`.

- **Name: TidyCuts** (final; the owner's choice, a TidySweep Software product).
  Crates are `tidycuts-*`, projects save as `.tcproj` (old `.rcproj` still
  open). Not checked (needs a person): trademark registers and store name
  reservation.
- **Done this session** (all tested; screenshots reviewed under Xvfb):
  - Library rework: sample pictures (`tidycuts-render/src/sample.rs`; they
    were drawn in code at first and are now the owner's own photographs, see
    "Latest round" below), filters
    with a before/after split, hover sweeps for effects that don't move, a
    multi-exposure still for motion effects, bigger stickers on dark tiles,
    template badges (length, clip count), search across every tab, Trending
    plus category sections, 6 tiles per row with a slider.
  - Audio visualizers redrawn (glow, peak caps, reflections, layered neon
    lines, seamless circle and ring) in `shaders/layout.wgsl`.
  - Preferences > AI assistants: Claude and OpenAI Codex (not the ChatGPT
    website or phone app: they need a public HTTPS server).
  - The Claude connection was tried for real with Claude Code over HTTP.
  - Known gaps fixed: empty split-screen cells no longer export as panels,
    imported LUTs are stored inside the project, Duration on a ramped clip
    stretches the ramp, dropping media on a cell in the preview works.
  - Crash recovery, export presets, chapters from markers, silence removal,
    scene splitting and beat markers (also as assistant tools), Windows
    installer script (Inno Setup) and Mac disk image script (both untested:
    they need GitHub's runners).
  - Audio mixer with level meters, proxy editing, multicam sync by audio
    (GCC-PHAT), voiceover from typed text with the system's own voices
    (`tidycuts-media/src/speech.rs`), the thumbnail maker (Edit menu;
    `thumbnail.rs`, `dialogs/thumbnail.rs`, assistant tool `make_thumbnail`),
    render previews (`tidycuts-engine/src/previews.rs`, `previews.rs` in the
    app: 4-second pieces fingerprinted by what affects them, amber/green bar on
    the ruler, lightning button; playback swaps ready pieces in via
    `Playback::set_previews`), the app icon (`scripts/make_icon.py`,
    `packaging/icons`), and the open-source package (README, CONTRIBUTING,
    SECURITY, CODE_OF_CONDUCT, THIRD-PARTY-NOTICES, `deny.toml`, a licences
    job in CI). 37 assistant tools.
  - Fixed a real playback bug: jumping back while playing could freeze the
    picture (stale prepared frames); frames are now tagged with their epoch.
  - CI uses the newest stable Rust, whose clippy can be stricter than a
    local one: run `scripts/check.sh +1.99.0` (install it with
    `rustup toolchain install 1.99.0 -c clippy -c rustfmt`) before pushing
    `main`.
- **The look** is the "Graphite" design the owner chose (mockup B) and asked to be
  built 1:1. Its numbers: title bar 40; library 640 wide with a 44-pixel tab
  strip; properties panel 310 wide, always open (toolbar button hides it);
  timeline toolbar 40, ruler 30, track headers 140, lanes 70 (main video) / 50
  (other video) / 58 (audio) with clips inset 4. Colours, weights and radii are in
  `theme.rs`; the slim sliders are `widgets::slim_slider`/`slider_with` (3-pixel
  rail with an 11-pixel handle in the panel, 4-pixel rail with a 14-pixel handle in
  the library and zoom); Inter is used at weights 400/500/550/600/650/700 from one
  variable font. To compare with the mockup: the scratchpad tools used were
  `cmp.py` (mockup above, app below) and `edges.py` (pixel runs along a line).
- **Still to do**: the more modern look for the whole UI (the owner: "it looks a
  bit outdated"); free offline AI add-ons (step 5: auto captions need a
  speech model download, neural voices, optional "bring your own key" image
  generation); an installer test run on GitHub's runners (Installers
  workflow) once the owner agrees to the Mac minutes.
- Where this runs: Claude Code cloud session (Linux). Needs `libasound2-dev
  libxkbcommon-dev libxkbcommon-x11-dev libgtk-3-dev mesa-vulkan-drivers
  ffmpeg`; for screenshots `xvfb xdotool imagemagick`. Run the app with
  `Xvfb :99 -screen 0 1600x960x24 &` and `DISPLAY=:99 target/debug/tidycuts`;
  give the window focus with `xdotool windowfocus` before sending keys.
  Windows work needs the GitHub Actions runner or the owner's PC.

## Building and testing

- Rust stable. Linux needs `libasound2-dev libxkbcommon-dev libgtk-3-dev`
  (and `ffmpeg` on PATH to run). Run with `cargo run --release -p tidycuts-app`.
- Before every commit: `cargo fmt --all`, then
  `cargo clippy --workspace --all-targets -- -D warnings` (CI fails on any
  warning), then `cargo test --workspace`.
- Windows build from Linux: install mingw-w64 and the
  `x86_64-pc-windows-gnu` target, then
  `cargo build --release -p tidycuts-app --target x86_64-pc-windows-gnu`.
  Package `TidyCuts.exe` with the LGPL FFmpeg build from BtbN
  (`ffmpeg-master-latest-win64-lgpl`): `ffmpeg.exe` and `ffprobe.exe` in an
  `ffmpeg` folder beside it, plus FFmpeg's LICENSE. The CI workflow in
  `.github/workflows/ci.yml` does the same on GitHub's Windows runners.
- macOS: `scripts/make-macos-app.sh` on a Mac.
- LGPL FFmpeg has no libx264, so export uses GPU encoders or the
  Media Foundation / OpenH264 encoders (`working_encoders()` probes them).

## Code map

- `crates/tidycuts-core`: project model (`project.rs`, `clip.rs`,
  `keyframe.rs`, `time.rs`) and edit operations (`edit.rs`). New saved
  fields always get `#[serde(default)]` so old projects keep loading.
- `crates/tidycuts-media`: ffmpeg/ffprobe as subprocesses (probe, frame
  grabs, decoding) and cpal audio output.
- `crates/tidycuts-render`: the wgpu compositor (`compositor.rs`), WGSL
  shaders (`src/shaders`), and the catalogs of titles, transitions,
  effects, filters and stickers (`src/catalog`).
- `crates/tidycuts-engine`: decoders, the scrub cache, the frame planner
  (`plan.rs`), the audio mixer, playback and export.
- `crates/tidycuts-sounds`: music beds and sound effects synthesised in code,
  written to `<config>/tidycuts/sounds/<id>-v1.wav` on first use.
- `crates/tidycuts-app`: the eframe/egui 0.36 app on wgpu 30. `app.rs` (state,
  undo, shortcuts), `library.rs`, `timeline.rs`, `preview.rs`,
  `inspector/*.rs` (properties panel), `dialogs/*.rs`, `menus.rs`.

Conventions that matter:

- Every user action is one undo step: `app.edit(|p| ...)` for project
  changes, `commit_clip` for continuous slider edits.
- egui: use `pointer.press_origin()` for where a drag started (drag_started
  fires after a few pixels); `ui.new_child(UiBuilder::new().max_rect(r))`
  draws without moving the parent's cursor.
- The Inter font had its private-use glyphs removed so the Phosphor icon
  font shows (see `crates/tidycuts-render/fonts/README.md`).
- UI text is plain English, sentence case, no jargon.
