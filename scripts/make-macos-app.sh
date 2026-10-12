#!/usr/bin/env bash
# Wraps the release binary in a minimal TidyCuts.app bundle.
# Usage: make-macos-app.sh <path-to-binary> <output-dir>
set -euo pipefail
bin="$1"
out="$2"
version=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
app="$out/TidyCuts.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/TidyCuts"
sed "s/__VERSION__/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
# The app icon, from the iconset in packaging/icons (iconutil comes with macOS).
if command -v iconutil >/dev/null 2>&1; then
  iconutil -c icns packaging/icons/tidycuts.iconset -o "$app/Contents/Resources/AppIcon.icns"
fi
# Bundle FFmpeg if the build machine has portable copies in ./ffmpeg-macos.
if [ -x ffmpeg-macos/ffmpeg ] && [ -x ffmpeg-macos/ffprobe ]; then
  cp ffmpeg-macos/ffmpeg ffmpeg-macos/ffprobe "$app/Contents/Resources/"
fi
# Studio Voice: the voice model and ONNX Runtime, when fetch-voice-assets.sh has
# put them in ./voice-assets (it writes into an app; the files are moved in here).
if [ -d voice-assets/Contents ]; then
  cp -R voice-assets/Contents/Resources/models "$app/Contents/Resources/"
  mkdir -p "$app/Contents/Frameworks"
  cp voice-assets/Contents/Frameworks/* "$app/Contents/Frameworks/"
fi
# Sign everything ad hoc ("-"): it needs no developer account, and a sealed,
# signed bundle is what macOS expects. An unsigned downloaded app is often
# reported as "damaged and can't be opened". (Apple's own Developer ID
# signing and notarization would remove the "can't check it" prompt too.)
if command -v codesign >/dev/null 2>&1; then
  for f in "$app/Contents/Resources/ffmpeg" "$app/Contents/Resources/ffprobe" "$app/Contents/Frameworks/libonnxruntime.dylib"; do
    if [ -f "$f" ]; then codesign --force --sign - "$f"; fi
  done
  codesign --force --deep --sign - "$app"
  codesign --verify --deep --strict --verbose=2 "$app"
fi
echo "Built $app"
