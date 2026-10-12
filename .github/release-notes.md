## TidyCuts __VERSION__

A big update to the free and open-source video editor for Windows and Mac: the
libraries were rebuilt in a modern style (160 titles, 148 effects, 96 filters,
130 transitions and 224 stickers listed by default, with the old designs behind
an **Old designs** switch), a use filter for every library (business, gaming,
travel, reviews and more), sound-reactive effects, and quick buttons beside the
timeline. Projects saved with 1.0.0 open as before. The full list is in the
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

FFmpeg is included. Everything TidyCuts makes (titles, effects, filters,
stickers, music) is made by TidyCuts itself and is free to use in your videos.
The fonts and the sample photographs on the library tiles are open licences or
public domain; they are listed in THIRD-PARTY-NOTICES.md.

What it does, how it was made, and the licence are on the
[project page](https://github.com/__REPO__#readme).
