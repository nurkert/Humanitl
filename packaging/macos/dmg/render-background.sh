#!/usr/bin/env bash
# Rechnet den Hintergrund des Disk-Images aus background.svg (HUM-316): eine
# TIFF-Datei mit 1x und 2x, damit Finder auf Retina-Displays scharf zeichnet,
# dazu preview.png (2x) zum Ansehen.
#
# Die Schrift ist Inter, die der App (packages/ui, typography.dart). Das
# Skript holt Inter 4.1 (SIL OFL 1.1) mit fester Prüfsumme in ein temporäres
# Verzeichnis und rendert über eine eigene fontconfig, damit das Ergebnis
# nicht von den Schriften des Rechners abhängt.
#
# Gebraucht werden rsvg-convert (`brew install librsvg`), curl, unzip und
# tiffutil (macOS). Das TIFF ist eingecheckt; wer das SVG ändert, fährt dieses
# Skript und checkt TIFF und Vorschau mit ein.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

inter_url="https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip"
inter_sha="9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e"
curl -fsSL -o "$work/inter.zip" "$inter_url"
echo "$inter_sha  $work/inter.zip" | shasum -a 256 -c - >/dev/null
mkdir -p "$work/fonts"
unzip -q -j "$work/inter.zip" 'extras/ttf/Inter-*.ttf' -d "$work/fonts"
cat >"$work/fonts.conf" <<CONF
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "fonts.dtd">
<fontconfig><dir>$work/fonts</dir><cachedir>$work/cache</cachedir></fontconfig>
CONF
export FONTCONFIG_FILE="$work/fonts.conf"

rsvg-convert -w 660 -h 420 "$here/background.svg" -o "$work/background.png"
rsvg-convert -w 1320 -h 840 "$here/background.svg" -o "$work/background@2x.png"
tiffutil -cathidpicheck "$work/background.png" "$work/background@2x.png" -out "$here/background.tiff"
cp "$work/background@2x.png" "$here/preview.png"
ls -l "$here/background.tiff" "$here/preview.png"
