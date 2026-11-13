# shellcheck shell=sh
# Sourced by run.sh after the suites in the sandbox (HUM-437): the cases
# esc-2/swapped_folder and esc-2/unplanned_mount.
#
# The plan opens and checks every folder it mounts. bwrap mounts the opened
# descriptor; a container engine takes a path and resolves it later (HUM-292,
# P13), and its own configuration can add a mount to every container. Before
# the agent exists, the shim compares the mount table with the one the plan
# expects and the device and inode of every checked folder with what the
# launcher opened (docs/SECURITY.md section 3). Both cases need something done
# between the plan and the start, on the host, so they run here and not in the
# sandbox, through escape-launch with the backend of the run:
#
# - swapped_folder: escape-launch --test-swap-work renames the project folder
#   after the plan and puts another one in its place, with the same paths and
#   a file HUMANITL_SWAPPED_FOLDER. The start must be refused with SANDBOX_042,
#   or the sandbox must see the checked folder (bwrap mounts the descriptor).
#   A command that sees the marker is a leak.
# - unplanned_mount: escape-launch --test-unplanned-mount puts a tmpfs on /etc
#   (under an engine) or on a new point /humanitl-unplanned (under bwrap, where
#   /etc would hide checked files) into the line, not into the expected table, the way a configuration source
#   of the engine would. The start must be refused with SANDBOX_049, and the
#   command must not run.
#
# Under bwrap the run builds escape-launch as well; without it both cases are
# red, never a skip.
#
# Needs from run.sh: OUT, HERE, STATE, LAUNCHER, ESCAPE_BACKEND, PROFILE_FILE,
# DAEMON_XDG, DAEMON_PROXY_SOCK, DAEMON_CA_DIR, SHIM_INSTALLED and record_case.

# The command the sandbox runs in both cases: it names the swapped folder when
# it sees it, and says that it ran at all.
MOUNT_PROOF_COMMAND='test -e /work/HUMANITL_SWAPPED_FOLDER && echo MOUNT-PROOF-SWAPPED-SEEN; echo MOUNT-PROOF-AGENT-RAN'

# One start through escape-launch with one test flag; the log gets stdout and
# stderr, the exit code is the launcher's.
#
# Ohne --image, auch wenn der Lauf HUMANITL_ESCAPE_IMAGE hat (HUM-455): Beide
# Fälle prüfen den Abgleich zwischen Plan und Start, nicht die Wurzel, und die
# Wurzel unter einem Abbild prüfen esc-1 bis esc-3 im selben Lauf.
mount_proof_run() {
    mpr_log="$1"
    mpr_flag="$2"
    XDG_DATA_HOME="$DAEMON_XDG/data" \
        XDG_CONFIG_HOME="$DAEMON_XDG/config" \
        "$LAUNCHER" \
        --backend "$ESCAPE_BACKEND" \
        --profile "$PROFILE_FILE" \
        --state "$STATE" \
        --work "$MOUNT_PROOF_WORK" \
        --tests-dir "$HERE" \
        --proxy-socket "$DAEMON_PROXY_SOCK" \
        --ca-cert "$DAEMON_CA_DIR/ca.crt" \
        --ca-bundle "$DAEMON_CA_DIR/ca-bundle.crt" \
        --shim "$SHIM_INSTALLED" \
        "$mpr_flag" \
        -- /bin/sh -c "$MOUNT_PROOF_COMMAND" > "$mpr_log" 2>&1
}

# The verdict of swapped_folder from the log and the exit code.
mount_proof_swap_verdict() {
    if grep -q MOUNT-PROOF-SWAPPED-SEEN "$1"; then
        echo "fail LEAK: the command saw the folder put in place after the check; see target/escape/$(basename "$1")"
    elif grep -q SANDBOX_042 "$1" && ! grep -q MOUNT-PROOF-AGENT-RAN "$1"; then
        echo "pass the swapped folder was detected and the start refused with SANDBOX_042"
    elif [ "$2" = 0 ] && grep -q MOUNT-PROOF-AGENT-RAN "$1"; then
        echo "pass the sandbox saw the checked folder, not the one put in its place"
    else
        echo "fail neither refused with SANDBOX_042 nor started in the checked folder (exit $2); see target/escape/$(basename "$1")"
    fi
}

# The verdict of unplanned_mount from the log.
mount_proof_unplanned_verdict() {
    if grep -q MOUNT-PROOF-AGENT-RAN "$1"; then
        echo "fail the command ran with a mount the plan does not hold; see target/escape/$(basename "$1")"
    elif grep -q SANDBOX_049 "$1"; then
        echo "pass the mount outside the plan was found and the start refused with SANDBOX_049"
    else
        echo "fail the start was not refused with SANDBOX_049 (exit $2); see target/escape/$(basename "$1")"
    fi
}

mount_proof_cases() {
    MOUNT_PROOF_WORK="$OUT/mount-proof/work"
    if [ ! -x "$LAUNCHER" ]; then
        for mp_case in swapped_folder unplanned_mount; do
            record_case esc-2 "$mp_case" fail "no escape-launch binary at $LAUNCHER; the host half of ESC-2 cannot run"
        done
        return
    fi
    rm -rf "$OUT/mount-proof"
    # The paths the profile masks, so that every mask has its target in the
    # folder that is checked and in the one put in its place.
    mkdir -p "$MOUNT_PROOF_WORK/.git/hooks" "$MOUNT_PROOF_WORK/.vscode"
    printf 'export MOUNT_PROOF=1\n' > "$MOUNT_PROOF_WORK/.envrc"

    set +e
    mount_proof_run "$OUT/mount-swapped.log" --test-swap-work
    mp_code=$?
    set -e
    mp_verdict=$(mount_proof_swap_verdict "$OUT/mount-swapped.log" "$mp_code")
    record_case esc-2 swapped_folder "${mp_verdict%% *}" "${mp_verdict#* }"

    set +e
    mount_proof_run "$OUT/mount-unplanned.log" --test-unplanned-mount
    mp_code=$?
    set -e
    mp_verdict=$(mount_proof_unplanned_verdict "$OUT/mount-unplanned.log" "$mp_code")
    record_case esc-2 unplanned_mount "${mp_verdict%% *}" "${mp_verdict#* }"
}
