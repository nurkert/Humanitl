#!/usr/bin/env bash
# Prueft das .deb mit lintian, im Wegwerf-Container wie `check-install.sh`.
#
# Aufruf (als root, im Container): lintian.sh <deb>
#
# Fehler (`E:`) und Warnungen (`W:`) lassen den Lauf scheitern. Jede Meldung,
# die bleiben darf, steht mit Begruendung in `packaging/deb/lintian-overrides`;
# die Datei reist im Paket mit (`/usr/share/lintian/overrides/humanitl`), so
# wie Debian es vorsieht. Hinweise (`I:`) und pedantische Meldungen (`P:`)
# werden gezeigt, aber nicht erzwungen.
set -euo pipefail

[[ $# -eq 1 ]] || { echo "usage: $0 <deb>" >&2; exit 2; }
deb="$(readlink -f "$1")"
[[ "$(id -u)" -eq 0 ]] || { echo "error: run as root inside a throwaway container" >&2; exit 1; }

export DEBIAN_FRONTEND=noninteractive
if ! command -v lintian >/dev/null; then
  apt-get update -qq
  apt-get install -y -qq --no-install-recommends lintian >/dev/null
fi
lintian --version
lintian --display-level '>=pedantic' --show-overrides --fail-on error,warning "$deb"
echo "== lintian: no errors, no warnings"
