# Third-party notices

TidyCuts is built from its own code plus the software and fonts listed here,
each under its own licence. Nothing below is changed except where noted.

## FFmpeg

TidyCuts runs FFmpeg (`ffmpeg` and `ffprobe`) as **separate programs** to read
and write video and audio. The builds shipped with TidyCuts are the
**LGPL** builds (no GPL parts, so no x264 or x265):

- Windows: the `win64-lgpl` build from <https://github.com/BtbN/FFmpeg-Builds>.
- macOS: built from the official source release at <https://ffmpeg.org/releases/>
  with `--disable-gpl --disable-nonfree` (see `.github/workflows/release.yml`).

FFmpeg is licensed under the GNU Lesser General Public License, version 2.1 or
later. The licence text is installed next to the programs in
`ffmpeg/FFMPEG-LICENSE.txt`. FFmpeg's source code is available from
<https://ffmpeg.org/download.html>; to use your own build, put it next to
TidyCuts or set `TIDYCUTS_FFMPEG_DIR`. FFmpeg is a trademark of Fabrice Bellard.

## Fonts

All fonts are in `crates/tidycuts-render/fonts/` and are free for any use,
including in the videos people make with TidyCuts.

| Font | Licence |
| --- | --- |
| Abril Fatface, Anton, Bangers, Bebas Neue, Black Ops One, DM Serif Display, Great Vibes, Inter, Lobster, Monoton, Pacifico, Poppins, Press Start 2P, Righteous, VT323 | SIL Open Font License 1.1 (`OFL-inter.txt`, `OFL-poppins.txt` and each font's own notice at <https://fonts.google.com>) |
| Permanent Marker, Special Elite | Apache License 2.0 (`APACHE-LICENSE.txt`) |

Inter has had its private-use characters removed so the icon font shows (see
`crates/tidycuts-render/fonts/README.md`).

## Icons

The icons come from [Phosphor Icons](https://phosphoricons.com), MIT licence,
through the `egui-phosphor` library.

## Rust libraries

The complete list, with versions and licences, is in
[docs/third-party-crates.md](docs/third-party-crates.md). They are all under
permissive licences. `cargo deny check` (see `deny.toml`) keeps it that way.

## Built-in content

The titles, transitions, effects, filters, stickers, templates, music and
sound effects are drawn or synthesised by TidyCuts itself, so there is nothing
third-party in them.

The sample pictures that library tiles are shown on (`crates/tidycuts-render/assets/`)
are photographs taken by the app's author, who gave their permission to
include them, and one cut-out portrait of the author.
