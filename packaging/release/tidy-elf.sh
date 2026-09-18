#!/usr/bin/env bash
# Raeumt die ELF-Dateien einer Kopie von Bundle und Programmen auf, bevor sie
# in ein Paket oder Archiv gehen. Arbeitet an Ort und Stelle; nur auf Kopien
# aufrufen, nie auf dem Build-Verzeichnis selbst.
#
# Aufruf: packaging/release/tidy-elf.sh <flutter-bundle-copy> <bin-dir-copy>
#
# 1. RUNPATH: Die Plugin-Bibliotheken des Bundles tragen den absoluten Pfad des
#    Build-Rechners (`app/linux/flutter/ephemeral`), weil Flutters CMake sie mit
#    `install(FILES ...)` kopiert und den Pfad dabei nicht umschreibt. Auf einem
#    anderen Rechner zeigt er ins Leere oder in ein fremdes Verzeichnis
#    (lintian: custom-library-search-path). Behalten wird nur, was mit $ORIGIN
#    beginnt; die Anwendung findet ihre Bibliotheken ueber `$ORIGIN/lib`, und
#    die Plugins werden erst geladen, wenn libflutter schon da ist.
# 2. Symbole: Anwendung, Plugins und die eigenen Programme werden gestrippt.
#    libflutter_linux_gtk.so und libapp.so liefert Flutter im Release-Modus
#    schon ohne Symbole; libapp.so ist ein AOT-Snapshot und bleibt unberuehrt.
set -euo pipefail

[[ $# -eq 2 ]] || { echo "usage: $0 <flutter-bundle-copy> <bin-dir-copy>" >&2; exit 2; }
bundle="$1"
bin="$2"

patchelf="${PATCHELF:-patchelf}"
command -v "$patchelf" >/dev/null || { echo "error: patchelf is required (apt-get install patchelf)" >&2; exit 1; }

for f in "$bundle/humanitl" "$bundle"/lib/*.so; do
  old="$("$patchelf" --print-rpath "$f")"
  [[ -n "$old" ]] || continue
  # shellcheck disable=SC2016 # $ORIGIN ist woertlich gemeint.
  new="$(tr ':' '\n' <<<"$old" | grep -E '^\$ORIGIN(/|$)' | paste -sd: - || true)"
  if [[ "$new" != "$old" ]]; then
    if [[ -n "$new" ]]; then
      "$patchelf" --set-rpath "$new" "$f"
    else
      "$patchelf" --remove-rpath "$f"
    fi
    echo "RUNPATH of $(basename "$f"): '$old' -> '$new'"
  fi
done

targets=("$bundle/humanitl" "$bin/humanitld" "$bin/humanitl" "$bin/humanitl-shim")
for f in "$bundle"/lib/*.so; do
  case "$(basename "$f")" in
    libflutter_linux_gtk.so | libapp.so) ;;
    *) targets+=("$f") ;;
  esac
done
strip --strip-unneeded --remove-section=.comment --remove-section=.note "${targets[@]}"
