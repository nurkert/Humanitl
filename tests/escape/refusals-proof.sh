# shellcheck shell=sh
# Sourced by run.sh after the suites in the sandbox (HUM-138): the case
# esc-1/refusals_reported.
#
# ESC-1 proves that the filter refuses socket(AF_UNIX) and
# socket(AF_INET, SOCK_DGRAM) with EPERM. This case proves the other half of
# the promise "you see what your agent does": the refusal is also reported.
# It starts one more sandbox the way the suites start theirs
# (escape_sandbox_run in launch.sh, for the backend of the run), lets it try
# exactly three AF_UNIX sockets and two datagram sockets, and reads what the
# launcher says after the run. `humanitl sandbox run` and escape-launch print
# the same lines (humanitl_sandbox::Refusal::summary_line, HUM-435). The
# numbers must be exact: a report that counted the shim's own probes, or lost
# an attempt, would be a report nobody can trust.
#
# Needs from run.sh: OUT, record_case and everything escape_sandbox_run needs.
# Without python3 on the host the sandbox has none either (it sees the host's
# /usr), and the case is a skip, never a pass.

REFUSALS_UNIX=3
REFUSALS_DGRAM=2

refusals_case() {
    refusals_log="$OUT/refusals.log"
    refusals_python=""
    for candidate in /usr/bin/python3 /bin/python3; do
        if [ -x "$candidate" ]; then
            refusals_python="$candidate"
            break
        fi
    done
    if [ -z "$refusals_python" ]; then
        record_case esc-1 refusals_reported skip "no python3 on this machine; nothing in the sandbox can call socket(2)"
        return
    fi
    # One process, exact counts, and an exit code that says whether every
    # attempt came back EPERM.
    refusals_script="import errno, socket, sys
def probe(*a):
    try:
        socket.socket(*a).close()
        return 0
    except OSError as e:
        return e.errno
codes = [probe(socket.AF_UNIX, socket.SOCK_STREAM) for _ in range($REFUSALS_UNIX)]
codes += [probe(socket.AF_INET, socket.SOCK_DGRAM) for _ in range($REFUSALS_DGRAM)]
sys.exit(0 if all(c == errno.EPERM for c in codes) else 9)"
    set +e
    escape_sandbox_run "$refusals_log" "$refusals_python" -c "$refusals_script"
    refusals_code=$?
    set -e
    if [ "$refusals_code" != 0 ]; then
        record_case esc-1 refusals_reported fail \
            "the attempts did not all come back EPERM, or the run failed (exit $refusals_code); see target/escape/refusals.log"
        return
    fi
    refusals_unix=$(grep -cxF "  ${REFUSALS_UNIX}x socket(AF_UNIX, SOCK_STREAM)  family not allowed" "$refusals_log" || true)
    refusals_dgram=$(grep -cxF "  ${REFUSALS_DGRAM}x socket(AF_INET, SOCK_DGRAM)  type not allowed" "$refusals_log" || true)
    refusals_lines=$(grep -E '^  [0-9]+x socket\(' "$refusals_log" | tr '\n' ';')
    if [ "$refusals_unix" = 1 ] && [ "$refusals_dgram" = 1 ]; then
        record_case esc-1 refusals_reported pass \
            "refused and reported: $refusals_lines"
    else
        record_case esc-1 refusals_reported fail \
            "expected ${REFUSALS_UNIX}x AF_UNIX and ${REFUSALS_DGRAM}x AF_INET/SOCK_DGRAM in the report, got: ${refusals_lines:-nothing}; see target/escape/refusals.log"
    fi
}
