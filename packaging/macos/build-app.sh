#!/usr/bin/env bash
# Legt die Binaries, den Katalog und das Sandbox-Profil in die Flutter-App,
# signiert alles ad-hoc und packt Humanitl-<version>-macos.dmg und .zip
# (HUM-316).
#
# Aufruf: packaging/macos/build-app.sh <0.0.N> <bin-dir> <Humanitl.app> <out-dir>
#
# Aufbau im Bündel, gleich dem Archiv unter Linux, damit Daemon und App ihre
# bestehende Suche nutzen (Katalog unter `<exe>/../../share`, Profil in einem
# Vorfahren, Shim neben dem Daemon, Kommandozeile unter `<app>/bin`):
#
#   Humanitl.app/Contents/MacOS/Humanitl          die App
#   Humanitl.app/Contents/MacOS/bin/              humanitld, humanitl, humanitl-shim
#   Humanitl.app/Contents/MacOS/share    -> ../Resources/share
#   Humanitl.app/Contents/MacOS/profiles -> ../Resources/profiles
#   Humanitl.app/Contents/Resources/share/humanitl/catalog/
#   Humanitl.app/Contents/Resources/profiles/sandbox/default.toml
#
# Die Daten liegen unter Resources und nicht unter MacOS: codesign behandelt
# jede Datei unter MacOS als Code und legt ihre Signatur in erweiterte
# Attribute, die `unzip` und viele Kopien verlieren. Die relativen Verweise
# halten die Suche von Daemon und App unverändert.
#
# Gebraucht wird dmgbuild (`pip install dmgbuild==1.6.5`; ein anderer Pfad in
# `DMGBUILD`).
#
# Ohne Apple Developer ID nur ad-hoc signiert (Entscheidung vom 2026-10-29):
# Gatekeeper lässt die App beim ersten Start nicht ohne Weiteres durch, die
# Release-Notes nennen den Schritt dafür.
set -euo pipefail

[[ $# -eq 4 ]] || { echo "usage: $0 <0.0.N> <bin-dir> <Humanitl.app> <out-dir>" >&2; exit 2; }
version="$1"
bin="$(cd "$2" && pwd)"
app_src="$(cd "$3" && pwd)"
mkdir -p "$4"
out="$(cd "$4" && pwd)"
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
source "$root/packaging/release/version.sh"
require_prerelease_version "$version"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
stage="$work/Humanitl"
mkdir -p "$stage"
ditto "$app_src" "$stage/Humanitl.app"
macos="$stage/Humanitl.app/Contents/MacOS"
resources="$stage/Humanitl.app/Contents/Resources"
install -d -m 0755 "$macos/bin" "$resources/share/humanitl/catalog" "$resources/profiles/sandbox"
install -m 0755 "$bin/humanitld" "$bin/humanitl" "$bin/humanitl-shim" "$macos/bin/"
install -m 0644 "$root/catalog/domains.yaml" "$root/catalog/ranks-top100k.csv.gz" \
  "$root/catalog/RANKS-LICENSE" "$resources/share/humanitl/catalog/"
install -m 0644 "$root/profiles/sandbox/default.toml" "$resources/profiles/sandbox/"
ln -s ../Resources/share "$macos/share"
ln -s ../Resources/profiles "$macos/profiles"

# Erst jedes Programm, dann das Bündel: `--deep` allein signiert Code in
# Unterordnern von MacOS nicht verlässlich.
for name in humanitld humanitl humanitl-shim; do
  codesign --force --sign - --timestamp=none "$macos/bin/$name"
done
codesign --force --deep --sign - --timestamp=none "$stage/Humanitl.app"
codesign --verify --deep --strict --verbose=2 "$stage/Humanitl.app"

name="Humanitl-$version-macos"
rm -f "$out/$name.zip" "$out/$name.dmg"
# The zip carries the app with the notes beside it; the disk image only the
# app and the Applications link, on the designed background.
install -m 0644 "$root/README.md" "$root/LICENSE" "$stage/"
sed "s|@VERSION@|$version|g" "$here/INSTALL.txt" >"$stage/INSTALL.txt"
( cd "$work" && ditto -c -k --sequesterRsrc --keepParent "Humanitl" "$out/$name.zip" )

# dmgbuild draws the window (background, icon positions, no toolbars) without
# remote controlling Finder, so a CI runner builds the same image.
"${DMGBUILD:-dmgbuild}" -s "$here/dmg/settings.py" \
  -D app="$stage/Humanitl.app" \
  -D background="$here/dmg/background.tiff" \
  -D icon="$stage/Humanitl.app/Contents/Resources/AppIcon.icns" \
  "Humanitl $version" "$out/$name.dmg"
hdiutil verify -quiet "$out/$name.dmg"
ls -l "$out/$name.zip" "$out/$name.dmg"
