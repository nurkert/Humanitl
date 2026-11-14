#!/bin/sh
# ESC-2 — the mount surface: what is reachable from inside, and nothing else.
# Runs INSIDE the sandbox: humanitl sandbox run --profile test -- /tests/escape/esc-2-mounts.sh
#
# The mount surface is decided by the profile (HUM-010) and rendered by the
# launcher (HUM-011). Until the proxy exists (HUM-015), escape-launch binds a
# placeholder socket, bound and unused, at the host path the daemon will use
# (HUM-013), so that "one socket, and it is the proxy" is measured against a
# real socket file and not assumed.
#
# The claim under test (SECURITY.md, THREAT-MODEL K-03, K-05, K-06, K-07):
# exactly one Unix socket exists in the sandbox and it is the proxy; no host
# session channel (X11, Wayland, D-Bus, docker), no host home, no host runtime
# directory, no host /proc and no host /sys.
#
# The same file runs against every backend (HUM-435, HUMANITL_ESCAPE_BACKEND in
# run.sh). The section "the container engine" at the end holds what a
# container engine adds (THREAT-MODEL K-27): its own files in the sandbox, its
# own sockets, the user's engine configuration. Every probe there also runs
# under bwrap, where it holds because nothing of the kind exists; under an
# engine it holds only if the adapter keeps it out.

set -u
ESC_LIB="${ESC_LIB:-$(dirname "$0")/lib.sh}"
# shellcheck source=tests/escape/lib.sh
. "$ESC_LIB"

esc_begin esc-2

# --- no host session channel --------------------------------------------------

probe no_x11     test -e /tmp/.X11-unix
probe no_wayland sh -c 'ls /run/user/*/wayland-* 2>/dev/null | grep -q .'
probe no_dbus    sh -c 'ls /run/user/*/bus /run/dbus/system_bus_socket 2>/dev/null | grep -q .'
probe no_docker  sh -c 'test -e /var/run/docker.sock -o -e /run/docker.sock'

# --- no host filesystem -------------------------------------------------------

probe no_host_home   sh -c 'ls /home 2>/dev/null | grep -vx agent | grep -q .'
probe no_runtime_dir test -d /run/user
probe no_host_sys    test -d /sys/kernel
# Not a single file under /sys, only empty directories at most: Podman mounts
# the host's sysfs and its own cgroup beneath; the adapter hides both (HUM-453).
probe no_sys_content sh -c 'find /sys -mindepth 1 ! -type d 2>/dev/null | grep -q .'
# run.sh writes device and inode of the host files ESC-2 compares into
# /work/results/host-ids before the suites (escape_host_ids in launch.sh,
# HUM-439), and the reference of the image when the root is one.
esc_host_id() { sed -n "s/^$1 //p" /work/results/host-ids 2> /dev/null; }
esc_image=$(esc_host_id image)
# The kernel level (HUM-445): a guest kernel behind virtiofs, where a few
# physical facts differ from a shared kernel while the claim is the same.
# Those cases have a kernel variant below, named *_kernel, each checking the
# claim at least as strictly; the table in tests/escape/README.md says why
# each is equivalent. run.sh writes `level kernel` only for that backend.
esc_level=$(esc_host_id level)

# esc_host_file PATH KEY — exit 0, a leak for `probe`, when PATH in the
# sandbox is the host's file KEY (same device and inode).
esc_host_file() {
    command -v stat > /dev/null 2>&1 || { echo "no stat on this image"; return 127; }
    esc_hf_want=$(esc_host_id "$2")
    [ -n "$esc_hf_want" ] || return 1
    esc_hf_got=$(stat -L -c '%d:%i' "$1" 2> /dev/null) || return 1
    [ "$esc_hf_got" = "$esc_hf_want" ] || return 1
    echo "$1 is the host's file ($esc_hf_got)"
}

if [ -n "$esc_image" ]; then
    # An image brings its own /etc (HUM-439); the host's files must not be in
    # it, and only device and inode tell the two apart.
    probe no_etc_shadow esc_host_file /etc/shadow shadow
    probe no_machine_id esc_host_file /etc/machine-id machine_id
else
    probe no_etc_shadow test -e /etc/shadow
    probe no_machine_id sh -c 'test -s /etc/machine-id'
fi

# The CA and the bundle in the sandbox are the daemon's files on the host, by
# device and inode, under every backend and under an image too (HUM-439,
# THREAT-MODEL K-28): an image that carried a CA or a bundle of its own at the
# same path would be another file.
#
# esc_from_host PATH KEY — exit 0 when PATH in the sandbox is the host's file
# KEY, 1 when it is another file, 2 when the comparison cannot be made.
esc_from_host() {
    command -v stat > /dev/null 2>&1 || { echo "no stat on this image"; return 127; }
    esc_fh_want=$(esc_host_id "$2")
    [ -n "$esc_fh_want" ] || { echo "run.sh wrote no host id for $2"; return 2; }
    esc_fh_got=$(stat -L -c '%d:%i' "$1" 2>&1) || {
        echo "stat $1 failed: $esc_fh_got"
        return 2
    }
    [ "$esc_fh_got" = "$esc_fh_want" ] || {
        echo "$1 in the sandbox is $esc_fh_got, the daemon's is $esc_fh_want"
        return 1
    }
    echo "$1 is the daemon's file ($esc_fh_got)${esc_image:+, under the image $esc_image}"
}
# esc_from_host_kernel PATH KEY — the kernel variant. A file shared from the
# host over virtiofs keeps its inode, not its device number: the guest numbers
# its own devices (HUM-442 Q5), so the device is not compared. In its place
# the file must lie on the virtiofs share and carry the content of the
# daemon's file, byte for byte by SHA-256, so that no other file with the
# same inode number on another file system could pass.
esc_fs_type() {
    esc_ft_point=$(stat -L -c '%m' "$1" 2> /dev/null) || return 1
    awk -v p="$esc_ft_point" '{ for (i = 1; i <= NF; i++) if ($i == "-") { if ($5 == p) t = $(i + 1); break } } END { if (t == "") exit 1; print t }' /proc/self/mountinfo
}
esc_from_host_kernel() {
    command -v stat > /dev/null 2>&1 || { echo "no stat on this image"; return 127; }
    command -v sha256sum > /dev/null 2>&1 || { echo "no sha256sum on this image"; return 127; }
    esc_fk_want=$(esc_host_id "$2")
    esc_fk_sha=$(esc_host_id "${2}_sha256")
    [ -n "$esc_fk_want" ] && [ -n "$esc_fk_sha" ] || { echo "run.sh wrote no host id or digest for $2"; return 2; }
    esc_fk_ino=$(stat -L -c '%i' "$1" 2>&1) || { echo "stat $1 failed: $esc_fk_ino"; return 2; }
    [ "$esc_fk_ino" = "${esc_fk_want#*:}" ] || {
        echo "$1 in the guest has inode $esc_fk_ino, the daemon's file ${esc_fk_want#*:}"
        return 1
    }
    esc_fk_type=$(esc_fs_type "$1") || { echo "no mount found for $1"; return 2; }
    [ "$esc_fk_type" = virtiofs ] || { echo "$1 lies on $esc_fk_type, not on the virtiofs share"; return 1; }
    esc_fk_got=$(sha256sum "$1" 2>&1 | cut -d' ' -f1)
    [ "$esc_fk_got" = "$esc_fk_sha" ] || {
        echo "$1 in the guest has SHA-256 $esc_fk_got, the daemon's file $esc_fk_sha"
        return 1
    }
    echo "$1 is the daemon's file: inode $esc_fk_ino on virtiofs, SHA-256 $esc_fk_got"
}
if [ "$esc_level" = kernel ]; then
    expect_ok ca_is_the_hosts_kernel esc_from_host_kernel /etc/humanitl/ca.crt ca
    expect_ok ca_bundle_is_the_hosts_kernel esc_from_host_kernel /etc/ssl/certs/ca-certificates.crt ca_bundle
else
    expect_ok ca_is_the_hosts esc_from_host /etc/humanitl/ca.crt ca
    expect_ok ca_bundle_is_the_hosts esc_from_host /etc/ssl/certs/ca-certificates.crt ca_bundle
fi

# --- exactly one socket, and it is the proxy ----------------------------------
#
# esc_find_sockets in lib.sh is the probe (-xtype, no -xdev; the comment there
# says why), and selftest.sh proves it sees a socket, plain and bind-mounted,
# before run.sh trusts it with a sandbox. The launcher mounts exactly one
# socket FILE, never its directory (HUM-013); the shim binary sits next to it
# under /run/humanitl and is a regular file. /dev/shm is part of the list: it
# is the one writable tmpfs under /dev, and the probe does not prune it with
# the rest of /dev.
#
# Leading blanks in the pattern: some wc implementations pad the count.
esc_socket_count() { esc_find_sockets / | wc -l; }
# The kernel variant (HUM-445): the door is no file in a guest but a vsock
# port of the host, reached only through the shim's bridge on 127.0.0.1:3128
# (the agent's filter refuses AF_VSOCK and AF_UNIX, ESC-1). The claim
# "exactly one way out, and it is the proxy" becomes: not a single Unix socket
# anywhere in the guest, walked over the whole file system as above (wider
# than the shim's own walk over the writable places), the agent's proxy
# settings name only 127.0.0.1:3128, and that door answers as the proxy.
esc_proxy_vars() { env | grep -iE '^(https?|all)_proxy=' | sed 's/^[^=]*=//' | sort -u; }
esc_door_answer() {
    python3 -c '
import socket
s = socket.create_connection(("127.0.0.1", 3128), timeout=10)
s.sendall(b"GET http://humanitl.internal/ HTTP/1.0\r\nHost: humanitl.internal\r\n\r\n")
data = b""
while len(data) < 65536:
    chunk = s.recv(4096)
    if not chunk:
        break
    data += chunk
print(data.decode("utf-8", "replace"))
'
}
# esc_socket_walk_kernel — the positive control and the walk in one: a
# socket node planted in /dev/shm (mknod, as the agent's filter refuses
# socket(AF_UNIX)) must be listed, nothing else may be, and the walk must
# complete (find exits 0 for every root). A walk that broke off or saw
# nothing is red. The roots are the places a socket can be in the guest: the
# writable ones and every mount point the plan leaves empty (/work, /dev with
# /dev/shm, /tmp, /run, /home); the rest is the plan's read-only share of the
# host's files, in which the shim's single_socket and the host check allow
# no socket. The walk does not cover that share: a walk over the whole host
# /usr through virtiofs runs out of the file system daemon's descriptors
# (measured 2026-11-14, ENFILE after some 520000), and the agent cannot read
# every directory there either.
ESC_SOCKET_ROOTS_KERNEL="/work /dev /tmp /run /home"
esc_socket_walk_kernel() {
    esc_sw_plant=/dev/shm/humanitl-escape-control.sock
    python3 -c 'import os, stat, sys; os.mknod(sys.argv[1], stat.S_IFSOCK | 0o600)' "$esc_sw_plant" || {
        echo "cannot plant $esc_sw_plant"
        return 2
    }
    esc_sw_out=""
    esc_sw_code=0
    for esc_sw_root in $ESC_SOCKET_ROOTS_KERNEL; do
        esc_sw_found=$(esc_find_sockets "$esc_sw_root") || esc_sw_code=$?
        esc_sw_out="$esc_sw_out$esc_sw_found"
    done
    rm -f "$esc_sw_plant"
    [ "$esc_sw_code" -eq 0 ] || { echo "the walk did not complete (find exit $esc_sw_code): $esc_sw_out"; return 1; }
    [ "$esc_sw_out" = "$esc_sw_plant" ] || { echo "expected only $esc_sw_plant, the walk listed: $esc_sw_out"; return 1; }
    echo "the walk saw the planted socket and nothing else"
}
if [ "$esc_level" = kernel ]; then
    expect_ok     socket_none_kernel esc_socket_walk_kernel
    expect_only   door_is_loopback_kernel '^http://127\.0\.0\.1:3128$' esc_proxy_vars
    expect_output door_reaches_proxy_kernel '^humanitl session=' esc_door_answer
else
    expect_output exactly_one_socket '^[[:space:]]*1$' esc_socket_count
    expect_only   socket_is_proxy '^/run/humanitl/proxy\.sock$' esc_find_sockets /
    # HUM-013: the door is a socket FILE, and the daemon's control socket and
    # token, which live next to the proxy directory on the host, are not here.
    expect_ok     proxy_is_socket_file test -S /run/humanitl/proxy.sock
fi
probe         no_daemon_socket sh -c 'test -e /run/humanitl/daemon.sock -o -e /run/humanitl/token'
# Kernel variant (HUM-445): /run/humanitl holds the shim, the guest plan and
# the guest kernel's modules, never the daemon's socket or token. The listing
# must succeed and name nothing else; a listing that fails is red, not a pass.
# (The case under bwrap passes on a failing ls; under the kernel level it did
# so once only because a walk over the whole share had used up the
# descriptors of the file system daemon.)
if [ "$esc_level" = kernel ]; then
    expect_only   proxy_socket_dir_is_not_host_kernel '^(humanitl-shim|guest-plan\.json|modules)$' ls -A /run/humanitl
else
    probe         proxy_socket_dir_is_not_host sh -c 'ls /run/humanitl | grep -vxE "proxy\.sock|humanitl-shim" | grep -q .'
fi

# --- the host environment did not come along ----------------------------------
#
# run.sh puts HUMANITL_ESCAPE_MARKER into its own environment before launching.
# Grepping for the NAME, not the value, keeps the marker on the host: passing
# the value in would plant the very thing the probe looks for.
probe no_marker_leak   sh -c 'grep -la HUMANITL_ESCAPE_MARKER /proc/[0-9]*/environ 2>/dev/null'
probe no_host_env_leak sh -c 'env | grep -qE "^(XDG_RUNTIME_DIR|DBUS_SESSION_BUS_ADDRESS|DISPLAY|WAYLAND_DISPLAY|SSH_AUTH_SOCK|GPG_AGENT_INFO)="'

# --- own namespaces -----------------------------------------------------------
#
# With --unshare-pid the process that enters the namespace becomes PID 1. That
# is the shim (bwrap --as-pid-1, HUM-203), never the host init: seeing systemd
# there would mean the PID namespace was not entered at all.
probe         pid1_is_not_host_init sh -c 'grep -qxE "systemd|init" /proc/1/comm'
expect_output hostname_sandbox '^sandbox$' sh -c 'cat /proc/sys/kernel/hostname'
expect_output shm_is_tmpfs 'tmpfs' sh -c 'grep " /dev/shm " /proc/self/mountinfo'

# --- the project directory, and the masks over it -----------------------------

# run.sh seeds HUMANITL_MASK_CANARY into the host copy of every path the profile
# masks or covers with a tmpfs. Reading the canary here means the mask did not
# take. Not expect_empty: a mask that answers EACCES hides the content just as
# well as one that answers with an empty file, and demanding one of the two
# would tie the test to how bwrap happens to implement it today.
expect_ok work_is_writable sh -c 'touch /work/.humanitl-escape-write && rm -f /work/.humanitl-escape-write'
probe masked_envrc_hides_content     sh -c 'grep -a HUMANITL_MASK_CANARY /work/.envrc 2>/dev/null'
probe masked_gitconfig_hides_content sh -c 'grep -a HUMANITL_MASK_CANARY /work/.git/config 2>/dev/null'
probe git_hooks_hide_content         sh -c 'grep -ra HUMANITL_MASK_CANARY /work/.git/hooks 2>/dev/null'
probe editor_dirs_hide_content       sh -c 'grep -ra HUMANITL_MASK_CANARY /work/.vscode /work/.idea 2>/dev/null'
# The kernel variant (HUM-445): the profile's tmpfs over .git/hooks lies in
# the jail of the file system daemon of /work, so the guest sees the share,
# not a tmpfs mount. The claim of the mask holds when the directory is empty,
# is not the host's directory (another inode than the one run.sh recorded),
# and a hook written here never reaches the host: run.sh checks the host copy
# after this suite (git_hooks_write_stays_kernel).
esc_hooks_not_host() {
    esc_hn_host=$(esc_host_id git_hooks)
    [ -n "$esc_hn_host" ] || { echo "run.sh wrote no host id for git_hooks"; return 2; }
    esc_hn_ino=$(stat -c '%i' /work/.git/hooks 2>&1) || { echo "stat failed: $esc_hn_ino"; return 2; }
    [ "$esc_hn_ino" != "${esc_hn_host#*:}" ] || { echo "/work/.git/hooks is the host's directory, inode $esc_hn_ino"; return 1; }
    echo "inode $esc_hn_ino, the host's ${esc_hn_host#*:}"
}
if [ "$esc_level" = kernel ]; then
    expect_empty git_hooks_empty_kernel sh -c 'ls -A /work/.git/hooks'
    expect_ok    git_hooks_not_host_kernel esc_hooks_not_host
    expect_ok    git_hooks_write_kernel sh -c 'printf "#!/bin/sh\necho planted\n" > /work/.git/hooks/humanitl-escape-hook'
else
    expect_output git_hooks_is_tmpfs 'tmpfs' sh -c 'grep " /work/.git/hooks " /proc/self/mountinfo'
fi

# --- the container engine (HUM-435) -------------------------------------------
#
# Podman puts /run/.containerenv into every container. Without --privileged it
# is empty, with one exception: `graphRootMounted=1` when the engine's own
# storage is mounted into the container. Anything in it (the name, the id, the
# image, the storage) is knowledge of the host the agent has no business with.
# Under bwrap the file does not exist, which is empty as well.
expect_empty containerenv_empty sh -c 'cat /run/.containerenv 2>/dev/null'

# The engine's own channels: conmon's attach and control sockets and the
# engine's API socket stay on the host. exactly_one_socket above counts them
# already; this case names them, so that a regression reads as what it is.
# /proc/net/unix lists every Unix socket of the network namespace, bound to a
# path or not, so a socket of the engine that shared the namespace would show
# up there even without a file in the sandbox. Exit 1 when nothing matches,
# which is the pass of `probe`.
# Under the kernel level the walk takes the roots of socket_none_kernel, for
# the reason given there.
esc_sockets_anywhere() {
    if [ "$esc_level" = kernel ]; then
        for esc_sa_root in $ESC_SOCKET_ROOTS_KERNEL; do
            esc_find_sockets "$esc_sa_root"
        done
    else
        esc_find_sockets /
    fi
}
esc_engine_sockets() {
    esc_es_found=$({
        esc_sockets_anywhere | grep -E 'attach|conmon|podman|/ctl$|notify'
        grep -E 'attach|conmon|podman|/ctl$|notify' /proc/net/unix 2>/dev/null
    })
    [ -n "$esc_es_found" ] || return 1
    printf '%s\n' "$esc_es_found"
}
probe no_engine_socket esc_engine_sockets

# PID 1 is the shim, with the filter on, under every backend: an engine that
# put its own init (catatonit) in front of it would leave a process without
# the filter in the sandbox. esc-1 measures the name (pid1_is_shim) and the
# status line (seccomp_parent_mode_2) one by one; this case ties them to the
# same process.
esc_pid1_seccomp() {
    printf '%s %s\n' "$(cat /proc/1/comm 2>/dev/null)" \
        "$(sed -n 's/^Seccomp:[[:space:]]*/Seccomp: /p' /proc/1/status 2>/dev/null)"
}
expect_output pid1_shim_seccomp_2 '^humanitl-shim Seccomp: 2$' esc_pid1_seccomp

# The user's engine configuration adds nothing. tests/escape/plant-engine-config.sh
# plants a mounts.conf entry onto /run/secrets and a volume onto
# /humanitl-engine-volume in the CI job escape-tests-podman, after proving
# that a plain container gets both; here neither may exist. Where nothing was
# planted they hold trivially, and the host half (engine_hook_not_run in
# engine-config-proof.sh) says so.
probe no_engine_secrets sh -c 'ls -A /run/secrets 2>/dev/null | grep -q .'
probe no_engine_volume  test -e /humanitl-engine-volume

esc_end
