## TidyCuts __VERSION__

A faster Image Generator and smoother editing, in the free and open-source video editor
for Windows and Mac:

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
