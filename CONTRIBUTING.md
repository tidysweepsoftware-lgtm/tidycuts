# Contributing to TidyCuts

Thank you for helping. TidyCuts is a desktop video editor written in Rust.
This page tells you how to get set up, how a change travels from your computer
into the program, and what we look for in one.

## Getting set up

1. Install [Rust](https://rustup.rs) (the stable toolchain) and FFmpeg
   (`ffmpeg` and `ffprobe` on your PATH).
2. On Linux, install the libraries: `libasound2-dev libxkbcommon-dev
   libxkbcommon-x11-dev libgtk-3-dev`, and a Vulkan driver (`mesa-vulkan-drivers`
   gives a software one).
3. Run it: `cargo run --release -p tidycuts-app`.

More detail, including Windows and macOS, is in [docs/building.md](docs/building.md).

More background (what the project is, how it is built and tested, lessons learned) is in
[docs/development-notes.md](docs/development-notes.md).

## Making a change

- Start from `main` and keep each change to one thing.
- Run `scripts/check.sh` before you commit. It formats the code, runs clippy
  with every warning treated as an error, and runs the whole test suite. A
  change that doesn't pass doesn't go in.
- Add a test for anything you can test. Most of the editing logic lives in
  `tidycuts-core` and `tidycuts-engine` and has no window, so tests are cheap.
  Tests that need a graphics card or FFmpeg skip themselves when it's missing.
- Write the user-facing words in plain English, in sentence case, without
  jargon. The people using TidyCuts are video makers, not programmers.

## How the code is laid out

| Crate | What it does |
| --- | --- |
| `tidycuts-core` | The project model and the edits you can make to it. No graphics, no files besides the project file. |
| `tidycuts-media` | FFmpeg as a helper program (probing, decoding) and audio output. |
| `tidycuts-render` | The GPU compositor, the shaders, and the built-in titles, effects, filters, stickers and sample pictures. |
| `tidycuts-engine` | Decoding, previews, the audio mixer, export, and analysis (silence, scenes, beats, sync, proxies). |
| `tidycuts-sounds` | The music beds and sound effects, made in code. |
| `tidycuts-mcp` | A small server that lets Claude or Codex drive the editor. |
| `tidycuts-app` | The window, built with egui. |

Habits worth knowing:

- Every change a person makes is one undo step. Use `app.edit(|project| …)`.
- New fields on saved data get `#[serde(default)]`, so old projects still open.
- Built-in content is made by the program itself. Please don't add stock photos,
  recordings or other third-party artwork.
- Ids in the catalogs (`basic.title`, `filter.vivid`…) are saved in projects.
  Never rename one.

## Using AI tools

TidyCuts was itself written with Claude's help (see
[docs/how-this-was-made.md](docs/how-this-was-made.md)), so AI-assisted changes are
welcome. The rules are the same as for any change: it must pass
`scripts/check.sh`, you must have read and understood it, and it must not contain
anything you don't have the right to share. If an AI helped, say so in the commit
message.

## Reporting a problem

Open an issue and tell us what you did, what you expected, and what happened.
For a security problem, please read [SECURITY.md](SECURITY.md) first.

## Licence of your contribution

By sending a change you agree that it is released under the same licence as the
rest of TidyCuts (see the LICENSE file).
