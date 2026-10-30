#!/usr/bin/env bash
# Setzt die Scriptlets `post` und `postun` des RPM-Pakets zusammen (HUM-339).
#
# Aufruf: packaging/rpm/scriptlets.sh <out-dir>
#
# Wie `packaging/deb/maintscripts.sh` fuer das .deb: Die Zeile
# `#HUMANITL_USERS#` in `post.in` und `postun.in` wird durch
# `packaging/deb/maintscript-users.sh` ersetzt. Beide Pakete tragen damit
# denselben Teil, der die Nutzerinstanzen findet und neu laedt, Wort fuer Wort.
# `build-rpm.sh` ruft das fuer das Paket, `scriptlets-test.sh` fuer seine
# Probe.
#
# rpm setzt in Scriptlets, die im Spec stehen, Makros ein; der Teil enthaelt
# `printf '%s %s\n'`. Deshalb stehen die Skripte nicht im Spec, sondern werden
# mit `%post -f` gelesen, und `build-rpm.sh` vergleicht danach, was im Paket
# steht, Byte fuer Byte mit diesen Dateien.
set -euo pipefail

[[ $# -eq 1 ]] || { echo "usage: $0 <out-dir>" >&2; exit 2; }
out="$1"
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$out"

fragment="$here/../deb/maintscript-users.sh"
[[ -f "$fragment" ]] || { echo "error: $fragment is missing" >&2; exit 1; }
for name in post postun; do
  template="$here/$name.in"
  [[ "$(grep -c '^#HUMANITL_USERS#$' "$template")" -eq 1 ]] \
    || { echo "error: $template must carry the line #HUMANITL_USERS# exactly once" >&2; exit 1; }
  while IFS= read -r line || [[ -n "$line" ]]; do
    if [[ "$line" == '#HUMANITL_USERS#' ]]; then
      cat "$fragment"
    else
      printf '%s\n' "$line"
    fi
  done <"$template" >"$out/$name"
  if grep -q '^#HUMANITL_USERS#$' "$out/$name"; then
    echo "error: a marker is left in $out/$name" >&2
    exit 1
  fi
  chmod 0644 "$out/$name"
done
