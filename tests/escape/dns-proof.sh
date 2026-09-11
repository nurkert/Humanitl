# shellcheck shell=sh
# The DNS proof of ESC-3, taken on the host (HUM-115). Sourced by run.sh.
#
# ADR-006 says a name is resolved only after the decision, and THREAT-MODEL
# K-10 says ESC-3 observes that from the host. This file is that observation.
# run.sh calls, in this order:
#
#   dns_stub_start    before the daemon starts: tests/escape/dns-stub.py on a
#                     free UDP port of 127.0.0.1 (never 53), and DNS_NAMESERVER
#                     set to its address for the daemon of the run
#   dns_watch_start   right before ESC-3: a watcher in the background that
#                     follows the queue of the daemon through `humanitl`
#   dns_watch_stop    right after ESC-3
#   dns_host_cases    after the suites in the sandbox: three cases,
#                     esc-3/dns_not_before_decision, esc-3/dns_after_allow_once
#                     and esc-3/meta_no_dns_lookup
#   dns_stub_stop     after the daemon has stopped
#
# How the proof works. With resolver.nameserver set, the daemon resolves
# through HickoryResolver, which asks that one server and nobody else: no
# /etc/resolv.conf, no /etc/hosts, no second server, and no cache of its own
# (resolver.cache_ttl_secs is 0 for the run as well). The log of the stub is
# therefore the complete list of names the daemon resolved, with the time of
# each question. ESC-3 sends two requests at its end, one to held.esc3.test,
# which nobody decides, and one to allowed.esc3.test, which the watcher
# allows. The watcher copies the log at the moment the daemon reports each of
# them as held, and notes the time right before `humanitl flows decide`.
#
# An absence is evidence only next to a presence. Every case that says "this
# name is not in the log" first checks that the same log holds the one name
# that has to be there: without it, the setting, the stub or the watcher did
# not work, and an empty log would read as "nothing was resolved". The same
# holds for the meta endpoint: its absence counts only when ESC-3 was actually
# answered by it.
#
# Needs from run.sh: OUT, HERE, RESULTS, CLI, STATE, DAEMON_XDG and
# record_case. POSIX sh like the rest of the harness, no `local`; every name
# here starts with dns_ or DNS_.

DNS_LOG="$OUT/dns.log"
DNS_PORT_FILE="$OUT/dns-stub.port"
DNS_WATCH="$OUT/dns-watch"
# The two names of the proof. Neither may appear in resolver.overrides: an
# override answers before any question and would never reach the stub.
DNS_HELD_HOST=held.esc3.test
DNS_ALLOWED_HOST=allowed.esc3.test
# Every other name ESC-3 sends through the proxy without anybody allowing it:
# held into the timeout (blocked.example, the IDN, the look-alike of the meta
# host) or blocked at once for a contradicting authority (github.com with
# Host: evil.io). None of them may ever reach the name server.
DNS_UNDECIDED_HOSTS="blocked.example xn--80ak6aa92e.com evil-humanitl.internal github.com evil.io"
# The watcher gives up after this many seconds; ESC-3 needs well under one
# minute on a developer machine.
DNS_WATCH_SECONDS=180

dns_stub_pid=""
dns_watch_pid=""
DNS_NAMESERVER=""

# dns_cli ARGS... — the command line in the XDG tree of the daemon of this run.
dns_cli() {
    XDG_RUNTIME_DIR="$STATE/runtime" \
        XDG_DATA_HOME="$DAEMON_XDG/data" \
        XDG_CONFIG_HOME="$DAEMON_XDG/config" \
        HOME="$DAEMON_XDG/home" \
        "$CLI" "$@"
}

# dns_now_ms — milliseconds since the epoch, from the same clock dns-stub.py
# stamps its lines with.
dns_now_ms() {
    python3 -c 'import time; print(int(time.time() * 1000))'
}

# dns_held_flow HOST — the id of a flow to HOST that is held right now, or an
# empty line. `host:` in the filter matches the name and names below it.
dns_held_flow() {
    dns_cli --json flows list "state:held host:$1" 2> /dev/null |
        python3 -c '
import json, sys
try:
    flows = json.load(sys.stdin).get("flows") or []
except ValueError:
    flows = []
print(flows[0]["flow_id"] if flows else "")
' 2> /dev/null || true
}

# dns_count FILE NAME — how many questions in FILE ask for exactly NAME.
dns_count() {
    if [ ! -f "$1" ]; then
        echo 0
        return
    fi
    awk -v name="$2" '$2 == name { n++ } END { print n + 0 }' "$1"
}

# dns_stub_start — start the stub. Returns 1 when there is no python3, which
# run.sh reports as three skipped cases; a stub that does not come up although
# python3 is there is an error of the harness (return 2).
dns_stub_start() {
    if ! command -v python3 > /dev/null 2>&1; then
        return 1
    fi
    mkdir -p "$DNS_WATCH"
    : > "$DNS_LOG"
    rm -f "$DNS_PORT_FILE"
    python3 "$HERE/dns-stub.py" "$DNS_LOG" "$DNS_PORT_FILE" > "$OUT/dns-stub.log" 2>&1 &
    dns_stub_pid=$!
    dns_waited=0
    while [ ! -s "$DNS_PORT_FILE" ] && [ "$dns_waited" -lt 200 ]; do
        sleep 0.05
        dns_waited=$((dns_waited + 1))
    done
    if [ ! -s "$DNS_PORT_FILE" ]; then
        return 2
    fi
    DNS_NAMESERVER="127.0.0.1:$(cat "$DNS_PORT_FILE")"
    return 0
}

dns_stub_stop() {
    if [ -n "$dns_stub_pid" ]; then
        kill -TERM "$dns_stub_pid" 2> /dev/null || true
        wait "$dns_stub_pid" 2> /dev/null || true
        dns_stub_pid=""
    fi
}

# dns_watch — the watcher itself, run in the background by dns_watch_start.
#
# Two things, polled every tenth of a second: the first time held.esc3.test is
# held, copy the log (while-held.log); the first time allowed.esc3.test is
# held, copy the log (before-allow.log), note the time and allow the flow. The
# hold timeout of the run is two seconds, so the decision has to come within
# them; a watcher that is too late leaves no `decided` behind, and the case
# says so instead of passing.
dns_watch() {
    dns_watch_end=$(($(date +%s) + DNS_WATCH_SECONDS))
    dns_watch_held=""
    while [ "$(date +%s)" -lt "$dns_watch_end" ]; do
        if [ -z "$dns_watch_held" ]; then
            dns_watch_held=$(dns_held_flow "$DNS_HELD_HOST")
            if [ -n "$dns_watch_held" ]; then
                cp "$DNS_LOG" "$DNS_WATCH/while-held.log"
                printf '%s\n' "$dns_watch_held" > "$DNS_WATCH/held.id"
            fi
        fi
        dns_watch_allowed=$(dns_held_flow "$DNS_ALLOWED_HOST")
        if [ -n "$dns_watch_allowed" ]; then
            cp "$DNS_LOG" "$DNS_WATCH/before-allow.log"
            printf '%s\n' "$dns_watch_allowed" > "$DNS_WATCH/allowed.id"
            dns_now_ms > "$DNS_WATCH/decide.ms"
            if dns_cli --json flows decide "$dns_watch_allowed" allow \
                > "$DNS_WATCH/decide.out" 2>&1; then
                : > "$DNS_WATCH/decided"
            fi
            return 0
        fi
        sleep 0.1
    done
}

dns_watch_start() {
    if [ -n "$dns_stub_pid" ]; then
        dns_watch &
        dns_watch_pid=$!
    fi
}

dns_watch_stop() {
    if [ -n "$dns_watch_pid" ]; then
        kill "$dns_watch_pid" 2> /dev/null || true
        wait "$dns_watch_pid" 2> /dev/null || true
        dns_watch_pid=""
    fi
}

# dns_host_cases — the three verdicts, written straight into the summary.
dns_host_cases() {
    echo "== esc-3 (host) =="
    if [ -z "$DNS_NAMESERVER" ]; then
        for dns_case in dns_not_before_decision dns_after_allow_once meta_no_dns_lookup; do
            record_case esc-3 "$dns_case" skip \
                "no python3 on the host, so no recording name server; the proof cannot be taken"
        done
        return
    fi

    dns_lines=$(wc -l < "$DNS_LOG" | tr -d ' ')
    dns_allowed=$(dns_count "$DNS_LOG" "$DNS_ALLOWED_HOST")
    if [ "$dns_allowed" -ge 1 ]; then
        dns_control=""
    else
        dns_control="no positive control: dns.log ($dns_lines lines) has no question for $DNS_ALLOWED_HOST, so the daemon never asked the stub, and a name that is missing from the log proves nothing"
    fi

    # --- 1: nothing is resolved before a decision ---------------------------
    if [ -n "$dns_control" ]; then
        record_case esc-3 dns_not_before_decision fail "$dns_control"
    elif [ ! -f "$DNS_WATCH/while-held.log" ]; then
        record_case esc-3 dns_not_before_decision fail \
            "the watcher never saw $DNS_HELD_HOST held, so the log was never read while it waited; see target/escape/esc-3.log"
    elif [ ! -f "$DNS_WATCH/before-allow.log" ]; then
        record_case esc-3 dns_not_before_decision fail \
            "the watcher never saw $DNS_ALLOWED_HOST held"
    else
        dns_leaked=""
        dns_n=$(dns_count "$DNS_WATCH/while-held.log" "$DNS_HELD_HOST")
        [ "$dns_n" -eq 0 ] || dns_leaked="$dns_leaked $DNS_HELD_HOST while it was held ($dns_n);"
        dns_n=$(dns_count "$DNS_WATCH/before-allow.log" "$DNS_ALLOWED_HOST")
        [ "$dns_n" -eq 0 ] || dns_leaked="$dns_leaked $DNS_ALLOWED_HOST before its decision ($dns_n);"
        for dns_host in "$DNS_HELD_HOST" $DNS_UNDECIDED_HOSTS; do
            dns_n=$(dns_count "$DNS_LOG" "$dns_host")
            [ "$dns_n" -eq 0 ] || dns_leaked="$dns_leaked $dns_host, never allowed ($dns_n);"
        done
        if [ -z "$dns_leaked" ]; then
            record_case esc-3 dns_not_before_decision pass \
                "while $DNS_HELD_HOST was held dns.log had $(wc -l < "$DNS_WATCH/while-held.log" | tr -d ' ') lines, none for it; before the decision on $DNS_ALLOWED_HOST none for it; no line at all for $DNS_HELD_HOST $DNS_UNDECIDED_HOSTS; control: $dns_allowed line for $DNS_ALLOWED_HOST"
        else
            record_case esc-3 dns_not_before_decision fail "resolved without a decision:$dns_leaked"
        fi
    fi

    # --- 2: after the one allow, that name once, and nothing after it ---------
    if [ ! -f "$DNS_WATCH/decided" ]; then
        record_case esc-3 dns_after_allow_once fail \
            "the watcher did not allow $DNS_ALLOWED_HOST within the hold timeout: $(cat "$DNS_WATCH/decide.out" 2> /dev/null || echo 'never saw it held')"
    else
        dns_decide_ms=$(cat "$DNS_WATCH/decide.ms")
        dns_allowed_id=$(cat "$DNS_WATCH/allowed.id")
        dns_line=$(awk -v name="$DNS_ALLOWED_HOST" '$2 == name' "$DNS_LOG" | head -n 1)
        dns_line_ms=$(printf '%s\n' "$dns_line" | awk '{ print $1 + 0 }')
        dns_last=$(tail -n 1 "$DNS_LOG")
        dns_error=$(dns_cli --json flows show "$dns_allowed_id" 2> /dev/null |
            python3 -c 'import json, sys; print(json.load(sys.stdin).get("error", ""))' \
                2> /dev/null || true)
        if [ "$dns_allowed" -ne 1 ]; then
            record_case esc-3 dns_after_allow_once fail \
                "dns.log has $dns_allowed questions for $DNS_ALLOWED_HOST, expected exactly one: $(awk -v name="$DNS_ALLOWED_HOST" '$2 == name' "$DNS_LOG")"
        elif [ "$dns_line_ms" -le "$dns_decide_ms" ]; then
            record_case esc-3 dns_after_allow_once fail \
                "the question for $DNS_ALLOWED_HOST ($dns_line) is not after the decision at $dns_decide_ms"
        elif [ "$dns_last" != "$dns_line" ]; then
            record_case esc-3 dns_after_allow_once fail \
                "dns.log goes on after the allowed name; last line: $dns_last"
        elif [ "$dns_error" != upstream_dns ]; then
            record_case esc-3 dns_after_allow_once fail \
                "flows show $dns_allowed_id says error=\"$dns_error\", expected upstream_dns"
        else
            record_case esc-3 dns_after_allow_once pass \
                "one question, $((dns_line_ms - dns_decide_ms)) ms after the decision at $dns_decide_ms: $dns_line; nothing after it; flows show $dns_allowed_id: error=upstream_dns"
        fi
    fi

    # --- 3: the meta endpoint is answered without a name service ------------
    dns_meta=$(dns_count "$DNS_LOG" humanitl.internal)
    if [ -n "$dns_control" ]; then
        record_case esc-3 meta_no_dns_lookup fail "$dns_control"
    elif ! grep -q '^RESULT esc-3 meta_status pass ' "$RESULTS"; then
        record_case esc-3 meta_no_dns_lookup fail \
            "ESC-3 got no status from humanitl.internal (meta_status is not pass), so a missing question proves nothing"
    elif [ "$dns_meta" -ne 0 ]; then
        record_case esc-3 meta_no_dns_lookup fail \
            "dns.log has $dns_meta questions for humanitl.internal"
    else
        record_case esc-3 meta_no_dns_lookup pass \
            "ESC-3 was answered by humanitl.internal (meta_status pass) and dns.log ($dns_lines lines) has no question for it"
    fi
}
