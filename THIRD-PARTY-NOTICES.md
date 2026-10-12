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

## RNNoise (Voice Clean-up)

Voice Clean-up uses the RNNoise neural network by Jean-Marc Valin (Xiph.Org,
Mozilla), through its Rust port **nnnoiseless** by Joe Neeman. It runs inside
TidyCuts, on your computer, and the network's weights are part of the crate.
Both are under the **BSD 3-Clause** licence:

- <https://github.com/xiph/rnnoise>
- <https://github.com/jneem/nnnoiseless>

Copyright (c) 2017 Mozilla, 2008-2018 Xiph.Org Foundation, 2013 Jean-Marc
Valin, CSIRO and Octasic. Redistribution and use in source and binary forms
are permitted under the conditions of that licence; the names of the
copyright holders may not be used to endorse products made with it.

## Studio Voice (voice model and ONNX Runtime)

- **MossFormer2_SE_48K**, a speech-enhancement network from ClearerVoice-Studio
  (Alibaba Speech Lab), under the **Apache License 2.0**:
  <https://github.com/modelscope/ClearerVoice-Studio> and its model card at
  <https://huggingface.co/alibabasglab/MossFormer2_SE_48K>. Per that model card it was
  trained on large datasets that include open-source and private data. TidyCuts
  includes the weights converted to the ONNX format (`models/mossformer2_se_48k.onnx`);
  `tools/voice-model/` has the conversion script. No other change was made.
- **ONNX Runtime** by Microsoft, under the **MIT** licence, shipped as a library next
  to the program (`onnxruntime/`, with its own `ONNXRUNTIME-LICENSE.txt` and
  `ONNXRUNTIME-THIRD-PARTY-NOTICES.txt`): <https://github.com/microsoft/onnxruntime>.
  The Rust crate **ort** (MIT or Apache-2.0) loads it.

## Fonts

All fonts are in `crates/tidycuts-render/fonts/` and are free for any use,
including in the videos people make with TidyCuts.

| Font | Licence |
| --- | --- |
| Abril Fatface, Anton, Bangers, Bebas Neue, Black Ops One, DM Serif Display, Great Vibes, Inter, Lobster, Monoton, Pacifico, Poppins, Press Start 2P, Righteous, VT323 | SIL Open Font License 1.1 (`OFL-inter.txt`, `OFL-poppins.txt` and each font's own notice at <https://fonts.google.com>) |
| Allura, Archivo Black, Audiowide, Big Shoulders Display, Bowlby One, Bricolage Grotesque, Bungee, Bungee Inline, Caveat, Chakra Petch, Chango, Cinzel, Cinzel Decorative, Courgette, Damion, Dela Gothic One, Faster One, Gloock, IBM Plex Mono, JetBrains Mono, Kalam, Kaushan Script, Krona One, Lilita One, Limelight, Londrina Solid, Major Mono Display, Marck Script, Michroma, Montserrat, Mr Dafoe, Nanum Brush Script, Norican, Oswald, Outfit, Parisienne, Passion One, Playball, Poiret One, Quantico, Rajdhani, Rubik Glitch, Russo One, Sacramento, Sora, Space Grotesk, Space Mono, Staatliches, Stalinist One, Teko, Titan One, Unbounded | SIL Open Font License 1.1 (each family's own text, `OFL-<family>.txt`) |
| Permanent Marker, Special Elite | Apache License 2.0 (`APACHE-LICENSE.txt`) |

Inter has had its private-use characters removed so the icon font shows (see
`crates/tidycuts-render/fonts/README.md`).

## Sample photographs

The pictures used for library thumbnails and previews (`crates/tidycuts-render/assets/photos/`)
come from [StockSnap.io](https://stocksnap.io), where every picture is released under the
Creative Commons CC0 public-domain dedication: free for any use, no credit required. They are
listed here anyway, with the page each came from. The files are the 960-pixel-wide versions.

| File | Picture | Source page |
| --- | --- | --- |
| `forest.jpg` | Low sun through trees beside a lake | <https://stocksnap.io/photo/sun-trees-I5UDEKJLAC> |
| `lake.jpg` | Orange sunset over a lake | <https://stocksnap.io/photo/sunset-lake-8UOSAA3FRO> |
| `bird.jpg` | A duck on blue water | <https://stocksnap.io/photo/bird-wildlife-76KYFFGJIG> |
| `mountain.jpg` | A green mountain valley with flowers | <https://stocksnap.io/photo/nature-landscape-T3OJ9T8POI> |
| `cat.jpg` | A black cat on a sofa | <https://stocksnap.io/photo/cat-blackcat-PONWAT2C3P> |
| `cat_nap.jpg` | A cat asleep on a bed | <https://stocksnap.io/photo/sleeping-cat-HCLLPLCTG9> |
| `city_night.jpg` | A city skyline at night | <https://stocksnap.io/photo/night-dark-H01L2BILXC> |
| `city.jpg` | A bridge and skyline by day | <https://stocksnap.io/photo/city-bridge-NKZOS5SNK6> |
| `food.jpg` | A latte being poured | <https://stocksnap.io/photo/barista-latte-DTKOOFPC06> |

The portrait cut-out (`assets/portrait.png`) is a photograph of the app's author, used with
their permission.
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
