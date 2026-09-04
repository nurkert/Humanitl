# shellcheck shell=sh
# Die Helfer der Demoskripte (HUM-021, HUM-036, CONVENTIONS.md 3.11).
#
# Die Datei gehört den Demoskripten `m1_sealed_box.sh` und
# `m2_first_decision/run.sh`; wer sie ändert, prüft beide. Sie bleibt POSIX-sh,
# ohne `local`, ohne Arrays, ohne `[[`: `tests/escape/run.sh` startet denselben
# Daemon auf demselben Weg, buchstabiert ihn aber selbst aus, weil es ein
# eigenes Fehlermodell hat (`record_error` und Exit 2).
#
# Alles, was hier startet, gehört einem eigenen Wegwerf-Baum: eigenes
# `XDG_RUNTIME_DIR`, eigenes `XDG_DATA_HOME`, eigenes `HOME`. Ein laufender
# Daemon des Entwicklers wird dadurch nie berührt, und der Lauf hinterlässt
# nichts, was der nächste erbt.
#
# Zwei Fallstricke, die die Form der Helfer bestimmen:
#
#   * Der Pfad eines Unix-Sockets passt in 108 Bytes (`sun_path`). Der
#     Arbeitsbaum liegt deshalb unter `/tmp` und nicht unter einem womöglich
#     tiefen `TMPDIR`; `e2e_short_workdir` legt ihn an.
#   * Die Kommandozeile findet Daemon-Socket, Token, Proxy-Socket und CA über
#     `humanitl_config::Paths`, also über `XDG_RUNTIME_DIR` und
#     `XDG_DATA_HOME`. Daemon und Kommandozeile laufen deshalb mit genau
#     demselben XDG-Baum; `start_daemon` legt ihn an und merkt ihn sich in
#     `E2E_XDG_*`, `humanitl` reicht ihn bei jedem Aufruf weiter. Läge der
#     Proxy-Socket woanders als unter dem Laufzeitverzeichnis, lehnte die
#     Mount-Politik ihn mit `SANDBOX_006` ab.

# Der Wurzelpfad des Arbeitsbaums. Der Aufrufer darf ihn vorgeben; sonst wird
# er aus dem Skript abgeleitet, das gerade läuft (beide liegen zwei Ebenen
# unter der Wurzel).
E2E_ROOT="${E2E_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"

# Wohin cargo baut und woher die Binaries gelesen werden. Ein relativer Wert
# von CARGO_TARGET_DIR zählt von `daemon/` aus, weil cargo dort läuft; ein
# fester Pfad hier würde eine veraltete Kopie starten, während cargo woanders
# baut (derselbe Fallstrick wie in tests/escape/run.sh).
case "${CARGO_TARGET_DIR:-}" in
"") E2E_TARGET_DIR="$E2E_ROOT/daemon/target" ;;
/*) E2E_TARGET_DIR="$CARGO_TARGET_DIR" ;;
*) E2E_TARGET_DIR="$E2E_ROOT/daemon/$CARGO_TARGET_DIR" ;;
esac

E2E_DAEMON="$E2E_TARGET_DIR/debug/humanitld"
E2E_CLI="$E2E_TARGET_DIR/debug/humanitl"

# Das Sandbox-Profil, in dem die Sandboxen dieses Laufs starten.
#
# `humanitl` nimmt einen Namen, keinen Pfad: ein geklontes Repository darf die
# Politik der Sandbox nicht mitbringen. Der erste Ort, an dem es sucht, ist
# `$XDG_CONFIG_HOME/humanitl/profiles/sandbox/<name>.toml`, also der Baum des
# Nutzers. `start_daemon` legt die Profile des Arbeitsbaums dorthin, statt
# sich darauf zu verlassen, dass `profiles/sandbox` über dem Binary liegt: mit
# `CARGO_TARGET_DIR` irgendwo außerhalb des Arbeitsbaums täte es das nicht.
E2E_PROFILE_NAME=test

# Der Shim wird gebaut, wie er ausgeliefert wird (HUM-012): eigenes Profil,
# statisch gelinkt. Der musl-Zielbau ist die erste Wahl; ohne ihn tut es der
# glibc-Zielbau mit `+crt-static`.
E2E_MUSL_TARGET="$(uname -m)-unknown-linux-musl"
if [ -d "$(rustc --print target-libdir --target "$E2E_MUSL_TARGET" 2> /dev/null || true)" ]; then
    E2E_SHIM="$E2E_TARGET_DIR/$E2E_MUSL_TARGET/shim/humanitl-shim"
else
    E2E_MUSL_TARGET=""
    E2E_SHIM="$E2E_TARGET_DIR/shim/humanitl-shim"
fi

# Die Adresse, unter der das Ziel im Netz-Namensraum des Laufs erreichbar ist.
#
# 198.51.100.0/24 ist TEST-NET-2 aus RFC 5737: dokumentierter Testbereich,
# nirgends geroutet, und vor allem keine private Adresse. Das ist der Grund für
# die Wahl: Der Proxy weist eine aufgelöste Adresse in einem privaten Bereich
# ab (`UpstreamError::PrivateAddress`, `humanitl_core::ip_is_private`), also
# auch `127.0.0.1`. Ein Ziel auf dem Loopback könnte die Freigabe nie belegen,
# weil die Antwort dann `502 upstream_private_address` wäre statt des Inhalts.
E2E_FAKE_ADDR="198.51.100.7"

# --- Meldungen ---------------------------------------------------------------

# e2e_say TEXT... — eine Zeile ins Protokoll des Laufs.
e2e_say() {
    printf 'e2e: %s\n' "$*"
}

# e2e_step TEXT... — die Überschrift eines Schritts.
e2e_step() {
    printf '\n== %s ==\n' "$*"
}

# e2e_die TEXT... — abbrechen, mit dem Schritt im Protokoll.
e2e_die() {
    printf 'e2e: FAILED: %s\n' "$*" >&2
    exit 1
}

# --- Aufräumen ---------------------------------------------------------------

# e2e_trap CLEANUP — CLEANUP an Beenden und an einen Abbruch hängen.
#
# `trap CLEANUP EXIT` allein genügt nicht. Ein Demoskript verbringt die meiste
# Zeit in `wait` auf einen Hintergrundprozess; trifft `SIGINT` die Shell dort
# und ist kein eigener Handler gesetzt, bricht nur der Wartelauf ab, und das
# Skript läuft weiter. Wer abgebrochen hat, bekäme am Ende einen grünen Bericht
# über einen Lauf, den er beendet zu haben glaubte — und ein hartes Töten
# hinterließe Daemon, Ziel, Sandbox-Baum und die privaten Schlüssel des Laufs.
#
# Der Handler räumt deshalb selbst auf, nimmt vorher den `EXIT`-Trap weg, damit
# nicht zweimal aufgeräumt wird, und endet mit 130 (dem verabredeten Code für
# einen Abbruch durch ein Signal, 128 + SIGINT).
e2e_trap() {
    trap "$1" EXIT
    trap "printf 'e2e: interrupted\\n' >&2; trap - EXIT; $1; exit 130" INT TERM HUP
}

# Wie viele Behauptungen dieser Lauf geprüft hat, gehalten oder nicht.
#
# Ein Demoskript, das grün ist, weil ein Zweig übersprungen wurde, ist
# schlimmer als keines. Der Zähler ist die Grundlage der Selbstprüfung am Ende
# eines Laufs; er wächst in `e2e_check`, durch das jede Behauptung geht.
E2E_ASSERTIONS=0

# e2e_check DESCRIPTION CONDITION-RESULT — eine Behauptung, die halten muss.
#
# Der Aufrufer prüft selbst und übergibt `ok` oder alles andere; der Helfer
# schreibt in beiden Fällen eine Zeile, damit im Protokoll steht, was geprüft
# wurde und nicht nur, was schiefging.
e2e_check() {
    E2E_ASSERTIONS=$((E2E_ASSERTIONS + 1))
    e2e_check_what="$1"
    shift
    if [ "$1" = ok ]; then
        printf '  ok    %s\n' "$e2e_check_what"
        return 0
    fi
    shift
    printf '  FAIL  %s\n' "$e2e_check_what" >&2
    e2e_die "$e2e_check_what: $*"
}

# e2e_expect DESCRIPTION EXPECTED ACTUAL — Gleichheit zweier Werte.
e2e_expect() {
    if [ "$2" = "$3" ]; then
        e2e_check "$1" ok
    else
        e2e_check "$1" no "expected $2, got $3"
    fi
}

# e2e_expect_match DESCRIPTION PATTERN TEXT — eine Zeile von TEXT passt auf PATTERN.
e2e_expect_match() {
    if printf '%s\n' "$3" | grep -qE "$2"; then
        e2e_check "$1" ok
    else
        e2e_check "$1" no "nothing matched /$2/ in: $3"
    fi
}

# --- Bauen -------------------------------------------------------------------

# e2e_build — Daemon, Kommandozeile und Shim bauen.
#
# Übersprungen mit `E2E_SKIP_BUILD=1`. Der Shim bekommt dieselbe Behandlung wie
# in tests/escape/run.sh: das `shim`-Profil und ein statischer, nicht
# verschiebbarer Link, damit in der Sandbox keine Bibliothek des Hosts fehlt.
e2e_build() {
    if [ "${E2E_SKIP_BUILD:-0}" = 1 ]; then
        e2e_say "E2E_SKIP_BUILD=1, using the binaries as they are"
        e2e_install_shim
        return 0
    fi
    e2e_step "building the daemon, the command line and the shim"
    (
        cd "$E2E_ROOT/daemon" &&
            CARGO_TARGET_DIR="$E2E_TARGET_DIR" cargo build \
                -p humanitld -p humanitl --bin humanitld --bin humanitl
    ) || e2e_die "cargo build of humanitld and humanitl failed"

    set -- cargo rustc -p humanitl-shim --bin humanitl-shim --profile shim
    if [ -n "$E2E_MUSL_TARGET" ]; then
        set -- "$@" --target "$E2E_MUSL_TARGET"
    fi
    set -- "$@" -- -C target-feature=+crt-static -C relocation-model=static
    (cd "$E2E_ROOT/daemon" && CARGO_TARGET_DIR="$E2E_TARGET_DIR" "$@") ||
        e2e_die "the static build of humanitl-shim failed"

    e2e_install_shim
}

# e2e_install_shim — den Shim dorthin legen, wo die Kommandozeile ihn sucht.
#
# `humanitl sandbox run` kennt kein `--shim`: es nimmt den Shim neben dem
# eigenen Binary oder aus den Verzeichnissen einer Installation
# (`/usr/lib/humanitl` und die beiden anderen). Der Lauf legt den statisch
# gelinkten Shim deshalb neben `humanitl`, also genau so, wie ein Paket ihn
# ausliefert. Der Zielbau steht dabei woanders (`shim`-Profil, womöglich
# musl), und das soll er auch: gemessen wird der Shim, der ausgeliefert wird.
e2e_install_shim() {
    [ -x "$E2E_SHIM" ] || e2e_die "no shim binary at $E2E_SHIM"
    # Erst weg, dann hin: cargo verlinkt `debug/humanitl-shim` hart mit seiner
    # Kopie unter `debug/deps`, und ein `cp` darüber schriebe durch beide.
    rm -f "$E2E_TARGET_DIR/debug/humanitl-shim"
    cp "$E2E_SHIM" "$E2E_TARGET_DIR/debug/humanitl-shim" ||
        e2e_die "cannot put the shim next to humanitl"
}

# --- Netz-Namensraum ---------------------------------------------------------

# e2e_enter_namespace ARGS... — das Skript im eigenen Netz-Namensraum neu starten.
#
# Der Lauf braucht ein Ziel, das der Proxy erreichen darf und die Sandbox
# nicht. Beides zugleich gibt es auf dem Host nicht: alles, was dort lokal
# lauscht, liegt auf dem Loopback, und den weist der Proxy als privat ab.
# Deshalb bekommt der ganze Lauf einen eigenen Netz-Namensraum, in dem
# `E2E_FAKE_ADDR` (TEST-NET-2, nicht privat) auf `lo` liegt. Das Ziel ist damit
# vom Host aus erreichbar, aus der Sandbox aber nicht, weil deren eigener
# Namensraum nur ein leeres `lo` hat — genau die Kontrollprobe, die Garantie 1
# braucht.
#
# Nebenwirkung, die dem Demo zugutekommt: Der Namensraum hat keine Route nach
# draußen. Der geblockte Request in Schritt 3 kann deshalb gar nicht im Netz
# gelandet sein, gleich was der Proxy versucht hätte.
#
# `unshare -rn` braucht dieselben unprivilegierten Nutzer-Namensräume wie
# `bwrap`; scheitert es, wäre die Sandbox ohnehin nicht startbar, und die
# Meldung sagt das.
e2e_enter_namespace() {
    if [ "${E2E_IN_NAMESPACE:-0}" = 1 ]; then
        ip link set lo up 2> /dev/null ||
            e2e_die "cannot bring up lo in the namespace; is iproute2 installed?"
        ip addr add "$E2E_FAKE_ADDR/32" dev lo 2> /dev/null ||
            e2e_die "cannot add $E2E_FAKE_ADDR to lo in the namespace"
        return 0
    fi
    command -v unshare > /dev/null 2>&1 ||
        e2e_die "unshare is missing; install util-linux"
    command -v ip > /dev/null 2>&1 ||
        e2e_die "ip is missing; install iproute2"
    if ! unshare -rn true 2> /dev/null; then
        e2e_die "unshare -rn failed: this machine forbids unprivileged user namespaces, so bwrap cannot start either; try: sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0"
    fi
    e2e_say "entering an own user and network namespace"
    E2E_IN_NAMESPACE=1
    export E2E_IN_NAMESPACE
    # Gebaut wird vor dem Wechsel: im neuen Namensraum gibt es kein Netz, und
    # ein cargo, das doch etwas laden wollte, hinge dort ohne Erklärung.
    exec unshare -rn "${E2E_SHELL:-bash}" "${E2E_SCRIPT:-$0}" "$@"
}

# --- Wegwerf-Baum ------------------------------------------------------------

# e2e_short_workdir — ein kurzes Arbeitsverzeichnis, das in `sun_path` passt.
#
# Setzt E2E_WORKDIR und legt die vier Unterverzeichnisse an, die der Daemon als
# XDG-Baum bekommt, dazu `work` (das Projektverzeichnis der Sandbox) und
# `state` (die Platzhalter des Starters, samt Laufzeitverzeichnis).
e2e_short_workdir() {
    E2E_WORKDIR=$(mktemp -d /tmp/hum-e2e-XXXXXX) ||
        e2e_die "cannot create a temporary directory under /tmp"
    mkdir -p \
        "$E2E_WORKDIR/data" "$E2E_WORKDIR/config" "$E2E_WORKDIR/home" \
        "$E2E_WORKDIR/work" "$E2E_WORKDIR/state" "$E2E_WORKDIR/out"
    e2e_say "workdir $E2E_WORKDIR"
}

# --- Warten ------------------------------------------------------------------

# wait_for_socket PATH SECONDS — warten, bis der Socket da ist.
wait_for_socket() {
    wait_socket_path="$1"
    wait_socket_left=$(( ${2:-10} * 20 ))
    while [ "$wait_socket_left" -gt 0 ]; do
        if [ -S "$wait_socket_path" ]; then
            return 0
        fi
        sleep 0.05
        wait_socket_left=$((wait_socket_left - 1))
    done
    return 1
}

# --- Das Ziel ----------------------------------------------------------------

# start_fake_upstream — das Ziel starten, das eine Freigabe erreichen darf.
#
# Setzt FAKE_HTTP auf den Port, den das Betriebssystem vergeben hat, und
# E2E_FAKE_PID auf den Prozess. Der Port kommt über eine Fifo zurück, damit das
# Skript nicht raten muss.
start_fake_upstream() {
    fake_fifo="$E2E_WORKDIR/upstream.port"
    rm -f "$fake_fifo"
    mkfifo "$fake_fifo" || e2e_die "cannot create the fifo for the upstream port"
    python3 "$E2E_ROOT/tests/e2e/fake_upstream.py" "$E2E_FAKE_ADDR" \
        > "$fake_fifo" 2> "$E2E_WORKDIR/out/upstream.log" &
    E2E_FAKE_PID=$!
    # `read` blockiert, bis der Server die Zeile schreibt; bricht er vorher ab,
    # bleibt FAKE_HTTP leer und der Aufrufer merkt es sofort.
    fake_word=""
    FAKE_HTTP=""
    read -r fake_word FAKE_HTTP < "$fake_fifo" || true
    rm -f "$fake_fifo"
    if [ "$fake_word" != PORT ] || [ -z "$FAKE_HTTP" ]; then
        e2e_die "the fake upstream did not report a port: $(cat "$E2E_WORKDIR/out/upstream.log" 2>/dev/null)"
    fi
    e2e_say "fake upstream on $E2E_FAKE_ADDR:$FAKE_HTTP (pid $E2E_FAKE_PID)"
}

# stop_fake_upstream — das Ziel beenden.
stop_fake_upstream() {
    if [ -n "${E2E_FAKE_PID:-}" ]; then
        kill "$E2E_FAKE_PID" 2> /dev/null || true
        wait "$E2E_FAKE_PID" 2> /dev/null || true
        E2E_FAKE_PID=""
    fi
}

# --- Der Daemon --------------------------------------------------------------

# start_daemon STATE_DIR XDG_DIR [HOLD_TIMEOUT_SECS] — den echten Daemon starten.
#
# Der Daemon läuft mit `XDG_RUNTIME_DIR=<STATE_DIR>/runtime`, damit sein
# Proxy-Socket dort liegt, wo die Mount-Politik ihn erwartet; die
# Kommandozeile läuft danach im selben Baum und findet ihn deshalb ohne ein
# einziges Flag. XDG_DIR trägt Daten, Konfiguration und HOME des Laufs.
#
# Setzt DAEMON_SOCK, DAEMON_TOKEN, DAEMON_PROXY_SOCK, DAEMON_CA_CERT,
# DAEMON_CA_BUNDLE, DAEMON_LOG und E2E_DAEMON_PID, dazu die vier `E2E_XDG_*`,
# mit denen `humanitl` denselben Baum sieht wie der Daemon.
start_daemon() {
    daemon_state="$1"
    daemon_xdg="$2"
    daemon_hold="${3:-10}"

    E2E_XDG_RUNTIME="$daemon_state/runtime"
    E2E_XDG_DATA="$daemon_xdg/data"
    E2E_XDG_CONFIG="$daemon_xdg/config"
    E2E_HOME="$daemon_xdg/home"
    DAEMON_RUNTIME="$daemon_state/runtime/humanitl"
    DAEMON_SOCK="$DAEMON_RUNTIME/daemon.sock"
    DAEMON_TOKEN="$DAEMON_RUNTIME/token"
    DAEMON_PROXY_SOCK="$DAEMON_RUNTIME/proxy/proxy.sock"
    DAEMON_CA_CERT="$daemon_xdg/data/humanitl/ca/ca.crt"
    DAEMON_CA_BUNDLE="$daemon_xdg/data/humanitl/ca/ca-bundle.crt"
    DAEMON_LOG="$daemon_xdg/daemon.log"

    if [ "${#DAEMON_PROXY_SOCK}" -ge 108 ]; then
        e2e_die "the proxy socket path is ${#DAEMON_PROXY_SOCK} bytes, and sun_path holds 107: $DAEMON_PROXY_SOCK"
    fi
    mkdir -p "$E2E_XDG_RUNTIME" "$E2E_XDG_DATA" "$E2E_XDG_CONFIG" "$E2E_HOME" \
        "$E2E_XDG_CONFIG/humanitl/profiles/sandbox"
    cp -f "$E2E_ROOT/profiles/sandbox/"*.toml "$E2E_XDG_CONFIG/humanitl/profiles/sandbox/" ||
        e2e_die "cannot put the sandbox profiles where humanitl looks for them"

    [ -x "$E2E_DAEMON" ] || e2e_die "no humanitld binary at $E2E_DAEMON"
    XDG_RUNTIME_DIR="$E2E_XDG_RUNTIME" \
        XDG_DATA_HOME="$E2E_XDG_DATA" \
        XDG_CONFIG_HOME="$E2E_XDG_CONFIG" \
        HOME="$E2E_HOME" \
        HUMANITL_HOLD__TIMEOUT_SECS="$daemon_hold" \
        "$E2E_DAEMON" > "$DAEMON_LOG" 2>&1 &
    E2E_DAEMON_PID=$!

    if ! wait_for_socket "$DAEMON_SOCK" 20 || ! wait_for_socket "$DAEMON_PROXY_SOCK" 20; then
        e2e_say "the daemon did not come up; its log follows"
        cat "$DAEMON_LOG" >&2 || true
        e2e_die "no daemon.sock and proxy.sock within twenty seconds"
    fi
    e2e_say "daemon up (pid $E2E_DAEMON_PID), hold timeout ${daemon_hold}s, log $DAEMON_LOG"
}

# stop_daemon — den Daemon geordnet beenden.
#
# `SIGTERM`, nicht `SIGKILL`: nur der geordnete Weg räumt Socket und Token weg,
# und dass er das tut, gehört zu dem, was das Demo zeigt.
stop_daemon() {
    if [ -n "${E2E_DAEMON_PID:-}" ]; then
        kill -TERM "$E2E_DAEMON_PID" 2> /dev/null || true
        wait "$E2E_DAEMON_PID" 2> /dev/null || true
        E2E_DAEMON_PID=""
    fi
}

# --- Die Kommandozeile -------------------------------------------------------
#
# Jede Fähigkeit ist zuerst ein RPC, und die Kommandozeile ist der dünne Client
# davor (ADR-018). Das Demo geht deshalb denselben Weg wie später der Nutzer:
# `humanitl` statt eines eigenen Testklienten. Die Kommandozeile findet Socket,
# Token, Proxy-Socket und CA über `humanitl_config::Paths`; das Einzige, was
# der Lauf ihr sagen muss, ist der XDG-Baum dieses Laufs.

# humanitl ARGS... — die Kommandozeile im XDG-Baum dieses Laufs.
humanitl() {
    XDG_RUNTIME_DIR="$E2E_XDG_RUNTIME" \
        XDG_DATA_HOME="$E2E_XDG_DATA" \
        XDG_CONFIG_HOME="$E2E_XDG_CONFIG" \
        HOME="$E2E_HOME" \
        "$E2E_CLI" "$@"
}

# wait_for_held SECONDS [FILTER] — auf den ersten wartenden Flow warten, Id auf stdout.
#
# Gepollt wird `humanitl flows list --json`, gefiltert im Daemon
# (`state:held`, dazu was der Aufrufer ergänzt: `host:<text>` oder ein freies
# Wort, das auf Host oder Pfad passt). Die Antwort ist eine Zeile JSON; `jq`
# nimmt die erste Id daraus. Ohne wartenden Flow ist die Liste leer, und die
# Schleife versucht es noch einmal.
wait_for_held() {
    held_left=$(( ${1:-10} * 10 ))
    held_filter="state:held"
    if [ -n "${2:-}" ]; then
        held_filter="$held_filter $2"
    fi
    while [ "$held_left" -gt 0 ]; do
        held_id=$(humanitl --json flows list "$held_filter" 2> /dev/null |
            jq -r '.flows[0].flow_id // empty' 2> /dev/null || true)
        if [ -n "$held_id" ]; then
            printf '%s\n' "$held_id"
            return 0
        fi
        sleep 0.1
        held_left=$((held_left - 1))
    done
    return 1
}

# flow_show ID — die Eckdaten eines Flows als JSON auf stdout.
flow_show() {
    humanitl --json flows show "$1"
}

# flow_row ID — die Zeile eines Flows aus der Historie als JSON auf stdout.
#
# Seit HUM-026 beantwortet der Daemon `ListFlows` aus der Aufzeichnung, also
# auch für Flows früherer Sitzungen. Kennt die Liste die Id nicht, ist das ein
# Fehlschlag (Rückgabewert 1) und keine leere Zeile, die der Aufrufer für einen
# Flow ohne Inhalt halten könnte.
flow_row() {
    flow_row_json=$(humanitl --json flows list 2> /dev/null) || return 1
    flow_row_out=$(printf '%s' "$flow_row_json" |
        jq -c --arg id "$1" '.flows[] | select(.flow_id == $id)' 2> /dev/null) || return 1
    [ -n "$flow_row_out" ] || return 1
    printf '%s\n' "$flow_row_out"
}

# flow_decide ID allow|block [NOTE] — einen wartenden Flow entscheiden.
flow_decide() {
    if [ -n "${3:-}" ]; then
        humanitl --json flows decide "$1" "$2" --note "$3" > /dev/null
    else
        humanitl --json flows decide "$1" "$2" > /dev/null
    fi
}

# daemon_info — `daemon status` als JSON auf stdout.
daemon_info() {
    humanitl --json daemon status
}

# --- Die Sandbox -------------------------------------------------------------

# sandbox_run CMD... — den Befehl in der Sandbox laufen lassen.
#
# `humanitl sandbox run`, wie CONVENTIONS.md 3.11 es verlangt: Das Demo läuft
# über dieselben Codepfade wie später der Nutzer, und was hier scheitert,
# scheitert auch bei ihm. Die Kommandozeile liest Profil, Proxy-Socket, CA und
# Shim selbst; genannt werden nur die drei Dinge, die zu diesem Lauf gehören:
# das Profil `test`, das Projektverzeichnis und das Testverzeichnis hinter dem
# Platzhalter `/tests/escape` des Profils.
#
# `-v` schaltet die Zeilen des Isolationsberichts frei (`check <name>
# pass|FAIL: <evidence>` auf stderr). stdout bleibt dem Befehl vorbehalten.
#
# Ist eine der drei Garantien rot, beendet `sandbox run` die Sandbox und endet
# mit 3. Beendet, nicht verhindert: Der Shim startet den Befehl unmittelbar
# nach seiner letzten Pruefzeile, geprueft wird auf dem Wirt danach
# (docs/THREAT-MODEL.md K-15).
sandbox_run() {
    [ -x "$E2E_CLI" ] || e2e_die "no humanitl binary at $E2E_CLI"
    humanitl -v sandbox run \
        --profile "$E2E_PROFILE_NAME" \
        --work "$E2E_WORKDIR/work" \
        --tests-dir "$E2E_ROOT/tests/escape" \
        -- "$@"
}
