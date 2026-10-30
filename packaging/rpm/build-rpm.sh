#!/usr/bin/env bash
# Baut `humanitl-<version>-1.x86_64.rpm` fuer Fedora 40 und neuer und
# openSUSE Tumbleweed (HUM-339), aus denselben Programmen und demselben
# Flutter-Bundle wie das .deb und mit derselben Ablage.
#
# Aufruf: packaging/rpm/build-rpm.sh <0.0.N> <bin-dir> <flutter-bundle> <out-dir>
#
# Gebaut wird auf dem Ubuntu-Laeufer der Release-Pipeline mit `rpmbuild` aus
# dem Ubuntu-Paket `rpm`. Das geht, weil nichts kompiliert wird: Dieses
# Skript legt den Baum an wie `packaging/deb/build-deb.sh`, und das Spec
# (`humanitl.spec.in`) kopiert ihn nur und beschreibt ihn.
#
#   /usr/lib/humanitl/                 Flutter-Bundle (Programm `humanitl`, lib/, data/)
#   /usr/lib/humanitl/bin/             humanitld, humanitl, humanitl-shim
#   /usr/bin/humanitl                  Symlink auf die Kommandozeile
#   /usr/bin/humanitl-app              Symlink auf die Anwendung
#   /usr/lib/systemd/user/humanitld.service
#   /usr/lib/systemd/user/humanitld.socket
#   /usr/share/applications/dev.humanitl.humanitl.desktop
#   /usr/share/icons/hicolor/scalable/apps/humanitl.svg
#   /usr/share/icons/hicolor/{64x64,128x128,256x256}/apps/humanitl.png
#   /usr/share/humanitl/catalog/       Domain-Katalog (PACKAGED_CATALOG in humanitld)
#   /usr/share/humanitl/profiles/sandbox/  Sandbox-Profile (PROFILE_DIRS)
#   /usr/share/licenses/humanitl/      LICENSE, copyright
#
# Die Pfade sind woertlich /usr/lib und nie `%{_libdir}`: Das ist auf rpm
# /usr/lib64, und `humanitl daemon install` erkennt das Paket an
# /usr/lib/systemd/user/humanitld.service und liest `ExecStart` daraus;
# Katalog, Profile und Shim sucht der Daemon unter festen Pfaden.
#
# Unterschiede zum .deb, mit Absicht:
# - Statt /usr/share/doc/humanitl/{copyright,changelog.gz} gibt es
#   `%license` unter /usr/share/licenses/humanitl und den `%changelog` im
#   Paketkopf, wie rpm es vorsieht. lintian-Overrides entfallen; die
#   Ausnahmen fuer rpmlint stehen in `humanitl.rpmlintrc` und reisen nicht im
#   Paket mit.
# - Die Abhaengigkeiten leitet rpm selbst aus den ELF-Dateien ab
#   (`elfdeps`), als Sonamen mit Symbolfassung. Dazu kommen von Hand
#   bubblewrap und die beiden Bibliotheken, die die Engine mit dlopen laedt.
#
# Scriptlets: `%post` und `%postun` tragen denselben Teil wie `postinst` und
# `postrm` des .deb (`packaging/deb/maintscript-users.sh`), zusammengesetzt
# von `scriptlets.sh`. Menue und Symbol-Cache aktualisieren auf Fedora und
# openSUSE die Datei-Trigger von desktop-file-utils, hicolor-icon-theme und
# gtk; dafuer braucht das Paket kein eigenes Scriptlet. Einen Nutzerdienst
# aktiviert nie das Paket, sondern jeder Mensch fuer sich mit
# `humanitl daemon install`.
#
# Reproduzierbar: Alle Zeiten, auch die Bauzeit im Paketkopf, sind die des
# Commits (`SOURCE_DATE_EPOCH`), der Name des Bau-Rechners ist fest. Zwei
# Baeue desselben Stands ergeben dieselbe Datei.
set -euo pipefail

[[ $# -eq 4 ]] || { echo "usage: $0 <0.0.N> <bin-dir> <flutter-bundle> <out-dir>" >&2; exit 2; }
version="$1"
bin="$(cd "$2" && pwd)"
bundle="$(cd "$3" && pwd)"
mkdir -p "$4"
out="$(cd "$4" && pwd)"

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
deb="$root/packaging/deb"
# shellcheck source=packaging/release/version.sh
source "$root/packaging/release/version.sh"
require_prerelease_version "$version"

command -v rpmbuild >/dev/null || { echo "error: rpmbuild is required (apt-get install rpm)" >&2; exit 1; }
for f in humanitld humanitl humanitl-shim; do
  [[ -x "$bin/$f" ]] || { echo "error: $bin/$f is missing or not executable" >&2; exit 1; }
done
[[ -x "$bundle/humanitl" && -d "$bundle/lib" && -d "$bundle/data" ]] \
  || { echo "error: $bundle is not a Flutter Linux bundle" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
tree="$work/tree"
lib="$tree/usr/lib/humanitl"

install -d -m 0755 "$lib" "$lib/bin" "$tree/usr/bin" "$tree/usr/lib/systemd/user" \
  "$tree/usr/share/applications" "$tree/usr/share/icons/hicolor/scalable/apps" \
  "$tree/usr/share/humanitl/catalog" "$tree/usr/share/humanitl/profiles/sandbox" \
  "$tree/usr/share/licenses/humanitl"

# Das Flutter-Bundle, wie `flutter build linux` es hinterlaesst.
cp -R --no-preserve=ownership "$bundle/." "$lib/"
install -m 0755 "$bin/humanitld" "$bin/humanitl" "$bin/humanitl-shim" "$lib/bin/"
ln -s ../lib/humanitl/bin/humanitl "$tree/usr/bin/humanitl"
ln -s ../lib/humanitl/humanitl "$tree/usr/bin/humanitl-app"

# RUNPATH des Build-Rechners entfernen und Symbole strippen, an der Kopie.
"$root/packaging/release/tidy-elf.sh" "$lib" "$lib/bin"

# Die Nutzer-Units, genau wie im .deb (`build-deb.sh`): dieselben Vorlagen,
# die `humanitl daemon install` kennt, mit dem Pfad des Pakets in `ExecStart`
# und ohne die Marke in der ersten Zeile.
first_line='# Installed by the humanitl package. To change it, run systemctl --user edit NAME; do not copy it.'
marker='# humanitl daemon install: written by Humanitl'
for name in humanitld.service humanitld.socket; do
  src="$root/packaging/systemd/$name"
  [[ "$(head -n 1 "$src")" == "$marker" ]] \
    || { echo "error: $src must start with the marker line of humanitl daemon install" >&2; exit 1; }
  sed -e "1s|.*|${first_line/NAME/$name}|" \
    -e 's|{humanitld}|/usr/lib/humanitl/bin/humanitld|' \
    "$src" >"$tree/usr/lib/systemd/user/$name"
done
unit="$tree/usr/lib/systemd/user/humanitld.service"
[[ "$(grep -c '^ExecStart=/usr/lib/humanitl/bin/humanitld$' "$unit")" -eq 1 ]] \
  || { echo "error: $unit must start /usr/lib/humanitl/bin/humanitld exactly once" >&2; exit 1; }
if grep -q '{humanitld}' "$tree/usr/lib/systemd/user/"*; then
  echo "error: a placeholder is left in the units of the package" >&2
  exit 1
fi
if grep -qxF "$marker" "$tree/usr/lib/systemd/user/"*; then
  echo "error: the marker of daemon install is left in the units of the package" >&2
  exit 1
fi

# Desktop-Datei, Symbole, Katalog und Profil: dieselben Dateien wie im .deb.
install -m 0644 "$deb/dev.humanitl.humanitl.desktop" \
  "$tree/usr/share/applications/dev.humanitl.humanitl.desktop"
install -m 0644 "$deb/humanitl.svg" "$tree/usr/share/icons/hicolor/scalable/apps/humanitl.svg"
for size in 64 128 256; do
  install -d -m 0755 "$tree/usr/share/icons/hicolor/${size}x${size}/apps"
  install -m 0644 "$deb/icons/humanitl-$size.png" "$tree/usr/share/icons/hicolor/${size}x${size}/apps/humanitl.png"
done
install -m 0644 "$root/catalog/domains.yaml" "$root/catalog/ranks-top100k.csv.gz" \
  "$root/catalog/RANKS-LICENSE" "$tree/usr/share/humanitl/catalog/"
# Nur das Profil "default". "test" gehoert den Escape-Tests.
install -m 0644 "$root/profiles/sandbox/default.toml" "$tree/usr/share/humanitl/profiles/sandbox/"
# Der volle Text der GPL und die Aufstellung aller Lizenzen im Paket
# (Programm, Symbol, Rangliste). Die Aufstellung ist die des .deb im Format
# DEP-5; ihr Satz ueber /usr/share/common-licenses gilt nur auf Debian und
# zeigt hier auf LICENSE daneben, wie im AUR-Paket (packaging/aur/render.sh).
install -m 0644 "$root/LICENSE" "$tree/usr/share/licenses/humanitl/LICENSE"
sed -e 's|^ On Debian systems, the complete text of the GNU General Public License$| The complete text of the GNU General Public License|' \
  -e 's|^ version 3 can be found in /usr/share/common-licenses/GPL-3\.$| version 3 is in /usr/share/licenses/humanitl/LICENSE.|' \
  "$deb/copyright" >"$tree/usr/share/licenses/humanitl/copyright"
chmod 0644 "$tree/usr/share/licenses/humanitl/copyright"
if grep -q 'common-licenses' "$tree/usr/share/licenses/humanitl/copyright"; then
  echo "error: the copyright of the RPM still names /usr/share/common-licenses" >&2
  exit 1
fi

# Rechte: Verzeichnisse 0755, Dateien 0644, die Programme 0755. Die
# Anwendung prueft die Kette ueber `<appdir>/bin/humanitl`; kein Verzeichnis
# darauf darf fuer Gruppe oder Welt schreibbar sein. Einziger Unterschied zum
# .deb: Die Bibliotheken des Bundles sind hier 0755, weil Fedora und openSUSE
# Shared Objects ausfuehrbar erwarten (rpmlint: shared-library-not-executable);
# Debian erwartet sie mit 0644.
find "$tree" -type d -exec chmod 0755 {} +
find "$tree/usr" -type f -exec chmod 0644 {} +
chmod 0755 "$lib/humanitl" "$lib/bin/humanitld" "$lib/bin/humanitl" "$lib/bin/humanitl-shim" \
  "$lib"/lib/*.so

bwrap_src="$root/daemon/crates/sandbox/src/bwrap.rs"
bwrap_min="$(sed -n 's/^pub const MIN_BWRAP_VERSION: Version = Version(\([0-9]*\), *\([0-9]*\), *\([0-9]*\));.*/\1.\2.\3/p' "$bwrap_src")"
[[ "$bwrap_min" =~ ^[0123456789]+\.[0123456789]+\.[0123456789]+$ ]] \
  || { echo "error: no MIN_BWRAP_VERSION found in $bwrap_src" >&2; exit 1; }

"$here/scriptlets.sh" "$work/scriptlets"

# Das Changelog: ein Eintrag je Vorabversion, mit dem Datum des Commits.
epoch="${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct 2>/dev/null || date +%s)}"
[[ "$epoch" =~ ^[0123456789]+$ ]] || { echo "error: SOURCE_DATE_EPOCH '$epoch' is not a number" >&2; exit 1; }
commit="$(git -C "$root" rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"
changelog_date="$(LC_ALL=C date -u -d "@$epoch" '+%a %b %d %Y')"

# Die Pfade kommen in das Spec; ein Zeichen, das Spec oder sed anders lesen,
# bricht den Bau lieber hier als mittendrin.
for p in "$tree" "$work/scriptlets"; do
  [[ "$p" =~ ^[A-Za-z0-9/._-]+$ ]] || { echo "error: unusable work path $p" >&2; exit 1; }
done
sed -e "s|@VERSION@|$version|g" -e "s|@BWRAP_MIN@|$bwrap_min|" \
  -e "s|@TREE@|$tree|" -e "s|@POST@|$work/scriptlets/post|" \
  -e "s|@POSTUN@|$work/scriptlets/postun|" -e "s|@CHANGELOG_DATE@|$changelog_date|" \
  -e "s|@COMMIT@|$commit|" "$here/humanitl.spec.in" >"$work/humanitl.spec"
if grep -q '@[A-Z_]*@' "$work/humanitl.spec"; then
  echo "error: a placeholder is left in the spec:" >&2
  grep -n '@[A-Z_]*@' "$work/humanitl.spec" >&2
  exit 1
fi

rpm_name="humanitl-${version}-1.x86_64.rpm"
rm -f "$out/$rpm_name"
# - `_build_name_fmt`: das Paket direkt unter <out-dir>, ohne Unterordner
#   je Architektur, unter dem Namen, den die Release-Pipeline erwartet.
# - `_binary_payload`: xz wie das .deb; Fedora, openSUSE und rpm 4.18 lesen es.
# - `clamp_mtime_...` und `use_source_date_epoch_as_buildtime`: Zeiten aus
#   SOURCE_DATE_EPOCH, `_buildhost` fest; sonst traegt jedes Paket die Uhrzeit
#   und den Namen des Laeufers.
SOURCE_DATE_EPOCH="$epoch" rpmbuild -bb --quiet --target x86_64 \
  --define "_topdir $work/rpmbuild" \
  --define "_rpmdir $out" \
  --define "_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm" \
  --define "_binary_payload w9.xzdio" \
  --define "_buildhost reproducible" \
  --define "clamp_mtime_to_source_date_epoch 1" \
  --define "use_source_date_epoch_as_buildtime 1" \
  "$work/humanitl.spec"
rpm_file="$out/$rpm_name"
[[ -f "$rpm_file" ]] || { echo "error: rpmbuild did not write $rpm_file" >&2; exit 1; }

# Nachpruefen, was rpm daraus gemacht hat.
# 1. Die Scriptlets stehen Byte fuer Byte so im Paket, wie `scriptlets.sh`
#    sie geschrieben hat; kein Makro wurde eingesetzt.
for pair in POSTIN:post POSTUN:postun; do
  tag="${pair%%:*}" file="${pair#*:}"
  if ! cmp -s <(rpm -qp --qf "%{$tag}" "$rpm_file") "$work/scriptlets/$file"; then
    echo "error: the $tag scriptlet in the package differs from $file:" >&2
    diff <(rpm -qp --qf "%{$tag}" "$rpm_file") "$work/scriptlets/$file" >&2 || true
    exit 1
  fi
done
# 2. Keine Datei ausserhalb dessen, was das Spec nennt (rpmbuild bricht bei
#    unverpackten Dateien selbst ab), und keine unter /usr/lib64 oder
#    /usr/lib/.build-id.
files="$(rpm -qpl "$rpm_file")"
if grep -E '^/usr/lib64(/|$)|^/usr/lib/\.build-id(/|$)' <<<"$files"; then
  echo "error: the package installs below /usr/lib64 or /usr/lib/.build-id" >&2
  exit 1
fi
# 3. Keine Abhaengigkeit auf eine private Bibliothek des Bundles, und die
#    Untergrenze von glibc steht drin.
requires="$(rpm -qp --requires "$rpm_file")"
if grep -E '^lib(flutter_linux_gtk|app|[a-z0-9_]+_plugin)\.so' <<<"$requires"; then
  echo "error: the package requires a private library of the bundle" >&2
  exit 1
fi
grep -q '^libc\.so\.6(GLIBC_2\.[0-9]*)(64bit)$' <<<"$requires" \
  || { echo "error: no glibc symbol version among the requirements; elfdeps did not run" >&2; exit 1; }
glibc_floor="$(sed -n 's/^libc\.so\.6(GLIBC_\(2\.[0-9]*\))(64bit)$/\1/p' <<<"$requires" | sort -t. -k2,2n | tail -n 1)"

echo "Requires:"
echo "  ${requires//$'\n'/$'\n'  }"
echo "glibc floor: $glibc_floor"
echo "built $rpm_file"
