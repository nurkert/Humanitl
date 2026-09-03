# shellcheck shell=sh
# Shared helpers of the escape harness (HUM-006, CONVENTIONS.md 3.11 and 4.10).
#
# RED IS THE CORRECT STATE UNTIL SPRINT 1 CLOSES. The harness is written before
# the thing it guards: the launcher (HUM-011), the shim with its seccomp filter
# (HUM-012) and the proxy (HUM-013/015) do not exist yet, so every probe that
# depends on them reports `fail` with the evidence that made it fail. See
# tests/escape/README.md for the probe-by-probe list and the issue that turns
# each one green.
#
# The file is sourced twice over: once inside the sandbox by esc-1 to esc-3, and
# once on the host by selftest.sh and by the ESC-4/ESC-5 placeholders. It is
# therefore POSIX sh, uses no bashism, no `local`, and never assumes that a
# helper binary exists.
#
# Vocabulary. A "probe" is an exfiltration attempt: it passes when it FAILS.
# An expectation is an observation that must hold: it passes when it SUCCEEDS.
# Mixing the two up is the classic way to write a security test that is green
# because nothing ran, so the two families have different names:
#
#   probe NAME CMD...                pass when CMD exits non-zero
#   probe_eperm NAME CMD...          pass when CMD reports `refused: EPERM`, the filter's own answer
#   probe_syscall NAME CMD...        pass when CMD reports `<syscall>: EPERM`; ENOSYS/EINVAL is a skip
#   expect_ok NAME CMD...            pass when CMD exits zero
#   expect_output NAME PAT CMD...    pass when one output line matches -E PAT
#   expect_only NAME PAT CMD...      pass when output is non-empty and EVERY line matches
#   expect_empty NAME CMD...         pass when the output has no non-blank line
#   skip NAME REASON...              record a case that this sprint cannot decide
#   esc_find_sockets ROOT            the socket list ESC-2 measures; selftest.sh runs it too
#
# A command that is simply not installed exits 127. Every helper turns that into
# `skip`, never into `pass`: "the tool is missing" must never read as "the
# sandbox held". That single rule is what keeps a thin container image from
# producing a wall of false green.
#
# Result format, one line per case, read by junit.sh:
#
#   RESULT <suite> <name> <pass|fail|skip|error> <detail...>
#
# and one sentinel line per suite:
#
#   SUITE-DONE <suite> <case count>
#
# The sentinel is how run.sh tells "the sandbox never started" apart from "the
# sandbox started and a probe leaked": no sentinel means no verdict at all.

ESC_SUITE="${ESC_SUITE:-unknown}"
ESC_COUNT=0
# Details land in an XML attribute; long ones are cut so the file stays readable.
ESC_DETAIL_MAX=400

# esc_begin SUITE — name the suite and truncate its result file.
#
# ESC_RESULTS wins when it is set (the host runs it that way). Otherwise the
# file is derived from ESC_OUT_DIR, which defaults to /work/results: /work is the
# one writable mount the host can read back afterwards (the tmpfs on /tmp and
# /dev/shm dies with the sandbox), so run.sh picks the results up from there once
# bwrap has exited.
esc_begin() {
    ESC_SUITE="$1"
    if [ -z "${ESC_RESULTS:-}" ]; then
        ESC_RESULTS="${ESC_OUT_DIR:-/work/results}/$ESC_SUITE.txt"
    fi
    esc_dir=$(dirname "$ESC_RESULTS")
    mkdir -p "$esc_dir" 2>/dev/null || true
    if ! : > "$ESC_RESULTS" 2>/dev/null; then
        echo "escape: cannot write $ESC_RESULTS" >&2
        exit 70
    fi
    ESC_COUNT=0
    echo "== $ESC_SUITE =="
}

# esc_end — write the sentinel that proves the suite ran to the last line.
esc_end() {
    printf 'SUITE-DONE %s %s\n' "$ESC_SUITE" "$ESC_COUNT" >> "$ESC_RESULTS"
    echo "== $ESC_SUITE done, $ESC_COUNT cases =="
}

# esc_record NAME STATUS DETAIL... — append one result line and echo it.
esc_record() {
    esc_name="$1"
    esc_status="$2"
    shift 2
    esc_detail=$(printf '%s' "$*" | tr '\n\r\t' '   ' | cut -c "1-$ESC_DETAIL_MAX")
    printf 'RESULT %s %s %s %s\n' \
        "$ESC_SUITE" "$esc_name" "$esc_status" "$esc_detail" >> "$ESC_RESULTS"
    ESC_COUNT=$((ESC_COUNT + 1))
    printf '  %-5s %-28s %s\n' "$esc_status" "$esc_name" "$esc_detail"
}

# esc_capture CMD... — run CMD, keep stdout+stderr in ESC_OUT and the code in ESC_CODE.
esc_capture() {
    ESC_OUT=$("$@" 2>&1)
    ESC_CODE=$?
}

# probe NAME CMD... — an exfiltration attempt. It passes when the attempt fails.
probe() {
    esc_probe_name="$1"
    shift
    esc_capture "$@"
    if [ "$ESC_CODE" -eq 127 ]; then
        esc_record "$esc_probe_name" skip "no such helper on this image: $ESC_OUT"
    elif [ "$ESC_CODE" -ne 0 ]; then
        esc_record "$esc_probe_name" pass "denied, exit $ESC_CODE: $ESC_OUT"
    else
        esc_record "$esc_probe_name" fail "LEAK: the attempt succeeded: $ESC_OUT"
    fi
}

# probe_eperm NAME CMD... — an attempt the seccomp filter must refuse, and
# refuse with EPERM. CONVENTIONS 4.10 says what the filter answers for every
# family and type outside allow_families x allow_types: EPERM, nothing else.
# So the errno is the verdict, not the exit code, and CMD reports it as a line
# ending in `refused: <ERRNO>` (esc_socket in esc-1 does).
#
#   refused: EPERM          pass, the filter (or a dropped capability) said no
#   refused: EAFNOSUPPORT   skip, this kernel has no such family, so the call
#                           never reached a filter; "could not try" is not "held"
#   refused: <anything>     fail, something refused the call but not with the
#                           errno the guarantee names; the filter did not answer
#   no `refused:` line      fail: exit 0 is a leak, anything else is a helper
#                           that broke before it could report
#
# A plain `probe` would be green for EAFNOSUPPORT, EACCES or EINVAL alike, and
# "the kernel does not have vsock" would read as "the sandbox blocked vsock".
probe_eperm() {
    esc_eperm_name="$1"
    shift
    esc_capture "$@"
    if [ "$ESC_CODE" -eq 127 ]; then
        esc_record "$esc_eperm_name" skip "no such helper on this image: $ESC_OUT"
        return
    fi
    esc_eperm_errno=$(printf '%s\n' "$ESC_OUT" | sed -n 's/.*refused: \([A-Za-z0-9_=]*\)$/\1/p' | tail -n 1)
    if [ -z "$esc_eperm_errno" ]; then
        if [ "$ESC_CODE" -eq 0 ]; then
            esc_record "$esc_eperm_name" fail "LEAK: the attempt succeeded: $ESC_OUT"
        else
            esc_record "$esc_eperm_name" fail "no verdict, the helper exited $ESC_CODE without a refused: line: $ESC_OUT"
        fi
        return
    fi
    case "$esc_eperm_errno" in
    EPERM)
        esc_record "$esc_eperm_name" pass "denied with EPERM: $ESC_OUT" ;;
    EAFNOSUPPORT)
        esc_record "$esc_eperm_name" skip "kernel lacks family, the filter was never asked: $ESC_OUT" ;;
    *)
        esc_record "$esc_eperm_name" fail "refused with $esc_eperm_errno, not EPERM; the filter is not what answered: $ESC_OUT" ;;
    esac
}

# probe_syscall NAME CMD... — a syscall from deny_syscalls (CONVENTIONS 3.4)
# that the filter must refuse, and refuse with EPERM. CMD dials the syscall
# raw and reports the kernel's answer as a line `<syscall>: <ERRNO>`
# (esc_syscall in esc-1 does); a line of any other shape means the call went
# through. Like probe_eperm, the errno is the verdict, not the exit code.
#
#   <syscall>: EPERM           pass, the filter said no
#   <syscall>: ENOSYS          skip, this kernel does not have the syscall at
#   <syscall>: EINVAL          all (no CONFIG_KEYS, no io_uring, no such
#                              number), so the filter was never asked;
#                              "could not try" is not "held"
#   <syscall>: <anything>      fail, something refused the call but not with
#                              the errno the guarantee names. That something is
#                              the kernel doing its ordinary job: ENOTSUP for a
#                              descriptor that is no ring, ENOKEY for a key
#                              nobody added. The filter did not answer.
#   no `<syscall>: ERRNO` line fail: exit 0 is a leak, anything else is a
#                              helper that broke before it could report
#
# The skip set differs from probe_eperm on purpose: a socket family the kernel
# lacks says EAFNOSUPPORT, a syscall it lacks says ENOSYS.
probe_syscall() {
    esc_sc_name="$1"
    shift
    esc_capture "$@"
    if [ "$ESC_CODE" -eq 127 ]; then
        esc_record "$esc_sc_name" skip "no such helper on this image: $ESC_OUT"
        return
    fi
    esc_sc_errno=$(printf '%s\n' "$ESC_OUT" | sed -n 's/^[A-Za-z0-9_]*: \([A-Za-z0-9_=]*\)$/\1/p' | tail -n 1)
    if [ -z "$esc_sc_errno" ]; then
        if [ "$ESC_CODE" -eq 0 ]; then
            esc_record "$esc_sc_name" fail "LEAK: the attempt succeeded: $ESC_OUT"
        else
            esc_record "$esc_sc_name" fail "no verdict, the helper exited $ESC_CODE without a <syscall>: ERRNO line: $ESC_OUT"
        fi
        return
    fi
    case "$esc_sc_errno" in
    EPERM)
        esc_record "$esc_sc_name" pass "denied with EPERM: $ESC_OUT" ;;
    ENOSYS | EINVAL)
        esc_record "$esc_sc_name" skip "kernel lacks the syscall ($esc_sc_errno), the filter was never asked: $ESC_OUT" ;;
    *)
        esc_record "$esc_sc_name" fail "refused with $esc_sc_errno, not EPERM; the filter is not what answered: $ESC_OUT" ;;
    esac
}

# expect_ok NAME CMD... — an operation the sandbox must keep working.
expect_ok() {
    esc_ok_name="$1"
    shift
    esc_capture "$@"
    if [ "$ESC_CODE" -eq 127 ]; then
        esc_record "$esc_ok_name" skip "no such helper on this image: $ESC_OUT"
    elif [ "$ESC_CODE" -eq 0 ]; then
        esc_record "$esc_ok_name" pass "ok: $ESC_OUT"
    else
        esc_record "$esc_ok_name" fail "exit $ESC_CODE: $ESC_OUT"
    fi
}

# expect_output NAME PATTERN CMD... — at least one output line matches PATTERN.
#
# The exit code is deliberately ignored: what is being checked is the
# observation, not whether the tool that produced it liked its arguments.
expect_output() {
    esc_out_name="$1"
    esc_out_pat="$2"
    shift 2
    esc_capture "$@"
    if [ "$ESC_CODE" -eq 127 ]; then
        esc_record "$esc_out_name" skip "no such helper on this image: $ESC_OUT"
    elif printf '%s\n' "$ESC_OUT" | grep -qE "$esc_out_pat"; then
        esc_record "$esc_out_name" pass "matched /$esc_out_pat/: $ESC_OUT"
    else
        esc_record "$esc_out_name" fail "nothing matched /$esc_out_pat/, output was: $ESC_OUT"
    fi
}

# expect_only NAME PATTERN CMD... — output is non-empty and every line matches.
#
# "Only lo" and "only the proxy socket" are statements about the whole list, not
# about one line in it; expect_output would be green with a second interface
# right next to the first.
expect_only() {
    esc_only_name="$1"
    esc_only_pat="$2"
    shift 2
    esc_capture "$@"
    if [ "$ESC_CODE" -eq 127 ]; then
        esc_record "$esc_only_name" skip "no such helper on this image: $ESC_OUT"
        return
    fi
    esc_only_lines=$(printf '%s\n' "$ESC_OUT" | grep -v '^[[:space:]]*$')
    if [ -z "$esc_only_lines" ]; then
        esc_record "$esc_only_name" fail "no output at all, expected every line to match /$esc_only_pat/"
        return
    fi
    esc_only_bad=$(printf '%s\n' "$esc_only_lines" | grep -vE "$esc_only_pat")
    if [ -n "$esc_only_bad" ]; then
        esc_record "$esc_only_name" fail "unexpected next to /$esc_only_pat/: $esc_only_bad"
    else
        esc_record "$esc_only_name" pass "only /$esc_only_pat/: $esc_only_lines"
    fi
}

# expect_empty NAME CMD... — the command prints nothing but blank lines.
expect_empty() {
    esc_empty_name="$1"
    shift
    esc_capture "$@"
    if [ "$ESC_CODE" -eq 127 ]; then
        esc_record "$esc_empty_name" skip "no such helper on this image: $ESC_OUT"
        return
    fi
    esc_empty_lines=$(printf '%s\n' "$ESC_OUT" | grep -v '^[[:space:]]*$')
    if [ -z "$esc_empty_lines" ]; then
        esc_record "$esc_empty_name" pass "empty"
    else
        esc_record "$esc_empty_name" fail "expected nothing, got: $esc_empty_lines"
    fi
}

# skip NAME REASON... — a case this sprint cannot decide yet.
skip() {
    esc_skip_name="$1"
    shift
    esc_record "$esc_skip_name" skip "$*"
}

# esc_find_sockets ROOT — every Unix socket below ROOT, one path per line.
#
# The probe behind ESC-2 "exactly one socket, and it is the proxy". It lives
# here rather than in esc-2 so that selftest.sh runs the very same command
# against a socket whose place is known, before a sandbox is trusted with it.
# Two details carry the probe, and the first version of it got both wrong:
#
#   * -xtype s, not -type s. bwrap bind-mounts the proxy socket over an empty
#     regular file on its root tmpfs, and GNU find trusts readdir's d_type for a
#     bare -type, which still says "regular file" for that mount point. -xtype
#     forces the lstat that sees the socket behind the mount.
#   * no -xdev. /work, /tmp, /var/tmp, /dev/shm and /home/agent are separate
#     mounts, and a socket an agent plants in /work is exactly what must turn
#     up. /proc, /sys and /dev are kept out by name instead, which is all -xdev
#     was ever there for.
#   * /dev/shm is NOT pruned with the rest of /dev. It is a writable tmpfs, the
#     one place under /dev where an agent can plant a socket, and the second
#     version of this probe hid it along with /dev/pts. Every other entry of
#     /dev is pruned by name, so a socket in /dev/log on the host still stays
#     out of the list; selftest.sh plants one socket in dev/shm and one next to
#     it in dev, and only the first may show up.
esc_find_sockets() {
    esc_find_root="${1%/}"
    find "${esc_find_root:-/}" \
        \( -path "$esc_find_root/proc" -o -path "$esc_find_root/sys" \
           -o \( -path "$esc_find_root/dev/*" \
                 ! -path "$esc_find_root/dev/shm" ! -path "$esc_find_root/dev/shm/*" \) \) -prune \
        -o -xtype s -print 2>/dev/null
}
