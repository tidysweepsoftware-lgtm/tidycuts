# Everything TidyCuts can do

This is the full list. The [project page](../README.md) has the short version
with pictures, and [shortcuts.md](shortcuts.md) lists the keyboard shortcuts.


### Layout and library

**Layout.** Library top left, player top right, timeline along the bottom.
Double-click a clip and its properties open in the library's place.

**Library.** Media, Audio (9 music tracks and 61 sound effects), Titles (160),
Transitions (130), Effects (148), Filters (96), Stickers (224), Layouts (16 split
screens and 7 audio visualizers) and Templates (21). Every tile shows the real result on a sample picture; filters
show before and after; hovering plays a short loop; templates show their length
and clip count; the search box covers every tab; each tab starts with Trending
and then its categories.

**Timeline.**
- Up to 10 video tracks and 5 audio tracks, for A-roll and B-roll editing.
- A magnetic main track: cutting or deleting closes the gap, and clips on other
  tracks after the cut move with it.
- A scissors button on the playhead, markers, freeze frame and snapshot.
- Drag clips to move them and their edges to trim; drag the ruler to zoom, click
  it to jump, drag the playhead to scrub.
- The right-click menu has the usual clip commands: cut, copy, paste, duplicate,
  split, ripple delete, crop and zoom, rotate, speed, reverse, detach audio,
  mute, replace, colour marks and more.

**Clip properties.**
- Video: transform, crop, opacity, blend modes, masks, drop shadow, auto enhance,
  stabilisation, lens correction, denoise, upscaling and sharpening.
- Audio: **Enhance Speech** (one click for a talking voice), **Voice Clean-up**
  (Studio Voice, a neural network that keeps the voice and turns down fans, hiss,
  traffic and room noise; it runs on your computer), Leveler (lifts quiet speech), a
  peak limiter, volume, fades, pan, pitch, voice changer, equaliser, Normalize (turns
  down only what is too loud), hiss, wind and hum removal, de-esser, ducking.
- Color: exposure, contrast, temperature and tint, highlights and shadows,
  saturation, HSL for eight colour ranges, curves, and LUTs (built-in looks or
  your own `.cube` files, kept inside the project).
- Speed: constant speed, pitch-correct audio, reverse, and speed ramping with
  presets (Montage, Hero Moment, Bullet Time…) and a draggable graph.
- Animation: keyframes with easing, plus in, out and loop presets.

**Smart cuts.** Remove the silent parts of a clip, split at scene changes, put
markers on the beat, and line clips up by their sound (for several cameras or a
separate recorder).

**Hide a face or anything else.** Effects → Blur → Blur Area or Pixelate Area:
choose an oval, a rectangle or a rounded shape, move and resize it, soften the
edge, or blur everything except the shape. Every setting can be keyframed, so the
shape can follow a moving face.

**Modern titles and captions.** Titles (160) include glass cards, editorial and
poster styles, and animated captions: word pop, letters rising or popping in,
tracking in, flicker on, karaoke highlighting and more. Letter and word
animations are drawn by TidyCuts, one glyph at a time.

**Modern looks.** Cinematic and Trendy effects (halation, pro-mist, beat flash,
RGB pulse, speed lines, camcorder, prism, punch zoom…), light leaks, particles
and 96 filters in styles such as Minimal, Pastel, Creator, Lifestyle, Night,
Travel and Film. Transitions include light leaks, whip pans, glitches and warps.

**Voiceover.** Type some text, pick one of the computer's own voices, and it is
read aloud onto an audio track. It works offline and uses no AI service.

**Render preview.** Parts of the timeline that are hard to play (effects,
transitions, many layers, very big videos) get an amber mark on the ruler. The
lightning button next to the mixer button draws them ahead of time, and they
turn green and play smoothly. Editing a part makes its preview stale again; the
files are only for playback, and exports never use them. View menu → Delete
Preview Files clears them.

**Thumbnails.** Pick a video frame, a photo or the timeline, type the words, choose
a lettering style and where it sits, and save a PNG or JPG (widescreen, square or
vertical). Edit menu → Make Thumbnail.

**Image Thumbnail Generator.** The sparkle button beside the timeline (or Edit menu → Image
Thumbnail Generator). Type an idea and get four pictures drawn on your computer; pick a look
and a shape, add up to three inspiration pictures to copy their colours and mood, or a photo of
a person to draw in. *My photo in a scene* keeps a real photo exact and draws the scene around it.
*Add words* opens the thumbnail maker with the picture. The first time you choose and download
the models (Standard 5.1 GB or High quality 8.7 GB, once); after that it works offline, and
both sizes can be kept and chosen per thumbnail.

**Colour.** Sliders, HSL ranges, curves, LUTs and four colour wheels (lift, gamma,
gain, offset) for fine colour work. View menu → Scopes shows a waveform, an RGB
parade, a vectorscope and a histogram of what the preview shows.

**Sound meter and loudness.** A level meter is always on at the right edge of the timeline
(two bars, a decibel scale, a held peak, a clip lamp and a live loudness number). View >
Loudness shows how loud the whole video is; the Export window can set it to a target.

**Auto Captions.** Select a clip with speech and press the subtitle button beside the timeline
(or Titles tab > Auto Captions, or Clip menu). Pick the language (99 are understood, or let it
detect), and TidyCuts listens on your computer with Whisper and adds the words as subtitle
titles you can edit. The speech model (574 MB) is downloaded once, the first time. It copes
with soft and whispered voices, leaves out sounds such as sighs, and puts each caption on
screen only while the voice is there. **Lower Under Speech** (Clip menu or the Audio tab)
dips a music clip under every spoken part.

**Normalize and Enhance Speech.** On the Audio tab, **Normalize** is one click and only turns
down what is too loud: anything above its limit (-18 LUFS, changeable with the Loudest level
slider) is lowered smoothly, everything else is left exactly as it is, nothing is made louder
and peaks never clip. **Enhance Speech** removes background noise from a voice (Studio Voice,
included), adds a podcast tone and turns Normalize on.

**Stabilization.** Video tab > Stabilization watches the clip once, works out how the camera
moved, and takes the shake out (a walking person in the picture does not fool it). The
picture is zoomed in a little to hide the edges.

**Silence Detection.** The button in the timeline toolbar (beside mute), or Clip menu → Silence Detection: set how quiet counts as
silence, how long a pause must last and how much to keep either side, press
Analyze, see the quiet parts marked on the waveform, click any you want to keep and
Export to Timeline.

**Layouts.** View menu → Layout: Standard, Mirrored, or a left-menus layout where the
library and the properties share the left side next to the preview. Drag the edges
of the panels to resize them.

**Audio mixer.** Volume, mute and solo for every track, a master fader and level
meters.

**Big videos.** Proxies (small copies) are made automatically for videos above
1080p, so editing stays smooth; export always uses the original.

**Safety.** Every change is one undo step; autosave; after a crash TidyCuts
offers your unsaved work back.

**Export.** MP4, MOV, MKV, WEBM, AVI, WMV, MPEG-2, TS, GIF, MP3, WAV, M4A and
FLAC. Presets for widescreen HD and 4K, vertical phone video, square posts, small files and web video; quality,
resolution, frame rate, codec, GPU encoders, bitrate, a range, and chapters from
your markers.

**AI assistants.** Off by default. Turn it on in Preferences and Claude Code or
OpenAI Codex can edit the open project with you, locally and with a secret key.
See [assistants.md](assistants.md).

Previews and export run on the GPU (wgpu: Direct3D 12 / Vulkan on Windows,
Metal on macOS).
