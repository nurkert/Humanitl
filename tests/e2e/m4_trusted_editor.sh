#!/usr/bin/env bash
# Das Demoskript des Meilensteins M4 (HUM-055).
#
# M4 zeigt, dass Editor, Audit-Kette, Konfiguration und Paket zusammen
# arbeiten: Ein Agent schickt eine Anfrage mit Kundenname, Mailadresse und
# IBAN, sie wird gehalten, ein Mensch pseudonymisiert sie im Editor und sendet
# die bearbeitete Fassung, und beim Ziel kommt kein einziges Original an. Die
# Audit-Kette hält, bricht nachweisbar bei einer geänderten Zeile, eine
# Einstellung aus `humanitl config set` wirkt, und das Paket besteht lintian.
#
# Die Schritte und ihre Ausgabe `[m4] step N ok`:
#
#    1. Bauen: Daemon, Kommandozeile, Shim, die Anwendung im Debug-Bau.
#    2. Wegwerf-Baum mit eigenen XDG-Verzeichnissen; der Audit-Schlüssel liegt
#       als Datei mit 0600 darin, kein Schlüsselbund ist beteiligt.
#    3. `config.toml`: Haltefrist 120 s, Nutzerbegriff „Müller GmbH",
#       `ui.language = "en"`, `audit.anchor_every = 5`.
#    4. Daemon starten, `humanitl daemon status` antwortet binnen 5 s.
#    5. Das Ziel `api.example.test` mit einem Blatt der Test-CA des Laufs.
#    6. `humanitl run` startet den Fake-Agenten mit der Anfrage aus
#       `fixtures/m4_request.json`; sie wird mit drei Funden gehalten.
#    7. Der Bildschirm (`app/integration_test/m4_trusted_editor_test.dart`):
#       Editor, „Replace all with pseudonyms", Senden, Chip „Edited".
#    8. **Sicherheitsbeweis:** Das Ziel hat nur Pseudonyme bekommen.
#    9. **Sicherheitsbeweis:** `humanitl audit verify` hält, der Export enthält
#       kein Original, eine bearbeitete Freigabe und einen Anker.
#   10. **Sicherheitsbeweis:** Eine geänderte Zeile bricht die Kette (Exit 4).
#   11. Entfällt, siehe unten.
#   12. `humanitl config set hold.timeout_secs 10` wirkt nach dem Neustart des
#       Daemons: Eine zweite Anfrage verfällt nach höchstens 12 s.
#   13. Das `.deb` aus einem Release-Bau besteht lintian ohne `E:`, und das
#       Release-Binary kennt keinen Test-Hebel `static_hosts`.
#   14. Aufräumen, Exit 0.
#
# Die Schritte 8, 9 und 10 sind die Sicherheitsbeweise des Sprints. Keiner von
# ihnen darf mit `|| true` oder einer ähnlichen Hintertür abgeschwächt werden;
# das ist ein Punkt der Review-Checkliste (`backlog/sprint-4.md`, HUM-055).
#
#   ./tests/e2e/m4_trusted_editor.sh     bauen und laufen
#   E2E_SKIP_BUILD=1 …                   die Binaries und die App nehmen, wie sie sind
#   E2E_TRACE=1 …                        zusätzlich `set -x` (die CI setzt es)
#   M4_PACKAGE=run|skip|only             Schritt 13 mitfahren (Vorgabe), auslassen
#                                        oder nur ihn fahren. Die CI fährt ihn im
#                                        eigenen Job `e2e-m4-package`, damit
#                                        `e2e-m4` unter zehn Minuten bleibt.
#   M4_SKIP_REPLACE=1 …                  die Negativprobe `e2e_m4_fails_when_original_leaks`:
#                                        Der Bildschirm ersetzt nur die IBAN und
#                                        sendet Kundenname und Mailadresse im
#                                        Klartext; Schritt 8 muss rot werden.
#                                        Nur von Hand, nie in CI.
#
# Exit-Codes: 0 alles belegt, 1 eine Behauptung hielt nicht oder eine
# Voraussetzung fehlte, 130 ein Abbruch durch ein Signal.
#
# --- Was der Lauf gegenüber der Spezifikation anpasst, und warum ------------
#
# Die Umfangsentscheidungen vom 2026-09-18 und 2026-10-01 (Kopf von
# `backlog/sprint-4.md`) haben Teile von M4 hinter 0.1.0 geschoben. Der Lauf
# prüft, was `main` kann, und sagt, was er nicht prüft:
#
#   * Kein Mapping-Panel, keine drei `pseudonym.created`-Records, kein Export
#     ohne Originale aus der Zuordnung: HUM-048 ist verschoben. Die Zuordnung
#     lebt nur im Speicher der Anwendung, der Daemon schreibt nie einen
#     `pseudonym.created`-Record.
#   * Der Nutzerbegriff wird `<TERM_1>`, nicht `Client-A`: `findings.user_terms`
#     ist auf `main` eine Liste von Texten ohne Alias, und die Anwendung liest
#     die Konfiguration des Daemons nicht (`editor_host.dart`, HUM-170).
#   * Schritt 11 (Sprache live umschalten) entfällt: Die Sprache des Fensters
#     kommt nicht aus `ui.language` (HUM-172), und Live-Neuladen über
#     `ConfigChanged` ist verschoben (zweite Umfangsentscheidung 2026-09-18).
#   * Schritt 12 zeigt die Wirkung einer Einstellung nach einem Neustart des
#     Daemons statt live, aus demselben Grund.
#   * Kein `experimental.static_hosts` und kein `extra_upstream_ca`: Seit
#     HUM-087 gibt es dafür `resolver.overrides` und `resolver.test_ca`, die
#     der Daemon nur mit `--allow-test-ca` annimmt. Der Lauf nutzt denselben
#     Weg wie M2 und M3, in einem eigenen Netz-Namensraum mit einer Adresse
#     aus TEST-NET-2; Loopback wiese der Proxy als privat ab. Schritt 13 prüft
#     trotzdem, dass das Release-Binary den Namen `static_hosts` nicht kennt.
#   * Kein Schlüsselbund und kein `KEYS_001`: Der Audit-Schlüssel ist auf
#     `main` eine Datei (`AuditKey::load_or_create_file`). Schritt 2 prüft
#     deshalb Ort und Rechte der Datei.

set -euo pipefail

M4_DIR="$(cd "$(dirname "$0")" && pwd)"
E2E_ROOT="$(cd "$M4_DIR/../.." && pwd)"
E2E_SCRIPT="$M4_DIR/m4_trusted_editor.sh"
E2E_SHELL=bash
export E2E_ROOT E2E_SCRIPT E2E_SHELL

# shellcheck source=tests/e2e/lib.sh
. "$E2E_ROOT/tests/e2e/lib.sh"

if [ "${E2E_TRACE:-0}" = 1 ]; then
    set -x
fi

# --- Was dieser Lauf erwartet ------------------------------------------------

M4_UI_TEST="$E2E_ROOT/app/integration_test/m4_trusted_editor_test.dart"
M4_FIXTURE="$M4_DIR/fixtures/m4_request.json"
M4_OUT="$E2E_ROOT/target/e2e/m4"
M4_PACKAGE_OUT="$E2E_ROOT/target/e2e/m4-package"

# So viele Behauptungen prüft ein vollständiger Lauf der Schritte 2 bis 12.
# Ein Skript, das grün ist, weil ein Zweig übersprungen wurde, ist schlimmer
# als keines (`tests/e2e/README.md`, „Die Zahl der Zusicherungen").
M4_EXPECTED_ASSERTIONS=37

# Der Host der Anfrage und seine Ports im Namensraum. Root im eigenen
# Namensraum darf 443 binden; der Proxy braucht damit keine Portumlenkung.
M4_HOST="api.example.test"
M4_HTTPS_PORT=443
M4_HTTP_PORT=80

# Die Haltefrist für den Bildschirm-Teil und die für Schritt 12.
#
# 120 Sekunden für den Editor: Der Treiber zeichnet unter Xvfb auf einem
# geteilten Läufer, öffnet den Editor und wartet auf zwei Antworten des
# Daemons; die Frist ist das Budget dafür, nicht eine Aussage über das Produkt.
# 10 Sekunden für Schritt 12, und 12 Sekunden Obergrenze bis `timed_out`, wie
# die Spezifikation sie nennt.
M4_HOLD_EDITOR=120
M4_HOLD_SHORT=10
M4_TIMEOUT_BOUND=12

# Was nie beim Ziel und nie im Audit-Export stehen darf: Anfänge der drei
# Originale, so kurz, dass auch eine teilweise Ersetzung auffiele.
#
# `GmbH` fängt den Rest des Kundennamens, `u00fcller` dasselbe Wort, wenn ein
# Agent oder eine Bearbeitung das `ü` als JSON-Folge `\u00fc` schreibt.
M4_ORIGINALS='anna@|mueller\.de|DE89|0532013000|Müller|GmbH|u00fcller'

# Was stattdessen ankommen muss.
M4_PSEUDONYMS='<TERM_1> <EMAIL_1> <IBAN_1>'

case "${M4_PACKAGE:-run}" in
run | skip | only) M4_PACKAGE_MODE="${M4_PACKAGE:-run}" ;;
*) e2e_die "M4_PACKAGE must be run, skip or only, not \"$M4_PACKAGE\"" ;;
esac

# m4_ok N [TEXT] — die Zeile, an der ein Mensch und die CI den Fortschritt lesen.
m4_ok() {
    printf '[m4] step %s ok%s\n' "$1" "${2:+ ($2)}"
}

# m4_count_lines PATTERN FILE — wie viele Zeilen von FILE auf PATTERN passen.
#
# `grep -c` endet mit 1, wenn nichts passt, und mit 2 bei einem Fehler. Nur die
# 1 ist hier eine Antwort (die Zahl 0); eine 2 bricht den Lauf ab, statt als
# „null Treffer" durchzugehen. So braucht keine Zählung ein `|| true`.
m4_count_lines() {
    m4_count_status=0
    m4_count_out=$(grep -c -E -- "$1" "$2") || m4_count_status=$?
    case "$m4_count_status" in
    0 | 1) printf '%s\n' "${m4_count_out:-0}" ;;
    *) e2e_die "grep could not read $2 (exit $m4_count_status)" ;;
    esac
}

# ============================================================================
# Außerhalb des Namensraums: bauen (Schritt 1), den Lauf starten, packen
# (Schritt 13). Im Namensraum gibt es kein Netz, also weder cargo noch pub.
# ============================================================================

# m4_duration — wie lange der Lauf bis hierher gebraucht hat, in Sekunden.
#
# Die CI liest daran das Ziel „unter zehn Minuten" ab (HUM-055). Ein harter
# Zeitdeckel im Job wäre bei kaltem Cache rot, ohne dass am Produkt etwas
# falsch wäre; deshalb steht die Zahl im Protokoll, auch nach einem Fehlschlag.
m4_duration() {
    printf '[m4] duration %ss\n' "$SECONDS"
}

if [ "${E2E_IN_NAMESPACE:-0}" != 1 ]; then
    # Am Ende jedes Laufs, grün oder rot, auch nach `e2e_die`.
    trap m4_duration EXIT
    if [ "$M4_PACKAGE_MODE" != only ]; then
        for tool in bwrap curl jq python3 ip unshare openssl dbus-run-session; do
            command -v "$tool" > /dev/null 2>&1 ||
                e2e_die "$tool is missing; the demo needs bubblewrap, curl, jq, python3, iproute2, util-linux, openssl and dbus"
        done
        [ -f "$M4_UI_TEST" ] || e2e_die "the integration test of the screen is missing: $M4_UI_TEST"
        command -v flutter > /dev/null 2>&1 || e2e_die "the screen half needs flutter on PATH"
        command -v xvfb-run > /dev/null 2>&1 || e2e_die "the screen half needs xvfb-run on PATH"
        [ -d "$E2E_ROOT/app/lib/core/ipc/generated" ] ||
            e2e_die "app/lib/core/ipc/generated is missing; run 'make flutter-codegen' first (it needs protoc and protoc-gen-dart)"

        # --- 1. Bauen --------------------------------------------------------
        e2e_step "1. build the daemon, the command line, the shim and the app"
        if [ "${E2E_SKIP_BUILD:-0}" = 1 ]; then
            [ -f "$E2E_ROOT/app/.dart_tool/package_config.json" ] ||
                e2e_die "app/.dart_tool/package_config.json is missing; run 'make flutter-get' first, or drop E2E_SKIP_BUILD"
            e2e_say "E2E_SKIP_BUILD=1, using the Flutter app as it is"
        else
            (cd "$E2E_ROOT/app" && flutter pub get) || e2e_die "flutter pub get failed"
            (cd "$E2E_ROOT/app" && flutter build linux --debug) ||
                e2e_die "flutter build linux --debug failed"
        fi
        e2e_build
        m4_ok 1

        # --- 2 bis 12 im eigenen Nutzer- und Netz-Namensraum ------------------
        #
        # Als Kind und nicht mit `exec` wie in `lib.sh`: Nach dem Namensraum
        # kommt noch Schritt 13, und der braucht das Netz für cargo.
        unshare -rn true 2> /dev/null ||
            e2e_die "unshare -rn failed: this machine forbids unprivileged user namespaces, so bwrap cannot start either; try: sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0"
        e2e_say "entering an own user and network namespace"
        m4_inner_status=0
        E2E_IN_NAMESPACE=1 M4_PUB_CACHE="${PUB_CACHE:-$HOME/.pub-cache}" \
            unshare -rn bash "$E2E_SCRIPT" || m4_inner_status=$?
        [ "$m4_inner_status" = 0 ] ||
            e2e_die "steps 2 to 12 failed (exit $m4_inner_status); the artefacts are in $M4_OUT"
    fi

    # --- 13. Das Paket -------------------------------------------------------
    if [ "$M4_PACKAGE_MODE" = skip ]; then
        printf '[m4] step 13 skipped (M4_PACKAGE=skip; the CI job e2e-m4-package runs it)\n'
    else
        e2e_step "13. the .deb from a release build passes lintian"
        for tool in dpkg-deb dpkg-shlibdeps patchelf lintian strings; do
            command -v "$tool" > /dev/null 2>&1 ||
                e2e_die "$tool is missing; step 13 needs dpkg-dev, patchelf, lintian and binutils (or run with M4_PACKAGE=skip)"
        done
        m4_version=0.0.0
        m4_bundle="$E2E_ROOT/app/build/linux/x64/release/bundle"
        rm -rf "$M4_PACKAGE_OUT"
        mkdir -p "$M4_PACKAGE_OUT/bin" "$M4_PACKAGE_OUT/dist"
        "$E2E_ROOT/packaging/release/build-binaries.sh" "$M4_PACKAGE_OUT/bin" ||
            e2e_die "the release build of the three binaries failed"
        (cd "$E2E_ROOT/app" && flutter build linux --release --build-name "$m4_version") ||
            e2e_die "flutter build linux --release failed"
        "$E2E_ROOT/packaging/release/check-version.sh" "$m4_version" "$M4_PACKAGE_OUT/bin" "$m4_bundle" ||
            e2e_die "the binaries and the app do not carry version $m4_version"
        "$E2E_ROOT/packaging/deb/build-deb.sh" "$m4_version" "$M4_PACKAGE_OUT/bin" "$m4_bundle" "$M4_PACKAGE_OUT/dist" ||
            e2e_die "build-deb.sh failed"
        m4_deb="$M4_PACKAGE_OUT/dist/humanitl_${m4_version}_amd64.deb"
        [ -s "$m4_deb" ] || e2e_die "no package at $m4_deb"

        # Was das Paket ausliefert, ausgepackt und nicht aus dem Bauverzeichnis:
        # Gemessen wird, was beim Nutzer landet.
        m4_unpacked="$M4_PACKAGE_OUT/unpacked"
        dpkg-deb -x "$m4_deb" "$m4_unpacked" || e2e_die "dpkg-deb -x could not unpack $m4_deb"

        # Die Test-Hebel des Vertrauens (HUM-087) dürfen im Paket nirgends
        # eingeschaltet sein: weder `--allow-test-ca` in einer Unit noch
        # `test_ca` in einer mitgelieferten Konfiguration. Durchsucht werden die
        # Units, die Profile und der Katalog unter `/usr/share/humanitl`, der
        # Desktop-Eintrag und ein etwaiges `/etc`; die Programme selbst nicht,
        # denn dort steht der Schalter als Name, den der Daemon kennt.
        m4_config_dirs=""
        for m4_dir in usr/lib/systemd usr/share/humanitl usr/share/applications etc; do
            [ -d "$m4_unpacked/$m4_dir" ] && m4_config_dirs="$m4_config_dirs $m4_unpacked/$m4_dir"
        done
        [ -n "$m4_config_dirs" ] || e2e_die "the package carries neither units nor configuration under $m4_unpacked"
        e2e_expect_match "the package ships its user units" '\.service$' \
            "$(find "$m4_unpacked/usr/lib/systemd" -type f 2> /dev/null)"
        m4_hook_status=0
        # shellcheck disable=SC2086 # m4_config_dirs ist absichtlich eine Wortliste.
        grep -r -n -e allow-test-ca -e test_ca $m4_config_dirs \
            > "$M4_PACKAGE_OUT/test-hooks.txt" 2>&1 || m4_hook_status=$?
        e2e_expect "no unit or configuration in the package mentions allow-test-ca or test_ca (grep exit 1 means none)" 1 \
            "$m4_hook_status"

        # Die Test-Hebel der Spezifikation (`static_hosts`, `extra_upstream_ca`)
        # gibt es auf `main` nicht (HUM-055, Abgleich der Schritte). Die beiden
        # Prüfungen bleiben als Erinnerung: Wer sie einführt, darf sie nur
        # hinter einem Feature bauen, das in diesem Binary fehlt.
        m4_humanitld=$(find "$m4_unpacked" -type f -name humanitld | head -n 1)
        [ -n "$m4_humanitld" ] || e2e_die "the package carries no humanitld"
        strings "$m4_humanitld" > "$M4_PACKAGE_OUT/humanitld.strings"
        e2e_expect "the packaged humanitld knows no static_hosts" 0 \
            "$(m4_count_lines 'static_hosts' "$M4_PACKAGE_OUT/humanitld.strings")"
        e2e_expect "and no extra_upstream_ca" 0 \
            "$(m4_count_lines 'extra_upstream_ca' "$M4_PACKAGE_OUT/humanitld.strings")"

        # Die Spezifikation verlangt „ohne `E:`". Die strengere Prüfung mit
        # Warnungen fährt der Release-Lauf in einem frischen Ubuntu-Container
        # (`packaging/deb/lintian.sh`); hier läuft lintian auf dem Läufer, und
        # dessen Fassung ist nicht die des Containers.
        m4_lintian_status=0
        lintian --display-level '>=warning' --show-overrides --fail-on error "$m4_deb" \
            > "$M4_PACKAGE_OUT/lintian.txt" 2>&1 || m4_lintian_status=$?
        cat "$M4_PACKAGE_OUT/lintian.txt"
        e2e_expect "lintian ends without an error" 0 "$m4_lintian_status"
        e2e_expect "and reports no E: line" 0 \
            "$(m4_count_lines '^E: ' "$M4_PACKAGE_OUT/lintian.txt")"
        m4_ok 13 "$(basename "$m4_deb")"
    fi

    # --- 14. Aufräumen -------------------------------------------------------
    # Der Namensraum hat seinen Wegwerf-Baum selbst entfernt; hier bleibt nur
    # zu sagen, dass alles durch ist.
    m4_ok 14
    if [ "$M4_PACKAGE_MODE" = only ]; then
        printf 'M4 demo: step 13 OK (steps 2-12 not run)\n'
    else
        printf 'M4 demo: OK\n'
    fi
    exit 0
fi

# ============================================================================
# Im Namensraum: Schritte 2 bis 12.
# ============================================================================

e2e_enter_namespace "$@"

rm -rf "$M4_OUT"
mkdir -p "$M4_OUT"

# --- 2. Der Wegwerf-Baum ------------------------------------------------------

e2e_step "2. a throwaway tree with its own XDG directories"
e2e_short_workdir
M4_CA_DIR="$E2E_WORKDIR/ca-test"
M4_PROJECT="$E2E_WORKDIR/home/project"
M4_CAPTURE="$E2E_WORKDIR/out/upstream-requests.jsonl"
M4_UPSTREAM_LOG="$E2E_WORKDIR/out/fake-upstream.log"
M4_AGENT_OUT="$E2E_WORKDIR/out/agent.jsonl"
M4_AGENT_ERR="$E2E_WORKDIR/out/agent.log"
M4_AGENT2_OUT="$E2E_WORKDIR/out/agent-2.jsonl"
M4_AGENT2_ERR="$E2E_WORKDIR/out/agent-2.log"
M4_UI_READY="$E2E_WORKDIR/ui-ready"
M4_UI_LOG="$E2E_WORKDIR/out/ui.log"
M4_AUDIT_EXPORT="$E2E_WORKDIR/out/audit.jsonl"
M4_UI_PID=""
M4_RUN_PID=""
mkdir -p "$M4_PROJECT"

# So lange darf der Bildschirm-Treiber laufen, bevor ein Wachhund ihn
# erschlägt. Der Test selbst bricht nach vier Minuten ab; die Frist hier ist
# der Riegel darüber, damit ein hängender Läufer nicht den ganzen Job hält.
M4_UI_WAIT_SECS=600

m4_ok 2 "$E2E_WORKDIR"

M4_OWN_PGID=$(ps -o pgid= -p $$ 2> /dev/null | tr -d ' ') || M4_OWN_PGID=""

# m4_kill_screen — den Treiber samt `xvfb-run` und `Xvfb` beenden.
#
# Dieselbe Überlegung wie `m2_kill_screen` in `m2_first_decision/run.sh`: Die
# Prozessgruppe des Treibers ist eine eigene (`setsid`), und nur dann wird die
# ganze Gruppe erschlagen. `|| m4_kill_pgid=""` fängt das `ps` auf einen schon
# beendeten Treiber ab, das sonst unter `set -e` das Aufräumen abbräche.
m4_kill_screen() {
    [ -n "${M4_UI_PID:-}" ] || return 0
    m4_kill_pgid=$(ps -o pgid= -p "$M4_UI_PID" 2> /dev/null | tr -d ' ') || m4_kill_pgid=""
    if [ -n "$m4_kill_pgid" ] && [ "$m4_kill_pgid" != "${M4_OWN_PGID:-}" ]; then
        m4_kill_target="-$m4_kill_pgid"
    else
        m4_kill_target="$M4_UI_PID"
    fi
    kill -TERM "$m4_kill_target" 2> /dev/null || true
    m4_kill_left=10
    while [ "$m4_kill_left" -gt 0 ] && kill -0 "$M4_UI_PID" 2> /dev/null; do
        sleep 0.1
        m4_kill_left=$((m4_kill_left - 1))
    done
    kill -KILL "$m4_kill_target" 2> /dev/null || true
    wait "$M4_UI_PID" 2> /dev/null || true
    M4_UI_PID=""
}

# collect — Prozesse beenden, Artefakte sichern, Wegwerf-Baum entfernen.
#
# Die Artefakte, die HUM-055 nach einem Fehlschlag verlangt: das Bild des
# Bildschirms (`m4-failure.png`, vom Treiber geschrieben), `humanitld.log`,
# `audit.jsonl` (der Export, sonst eine Kopie der Kette) und
# `fake-upstream.log`, dazu das Protokoll des Treibers und was das Ziel
# empfangen hat.
collect() {
    if [ -n "${M4_RUN_PID:-}" ]; then
        kill "$M4_RUN_PID" 2> /dev/null || true
        wait "$M4_RUN_PID" 2> /dev/null || true
        M4_RUN_PID=""
    fi
    m4_kill_screen
    stop_daemon
    stop_fake_upstream
    if [ -n "${E2E_WORKDIR:-}" ] && [ -d "$E2E_WORKDIR" ]; then
        cp -f "$E2E_WORKDIR"/out/* "$M4_OUT/" 2> /dev/null || true
        cp -f "$E2E_WORKDIR/daemon.log" "$M4_OUT/humanitld.log" 2> /dev/null || true
        if [ ! -s "$M4_OUT/audit.jsonl" ] && [ -n "${E2E_XDG_DATA:-}" ]; then
            cp -f "$E2E_XDG_DATA/humanitl/audit/audit.jsonl" "$M4_OUT/audit.jsonl" 2> /dev/null || true
        fi
        case "$E2E_WORKDIR" in
        /tmp/hum-e2e-*) rm -rf "$E2E_WORKDIR" ;;
        esac
    fi
}
e2e_trap collect

# --- 3. Die Konfiguration ----------------------------------------------------

e2e_step "3. config.toml of this run"
sh "$M4_DIR/fake-upstream/gen-test-ca.sh" "$M4_CA_DIR" "$M4_HOST" ||
    e2e_die "openssl could not create the test certificate"
mkdir -p "$E2E_WORKDIR/config/humanitl"
cat > "$E2E_WORKDIR/config/humanitl/config.toml" << EOF
# Die Konfiguration des M4-Demolaufs (HUM-055), von m4_trusted_editor.sh geschrieben.

[hold]
timeout_secs = $M4_HOLD_EDITOR

[findings]
user_terms = ["Müller GmbH"]

[ui]
language = "en"
notifications = false

[audit]
anchor_every = 5

[resolver]
# Test-Hebel aus HUM-087, nur wirksam mit --allow-test-ca.
overrides = { "$M4_HOST" = "$E2E_FAKE_ADDR" }
test_ca = "$M4_CA_DIR/test-ca.crt"
EOF
e2e_say "config $E2E_WORKDIR/config/humanitl/config.toml"
m4_ok 3

# --- 4. Der Daemon -------------------------------------------------------------

e2e_step "4. the daemon starts and answers within five seconds"
# `config`: Die Haltefrist kommt aus der Datei und nicht aus der Umgebung, sonst
# bewiese Schritt 12 nichts (`lib.sh`, `start_daemon`).
start_daemon "$E2E_WORKDIR/state" "$E2E_WORKDIR" config --allow-test-ca
# Socket und Token gehören dem Menschen allein; ohne 0700 hielte der
# Setup-Bildschirm den Start mit `DOCTOR_004` an (wie im M2-Lauf).
chmod 700 "$E2E_XDG_RUNTIME"

m4_status_ok=no
m4_status_left=50
while [ "$m4_status_left" -gt 0 ]; do
    if humanitl daemon status > /dev/null 2>&1; then
        m4_status_ok=ok
        break
    fi
    sleep 0.1
    m4_status_left=$((m4_status_left - 1))
done
e2e_check "humanitl daemon status exits 0 within five seconds" "$m4_status_ok" \
    "$(humanitl daemon status 2>&1 | tail -n 3)"
e2e_expect "the daemon took its hold timeout from config.toml" "$M4_HOLD_EDITOR" \
    "$(humanitl --json config get hold.timeout_secs | jq -r '.value')"
e2e_expect "and the user term of this run" '["Müller GmbH"]' \
    "$(humanitl --json config get findings.user_terms | jq -c '.value')"
e2e_expect "and anchors every five records" 5 \
    "$(humanitl --json config get audit.anchor_every | jq -r '.value')"

# Der Rest von Schritt 2, sobald der Daemon den Schlüssel angelegt hat: Er liegt
# als Datei im Wegwerf-Baum, nur für den Besitzer lesbar. Ein Schlüsselbund ist
# auf `main` an der Audit-Kette nicht beteiligt (`AuditKey::load_or_create_file`).
m4_key="$E2E_XDG_DATA/humanitl/keys/audit.key"
if [ ! -s "$m4_key" ]; then
    m4_key=$(find "$E2E_XDG_DATA/humanitl" -name audit.key -type f 2> /dev/null | head -n 1)
fi
e2e_check "the audit key is a file inside the throwaway tree" \
    "$(case "$m4_key" in "$E2E_WORKDIR"/*) [ -s "$m4_key" ] && echo ok ;; esac)" \
    "no audit.key under $E2E_XDG_DATA/humanitl"
e2e_expect "and only its owner may read it" 600 "$(stat -c '%a' "$m4_key")"
m4_ok 4

# --- 5. Das Ziel ---------------------------------------------------------------

e2e_step "5. the target api.example.test, with a leaf of the run's test CA"
m4_fifo="$E2E_WORKDIR/upstream.ready"
mkfifo "$m4_fifo"
python3 "$M4_DIR/fake-upstream/fake_upstream.py" \
    --address "$E2E_FAKE_ADDR" \
    --http-port "$M4_HTTP_PORT" \
    --https-port "$M4_HTTPS_PORT" \
    --cert "$M4_CA_DIR/upstream.crt" \
    --key "$M4_CA_DIR/upstream.key" \
    --capture "$M4_CAPTURE" \
    > "$m4_fifo" 2> "$M4_UPSTREAM_LOG" &
E2E_FAKE_PID=$!
m4_ready=""
read -r m4_ready < "$m4_fifo" || true
rm -f "$m4_fifo"
[ "$m4_ready" = "READY http=$M4_HTTP_PORT https=$M4_HTTPS_PORT" ] ||
    e2e_die "the fake upstream did not come up as asked (got \"$m4_ready\"): $(cat "$M4_UPSTREAM_LOG" 2> /dev/null)"

# Die positive Kontrolle am Proxy vorbei: Das Ziel antwortet über TLS mit dem
# Blatt der Test-CA. Ohne sie hieße ein leeres Capture in Schritt 8 nur „hier
# antwortet niemand".
m4_reach=$(curl -sS --max-time 5 --noproxy '*' --cacert "$M4_CA_DIR/test-ca.crt" \
    --resolve "$M4_HOST:$M4_HTTPS_PORT:$E2E_FAKE_ADDR" \
    "https://$M4_HOST/reachable" | jq -r '.path // ""' 2> /dev/null) || m4_reach=""
e2e_expect "the target answers over TLS with the leaf of the test CA" /reachable "$m4_reach"
e2e_expect_match "and the daemon loaded the test root, because this run passed --allow-test-ca" \
    '"roots":1' "$(grep -F -- '--allow-test-ca' "$DAEMON_LOG" | head -n 1)"
m4_ok 5

# --- 6. Die Sitzung ------------------------------------------------------------

e2e_step "6. humanitl run starts the fake agent with the request of the fixture"

# Erst der Bildschirm, dann der Agent: Ein Treiber, der nach dem Halten erst
# baut, verbrauchte die Haltefrist dafür.
#
# Die Umgebung ist dieselbe wie im M2-Lauf: der XDG-Baum des Daemons, Socket
# und Token ausdrücklich, ein frisches `HOME`, der Paket-Cache des Aufrufers.
# `env -u WAYLAND_DISPLAY` und `GDK_BACKEND=x11`: Das Fenster gehört in den
# Xvfb und nie auf den Bildschirm dessen, der den Lauf startet.
#
# Der Treiber bekommt einen eigenen Sitzungsbus, gestartet hier im
# Namensraum und mit einem Socket als Datei im Wegwerf-Baum. Der Bus eines
# `dbus-run-session` draußen taugt dafür nicht: dbus 1.14 (Ubuntu 24.04)
# lauscht mit `unix:tmpdir` auf einem abstrakten Socket, und abstrakte Sockets
# gehören dem Netz-Namensraum, in dem sie entstanden; aus `unshare -rn`
# heraus ist er unerreichbar. Ohne Diensteverzeichnis startet dieser Bus auch
# keine Portale, die den Bildschirm des Aufrufers oder sein
# `XDG_RUNTIME_DIR` anfassen könnten. Die Konfiguration ist die kleinste, die
# `dbus-daemon` annimmt.
M4_DBUS_DIR="$E2E_WORKDIR/dbus"
mkdir -p "$M4_DBUS_DIR"
cat > "$M4_DBUS_DIR/session.conf" << EOF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:dir=$M4_DBUS_DIR</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
EOF
rm -f "$M4_UI_READY"
setsid env -C "$E2E_ROOT/app" -u WAYLAND_DISPLAY -u DBUS_SESSION_BUS_ADDRESS \
    GDK_BACKEND=x11 \
    XDG_RUNTIME_DIR="$E2E_XDG_RUNTIME" \
    XDG_DATA_HOME="$E2E_XDG_DATA" \
    XDG_CONFIG_HOME="$E2E_XDG_CONFIG" \
    HOME="$E2E_HOME" \
    PUB_CACHE="$M4_PUB_CACHE" \
    FLUTTER_SUPPRESS_ANALYTICS=1 \
    HUMANITL_SOCKET="$DAEMON_SOCK" \
    HUMANITL_TOKEN="$DAEMON_TOKEN" \
    HUMANITL_E2E_M4_READY="$M4_UI_READY" \
    HUMANITL_E2E_M4_HOST="$M4_HOST" \
    HUMANITL_E2E_M4_SKIP_REPLACE="${M4_SKIP_REPLACE:-0}" \
    HUMANITL_E2E_SHOTS="$E2E_WORKDIR/out" \
    dbus-run-session --config-file="$M4_DBUS_DIR/session.conf" -- \
    xvfb-run -a --server-args='-screen 0 1600x1000x24' \
    flutter --no-version-check test --no-pub \
    integration_test/m4_trusted_editor_test.dart -d linux \
    > "$M4_UI_LOG" 2>&1 &
M4_UI_PID=$!
e2e_say "screen driver started (pid $M4_UI_PID), log $M4_UI_LOG"

m4_ui_left=3000
while [ ! -e "$M4_UI_READY" ]; do
    kill -0 "$M4_UI_PID" 2> /dev/null ||
        e2e_die "the screen driver ended before it was ready; its log: $(tail -n 30 "$M4_UI_LOG" 2> /dev/null)"
    [ "$m4_ui_left" -gt 0 ] || e2e_die "the screen did not come up within 600s; its log is in $M4_UI_LOG"
    sleep 0.2
    m4_ui_left=$((m4_ui_left - 1))
done
e2e_say "the screen is up and the daemon answered it"

cp "$M4_DIR/fake-agent/fake_agent.py" "$M4_PROJECT/fake_agent.py"
cp "$M4_FIXTURE" "$M4_PROJECT/m4_request.json"

# `humanitl run` im Projektverzeichnis, wie ein Mensch ihn startet (HUM-067).
(
    cd "$M4_PROJECT" &&
        humanitl -v run -- /usr/bin/python3 /work/fake_agent.py --request /work/m4_request.json \
            > "$M4_AGENT_OUT" 2> "$M4_AGENT_ERR"
) &
M4_RUN_PID=$!

M4_FLOW=$(wait_for_held 30 "host:$M4_HOST") ||
    e2e_die "the request to $M4_HOST was not held within thirty seconds; agent log: $(tail -n 20 "$M4_AGENT_ERR" 2> /dev/null)"
e2e_say "held flow $M4_FLOW"
m4_row=$(flow_row "$M4_FLOW") || e2e_die "the held flow $M4_FLOW is not in the flow list"
e2e_expect "the held request is the POST of the fixture" "POST /tickets" \
    "$(printf '%s' "$m4_row" | jq -r '"\(.method) \(.path)"')"
e2e_expect "and it carries three findings: the user term, the mail address and the IBAN" 3 \
    "$(printf '%s' "$m4_row" | jq -r '.finding_count')"
m4_ok 6 "flow $M4_FLOW"

# --- 7. Der Bildschirm ---------------------------------------------------------

e2e_step "7. the screen pseudonymises the request in the editor and sends it"

m4_wait_screen() {
    (
        sleep "$M4_UI_WAIT_SECS"
        kill -0 "$M4_UI_PID" 2> /dev/null || exit 0
        printf 'e2e: the screen driver still runs after %ss; ending it\n' "$M4_UI_WAIT_SECS" >&2
        m4_kill_screen
    ) &
    m4_watchdog=$!
    m4_screen_status=0
    wait "$M4_UI_PID" || m4_screen_status=$?
    kill "$m4_watchdog" 2> /dev/null || true
    wait "$m4_watchdog" 2> /dev/null || true
    M4_UI_PID=""
    return "$m4_screen_status"
}
m4_ui_status=0
m4_wait_screen || m4_ui_status=$?
e2e_expect "the screen driver passed (log $M4_UI_LOG)" 0 "$m4_ui_status"

m4_agent_status=0
wait "$M4_RUN_PID" || m4_agent_status=$?
M4_RUN_PID=""
e2e_expect "humanitl run exited cleanly" 0 "$m4_agent_status"
# `humanitl run` reicht das Terminal des Agenten durch, seine Hinweiszeilen
# stehen also zwischen den JSON-Zeilen; `fromjson?` überliest sie.
e2e_expect "the agent got the answer of the target" 200 \
    "$(jq -rR 'fromjson? | select(.url? // "" | endswith("/tickets")) | .status' "$M4_AGENT_OUT" | head -n 1)"

m4_row=$(flow_row "$M4_FLOW") || e2e_die "the decided flow $M4_FLOW is not in the flow list"
e2e_expect "the daemon recorded the decision as allow_edited" allow_edited \
    "$(printf '%s' "$m4_row" | jq -r '.decision')"
e2e_expect "and marks the flow as edited, the chip the history shows" true \
    "$(printf '%s' "$m4_row" | jq -r '.edited')"
m4_ok 7

# --- 8. Sicherheitsbeweis: nur Pseudonyme beim Ziel --------------------------
#
# Was dieser Schritt beweist: Die eine Anfrage an `/tickets`, die nach der
# Bearbeitung beim Ziel ankam, trägt an Stelle von Kundenname, Mailadresse und
# IBAN die drei Pseudonyme und keinen Anfang eines Originals, und ihr Rumpf ist
# mit einer stimmigen `content-length` und ohne `transfer-encoding`
# eingerahmt. Gelesen wird das Capture des Ziels, also die Bytes nach dem
# Proxy, nicht der Entwurf der Anwendung.

e2e_step "8. the target received pseudonyms only"

m4_hits=$(jq -c 'select(.path == "/tickets")' "$M4_CAPTURE")
e2e_expect "the target received exactly one request to /tickets" 1 \
    "$(printf '%s\n' "$m4_hits" | grep -c '^{')"
# Die Bytes, wie sie ankamen (`body_b64`), nicht die als Text gelesene Fassung:
# Eine Ersetzung durch das Unicode-Ersatzzeichen sähe sonst aus wie ein
# fehlendes Original.
printf '%s' "$m4_hits" | jq -r '.body_b64' | base64 -d > "$E2E_WORKDIR/out/upstream-body.txt" ||
    e2e_die "the captured body is not valid base64"
m4_body=$(cat "$E2E_WORKDIR/out/upstream-body.txt")
# Zuerst die Abwesenheit der Originale: Sie ist die Aussage dieses Schritts,
# und die Negativprobe soll genau an ihr rot werden.
e2e_expect "its body holds no beginning of an original ($M4_ORIGINALS)" 0 \
    "$(m4_count_lines "$M4_ORIGINALS" "$E2E_WORKDIR/out/upstream-body.txt")"
for m4_pseudonym in $M4_PSEUDONYMS; do
    if grep -qF -- "$m4_pseudonym" "$E2E_WORKDIR/out/upstream-body.txt"; then
        e2e_check "and carries $m4_pseudonym" ok
    else
        e2e_check "and carries $m4_pseudonym" no "body: $m4_body"
    fi
done
e2e_expect "content-length is the byte length of the body" \
    "$(printf '%s' "$m4_hits" | jq -r '.body_bytes')" \
    "$(printf '%s' "$m4_hits" | jq -r '[.headers[] | select(.[0] == "content-length") | .[1]] | join(",")')"
e2e_expect "and no transfer-encoding frames it" 0 \
    "$(printf '%s' "$m4_hits" | jq -r '[.headers[] | select(.[0] == "transfer-encoding")] | length')"
m4_ok 8

# --- 9. Sicherheitsbeweis: die Audit-Kette -------------------------------------

e2e_step "9. the audit chain verifies, and its export holds no original"

m4_verify_status=0
humanitl audit verify > "$E2E_WORKDIR/out/audit-verify.txt" 2>&1 || m4_verify_status=$?
e2e_expect "humanitl audit verify exits 0" 0 "$m4_verify_status"
humanitl audit export --format jsonl --out "$M4_AUDIT_EXPORT" ||
    e2e_die "humanitl audit export failed"
[ -s "$M4_AUDIT_EXPORT" ] || e2e_die "the audit export is empty"
e2e_expect "the export holds no beginning of an original" 0 \
    "$(m4_count_lines "$M4_ORIGINALS" "$M4_AUDIT_EXPORT")"
e2e_expect "and no pseudonym either: the daemon never sees the mapping (HUM-048 is after 0.1.0)" 0 \
    "$(m4_count_lines '"kind":"pseudonym\.created"' "$M4_AUDIT_EXPORT")"
e2e_expect "it records exactly one edited release, the one of this flow" "$M4_FLOW" \
    "$(jq -r 'select(.kind == "flow.decided" and .data.edited == true) | .data.flow' "$M4_AUDIT_EXPORT" | paste -sd, -)"
m4_anchors=$(m4_count_lines '"kind":"audit\.anchor"' "$M4_AUDIT_EXPORT")
if [ "$m4_anchors" -ge 1 ]; then
    e2e_check "and at least one anchor ($m4_anchors)" ok
else
    e2e_check "and at least one anchor" no "no audit.anchor in $(wc -l < "$M4_AUDIT_EXPORT") records"
fi
m4_ok 9

# --- 10. Sicherheitsbeweis: eine geänderte Zeile bricht die Kette ------------
#
# Geändert wird an Ort und Stelle, mit derselben Länge: Der Daemon hängt
# weiter an dieselbe Datei an, und ein Neuschreiben der ganzen Datei liefe mit
# ihm um die Wette. Eine Ziffer im Zeitstempel der dritten Zeile wird eine
# andere; die Zeile bleibt gültiges JSON, nur ihr Hash stimmt nicht mehr.

e2e_step "10. a changed line breaks the chain, and the restored one holds again"

m4_audit="$E2E_XDG_DATA/humanitl/audit/audit.jsonl"
[ -s "$m4_audit" ] || e2e_die "no audit log at $m4_audit"
m4_tamper() {
    python3 - "$m4_audit" "$1" << 'PY'
import json, sys
path, mode = sys.argv[1], sys.argv[2]
with open(path, "r+b") as handle:
    lines = handle.read().split(b"\n")
    if len(lines) < 3:
        sys.exit("the audit log has fewer than three lines")
    offset = sum(len(line) + 1 for line in lines[:2])
    line = lines[2]
    marker = b'"ts":"'
    at = line.find(marker)
    if at < 0:
        sys.exit("line 3 has no ts field")
    at += len(marker) + 3  # die letzte Ziffer der Jahreszahl
    old = line[at:at + 1]
    if mode == "tamper":
        new = b"7" if old != b"7" else b"6"
        sys.stdout.write(old.decode() + "\n")
    else:
        new = mode.encode()
    handle.seek(offset + at)
    handle.write(new)
    json.loads(line[:at] + new + line[at + 1:])
PY
}
m4_original_digit=$(m4_tamper tamper)
m4_broken_status=0
humanitl audit verify > "$E2E_WORKDIR/out/audit-verify-tampered.txt" 2>&1 || m4_broken_status=$?
m4_tamper "$m4_original_digit" > /dev/null
e2e_expect "humanitl audit verify exits 4 on the changed line" 4 "$m4_broken_status"
e2e_expect_match "and names the broken chain" 'AUDIT_001' \
    "$(cat "$E2E_WORKDIR/out/audit-verify-tampered.txt")"
m4_restored_status=0
humanitl audit verify > /dev/null 2>&1 || m4_restored_status=$?
e2e_expect "after the restore it verifies again" 0 "$m4_restored_status"
m4_ok 10

# --- 11. Sprache ---------------------------------------------------------------
printf '[m4] step 11 skipped (dropped: the window language does not come from ui.language, HUM-172, and live reload is after 0.1.0)\n'

# --- 12. Eine Einstellung wirkt ----------------------------------------------

e2e_step "12. config set hold.timeout_secs 10 takes effect after a daemon restart"

humanitl config set hold.timeout_secs "$M4_HOLD_SHORT" > /dev/null ||
    e2e_die "humanitl config set hold.timeout_secs failed"
e2e_expect "config set wrote the new hold timeout into config.toml" "timeout_secs = $M4_HOLD_SHORT" \
    "$(grep -E '^timeout_secs' "$E2E_WORKDIR/config/humanitl/config.toml")"

# Der Neustart ist Absicht und keine Abkürzung: Live-Neuladen ist hinter 0.1.0
# verschoben, und das Formular sagt dem Menschen dasselbe.
stop_daemon
start_daemon "$E2E_WORKDIR/state" "$E2E_WORKDIR" config --allow-test-ca
chmod 700 "$E2E_XDG_RUNTIME"
e2e_expect "the restarted daemon runs with the new hold timeout" "$M4_HOLD_SHORT" \
    "$(humanitl --json config get hold.timeout_secs | jq -r '.value')"

(
    cd "$M4_PROJECT" &&
        humanitl -v run -- /usr/bin/python3 /work/fake_agent.py --request /work/m4_request.json \
            > "$M4_AGENT2_OUT" 2> "$M4_AGENT2_ERR"
) &
M4_RUN_PID=$!
M4_FLOW2=$(wait_for_held 30 "host:$M4_HOST") ||
    e2e_die "the second request was not held within thirty seconds; agent log: $(tail -n 20 "$M4_AGENT2_ERR" 2> /dev/null)"
# Die Uhr läuft ab dem Eintreffen beim Daemon (`received_at` der Flow-Liste),
# nicht ab dem Moment, in dem dieses Skript den Flow zuerst sah: Sonst
# verschwände die Zeit bis zur ersten Abfrage aus der Messung. `received_at`
# hat ganze Sekunden und ist abgeschnitten; die gemessene Dauer ist damit
# höchstens eine Sekunde zu lang, nie zu kurz, und die Grenze wird dadurch
# strenger, nicht laxer. Daemon und Skript lesen dieselbe Uhr.
m4_held_at=$(flow_row "$M4_FLOW2" | jq -r '.received_at // empty') \
    || m4_held_at=""
case "$m4_held_at" in
'' | *[!0-9]*) e2e_die "the flow list gives no received_at for $M4_FLOW2: \"$m4_held_at\"" ;;
esac
m4_timed_out=no
while :; do
    m4_elapsed=$(awk -v a="$m4_held_at" -v b="$(date +%s.%N)" 'BEGIN { printf "%.1f", b - a }')
    m4_state=$(flow_row "$M4_FLOW2" 2> /dev/null | jq -r '.decision' 2> /dev/null) || m4_state=""
    if [ "$m4_state" = timed_out ]; then
        m4_timed_out=ok
        break
    fi
    awk -v e="$m4_elapsed" -v b="$M4_TIMEOUT_BOUND" 'BEGIN { exit !(e > b) }' && break
    sleep 0.2
done
e2e_check "the second request is timed_out in flows list --json within ${M4_TIMEOUT_BOUND}s (${m4_elapsed}s)" \
    "$m4_timed_out" "decision \"$m4_state\" after ${m4_elapsed}s"
m4_agent_status=0
wait "$M4_RUN_PID" || m4_agent_status=$?
M4_RUN_PID=""
e2e_expect "the second humanitl run exited cleanly" 0 "$m4_agent_status"
e2e_expect "the target still received exactly one request to /tickets" 1 \
    "$(jq -c 'select(.path == "/tickets")' "$M4_CAPTURE" | grep -c '^{')"
m4_verify_status=0
humanitl audit verify > /dev/null 2>&1 || m4_verify_status=$?
e2e_expect "and the chain still verifies across the restart" 0 "$m4_verify_status"
m4_ok 12 "timed_out after ${m4_elapsed}s"

humanitl --json flows list > "$E2E_WORKDIR/out/flows.json" 2> /dev/null || true

# --- Selbstprüfung ---------------------------------------------------------------

e2e_expect "this run checked as many assertions as a complete one" \
    "$M4_EXPECTED_ASSERTIONS" "$((E2E_ASSERTIONS + 1))"
