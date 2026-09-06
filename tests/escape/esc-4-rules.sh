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
# The engine is a pure crate: no IO, no async. Each `rule_*` case therefore runs
# the matching test of `daemon/crates/rules/tests/escape_table.rs`, which carries
# the same name and evaluates `tests/fixtures/esc4.yaml`.
#
# Since HUM-114 the same questions are asked a second time along the path a user
# takes: `rules_cli_01` to `rules_cli_15` are the first fifteen rows of the host
# table in HUM-022, each one a `humanitl rules test URL` against the daemon of
# this run, with the verdict read from the line and the exit code (0 allow, 10
# block, 11 ask) read from the process. Two answers to the same question, one
# from the engine and one from the whole way through the daemon; a difference
# between them would be the second rule engine ADR-018 forbids.
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
# Die Kommandozeile und der XDG-Baum des Laufs; run.sh reicht beides herein.
CLI=${ESC_CLI:-}
CLI_RUNTIME=${ESC_XDG_RUNTIME_DIR:-}
CLI_CONFIG=${ESC_XDG_CONFIG_HOME:-}
CLI_DATA=${ESC_XDG_DATA_HOME:-}
CLI_HOME=${ESC_HOME:-}
RULES_FIXTURE=${ESC_RULES_FIXTURE:-$DAEMON_DIR/crates/rules/tests/fixtures/esc4.yaml}

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

# --- the same table over the command line (HUM-114) ---------------------------
#
# The rule set of this run becomes the fixture, and it becomes it HERE and not
# in run.sh: until this line the daemon of the run answers with the rules ESC-1
# to ESC-3 need, and `blocked.example` must be held there, not allowed. The
# body-cap probe above is the last case that speaks to the live proxy, so
# nothing after this reload depends on the rules before it.
install_cli_rules() {
    if [ -z "$CLI" ] || [ ! -x "$CLI" ]; then
        echo "no humanitl binary at '${CLI:-<unset>}'"
        return 127
    fi
    if [ -z "$CLI_CONFIG" ]; then
        echo "no XDG config directory of this run; the daemon cannot be asked"
        return 127
    fi
    if [ ! -f "$RULES_FIXTURE" ]; then
        echo "no rule fixture at $RULES_FIXTURE"
        return 127
    fi
    mkdir -p "$CLI_CONFIG/humanitl" || return 1
    cp -f "$RULES_FIXTURE" "$CLI_CONFIG/humanitl/rules.yaml" || return 1
    cli rules reload
}

# cli ARGS... — the command line of this run, in the XDG tree of its daemon.
cli() {
    XDG_RUNTIME_DIR="$CLI_RUNTIME" \
        XDG_CONFIG_HOME="$CLI_CONFIG" \
        XDG_DATA_HOME="$CLI_DATA" \
        HOME="$CLI_HOME" \
        "$CLI" "$@"
}

# rules_cli URL [ARGS...] — one probe, as `exit=<n> verdict=<word>` plus what
# the command wrote. The exit code is part of the evidence and not only the
# verdict line: a script reads the number, a human reads the line, and the case
# is green only when both say the same thing.
rules_cli() {
    if [ -z "$CLI" ] || [ ! -x "$CLI" ]; then
        echo "no humanitl binary at '${CLI:-<unset>}'"
        exit 127
    fi
    rules_cli_url="$1"
    shift
    rules_cli_out=$(cli rules test "$rules_cli_url" "$@" 2>&1)
    rules_cli_code=$?
    rules_cli_verdict=$(printf '%s\n' "$rules_cli_out" | sed -n 's/^verdict: \(.*\)$/\1/p')
    printf 'exit=%s verdict=%s %s\n' \
        "$rules_cli_code" \
        "${rules_cli_verdict:-none}" \
        "$(printf '%s' "$rules_cli_out" | tr '\n' ' ')"
}

# llm_cli URL — the endpoint probe, as `exit=<n> code=<CODE>` plus its output.
llm_cli() {
    if [ -z "$CLI" ] || [ ! -x "$CLI" ]; then
        echo "no humanitl binary at '${CLI:-<unset>}'"
        exit 127
    fi
    llm_cli_out=$(cli llm test "$1" 2>&1)
    llm_cli_code=$?
    llm_cli_code_name=$(printf '%s\n' "$llm_cli_out" | sed -n 's/.*\[\(LLM_[0-9]*\)\].*/\1/p' | head -n 1)
    printf 'exit=%s code=%s %s\n' \
        "$llm_cli_code" \
        "${llm_cli_code_name:-none}" \
        "$(printf '%s' "$llm_cli_out" | tr '\n' ' ')"
}

cli_setup=$(install_cli_rules 2>&1)
cli_setup_code=$?
if [ "$cli_setup_code" -ne 0 ]; then
    n=1
    while [ "$n" -le 15 ]; do
        skip "$(printf 'rules_cli_%02d' "$n")" \
            "the rule set of this run could not be installed: $cli_setup"
        n=$((n + 1))
    done
    skip llm_cli_unreachable "the command line of this run is not available: $cli_setup"
else
    # Rows 1 to 7: `*.github.com` matches exactly one label, and a name is
    # compared label by label after normalisation.
    expect_output rules_cli_01 '^exit=0 verdict=allow' \
        rules_cli https://api.github.com/x
    expect_output rules_cli_02 '^exit=11 verdict=ask' \
        rules_cli https://github.com/x
    expect_output rules_cli_03 '^exit=11 verdict=ask' \
        rules_cli https://a.b.github.com/x
    expect_output rules_cli_04 '^exit=11 verdict=ask' \
        rules_cli https://evil-github.com/x
    expect_output rules_cli_05 '^exit=11 verdict=ask' \
        rules_cli https://github.com.evil.io/x
    expect_output rules_cli_06 '^exit=0 verdict=allow' \
        rules_cli https://API.GITHUB.COM./x
    expect_output rules_cli_07 '^exit=0 verdict=allow' \
        rules_cli https://api.github.com./x

    # Rows 8 to 12: `**.github.com` covers the apex and any depth. The rule of
    # the fixture that carries the double star is the POST block, so these five
    # ask with a method; that is also the first-match-wins case of row 9.
    expect_output rules_cli_08 '^exit=10 verdict=block' \
        rules_cli https://github.com/x --method POST
    expect_output rules_cli_09 '^exit=10 verdict=block' \
        rules_cli https://api.github.com/x --method POST
    expect_output rules_cli_10 '^exit=10 verdict=block' \
        rules_cli https://a.b.c.github.com/x --method POST
    expect_output rules_cli_11 '^exit=11 verdict=ask' \
        rules_cli https://github.com.evil.io/x --method POST
    expect_output rules_cli_12 '^exit=11 verdict=ask' \
        rules_cli https://notgithub.com/x --method POST

    # Rows 13 to 15: an exact pattern matches exactly one name, and case is not
    # part of the name.
    expect_output rules_cli_13 '^exit=10 verdict=block' \
        rules_cli https://exact.example/x
    expect_output rules_cli_14 '^exit=11 verdict=ask' \
        rules_cli https://www.exact.example/x
    expect_output rules_cli_15 '^exit=10 verdict=block' \
        rules_cli https://EXACT.Example/x

    # Nicht aus der Host-Tabelle, aber hier zu Hause: Dies ist die einzige
    # Stelle der Sammlung, an der ein echter Daemon steht, und `humanitl llm
    # test` gegen einen toten Port muss ein Befund sein und keine leere
    # Antwort. Der Port 1 gehoert keinem Dienst; die Probe geht ins Host-Netz
    # und nie durch die Sandbox.
    expect_output llm_cli_unreachable '^exit=1 code=LLM_001 ' \
        llm_cli http://127.0.0.1:1/
fi

esc_end
