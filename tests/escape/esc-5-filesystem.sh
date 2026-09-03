#!/bin/sh
# ESC-5 — filesystem, terminal and audit trail. PLACEHOLDER: every case skipped.
#
# The suite belongs to HUM-043 (symlink marking, masked paths), HUM-050 (the
# terminal widget with its escape sequences) and HUM-029 (the audit hash chain).
# The file exists now because docs/SECURITY.md and docs/THREAT-MODEL.md already
# point at ESC-5 and scripts/ci/lint-docs.sh checks that the file behind the
# reference is real. Runs on the HOST; see the note in esc-4-rules.sh.

set -u
ESC_LIB="${ESC_LIB:-$(dirname "$0")/lib.sh}"
# shellcheck source=tests/escape/lib.sh
. "$ESC_LIB"

esc_begin esc-5

skip symlink_out_of_work_is_marked "/work/x -> /home shows up in the session summary, HUM-043"
skip masked_path_stays_masked      "/work/.envrc and /work/.git/config stay empty, HUM-043"
skip osc52_does_not_reach_host     "OSC 52 leaves the host clipboard untouched, HUM-050"
skip osc8_and_title_are_inert      "OSC 8 and title sequences are disabled in the terminal, HUM-050"
skip audit_delete_is_detected      "deleting an entry breaks the hash chain, HUM-029"
skip audit_truncate_is_detected    "truncating the file is reported as truncation, HUM-029"

esc_end
