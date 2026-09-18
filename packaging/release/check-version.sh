#!/usr/bin/env bash
# Prueft, dass die gebauten Programme die Version tragen, unter der sie
# veroeffentlicht werden.
#
# Aufruf: packaging/release/check-version.sh <0.0.N> <bin-dir> [<flutter-bundle>]
#
# `humanitl --version` und `humanitld --version` muessen genau
# "<name> <version>" ausgeben. Ist ein Flutter-Bundle angegeben, muss
# `data/flutter_assets/version.json` dieselbe Version tragen; dorthin schreibt
# `flutter build linux --build-name`. Eine Abweichung ist ein Fehler, keine
# Warnung: Ein Paket 0.0.3, dessen Daemon sich als 0.0.0 meldet, waere genau
# die Art Widerspruch, den spaeter niemand mehr aufklaert.
set -euo pipefail

[[ $# -eq 2 || $# -eq 3 ]] || { echo "usage: $0 <0.0.N> <bin-dir> [<flutter-bundle>]" >&2; exit 2; }
version="$1"
bin="$2"
bundle="${3:-}"

# shellcheck source=packaging/release/version.sh
source "$(dirname "$0")/version.sh"
require_prerelease_version "$version"

status=0
for name in humanitl humanitld; do
  got="$("$bin/$name" --version)"
  want="$name $version"
  if [[ "$got" == "$want" ]]; then
    echo "ok: $name --version prints '$got'"
  else
    echo "error: $name --version prints '$got', expected '$want'" >&2
    status=1
  fi
done

if [[ -n "$bundle" ]]; then
  json="$bundle/data/flutter_assets/version.json"
  got="$(python3 -c 'import json, sys; print(json.load(open(sys.argv[1]))["version"])' "$json")"
  if [[ "$got" == "$version" ]]; then
    echo "ok: the app bundle carries version $got"
  else
    echo "error: $json carries version '$got', expected '$version'" >&2
    status=1
  fi
fi
exit "$status"
