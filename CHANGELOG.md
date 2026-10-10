# Changes

All notable changes to TidyCuts are listed here, newest first.

## 1.0.0 (first release)

**Editing**
- A Thumbnail button (a little picture) beside the snapshot camera under the player opens the thumbnail maker; it is also in Edit → Make Thumbnail.
- The files are labelled with the version: `TidyCuts-Setup-1.0.0.exe`, `TidyCuts-1.0.0-windows-portable.zip`, `TidyCuts-1.0.0-macos.dmg` and `TidyCuts-1.0.0-macos.zip`, with a `SHA256SUMS.txt`.
- Presets, sizes, templates and the thumbnail maker no longer carry other companies' names or logos: Widescreen HD, Widescreen 4K, Vertical phone, Square post, Channel Intro.
- Magnetic main track, up to 10 video and 5 audio tracks, split, trim, ripple
  delete, detach audio, freeze frame, snapshot, markers.
- Colour: exposure, contrast, temperature and more, HSL ranges, curves, LUTs
  (built-in looks and `.cube` files kept inside the project).
- Speed: constant speed and speed ramping with presets and a draggable graph,
  pitch-corrected audio.
- Split-screen layouts (16) and audio visualizers (7).
- Smart cuts: remove silent parts, split at scene changes, mark the beat, line
  clips up by their sound.
- Audio mixer with volume, mute, solo, a master fader and level meters.
- Proxy editing for big videos.
- New look ("Graphite"): flat panels that meet in thin lines, a violet accent, the properties panel always open on the right, underlined tabs, slim sliders and a re-proportioned timeline.
- Render preview: hard-to-play parts are drawn ahead of time and play smoothly (lightning button, ruler marks).
- Fixed: jumping back while playing could leave the picture frozen until the clock caught up.
- Thumbnail maker (Edit → Make Thumbnail): a picture with big lettering, saved as PNG or JPG.
- Voiceovers from typed text, spoken by the computer's own voices (offline).
- Timeline: tracks sit at the bottom with a free lane above the top video track and below the last audio track, so there is always somewhere to drop a clip that starts a new track; the line under a track header can be dragged to make the track taller (bigger waveforms) or shorter; titles, stickers, effects and visualizers dropped on the timeline get a track of their own and never go into the main track.
- Layouts (View → Layout): Standard, Mirrored, and a left-menus layout where the library and the properties of a double-clicked clip share the wide left side next to the preview; the properties panel's edge can be dragged; library tiles stretch to fill each row.
- Colour wheels (lift, gamma, gain, offset) in the Color tab and View → Scopes: waveform, RGB parade, vectorscope and histogram of the picture in the preview.
- Silence Detection: volume threshold, minimum duration, softening buffer, a waveform with the quiet parts marked, Export to Timeline.
- Waveforms are drawn as a filled shape rising from the bottom of the track, scaled to each recording so quiet speech fills the track and the peaks stand out (before: a flat comb on a decibel scale).
- A video and the audio detached from it stay linked: cutting either cuts both, deleting either deletes both (Clip menu → Unlink to edit them on their own); a link icon shows on linked clips.
- Double-clicking a title, sticker, visualizer or effect (on the timeline or in the preview) opens its own settings tab, with the text box ready, not the Video tab.
- The timeline ruler shows a double-arrow cursor and a hint when you hover it (drag sideways to zoom), and says how much is in view while you drag.
- Fixed: the live player showed the picture darker than the file (an sRGB view was decoded twice); it now matches the exported video.
- Fixed: Speed and Title tabs and the "Nothing selected" text ran past the edge of the panel; tall dialogs scroll in a short window; a relaxed double-click now opens a clip's properties.
- Blur Area and Pixelate Area effects (Effects → Blur): hide a face, a number plate or anything else inside an ellipse, rectangle or rounded shape with adjustable position, size and soft edge, or blur everything except it; every setting can be keyframed so the area follows a moving subject.
- 58 new stickers: Symbols (check, cross, question, bulb, bolt, note, crown, trophy, gift, rocket, pin, play, camera, microphone, clapperboard), Nature (sun, cloud, rainbow, moon, snowflake, flower, leaf, raindrop), social call-outs (hashtag, mention, verified, views, link in bio, swipe up, watch now, new video, save, repost, like count, subscribers, comment below, thanks for watching, giveaway, trending, now playing, notifications on, five stars), five more emoji, badges (hot, free, best, 50% off), lines (zigzag, spiral, dotted wave) and animated shooting star, snowfall and bubbles.
- The pictures that tiles in Transitions, Effects and Filters are shown on are now real photographs (a lake at sunset, a swan, young swans, trees in the evening light, a cat) and a cut-out portrait for effects that need a face, instead of drawn scenes.
- Modern effects (19 new, in two new groups): **Cinematic** (Halation, Pro-Mist, Dream Glow, Anamorphic Streaks, Light Sweep, Spotlight) and **Trendy** (Beat Flash, RGB Pulse, Speed Lines, Echo, Camcorder with a blinking REC dot and running timecode, Fluid Colour, Prism, Liquid Warp, Punch Zoom, Impact Hit), plus Dust Motes, Embers and Mist under Particles. The beat effects take a tempo in beats per minute. The older effects are still there; the modern ones lead the library and the Trending row.
- Modern text: seven new title styles (Glass Card, Message Card, Poster Stack, Refined, Editorial, Gold Foil, Quote), a Modern group of ten titles and an Animated Captions group of nine; new text animations Letters Rise, Letters Pop, Words Pop, Tracking In, Karaoke Highlight (the words light up one after another), Slam and Flicker On.
- 42 new filters in seven groups (Minimal, Pastel, Creator, Lifestyle, Night, Nature, Travel): soft, matte, pastel and creator looks with lifted blacks, gentle desaturation and tinted highlights.
- 9 new transitions: Light Wipe (two directions), Zoom Punch, Film Roll (up and down), Shadow Fade and Whip Right, Up and Down. The old heart, star and hexagon wipes moved to the end as "Classic Shapes".
- A Silence Detection button in the timeline toolbar (next to mute), enabled when a clip with sound is selected.
- The picture at the top of the Export window is now a real frame of the timeline, drawn the way the export draws it (before it showed the live player, which is black wherever nothing is on screen).
- Fixed: the app crashed when it was minimized (while rendering or not) and opened again: a minimized window has no size and the panels' maths went negative; nothing is drawn while the window is that small now.
- Fixed: effects numbered above 79 drew nothing in software graphics drivers because of one huge `switch` in the effect shader; effects are now dispatched in three groups.
- Mac: the app is now signed ad hoc and checked inside the disk image (an unsigned downloaded app is often reported as "damaged"); the disk image has a read-me with the "Open Anyway" steps, and the build also offers the app as a zip.
- Faster exports: the graphics card draws video colours (NV12) so ffmpeg gets a third of the data and has no colour conversion to do; the encoder on the card that draws the picture is tried first (NVENC, AMF, Quick Sync, VideoToolbox); Preferences → Performance lists the card and the encoder each format uses.

**Library** — 68 titles, 73 transitions, 105 effects, 96 filters, 117 stickers,
21 templates, music and sound effects, all made by TidyCuts. Pictures show the
real result, filters show before and after, hover plays a loop, search covers
every tab.

**Export** — 13 formats, presets for widescreen HD and 4K, vertical phone video,
square posts, small files and web video, chapters from markers, GPU encoders.

**Safety** — crash recovery, autosave, every change can be undone.

**Assistants** — Claude and OpenAI Codex can edit with you through a built-in,
local-only connection that is off until you turn it on.
