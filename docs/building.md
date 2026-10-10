# Building TidyCuts

TidyCuts is a Rust program. You need the stable Rust toolchain
([rustup.rs](https://rustup.rs)) and FFmpeg.

## All systems

```
cargo run --release -p tidycuts-app      # run it
scripts/check.sh                         # formatting, clippy and all tests
```

FFmpeg (`ffmpeg` and `ffprobe`) is found in this order: the folder named in the
`TIDYCUTS_FFMPEG_DIR` environment variable, next to the TidyCuts program (or in
an `ffmpeg` folder beside it, or in a macOS app's `Resources`), common install
folders, then your PATH. Any recent FFmpeg works. The builds we ship use an
LGPL one (see [../THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md)).

## Linux

```
sudo apt install ffmpeg libasound2-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libgtk-3-dev mesa-vulkan-drivers
```

The last package is a software Vulkan driver. With it, the graphics tests and the
whole program run on a computer without a graphics card (slowly), which is how
the automated checks work.

To look at the program without a screen, run it under a virtual display:

```
Xvfb :99 -screen 0 1600x960x24 &
DISPLAY=:99 cargo run -p tidycuts-app
```

## Windows

Install Rust with the MSVC toolchain and the Visual Studio Build Tools
("Desktop development with C++"). Then `cargo build --release -p tidycuts-app`.
Put `ffmpeg.exe` and `ffprobe.exe` in an `ffmpeg` folder beside `tidycuts.exe`.

The installer is built by GitHub Actions (see [releasing.md](releasing.md)). To
build it by hand, install [Inno Setup](https://jrsoftware.org/isinfo.php), put the
program and the `ffmpeg` folder in `dist\TidyCuts`, and run
`iscc /DAppVersion=1.0.0 packaging\windows\tidycuts.iss`.

## macOS

`cargo build --release -p tidycuts-app`, then
`scripts/make-macos-app.sh target/release/tidycuts dist` makes `dist/TidyCuts.app`,
and `scripts/make-macos-dmg.sh dist dist/TidyCuts.dmg` makes the disk image.
If a folder `ffmpeg-macos` holding `ffmpeg` and `ffprobe` is present, the app
bundle includes them; otherwise install FFmpeg yourself (`brew install ffmpeg`).

The app isn't signed. Signing and notarizing it needs an Apple Developer
account; until then people open it with right-click, Open the first time.

## The app icon

The icon is drawn by `scripts/make_icon.py` (needs Python with Pillow and
ImageMagick) and the results are kept in `packaging/icons/`: the Windows
`.ico`, the macOS iconset (turned into an `.icns` by `scripts/make-macos-app.sh`)
and the 256-pixel picture the window uses. To change the icon, edit the script
and run `python3 scripts/make_icon.py` from the repository root.

## Where things are saved

| What | Where |
| --- | --- |
| Settings, backups, the sounds the program makes | the system's config folder, `TidyCuts` |
| Preview cache and proxies | the system's cache folder, `TidyCuts` (changeable in Preferences) |
| Your projects | wherever you save them (`.tcproj`) |
