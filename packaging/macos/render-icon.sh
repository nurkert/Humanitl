#!/usr/bin/env bash
# Rechnet das App-Symbol für macOS aus `packaging/deb/humanitl.svg` (HUM-315).
#
# macOS legt Symbole auf ein Raster: Die Kachel nimmt 824 von 1024 Pixeln ein,
# zentriert, mit durchsichtigem Rand. Das SVG füllt seine Fläche fast ganz;
# dieses Skript rendert es deshalb auf 824 und setzt es auf 1024, dann jede
# Größe des AppIcon-Sets der App.
#
# Aufruf: packaging/macos/render-icon.sh
# Gebraucht werden rsvg-convert und ImageMagick 7 (`brew install librsvg imagemagick`).
# Die PNGs sind eingecheckt; wer das SVG ändert, fährt dieses Skript.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
svg="$root/packaging/deb/humanitl.svg"
set_dir="$root/app/macos/Runner/Assets.xcassets/AppIcon.appiconset"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

rsvg-convert -w 824 -h 824 "$svg" -o "$work/tile.png"
magick -size 1024x1024 xc:none "$work/tile.png" -gravity center -composite "$work/1024.png"
for size in 16 32 64 128 256 512 1024; do
  magick "$work/1024.png" -resize "${size}x${size}" -strip "$set_dir/app_icon_${size}.png"
done
ls -l "$set_dir"/app_icon_*.png
