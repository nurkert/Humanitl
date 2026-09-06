#!/usr/bin/env sh
# Der Einstieg des End-to-End-Laufs. `make e2e` und die CI-Jobs rufen ihn.
#
#   ./tests/e2e/run.sh            bauen und die Demos der Meilensteine fahren
#   E2E_SKIP_BUILD=1 ./tests/e2e/run.sh
#                                 die Binaries nehmen, wie sie sind
#   E2E_ONLY=m1|m2|m3 ./tests/e2e/run.sh
#                                 nur eines der drei fahren
#
# Ein Meilenstein, ein Demoskript, und jedes bleibt stehen, wenn das nächste
# dazukommt: Ein Sprint gilt erst als fertig, wenn alle bisherigen Demos grün
# sind (BACKLOG.md 8, CONTRIBUTING.md „Sprint gate"). Der Einstieg tut selbst
# nichts weiter, damit ein Entwickler ein Demo auch direkt aufrufen kann und
# dabei genau dasselbe bekommt wie die CI.
#
# Der zweite Lauf baut nicht noch einmal: Beide Skripte bauen dieselben drei
# Binaries, und der erste hat es schon getan.
#
# Exit-Codes: 0 alle Demos grün, sonst der Code des ersten, das rot war.
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
ONLY="${E2E_ONLY:-all}"

if [ "$ONLY" = all ] || [ "$ONLY" = m1 ]; then
    echo "== M1: the sealed box =="
    "$HERE/m1_sealed_box.sh" "$@"
    E2E_SKIP_BUILD=1
    export E2E_SKIP_BUILD
fi

if [ "$ONLY" = all ] || [ "$ONLY" = m2 ]; then
    echo
    echo "== M2: the first decision =="
    "$HERE/m2_first_decision/run.sh" "$@"
    E2E_SKIP_BUILD=1
    export E2E_SKIP_BUILD
fi

if [ "$ONLY" = all ] || [ "$ONLY" = m3 ]; then
    echo
    echo "== M3: the agent inside =="
    # Der eigene Test des Mock-Sprachmodells zuerst, und als eigener Schritt:
    # Der Mock trägt das ganze Milestone, und ein roter M3-Lauf soll von einem
    # kaputten Testdouble unterscheidbar sein (HUM-046).
    "$HERE/mock_llm/self_test.sh"
    "$HERE/m3_agent_inside/run.sh" "$@"
fi
