#!/usr/bin/env bash
# Setzt `postinst` und `postrm` des Pakets zusammen (HUM-284).
#
# Aufruf: packaging/deb/maintscripts.sh <out-dir>
#
# Die Zeile `#HUMANITL_USERS#` in `postinst.in` und `postrm.in` wird durch
# `maintscript-users.sh` ersetzt, so wie debhelper `#DEBHELPER#` ersetzt;
# danach steht in keinem Skript mehr eine Marke. `build-deb.sh` ruft das fuer
# `DEBIAN/`, `maintscripts-test.sh` fuer seine Probe: Beide pruefen damit
# dieselben Skripte.
set -euo pipefail

[[ $# -eq 1 ]] || { echo "usage: $0 <out-dir>" >&2; exit 2; }
out="$1"
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$out"

fragment="$here/maintscript-users.sh"
for name in postinst postrm; do
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
  chmod 0755 "$out/$name"
done
