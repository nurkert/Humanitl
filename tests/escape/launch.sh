# shellcheck shell=sh
# The host side of the escape harness: which way into the sandbox, and how a
# suite's results reach the summary (HUM-435).
#
# Sourced by run.sh, by refusals-proof.sh through run.sh, and by selftest.sh,
# which runs these functions against stand-ins before run.sh trusts them with
# a real sandbox. POSIX sh like lib.sh; the names start with escape_ or
# record_, so that they never meet the esc_ helpers of lib.sh.
#
# One suite, every backend. HUMANITL_ESCAPE_BACKEND picks the backend
# (default bwrap), and the scripts in the sandbox are the same for all of them:
#
#   bwrap   through `humanitl sandbox run --profile test`, the path a user
#           takes (CONVENTIONS.md 3.11).
#   other   through `escape-launch --backend <name>`. A level that is not
#           admitted yet is refused by `humanitl sandbox run` with SANDBOX_048
#           (backlog/sprint-7.md, the gate of the sprint), and only the escape
#           suite and the tests may start it, through an argument in code:
#           escape-launch is built only with the cargo feature
#           unadmitted-backends, which neither humanitld nor humanitl ever
#           enables. It gets the proxy socket, the CA and the bundle of the
#           daemon of this run, so ESC-3 measures the same proxy.
#
# A launcher that cannot start the sandbox leaves no result file, and that is
# an error, never a pass and never a skip: a backend that is missing on this
# machine is a run without a verdict.
#
# Needs from the caller: RESULTS, and for escape_sandbox_run ESCAPE_BACKEND,
# CLI, LAUNCHER, PROFILE_NAME, PROFILE_FILE, WORK, HERE, STATE, DAEMON_XDG,
# DAEMON_PROXY_SOCK, DAEMON_CA_DIR and SHIM_INSTALLED. harness_error is set
# to 1 by record_error.

# record_error SUITE NAME DETAIL — a case the suites themselves cannot report
# because they never ran, written straight into the summary.
record_error() {
    printf 'RESULT %s %s error %s\n' "$1" "$2" "$3" >> "$RESULTS"
    printf '  %-5s %-28s %s\n' error "$1/$2" "$3"
    harness_error=1
}

# record_case SUITE NAME STATUS DETAIL — a case the host decides, in the same
# fixed format the suites inside the sandbox write.
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

# escape_backend_valid NAME — a backend name as the launchers take it: lower
# case letters, digits and dashes. Anything else never reaches a command line.
escape_backend_valid() {
    case "$1" in
    "" | *[!a-z0-9-]* | -*) return 1 ;;
    *) return 0 ;;
    esac
}

# escape_launcher_name — the way into the sandbox, for the report.
escape_launcher_name() {
    if [ "$ESCAPE_BACKEND" = bwrap ]; then
        printf 'humanitl sandbox run'
    else
        printf 'escape-launch --backend %s' "$ESCAPE_BACKEND"
        if [ -n "${ESCAPE_IMAGE:-}" ]; then
            printf ' --image %s' "$ESCAPE_IMAGE"
        fi
    fi
}

# escape_sandbox_run LOG CMD... — run CMD in a sandbox of the chosen backend,
# stdout and stderr into LOG. Returns the launcher's exit code: the code of
# CMD, 3 for a sandbox that did not start or whose guarantees were not proven.
#
# The command line runs in the XDG tree of the daemon of this run, HOME
# included. escape-launch keeps the real HOME and XDG_RUNTIME_DIR: a container
# engine keeps its storage under HOME and its own state under the user's
# runtime directory, and a second storage next to the user's would break the
# user's one. It points its own runtime directory at STATE/runtime itself
# (--state), which is where the daemon of this run has its sockets.
escape_sandbox_run() {
    esr_log="$1"
    shift
    if [ "$ESCAPE_BACKEND" = bwrap ]; then
        XDG_RUNTIME_DIR="$STATE/runtime" \
            XDG_DATA_HOME="$DAEMON_XDG/data" \
            XDG_CONFIG_HOME="$DAEMON_XDG/config" \
            HOME="$DAEMON_XDG/home" \
            "$CLI" -v sandbox run \
            --profile "$PROFILE_NAME" \
            --work "$WORK" \
            --tests-dir "$HERE" \
            -- "$@" > "$esr_log" 2>&1
    else
        XDG_DATA_HOME="$DAEMON_XDG/data" \
            XDG_CONFIG_HOME="$DAEMON_XDG/config" \
            "$LAUNCHER" \
            --backend "$ESCAPE_BACKEND" \
            ${ESCAPE_IMAGE:+--image "$ESCAPE_IMAGE"} \
            --profile "$PROFILE_FILE" \
            --state "$STATE" \
            --work "$WORK" \
            --tests-dir "$HERE" \
            --proxy-socket "$DAEMON_PROXY_SOCK" \
            --ca-cert "$DAEMON_CA_DIR/ca.crt" \
            --ca-bundle "$DAEMON_CA_DIR/ca-bundle.crt" \
            --shim "$SHIM_INSTALLED" \
            -- "$@" > "$esr_log" 2>&1
    fi
}

# escape_host_ids FILE — device and inode of the host files that ESC-2
# compares from inside the sandbox (HUM-439): the daemon's CA and the bundle
# it writes, which must be the very files in the sandbox, and the host's /etc/shadow and
# /etc/machine-id, which must not be. One line per file that exists,
# `KEY DEV:INO`, and `image REF` when the root is an image (ESCAPE_IMAGE).
# The sandbox reads it under /work/results/host-ids.
#
# Needs DAEMON_CA_DIR, and WORK under the kernel level; ESCAPE_IMAGE and
# ESCAPE_BACKEND may be empty.
escape_host_ids() {
    {
        for ehi_pair in "ca $DAEMON_CA_DIR/ca.crt" "ca_bundle $DAEMON_CA_DIR/ca-bundle.crt" "shadow /etc/shadow" "machine_id /etc/machine-id"; do
            ehi_key=${ehi_pair%% *}
            ehi_path=${ehi_pair#* }
            if ehi_id=$(stat -L -c '%d:%i' "$ehi_path" 2> /dev/null); then
                printf '%s %s\n' "$ehi_key" "$ehi_id"
            fi
        done
        if [ -n "${ESCAPE_IMAGE:-}" ]; then
            printf 'image %s\n' "$ESCAPE_IMAGE"
        fi
        # The kernel level (HUM-445): esc-2 runs its kernel variants and
        # needs the content of the CA and the bundle and the host's
        # .git/hooks for them.
        if [ "${ESCAPE_BACKEND:-}" = qemu ]; then
            printf 'level kernel\n'
            for ehi_pair in "ca $DAEMON_CA_DIR/ca.crt" "ca_bundle $DAEMON_CA_DIR/ca-bundle.crt"; do
                ehi_key=${ehi_pair%% *}
                if ehi_sum=$(sha256sum "${ehi_pair#* }" 2> /dev/null); then
                    printf '%s_sha256 %s\n' "$ehi_key" "${ehi_sum%% *}"
                fi
            done
            if ehi_id=$(stat -c '%d:%i' "$WORK/.git/hooks" 2> /dev/null); then
                printf 'git_hooks %s\n' "$ehi_id"
            fi
        fi
    } > "$1"
}

# escape_collect SUITE CODE — the result file of a suite that ran in the
# sandbox, into the summary. No file is `sandbox_not_started`, a file without
# the sentinel `sandbox_died`; both are errors.
escape_collect() {
    ec_suite="$1"
    ec_code="$2"
    ec_file="$WORK/results/$ec_suite.txt"
    if [ ! -f "$ec_file" ]; then
        record_error "$ec_suite" sandbox_not_started \
            "$(escape_launcher_name) exited $ec_code and left no result file; see target/escape/$ec_suite.log"
        return
    fi
    if ! grep -q "^SUITE-DONE $ec_suite " "$ec_file"; then
        record_error "$ec_suite" sandbox_died \
            "the suite stopped before its last case; see target/escape/$ec_suite.log"
    else
        escape_count_matches "$ec_suite" "$ec_file"
    fi
    grep '^RESULT ' "$ec_file" >> "$RESULTS" || true
}

# escape_count_matches SUITE FILE — the sentinel names how many cases the
# suite recorded; a file whose RESULT lines are none or another number is an
# error (HUM-445): a suite that recorded nothing proves nothing, and a lost or
# extra line means the file is not the suite's own.
escape_count_matches() {
    ecm_said=$(sed -n "s/^SUITE-DONE $1 \([0-9][0-9]*\)\$/\1/p" "$2" | tail -n 1)
    ecm_got=$(grep -c "^RESULT $1 " "$2" || true)
    if [ -z "$ecm_said" ] || [ "$ecm_said" -eq 0 ] || [ "$ecm_got" -ne "$ecm_said" ]; then
        record_error "$1" result_count \
            "the sentinel names ${ecm_said:-no} case(s), the file holds $ecm_got RESULT line(s); see target/escape/$1.log"
    fi
}
