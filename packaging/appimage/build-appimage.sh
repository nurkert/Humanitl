#!/usr/bin/env bash
# Baut `Humanitl-<version>-x86_64.AppImage` aus fertigen Programmen und dem
# Flutter-Bundle (HUM-053).
#
# Aufruf: packaging/appimage/build-appimage.sh <0.0.N> <bin-dir> <flutter-bundle> <out-dir>
#
# Werkzeug: `appimagetool` und die Laufzeit des AppImage, beide auf eine
# Fassung und eine Pruefsumme festgelegt (unten). Liegen sie nicht unter
# `$APPIMAGE_TOOLS` (Vorgabe `daemon/target/appimage-tools`), laedt das Skript
# sie von GitHub und prueft sie, bevor es sie ausfuehrt. `appimagetool` ist
# selbst ein AppImage; es laeuft mit `--appimage-extract-and-run`, damit der
# Bau kein FUSE braucht.
#
# Der Baum im Bild ist derselbe wie im Archiv, nur unter `usr/lib/humanitl/`:
#
#   AppRun                                 packaging/appimage/AppRun
#   humanitl.desktop, humanitl.svg, .DirIcon
#   usr/lib/humanitl/                      Flutter-Bundle (humanitl, lib/, data/)
#   usr/lib/humanitl/bin/                  humanitld, humanitl, humanitl-shim
#   usr/lib/humanitl/share/humanitl/catalog/   Katalog (`catalog_dir` in humanitld)
#   usr/lib/humanitl/profiles/sandbox/     Sandbox-Profil (`tree_dirs` im Sandbox-Dienst)
#
# Die Anwendung findet die Kommandozeile unter `bin/humanitl` neben sich
# (`installServiceCandidate` in app/lib/core/ui/fix_control.dart), der Daemon
# Katalog und Profil relativ zu seinem eigenen Pfad.
#
# **Keine Bibliothek des Systems im Bild.** Nur das Flutter-Bundle; GTK, GLib
# und Wayland kommen vom System, und `AppRun` setzt kein `LD_LIBRARY_PATH`.
# Das Skript prueft das am Ende: Liegt im Bild eine `libgtk`, `libglib`,
# `libwayland` oder `libEGL`, bricht es ab.
set -euo pipefail

[[ $# -eq 4 ]] || { echo "usage: $0 <0.0.N> <bin-dir> <flutter-bundle> <out-dir>" >&2; exit 2; }
version="$1"
bin="$(cd "$2" && pwd)"
bundle="$(cd "$3" && pwd)"
mkdir -p "$4"
out="$(cd "$4" && pwd)"

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
# shellcheck source=packaging/release/version.sh
source "$root/packaging/release/version.sh"
require_prerelease_version "$version"

# Die Fassungen des Werkzeugs, mit Pruefsumme. Wer sie anhebt, hebt beide
# Zeilen zusammen an.
tool_url="https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage"
tool_sha256="ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0"
runtime_url="https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"
runtime_sha256="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"

tools="${APPIMAGE_TOOLS:-$root/daemon/target/appimage-tools}"
mkdir -p "$tools"

# Holt `url` nach `file`, wenn es dort nicht schon mit `sum` liegt, und prueft
# die Summe in jedem Fall. Eine Datei mit falscher Summe wird nie ausgefuehrt.
fetch() {
  local url="$1" sum="$2" file="$3"
  if [[ ! -f "$file" ]] || ! echo "$sum  $file" | sha256sum -c --quiet - >/dev/null 2>&1; then
    curl -fsSL --retry 3 -o "$file.part" "$url"
    mv "$file.part" "$file"
  fi
  echo "$sum  $file" | sha256sum -c --quiet - \
    || { echo "error: $file does not match its pinned sha256" >&2; rm -f "$file"; exit 1; }
}
fetch "$tool_url" "$tool_sha256" "$tools/appimagetool-x86_64.AppImage"
fetch "$runtime_url" "$runtime_sha256" "$tools/runtime-x86_64"
chmod 0755 "$tools/appimagetool-x86_64.AppImage"

for f in humanitld humanitl humanitl-shim; do
  [[ -x "$bin/$f" ]] || { echo "error: $bin/$f is missing or not executable" >&2; exit 1; }
done
[[ -x "$bundle/humanitl" && -d "$bundle/lib" && -d "$bundle/data" ]] \
  || { echo "error: $bundle is not a Flutter Linux bundle" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
appdir="$work/Humanitl.AppDir"
lib="$appdir/usr/lib/humanitl"
install -d -m 0755 "$lib/bin" "$lib/share/humanitl/catalog" "$lib/profiles/sandbox"

cp -R --no-preserve=ownership "$bundle/." "$lib/"
install -m 0755 "$bin/humanitld" "$bin/humanitl" "$bin/humanitl-shim" "$lib/bin/"
"$root/packaging/release/tidy-elf.sh" "$lib" "$lib/bin"

install -m 0644 "$root/catalog/domains.yaml" "$root/catalog/ranks-top100k.csv.gz" \
  "$root/catalog/RANKS-LICENSE" "$lib/share/humanitl/catalog/"
# Nur "default"; "test" gehoert den Escape-Tests (siehe build-deb.sh).
install -m 0644 "$root/profiles/sandbox/default.toml" "$lib/profiles/sandbox/"

install -m 0755 "$here/AppRun" "$appdir/AppRun"
# Dieselbe Desktop-Datei wie im Paket: Eine zweite Fassung liefe ihr davon.
# `Exec` nennt `humanitl-app`; Werkzeuge, die ein AppImage ins Menue holen,
# ersetzen die Zeile ohnehin durch den Pfad des Bildes.
install -m 0644 "$root/packaging/deb/humanitl.desktop" "$appdir/humanitl.desktop"
install -m 0644 "$root/packaging/deb/humanitl.svg" "$appdir/humanitl.svg"
ln -s humanitl.svg "$appdir/.DirIcon"

# Keine Bibliothek, die vom System kommen muss.
forbidden="$(find "$appdir" -name 'libgtk*' -o -name 'libgdk*' -o -name 'libglib*' \
  -o -name 'libgio*' -o -name 'libwayland*' -o -name 'libEGL*' -o -name 'libGL*' | head -n 5)"
if [[ -n "$forbidden" ]]; then
  echo "error: the AppImage would bundle system libraries:" >&2
  echo "$forbidden" >&2
  exit 1
fi

epoch="${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct 2>/dev/null || date +%s)}"
image="$out/Humanitl-$version-x86_64.AppImage"
rm -f "$image"
SOURCE_DATE_EPOCH="$epoch" ARCH=x86_64 VERSION="$version" \
  "$tools/appimagetool-x86_64.AppImage" --appimage-extract-and-run \
  --no-appstream --runtime-file "$tools/runtime-x86_64" \
  "$appdir" "$image"
chmod 0755 "$image"
echo "built $image"
