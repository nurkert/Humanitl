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
probe no_etc_shadow  test -e /etc/shadow
probe no_machine_id  sh -c 'test -s /etc/machine-id'

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
expect_output exactly_one_socket '^[[:space:]]*1$' esc_socket_count
expect_only   socket_is_proxy '^/run/humanitl/proxy\.sock$' esc_find_sockets /
# HUM-013: the door is a socket FILE, and the daemon's control socket and
# token, which live next to the proxy directory on the host, are not here.
expect_ok     proxy_is_socket_file test -S /run/humanitl/proxy.sock
probe         no_daemon_socket sh -c 'test -e /run/humanitl/daemon.sock -o -e /run/humanitl/token'
probe         proxy_socket_dir_is_not_host sh -c 'ls /run/humanitl | grep -vxE "proxy\.sock|humanitl-shim" | grep -q .'

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
expect_output git_hooks_is_tmpfs 'tmpfs' sh -c 'grep " /work/.git/hooks " /proc/self/mountinfo'

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
esc_engine_sockets() {
    esc_es_found=$({
        esc_find_sockets / | grep -E 'attach|conmon|podman|/ctl$|notify'
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
