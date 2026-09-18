#!/usr/bin/env bash
# Baut `humanitl-<version>-linux-x86_64.tar.gz`, das Archiv neben dem .deb.
#
# Aufruf: packaging/release/build-tarball.sh <0.0.N> <bin-dir> <flutter-bundle> <out-dir>
#
# Das Archiv hat ein Wurzelverzeichnis `humanitl-<version>-linux-x86_64/`.
# Neben bin/ und app/ liegen dort share/humanitl/catalog und
# profiles/sandbox, weil der Daemon beides relativ zu seinem eigenen Pfad
# sucht (`catalog_dir` in humanitld, `tree_dirs` im Sandbox-Dienst); ohne sie
# liefe er mit leerem Katalog und ohne Sandbox-Profil.
#
# Besitzer, Zeitstempel und Reihenfolge sind festgelegt, damit derselbe Stand
# dasselbe Archiv ergibt.
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
source "$here/version.sh"
require_prerelease_version "$version"

name="humanitl-$version-linux-x86_64"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
top="$work/$name"

install -d -m 0755 "$top/bin" "$top/app" "$top/share/humanitl/catalog" "$top/profiles/sandbox"
cp -R --no-preserve=ownership "$bundle/." "$top/app/"
install -m 0755 "$bin/humanitld" "$bin/humanitl" "$bin/humanitl-shim" "$top/bin/"
"$here/tidy-elf.sh" "$top/app" "$top/bin"

install -m 0644 "$root/catalog/domains.yaml" "$root/catalog/ranks-top100k.csv.gz" \
  "$root/catalog/RANKS-LICENSE" "$top/share/humanitl/catalog/"
# Nur "default"; "test" gehoert den Escape-Tests (siehe build-deb.sh).
install -m 0644 "$root/profiles/sandbox/default.toml" "$top/profiles/sandbox/"
install -m 0644 "$root/README.md" "$root/LICENSE" "$top/"
sed "s|@VERSION@|$version|g" "$here/INSTALL.txt" >"$top/INSTALL.txt"
chmod 0644 "$top/INSTALL.txt"

epoch="${SOURCE_DATE_EPOCH:-$(git -C "$root" log -1 --format=%ct 2>/dev/null || date +%s)}"
archive="$out/$name.tar.gz"
# --mode: Rechte unabhaengig von der umask des Bau-Rechners, niemand ausser
# dem Besitzer darf schreiben.
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$epoch" \
  --mode='u+rwX,go+rX,go-w' \
  -C "$work" -cf - "$name" | gzip -9n >"$archive"
echo "built $archive"
