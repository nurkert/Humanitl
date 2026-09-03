#!/usr/bin/env bash
# Das Demoskript des Meilensteins M1 (HUM-021).
#
# Es beweist in einem Lauf die drei Sätze, mit denen M1 steht oder fällt
# (BACKLOG.md 7 und 8):
#
#   1. Die Sandbox ist dicht. `humanitl sandbox check` zeigt die drei
#      Garantien, `humanitl sandbox run` belegt sie noch einmal aus der
#      laufenden Sandbox, und ein Prozess, der die Proxy-Umgebung ignoriert
#      (`env -i`), kommt weder an ein Ziel, das vom Host aus nachweislich
#      antwortet, noch an einen Namen.
#   2. Ein Request wird gehalten. Der `curl` aus der Sandbox erscheint als
#      wartender Flow im Daemon, mit seinem Host.
#   3. Die Entscheidung des Menschen wirkt. Ein Block endet beim wartenden
#      Klienten als 403 mit dem verabredeten Body, eine Freigabe als die
#      Antwort des Ziels, und was niemand entscheidet, läuft in die
#      Zeitüberschreitung.
#
# Gefahren wird alles über die Kommandozeile `humanitl`, nicht über ein
# Ad-hoc-Skript: Das Demo soll dieselben Codepfade nehmen, die später beim
# Nutzer laufen (HUM-064, CONVENTIONS.md 3.11).
#
# Der ganze Lauf liegt in einem eigenen Nutzer- und Netz-Namensraum
# (`e2e_enter_namespace`). Das hat zwei Gründe. Erstens braucht Schritt 3 ein
# Ziel, das der Proxy erreichen darf: Der Proxy weist jede aufgelöste Adresse
# in einem privaten Bereich ab, also auch das Loopback, und ein Ziel auf
# 127.0.0.1 könnte eine Freigabe deshalb nie belegen. Im eigenen Namensraum
# liegt das Ziel auf einer Adresse aus TEST-NET-2, die nicht privat ist.
# Zweitens hat der Namensraum keine Route nach draußen: Der geblockte Request
# in Schritt 3 kann damit gar nicht im Netz gelandet sein, und der Lauf braucht
# kein Internet.
#
#   ./tests/e2e/m1_sealed_box.sh          bauen und laufen
#   E2E_SKIP_BUILD=1 ./tests/e2e/m1_sealed_box.sh
#                                         die Binaries nehmen, wie sie sind
#   E2E_TRACE=1 ./tests/e2e/m1_sealed_box.sh
#                                         zusätzlich `set -x` (die CI setzt es)
#
# Exit-Codes: 0 alles belegt, 1 eine Behauptung hielt nicht oder eine
# Voraussetzung fehlte. Was jede Behauptung geprüft hat, steht als eigene Zeile
# im Protokoll, auch wenn sie hielt.

set -euo pipefail

E2E_SCRIPT="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
E2E_SHELL=bash
export E2E_SCRIPT E2E_SHELL

# shellcheck source=tests/e2e/lib.sh
. "$(dirname "$0")/lib.sh"

if [ "${E2E_TRACE:-0}" = 1 ]; then
    set -x
fi

# --- Voraussetzungen ---------------------------------------------------------

for tool in bwrap curl jq python3 ip unshare; do
    command -v "$tool" > /dev/null 2>&1 ||
        e2e_die "$tool is missing; the demo needs bubblewrap, curl, jq, python3, iproute2 and util-linux"
done

# Gebaut wird vor dem Wechsel in den Namensraum, weil es dort kein Netz gibt.
e2e_build
e2e_enter_namespace "$@"

# --- Der Lauf ----------------------------------------------------------------

E2E_OUT="$E2E_ROOT/target/e2e"
rm -rf "$E2E_OUT"
mkdir -p "$E2E_OUT"

e2e_short_workdir

collect() {
    stop_daemon
    stop_fake_upstream
    cp -f "$E2E_WORKDIR"/out/* "$E2E_OUT/" 2> /dev/null || true
    cp -f "$E2E_WORKDIR"/daemon.log "$E2E_OUT/" 2> /dev/null || true
}
trap collect EXIT

start_fake_upstream
start_daemon "$E2E_WORKDIR/state" "$E2E_WORKDIR" 10

# Der Beleg, dass das Ziel überhaupt antwortet, bevor irgendwo behauptet wird,
# die Sandbox komme nicht an es heran. Ohne diese Zeile hieße ein
# fehlgeschlagener `curl` in Schritt 2 nur „hier antwortet niemand".
reachable=$(curl -sS --max-time 5 "http://$E2E_FAKE_ADDR:$FAKE_HTTP/reachable" || true)
e2e_expect_match "the target answers on the host of the namespace" \
    '"path": "/reachable"' "$reachable"

info=$(daemon_info)
e2e_expect_match "the daemon serves GetInfo and runs a proxy session" \
    '"session_id":"[0-9a-f-]{36}"' "$info"

# --- 1. Isolation ------------------------------------------------------------

e2e_step "1. the sandbox is sealed"

# `humanitl sandbox check` startet eine kurzlebige Sandbox im ausgelieferten
# Profil und zeigt die drei Garantien als Tabelle mit ✓ (HUM-064). Es braucht
# den Daemon nicht: Socket und CA sind Platzhalter in einem eigenen
# Verzeichnis, damit ein Selbsttest dem laufenden Daemon nichts wegnimmt.
selftest=$(humanitl sandbox check 2>&1) ||
    e2e_die "humanitl sandbox check failed: $selftest"
printf '%s\n' "$selftest" | sed 's/^/  /'
for guarantee in no_network_interface single_socket seccomp_active; do
    e2e_expect_match "sandbox check proves $guarantee" "✓ +$guarantee" "$selftest"
done

# Und dasselbe noch einmal für die Sandbox, in der der Rest des Demos läuft:
# `humanitl sandbox run` läuft fail-closed, startet den Befehl also nur, wenn
# alle drei Garantien aus der laufenden Sandbox belegt sind, und schreibt mit
# `-v` je eine Zeile `check <name> pass|FAIL: <evidence>` nach stderr
# (HUM-011, HUM-064, CONVENTIONS 4.12).
isolation=$(sandbox_run /bin/true 2>&1 > /dev/null || e2e_die "the sandbox did not start")
printf '%s\n' "$isolation" | sed 's/^/  /'
checks=$(printf '%s\n' "$isolation" | grep -c 'check .* pass:' || true)
e2e_expect "three isolation guarantees are proven from inside the sandbox" 3 "$checks"
e2e_expect_match "no interface but lo" 'check no_network_interface pass' "$isolation"
e2e_expect_match "exactly one socket, and it is the proxy" 'check single_socket pass' "$isolation"
e2e_expect_match "seccomp is active in the agent process" 'check seccomp_active pass' "$isolation"

# --- 2. Ohne Proxy-Env kein Weg ----------------------------------------------

e2e_step "2. without the proxy environment there is no way out"

# `env -i` wirft HTTP_PROXY und alles andere weg. Das Ziel ist dasselbe, das
# eben vom Host aus geantwortet hat; hier darf es nicht erreichbar sein.
no_proxy_ip=$(sandbox_run /usr/bin/env -i \
    /usr/bin/curl -sS --max-time 5 "http://$E2E_FAKE_ADDR:$FAKE_HTTP/echo" 2>&1 || true)
e2e_expect_match "no route to the target that answers on the host" \
    'Network is unreachable|Failed to connect|Couldn.t connect|Connection refused' "$no_proxy_ip"
if printf '%s\n' "$no_proxy_ip" | grep -q '"path"'; then
    e2e_check "the target answered a request that bypassed the proxy" no "$no_proxy_ip"
else
    e2e_check "the target did not answer a request that bypassed the proxy" ok
fi

no_proxy_dns=$(sandbox_run /usr/bin/env -i \
    /usr/bin/curl -sS --max-time 5 http://example.com/ 2>&1 || true)
e2e_expect_match "no name is resolved inside the sandbox" \
    'Could not resolve host|Resolving timed out' "$no_proxy_dns"

# --- 3. Gehalten und geblockt ------------------------------------------------

e2e_step "3. a request is held and a human blocks it"

# Das Ziel ist der Fake-Upstream auf einem eigenen Pfad, nicht ein Name im
# Internet. Nur so lässt sich am Ende beides zugleich belegen: dass die
# freigegebene Anfrage ankam und dass die geblockte dasselbe Ziel nie erreicht
# hat. Ein Ziel, das gar nicht antwortet, bewiese das zweite nicht.
sandbox_run /usr/bin/curl -sS --max-time 60 -o /work/out.txt -w '%{http_code}' \
    "http://$E2E_FAKE_ADDR:$FAKE_HTTP/blocked" \
    > "$E2E_WORKDIR/out/code.txt" 2> "$E2E_WORKDIR/out/blocked.log" &
blocked_pid=$!

flow=$(wait_for_held 20 /blocked) ||
    e2e_die "no held flow for /blocked within twenty seconds"
e2e_say "held flow $flow"
detail=$(flow_show "$flow")
e2e_expect "the held flow names the host the agent asked for" \
    "$E2E_FAKE_ADDR:$FAKE_HTTP" "$(printf '%s' "$detail" | jq -r .host)"
e2e_expect "and the path it asked for" /blocked "$(printf '%s' "$detail" | jq -r .path)"
e2e_expect "and it is really waiting" held "$(printf '%s' "$detail" | jq -r .state)"

flow_decide "$flow" block "nicht in diesem Lauf" ||
    e2e_die "the daemon refused the block"
wait "$blocked_pid" || true

e2e_expect "the waiting client gets 403" 403 "$(cat "$E2E_WORKDIR/out/code.txt")"
body=$(cat "$E2E_WORKDIR/work/out.txt")
e2e_expect_match "the body is the agreed block answer" '^Blocked by Humanitl\.$' "$body"
e2e_expect_match "and names the human as the reason" '^reason: user$' "$body"
e2e_expect_match "and the flow the answer belongs to" "^flow: $flow\$" "$body"
# Der Body nennt den Host ohne Port; `flows show` nennt ihn mit, weil dort die
# Authority steht, wie sie in einer URL stünde.
e2e_expect_match "and the host that was asked for" "^host: $E2E_FAKE_ADDR\$" "$body"
e2e_expect_match "and carries the note of the human" '^note: nicht in diesem Lauf$' "$body"

# --- 4. Gehalten und erlaubt -------------------------------------------------

e2e_step "4. a request is held and a human allows it"

sandbox_run /usr/bin/curl -sS --max-time 60 -o /work/out2.txt -w '%{http_code}' \
    "http://$E2E_FAKE_ADDR:$FAKE_HTTP/echo" \
    > "$E2E_WORKDIR/out/code2.txt" 2> "$E2E_WORKDIR/out/allowed.log" &
allowed_pid=$!

flow=$(wait_for_held 20 /echo) ||
    e2e_die "no held flow for /echo within twenty seconds"
e2e_say "held flow $flow"
flow_decide "$flow" allow || e2e_die "the daemon refused the allow"
wait "$allowed_pid" || true

e2e_expect "the released client gets 200" 200 "$(cat "$E2E_WORKDIR/out/code2.txt")"
answer=$(cat "$E2E_WORKDIR/work/out2.txt")
e2e_say "the target answered: $answer"
printf '%s' "$answer" | jq -e '.path == "/echo"' > /dev/null ||
    e2e_check "the answer of the target reaches the agent unchanged" no "$answer"
e2e_check "the answer of the target reaches the agent unchanged" ok

# --- 5. Was niemand entscheidet ----------------------------------------------

e2e_step "5. what nobody decides runs into the hold timeout"

sandbox_run /usr/bin/curl -sS --max-time 60 -o /work/out3.txt -w '%{http_code}' \
    "http://$E2E_FAKE_ADDR:$FAKE_HTTP/echo" \
    > "$E2E_WORKDIR/out/code3.txt" 2> "$E2E_WORKDIR/out/timeout.log" &
wait $! || true

# 504, nicht 403: `BlockReason::Timeout` ist nach CONVENTIONS.md 3.2 ein
# Gateway-Timeout. Die Fassung des Skripts in sprint-1.md nennt hier noch 403
# und ist damit älter als die Entscheidung in 4.11.
e2e_expect "the undecided client gets 504" 504 "$(cat "$E2E_WORKDIR/out/code3.txt")"
timed_out=$(cat "$E2E_WORKDIR/work/out3.txt")
e2e_expect_match "and the body names the timeout" '^reason: timeout$' "$timed_out"

# --- Was das Ziel gesehen hat ------------------------------------------------

e2e_step "what the target itself saw"

# Die Gegenprobe zu allem, und zwar am selben Ziel: Schritt 3 (geblockt),
# Schritt 4 (freigegeben) und Schritt 5 (Zeitüberschreitung) haben alle den
# Fake-Upstream gemeint, jeder auf seinem Pfad. Bedient hat er genau eine
# Anfrage, die aus Schritt 4. `/blocked` steht in seinem Protokoll null mal:
# Was ein Mensch verboten hat, ist nie bei ihm angekommen, und Schritt 2 (am
# Proxy vorbei) und Schritt 5 (nie entschieden) ebenso wenig.
served=$(grep -c ' /echo ' "$E2E_WORKDIR/out/upstream.log" || true)
e2e_expect "the target served exactly the one request a human allowed" 1 "$served"
blocked_served=$(grep -c ' /blocked ' "$E2E_WORKDIR/out/upstream.log" || true)
e2e_expect "and never the one a human forbade" 0 "$blocked_served"

# Das dritte Kriterium aus der Spezifikation, „null aufgelöste Namen in der
# Sandbox", zählt HUM-024: Erst der Resolver des Daemons führt Buch darüber,
# wer welchen Namen aufgelöst hat. Bis dahin belegt Schritt 2 dasselbe von der
# anderen Seite: In der Sandbox scheitert jede Auflösung.

# --- Der geordnete Abschied --------------------------------------------------

e2e_step "the daemon leaves nothing behind"

stop_daemon
[ ! -e "$DAEMON_SOCK" ] ||
    e2e_check "SIGTERM removes the daemon socket" no "$DAEMON_SOCK is still there"
e2e_check "SIGTERM removes the daemon socket" ok
[ ! -e "$DAEMON_TOKEN" ] ||
    e2e_check "SIGTERM removes the token" no "$DAEMON_TOKEN is still there"
e2e_check "SIGTERM removes the token" ok
[ ! -e "$DAEMON_PROXY_SOCK" ] ||
    e2e_check "SIGTERM ends the proxy session" no "$DAEMON_PROXY_SOCK is still there"
e2e_check "SIGTERM ends the proxy session" ok

echo
echo "M1 demo: OK"
