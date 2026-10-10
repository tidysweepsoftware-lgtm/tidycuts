# TidyCuts: notes for Claude (and other AI assistants)

TidyCuts is a free, open-source video editor for Windows and Mac, written in Rust
(egui + wgpu; FFmpeg is run as a separate program). Read
[docs/development-notes.md](docs/development-notes.md) first: what the project is, what
the owner asked for, how it is built and tested, and lessons learned.

## Build and check

- Run: `cargo run --release -p tidycuts-app` (needs Rust and FFmpeg; see docs/building.md).
- Before every commit: `scripts/check.sh` (format, clippy with warnings as errors, all
  tests). CI uses a newer Rust, so also run `scripts/check.sh +1.99.0` before pushing.
- Releases: docs/releasing.md. The *Installers* workflow builds the Windows installer, the
  portable zip and the Mac disk image, and makes a draft release.

## Rules that matter

- Every user action is one undo step (`app.edit(|p| ...)`). New saved fields get
  `#[serde(default)]`. Catalog ids (`basic.title`, `filter.vivid`...) are saved in projects:
  never rename one.
- Everything built in (titles, effects, filters, stickers, templates, music, sounds) is
  drawn or synthesised by the program itself. Do not add stock photos, footage, presets
  or other third-party artwork.
- Do not name other companies' products, or use their logos, in the app, the docs or the
  code. Say "widescreen", "vertical" and "square", not brand names.
- User-facing words are plain English, sentence case, no jargon.
- Check what you build by looking at it: render the contact sheets
  (`crates/tidycuts-render/examples`) and run the app under a virtual display; see
  docs/building.md.
- The owner is not a programmer. Keep messages short and plain, say what is and isn't
  tested, and ask before anything costly or public (builds on Mac runners, releases,
  publishing, renaming).
