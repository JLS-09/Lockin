#!/usr/bin/env bash
# Regenerates assets/logo/ (SVG + PNG) and the tray PNGs used by the app.
# Needs: python3 with fonttools, brotli and uharfbuzz; rsvg-convert (librsvg);
# the Space Grotesk Bold font file, e.g. from `npm pack @fontsource/space-grotesk`.
#
# Usage: tools/export-logo.sh <space-grotesk-latin-700-normal.woff>
set -euo pipefail
cd "$(dirname "$0")/.."

font="$1"
out=assets/logo
tmp=$(mktemp -d)

python3 tools/gen_logo.py "$font" "$tmp"
rm -rf "$out/svg" "$out/png"
cp -r "$tmp/svg" "$out/svg"
cp "$tmp/app/lockin-mark.svg" app/src/lib/assets/lockin-mark.svg

mkdir -p "$out/png/tray"
for v in dark light; do
  rsvg-convert -w 1024 -h 1024 "$out/svg/lockin-icon-$v.svg" -o "$out/png/lockin-icon-$v-1024.png"
done
for v in on-dark on-light; do
  rsvg-convert -w 1024 -h 1024 "$out/svg/lockin-mark-$v.svg" -o "$out/png/lockin-mark-$v-1024.png"
  rsvg-convert -h 256 "$out/svg/lockin-logo-$v.svg" -o "$out/png/lockin-logo-$v.png"
  rsvg-convert -h 256 "$out/svg/lockin-wordmark-padlock-$v.svg" -o "$out/png/lockin-wordmark-padlock-$v.png"
  for px in 48 32 22; do
    svg="$out/svg/tray/lockin-tray-$px-$v.svg"
    rsvg-convert -w $px -h $px "$svg" -o "$out/png/tray/lockin-tray-$px-$v.png"
    rsvg-convert -w $((px * 2)) -h $((px * 2)) "$svg" -o "$out/png/tray/lockin-tray-$px-$v@2x.png"
  done
done
cp "$out"/png/tray/*.png app/src-tauri/icons/tray/

cp "$out/svg/lockin-icon-dark.svg" app/static/favicon.svg
rsvg-convert -w 64 -h 64 app/static/favicon.svg -o app/static/favicon.png

rm -rf "$tmp"
echo "Done. Regenerate the Tauri icons with: cd app && pnpm tauri icon ../$out/png/lockin-icon-dark-1024.png"
