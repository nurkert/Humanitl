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
# is bwrap (later the shim), never the host init: seeing systemd there would
# mean the PID namespace was not entered at all.
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

esc_end
