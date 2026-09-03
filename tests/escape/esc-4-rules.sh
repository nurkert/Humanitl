#!/bin/sh
# ESC-4 — the rule table. PLACEHOLDER: every case is recorded as skipped.
#
# The suite belongs to HUM-022, which brings the rule evaluation it would test.
# The file exists now for two reasons: docs/SECURITY.md and docs/THREAT-MODEL.md
# already point at ESC-4 and scripts/ci/lint-docs.sh checks that every referenced
# ESC-N names a real file, and a claim that nobody records is a claim that
# quietly disappears. A skipped case in escape.xml keeps it in the report.
#
# Unlike ESC-1 to ESC-3 this runs on the HOST, not in the sandbox: a skip needs
# no isolation, and pretending otherwise would make the placeholder depend on the
# very launcher it is waiting for.

set -u
ESC_LIB="${ESC_LIB:-$(dirname "$0")/lib.sh}"
# shellcheck source=tests/escape/lib.sh
. "$ESC_LIB"

esc_begin esc-4

skip rule_table_first_match_wins     "rule evaluation arrives with HUM-022"
skip rule_session_before_persistent  "session rules take precedence (CONVENTIONS 4.5), HUM-022"
skip rule_host_glob_labels           "label globs, ** including the apex (CONVENTIONS 3.3), HUM-022"
skip rule_homograph_host             "xn-- host is never matched by a glob (RULES_002), HUM-022"
skip rule_ip_literal_host            "HostName::Ip only matches ip:/cidr: rules, HUM-022"
skip rule_unknown_method_asks        "an unknown method falls through to Ask, HUM-022"
skip rule_websocket_upgrade          "match.upgrade: websocket, HUM-022"
skip rule_body_over_cap              "a body over limits.hold_body_cap_bytes is blocked with 413, HUM-016"

esc_end
