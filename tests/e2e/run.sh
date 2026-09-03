#!/usr/bin/env sh
# Der Einstieg des End-to-End-Laufs. `make e2e` und der CI-Job `e2e` rufen ihn.
#
#   ./tests/e2e/run.sh            bauen und das Demo des aktuellen Meilensteins fahren
#   E2E_SKIP_BUILD=1 ./tests/e2e/run.sh
#                                 die Binaries nehmen, wie sie sind
#
# Ein Meilenstein, ein Demoskript: heute `m1_sealed_box.sh`. Kommt M2 dazu,
# wird hier eine zweite Zeile stehen und keine zweite Datei nötig sein. Der
# Einstieg tut selbst nichts weiter, damit ein Entwickler das Demo auch direkt
# aufrufen kann und dabei genau dasselbe bekommt wie die CI.
#
# Exit-Codes: 0 alle Demos grün, sonst der Code des ersten, das rot war.
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"

echo "== M1: the sealed box =="
"$HERE/m1_sealed_box.sh" "$@"
