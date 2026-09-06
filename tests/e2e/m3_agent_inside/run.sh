#!/usr/bin/env bash
# Das Demoskript des Meilensteins M3 (HUM-046).
#
# M1 hat gezeigt, dass die Kiste dicht ist. M2 hat den ersten vollständigen
# Arbeitsabschnitt eines Menschen gezeigt: siebzehn Anfragen, eine Gruppe, eine
# Sitzungsregel, ein Block, eine Zeitüberschreitung, die Historie. M3 zeigt den
# Agenten selbst — die Sitzung, wie ein Mensch sie startet, mit dem
# Sprachmodell daneben.
#
# Belegt werden in einem Lauf:
#
#   1. **Eine Sitzung, wie ein Mensch sie startet.** `humanitl run` im
#      Projektverzeichnis (HUM-067). Der Daemon löst das Profil auf, baut die
#      Regeln der Sitzung, startet die Sandbox an einem Pseudoterminal und
#      schickt seine drei Garantien als Ereignis zurück (HUM-041).
#   2. **Die Durchreiche zum Sprachmodell.** Zehn SSE-Rahmen kommen beim Agenten
#      an, verteilt über die Zeit; der Mock hat genau eine Inferenzanfrage
#      bedient und trägt den Rumpf, den der Agent geschickt hat. Der Fluss
#      selbst bleibt in der Warteschlange und in der Liste unsichtbar — das ist
#      die Ausnahme, die die Durchreiche ausmacht.
#   3. **Die mitgelieferten Regeln greifen.** Der Modellkatalog `models.dev`
#      wird ohne Rückfrage geblockt, bevor irgendein Name aufgelöst wird.
#   4. **Ein Mensch entscheidet über gRPC, und die Antwort erreicht den
#      Agenten.** Eine Anfrage wartet, wird über die Kommandozeile freigegeben,
#      und der Agent bekommt den Inhalt des Ziels über TLS zurück.
#   5. **Das Terminal des Agenten.** Ein zweites Terminal hängt sich mit
#      `humanitl sandbox attach --read-only` an dieselbe Sitzung (HUM-042) und
#      sieht die Hinweiszeilen des Daemons. Ein Pfad, der eine solche Zeile
#      fälschen will, verliert dabei die eckige Klammer, die dem Absender
#      gehört; eine OSC-52-Nutzlast des Agenten kommt nicht durch, eine
#      Farbfolge schon.
#   6. **Die Notiz eines Menschen.** Sie erreicht den Agenten als genau eine
#      Kopfzeile, auch wenn sie CR, LF und ein Steuerbyte enthält.
#   7. **Was der Lauf im Projekt hinterlassen hat.** `humanitl sessions
#      summary` nennt die eine Datei, die der Agent geschrieben hat (HUM-043).
#   8. **Eine zweite Sitzung ohne Menschen.** `humanitl run --profile llm-only`
#      lässt die Inferenz durch und blockt alles andere, ohne dass jemand
#      wartet (HUM-066, HUM-067).
#   9. **Derselbe Lauf mit echtem OpenCode**, wenn die Sandbox das Binary sieht.
#      Sonst wird die Variante ausdrücklich als übersprungen gemeldet, nie als
#      bestanden.
#
# Gefahren wird alles über die Kommandozeile `humanitl`, nicht über einen
# eigenen gRPC-Klienten, und der Agent spricht über `curl` mit dem Proxy:
# Gemessen werden sollen die Codepfade, die später beim Nutzer laufen
# (`backlog/CONVENTIONS.md` 3.11, ADR-018).
#
# Der ganze Lauf liegt in einem eigenen Nutzer- und Netz-Namensraum, aus
# denselben zwei Gründen wie bei M1 und M2: Die beiden Ziele brauchen eine
# Adresse, die der Proxy erreichen darf (also keine private), und der
# Namensraum hat keine Route nach draußen, der Lauf also kein Netz.
#
#   ./tests/e2e/m3_agent_inside/run.sh   bauen und laufen
#   E2E_SKIP_BUILD=1 …                   die Binaries nehmen, wie sie sind
#   E2E_TRACE=1 …                        zusätzlich `set -x` (die CI setzt es)
#   M3_OPENCODE=0|1|auto …               die OpenCode-Variante aus- oder
#                                        erzwingen; ohne Angabe läuft sie,
#                                        sobald die Sandbox das Binary sieht
#
# Exit-Codes: 0 alles belegt, 1 eine Behauptung hielt nicht oder eine
# Voraussetzung fehlte, 130 ein Abbruch durch ein Signal. Jede geprüfte
# Behauptung steht als eigene Zeile im Protokoll, auch wenn sie hielt, und am
# Ende steht, wie viele es waren und was übersprungen wurde.
#
# --- Was ein grüner Lauf trägt, und was nicht --------------------------------
#
# Ein Gate ist nur so viel wert, wie ein späterer Leser über seine Reichweite
# weiß. Deshalb ausdrücklich:
#
#   * Er sagt **nichts über den Bildschirm**. Sandbox-Bildschirm, Terminal-Reiter
#     und Isolations-Panel werden nicht bedient; die Oberflächen-Hälfte von M3
#     gehört in den Job `e2e-xvfb` und ist nicht gebaut.
#   * Er sagt **nichts über die Audit-Kette**. `humanitl audit` ist ein
#     Platzhalter (HUM-070); der Lauf hält das mit einem Stolperdraht fest,
#     der rot wird, sobald es einen gibt.
#   * Er sagt **nichts über den Durchreich-Fluss in der Liste**. Die
#     Kommandozeile hat keinen Schalter für `include_passthrough`
#     (`daemon/bin/humanitl/src/cmd/flows.rs`, `include_passthrough: false`);
#     der Lauf misst die Durchreiche deshalb an dem, was der Agent bekam, an
#     dem, was der Mock empfangen hat, und an ihrer Abwesenheit in der Liste —
#     nicht an einem Feld `decision_source`. Auch das trägt ein Stolperdraht.
#   * Er sagt **nichts über eine Verweigerung ohne `--allow-test-ca`**. Dieser
#     Lauf fährt mit dem Flag; die andere Richtung misst der Rust-Test
#     `a_test_ca_is_only_trusted_with_the_flag` (HUM-087).
#   * Er sagt nichts über Neustarts (das prüft M1), über die Gruppierung und
#     die Historie in der Breite (das prüft M2) und über Benachrichtigungen
#     (abgeschaltet).
#
# Was er trägt, steht oben in der Aufzählung, und jede einzelne Behauptung
# steht im Protokoll des Laufs.

set -euo pipefail

M3_DIR="$(cd "$(dirname "$0")" && pwd)"
E2E_ROOT="$(cd "$M3_DIR/../../.." && pwd)"
E2E_SCRIPT="$M3_DIR/run.sh"
E2E_SHELL=bash
export E2E_ROOT E2E_SCRIPT E2E_SHELL

# shellcheck source=tests/e2e/lib.sh
. "$E2E_ROOT/tests/e2e/lib.sh"

if [ "${E2E_TRACE:-0}" = 1 ]; then
    set -x
fi

# --- Was dieser Lauf erwartet ------------------------------------------------

# So viele Behauptungen prüft ein vollständiger Lauf ohne die
# OpenCode-Variante. Die Selbstprüfung am Ende vergleicht die Zahl mit dem
# Zähler aus `lib.sh`. Ein Skript, das grün ist, weil ein Zweig übersprungen
# wurde, ist schlimmer als keines; deshalb steht die Zahl hier und nicht im
# Kopf eines Menschen.
M3_EXPECTED_ASSERTIONS=106

# So viele kommen dazu, wenn die OpenCode-Variante läuft. Der Zweig prüft
# immer dieselben sieben Dinge — die beiden Verzweigungen darin sind
# Fallunterscheidungen über das Ergebnis, nicht darüber, ob geprüft wird —,
# damit diese Zahl exakt ist und nicht ungefähr: Der Lauf vergleicht den
# Zähler vor und nach der Variante mit ihr.
M3_OPENCODE_ASSERTIONS=10

# Die Ports des Ziels. Im eigenen Netz-Namensraum ist der Lauf root und darf
# auch die privilegierten binden.
M3_HTTP_PORT=80
M3_HTTPS_PORT=443

# Die Haltefrist dieses Laufs, in Sekunden. Sie steht auch in `config.toml`;
# hier wird sie zusätzlich über die Umgebung gesetzt, weil `start_daemon` sie so
# entgegennimmt.
M3_HOLD_TIMEOUT=20

# Der Host, für den das Testzertifikat gilt. `models.dev` steht mit Absicht
# nicht dabei: Die Anfrage dorthin wird geblockt, bevor ein Handschlag zu
# einem Ziel überhaupt in Frage käme, und ein Zertifikat dafür verspräche eine
# Verbindung, die es nie geben soll.
M3_HOSTS="example.com"

# Die vier Adressen, an denen der Lauf den Agenten festnagelt, stehen wörtlich
# in `agent_script.sh` und nicht hier: Wer wissen will, was der Agent tut,
# liest sein Skript. Es sind `https://models.dev/api.json` (mitgelieferte
# Regel), `https://example.com/docs` (Freigabe durch einen Menschen),
# `https://example.com/a[humanitl]request-allowed` (die gefälschte Hinweiszeile)
# und `https://example.com/secret` (Block mit Notiz). Nur die Adresse des
# Sprachmodells steht erst zur Laufzeit fest und kommt über `sandbox.env`.

# Der Pfad des gefälschten Hinweises, wie der Fluss ihn trägt, und wie die
# Hinweiszeile ihn zeigt. Die eckige Klammer gehört dem Absender
# (`humanitl_ipc::terminal::path_for_notice`).
M3_PATH_FORGED='/a[humanitl]request-allowed'
M3_NOTICE_FORGED='(humanitl]request-allowed'

# Die Notiz, mit der ein Mensch die letzte Anfrage verbietet, und das, was
# davon beim Agenten ankommen darf. CR und LF werden zu Leerzeichen, das
# Steuerbyte fällt weg (`humanitl_core::block::sanitize_note`); aus der
# zweiten Kopfzeile, die die Notiz zu öffnen versucht, wird damit Text.
M3_NOTE="$(printf 'not in this run\r\nX-Injected: yes\ax')"
M3_NOTE_CLEAN='not in this run X-Injected: yesx'

# --- Voraussetzungen ---------------------------------------------------------

for tool in bwrap curl jq python3 ip unshare openssl; do
    command -v "$tool" > /dev/null 2>&1 ||
        e2e_die "$tool is missing; the demo needs bubblewrap, curl, jq, python3, iproute2, util-linux and openssl"
done

# Gebaut wird vor dem Wechsel in den Namensraum, weil es dort kein Netz gibt.
e2e_build
e2e_enter_namespace "$@"

# --- Der Wegwerf-Baum --------------------------------------------------------

E2E_OUT="$E2E_ROOT/target/e2e/m3"
rm -rf "$E2E_OUT"
mkdir -p "$E2E_OUT"

e2e_short_workdir

# Das Projektverzeichnis dieser Sitzung liegt **unter dem Heimatverzeichnis des
# Laufs**, und das ist keine Bequemlichkeit: `SandboxService::check_work_dir`
# nimmt ein Projekt nur an, wenn es unter `$HOME` liegt oder genau das
# Verzeichnis aus `sandbox.work_dir` ist (`SANDBOX_006`). Ein Client darf sich
# `/etc` nicht als Projekt wünschen. `$E2E_WORKDIR/work` aus `lib.sh` liegt
# neben dem Heimatverzeichnis und wäre abgelehnt worden — der M2-Lauf startet
# seine Sandbox über `humanitl sandbox run`, wo diese Prüfung nicht greift.
M3_PROJECT="$E2E_WORKDIR/home/project"
mkdir -p "$M3_PROJECT"

M3_CA_DIR="$E2E_WORKDIR/ca-test"
M3_AGENT_OUT="$E2E_WORKDIR/out/agent.transcript"
M3_AGENT_ERR="$E2E_WORKDIR/out/agent.log"
M3_ATTACH_OUT="$E2E_WORKDIR/out/attached.transcript"
M3_ATTACH_ERR="$E2E_WORKDIR/out/attached.log"
M3_LLM_LOG="$E2E_WORKDIR/out/mock_llm.log"
M3_UPSTREAM_LOG="$E2E_WORKDIR/out/upstream.log"
# Zwei Stände der Flow-Liste, und beide gehören in die Artefakte. Der erste
# ist der, auf den die Schritte 7 und 11 sich beziehen: die vier Flüsse der
# ersten Sitzung. Der zweite entsteht am Ende und trägt alles, was danach noch
# dazugekommen ist — die llm-only-Sitzung und, wo sie läuft, die
# OpenCode-Variante. Ein einziger Stand hieße, dass das hochgeladene Artefakt
# eine Historie zeigt, die der Lauf selbst überholt hat.
M3_FLOWS_FIRST_JSON="$E2E_WORKDIR/out/flows-after-first-session.json"
M3_FLOWS_JSON="$E2E_WORKDIR/out/flows.json"
M3_SUMMARY_JSON="$E2E_WORKDIR/out/summary.json"
M3_ONLY_OUT="$E2E_WORKDIR/out/llm_only.transcript"
M3_ONLY_ERR="$E2E_WORKDIR/out/llm_only.log"
M3_OC_OUT="$E2E_WORKDIR/out/opencode.transcript"
M3_OC_ERR="$E2E_WORKDIR/out/opencode.log"
M3_RUN_PID=""
M3_ATTACH_PID=""
M3_LLM_PID=""
M3_SANDBOX_ID=""

# Was der Lauf ausgelassen hat, in Worten. Ein übersprungener Zweig darf am
# Ende nicht wie ein bestandener aussehen; diese Liste steht im Abschlussblock
# und macht aus „OK" ein „OK, mit diesen Lücken".
M3_SKIPPED=""

m3_skip() {
    e2e_say "SKIPPED: $*"
    if [ -z "$M3_SKIPPED" ]; then
        M3_SKIPPED="$*"
    else
        M3_SKIPPED="$M3_SKIPPED; $*"
    fi
}

# Aufräumen, das auch nach einem Fehlschlag greift: erst die Prozesse dieses
# Laufs, dann die Protokolle in den Artefakt-Ordner, dann der Wegwerf-Baum.
collect() {
    for pid in "$M3_ATTACH_PID" "$M3_RUN_PID"; do
        if [ -n "$pid" ]; then
            kill "$pid" 2> /dev/null || true
            wait "$pid" 2> /dev/null || true
        fi
    done
    M3_ATTACH_PID=""
    M3_RUN_PID=""
    stop_daemon
    stop_fake_upstream
    if [ -n "$M3_LLM_PID" ]; then
        kill "$M3_LLM_PID" 2> /dev/null || true
        wait "$M3_LLM_PID" 2> /dev/null || true
        M3_LLM_PID=""
    fi
    if [ -n "${E2E_WORKDIR:-}" ] && [ -d "$E2E_WORKDIR" ]; then
        cp -f "$E2E_WORKDIR"/out/* "$E2E_OUT/" 2> /dev/null || true
        cp -f "$E2E_WORKDIR"/daemon.log "$E2E_OUT/" 2> /dev/null || true
        case "$E2E_WORKDIR" in
        /tmp/hum-e2e-*) rm -rf "$E2E_WORKDIR" ;;
        esac
    fi
}
# Auch bei einem Abbruch, nicht nur beim geordneten Ende (`lib.sh`, `e2e_trap`).
e2e_trap collect

# --- Helfer dieses Laufs -----------------------------------------------------

# m3_flow_page [FILTER] — eine Seite der Flow-Liste als JSON.
m3_flow_page() {
    if [ -z "${1:-}" ]; then
        humanitl --json flows list --asc 2> /dev/null
    else
        humanitl --json flows list --asc "$1" 2> /dev/null
    fi
}

# m3_count [FILTER] — wie viele Flüsse der Filter trifft, `-1` bei einem Fehler.
m3_count() {
    if ! m3_count_out=$(m3_flow_page "${1:-}" | jq -r '.flows | length' 2> /dev/null); then
        m3_count_out=-1
    fi
    [ -n "$m3_count_out" ] || m3_count_out=-1
    printf '%s\n' "$m3_count_out"
}

# m3_row FILTER — die erste Zeile des Filters als JSON, sonst Rückgabewert 1.
m3_row() {
    m3_row_out=$(m3_flow_page "$1" | jq -c '.flows[0] // empty' 2> /dev/null) || return 1
    [ -n "$m3_row_out" ] || return 1
    printf '%s\n' "$m3_row_out"
}

# m3_field FILTER FIELD — ein Feld der ersten Zeile, oder der leere Text.
m3_field() {
    m3_row "$1" | jq -r --arg field "$2" '.[$field] // ""' 2> /dev/null || true
}

# m3_agent_line MARK — die erste Zeile der Agentenausgabe, die auf MARK passt.
#
# Das Transkript kommt von einem Pseudoterminal; seine Zeilendisziplin macht
# aus `\n` ein `\r\n`. Das CR fällt hier weg, damit ein Vergleich nicht an
# einem unsichtbaren Byte scheitert.
m3_agent_line() {
    tr -d '\r' < "$M3_AGENT_OUT" | grep -m 1 -e "$1" || true
}

# m3_agent_value MARK — der Text hinter dem ersten `=` einer solchen Zeile.
m3_agent_value() {
    m3_agent_line "$1" | sed -e 's/^[^=]*=//'
}

# m3_notice_count PATTERN — wie oft eine Hinweiszeile im angehängten Terminal
# auf PATTERN passt.
m3_notice_count() {
    tr -d '\r' < "$M3_ATTACH_OUT" | grep -c -e "$1" || true
}

# m3_llm_hits PATTERN — wie oft das Sprachmodell eine passende Anfrage bediente.
m3_llm_hits() {
    grep -c -e "$1" "$M3_LLM_LOG" 2> /dev/null || true
}

# m3_upstream_hits PATTERN — wie oft das Ziel eine passende Anfrage bediente.
m3_upstream_hits() {
    grep -c -e "$1" "$M3_UPSTREAM_LOG" 2> /dev/null || true
}

# m3_has_escape FILE — ob in FILE eine OSC-Einleitung (`ESC ]`) steht.
#
# Der Vergleich läuft byteweise (`LC_ALL=C`), weil das Transkript ein Bytestrom
# ist und keine Textdatei in der Sprache des Läufers.
m3_has_escape() {
    LC_ALL=C grep -q -- "$(printf '\033')]" "$1"
}

# m3_wait_ready FILE PID SECONDS — auf die Bereitschaftszeile eines Servers
# warten, sie auf stdout ausgeben.
#
# Rückgabewert 0 mit der Zeile, 1 nach Ablauf der Frist, 2 wenn der Prozess
# vorher gestorben ist.
#
# **Warum eine Datei und keine Fifo, und warum überhaupt eine Frist.** Die
# ersten Fassungen von M2 und M3 lasen die Zeile mit `read < fifo`. Das wartet
# unbegrenzt, und zwar schon beim Öffnen: Kommt der Server gar nicht hoch —
# ein Syntaxfehler, ein belegter Port, ein fehlendes `python3` —, steht der
# ganze Lauf, bis die CI ihn nach dreissig Minuten abbricht, und im Bericht
# steht eine Zeitüberschreitung, die nichts sagt. Mit einer Datei ist das
# Warten ein Poll, und daneben passt die zweite Frage: Lebt der Prozess
# überhaupt noch? Stirbt er, ist die Antwort sofort da und nicht erst nach der
# Frist. Das ist dieselbe Regel wie bei `--max-time` in den Agentenskripten,
# nur eine Stufe früher.
m3_wait_ready() {
    m3_ready_left=$(( ${3:-20} * 10 ))
    while [ "$m3_ready_left" -gt 0 ]; do
        m3_ready_line=$(grep -m 1 '^READY ' "$1" 2> /dev/null || true)
        if [ -n "$m3_ready_line" ]; then
            printf '%s\n' "$m3_ready_line"
            return 0
        fi
        if ! kill -0 "$2" 2> /dev/null; then
            return 2
        fi
        sleep 0.1
        m3_ready_left=$((m3_ready_left - 1))
    done
    return 1
}

# m3_ready_failed WHAT PID CODE LOG — die Meldung zu einer misslungenen
# Bereitschaft, mit dem Protokoll des Servers.
m3_ready_failed() {
    if [ "$3" = 2 ]; then
        e2e_die "$1 died before it reported that it was ready; its log follows:
$(cat "$4" 2> /dev/null)"
    fi
    kill "$2" 2> /dev/null || true
    e2e_die "$1 did not report that it was ready within the deadline; its log follows:
$(cat "$4" 2> /dev/null)"
}

# m3_start_llm — das Sprachmodell dieses Laufs starten.
#
# Der Port kommt über die Bereitschaftszeile zurück, damit das Skript ihn nicht
# raten muss; `--port 0` lässt das Betriebssystem einen freien wählen, und eine
# feste Zahl hier kollidierte irgendwann mit etwas anderem im Namensraum.
m3_start_llm() {
    m3_ready_file="$E2E_WORKDIR/llm.ready"
    rm -f "$m3_ready_file"
    : > "$m3_ready_file"
    python3 "$E2E_ROOT/tests/e2e/mock_llm/mock_llm.py" \
        --address "$E2E_FAKE_ADDR" --port 0 --chunks 10 --delay 30 \
        > "$m3_ready_file" 2> "$M3_LLM_LOG" &
    M3_LLM_PID=$!
    m3_ready=$(m3_wait_ready "$m3_ready_file" "$M3_LLM_PID" 20) ||
        m3_ready_failed "the mock language model" "$M3_LLM_PID" "$?" "$M3_LLM_LOG"
    M3_LLM_PORT="${m3_ready#READY http=}"
    # Die ganze Zeile, nicht nur ihr Anfang: Ohne diese Prüfung ginge der Lauf
    # mit einem leeren Port weiter, und jede Anfrage des Agenten endete an
    # einer Adresse, die niemandem gehört. Der Mock trägt dieses Milestone; er
    # soll hier auffallen und nicht dort.
    if [ "$m3_ready" = "$M3_LLM_PORT" ] || [ -z "$M3_LLM_PORT" ]; then
        e2e_die "the mock language model did not report a port (got \"$m3_ready\"): $(cat "$M3_LLM_LOG" 2> /dev/null)"
    fi
    M3_LLM_ENDPOINT="http://$E2E_FAKE_ADDR:$M3_LLM_PORT"
    e2e_say "mock language model on $M3_LLM_ENDPOINT (pid $M3_LLM_PID)"
}

# m3_start_upstream — das zweite Ziel starten, im Klartext und über TLS.
m3_start_upstream() {
    m3_ready_file="$E2E_WORKDIR/upstream.ready"
    rm -f "$m3_ready_file"
    : > "$m3_ready_file"
    python3 "$E2E_ROOT/tests/e2e/fake-upstream/fake_upstream.py" \
        --address "$E2E_FAKE_ADDR" \
        --http-port "$M3_HTTP_PORT" \
        --https-port "$M3_HTTPS_PORT" \
        --cert "$M3_CA_DIR/upstream.crt" \
        --key "$M3_CA_DIR/upstream.key" \
        > "$m3_ready_file" 2> "$M3_UPSTREAM_LOG" &
    E2E_FAKE_PID=$!
    m3_ready=$(m3_wait_ready "$m3_ready_file" "$E2E_FAKE_PID" 20) ||
        m3_ready_failed "the fake upstream" "$E2E_FAKE_PID" "$?" "$M3_UPSTREAM_LOG"
    if [ "$m3_ready" != "READY http=$M3_HTTP_PORT https=$M3_HTTPS_PORT" ]; then
        e2e_die "the fake upstream did not come up as asked (got \"$m3_ready\"): $(cat "$M3_UPSTREAM_LOG" 2> /dev/null)"
    fi
    e2e_say "fake upstream on $E2E_FAKE_ADDR ($m3_ready, pid $E2E_FAKE_PID)"
}

# m3_write_config — die Konfiguration dieses Laufs in den XDG-Baum legen.
m3_write_config() {
    mkdir -p "$E2E_WORKDIR/config/humanitl"
    sed -e "s|@UPSTREAM_ADDR@|$E2E_FAKE_ADDR|g" \
        -e "s|@TEST_CA@|$M3_CA_DIR/test-ca.crt|g" \
        -e "s|@LLM_ENDPOINT@|$M3_LLM_ENDPOINT|g" \
        "$M3_DIR/config.toml" > "$E2E_WORKDIR/config/humanitl/config.toml"
    e2e_say "config $E2E_WORKDIR/config/humanitl/config.toml"
}

# m3_wait_for_started SECONDS — auf die Startzeile der Sitzung warten, Id auf stdout.
#
# `humanitl -v run` schreibt sie, sobald der Daemon die Sandbox gestartet hat.
# Sie ist zugleich die Kennung, mit der später die Zusammenfassung abgerufen
# wird, und der Zeitpunkt, ab dem sich ein zweites Terminal anhängen kann.
m3_wait_for_started() {
    m3_started_left=$(( ${1:-20} * 10 ))
    while [ "$m3_started_left" -gt 0 ]; do
        m3_started_id=$(grep -o 'sandbox [0-9a-f-]\{36\} started' "$M3_AGENT_ERR" 2> /dev/null |
            head -n 1 | cut -d' ' -f2)
        if [ -n "$m3_started_id" ]; then
            printf '%s\n' "$m3_started_id"
            return 0
        fi
        sleep 0.1
        m3_started_left=$((m3_started_left - 1))
    done
    return 1
}

# m3_decide FILTER allow|block [NOTE] — auf einen wartenden Fluss warten und
# ihn entscheiden. Gibt den Pfad des Flusses auf stdout aus.
m3_decide() {
    m3_decide_id=$(wait_for_held 15 "$1") ||
        e2e_die "no request matching \"$1\" waited for a decision within fifteen seconds"
    m3_decide_path=$(humanitl --json flows show "$m3_decide_id" |
        jq -r '.path // ""' 2> /dev/null || true)
    if [ -n "${3:-}" ]; then
        flow_decide "$m3_decide_id" "$2" "$3" ||
            e2e_die "the daemon refused the $2 for $m3_decide_id"
    else
        flow_decide "$m3_decide_id" "$2" ||
            e2e_die "the daemon refused the $2 for $m3_decide_id"
    fi
    # Die Meldung geht nach stderr: stdout dieser Funktion ist der Pfad, den
    # der Aufrufer in einer Kommandosubstitution einsammelt.
    e2e_say "decided $2 for $m3_decide_id ($m3_decide_path)" >&2
    printf '%s\n' "$m3_decide_path"
}

# m3_opencode_in_sandbox — der Pfad von OpenCode, so wie die Sandbox ihn sähe.
#
# **Nicht `command -v opencode`.** Der Host sucht in `$PATH` des Entwicklers,
# und dort liegt das Binary oft unter `~/.local/bin`; die Sandbox hängt nur
# `/usr` ein und hat `PATH=/usr/local/bin:/usr/bin:/bin`
# (`profiles/sandbox/default.toml`). Ein Lauf, der `command -v` glaubte, hielte
# die Variante für fahrbar und scheiterte drinnen an `AGENT_004` — an etwas
# anderem also, als er messen wollte.
m3_opencode_in_sandbox() {
    for m3_oc_dir in /usr/local/bin /usr/bin /bin; do
        if [ -x "$m3_oc_dir/opencode" ]; then
            printf '%s/opencode\n' "$m3_oc_dir"
            return 0
        fi
    done
    return 1
}

# --- Zertifikat, Ziele, Daemon -----------------------------------------------

e2e_step "the run brings its own certificate authority, its own two targets and its own daemon"

# shellcheck disable=SC2086 # M3_HOSTS ist absichtlich eine Wortliste.
sh "$E2E_ROOT/tests/e2e/fake-upstream/gen-test-ca.sh" "$M3_CA_DIR" $M3_HOSTS ||
    e2e_die "openssl could not create the test certificate"
e2e_say "test CA in $M3_CA_DIR, valid for $M3_HOSTS"

m3_start_llm
m3_start_upstream
m3_write_config
# `--allow-test-ca` ist das, was den `https`-Verkehr dieses Laufs möglich macht
# (HUM-087). Es steht hier im Startbefehl und nicht in einer Umgebungsvariablen:
# Ein Flag, das das Vertrauen des Daemons erweitert, soll an genau einer
# sichtbaren Stelle stehen.
start_daemon "$E2E_WORKDIR/state" "$E2E_WORKDIR" "$M3_HOLD_TIMEOUT" --allow-test-ca

# Die beiden Belege, dass die Ziele antworten, bevor irgendwo behauptet wird,
# eine Anfrage sei nicht bei ihnen angekommen.
llm_probe=$(curl -sS --max-time 5 --noproxy '*' "$M3_LLM_ENDPOINT/api/tags" || true)
e2e_expect "the language model answers on the host of the namespace" "mock:latest" \
    "$(printf '%s' "$llm_probe" | jq -r '.models[0].name // ""' 2> /dev/null || true)"

reachable=$(curl -sS --max-time 5 --noproxy '*' \
    "http://$E2E_FAKE_ADDR:$M3_HTTP_PORT/reachable" || true)
e2e_expect "the second target answers on the host of the namespace" /reachable \
    "$(printf '%s' "$reachable" | jq -r '.path // ""' 2> /dev/null || true)"

info=$(daemon_info)
e2e_expect_match "the daemon serves GetInfo and runs a proxy session" \
    '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$' \
    "$(printf '%s' "$info" | jq -r '.session_id // ""')"

# Die Konfiguration dieses Laufs, mit ihren Werten und nicht nur ihren
# Schlüsseln. Scheiterte die Ersetzung der Platzhalter still, stünde
# `@UPSTREAM_ADDR@` in der Datei, und die Schritte darunter belegten etwas
# anderes, als sie sagen.
overrides=$(humanitl --json config get resolver.overrides |
    jq -r '.value | to_entries | map("\(.key)=\(.value)") | join(" ")')
e2e_expect "the resolver override points the second host at the target" \
    "example.com=$E2E_FAKE_ADDR" "$overrides"

configured_ca=$(humanitl --json config get resolver.test_ca | jq -r '.value')
e2e_expect "the configuration names the test CA of this run" \
    "$M3_CA_DIR/test-ca.crt" "$configured_ca"

if [ -s "$configured_ca" ] &&
    head -n 1 "$configured_ca" | grep -q '^-----BEGIN CERTIFICATE-----$'; then
    e2e_check "and that file really is a certificate" ok
else
    e2e_check "and that file really is a certificate" no \
        "$configured_ca is missing, empty or not PEM"
fi

# Der Stolperdraht zu `https`, in derselben umgedrehten Form wie im M2-Lauf
# (HUM-087, `backlog/CONVENTIONS.md` 4.22): Er stirbt, wenn `humanitld` das
# Flag **nicht mehr** kennt, und sagt, was dann zurückzudrehen wäre.
if "$E2E_DAEMON" --help 2>&1 | grep -q -- '--allow-test-ca'; then
    e2e_check "humanitld offers --allow-test-ca, the https leg rests on it" ok
else
    e2e_check "humanitld offers --allow-test-ca, the https leg rests on it" no \
        "humanitld no longer knows --allow-test-ca. Either it was removed, then STEP3 to STEP5 of the agent script have to go back to http:// and this run has to drop resolver.test_ca (see backlog/CONVENTIONS.md 4.22 and 4.29), or it was renamed, then this run has to start the daemon with the new name."
fi

# Und dass dieser Lauf es auch übergeben hat. `DAEMON_ARGV` kommt aus
# `start_daemon` und ist das, was wirklich an `humanitld` ging.
e2e_expect_match "and this run started the daemon with it" \
    '(^| )--allow-test-ca( |$)' "$DAEMON_ARGV"

# Und der Daemon sagt es auch selbst, statt dass der Lauf es aus einem
# Ausbleiben schließt.
daemon_trust_line=$(grep -F -- '--allow-test-ca' "$DAEMON_LOG" | head -n 1)
e2e_expect_match "and the daemon says on its own that it loaded the root" \
    '"roots":1' "$daemon_trust_line"
e2e_expect_match "and names the file it loaded it from" \
    "\"path\":\"$M3_CA_DIR/test-ca.crt\"" "$daemon_trust_line"
e2e_expect_match "and says it at the level a widened trust deserves" \
    '"level":"WARN"' "$daemon_trust_line"

if grep -q -E 'CONFIG_01[01]' "$DAEMON_LOG"; then
    e2e_check "and the daemon had nothing to complain about the pair" no \
        "$(grep -E -m 1 'CONFIG_01[01]' "$DAEMON_LOG")"
fi
e2e_check "and the daemon had nothing to complain about the pair" ok

# Und die beiden Schlüssel, an denen die Durchreiche hängt.
e2e_expect "the configuration names the mock as the language model of this session" \
    "$M3_LLM_ENDPOINT/" "$(humanitl --json config get llm.endpoint | jq -r '.value')"
e2e_expect "and the sandbox environment carries its address for the agent" \
    "$M3_LLM_ENDPOINT" \
    "$(humanitl --json config get sandbox.env | jq -r '.value.LLM // ""')"

# --- 1. Die Regeln dieser Sitzung --------------------------------------------

e2e_step "1. the rule set of this session: the passthrough first, the bundled blocks behind it"

rules_json=$(humanitl --json rules list --all)
passthrough=$(printf '%s' "$rules_json" |
    jq -c '.rules[] | select(.rule_id == "01920000-0000-7000-8000-0000000000ff")')
[ -n "$passthrough" ] ||
    e2e_die "the adapter built no passthrough rule from llm.endpoint"

e2e_expect "the passthrough to the language model is evaluated first" 1 \
    "$(printf '%s' "$passthrough" | jq -r '.position')"
e2e_expect "and it names the address of this run's model" "$E2E_FAKE_ADDR" \
    "$(printf '%s' "$passthrough" | jq -r '.host')"
e2e_expect "and its port" "$M3_LLM_PORT" \
    "$(printf '%s' "$passthrough" | jq -r '.port')"
# Der Vermerk, der die Durchreiche von jeder anderen Regel unterscheidet: Sie
# darf ein Ziel im eigenen Netz erreichen, weil ein lokales Sprachmodell dort
# steht. Keine Regel eines Menschen und keine aus einer Datei bekommt ihn
# (HUM-104).
e2e_expect "and it may reach a private address, which no other rule may" true \
    "$(printf '%s' "$passthrough" | jq -r '.allow_private')"
e2e_expect "and it comes from the adapter, not from a file" bundled \
    "$(printf '%s' "$passthrough" | jq -r '.origin')"

catalog_rule=$(printf '%s' "$rules_json" |
    jq -c '.rules[] | select(.rule_id == "01920000-0000-7000-8000-000000000001")')
e2e_expect "the bundled rule for the model catalog is in force" false \
    "$(printf '%s' "$catalog_rule" | jq -r '.disabled')"

# --- 2. Die Sitzung ----------------------------------------------------------

e2e_step "2. humanitl run starts the session in the project directory"

cp "$M3_DIR/agent_script.sh" "$M3_PROJECT/agent_script.sh"
chmod +x "$M3_PROJECT/agent_script.sh"

# `humanitl run` ohne `--work`: Das Projekt ist das Verzeichnis, in dem der
# Befehl steht, genau wie beim Nutzer (HUM-067). `-v` schaltet die Zeilen des
# Daemons frei — die Startzeile der Sandbox, die drei Garantien und die
# Befunde; `stdout` bleibt dem Agenten vorbehalten.
(
    cd "$M3_PROJECT" &&
        humanitl -v run -- /bin/sh /work/agent_script.sh \
            > "$M3_AGENT_OUT" 2> "$M3_AGENT_ERR"
) &
M3_RUN_PID=$!

M3_SANDBOX_ID=$(m3_wait_for_started 25) ||
    e2e_die "the daemon did not report a started sandbox within twenty-five seconds; its log: $(tail -n 20 "$M3_AGENT_ERR" 2> /dev/null)"
e2e_say "sandbox $M3_SANDBOX_ID"

# Ein zweites Terminal an derselben Sitzung. Es schreibt nichts (`--read-only`)
# und nimmt dem Agenten damit den Schreiberplatz nicht weg; was es sieht, sind
# die gefilterten Bytes des Agenten und die Hinweiszeilen des Daemons dazwischen
# (HUM-042). Der Rückstand aus dem Ring kommt beim Anhängen mit, also fehlt
# nichts, was vor diesem Moment lag.
humanitl sandbox attach --read-only > "$M3_ATTACH_OUT" 2> "$M3_ATTACH_ERR" &
M3_ATTACH_PID=$!

# --- 3. Die drei Entscheidungen eines Menschen -------------------------------

e2e_step "3. a human decides the three requests that wait"

decided_docs=$(m3_decide 'path:/docs' allow)
e2e_expect "the request a human released is the one the agent asked for" /docs \
    "$decided_docs"

# Der gefälschte Pfad lässt sich nicht als Filter schreiben — er trägt eine
# eckige Klammer, und die Grammatik der Liste kennt keine Maskierung. Gefragt
# wird deshalb nach dem Host: Die Anfrage davor ist entschieden, also wartet
# genau diese.
decided_forged=$(m3_decide 'host:example.com' allow)
e2e_expect "and the next one carries the path that tries to forge a notice" \
    "$M3_PATH_FORGED" "$decided_forged"

decided_secret=$(m3_decide 'path:/secret' block "$M3_NOTE")
e2e_expect "and the one a human forbade is the third" /secret "$decided_secret"

# --- Warten, bis der Agent fertig ist ----------------------------------------

e2e_step "the agent runs to its end, and the terminal detaches by itself"

# Der Ausgang ist ein Messwert, kein Kommentar. `humanitl run` reicht den
# Exit-Code des Agenten durch (`run.rs`, `drive`), und dieses Agentenskript
# endet auf 0; jede andere Zahl heißt, dass etwas anderes passiert ist als
# geprüft wird — eine fehlende Voraussetzung in der Sandbox (97), eine rote
# Garantie (3), kein Daemon (2). Ein Lauf, der das nur ins Protokoll schreibt
# und trotzdem grün meldet, besteht, indem er weniger prüft.
# `|| status=$?` statt eines nackten `wait`: Unter `set -e` beendete ein
# `wait`, das einen Fehlschlag meldet, das Skript, bevor die Zahl gelesen wäre
# — und der Lauf endete ohne die Zeile, die sagt, was los war.
m3_run_status=0
wait "$M3_RUN_PID" || m3_run_status=$?
M3_RUN_PID=""
e2e_expect "humanitl run exited cleanly" 0 "$m3_run_status"
wait "$M3_ATTACH_PID" 2> /dev/null || true
M3_ATTACH_PID=""

e2e_expect "the agent ran to its last line" "STEP8 done" "$(m3_agent_line '^STEP8 done')"

# Die drei Garantien, gemessen in der Sandbox, die den Verkehr dieses Laufs
# getragen hat, und vom Daemon als `SandboxEvent.check` geschickt (HUM-041).
# Der Lauf liest die Zeilen ausdrücklich und glaubt nicht dem Exit-Code allein:
# M3 ist neben M2 der einzige Lauf, in dem die Sandbox echten Verkehr trägt,
# und ein Bericht, in den niemand sieht, ist kein Beleg.
isolation=$(cat "$M3_AGENT_ERR")
e2e_expect_match "the sandbox that carried this run had no interface but lo" \
    '^\[ok  \] no network interface ' "$isolation"
e2e_expect_match "and exactly one socket, and it was the proxy" \
    '^\[ok  \] one door .*proxy\.sock' "$isolation"
e2e_expect_match "and seccomp was active in the agent process" \
    '^\[ok  \] seccomp active ' "$isolation"
e2e_expect_match "and the daemon named the sandbox it started" \
    "sandbox $M3_SANDBOX_ID started" "$isolation"
# Und es hat gesagt, dass die Durchreiche an der Warteschlange vorbeigeht. Der
# Befund ist kein Fehler, sondern die Ansage des einen Kanals, den ein Mensch
# nicht zu sehen bekommt (BACKLOG.md 4.2, `docs/SECURITY.md` 3.1).
e2e_expect_match "and warned that the language model is not on a private network" \
    'LLM_006' "$isolation"

# --- 4. Die Durchreiche zum Sprachmodell -------------------------------------

e2e_step "4. the passthrough to the language model, from three sources"

e2e_expect "the agent got all ten token frames" 10 "$(m3_agent_value '^STEP1 frames=')"
e2e_expect_match "the first of them is tok0" 'tok0 ' "$(m3_agent_line 'tok0 ')"
e2e_expect_match "the last of them is tok9" 'tok9 ' "$(m3_agent_line 'tok9 ')"
e2e_expect_match "and the stream ended with the done frame" \
    '^data: \[DONE\]$' "$(m3_agent_line '^data: \[DONE\]')"

# Und sie kamen verteilt über die Zeit. Zehn Rahmen im Abstand von 30 ms sind
# mindestens 0,27 s zwischen dem ersten und dem letzten Byte; verlangt werden
# 0,15 s, damit ein langsamer Läufer nicht an der Genauigkeit scheitert, und
# ein Proxy, der den Strom sammelte und erst am Ende weitergäbe, käme auf
# nahezu null. Das ist die eine Zusicherung, die „gestreamt, nicht gehalten"
# von „durchgekommen" unterscheidet.
timing=$(m3_agent_line '^STEP1 timing ')
e2e_expect "and they arrived spread over time, not in one piece" yes \
    "$(printf '%s\n' "$timing" | awk -F'[= ]' '{ ttfb = $4; total = $6 }
        END { print (total - ttfb >= 0.15) ? "yes" : "no (" total - ttfb "s)" }')"

# Die zweite Quelle: was der Mock empfangen hat.
e2e_expect "the language model served exactly one inference request" 1 \
    "$(m3_llm_hits '^mock-llm: POST /v1/chat/completions 200 ')"
e2e_expect "and served it as a stream, not as one piece" 1 \
    "$(m3_llm_hits ' stream$')"
last_body=$(curl -sS --max-time 5 --noproxy '*' "$M3_LLM_ENDPOINT/_debug/last" || true)
e2e_expect_match "and the body it received is the one the agent sent" \
    'hello from the sandbox' \
    "$(printf '%s' "$last_body" | jq -r '.body // ""' 2> /dev/null || true)"

# Die dritte: die Liste. Ein Durchreich-Fluss ist dort unsichtbar, solange
# niemand `include_passthrough` verlangt — und die Kommandozeile kann es nicht
# (siehe den Stolperdraht in Schritt 11). Die Null hängt deshalb an einer
# positiven Zahl aus derselben Quelle, sonst wäre sie auch dann grün, wenn gar
# nichts aufgezeichnet worden wäre (`backlog/CONVENTIONS.md` 4.22).
humanitl --json flows list --asc > "$M3_FLOWS_FIRST_JSON" 2> /dev/null || true
e2e_expect "the passthrough flow stays out of the list the command line shows" 0 \
    "$(m3_count "host:$E2E_FAKE_ADDR")"
# Acht sichtbare Flüsse: der Katalog aus Schritt 5, die vier ungefragten
# Abrufe des Starts aus Schritt 5b und die drei, über die ein Mensch
# entscheidet. Die Durchreiche ist keiner davon.
e2e_expect "while the eight flows the agent really made are in it" 8 "$(m3_count '')"
e2e_expect "three of them the moderated path" 3 "$(m3_count 'host:example.com')"

# --- 5. Die mitgelieferte Regel ----------------------------------------------

e2e_step "5. the bundled rule blocks the model catalogue before any name is resolved"

e2e_expect "the agent got 403 for the catalogue" "STEP2 status=403" \
    "$(m3_agent_line '^STEP2 status=')"
e2e_expect "the flow was blocked" block "$(m3_field 'host:models.dev' decision)"
e2e_expect "by the bundled rule of the adapter" \
    01920000-0000-7000-8000-000000000001 "$(m3_field 'host:models.dev' rule_id)"
# Kein Eintrag in `resolver.overrides`, und trotzdem kein Auflösungsfehler: Der
# Block fiel, bevor der Name überhaupt zu einer Adresse werden musste (ADR-006).
e2e_expect "and it never got as far as resolving the name" "" \
    "$(m3_field 'host:models.dev' error)"
e2e_expect "and the target never saw the request" 0 "$(m3_upstream_hits '/api.json')"
e2e_expect "while it did serve the two a human released" 2 \
    "$(m3_upstream_hits '^fake-upstream: https example.com GET ')"

# --- 5b. Das Rauschbudget des ersten Starts ----------------------------------

e2e_step "5b. nothing an agent asks for on its own reaches a human"

# HUM-038: Beim ersten Start darf der Mensch keine Flut ungefragter Anfragen
# sehen. Der Agent hat gerade vier Adressen abgerufen, die ein frisch
# gestartetes OpenCode von sich aus abruft — Release-Check, Telemetrie,
# Modellkatalog, Freigabe-Seite —, dazu den Katalog aus Schritt 5. Jede davon
# entscheidet eine mitgelieferte Regel; keine davon wartet auf einen Menschen.
#
# Gemessen wird beides: dass die Regeln greifen (fünf blockierte Flüsse mit
# Regel-Id) und dass die Warteschlange leer ist. Die zweite Hälfte allein wäre
# grün, wenn der Agent gar nichts versucht hätte.
# Gefragt wird nach dem **Pfad** und geprüft wird die **Regel-Id**, nicht der
# Host und nicht die Entscheidung. Beides aus dem Review von Antigravity: Ein
# Filter `host:opencode.ai` trifft auch `models.opencode.ai` — die Historie
# vergleicht Hosts als Suffix —, und `decision: block` sagt nicht, wer
# entschieden hat; ein Mensch blockt genauso. Mit der Id steht da, welche
# mitgelieferte Regel gegriffen hat, und die Prüfung fällt, sobald eine davon
# fehlt.
for m3_noise in \
    'api.github.com|/repos/anomalyco/opencode/releases/latest|01920000-0000-7000-8000-000000000002' \
    'eu.posthog.com|/i/v0/e|01920000-0000-7000-8000-000000000003' \
    'models.opencode.ai|/api.json|01920000-0000-7000-8000-000000000009' \
    'opencode.ai|/share/abc|01920000-0000-7000-8000-00000000000a'; do
    # Host **und** Pfad: `/api.json` allein trifft auch den Katalog aus
    # Schritt 5, und `host:opencode.ai` allein träfe `models.opencode.ai` mit.
    # Erst beide zusammen benennen genau einen Fluss.
    m3_noise_host=${m3_noise%%|*}
    m3_noise_rest=${m3_noise#*|}
    m3_noise_path=${m3_noise_rest%%|*}
    m3_noise_rule=${m3_noise_rest#*|}
    e2e_expect "the agent got 403 for $m3_noise_host" "403" \
        "$(m3_agent_line "^STEP2B https://$m3_noise_host" | sed 's/.*status=//')"
    e2e_expect "and the bundled rule of that host decided it, not a human" \
        "$m3_noise_rule" \
        "$(m3_field "host:$m3_noise_host path:$m3_noise_path" rule_id)"
done

# Die Zahl, um die es geht: Was ein Agent beim Start von sich aus tut, erreicht
# keinen Menschen. Gemessen an den Hinweiszeilen, die der Daemon in das
# Terminal schreibt — sie entstehen in dem Augenblick, in dem eine Anfrage
# wartet, und überleben deshalb das Ende der Sitzung. Eine Zählung von
# `state:held` nach der Sitzung sagte nur, dass gerade nichts mehr wartet
# (Review Antigravity).
e2e_expect "and not one of them ever waited for a human" 0 \
    "$(m3_notice_count '^\[humanitl\] request held: .*\(github\|posthog\|opencode\)')"
e2e_expect "and nothing is left in the queue either" 0 "$(m3_count 'state:held')"

# --- 6. Die Entscheidung eines Menschen erreicht den Agenten -----------------

e2e_step "6. the released request comes back with the content of the target"

e2e_expect "the agent got 200" "STEP3 status=200" "$(m3_agent_line '^STEP3 status=')"
# Der Status allein sagt zu wenig: Eine `200` könnte auch vom Proxy selbst
# kommen oder vom Meta-Endpunkt. Geprüft wird deshalb der Rumpf, den der Agent
# gesehen hat.
fetch_body=$(m3_agent_line '"path": "/docs"')
e2e_expect "and the body is the answer of the target, not of anyone else" /docs \
    "$(printf '%s' "$fetch_body" | jq -r '.path // ""' 2> /dev/null || true)"
e2e_expect "and the target saw the host the agent asked for" example.com \
    "$(printf '%s' "$fetch_body" | jq -r '.host // ""' 2> /dev/null || true)"
e2e_expect "the flow was allowed" allow "$(m3_field 'path:/docs' decision)"
e2e_expect "by a human, without a rule behind it" "" "$(m3_field 'path:/docs' rule_id)"
e2e_expect "and it went to the TLS port of the target" "$M3_HTTPS_PORT" \
    "$(m3_row 'path:/docs' | jq -r '.authority.port // 0')"
e2e_expect "and it left no upstream error" "" "$(m3_field 'path:/docs' error)"

# --- 7. Die Hinweiszeilen im angehängten Terminal ----------------------------

e2e_step "7. the attached terminal sees the daemon's notices, and a forged one loses its bracket"

e2e_expect "the attached terminal saw the request wait" 1 \
    "$(m3_notice_count '^\[humanitl\] request held: GET example\.com/docs · waiting for you$')"
e2e_expect "and saw it released" 1 \
    "$(m3_notice_count '^\[humanitl\] request allowed: GET example\.com/docs$')"
e2e_expect "and saw the catalogue blocked by the rule" 1 \
    "$(m3_notice_count '^\[humanitl\] request blocked: GET models\.dev/api\.json$')"
e2e_expect "and saw the last one blocked by a human" 1 \
    "$(m3_notice_count '^\[humanitl\] request blocked: GET example\.com/secret$')"

# Die Fälschung. Der Agent hat `[humanitl]` in seinen eigenen Pfad geschrieben;
# `path_for_notice` macht aus der eckigen Klammer eine runde, damit die Zeile
# nur einen Absender trägt. Beide Hälften werden gemessen: die Zeile, die der
# Mensch liest, und der Fluss, der weiterhin den Pfad trägt, den der Agent
# wirklich geschickt hat.
e2e_expect "the forged notice lost the bracket that belongs to the sender" 1 \
    "$(m3_notice_count "^\\[humanitl\\] request held: GET example\\.com/a$M3_NOTICE_FORGED · waiting for you\$")"
# Und deshalb trägt keine Zeile zwei Absender. Das ist die eigentliche
# Behauptung: Nicht dass die Klammer fehlt, sondern dass niemand außer dem
# Daemon eine Hinweiszeile eröffnen kann.
e2e_expect "and no line in the terminal carries a second sender" 0 \
    "$(m3_notice_count '\[humanitl\].*\[humanitl\]')"
# Und die andere Hälfte desselben Paares: In der Aufzeichnung steht der Pfad
# unverändert, mit der eckigen Klammer. Gesäubert wird die Anzeige, nicht der
# Fluss — wer später wissen will, was der Agent wirklich geschickt hat, findet
# es. Gefragt wird über `jq` und nicht über einen Filter: Die Grammatik der
# Liste kennt keine Maskierung für eine eckige Klammer.
e2e_expect "while the history carries the path the agent really sent, bracket and all" 1 \
    "$(jq -r --arg path "$M3_PATH_FORGED" \
        '[.flows[] | select(.path == $path)] | length' "$M3_FLOWS_FIRST_JSON")"
# Und die dritte Seite derselben Anfrage: was der Agent selbst gesehen hat. Der
# gefälschte Pfad ändert nichts an der Entscheidung eines Menschen — die
# Freigabe gilt, und die Antwort kommt an. Ohne diese Zeile prüfte der Schritt
# nur die Anzeige und die Aufzeichnung und nicht das Ende des Wegs.
e2e_expect "and the agent got the answer for it" "STEP4 status=200" \
    "$(m3_agent_line '^STEP4 status=')"

# Und genau drei Anfragen haben gewartet: die drei, über die ein Mensch
# entschieden hat. Gezählt wird `request held`, nicht die Gesamtzahl der
# Hinweiszeilen — die Ansage einer Entscheidung, die fällt, **bevor** der
# Zuhörer der Warteschlange angemeldet ist, geht verloren (siehe den Kopf des
# Agentenskripts), und eine Zahl, die davon abhinge, wäre auf einem langsamen
# Läufer mal sieben und mal acht. Eine wartende Anfrage kann nicht zu früh
# kommen: Sie wartet, bis jemand entscheidet.
e2e_expect "exactly three requests waited for a human" 3 \
    "$(m3_notice_count '^\[humanitl\] request held: ')"
# Und keine davon war die Durchreiche: Sie wird nie gehalten. Die Null hängt an
# den drei darüber (`backlog/CONVENTIONS.md` 4.22).
e2e_expect "and the language model was never one of them" 0 \
    "$(m3_notice_count "^\\[humanitl\\] request held: .*$E2E_FAKE_ADDR")"

# --- 8. Die Notiz eines Menschen ---------------------------------------------

e2e_step "8. the note of the human reaches the agent as exactly one header line"

e2e_expect "the agent got 403 for the request a human forbade" "STEP5 status=403" \
    "$(m3_agent_line '^STEP5 status=')"
e2e_expect "the note reaches him as one header line, cleaned" \
    "X-Humanitl-Note: $M3_NOTE_CLEAN" "$(m3_agent_value '^STEP5 note-header=')"
e2e_expect "and the second header line it tried to open is not a header" 0 \
    "$(m3_agent_value '^STEP5 injected-headers=')"
e2e_expect_match "and the body names the human as the reason" \
    '^reason: user$' "$(m3_agent_line '^reason: user')"
e2e_expect_match "and carries the same note" \
    "^note: $M3_NOTE_CLEAN\$" "$(m3_agent_line '^note: ')"

# --- 9. Der Filter des Terminals ---------------------------------------------

e2e_step "9. what the agent writes into the transcript is filtered, and colour survives"

# Das Transkript wird als CI-Artefakt im Browser geöffnet und im Terminal eines
# Menschen gelesen; es ist einer der fünf erklärten Seitenkanäle (BACKLOG.md
# 4.2). Die beiden Zusicherungen sind ein Paar: Eine Datei ohne `ESC ]` wäre
# auch dann grün, wenn der Agent nie geschrieben hätte.
if m3_has_escape "$M3_AGENT_OUT"; then
    e2e_check "the clipboard sequence of the agent did not reach the transcript" no \
        "an OSC introducer (ESC ]) is in $M3_AGENT_OUT"
else
    e2e_check "the clipboard sequence of the agent did not reach the transcript" ok
fi
e2e_expect_match "while its colour sequence did" \
    "$(printf 'colour<\033\\[31mred')" "$(m3_agent_line '^STEP6 filter ')"
if m3_has_escape "$M3_ATTACH_OUT"; then
    e2e_check "and the attached terminal is filtered the same way" no \
        "an OSC introducer (ESC ]) is in $M3_ATTACH_OUT"
else
    e2e_check "and the attached terminal is filtered the same way" ok
fi
# Und derselbe positive Anker wie eine Zeile darüber, aus derselben Datei: Ein
# Anhang, der gar nichts geliefert hätte, bestünde die Prüfung auf ein
# fehlendes `ESC ]` sonst mühelos.
e2e_expect_match "while its colour sequence reached the attached terminal too" \
    "$(printf 'colour<\033\\[31mred')" \
    "$(tr -d '\r' < "$M3_ATTACH_OUT" | grep -m 1 '^STEP6 filter ' || true)"

# --- 10. Was der Lauf im Projekt hinterlassen hat ----------------------------

e2e_step "10. the session summary names the one file the agent wrote"

# Was der Agent geschrieben zu haben behauptet, in Bytes. Die Zahl steht hier
# und nicht nur „irgendetwas": Sie ist das Gegenstück zu der, die der Daemon
# gemessen hat, und erst beide zusammen sagen, dass über dieselbe Datei
# gesprochen wird.
e2e_expect "the agent wrote its file" "STEP7 wrote=38" "$(m3_agent_line '^STEP7 wrote=')"
humanitl --json sessions summary "$M3_SANDBOX_ID" > "$M3_SUMMARY_JSON" ||
    e2e_die "the daemon has no summary for sandbox $M3_SANDBOX_ID"
e2e_expect "the summary of the session lists exactly one change" 1 \
    "$(jq -r '.changes | length' "$M3_SUMMARY_JSON")"
e2e_expect "and it is the file the agent wrote" notes.txt \
    "$(jq -r '.changes[0].path' "$M3_SUMMARY_JSON")"
e2e_expect "with the size the agent measured itself" 38 \
    "$(jq -r '.changes[0].size' "$M3_SUMMARY_JSON")"
e2e_expect "and the summary names the project directory of this session" \
    "$M3_PROJECT" "$(jq -r '.work_dir' "$M3_SUMMARY_JSON")"
# Und die Datei liegt wirklich dort, nicht nur in einem Bericht darüber.
if [ -s "$M3_PROJECT/notes.txt" ]; then
    e2e_check "and the file really is in the project on the host" ok
else
    e2e_check "and the file really is in the project on the host" no \
        "$M3_PROJECT/notes.txt is missing or empty"
fi

# --- 11. Die Historie und die beiden Stolperdrähte ---------------------------

e2e_step "11. the history holds every flow the agent made, in the order they arrived"

e2e_expect "the history holds the eight flows of this session" 8 "$(m3_count '')"
# Die Reihenfolge ist die des Agenten: erst der Katalog, dann die vier
# ungefragten Abrufe des Starts, dann die drei, über die ein Mensch entscheidet.
e2e_expect "in the order the agent asked for them" \
    "/api.json /repos/anomalyco/opencode/releases/latest /i/v0/e /api.json /share/abc /docs $M3_PATH_FORGED /secret" \
    "$(jq -r '[.flows[].path] | join(" ")' "$M3_FLOWS_FIRST_JSON")"

# Zwei Stolperdrähte bleiben stehen, weil das, worauf sie warten, noch fehlt.
# Beide sind so geschrieben, dass sie rot werden, sobald es da ist — eine
# Prüfung, die nach der Reparatur ersatzlos verschwände, hinterließe eine Lücke
# genau dort, wo vorher eine Zusicherung stand (`backlog/CONVENTIONS.md` 4.22).
audit_says=$("$E2E_CLI" audit 2>&1 || true)
if printf '%s' "$audit_says" | grep -q 'HUM-070'; then
    e2e_check "humanitl audit is still a placeholder, so this run proves nothing about the chain" ok
else
    e2e_check "humanitl audit is still a placeholder, so this run proves nothing about the chain" no \
        "humanitl audit no longer refers to HUM-070. The audit chain exists now, so this run has to verify it: export the chain of this session and check it end to end (step 8 of backlog/sprint-3.md HUM-046)."
fi

if "$E2E_CLI" flows list --help 2>&1 | grep -q -- '--include-passthrough'; then
    e2e_check "humanitl flows list still hides the passthrough with no way to ask for it" no \
        "humanitl flows list has a switch for include_passthrough now. Step 4 of this run has to stop measuring the passthrough by its absence and assert the flow itself instead (backlog/CONVENTIONS.md 4.29)."
else
    e2e_check "humanitl flows list still hides the passthrough with no way to ask for it" ok
fi

# --- 12. Eine zweite Sitzung, ohne wartenden Menschen ------------------------

e2e_step "12. humanitl run --profile llm-only lets inference through and blocks the rest"

cp "$M3_DIR/agent_script_llm_only.sh" "$M3_PROJECT/agent_script_llm_only.sh"
chmod +x "$M3_PROJECT/agent_script_llm_only.sh"

# Auch hier ist der Ausgang ein Messwert. Diese Sitzung wird von niemandem
# entschieden, ihr Agent endet auf 0, und `humanitl run` reicht das durch:
# Alles andere heißt, dass das Profil, die Regel oder die Sandbox nicht getan
# hat, was der Schritt behauptet.
m3_only_status=0
(
    cd "$M3_PROJECT" &&
        humanitl -v run --profile llm-only -- /bin/sh /work/agent_script_llm_only.sh \
            > "$M3_ONLY_OUT" 2> "$M3_ONLY_ERR"
) || m3_only_status=$?
e2e_expect "the llm-only session exited cleanly" 0 "$m3_only_status"

only_out=$(tr -d '\r' < "$M3_ONLY_OUT")
e2e_expect_match "the second session ran to its end" '^LLM3 done$' "$only_out"
e2e_expect_match "the agent got its ten frames from the language model" \
    '^LLM1 frames=10$' "$only_out"
e2e_expect_match "and 403 for everything else" '^LLM2 status=403$' "$only_out"
# `403` und nicht `504`: Die Regel `block host "**"` des Profils hat
# entschieden, nicht eine abgelaufene Frist. Der Unterschied steht so auch im
# Briefing des Agenten (HUM-071) und ist der Beleg, dass die Regeln des Profils
# verdrahtet sind (HUM-066, HUM-067).
e2e_expect_match "and a rule decided it, not an expired deadline" \
    '^LLM2 reason=reason: rule$' "$only_out"
e2e_expect "the history grew by exactly one visible flow" 9 "$(m3_count '')"
# Und die Zahl allein sagte nur, dass eine Zeile dazukam. Woher sie kam, sagen
# die beiden Gründe: Sechs der neun sind Blocks einer Regel — der Modellkatalog
# und die vier ungefragten Abrufe der ersten Sitzung, dazu dieser hier —, und
# genau einer ist der Block eines Menschen. Niemand hat in dieser Sitzung
# entschieden, und trotzdem ist der Agent nicht in eine Frist gelaufen.
e2e_expect "six of the nine were blocked by a rule" 6 "$(m3_count 'reason:rule')"
e2e_expect "and exactly one by a human" 1 "$(m3_count 'reason:user')"
e2e_expect "and the language model served a second inference request" 2 \
    "$(m3_llm_hits '^mock-llm: POST /v1/chat/completions 200 ')"

# --- 13. Derselbe Lauf mit echtem OpenCode -----------------------------------

e2e_step "13. the same session with the real OpenCode, if the sandbox can see it"

m3_before_opencode="$E2E_ASSERTIONS"
m3_opencode_path="$(m3_opencode_in_sandbox || true)"
m3_opencode_wanted="${M3_OPENCODE:-auto}"

if [ "$m3_opencode_wanted" = 0 ]; then
    m3_skip "the OpenCode variant is switched off for this run (M3_OPENCODE=0); nothing about a real agent was verified"
elif [ -z "$m3_opencode_path" ]; then
    if [ "$m3_opencode_wanted" = 1 ]; then
        e2e_die "M3_OPENCODE=1 was asked for, but no opencode binary lies under /usr/local/bin, /usr/bin or /bin, which is the whole PATH of the sandbox. Install it there (sudo install -m 0755 \"\$(command -v opencode)\" /usr/local/bin/opencode) or drop M3_OPENCODE=1."
    fi
    m3_skip "no opencode under /usr/local/bin, /usr/bin or /bin, the whole PATH of the sandbox, so the real-agent variant did not run; nothing about OpenCode was verified. Install it there or set M3_OPENCODE=1 to make this a failure"
else
    e2e_say "opencode at $m3_opencode_path"

    # **Die Zähler von vorher.** Alles, was dieser Zweig über den echten Agenten
    # behauptet, ist ein Zuwachs und keine Gesamtzahl. Zu diesem Zeitpunkt hat
    # der Mock längst die Startprobe, die Inferenz des Skript-Agenten,
    # `/_debug/last` und die Inferenz der llm-only-Sitzung bedient, und
    # `models.dev` steht mit einem geblockten Fluss in der Historie. Eine
    # Zusicherung über die Summe wäre hier schon vor dem Start wahr — sie
    # bestünde auch dann, wenn OpenCode das Modell nie anspräche und den
    # Katalog doch abriefe. Deshalb: erst messen, dann starten, dann die
    # Differenz prüfen.
    m3_oc_llm_before=$(m3_llm_hits '^mock-llm: ')
    m3_oc_inference_before=$(m3_llm_hits '^mock-llm: POST /v1/chat/completions ')
    m3_oc_dev_before=$(m3_count 'host:models.dev')
    m3_oc_ai_before=$(m3_count 'host:models.opencode.ai')

    m3_oc_status=0
    (
        cd "$M3_PROJECT" &&
            humanitl -v run --ask none -- "$m3_opencode_path" run 'say the word humanitl' \
                > "$M3_OC_OUT" 2> "$M3_OC_ERR"
    ) || m3_oc_status=$?

    # **Der einzige Ausgang, den dieser Zweig nicht auf 0 festnagelt, und der
    # Grund dafür.** `humanitl run` reicht den Exit-Code des Agenten durch, und
    # ein echtes OpenCode gegen ein Modell, das `tok0 ` sagt, darf sich
    # beschweren; ein `0` zu verlangen hieße, eine Aussage über OpenCode zu
    # treffen statt über Humanitl. Was hier **nicht** stehen darf, sind die
    # drei Codes, mit denen Humanitl selbst abgelehnt hat: 2 kein Daemon, 3
    # eine rote Garantie, 4 eine Sicherheitsverletzung
    # (`humanitl --help`, Exit codes). 1 bleibt zugelassen, weil es zugleich
    # der Fehlercode der Kommandozeile und ein möglicher Ausgang des Agenten
    # ist — die beiden sind über den Code nicht zu trennen, und der Zweig sagt
    # das lieber, als eine Trennung zu behaupten.
    case "$m3_oc_status" in
    2 | 3 | 4)
        e2e_check "Humanitl itself did not refuse the real session" no \
            "humanitl run ended with $m3_oc_status: 2 means no daemon, 3 a failed isolation check, 4 a security violation. None of them is OpenCode's own exit code."
        ;;
    *)
        e2e_check "Humanitl itself did not refuse the real session" ok
        ;;
    esac

    # Was gemessen wird, ist der Verkehr und nicht die Antwort des Modells: Ein
    # Mock, der `tok0 ` sagt, bringt keinen Agenten dazu, etwas Sinnvolles zu
    # tun. Die Aussagen sind deshalb die des Issues: Der Agent spricht mit dem
    # Sprachmodell über die Durchreiche, er holt seinen Modellkatalog nicht aus
    # dem Netz, und er lässt niemanden warten.
    m3_oc_log=$(cat "$M3_OC_ERR")
    e2e_expect_match "the daemon started a sandbox for the real agent" \
        'sandbox [0-9a-f-]* started' "$m3_oc_log"

    # Alle drei Garantien, einzeln, wie beim Skript-Agenten. Eine Prüfung nur
    # auf `no network interface` bliebe grün, wenn `one door` oder `seccomp
    # active` in diesem Lauf brächen — und das sind die drei Sätze, auf denen
    # das ganze Produkt steht.
    e2e_expect_match "and it had no interface but lo either" \
        '^\[ok  \] no network interface ' "$m3_oc_log"
    e2e_expect_match "and exactly one socket, and it was the proxy" \
        '^\[ok  \] one door .*proxy\.sock' "$m3_oc_log"
    e2e_expect_match "and seccomp was active in the agent process" \
        '^\[ok  \] seccomp active ' "$m3_oc_log"

    # Der Zuwachs am Sprachmodell, nicht die Summe. Zwei Zahlen, weil sie zwei
    # verschiedene Dinge sagen: dass OpenCode überhaupt mit dem Modell
    # gesprochen hat, und dass mindestens einmal davon Inferenz war und nicht
    # nur eine Modellliste.
    m3_oc_llm_added=$(( $(m3_llm_hits '^mock-llm: ') - m3_oc_llm_before ))
    if [ "$m3_oc_llm_added" -ge 1 ]; then
        e2e_check "the real agent added requests of its own to the language model" ok
    else
        e2e_check "the real agent added requests of its own to the language model" no \
            "the mock served $m3_oc_llm_added requests beyond the $m3_oc_llm_before of the scripted sessions; OpenCode never spoke to the model"
    fi
    m3_oc_inference_added=$(( $(m3_llm_hits '^mock-llm: POST /v1/chat/completions ') - m3_oc_inference_before ))
    if [ "$m3_oc_inference_added" -ge 1 ]; then
        e2e_check "and at least one of them was an inference request" ok
    else
        e2e_check "and at least one of them was an inference request" no \
            "the mock served $m3_oc_inference_added inference requests beyond the $m3_oc_inference_before of the scripted sessions"
    fi

    # **Unverändert, nicht „nicht erlaubt".** Ein echter Abruf, den die
    # mitgelieferte Regel blockt, bestünde eine Prüfung auf
    # `decision:allow == 0` mühelos — und „hat es nicht versucht" und „wurde
    # daran gehindert" sind zwei verschiedene Aussagen über einen Agenten. Der
    # Zweig fragt die interessantere: Die Zahl der Flüsse zu beiden
    # Katalog-Hosts ist dieselbe wie vor dem Lauf. Für `models.dev` ist das
    # nicht null, sondern der eine geblockte Fluss des Skript-Agenten.
    e2e_expect "and never asked the model catalogue host its documentation names" \
        "$m3_oc_dev_before" "$(m3_count 'host:models.dev')"
    e2e_expect "nor the one the installed version really uses" \
        "$m3_oc_ai_before" "$(m3_count 'host:models.opencode.ai')"
    e2e_expect "and left nothing waiting for a human" 0 "$(m3_count 'state:held')"

    m3_ran_opencode=$((E2E_ASSERTIONS - m3_before_opencode))
    [ "$m3_ran_opencode" = "$M3_OPENCODE_ASSERTIONS" ] ||
        e2e_die "the OpenCode variant ran $m3_ran_opencode assertions, not $M3_OPENCODE_ASSERTIONS; adjust M3_OPENCODE_ASSERTIONS in this script so the number keeps its meaning"
    M3_EXPECTED_ASSERTIONS=$((M3_EXPECTED_ASSERTIONS + M3_OPENCODE_ASSERTIONS))
fi

# --- Der geordnete Abschied --------------------------------------------------

e2e_step "the daemon leaves nothing behind"

# Der letzte Stand der Historie, für die Artefakte: alles, was dieser Lauf
# aufgezeichnet hat, und nicht nur der Vierer-Stand nach der ersten Sitzung.
# Geschrieben wird er, solange der Daemon noch läuft.
humanitl --json flows list --asc > "$M3_FLOWS_JSON" 2> /dev/null || true

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

if [ "$E2E_ASSERTIONS" -lt "$M3_EXPECTED_ASSERTIONS" ]; then
    e2e_die "only $E2E_ASSERTIONS of $M3_EXPECTED_ASSERTIONS assertions ran; a branch was skipped"
fi
if [ "$E2E_ASSERTIONS" -gt "$M3_EXPECTED_ASSERTIONS" ]; then
    e2e_say "note: $E2E_ASSERTIONS assertions ran, $M3_EXPECTED_ASSERTIONS were expected;"
    e2e_say "      raise M3_EXPECTED_ASSERTIONS in this script so the number keeps its meaning"
fi
e2e_say "$E2E_ASSERTIONS assertions checked"

echo
if [ -n "$M3_SKIPPED" ]; then
    # Kein nacktes „OK". Ein übersprungener Zweig steht in der letzten Zeile,
    # damit niemand einen grünen Lauf für einen vollständigen hält.
    echo "M3 demo: OK with gaps — $M3_SKIPPED"
else
    echo "M3 demo: OK"
fi
