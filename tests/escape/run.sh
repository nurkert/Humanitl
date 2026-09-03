#!/usr/bin/env sh
# The escape suite, end to end. `make escape` calls this.
#
#   ./tests/escape/run.sh                 run everything, fail on a red probe
#   ESCAPE_ALLOW_FAIL=1 ./tests/escape/run.sh   report, but do not fail the build
#   ESCAPE_SKIP_BUILD=1 ./tests/escape/run.sh   use the escape-launch binary as built
#
# The launcher and the shim are built with cargo and read back from the same
# target directory: CARGO_TARGET_DIR when it is set (a relative one counts
# from daemon/, where cargo runs), daemon/target otherwise. The launcher is a
# debug build; the shim is built the way it ships, with the `shim` profile and
# statically linked, and handed to escape-launch with --shim. escape-launch
# uses it only when it speaks the contract (exit 125 without arguments,
# HUM-012); a stub is ignored, with a line on stderr, and every seccomp probe
# stays red. Two cases in the report, harness/shim_static and
# harness/shim_size, hold the shim to the acceptance criteria of HUM-012:
# `ldd` says "not a dynamic executable", and the binary is under 2 MB.
#
# The launcher (HUM-011) runs the sandbox through the real BwrapBackend with
# a bound placeholder socket in place of the proxy. What still turns red
# depends on the shim's seccomp filter (HUM-012) and on the proxy
# (HUM-013/015); tests/escape/README.md names the issue that turns each
# probe green. CI therefore runs this job with ESCAPE_ALLOW_FAIL=1 until
# HUM-021.
#
# Exit codes, and the difference matters:
#
#   0  every case passed or was skipped (or ESCAPE_ALLOW_FAIL=1 was set)
#   1  the sandbox started and at least one probe came back red
#   2  the sandbox could not be started at all, or the harness selftest failed
#
# A 2 is never tolerated by ESCAPE_ALLOW_FAIL. "No bwrap on this machine" is a
# statement about the machine, not about the guarantee, and reporting it as a
# probe failure would be a lie in the other direction.
#
# Needs no root. The AppArmor sysctl below is attempted with `sudo -n` and
# skipped without one; on a machine that restricts unprivileged user namespaces
# and grants no sudo, bwrap fails and the run ends in exit 2, which says so.
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
HERE="$ROOT/tests/escape"
OUT="$ROOT/target/escape"
WORK="$OUT/work"
STATE="$OUT/state"
# cargo honours CARGO_TARGET_DIR and puts the binary there, not under
# daemon/target; a relative value counts from the directory cargo runs in,
# which is $ROOT/daemon below. Reading the binary from a fixed path would
# execute whatever stale copy sits under daemon/target while cargo builds
# somewhere else, and the suite would judge a launcher that was never rebuilt.
case "${CARGO_TARGET_DIR:-}" in
"") TARGET_DIR="$ROOT/daemon/target" ;;
/*) TARGET_DIR="$CARGO_TARGET_DIR" ;;
*)  TARGET_DIR="$ROOT/daemon/$CARGO_TARGET_DIR" ;;
esac
LAUNCH="$TARGET_DIR/debug/escape-launch"
PROFILE="$ROOT/profiles/sandbox/test.toml"
RESULTS="$OUT/results.txt"

# The shim is built the way it ships (HUM-012): the `shim` profile of
# daemon/Cargo.toml, statically linked, so that the sandbox needs no library of
# the host under /usr. The musl target is the first choice; without it the
# glibc target with +crt-static does the same job. `-C relocation-model=static`
# makes the result an ET_EXEC rather than a static PIE, which is what lets
# `ldd` answer "not a dynamic executable", the acceptance criterion of the
# issue. A debug shim would be dynamic and three times the size, and the
# suite would then measure a binary nobody ships.
MUSL_TARGET="$(uname -m)-unknown-linux-musl"
if [ -d "$(rustc --print target-libdir --target "$MUSL_TARGET" 2> /dev/null || true)" ]; then
    SHIM_TARGET="$MUSL_TARGET"
else
    SHIM_TARGET=""
fi
if [ -n "$SHIM_TARGET" ]; then
    SHIM="$TARGET_DIR/$SHIM_TARGET/shim/humanitl-shim"
else
    SHIM="$TARGET_DIR/shim/humanitl-shim"
fi
# Under 2 MB, measured in bytes, as the acceptance criterion says.
SHIM_MAX_BYTES=2097152

rm -rf "$OUT"
mkdir -p "$OUT" "$WORK" "$STATE"
: > "$RESULTS"

# One case, written straight into the summary, for things the suites themselves
# cannot report because they never ran.
harness_error=0
record_error() {
    printf 'RESULT %s %s error %s\n' "$1" "$2" "$3" >> "$RESULTS"
    printf '  %-5s %-28s %s\n' error "$1/$2" "$3"
    harness_error=1
}

# record_case SUITE NAME STATUS DETAIL — a case the host decides, in the same
# fixed format the suites inside the sandbox write. Used for the two
# properties of the shipped shim that can only be measured outside: that it is
# statically linked and that it stays under 2 MB.
#
# The detail is squeezed onto one line and cut, exactly as esc_record in
# lib.sh does it: `ldd` answers a dynamic binary with one line per library,
# and a result line that grew a second line would be read as a case that never
# ran.
record_case() {
    case_detail=$(printf '%s' "$4" | tr '\n\r\t' '   ' | cut -c 1-400)
    printf 'RESULT %s %s %s %s\n' "$1" "$2" "$3" "$case_detail" >> "$RESULTS"
    printf '  %-5s %-28s %s\n' "$3" "$1/$2" "$case_detail"
}

# Ubuntu 24.04 ships an AppArmor restriction that makes an unprivileged bwrap
# fail with "Permission denied" (HUM-002). On a CI runner sudo is passwordless;
# on a developer machine it usually is not, so -n rather than a prompt.
if [ -r /proc/sys/kernel/apparmor_restrict_unprivileged_userns ] &&
    [ "$(cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns)" = 1 ]; then
    if ! sudo -n sysctl -w kernel.apparmor_restrict_unprivileged_userns=0 > /dev/null 2>&1; then
        echo "escape: apparmor restricts unprivileged user namespaces and sudo -n is not available" >&2
        echo "escape: bwrap will probably fail; run: sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0" >&2
    fi
fi

# The marker never enters the sandbox. ESC-2 looks for the NAME in every
# /proc/*/environ; finding it means the host environment came along.
HUMANITL_ESCAPE_MARKER="host-$$"
export HUMANITL_ESCAPE_MARKER
# A held request must turn into a timeout-block quickly, so that curl in ESC-3
# gets to read the 403 body instead of waiting out the default five minutes.
HUMANITL_HOLD__TIMEOUT_SECS=2
export HUMANITL_HOLD__TIMEOUT_SECS

# Seed the project directory with a canary in every path the profile masks or
# covers with a tmpfs. ESC-2 greps for it: reading it inside means the mask did
# not take.
CANARY="HUMANITL_MASK_CANARY=$HUMANITL_ESCAPE_MARKER"
mkdir -p "$WORK/.git/hooks" "$WORK/.vscode" "$WORK/.idea" "$WORK/results"
printf 'export %s\n' "$CANARY" > "$WORK/.envrc"
printf '[user]\n\tname = %s\n' "$CANARY" > "$WORK/.git/config"
printf '#!/bin/sh\necho %s\n' "$CANARY" > "$WORK/.git/hooks/pre-commit"
printf '{"marker": "%s"}\n' "$CANARY" > "$WORK/.vscode/settings.json"
printf '%s\n' "$CANARY" > "$WORK/.idea/workspace.xml"

echo "== harness selftest =="
if ! sh "$HERE/selftest.sh"; then
    echo "escape: the harness selftest failed; its verdicts cannot be trusted" >&2
    record_error harness selftest "lib.sh or junit.sh does not behave as documented"
    sh "$HERE/junit.sh" "$RESULTS" > "$OUT/escape.xml" || true
    exit 2
fi

echo "== building the launcher and the shim =="
if [ "${ESCAPE_SKIP_BUILD:-0}" != 1 ]; then
    # The same directory the binaries are read from, passed explicitly so that
    # the build and the run cannot disagree on where they went.
    if ! (cd "$ROOT/daemon" && CARGO_TARGET_DIR="$TARGET_DIR" cargo build -p humanitl-sandbox --bin escape-launch); then
        record_error harness build "cargo build -p humanitl-sandbox --bin escape-launch failed"
        sh "$HERE/junit.sh" "$RESULTS" > "$OUT/escape.xml"
        exit 2
    fi
    # The shim as it ships: static, small, its own profile. Two rustc flags
    # rather than a target when the musl target is not installed; the escape
    # suite must not depend on a rustup component that a developer machine may
    # not have.
    set -- cargo rustc -p humanitl-shim --bin humanitl-shim --profile shim
    if [ -n "$SHIM_TARGET" ]; then
        set -- "$@" --target "$SHIM_TARGET"
        echo "escape: building the shim for $SHIM_TARGET"
    else
        echo "escape: $MUSL_TARGET is not installed, building with +crt-static instead"
    fi
    set -- "$@" -- -C target-feature=+crt-static -C relocation-model=static
    if ! (cd "$ROOT/daemon" && CARGO_TARGET_DIR="$TARGET_DIR" "$@"); then
        record_error harness shim_build "the static build of humanitl-shim failed"
        sh "$HERE/junit.sh" "$RESULTS" > "$OUT/escape.xml"
        exit 2
    fi
fi
if [ ! -x "$LAUNCH" ]; then
    record_error harness launcher "no escape-launch binary at $LAUNCH"
    sh "$HERE/junit.sh" "$RESULTS" > "$OUT/escape.xml"
    exit 2
fi

# The two properties of the shipped shim that only the host can measure
# (HUM-012 acceptance criteria): statically linked, and under 2 MB. They are
# cases in the report, not a silent build step, because a shim that turned
# dynamic would only show up in the sandbox as "no such file or directory"
# for a library nobody mounted.
if [ ! -x "$SHIM" ]; then
    record_error harness shim_binary "no shim binary at $SHIM"
    sh "$HERE/junit.sh" "$RESULTS" > "$OUT/escape.xml"
    exit 2
fi
if command -v ldd > /dev/null 2>&1; then
    shim_ldd=$(ldd "$SHIM" 2>&1 || true)
    case "$shim_ldd" in
    *"not a dynamic executable"*)
        record_case harness shim_static pass "ldd: $shim_ldd" ;;
    *)
        record_case harness shim_static fail "the shim is not static, ldd says: $shim_ldd" ;;
    esac
else
    record_case harness shim_static skip "no ldd on this machine; the linkage cannot be measured"
fi
shim_bytes=$(wc -c < "$SHIM" | tr -d ' ')
if [ "$shim_bytes" -lt "$SHIM_MAX_BYTES" ]; then
    record_case harness shim_size pass "$shim_bytes bytes, limit $SHIM_MAX_BYTES"
else
    record_case harness shim_size fail "$shim_bytes bytes, limit $SHIM_MAX_BYTES"
fi

# ESC-1 to ESC-3 run inside the sandbox, one bwrap per suite so that a suite
# that kills its shell cannot take the others with it.
for n in 1 2 3; do
    suite="esc-$n"
    script=$(basename "$(ls "$HERE/$suite-"*.sh)")
    echo "== $suite ($script) =="
    # Redirection, not `| tee`: a pipeline hands back the exit code of its last
    # element, and the one that matters here is the launcher's.
    set +e
    "$LAUNCH" \
        --profile "$PROFILE" \
        --tests-dir "$HERE" \
        --work "$WORK" \
        --state "$STATE" \
        --shim "$SHIM" \
        -- /bin/sh "/tests/escape/$script" > "$OUT/$suite.log" 2>&1
    launch_code=$?
    set -e
    cat "$OUT/$suite.log"

    file="$WORK/results/$suite.txt"
    if [ ! -f "$file" ]; then
        record_error "$suite" sandbox_not_started \
            "escape-launch exited $launch_code and left no result file; see target/escape/$suite.log"
        continue
    fi
    if ! grep -q "^SUITE-DONE $suite " "$file"; then
        record_error "$suite" sandbox_died \
            "the suite stopped before its last case; see target/escape/$suite.log"
    fi
    grep '^RESULT ' "$file" >> "$RESULTS" || true
done

# ESC-4 and ESC-5 are placeholders whose every case is skipped. They run on the
# host: a skip needs no isolation, and making a placeholder depend on the very
# launcher it waits for would hide it behind the first sandbox failure.
for n in 4 5; do
    suite="esc-$n"
    script=$(ls "$HERE/$suite-"*.sh)
    echo "== $suite (placeholder, on the host) =="
    set +e
    ESC_OUT_DIR="$WORK/results" ESC_RESULTS= sh "$script"
    set -e
    file="$WORK/results/$suite.txt"
    if [ -f "$file" ]; then
        grep '^RESULT ' "$file" >> "$RESULTS" || true
    else
        record_error "$suite" placeholder_not_run "$script wrote no result file"
    fi
done

sh "$HERE/junit.sh" "$RESULTS" > "$OUT/escape.xml"

# The status is field 4 of a fixed-format line. Counting on the field and never
# on a substring matters: the detail behind it is captured command output, and
# a passing probe whose evidence happens to say "fail" must not flip the build.
count_status() {
    awk -v want="$1" '$1 == "RESULT" && $4 == want { n++ } END { print n + 0 }' "$RESULTS"
}
total=$(awk '$1 == "RESULT" { n++ } END { print n + 0 }' "$RESULTS")
passed=$(count_status pass)
failed=$(count_status fail)
skipped=$(count_status skip)
errored=$(count_status error)

echo
echo "== escape summary =="
echo "  cases   $total"
echo "  passed  $passed"
echo "  failed  $failed"
echo "  skipped $skipped"
echo "  errors  $errored"
echo "  report  $OUT/escape.xml"
echo "  raw     $RESULTS"
if [ "$failed" -gt 0 ]; then
    echo
    echo "red probes:"
    awk '$1 == "RESULT" && $4 == "fail" { sub(/^RESULT /, "  "); print }' "$RESULTS"
    echo
    echo "Red is the expected state in Sprint 0. tests/escape/README.md names the"
    echo "issue that turns each probe green."
fi

# Exit 2 before exit 1: a run without a verdict must not be read as a run with
# one, in either direction. The <error> cases in escape.xml say which suite
# never started (sandbox_not_started) or stopped early (sandbox_died).
if [ "$harness_error" -ne 0 ]; then
    echo "escape: no verdict: the sandbox could not be started, or a suite did not run to its end" >&2
    echo "escape: see the <error> cases in $OUT/escape.xml; ESCAPE_ALLOW_FAIL does not cover this" >&2
    exit 2
fi
if [ "$failed" -gt 0 ] && [ "${ESCAPE_ALLOW_FAIL:-0}" != 1 ]; then
    exit 1
fi
exit 0
