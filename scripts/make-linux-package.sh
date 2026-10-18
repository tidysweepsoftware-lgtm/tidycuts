#!/usr/bin/env bash
# Makes the Linux downloads: a .tar.gz, an AppImage (any distribution) and a .deb (Debian,
# Ubuntu, Mint...).
#   make-linux-package.sh <tidycuts binary> <out folder> <version> [natives folder]
# The natives folder (optional) holds whisper-linux/ and sdcpp-linux/ built by the release
# workflow. Without them the program still works; Auto Captions and the Image Generator
# then say their program is missing.
set -euo pipefail
bin="$1"; out="$2"; version="$3"; natives="${4:-}"
root="$(cd "$(dirname "$0")/.." && pwd)"
get() { grep -m1 "^$1=" "$root/packaging/voice-assets.txt" | cut -d= -f2-; }
sha() { sha256sum "$1" | cut -d' ' -f1; }
check() { [ "$(sha "$1")" = "$2" ] || { echo "Checksum of $1 is wrong" >&2; exit 1; }; }
repo="${GITHUB_REPOSITORY:-}"
if [ -z "$repo" ]; then
  repo=$(git -C "$root" remote get-url origin | sed -E 's#^.*github\.com[:/]##; s#\.git$##')
fi

work=$(mktemp -d)
mkdir -p "$out"
app="$out/TidyCuts"
rm -rf "$app"
mkdir -p "$app/ffmpeg" "$app/models" "$app/onnxruntime" "$app/whisper-cpp" "$app/imagegen"
cp "$bin" "$app/tidycuts"
chmod +x "$app/tidycuts"
cp "$root/README.md" "$root/LICENSE" "$root/THIRD-PARTY-NOTICES.md" "$app/"
cp "$root/packaging/icons/tidycuts-256.png" "$app/tidycuts.png"

# FFmpeg: the LGPL build, as separate programs. That download is replaced every day and is
# sometimes missing for a moment, so try again a few times.
for i in 1 2 3 4 5; do
  if curl -fL -o "$work/ffmpeg.tar.xz" https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-lgpl.tar.xz; then break; fi
  [ "$i" = 5 ] && { echo "FFmpeg download failed" >&2; exit 1; }
  sleep $((20 * i))
done
tar -xJf "$work/ffmpeg.tar.xz" -C "$work"
src=$(find "$work" -maxdepth 1 -type d -name 'ffmpeg-*' | head -1)
cp "$src/bin/ffmpeg" "$src/bin/ffprobe" "$app/ffmpeg/"
cp "$src/LICENSE.txt" "$app/ffmpeg/FFMPEG-LICENSE.txt"

# Studio Voice: the voice model and ONNX Runtime (see packaging/voice-assets.txt).
model_file=$(get MODEL_FILE)
if command -v gh >/dev/null 2>&1; then
  gh release download "$(get MODEL_TAG)" -R "$repo" -p "$model_file" -D "$work" --clobber
else
  curl -fL -o "$work/$model_file" "https://github.com/$repo/releases/download/$(get MODEL_TAG)/$model_file"
fi
check "$work/$model_file" "$(get MODEL_SHA256)"
cp "$work/$model_file" "$app/models/$model_file"
v=$(get ORT_VERSION)
curl -fL -o "$work/ort.tgz" "https://github.com/microsoft/onnxruntime/releases/download/v$v/onnxruntime-linux-x64-$v.tgz"
check "$work/ort.tgz" "$(get ORT_LINUX_SHA256)"
tar -xzf "$work/ort.tgz" -C "$work"
ort="$work/onnxruntime-linux-x64-$v"
cp -L "$ort/lib/libonnxruntime.so" "$app/onnxruntime/libonnxruntime.so"
cp "$ort/LICENSE" "$app/onnxruntime/ONNXRUNTIME-LICENSE.txt"
cp "$ort/ThirdPartyNotices.txt" "$app/onnxruntime/ONNXRUNTIME-THIRD-PARTY-NOTICES.txt"

# The programs built from source: whisper.cpp (Auto Captions) and stable-diffusion.cpp (Image Generator).
if [ -n "$natives" ] && [ -d "$natives/whisper-linux" ]; then
  cp -a "$natives/whisper-linux/." "$app/whisper-cpp/"
fi
if [ -n "$natives" ] && [ -d "$natives/sdcpp-linux" ]; then
  cp -a "$natives/sdcpp-linux/." "$app/imagegen/"
fi
chmod +x "$app/ffmpeg/"* "$app/whisper-cpp/whisper-cli" "$app/imagegen/sd-cli" "$app/imagegen/sd-server" 2>/dev/null || true

cat > "$app/tidycuts.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=TidyCuts
GenericName=Video Editor
Comment=A free, open-source video editor
Exec=tidycuts %F
Icon=tidycuts
Categories=AudioVideo;Video;AudioVideoEditing;
MimeType=video/mp4;video/x-matroska;video/quicktime;video/webm;
Terminal=false
EOF

name="TidyCuts-$version-linux-x86_64"

# 1. A folder in a .tar.gz: unpack it anywhere and run ./tidycuts
tar -czf "$out/$name.tar.gz" -C "$out" TidyCuts
echo "Made $out/$name.tar.gz"

# 2. An AppImage: one file that runs on most distributions.
appdir="$work/AppDir"
mkdir -p "$appdir/usr/bin"
cp -a "$app/." "$appdir/usr/bin/"
cp "$app/tidycuts.desktop" "$appdir/tidycuts.desktop"
cp "$app/tidycuts.png" "$appdir/tidycuts.png"
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/tidycuts" "$@"
EOF
chmod +x "$appdir/AppRun"
curl -fL -o "$work/appimagetool" https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x "$work/appimagetool"
ARCH=x86_64 "$work/appimagetool" --appimage-extract-and-run "$appdir" "$out/$name.AppImage"
chmod +x "$out/$name.AppImage"
echo "Made $out/$name.AppImage"

# 3. A .deb for Debian, Ubuntu, Linux Mint and the like.
deb="$work/deb"
mkdir -p "$deb/DEBIAN" "$deb/opt/tidycuts" "$deb/usr/bin" "$deb/usr/share/applications" "$deb/usr/share/icons/hicolor/256x256/apps"
cp -a "$app/." "$deb/opt/tidycuts/"
rm -f "$deb/opt/tidycuts/tidycuts.desktop" "$deb/opt/tidycuts/tidycuts.png"
ln -s /opt/tidycuts/tidycuts "$deb/usr/bin/tidycuts"
sed 's#^Exec=.*#Exec=/opt/tidycuts/tidycuts %F#' "$app/tidycuts.desktop" > "$deb/usr/share/applications/tidycuts.desktop"
cp "$app/tidycuts.png" "$deb/usr/share/icons/hicolor/256x256/apps/tidycuts.png"
size=$(du -sk "$deb/opt" | cut -f1)
cat > "$deb/DEBIAN/control" <<EOF
Package: tidycuts
Version: $version
Section: video
Priority: optional
Architecture: amd64
Installed-Size: $size
Depends: libc6 (>= 2.35), libasound2 | libasound2t64, libxkbcommon0, libgtk-3-0 | libgtk-3-0t64, libvulkan1 | mesa-vulkan-drivers
Maintainer: TidySweep Software <tidysweepsoftware@gmail.com>
Homepage: https://github.com/tidysweepsoftware-lgtm/tidycuts
Description: Free, open-source video editor
 TidyCuts is a video editor with a magnetic timeline, built-in titles, effects,
 filters, transitions and music, automatic captions, noise removal for voices
 and an AI picture maker for thumbnails that runs on your own computer.
EOF
dpkg-deb --build --root-owner-group "$deb" "$out/tidycuts_${version}_amd64.deb"
echo "Made $out/tidycuts_${version}_amd64.deb"
rm -rf "$work"
