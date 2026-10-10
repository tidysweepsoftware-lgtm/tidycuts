<div align="center">

<img src="docs/images/banner.png" alt="TidyCuts: the free, open-source video editor for Windows and Mac" width="100%">

<br><br>

<a href="../../releases/download/v1.0.0/TidyCuts-Setup-1.0.0.exe"><img src="docs/images/btn-windows.png" alt="Download TidyCuts for Windows" width="250"></a>&nbsp;&nbsp;&nbsp;<a href="../../releases/download/v1.0.0/TidyCuts-1.0.0-macos.dmg"><img src="docs/images/btn-mac.png" alt="Download TidyCuts for Mac" width="250"></a>

<sub>Windows 10 and 11 &nbsp;·&nbsp; Mac with Apple silicon (M1 or newer)</sub>

<sub>Version 1.0.0 · Free · No account · <a href="../../releases/latest">All downloads, including a portable Windows version</a></sub>

<br>

<img src="docs/images/stats.png" alt="68 titles, 105 effects, 96 filters, 73 transitions, 117 stickers and 70 sounds, all built in" width="100%">

<br><br>

<img src="docs/images/features.png" alt="Easy to pick up. Everything included. Private and fast." width="100%">

</div>

## See it in action

<p align="center">
  <img src="docs/images/shot-effects.png" alt="The Effects library: 105 effects, each shown on a real picture" width="49%">
  <img src="docs/images/shot-blur.png" alt="Blur Area and Pixelate Area hide a face or any area" width="49%">
  <img src="docs/images/shot-silence.png" alt="Silence Detection cuts the pauses out of a clip" width="49%">
  <img src="docs/images/shot-thumbnail.png" alt="The thumbnail maker puts big lettering on any frame" width="49%">
</p>

<p align="center"><sub>Effects · Blur a face · Silence Detection · Thumbnail maker</sub></p>

Plus a magnetic timeline with up to 10 video and 5 audio tracks, speed ramps and
keyframes · colour wheels, curves, LUTs and scopes · voiceover from typed text ·
scene and beat detection · export to 13 formats with graphics-card encoding · and an
optional link that lets an AI assistant edit with you (off until you turn it on).

[Full feature list](docs/features.md) · [More screenshots](docs/screenshots.md)

## Downloads

| Platform | File | For |
| --- | --- | --- |
| Windows | [TidyCuts-Setup-1.0.0.exe](../../releases/download/v1.0.0/TidyCuts-Setup-1.0.0.exe) | Installer for Windows 10 and 11 |
| Windows | [TidyCuts-1.0.0-windows-portable.zip](../../releases/download/v1.0.0/TidyCuts-1.0.0-windows-portable.zip) | No install: unzip and run |
| Mac | [TidyCuts-1.0.0-macos.dmg](../../releases/download/v1.0.0/TidyCuts-1.0.0-macos.dmg) | Apple silicon, macOS 11 or later |
| Mac | [TidyCuts-1.0.0-macos.zip](../../releases/download/v1.0.0/TidyCuts-1.0.0-macos.zip) | The same app, if the disk image gives trouble |

FFmpeg is included. Checksums for every file are on the [releases page](../../releases/latest).
Intel Macs are not supported yet.

## How it was made

TidyCuts was written by an AI. **Claude**, made by Anthropic, wrote the code, tests,
built-in content and documentation. The owner of TidySweep Software decided what to
build and tested every version on real Windows and Mac computers.
[Read the whole story, including what wasn't tested](docs/how-this-was-made.md).

## Open source

Released under the [Apache License 2.0](LICENSE): use it, change it and share it.
[Report a bug](../../issues/new/choose) · [Contribute](CONTRIBUTING.md) ·
[Security](SECURITY.md) · [Privacy](PRIVACY.md) · [Build it yourself](docs/building.md)

## Help installing

TidyCuts isn't signed with a paid certificate yet, so Windows and macOS may show a
warning the first time. It's about who published the file, not what's in it, and you
can read all of the code here.

<details>
<summary><b>Mac: "cannot verify that it is free of malware"</b></summary>
<br>

1. Click **Done** (not "Move to Trash").
2. Open the Apple menu → **System Settings** → **Privacy & Security**.
3. Scroll to **Security**, find *"TidyCuts" was blocked to protect your Mac*, and click **Open Anyway**.
4. Enter your Mac password or use Touch ID, then click **Open**.

It opens normally from then on.
</details>

<details>
<summary><b>Mac: "TidyCuts is damaged and can't be opened"</b></summary>
<br>

The app isn't damaged: macOS marks downloaded files and sometimes reacts badly to the mark.
Open **Terminal**, paste this line, press Return, and open TidyCuts again:

```
xattr -cr /Applications/TidyCuts.app
```

If the disk image itself won't open, use the `.zip` instead.
</details>

<details>
<summary><b>Windows: "Windows protected your PC"</b></summary>
<br>

Click **More info**, then **Run anyway**.
</details>

More detail, checksums and where TidyCuts keeps its files: [docs/install.md](docs/install.md).

<br>

<div align="center">
<sub>A <b>TidySweep Software</b> project · <a href="CHANGELOG.md">What's new</a> · <a href="docs/install.md">Install help</a> · <a href="docs/how-this-was-made.md">How it was made</a></sub>
</div>
