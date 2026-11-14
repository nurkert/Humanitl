# shellcheck shell=sh
# Sourced by run.sh under the backend qemu only (HUM-445): the kernel level
# with root in the guest, and what the host sees of a session.
#
# - kernel_netns_watch_start LOG / kernel_netns_watch_stop: while ESC-1 runs,
#   a watcher reads the session from escape-launch's log, finds the session's
#   unit with the user's service manager and records, for every process of
#   the unit, its program and the interfaces of its network namespace.
#   kernel_netns_case judges the record: every process sees only lo, and the
#   record holds the four helper processes (the emulator, the vsock device
#   and two file system daemons, one for the root and one for /work), so
#   that it measured the right unit (esc-guest/host_unit_only_lo).
# - kernel_guest_cases: two guests of two sessions. The second one starts
#   first and listens on vsock port 6000 as root without a filter
#   (guest-root.sh listen, HUM-442 Q9); once it listens, the first one runs
#   the probes of guest-root.sh as root without a filter, next to its agent.
#   Both start through escape-launch --test-guest-root-probe, which exists
#   only in the escape suite's launcher. Their results are the suite
#   esc-guest.
# - kernel_hooks_case: the hook esc-2 wrote into /work/.git/hooks of the
#   guest is not in the host's project (esc-2/git_hooks_write_stays_kernel).
#
# Needs from run.sh: OUT, HERE, STATE, WORK, LAUNCHER, PROFILE_FILE,
# DAEMON_XDG, DAEMON_PROXY_SOCK, DAEMON_CA_DIR, SHIM_INSTALLED, RESULTS,
# record_case, record_error and escape_collect.

# The helper processes every session of the kernel level runs (lines.rs of
# the adapter), by the name the kernel shows (15 characters at most), each
# with how many of it the record must hold.
KERNEL_HELPERS="qemu-system-x86:1 vhost-device-vs:1 virtiofsd:2"

kernel_launch() {
    kl_log="$1"
    kl_work="$2"
    kl_probe="$3"
    shift 3
    XDG_DATA_HOME="$DAEMON_XDG/data" \
        XDG_CONFIG_HOME="$DAEMON_XDG/config" \
        "$LAUNCHER" \
        --backend qemu \
        --profile "$PROFILE_FILE" \
        --state "$STATE" \
        --work "$kl_work" \
        --tests-dir "$HERE" \
        --proxy-socket "$DAEMON_PROXY_SOCK" \
        --ca-cert "$DAEMON_CA_DIR/ca.crt" \
        --ca-bundle "$DAEMON_CA_DIR/ca-bundle.crt" \
        --shim "$SHIM_INSTALLED" \
        --test-guest-root-probe "$kl_probe" \
        -- "$@" > "$kl_log" 2>&1
}

# The processes of the unit of SESSION, one line each: `<pid> <comm> <ifaces>`.
kernel_unit_record() {
    kur_unit="humanitl-vm-$1.service"
    kur_group=$(systemctl --user show -p ControlGroup --value "$kur_unit" 2> /dev/null) || return 1
    [ -n "$kur_group" ] || return 1
    find "/sys/fs/cgroup$kur_group" -name cgroup.procs -exec cat {} + 2> /dev/null | sort -un |
        while read -r kur_pid; do
            kur_comm=$(cat "/proc/$kur_pid/comm" 2> /dev/null) || continue
            kur_ifaces=$(tail -n +3 "/proc/$kur_pid/net/dev" 2> /dev/null |
                cut -d: -f1 | tr -d ' ' | sort | tr '\n' ',' | sed 's/,$//')
            printf '%s %s %s\n' "$kur_pid" "$kur_comm" "${kur_ifaces:-unreadable}"
        done
}

kernel_netns_watch_pid=""
kernel_netns_watch_start() {
    kernel_netns_record="$OUT/unit-netns.txt"
    : > "$kernel_netns_record"
    (
        knw_log="$1"
        knw_left=1200
        while [ "$knw_left" -gt 0 ]; do
            knw_session=$(sed -n 's/^escape-launch: session \([0-9a-f]*\)$/\1/p' "$knw_log" 2> /dev/null | head -n 1)
            # The unit may not have a cgroup yet; then the next round.
            if [ -n "$knw_session" ] && knw_now=$(kernel_unit_record "$knw_session"); then
                knw_complete=1
                for knw_helper in $KERNEL_HELPERS; do
                    knw_have=$(printf '%s\n' "$knw_now" | grep -c "^[0-9]* ${knw_helper%:*} " || true)
                    [ "$knw_have" -ge "${knw_helper#*:}" ] || knw_complete=0
                done
                if [ "$knw_complete" -eq 1 ]; then
                    printf '%s\n' "$knw_now" > "$kernel_netns_record"
                    exit 0
                fi
            fi
            sleep 0.1
            knw_left=$((knw_left - 1))
        done
    ) &
    kernel_netns_watch_pid=$!
}

kernel_netns_watch_stop() {
    if [ -n "$kernel_netns_watch_pid" ]; then
        kill "$kernel_netns_watch_pid" 2> /dev/null || true
        wait "$kernel_netns_watch_pid" 2> /dev/null || true
        kernel_netns_watch_pid=""
    fi
}

kernel_netns_case() {
    if [ ! -s "$kernel_netns_record" ]; then
        record_case esc-guest host_unit_only_lo fail \
            "no record of the session's unit with its helper processes (name:count $KERNEL_HELPERS) while esc-1 ran; see target/escape/esc-1.log"
        return
    fi
    knc_bad=$(awk '$3 != "lo"' "$kernel_netns_record")
    knc_all=$(tr '\n' ';' < "$kernel_netns_record")
    if [ -n "$knc_bad" ]; then
        record_case esc-guest host_unit_only_lo fail \
            "LEAK: a process of the session's unit sees more than lo: $(printf '%s' "$knc_bad" | tr '\n' ';')"
    else
        record_case esc-guest host_unit_only_lo pass \
            "every process of the unit sees only lo: $knc_all"
    fi
}

kernel_guest_cases() {
    kgc_sib="$OUT/sibling/work"
    rm -rf "$OUT/sibling"
    mkdir -p "$kgc_sib/results"
    rm -f "$WORK/results/sibling-ready" "$WORK/results/esc-guest.txt"

    echo "== esc-guest: second guest (listens on vsock port 6000) =="
    kernel_launch "$OUT/esc-guest-sibling.log" "$kgc_sib" \
        "/bin/sh /tests/escape/guest-root.sh listen" \
        /bin/sh /tests/escape/guest-root.sh wait esc-guest-sibling &
    kgc_sib_pid=$!
    kgc_left=1200
    while [ ! -f "$kgc_sib/results/sibling-ready" ] && [ "$kgc_left" -gt 0 ] &&
        kill -0 "$kgc_sib_pid" 2> /dev/null; do
        sleep 0.1
        kgc_left=$((kgc_left - 1))
    done
    if [ -f "$kgc_sib/results/sibling-ready" ]; then
        cp "$kgc_sib/results/sibling-ready" "$WORK/results/sibling-ready"
    fi

    echo "== esc-guest: probes as root without a filter =="
    set +e
    kernel_launch "$OUT/esc-guest.log" "$WORK" \
        "/bin/sh /tests/escape/guest-root.sh probe" \
        /bin/sh /tests/escape/guest-root.sh wait esc-guest
    kgc_code=$?
    set -e
    cat "$OUT/esc-guest.log"
    escape_collect esc-guest "$kgc_code"
    if [ "$kgc_code" -ne 0 ]; then
        record_error esc-guest launch \
            "escape-launch of the probing guest exited $kgc_code; see target/escape/esc-guest.log"
    fi

    # The second guest ends once it sees the stop; a minute at most.
    : > "$kgc_sib/results/sibling-stop"
    kgc_left=600
    while kill -0 "$kgc_sib_pid" 2> /dev/null && [ "$kgc_left" -gt 0 ]; do
        sleep 0.1
        kgc_left=$((kgc_left - 1))
    done
    if kill -0 "$kgc_sib_pid" 2> /dev/null; then
        kill -TERM "$kgc_sib_pid" 2> /dev/null || true
    fi
    set +e
    wait "$kgc_sib_pid"
    kgc_sib_code=$?
    set -e
    cat "$OUT/esc-guest-sibling.log"
    if [ ! -f "$kgc_sib/results/esc-guest-sibling.txt" ] ||
        ! grep -q '^SUITE-DONE esc-guest ' "$kgc_sib/results/esc-guest-sibling.txt"; then
        record_error esc-guest sibling_not_run \
            "the second guest wrote no complete result (exit $kgc_sib_code); see target/escape/esc-guest-sibling.log"
    else
        escape_count_matches esc-guest "$kgc_sib/results/esc-guest-sibling.txt"
        grep '^RESULT ' "$kgc_sib/results/esc-guest-sibling.txt" >> "$RESULTS" || true
        sed -n 's/^RESULT /  /p' "$kgc_sib/results/esc-guest-sibling.txt"
    fi
    if [ "$kgc_sib_code" -ne 0 ]; then
        record_error esc-guest sibling_launch \
            "escape-launch of the second guest exited $kgc_sib_code; see target/escape/esc-guest-sibling.log"
    fi
}

kernel_hooks_case() {
    if ! grep -q '^RESULT esc-2 git_hooks_write_kernel pass ' "$WORK/results/esc-2.txt" 2> /dev/null; then
        record_case esc-2 git_hooks_write_stays_kernel fail \
            "esc-2/git_hooks_write_kernel did not write the hook in the guest, so its absence on the host proves nothing"
    elif [ -e "$WORK/.git/hooks/humanitl-escape-hook" ]; then
        record_case esc-2 git_hooks_write_stays_kernel fail \
            "LEAK: the hook written in the guest is in the host's project: $WORK/.git/hooks/humanitl-escape-hook"
    else
        record_case esc-2 git_hooks_write_stays_kernel pass \
            "the hook written into /work/.git/hooks of the guest is not in the host's project"
    fi
}
