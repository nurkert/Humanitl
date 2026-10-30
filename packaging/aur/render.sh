#!/usr/bin/env bash
# Erzeugt die Dateien des AUR-Pakets humanitl-bin fuer ein Release (HUM-339):
# PKGBUILD, .SRCINFO, humanitl-bin.install und die lokalen Quellen, die das
# Release-Archiv nicht enthaelt (Nutzer-Units, Desktop-Datei, Symbole,
# copyright). Das Ausgabeverzeichnis ist danach genau der Inhalt des
# AUR-Git-Repositorys; hochladen tut der Maintainer von Hand (README.md).
#
# Aufruf: packaging/aur/render.sh <0.0.N> <archiv-oder-sha256> [<out-dir>]
#
#   <archiv-oder-sha256>  entweder die Datei humanitl-<version>-linux-x86_64.tar.gz,
#                         deren SHA-256 hier gerechnet wird, oder die 64
#                         Hex-Zeichen der Pruefsumme aus SHA256SUMS des Releases.
#   <out-dir>             optional; ohne Angabe ./aur-humanitl-bin-<version>
#                         im aktuellen Verzeichnis.
#
# Laeuft auf Linux und macOS und braucht kein makepkg: .SRCINFO entsteht aus
# dem fertigen PKGBUILD, das hier mit `source` gelesen wird, in der Form von
# `makepkg --printsrcinfo`. check-aur.sh vergleicht beides im Arch-Container.
#
# Die lokalen Quellen kommen aus diesem Arbeitsbaum. Fuer ein Release muss er
# also auf dem Tag v<version> stehen; sonst passt etwa die Unit nicht zu den
# Programmen im Archiv. Das Skript warnt, wenn HEAD nicht der Tag ist.
set -euo pipefail

[[ $# -eq 2 || $# -eq 3 ]] \
  || { echo "usage: $0 <0.0.N> <tarball-or-sha256> [<out-dir>]" >&2; exit 2; }
version="$1"
archive="$2"
out="${3:-./aur-humanitl-bin-$version}"

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
# shellcheck source=packaging/release/version.sh
source "$root/packaging/release/version.sh"
require_prerelease_version "$version"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

tarball_name="humanitl-$version-linux-x86_64.tar.gz"
if [[ -f "$archive" ]]; then
  [[ "$(basename "$archive")" == "$tarball_name" ]] \
    || { echo "error: $archive is not named $tarball_name" >&2; exit 1; }
  tarball_sha="$(sha256_of "$archive")"
elif [[ "$archive" =~ ^[0123456789abcdefABCDEF]{64}$ ]]; then
  tarball_sha="$(printf '%s' "$archive" | tr 'ABCDEF' 'abcdef')"
else
  echo "error: '$archive' is neither a file nor a SHA-256 of 64 hex digits" >&2
  exit 1
fi

if [[ "$(git -C "$root" rev-parse -q --verify "refs/tags/v$version^{commit}" 2>/dev/null || true)" \
  != "$(git -C "$root" rev-parse -q --verify HEAD 2>/dev/null || echo none)" ]]; then
  echo "warning: HEAD is not the tag v$version; the local sources come from this working tree" >&2
fi

mkdir -p "$out"
out="$(cd "$out" && pwd)"

# Die Nutzer-Units, wie build-deb.sh sie ins .deb legt: dieselben Vorlagen,
# die `humanitl daemon install` kennt, mit dem Pfad des Pakets in ExecStart
# und ohne die Marke von `daemon install` in der ersten Zeile. Die Units
# gehoeren dem Paket; `daemon install` aktiviert sie nur.
first_line='# Installed by the humanitl package. To change it, run systemctl --user edit NAME; do not copy it.'
marker='# humanitl daemon install: written by Humanitl'
for name in humanitld.service humanitld.socket; do
  src="$root/packaging/systemd/$name"
  [[ "$(head -n 1 "$src")" == "$marker" ]] \
    || { echo "error: $src must start with the marker line of humanitl daemon install" >&2; exit 1; }
  sed -e "1s|.*|${first_line/NAME/$name}|" \
    -e 's|{humanitld}|/usr/lib/humanitl/bin/humanitld|' \
    "$src" >"$out/$name"
done
[[ "$(grep -c '^ExecStart=/usr/lib/humanitl/bin/humanitld$' "$out/humanitld.service")" -eq 1 ]] \
  || { echo "error: the rendered unit must start /usr/lib/humanitl/bin/humanitld exactly once" >&2; exit 1; }
if grep -q '{humanitld}' "$out/humanitld.service" "$out/humanitld.socket"; then
  echo "error: a placeholder is left in the rendered units" >&2
  exit 1
fi

cp "$root/packaging/deb/dev.humanitl.humanitl.desktop" "$out/dev.humanitl.humanitl.desktop"
cp "$root/packaging/deb/humanitl.svg" "$out/humanitl.svg"
for size in 64 128 256; do
  cp "$root/packaging/deb/icons/humanitl-$size.png" "$out/humanitl-$size.png"
done
# copyright im DEP-5-Format des .deb: welche Datei unter welcher Lizenz steht.
# Die Pfade darin gelten fuer dieses Paket genauso.
# Nur der Satz zum Debian-Pfad des GPL-Textes wird angepasst: Auf Arch liegt
# der volle Text als LICENSE neben dieser Datei (PKGBUILD, package()).
debian_gpl=' On Debian systems, the complete text of the GNU General Public License'
[[ "$(grep -cxF "$debian_gpl" "$root/packaging/deb/copyright")" -eq 1 ]] \
  || { echo "error: packaging/deb/copyright must name the Debian path of the GPL text exactly once" >&2; exit 1; }
sed -e 's|^ On Debian systems, the complete text of the GNU General Public License$| The complete text of the GNU General Public License|' \
  -e 's|^ version 3 can be found in /usr/share/common-licenses/GPL-3\.$| version 3 is in /usr/share/licenses/humanitl-bin/LICENSE.|' \
  "$root/packaging/deb/copyright" >"$out/copyright"
if grep -q 'common-licenses' "$out/copyright"; then
  echo "error: the rendered copyright still names /usr/share/common-licenses" >&2
  exit 1
fi

# Das Install-Skript: Vorlage plus derselbe Teil, den maintscripts.sh in
# postinst und postrm des .deb einsetzt.
template="$here/humanitl-bin.install.in"
fragment="$root/packaging/deb/maintscript-users.sh"
[[ "$(grep -c '^#HUMANITL_USERS#$' "$template")" -eq 1 ]] \
  || { echo "error: $template must carry the line #HUMANITL_USERS# exactly once" >&2; exit 1; }
while IFS= read -r line || [[ -n "$line" ]]; do
  if [[ "$line" == '#HUMANITL_USERS#' ]]; then
    cat "$fragment"
  else
    printf '%s\n' "$line"
  fi
done <"$template" >"$out/humanitl-bin.install"

# Die Mindestversion von bubblewrap, wie build-deb.sh sie liest.
bwrap_src="$root/daemon/crates/sandbox/src/bwrap.rs"
bwrap_min="$(sed -n 's/^pub const MIN_BWRAP_VERSION: Version = Version(\([0-9]*\), *\([0-9]*\), *\([0-9]*\));.*/\1.\2.\3/p' "$bwrap_src")"
[[ "$bwrap_min" =~ ^[0123456789]+\.[0123456789]+\.[0123456789]+$ ]] \
  || { echo "error: no MIN_BWRAP_VERSION found in $bwrap_src" >&2; exit 1; }

pkgbuild="$(cat "$here/PKGBUILD.in")"
pkgbuild="${pkgbuild//@VERSION@/$version}"
pkgbuild="${pkgbuild//@BWRAP_MIN@/$bwrap_min}"
pkgbuild="${pkgbuild//@SHA256:tarball@/$tarball_sha}"
for name in humanitld.service humanitld.socket dev.humanitl.humanitl.desktop humanitl.svg \
  humanitl-64.png humanitl-128.png humanitl-256.png copyright; do
  pkgbuild="${pkgbuild//"@SHA256:$name@"/$(sha256_of "$out/$name")}"
done
if grep -Eq '@[A-Z0-9_]+(:[^@]*)?@' <<<"$pkgbuild"; then
  echo "error: a placeholder is left in the PKGBUILD:" >&2
  grep -E '@[A-Z0-9_]+(:[^@]*)?@' <<<"$pkgbuild" >&2
  exit 1
fi
printf '%s\n' "$pkgbuild" >"$out/PKGBUILD"

# .SRCINFO in der Form von `makepkg --printsrcinfo` (pacman 7,
# scripts/libmakepkg/srcinfo.sh): zuerst die Felder von pkgbase in fester
# Reihenfolge, je Wert eine Zeile mit Tabulator, dann der Abschnitt pkgname.
# Das PKGBUILD hat nur einen Paketnamen und keine Felder je Architektur.
# shellcheck disable=SC2154 # Die Felder setzt erst das gelesene PKGBUILD.
(
  set +u
  # shellcheck disable=SC1091
  source "$out/PKGBUILD"
  attr() {
    local name="$1"
    shift
    local value
    for value in "$@"; do
      [[ -n "$value" ]] && printf '\t%s = %s\n' "$name" "$value"
    done
    return 0
  }
  printf 'pkgbase = %s\n' "$pkgname"
  attr pkgdesc "$pkgdesc"
  attr pkgver "$pkgver"
  attr pkgrel "$pkgrel"
  attr epoch "$epoch"
  attr url "$url"
  attr install "$install"
  attr changelog "$changelog"
  attr arch "${arch[@]}"
  attr groups "${groups[@]}"
  attr license "${license[@]}"
  attr checkdepends "${checkdepends[@]}"
  attr makedepends "${makedepends[@]}"
  attr depends "${depends[@]}"
  attr optdepends "${optdepends[@]}"
  attr provides "${provides[@]}"
  attr conflicts "${conflicts[@]}"
  attr replaces "${replaces[@]}"
  attr noextract "${noextract[@]}"
  attr options "${options[@]}"
  attr backup "${backup[@]}"
  attr source "${source[@]}"
  attr validpgpkeys "${validpgpkeys[@]}"
  attr sha256sums "${sha256sums[@]}"
  printf '\npkgname = %s\n' "$pkgname"
) >"$out/.SRCINFO"

echo "rendered humanitl-bin $version (tarball sha256 $tarball_sha, bubblewrap >= $bwrap_min) into $out"
