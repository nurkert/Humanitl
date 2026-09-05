#!/bin/sh
# ESC-5 — filesystem, terminal and audit trail.
#
# The three filesystem cases belong to HUM-043 and are live since it landed;
# the terminal cases belong to HUM-050 and the audit cases to HUM-029, and they
# are still skipped. The file exists since Sprint 0 because docs/SECURITY.md and
# docs/THREAT-MODEL.md point at ESC-5 and scripts/ci/lint-docs.sh checks that
# the file behind the reference is real. Runs on the HOST; see the note in
# esc-4-rules.sh.
#
# The three live cases ask the same three questions the security claim about
# channel 1 rests on: does a symlink out of /work show up in the session
# summary, do the masked paths stay empty in the sandbox and unchanged on the
# host, and does a hook the agent writes stay inside the sandbox.
#
# Like ESC-4, each case runs the integration test of that name — here
# `daemon/crates/sandbox/tests/escape_worktree.rs`, which carries the same three
# names and drives the real launcher against a real bubblewrap. There is no
# command that could ask these questions from outside without starting half the
# daemon.

set -u
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ESC_LIB="${ESC_LIB:-$HERE/lib.sh}"
# shellcheck source=tests/escape/lib.sh
. "$ESC_LIB"

DAEMON_DIR=${ESC_DAEMON_DIR:-$HERE/../../daemon}
# The marker the integration test prints when bwrap is missing.
ESC_SKIP_MARKER="ESC5-SKIP"

esc_begin esc-5

# worktree_case NAME — run the integration test of that name and judge it.
#
# Three outcomes, and the middle one is what keeps this suite honest:
#
#   ESC5-SKIP <reason>   skip: two of the cases need bwrap and a kernel with
#                        unprivileged user namespaces. "The tool is missing"
#                        must never read as "the sandbox held" (lib.sh).
#   1 passed; 0 failed   pass. The count matters, not the exit code: a filter
#                        that matches nothing leaves libtest with "0 passed"
#                        and exit 0, which would be a green case without a
#                        single assertion.
#   anything else        fail, with the output as evidence.
worktree_case() {
    esc_wt_name="$1"
    esc_wt_out=$(
        cd "$DAEMON_DIR" 2>/dev/null || exit 127
        cargo test -p humanitl-sandbox --test escape_worktree \
            -- --exact "$esc_wt_name" --nocapture 2>&1
    )
    esc_wt_code=$?
    if [ "$esc_wt_code" -eq 127 ]; then
        esc_record "$esc_wt_name" skip \
            "no cargo, or no daemon directory at $DAEMON_DIR: $esc_wt_out"
        return
    fi
    case "$esc_wt_out" in
    *"$ESC_SKIP_MARKER"*)
        esc_record "$esc_wt_name" skip \
            "$(printf '%s\n' "$esc_wt_out" | grep -F "$ESC_SKIP_MARKER" | tail -n 1)"
        ;;
    *"1 passed; 0 failed"*)
        esc_record "$esc_wt_name" pass "the integration test of the same name holds"
        ;;
    *)
        esc_record "$esc_wt_name" fail "$esc_wt_out"
        ;;
    esac
}

worktree_case symlink_out_of_work_is_marked
worktree_case masked_path_stays_masked
worktree_case hooks_write_stays_in_sandbox

skip osc52_does_not_reach_host     "OSC 52 leaves the host clipboard untouched, HUM-050"
skip osc8_and_title_are_inert      "OSC 8 and title sequences are disabled in the terminal, HUM-050"
skip audit_delete_is_detected      "deleting an entry breaks the hash chain, HUM-029"
skip audit_truncate_is_detected    "truncating the file is reported as truncation, HUM-029"

esc_end
