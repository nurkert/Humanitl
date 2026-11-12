#!/bin/sh
# ESC-5 — filesystem, terminal and audit trail.
#
# The three filesystem cases belong to HUM-043, the two terminal cases to
# HUM-042 and the two audit cases to HUM-050; all are live since those issues
# landed. The file exists since Sprint 0 because
# docs/SECURITY.md and docs/THREAT-MODEL.md point at ESC-5 and
# scripts/ci/lint-docs.sh checks that the file behind the reference is real.
# Runs on the HOST; see the note in esc-4-rules.sh.
#
# The three filesystem cases ask the same three questions the security claim
# about channel 1 rests on: does a symlink out of /work show up in the session
# summary, do the masked paths stay empty in the sandbox and unchanged on the
# host, and does a hook the agent writes stay inside the sandbox.
#
# The two terminal cases ask the question channel 3 rests on: the agent writes
# OSC 52, OSC 8 and a window title at a real pseudo terminal, and none of them
# reaches a client. That is stronger than watching a clipboard: a sequence that
# never leaves the daemon cannot reach a terminal that would execute it
# (docs/SECURITY.md 3.3).
#
# The two audit cases ask what the audit chain promises (docs/SECURITY.md 8):
# a real daemon writes its chain and anchors it on stop, the test deletes an
# entry from the middle of that chain or cuts its anchored end, and the
# verifier, holding the daemon's key and the anchors from its database, reports
# both as a break. What the chain does not promise — a cut behind the last
# anchor while the daemon still runs — is its own test in the audit crate.
#
# Like ESC-4, each case runs the integration test of that name — the filesystem
# cases `daemon/crates/sandbox/tests/escape_worktree.rs`, the terminal cases
# `daemon/crates/ipc/tests/terminal.rs`, the audit cases
# `daemon/bin/humanitld/tests/daemon_end_to_end.rs`. The first two drive the
# real launcher against a real sandbox backend, the last one the real daemon;
# there is no command that could ask these questions from outside without
# starting half the daemon (`humanitl audit verify` arrives with HUM-070).
#
# Every backend (HUM-435). ESC_BACKEND, set by run.sh from
# HUMANITL_ESCAPE_BACKEND, names the backend of the run; empty or bwrap is the
# backend of the system, as before. For any other backend the filesystem and
# terminal cases start exactly that one: the integration tests read
# HUMANITL_ESCAPE_BACKEND and reach a level that is not admitted yet through
# Backend::detect_unadmitted, so cargo builds them with the feature
# unadmitted-backends, and only for such a run. That build is a cargo call of
# its own per package and never touches humanitl or humanitld, which refuse to
# build against it. A requested backend that is not usable fails the case,
# always: it is never a skip and never falls back to bwrap (acceptance
# criterion 2 of HUM-435).
#
# The two audit cases do not depend on the backend: they start a real daemon,
# send one flow through its proxy and read the chain it wrote, and no sandbox
# is started on the way. They run the same way under every backend.
#
# The disposable case (HUM-350) has one version per backend. Under bwrap it
# measures the overlay (daemon/crates/sandbox/tests/launcher.rs). A backend
# without a disposable sandbox (Podman until HUM-438) keeps the promise by
# refusing the plan instead of starting directly on the project; the case of
# the same name in escape_worktree.rs checks that refusal and turns red once
# the backend offers a disposable sandbox and needs the real measurement.

set -u
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ESC_LIB="${ESC_LIB:-$HERE/lib.sh}"
# shellcheck source=tests/escape/lib.sh
. "$ESC_LIB"

DAEMON_DIR=${ESC_DAEMON_DIR:-$HERE/../../daemon}
ESC_BACKEND=${ESC_BACKEND:-${HUMANITL_ESCAPE_BACKEND:-bwrap}}
# The cargo flags of a backend that is not admitted (see above); none for bwrap.
if [ "$ESC_BACKEND" = bwrap ]; then
    ESC_BACKEND_FEATURES=""
else
    ESC_BACKEND_FEATURES="--features unadmitted-backends"
fi
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
    backend_case "$1" humanitl-sandbox escape_worktree
}

# terminal_case NAME — the same for the terminal of the sandbox (HUM-042).
terminal_case() {
    backend_case "$1" humanitl-ipc terminal
}

# audit_case NAME — the same for the audit chain of a real daemon (HUM-050).
# No backend: see the head of this file.
audit_case() {
    cargo_case "$1" humanitld daemon_end_to_end
}

# backend_case NAME PACKAGE TEST — a case that starts a sandbox, against the
# backend of this run.
backend_case() {
    # Word splitting of the flags is intended: empty, or two words.
    # shellcheck disable=SC2086
    cargo_case "$1" "$2" "$3" $ESC_BACKEND_FEATURES
}

# cargo_case NAME PACKAGE TEST [CARGO FLAGS...] — run one integration test and
# judge it.
cargo_case() {
    esc_wt_name="$1"
    esc_wt_pkg="$2"
    esc_wt_test="$3"
    shift 3
    esc_wt_out=$(
        cd "$DAEMON_DIR" 2>/dev/null || exit 127
        # The tests read the backend from here; the audit cases ignore it.
        HUMANITL_ESCAPE_BACKEND="$ESC_BACKEND" \
            cargo test -p "$esc_wt_pkg" "$@" --test "$esc_wt_test" \
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

terminal_case osc52_does_not_reach_host
terminal_case osc8_and_title_are_inert

audit_case audit_delete_is_detected
audit_case audit_truncate_is_detected

# The disposable sandbox (HUM-350, docs/SECURITY.md 5.2, K-24): the agent
# writes, deletes and plants a symlink out of /work in a disposable run, and
# the project folder on the host stays byte for byte as it was; only the apply
# afterwards writes, and never the symlink or a setuid bit.
if [ "$ESC_BACKEND" = bwrap ]; then
    cargo_case a_disposable_sandbox_never_writes_to_the_project humanitl-sandbox launcher
else
    worktree_case a_disposable_sandbox_never_writes_to_the_project
fi

esc_end
