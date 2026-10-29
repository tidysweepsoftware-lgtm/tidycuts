## TidyCuts __VERSION__

The picture-model and speech-model downloads are now written by TidyCuts itself, which fixes the "curl: (23)" error in the Microsoft Store version, and a full disk or blocked folder is reported in plain words. Voiceovers use the Windows voices directly (1.3.6). Everything from 1.3.5 is included:

Edit by Text, Auto Reframe, an export queue, a Report button for AI pictures and now **Linux**, in the free
and open-source video editor for Windows, Mac and Linux:

- **Edit by Text**: the words of a clip as text. Cross out words (or the ums and uhs) and the matching
  pieces of the video are cut out and the gaps close.
- **Auto Reframe to Fill the Screen**: turns a wide video into a vertical or square one and follows what moves.
- **Export queue**: add another export while one is running; they run one after the other.
- **Image Generator**: **More like this** draws new pictures in the style of the one you chose, and **Report**
  lets you flag an inappropriate AI picture (Microsoft Store policy 11.16).
- **Keyboard Shortcuts** list in the Help menu, drop video files from the Desktop straight onto the timeline,
  Ctrl+C / Ctrl+X / Ctrl+V on the timeline, a trash button on tracks, double-click the sound meter for the mixer.
- **Keep my computer responsive while exporting** (on by default), and Auto Enhance no longer turns faces grey.
- **Linux**: AppImage (any distribution), .deb (Ubuntu, Debian, Mint) and .tar.gz.

From version 1.3.4:

- **Skimming and smooth scrubbing**: move the mouse over a paused timeline to see the picture under
  it without moving the playhead; dragging the playhead no longer hitches, jumps while playing show
  the new place at once, and you can grab the playhead while the video plays.
- **Proxies are optional**: a large video put on the timeline now asks whether to make a
  smooth-editing copy, and you can remember the answer.

From version 1.3.1:

- **Image Generator**: pictures appear one by one as they are drawn, the models stay loaded
  between presses, sizes from 720p to 4K, 1 to 4 pictures, a large preview, and Save opens on
  the Desktop.
- **Faster export** with a loudness target (about ten times faster checking), only two
  smooth-editing copies are made at once, and smoother plain cuts.

From version 1.3.0:

- **Image Thumbnail Generator**: type an idea and get four pictures to choose from, drawn on
  your computer for free. Add inspiration pictures to copy a look, draw a person in, or place a
  real photo (a product) into a drawn scene. The first time you choose which models to
  download (Standard 5.1 GB or High quality 8.7 GB), and nothing downloads until you press the
  button. Then **Add words** puts your title on in clear letters.
- **Normalize** now only turns down what is too loud: quiet and normal sound is left exactly as
  it is, nothing is made louder, and peaks never clip. **Enhance Speech** no longer lifts quiet
  voices.

From the version before (1.2.0):

- **Normalize** is now one click: it evens out loud and quiet parts, brings the clip to a
  comfortable loudness and never lets it clip. **Enhance Speech** removes background noise
  from a voice (the new Studio Voice, included), with a **sound meter** always visible
  beside the timeline, a **Loudness** readout and an export loudness target, **Lower Under
  Speech** for music, and **hearing protection** for headphone listeners.
- **Auto Captions** were rebuilt on Whisper: 99 languages, soft and whispered voices, and
  captions that appear only while someone is talking. The 574 MB speech model is downloaded
  once, the first time you use them.
- **Stabilization** was rebuilt to really calm shaky clips, **Auto Enhance** has better
  colour, and fixes for Speed and Track Matte.

Projects saved with earlier versions open as before. The full list is in the
[change log](https://github.com/__REPO__/blob/main/CHANGELOG.md).

### Downloads

| You have | Get this file |
| --- | --- |
| **Windows 10 or 11** (recommended) | `TidyCuts-Setup-__VERSION__.exe`: the installer |
| **Windows**, no installing | `TidyCuts-__VERSION__-windows-portable.zip`: unzip anywhere and run `TidyCuts.exe` |
| **Mac** with Apple silicon (M1 or newer) | `TidyCuts-__VERSION__-macos.dmg`: open it and drag TidyCuts to Applications |
| **Mac**, if the disk image is awkward | `TidyCuts-__VERSION__-macos.zip`: the same app in a zip |

`SHA256SUMS.txt` lists the checksum of every file, so you can check that a
download is the one we built.

### Opening it the first time

- **Windows:** the installer is not signed with a paid certificate yet, so
  Windows SmartScreen may say "unknown publisher". Choose **More info**, then
  **Run anyway**.
- **Mac:** the app is not notarized by Apple yet, so macOS may say it cannot
  check it for malware. Open **System Settings > Privacy & Security**, scroll
  down and choose **Open Anyway** next to TidyCuts. If macOS says the app is
  "damaged", run `xattr -cr /Applications/TidyCuts.app` once in Terminal.

FFmpeg, ONNX Runtime, the whisper.cpp program and the stable-diffusion.cpp program are included. Everything TidyCuts makes
(titles, effects, filters, stickers, music) is made by TidyCuts itself and is free to use in
your videos. The fonts and the sample photographs on the library tiles are open licences or
public domain; they are listed in THIRD-PARTY-NOTICES.md.

What it does, how it was made, and the licence are on the
[project page](https://github.com/__REPO__#readme).
