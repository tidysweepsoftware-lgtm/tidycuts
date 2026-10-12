# Changes

All notable changes to TidyCuts are listed here, newest first.

## 1.3.0 (a gentler Normalize and the Image Thumbnail Generator)

**Sound**
- **Normalize** now only ever turns sound down. It listens to how loud a clip is the way
  people hear it and, when that goes above a limit (-18 LUFS unless you change it with the new
  **Loudest level** slider, -30 to -8), lowers it smoothly by just enough. Everything at or
  under the limit goes through exactly as it is, nothing is ever made louder, and peaks stay
  under -1 dBFS. The 1.2.0 version lifted quiet sound by up to 15 dB and squeezed it with a
  filter that made normal sound rise and fall, which could be painfully loud. On a test
  recording of speech at -33, -24, -14 and -8 LUFS, the first two parts came out identical
  to the original, the other two at about -18 LUFS, nothing was turned up, and normal speech
  right after the loudest part was lowered by 0.3 dB at most.
- **Enhance Speech** no longer lifts quiet voices; it uses the new Normalize. The **Leveler**
  stays as its own tool for lifting quiet speech.
- Normalize works at once: it no longer waits for a loudness measurement.

**Thumbnails**
- **Image Thumbnail Generator** (the sparkle button beside the timeline, or Edit menu): type
  your idea and get four pictures to choose from, drawn on your computer, free and with no
  account. Pick a look (bold, cinematic, dark, clean or cartoon) and a shape (widescreen,
  square or vertical). Add up to three **inspiration** pictures and the new ones copy their
  colours, lighting and mood, or add a photo of a person to draw in (the face comes out
  similar, not identical). **My photo in a scene** keeps a real photo exact (a product, say):
  the scene is drawn around it, with its plain white background taken out and a glow and
  shadow added. **Add words** opens the thumbnail maker with the picture, so the title is
  always in clear, correctly spelled letters.
- It draws with FLUX.2 [klein] 4B (Apache 2.0) and Qwen3 4B (Apache 2.0), run by
  stable-diffusion.cpp (MIT). The first time, a window explains the download and lets you
  choose **Standard** (5.1 GB) or **High quality** (8.7 GB); nothing is downloaded until you
  press its button. You can keep both and pick one for each thumbnail, and change or delete
  them later (Manage, or Preferences > Image generator). On our test computer a picture takes
  about seven seconds once the models are loaded.

## 1.2.0 (sound, captions and steadier video)

**Sound**
- **Normalize** was rebuilt to do the whole job in one click. It evens out loud and quiet
  parts, brings the clip to -16 LUFS, and a look-ahead peak limiter after its last gain makes
  sure nothing clips. On a test recording whose three parts were 23 loudness units apart
  (-33, -19 and -10 LUFS), the old Normalize only turned everything down by 3 dB; the new one
  leaves them within 2 units (-16.9, -16.3 and -15.1) at -16.1 overall, with the peak held at
  -1.0 dBFS. Music stays natural (-14.0 to -15.9 LUFS, no clipping).
- **Enhance Speech**: one button on the Audio tab for a talking voice. It switches on Voice
  Clean-up, a rumble cut, level evening, a podcast tone and a peak limiter.
- **Voice Clean-up** with **Studio Voice**: a speech-enhancement network (MossFormer2,
  Apache-2.0, run through ONNX Runtime, both included) separates the voice from everything
  else, entirely on your computer. On our test recordings (speech with hiss, room noise, a
  fan with hum, street noise, camera noise and talking in the background) the old noise
  remover took away 0.6 dB of noise; the small RNNoise engine used before took away 26 dB;
  Studio Voice takes away about 37 dB at full strength, and the voice scores 3.4 of 5 on
  Microsoft's DNSMOS quality model (3.5 for the clean original). A 4-minute clip takes about
  20 seconds. Without the extra files the small RNNoise engine is used. The cleaned sound is
  made ahead of time and kept in the cache; the original plays until it is ready, and an
  export waits for it.
- **Leveler** and **Peak Limiter** (Volume section): lift a recording that is too quiet by up
  to 30 dB without letting loud parts clip. **Hiss Removal** replaces the old Noise Removal
  slider, with a tuned filter that only touches steady hiss.
- **Sound meter**, always on at the right edge of the timeline: two level bars with a decibel
  scale, a held peak, a lamp that lights when the sound reaches full scale (click to clear)
  and a live loudness number in LUFS.
- **Loudness** (View menu, or click the number under the meter): how loud the whole video is,
  its highest peak and loudness range, and how that compares with the streaming standard. The
  Export window has a **Loudness** choice (Streaming -14, Broadcast -23) that brings the
  finished video to that level (tested: within 0.3 LU on speech).
- **Lower Under Speech** (auto-duck): lowers a music clip wherever someone is talking. The
  speech is found with RNNoise's voice detector (all speech found, with no false alarms on
  test recordings, with and without noise). Clip menu, right-click menu or the Audio tab.
- **Hearing protection** (Preferences > General > Listening): keeps playback on headphones
  from getting loud. It turns the volume down only when the whole mix is loud, and does not
  change what you export.

**Captions**
- **Auto Captions** were rebuilt on **Whisper** (OpenAI, MIT licence), run by the whisper.cpp
  program that comes with TidyCuts. It listens to soft and whispered voices that the earlier
  engines mishear, understands 99 languages (the Language list has the common ones and
  "Detect the language"), and takes about a quarter of the clip's length on an ordinary
  computer. It is given only the stretches that contain a voice, so it does not invent text
  out of silence; sounds such as sighs and laughs are left out of the captions, and each word
  is placed where it is spoken (on a test where the true timing is known, no word starts
  before its line, where an earlier version put 13 of them up to 1.9 seconds early).
- The Whisper model (574 MB) is downloaded once, the first time you press **Download the
  speech model** in the Auto Captions window; after that captions work offline. The window
  checks the file against its checksum.
- Captions are made as normal title clips on a new track in one undo step. Titles tab, Clip
  menu, right-click menu or the button beside the timeline.

**Video**
- **Stabilization** was rebuilt. The old filter made steady clips shake. The clip is now
  watched once at low resolution (about 30 times faster than playing it), the camera's
  movement (sideways, up and down, and a small turn) is measured from many small blocks, and
  anything that moves differently from most of the picture, such as a walking person, is
  ignored. The camera's path is smoothed and each frame is moved by the difference. The
  picture is zoomed in by just enough to hide the edges. Tested on a clip with known shake:
  88% of the frame-to-frame shake removed at the default setting for a 6% zoom. Export waits
  for the analysis.
- **Auto Enhance** has a better colour balance: it trusts a colour cast only when the picture
  has many different hues (a sunset stays warm), moves the brightness towards a natural level
  by at most one stop, stretches flat pictures and lifts dull colours.
- Fixed: changing the speed to slower, faster and back to 1x now returns the clip and the
  clips after it to exactly where they were; Track Matte no longer turns the picture black
  when there is no clip above to use as the stencil; the Background section that fills empty
  space around a clip is now called **Frame Fill** and tells you when the clip already fills
  the whole frame.

**Also**
- **A welcome tour** the first time the program starts (Help > Take the Tour), and **Help >
  Check for Updates**, which asks GitHub only when you click it (the Microsoft Store version
  updates itself).
- Normalize now works in exports and in assistant edits (it was only measured when the switch
  was clicked in the window).
- The assistant link has `enhance_speech`, `auto_duck`, `measure_loudness`, `transcribe_clip`
  and `add_captions` tools, and `export_video` can set a loudness.

## 1.1.0 (the modern library)

The libraries were rebuilt in a modern style. The old designs are still there
behind an **Old designs** switch, and every project saved with 1.0.0 opens as
before.

**New designs**
- **Titles (160 listed, 223 in all):** cards that build themselves in stages (name cards, news bars, pills), clean and soft titles, bold and glossy 3D, gold and metal, neon and glass, script and elegant, tech and gaming, sport and fitness, business and events, 16 captions whose highlight follows the words, and 16 end credits. 54 new open fonts (74 in all).
- **Transitions (130 listed, 172 in all):** motion-graphic bars, circles, rings, colour waves and stepped blocks; brush strokes and paper tears; dust dissolves; camera shutter, speed rush and photo freeze; gold, silver and ice light sweeps; frosted panes and card pushes; glass balls, glass slabs, tile flips, torus rings, shattering glass and cracking ice.
- **Effects (148 listed, 214 in all):** 25 camera shakes and zooms, 10 flicker styles, glass shatter and cracks, glitter and starbursts, grunge, neon frames, fireworks, 3D layouts, 16 beat effects that follow a tempo, and 17 sound-reactive effects that move with the loudness, bass, mid or treble of the clip's own sound.
- **Filters (96 listed, 192 in all):** genre, soft matte, skin tones, film stocks, city night, colour maps, log recovery, gaming, business and food looks.
- **Stickers (224 listed, 341 in all):** animated call-outs (subscribe, follow, like, share, comment, bell, link, swipe), words in four treatments, marker marks that draw themselves, bursts, confetti and balloons, fire, smoke, lightning and energy, ratings, discount seals, price tags, ribbons, map pins, check seals, and screen graphics.

**Finding things**
- A row of **use chips** under the library search (business, gaming, travel, reviews, vlog, education, food, fitness, real estate, music, events) filters every tab to what suits that kind of video. Searching also finds uses.
- The library shows the new designs; **Old designs** brings the earlier ones back.
- Ten quick buttons beside the timeline for things that were only in menus: freeze frame, marker, detach audio, trim to playhead, smart cuts (scene changes, beat markers, line up by sound), rotate, voiceover, snapshot, thumbnail and scopes.

**Pictures**
- The sample pictures on the tiles are now public-domain (CC0) photographs, listed with their sources in THIRD-PARTY-NOTICES.md.
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
