#!/usr/bin/env bash
# Rechnet die PNG-Symbole des Pakets aus `packaging/deb/humanitl.svg` neu
# (HUM-259). `build-deb.sh` erwartet sie als `humanitl-64.png`,
# `humanitl-128.png` und `humanitl-256.png` neben diesem Skript und legt sie
# nach `/usr/share/icons/hicolor/<N>x<N>/apps/humanitl.png`.
#
# Aufruf: packaging/deb/icons/render.sh
#
# Die PNGs sind eingecheckt, damit der Paketbau kein Grafikprogramm braucht.
# Wer das SVG aendert, faehrt dieses Skript und checkt die drei Bilder im
# selben Commit mit ein. Gebraucht wird Inkscape 1.x.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
svg="$here/../humanitl.svg"
command -v inkscape >/dev/null || { echo "error: inkscape is not installed" >&2; exit 1; }

for size in 64 128 256; do
  inkscape "$svg" -w "$size" -h "$size" -o "$here/humanitl-$size.png"
done
file "$here"/humanitl-*.png || true
