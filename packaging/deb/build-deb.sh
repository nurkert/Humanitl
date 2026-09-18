#!/usr/bin/env bash
# Baut `humanitl_<version>_amd64.deb` aus fertigen Programmen und dem
# Flutter-Bundle, mit Debians eigenem Werkzeug (`dpkg-shlibdeps`,
# `dpkg-deb`) und ohne Maintainer-Skripte.
#
# Aufruf: packaging/deb/build-deb.sh <0.0.N> <bin-dir> <flutter-bundle> <out-dir>
#
# Das ist die deb-Haelfte von HUM-053 (`backlog/sprint-4.md`), soweit sie heute
# gebaut werden kann. Die Pfade folgen der Spezifikation dort:
#
#   /usr/lib/humanitl/                 Flutter-Bundle (Programm `humanitl`, lib/, data/)
#   /usr/lib/humanitl/bin/             humanitld, humanitl, humanitl-shim
#   /usr/bin/humanitl                  Symlink auf die Kommandozeile
#   /usr/bin/humanitl-app              Symlink auf die Anwendung
#   /usr/lib/systemd/user/humanitld.service
#   /usr/share/applications/humanitl.desktop
#   /usr/share/icons/hicolor/scalable/apps/humanitl.svg
#   /usr/share/humanitl/catalog/       Domain-Katalog (PACKAGED_CATALOG in humanitld)
#   /usr/share/humanitl/profiles/sandbox/  Sandbox-Profile (PROFILE_DIRS)
#   /usr/share/doc/humanitl/           copyright, changelog.gz
#
# Bewusst nicht dabei, jeweils mit Grund:
# - keine `humanitld.socket`: Der Daemon kennt `LISTEN_FDS` noch nicht, siehe
#   den Kommentar am Ende von `packaging/systemd/humanitld.service`.
# - kein `postinst`/`prerm`: Es gibt nichts, was als root beim Installieren
#   geschehen muesste. Die Trigger von `desktop-file-utils` und
#   `hicolor-icon-theme` aktualisieren Menue und Symbol-Cache selbst, und einen
#   Nutzerdienst aktiviert nie das Paket, sondern der Mensch
#   (`systemctl --user enable --now humanitld.service`), HUM-053 Fallstricke.
# - keine PNG-Symbole: Das SVG unter `scalable` genuegt jedem heutigen Desktop.
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

for f in humanitld humanitl humanitl-shim; do
  [[ -x "$bin/$f" ]] || { echo "error: $bin/$f is missing or not executable" >&2; exit 1; }
done
[[ -x "$bundle/humanitl" && -d "$bundle/lib" && -d "$bundle/data" ]] \
  || { echo "error: $bundle is not a Flutter Linux bundle" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
# dpkg-shlibdeps erkennt Bibliotheken unter debian/<paket>/ als eigene und
# verlangt fuer sie keine Abhaengigkeit; deshalb diese Form des Baums.
pkg="$work/debian/humanitl"
lib="$pkg/usr/lib/humanitl"

install -d -m 0755 "$lib" "$lib/bin" "$pkg/usr/bin" "$pkg/usr/lib/systemd/user" \
  "$pkg/usr/share/applications" "$pkg/usr/share/icons/hicolor/scalable/apps" \
  "$pkg/usr/share/humanitl/catalog" "$pkg/usr/share/humanitl/profiles/sandbox" \
  "$pkg/usr/share/doc/humanitl" "$pkg/DEBIAN"

# Das Flutter-Bundle, wie `flutter build linux` es hinterlaesst.
cp -R --no-preserve=ownership "$bundle/." "$lib/"
install -m 0755 "$bin/humanitld" "$bin/humanitl" "$bin/humanitl-shim" "$lib/bin/"
ln -s ../lib/humanitl/bin/humanitl "$pkg/usr/bin/humanitl"
ln -s ../lib/humanitl/humanitl "$pkg/usr/bin/humanitl-app"

# RUNPATH des Build-Rechners entfernen und Symbole strippen, an der Kopie.
"$root/packaging/release/tidy-elf.sh" "$lib" "$lib/bin"

# Die Nutzer-Unit ist dieselbe Vorlage, die `humanitl daemon install` schreibt
# (HUM-044); hier steht in `ExecStart` der Pfad des Pakets. Die erste Zeile,
# die Marke fuer `daemon install`, wird ersetzt: Diese Datei gehoert dem Paket.
unit_src="$root/packaging/systemd/humanitld.service"
[[ "$(grep -c '{humanitld}' "$unit_src")" -eq 1 ]] \
  || { echo "error: $unit_src must contain the placeholder {humanitld} exactly once" >&2; exit 1; }
sed -e '1s|.*|# Installed by the humanitl package. To change it, copy it to ~/.config/systemd/user/ and edit the copy.|' \
  -e 's|{humanitld}|/usr/lib/humanitl/bin/humanitld|' \
  "$unit_src" >"$pkg/usr/lib/systemd/user/humanitld.service"
chmod 0644 "$pkg/usr/lib/systemd/user/humanitld.service"

install -m 0644 "$here/humanitl.desktop" "$pkg/usr/share/applications/humanitl.desktop"
install -m 0644 "$here/humanitl.svg" "$pkg/usr/share/icons/hicolor/scalable/apps/humanitl.svg"
install -m 0644 "$root/catalog/domains.yaml" "$root/catalog/ranks-top100k.csv.gz" \
  "$root/catalog/RANKS-LICENSE" "$pkg/usr/share/humanitl/catalog/"
# Nur das Profil "default". "test" gehoert den Escape-Tests und traegt einen
# Platzhalter-Pfad, den erst deren Runner ersetzt.
install -m 0644 "$root/profiles/sandbox/default.toml" "$pkg/usr/share/humanitl/profiles/sandbox/"
install -m 0644 "$here/copyright" "$pkg/usr/share/doc/humanitl/copyright"

install -d -m 0755 "$pkg/usr/share/lintian/overrides"
install -m 0644 "$here/lintian-overrides" "$pkg/usr/share/lintian/overrides/humanitl"

# Das Changelog: ein Eintrag je Vorabversion. Das Datum ist das des Commits,
# damit zwei Baeue desselben Stands dieselbe Datei ergeben. Der Dateiname ist
# `changelog.gz` und nicht `changelog.Debian.gz`: Die Version traegt keine
# Debian-Revision (0.0.3, nicht 0.0.3-1), das Paket ist also ein natives, und
# fuer native Pakete verlangt die Policy (12.7) diesen Namen
# (lintian: wrong-name-for-changelog-of-native-package).
epoch="${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct 2>/dev/null || date +%s)}"
commit="$(git -C "$root" rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"
{
  echo "humanitl ($version) unstable; urgency=medium"
  echo
  echo "  * Pre-release $version, built from commit $commit. Not for production use."
  echo
  echo " -- Niko Burkert <humanitl@nurkert.de>  $(LC_ALL=C date -u -R -d "@$epoch")"
} | gzip -9n >"$pkg/usr/share/doc/humanitl/changelog.gz"
chmod 0644 "$pkg/usr/share/doc/humanitl/changelog.gz"

# Rechte, wie Debian sie erwartet: Verzeichnisse 0755, Dateien 0644, nur die
# Programme 0755. Die Bibliotheken des Bundles sind keine Programme.
find "$pkg" -type d -exec chmod 0755 {} +
find "$pkg/usr" -type f -exec chmod 0644 {} +
chmod 0755 "$lib/humanitl" "$lib/bin/humanitld" "$lib/bin/humanitl" "$lib/bin/humanitl-shim"

# Depends: abgeleitet, nicht geraten. dpkg-shlibdeps liest die dynamischen
# Abhaengigkeiten aller ELF-Dateien und uebersetzt sie in Pakete des Systems,
# auf dem gebaut wird. Der Shim ist statisch und hat keine.
elfs=()
while IFS= read -r -d '' f; do
  if [[ "$(head -c 4 "$f" | od -An -c | tr -d ' ')" == "177ELF" ]] \
    && ! ldd "$f" 2>&1 | grep -q 'not a dynamic executable'; then
    elfs+=("-e$f")
  fi
done < <(find "$pkg/usr" -type f -print0)
printf 'Source: humanitl\n\nPackage: humanitl\nArchitecture: amd64\n' >"$work/debian/control"
shlibs="$(cd "$work" && dpkg-shlibdeps -O -l"$lib/lib" "${elfs[@]}" 2>"$work/shlibdeps.err")" || {
  cat "$work/shlibdeps.err" >&2
  exit 1
}
if [[ -s "$work/shlibdeps.err" ]]; then
  echo "dpkg-shlibdeps said:" >&2
  cat "$work/shlibdeps.err" >&2
fi
shlibs="${shlibs#shlibs:Depends=}"
[[ -n "$shlibs" ]] || { echo "error: dpkg-shlibdeps derived no dependencies" >&2; exit 1; }
# bubblewrap ist kein gelinktes, sondern ein gestartetes Programm: Der Daemon
# ruft `bwrap` auf, und ohne ihn gibt es keine Sandbox. Die Mindestversion
# kommt aus `MIN_BWRAP_VERSION` in daemon/crates/sandbox/src/bwrap.rs, damit
# Paket und Laufzeitpruefung dieselbe Zahl nennen.
bwrap_src="$root/daemon/crates/sandbox/src/bwrap.rs"
bwrap_min="$(sed -n 's/^pub const MIN_BWRAP_VERSION: Version = Version(\([0-9]*\), *\([0-9]*\), *\([0-9]*\));.*/\1.\2.\3/p' "$bwrap_src")"
[[ "$bwrap_min" =~ ^[0123456789]+\.[0123456789]+\.[0123456789]+$ ]] \
  || { echo "error: no MIN_BWRAP_VERSION found in $bwrap_src" >&2; exit 1; }
# libegl1 und libgles2: Flutters Engine laedt libEGL.so.1 und libGLESv2.so.2
# ueber libepoxy mit dlopen. Das sieht weder ldd noch dpkg-shlibdeps; ohne
# die beiden bricht `humanitl-app` beim Start mit "Couldn't open libEGL.so.1"
# ab. `check-install.sh` startet die Anwendung deshalb einmal unter Xvfb.
depends="$shlibs, bubblewrap (>= $bwrap_min), libegl1, libgles2"

installed_size="$(du -sk --apparent-size --exclude=DEBIAN "$pkg" | cut -f1)"
sed -e "s|@VERSION@|$version|" -e "s|@INSTALLED_SIZE@|$installed_size|" \
  -e "s|@DEPENDS@|$depends|" "$here/control.in" >"$pkg/DEBIAN/control"
(cd "$pkg" && find usr -type f -print0 | sort -z | xargs -0 md5sum) >"$pkg/DEBIAN/md5sums"
chmod 0644 "$pkg/DEBIAN/control" "$pkg/DEBIAN/md5sums"

deb="$out/humanitl_${version}_amd64.deb"
SOURCE_DATE_EPOCH="$epoch" dpkg-deb --root-owner-group -Zxz --build "$pkg" "$deb"
echo "Depends: $depends"
echo "built $deb"
