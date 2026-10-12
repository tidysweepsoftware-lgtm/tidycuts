#!/usr/bin/env bash
# Downloads the Studio Voice files and puts them inside the Mac app:
#   fetch-voice-assets.sh <path-to-TidyCuts.app> [owner/name]
# Result: Contents/Resources/models/mossformer2_se_48k.onnx
#         Contents/Frameworks/libonnxruntime.dylib (and its licence files)
# Versions and checksums are in packaging/voice-assets.txt.
set -euo pipefail
app="$1"
repo="${2:-${GITHUB_REPOSITORY:-}}"
root="$(cd "$(dirname "$0")/.." && pwd)"
get() { grep -m1 "^$1=" "$root/packaging/voice-assets.txt" | cut -d= -f2-; }
if [ -z "$repo" ]; then
  repo=$(git -C "$root" remote get-url origin | sed -E 's#^.*github\.com[:/]##; s#\.git$##')
fi
sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
check() { [ "$(sha "$1")" = "$2" ] || { echo "Checksum of $1 is wrong" >&2; exit 1; }; }
work=$(mktemp -d)
mkdir -p "$app/Contents/Resources/models" "$app/Contents/Frameworks"

model_file=$(get MODEL_FILE)
if command -v gh >/dev/null 2>&1; then
  gh release download "$(get MODEL_TAG)" -R "$repo" -p "$model_file" -D "$work" --clobber
else
  curl -L -o "$work/$model_file" "https://github.com/$repo/releases/download/$(get MODEL_TAG)/$model_file"
fi
check "$work/$model_file" "$(get MODEL_SHA256)"
cp "$work/$model_file" "$app/Contents/Resources/models/$model_file"

v=$(get ORT_VERSION)
curl -L -o "$work/ort.tgz" "https://github.com/microsoft/onnxruntime/releases/download/v$v/onnxruntime-osx-arm64-$v.tgz"
check "$work/ort.tgz" "$(get ORT_MAC_SHA256)"
tar -xzf "$work/ort.tgz" -C "$work"
src="$work/onnxruntime-osx-arm64-$v"
cp -L "$src/lib/libonnxruntime.dylib" "$app/Contents/Frameworks/libonnxruntime.dylib"
cp "$src/LICENSE" "$app/Contents/Frameworks/ONNXRUNTIME-LICENSE.txt"
cp "$src/ThirdPartyNotices.txt" "$app/Contents/Frameworks/ONNXRUNTIME-THIRD-PARTY-NOTICES.txt"
# The speech models for Auto Captions (English always; the multilingual one only when asked).
fetch_asr() {
  name="$1"; url="$2"; want="$3"
  curl -L -o "$work/$name.tar.bz2" "$url"
  check "$work/$name.tar.bz2" "$want"
  mkdir -p "$work/$name"
  tar -xjf "$work/$name.tar.bz2" -C "$work/$name"
  inner=$(find "$work/$name" -mindepth 1 -maxdepth 1 -type d | head -1)
  mkdir -p "$app/Contents/Resources/models/$name"
  cp "$inner/model.int8.onnx" "$inner/tokens.txt" "$app/Contents/Resources/models/$name/"
}
fetch_asr asr-en "$(get ASR_EN_URL)" "$(get ASR_EN_SHA256)"
if [ "${TIDYCUTS_WITH_MULTILINGUAL:-0}" = "1" ]; then
  fetch_asr asr-multi "$(get ASR_MULTI_URL)" "$(get ASR_MULTI_SHA256)"
fi
rm -rf "$work"
echo "Studio Voice files are in $app"
