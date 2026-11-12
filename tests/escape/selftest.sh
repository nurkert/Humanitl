#!/usr/bin/env sh
# Selftest of the escape harness itself (HUM-006, "Tests").
#
# A security test suite whose verdict is inverted is worse than none: it reports
# green while the sandbox leaks. So before run.sh trusts lib.sh with a real
# sandbox, it runs the helpers against `true` and `false`, where the right answer
# is known, and checks that junit.sh turns those answers into well-formed XML.
#
# Runs on the host, needs nothing but a POSIX shell, awk and python3 (or
# xmllint) for the XML check. The socket-probe check uses python3 to bind a
# socket and bwrap to bind-mount it; without them it says so and moves on.
set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
TMP="${TMPDIR:-/tmp}/humanitl-escape-selftest.$$"
mkdir -p "$TMP" || exit 70
trap 'rm -rf "$TMP"' EXIT INT TERM

ESC_RESULTS="$TMP/results.txt"
export ESC_RESULTS
# shellcheck source=tests/escape/lib.sh
. "$HERE/lib.sh"

# --- exercise every helper on a command whose outcome is known ----------------

esc_begin selftest > "$TMP/stdout.log" 2>&1
{
    probe probe_on_failure false
    probe probe_on_success true
    probe probe_on_missing /nonexistent/helper-that-is-not-installed

    # The errno is the verdict, in the output line, not in the exit code.
    probe_eperm eperm_on_eperm     sh -c 'echo "AF_X/SOCK_Y refused: EPERM"; exit 1'
    probe_eperm eperm_on_success   sh -c 'echo "AF_X/SOCK_Y created"'
    probe_eperm eperm_on_other     sh -c 'echo "AF_X/SOCK_Y refused: EACCES"; exit 1'
    probe_eperm eperm_on_no_family sh -c 'echo "AF_X/SOCK_Y refused: EAFNOSUPPORT"; exit 1'
    probe_eperm eperm_on_crash     sh -c 'echo "Traceback (most recent call last)"; exit 1'
    probe_eperm eperm_on_missing   /nonexistent/helper-that-is-not-installed

    # Same shape, other line: `<syscall>: <ERRNO>`, and ENOSYS/EINVAL skip.
    probe_syscall syscall_on_eperm   sh -c 'echo "ptrace: EPERM"; exit 1'
    probe_syscall syscall_on_success sh -c 'echo "ptrace returned 0"'
    probe_syscall syscall_on_other   sh -c 'echo "io_uring_enter: ENOTSUP"; exit 1'
    probe_syscall syscall_on_enosys  sh -c 'echo "add_key: ENOSYS"; exit 1'
    probe_syscall syscall_on_einval  sh -c 'echo "keyctl: EINVAL"; exit 1'
    probe_syscall syscall_on_crash   sh -c 'echo "Traceback (most recent call last):"; exit 1'
    probe_syscall syscall_on_missing /nonexistent/helper-that-is-not-installed

    expect_ok ok_on_success true
    expect_ok ok_on_failure false
    expect_ok ok_on_missing /nonexistent/helper-that-is-not-installed

    expect_output out_match   '^hello$' echo hello
    expect_output out_nomatch '^bye$'   echo hello

    expect_only only_match    '^x$' printf 'x\nx\n'
    expect_only only_mismatch '^x$' printf 'x\ny\n'
    expect_only only_empty    '^x$' true

    expect_empty empty_ok  true
    expect_empty empty_bad echo noise

    skip skip_case    "recorded, not decided"
    skip xml_escaping 'angle <b> amp & quote " apos '"'"' done'

    esc_end
} >> "$TMP/stdout.log" 2>&1

# --- assert the verdicts ------------------------------------------------------

selftest_failures=0

selftest_expect() {
    st_name="$1"
    st_want="$2"
    st_got=$(awk -v n="$st_name" '$1 == "RESULT" && $3 == n { print $4 }' "$ESC_RESULTS")
    if [ "$st_got" != "$st_want" ]; then
        echo "selftest: $st_name is '$st_got', expected '$st_want'" >&2
        selftest_failures=$((selftest_failures + 1))
    fi
}

# A probe passes when the exfiltration attempt FAILS. Getting this pair the
# wrong way round is the single most expensive mistake this file can catch.
selftest_expect probe_on_failure pass
selftest_expect probe_on_success fail
# "not installed" is never "held".
selftest_expect probe_on_missing skip

# Only EPERM is the filter's answer. "This kernel has no such family" is a
# skip, any other errno and a helper that died without reporting one are red.
selftest_expect eperm_on_eperm     pass
selftest_expect eperm_on_success   fail
selftest_expect eperm_on_other     fail
selftest_expect eperm_on_no_family skip
selftest_expect eperm_on_crash     fail
selftest_expect eperm_on_missing   skip

# ENOSYS and EINVAL are "this kernel has no such syscall", a skip; ENOTSUP and
# friends are the kernel answering instead of the filter, red.
selftest_expect syscall_on_eperm   pass
selftest_expect syscall_on_success fail
selftest_expect syscall_on_other   fail
selftest_expect syscall_on_enosys  skip
selftest_expect syscall_on_einval  skip
selftest_expect syscall_on_crash   fail
selftest_expect syscall_on_missing skip

selftest_expect ok_on_success pass
selftest_expect ok_on_failure fail
selftest_expect ok_on_missing skip

selftest_expect out_match   pass
selftest_expect out_nomatch fail

selftest_expect only_match    pass
selftest_expect only_mismatch fail
selftest_expect only_empty    fail

selftest_expect empty_ok  pass
selftest_expect empty_bad fail

selftest_expect skip_case    skip
selftest_expect xml_escaping skip

if ! grep -q '^SUITE-DONE selftest 28$' "$ESC_RESULTS"; then
    echo "selftest: the sentinel line is missing or counts wrong:" >&2
    grep '^SUITE-DONE' "$ESC_RESULTS" >&2 || echo "  (no SUITE-DONE line at all)" >&2
    selftest_failures=$((selftest_failures + 1))
fi

# --- the socket probe can see a socket ----------------------------------------
#
# ESC-2 "exactly one socket, and it is the proxy" rests on esc_find_sockets, and
# the first version of that probe could not find the proxy socket at all: find
# trusted d_type over the bind mount, and -xdev hid every other mount. So the
# probe is run against a socket whose place is known before a sandbox is
# trusted with it: a plain one in a temporary directory, and, where bwrap can
# start here, the same socket bind-mounted over a tmpfs the way escape-launch
# mounts the proxy socket, next to a directory bind on another filesystem the
# way /work is. Neither may be missed. And a socket under dev/shm must show up
# while one directly under dev must not: /dev/shm is the writable tmpfs an agent
# can plant a socket in, and the second version of the probe pruned it together
# with the rest of /dev.

socket_probe_note="socket probe not checked"
if ! command -v python3 > /dev/null 2>&1; then
    echo "selftest: no python3, the socket probe was not checked" >&2
# Bound by a relative path from inside the directory: AF_UNIX paths are limited
# to 108 bytes, and TMPDIR alone can be longer than that.
elif ! mkdir -p "$TMP/sock/sub" "$TMP/mnt" ||
    ! (cd "$TMP/sock/sub" && python3 -c 'import socket; socket.socket(socket.AF_UNIX).bind("a.sock")'); then
    echo "selftest: cannot create a Unix socket under $TMP, the socket probe was not checked" >&2
else
    st_got=$(esc_find_sockets "$TMP/sock")
    if [ "$st_got" != "$TMP/sock/sub/a.sock" ]; then
        echo "selftest: esc_find_sockets does not find a plain socket; got: '$st_got'" >&2
        selftest_failures=$((selftest_failures + 1))
    fi
    socket_probe_note="socket probe sees a plain socket"

    # A dev tree of its own: shm is searched, everything else under dev is not.
    if ! mkdir -p "$TMP/root/dev/shm" "$TMP/root/dev/pts" ||
        ! (cd "$TMP/root/dev/shm" && python3 -c 'import socket; socket.socket(socket.AF_UNIX).bind("planted.sock")') ||
        ! (cd "$TMP/root/dev" && python3 -c 'import socket; socket.socket(socket.AF_UNIX).bind("log")') ||
        ! (cd "$TMP/root/dev/pts" && python3 -c 'import socket; socket.socket(socket.AF_UNIX).bind("hidden.sock")'); then
        echo "selftest: cannot plant sockets under $TMP/root/dev, the dev/shm check was not run" >&2
        selftest_failures=$((selftest_failures + 1))
    else
        st_got=$(esc_find_sockets "$TMP/root")
        if [ "$st_got" != "$TMP/root/dev/shm/planted.sock" ]; then
            echo "selftest: esc_find_sockets must list dev/shm/planted.sock and nothing else under dev; got: '$st_got'" >&2
            selftest_failures=$((selftest_failures + 1))
        fi
        socket_probe_note="socket probe sees a plain socket and one in dev/shm"
    fi

    st_want=$(printf '%s/mnt/proxy.sock\n%s/mnt/work/sub/a.sock\n' "$TMP" "$TMP")
    st_out=$(bwrap --ro-bind / / --dev /dev --proc /proc --unshare-user --die-with-parent \
        --tmpfs "$TMP/mnt" \
        --bind "$TMP/sock/sub/a.sock" "$TMP/mnt/proxy.sock" \
        --bind "$TMP/sock" "$TMP/mnt/work" \
        /bin/sh -c 'echo BWRAP-OK; . "$1"; esc_find_sockets "$2" | sort' sh "$HERE/lib.sh" "$TMP/mnt" 2>/dev/null)
    if printf '%s\n' "$st_out" | grep -qx BWRAP-OK; then
        st_got=$(printf '%s\n' "$st_out" | grep -vx BWRAP-OK)
        if [ "$st_got" != "$st_want" ]; then
            echo "selftest: esc_find_sockets misses a bind-mounted socket or one on another mount" >&2
            echo "selftest: wanted:" >&2
            printf '%s\n' "$st_want" | sed 's/^/  /' >&2
            echo "selftest: got:" >&2
            printf '%s\n' "$st_got" | sed 's/^/  /' >&2
            selftest_failures=$((selftest_failures + 1))
        fi
        socket_probe_note="socket probe sees plain, dev/shm and bind-mounted sockets"
    else
        echo "selftest: bwrap cannot start here, the bind-mount check of the socket probe was skipped"
    fi
fi

# --- the way into the sandbox, per backend (HUM-435) ---------------------------
#
# run.sh starts every suite through escape_sandbox_run in launch.sh and reads
# the result back with escape_collect. Both run here against stand-ins for the
# command line and for escape-launch, which write down their arguments: with
# HUMANITL_ESCAPE_BACKEND=podman the suite must go through escape-launch with
# that backend and the daemon's socket, never through the command line, and a
# launcher that cannot start the backend must leave an error, never a pass or
# a skip. The host half of the engine probes (engine-config-proof.sh) is
# checked the same way.

ST_LAUNCH="$TMP/launch"
mkdir -p "$ST_LAUNCH/bin" "$ST_LAUNCH/work/results" "$ST_LAUNCH/engine"
cat > "$ST_LAUNCH/bin/humanitl" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >> "$STANDIN_LOG.cli"
exit 0
EOF
cat > "$ST_LAUNCH/bin/escape-launch" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >> "$STANDIN_LOG.launcher"
if [ "$STANDIN_MODE" = results ]; then
    printf 'RESULT esc-9 standin pass ok\nSUITE-DONE esc-9 1\n' > "$STANDIN_WORK/results/esc-9.txt"
    exit 0
fi
echo "escape-launch: SANDBOX_034: the backend is not available on this machine" >&2
exit 3
EOF
chmod +x "$ST_LAUNCH/bin/humanitl" "$ST_LAUNCH/bin/escape-launch"

launch_failures=$(
    RESULTS="$ST_LAUNCH/results.txt"
    : > "$RESULTS"
    harness_error=0
    # shellcheck source=tests/escape/launch.sh
    . "$HERE/launch.sh"
    # shellcheck source=tests/escape/engine-config-proof.sh
    . "$HERE/engine-config-proof.sh"
    CLI="$ST_LAUNCH/bin/humanitl"
    LAUNCHER="$ST_LAUNCH/bin/escape-launch"
    PROFILE_NAME=test
    PROFILE_FILE="$ST_LAUNCH/profiles/test.toml"
    WORK="$ST_LAUNCH/work"
    STATE="$ST_LAUNCH/state"
    DAEMON_XDG="$ST_LAUNCH/daemon"
    DAEMON_PROXY_SOCK="$ST_LAUNCH/state/runtime/humanitl/proxy/proxy.sock"
    DAEMON_CA_DIR="$ST_LAUNCH/daemon/data/humanitl/ca"
    SHIM_INSTALLED="$ST_LAUNCH/bin/humanitl-shim"
    STANDIN_LOG="$ST_LAUNCH/argv"
    STANDIN_WORK="$WORK"
    export STANDIN_LOG STANDIN_WORK STANDIN_MODE
    n=0
    bad() {
        echo "selftest: $*" >&2
        n=$((n + 1))
    }

    for name in bwrap podman podman-krun; do
        escape_backend_valid "$name" || bad "escape_backend_valid refuses '$name'"
    done
    for name in "" Podman "-x" "a b" "a;b" "../x"; do
        ! escape_backend_valid "$name" || bad "escape_backend_valid accepts '$name'"
    done

    # Another backend: escape-launch with that backend, a suite that reports.
    ESCAPE_BACKEND=podman
    STANDIN_MODE=results
    escape_sandbox_run "$ST_LAUNCH/esc-9.log" /bin/sh /tests/escape/esc-9.sh
    code=$?
    escape_collect esc-9 "$code" > /dev/null
    [ "$code" = 0 ] || bad "the stand-in launcher exited $code"
    [ ! -e "$STANDIN_LOG.cli" ] || bad "HUMANITL_ESCAPE_BACKEND=podman went through the command line"
    for want in "--backend podman " "--proxy-socket $DAEMON_PROXY_SOCK " "--shim $SHIM_INSTALLED " \
        "--state $STATE " "--tests-dir $HERE " "-- /bin/sh /tests/escape/esc-9.sh"; do
        grep -qF -- "$want" "$STANDIN_LOG.launcher" 2>/dev/null ||
            bad "escape-launch was not called with '$want': $(cat "$STANDIN_LOG.launcher" 2>/dev/null)"
    done
    grep -qx 'RESULT esc-9 standin pass ok' "$RESULTS" || bad "escape_collect lost the result of esc-9"
    [ "$harness_error" = 0 ] || bad "a suite that reported set harness_error"

    # The backend cannot start: an error, never a pass or a skip.
    rm -f "$WORK/results/esc-9.txt"
    : > "$RESULTS"
    STANDIN_MODE=unavailable
    escape_sandbox_run "$ST_LAUNCH/esc-9.log" /bin/sh /tests/escape/esc-9.sh
    code=$?
    escape_collect esc-9 "$code" > /dev/null
    [ "$code" = 3 ] || bad "the unavailable stand-in exited $code, expected 3"
    awk '$1 == "RESULT" && $2 == "esc-9" && $3 == "sandbox_not_started" && $4 == "error" { found = 1 }
        END { exit !found }' "$RESULTS" ||
        bad "a backend that does not start is not an error: $(cat "$RESULTS")"
    grep -q 'escape-launch --backend podman exited 3' "$RESULTS" ||
        bad "the error does not name the launcher: $(cat "$RESULTS")"
    [ "$(awk '$4 == "pass" || $4 == "skip"' "$RESULTS")" = "" ] ||
        bad "a backend that does not start left a pass or a skip: $(cat "$RESULTS")"
    [ "$harness_error" = 1 ] || bad "a backend that does not start did not set harness_error"

    # bwrap: the command line, the user's path, and never escape-launch.
    rm -f "$STANDIN_LOG.launcher"
    ESCAPE_BACKEND=bwrap
    escape_sandbox_run "$ST_LAUNCH/esc-9.log" /bin/sh /tests/escape/esc-9.sh
    grep -qxF -- "-v sandbox run --profile test --work $WORK --tests-dir $HERE -- /bin/sh /tests/escape/esc-9.sh" \
        "$STANDIN_LOG.cli" 2>/dev/null || bad "bwrap did not go through humanitl sandbox run"
    [ ! -e "$STANDIN_LOG.launcher" ] || bad "bwrap went through escape-launch"

    # engine_hook_not_run: nothing for bwrap without a planted configuration, a
    # skip for an engine, red without the proof of planting or with the marker.
    engine_verdict() {
        : > "$RESULTS"
        engine_config_case > /dev/null
        awk '$3 == "engine_hook_not_run" { print $4 }' "$RESULTS"
    }
    unset HUMANITL_ESCAPE_ENGINE_CANARY_DIR
    ESCAPE_BACKEND=bwrap
    [ "$(engine_verdict)" = "" ] || bad "engine_hook_not_run recorded for bwrap without a planted configuration"
    ESCAPE_BACKEND=podman
    [ "$(engine_verdict)" = skip ] || bad "engine_hook_not_run without a planted configuration is not a skip"
    HUMANITL_ESCAPE_ENGINE_CANARY_DIR="$ST_LAUNCH/engine"
    [ "$(engine_verdict)" = fail ] || bad "engine_hook_not_run without the proof of planting is not red"
    : > "$ST_LAUNCH/engine/planted"
    [ "$(engine_verdict)" = pass ] || bad "engine_hook_not_run with a planted configuration and no marker is not green"
    : > "$ST_LAUNCH/engine/hook-ran"
    [ "$(engine_verdict)" = fail ] || bad "engine_hook_not_run with the marker of the hook is not red"

    echo "$n"
)
if [ "${launch_failures:-1}" != 0 ]; then
    echo "selftest: the way into the sandbox per backend has ${launch_failures:-an unknown number of} problem(s)" >&2
    selftest_failures=$((selftest_failures + 1))
fi

# --- assert that junit.sh turns them into well-formed XML ---------------------

if ! sh "$HERE/junit.sh" "$ESC_RESULTS" > "$TMP/escape.xml"; then
    echo "selftest: junit.sh failed" >&2
    selftest_failures=$((selftest_failures + 1))
fi

if command -v xmllint > /dev/null 2>&1; then
    if ! xmllint --noout "$TMP/escape.xml"; then
        echo "selftest: junit.sh produced XML that xmllint rejects" >&2
        selftest_failures=$((selftest_failures + 1))
    fi
elif command -v python3 > /dev/null 2>&1; then
    if ! python3 -c 'import sys, xml.etree.ElementTree as e; e.parse(sys.argv[1])' "$TMP/escape.xml"; then
        echo "selftest: junit.sh produced XML that the parser rejects" >&2
        selftest_failures=$((selftest_failures + 1))
    fi
else
    echo "selftest: neither xmllint nor python3, XML was not parsed" >&2
fi

for st_want in 'tests="28"' 'failures="12"' 'skipped="9"' 'errors="0"'; do
    if ! grep -q "$st_want" "$TMP/escape.xml"; then
        echo "selftest: the testsuite element does not carry $st_want" >&2
        sed -n '2p' "$TMP/escape.xml" >&2
        selftest_failures=$((selftest_failures + 1))
    fi
done

# The raw angle bracket from xml_escaping must not survive into the document.
if grep -q '<b>' "$TMP/escape.xml"; then
    echo "selftest: junit.sh did not escape markup in a detail" >&2
    selftest_failures=$((selftest_failures + 1))
fi

if [ "$selftest_failures" -ne 0 ]; then
    echo "selftest: $selftest_failures problem(s); the harness cannot be trusted" >&2
    cat "$TMP/stdout.log" >&2
    exit 1
fi

echo "selftest: ok (28 cases, junit.sh well-formed, $socket_probe_note, the launcher of each backend checked)"
