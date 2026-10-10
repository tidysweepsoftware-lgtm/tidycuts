#!/usr/bin/env bash
# Puts TidyCuts.app into a disk image with a shortcut to /Applications.
# Usage: make-macos-dmg.sh <dir-holding-TidyCuts.app> <output.dmg>
set -euo pipefail
dir="$1"
out="$2"
stage=$(mktemp -d)
cp -R "$dir/TidyCuts.app" "$stage/"
ln -s /Applications "$stage/Applications"
cat > "$stage/READ ME FIRST.txt" <<'EOF_README'
TidyCuts for Mac

1. Drag TidyCuts onto the Applications folder.
2. Open TidyCuts from Applications. This version is not notarized by Apple
   (that needs a paid developer account), so the first time macOS may say it
   cannot check the app:
   - Open System Settings > Privacy & Security, scroll down and click
     "Open Anyway" next to TidyCuts, then click "Open".
   - If macOS says the app is "damaged", open the Terminal app, run
         xattr -cr /Applications/TidyCuts.app
     and open TidyCuts again. (That only removes the "downloaded from the
     internet" mark from the app.)
EOF_README
rm -f "$out"
hdiutil create -volname "TidyCuts" -srcfolder "$stage" -ov -format UDZO "$out"
rm -rf "$stage"
# Make sure the image opens and the app inside is intact.
hdiutil verify "$out"
echo "Built $out"
