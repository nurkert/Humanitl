#!/bin/sh
# ESC-4 — the rule table (BACKLOG.md 4.5, test 4).
#
# The suite belongs to HUM-022. Every case here asks the rule engine one of the
# questions the security claim rests on: does the first matching rule win, does
# a session rule come before a persistent one, is a host glob compared label by
# label, does a homograph stay a different name, does an IP literal ever match a
# name pattern, does an unknown method fall through to Ask, is a WebSocket
# upgrade its own decision, and does a body over the cap get refused even where
# a rule says allow.
#
# The engine is a pure crate: no IO, no async, so there is no binary that reads
# a rules file from the command line yet. Each case therefore runs the matching
# test of `daemon/crates/rules/tests/escape_table.rs`, which carries the same
# name and evaluates `tests/fixtures/esc4.yaml`. Once `humanitl rules test URL`
# exists (HUM-065), the script can additionally take the path the user takes.
#
# `rule_body_over_cap` has two halves and needs both: the engine says `allow`
# for the host in question, and the running proxy answers `413` with
# `reason: body_cap` regardless (HUM-016, ADR-005). The cap is decided before a
# rule is asked, and no rule lifts it. The probe speaks to the proxy over its
# unix socket with the same bytes curl sends through the bridge inside the
# sandbox (see esc-3-egress.sh); run.sh hands it the socket and the cap of this
# run in ESC_PROXY_SOCK and ESC_BODY_CAP.
#
# Unlike ESC-1 to ESC-3 this runs on the HOST, not in the sandbox: the rule
# engine decides before anything leaves the machine, and a decision needs no
# isolation to be measured. What the sandbox holds is ESC-1 to ESC-3.

set -u
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ESC_LIB="${ESC_LIB:-$HERE/lib.sh}"
# shellcheck source=tests/escape/lib.sh
. "$ESC_LIB"

DAEMON_DIR=${ESC_DAEMON_DIR:-$HERE/../../daemon}
PROXY_SOCK=${ESC_PROXY_SOCK:-}
BODY_CAP=${ESC_BODY_CAP:-1024}

esc_begin esc-4

# rules_case NAME — run the test of that name and print its result line.
#
# The exit code alone would not do: a filter that matches nothing leaves
# libtest with "0 passed" and exit 0, which would be a green case without a
# single assertion. The probe therefore looks for "1 passed; 0 failed".
rules_case() {
    (
        cd "$DAEMON_DIR" 2>/dev/null || exit 127
        cargo test -p humanitl-rules --test escape_table -- --exact "$1" 2>&1
    )
}

# body_cap_case — what the running proxy answers to a body over the cap.
#
# Two requests: one byte over the cap, and exactly the cap. The second is what
# makes the first mean something — at the cap the request is held and becomes
# the timeout block of this run (run.sh sets a hold timeout of two seconds), so
# the 413 is provably the cap and not a blanket refusal. Without a socket or
# without python3 the case is a skip (exit 127): "no daemon" must never read as
# "the cap held".
body_cap_case() {
    if [ -z "$PROXY_SOCK" ] || [ ! -S "$PROXY_SOCK" ]; then
        echo "no proxy socket at '${PROXY_SOCK:-<unset>}'; nothing to ask"
        exit 127
    fi
    if ! command -v python3 > /dev/null 2>&1; then
        echo "no python3 on this image; the proxy cannot be asked"
        exit 127
    fi
    python3 "$HERE/body_cap.py" "$PROXY_SOCK" "$BODY_CAP"
}

# rule_body_over_cap — both halves in one line of evidence.
body_cap_probe() {
    engine=$(rules_case rule_body_over_cap)
    case "$engine" in
        *"1 passed; 0 failed"*)
            ;;
        *)
            echo "the rule engine does not allow this host, the probe would prove nothing: $engine"
            return 1
            ;;
    esac
    proxy=$(body_cap_case)
    proxy_code=$?
    if [ "$proxy_code" -ne 0 ]; then
        echo "$proxy"
        return "$proxy_code"
    fi
    echo "allow_rule=matched $proxy"
}

if ! command -v cargo > /dev/null 2>&1; then
    for case_name in \
        rule_table_first_match_wins \
        rule_session_before_persistent \
        rule_host_glob_labels \
        rule_homograph_host \
        rule_ip_literal_host \
        rule_unknown_method_asks \
        rule_websocket_upgrade \
        rule_body_over_cap
    do
        skip "$case_name" "no cargo on this image; the rule engine cannot be asked"
    done
else
    # One build up front, so a compile error is reported once and every case
    # after it measures the engine instead of the compiler.
    if ! (cd "$DAEMON_DIR" && cargo test -q -p humanitl-rules --test escape_table --no-run) \
        > "${TMPDIR:-/tmp}/esc-4-build.log" 2>&1; then
        echo "esc-4: the rule tests do not build; see ${TMPDIR:-/tmp}/esc-4-build.log" >&2
    fi

    expect_output rule_table_first_match_wins \
        '1 passed; 0 failed' rules_case rule_table_first_match_wins
    expect_output rule_session_before_persistent \
        '1 passed; 0 failed' rules_case rule_session_before_persistent
    expect_output rule_host_glob_labels \
        '1 passed; 0 failed' rules_case rule_host_glob_labels
    expect_output rule_homograph_host \
        '1 passed; 0 failed' rules_case rule_homograph_host
    expect_output rule_ip_literal_host \
        '1 passed; 0 failed' rules_case rule_ip_literal_host
    expect_output rule_unknown_method_asks \
        '1 passed; 0 failed' rules_case rule_unknown_method_asks
    expect_output rule_websocket_upgrade \
        '1 passed; 0 failed' rules_case rule_websocket_upgrade
    expect_output rule_body_over_cap \
        '^allow_rule=matched over_cap=413/body_cap at_cap=(504|403)/' body_cap_probe
fi

esc_end
