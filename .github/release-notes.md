## TidyCuts __VERSION__

Better sound, captions that work on soft voices, and steadier video, in the free and
open-source video editor for Windows and Mac:

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

FFmpeg, ONNX Runtime and the whisper.cpp program are included. Everything TidyCuts makes
(titles, effects, filters, stickers, music) is made by TidyCuts itself and is free to use in
your videos. The fonts and the sample photographs on the library tiles are open licences or
public domain; they are listed in THIRD-PARTY-NOTICES.md.

What it does, how it was made, and the licence are on the
[project page](https://github.com/__REPO__#readme).
