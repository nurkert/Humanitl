#!/bin/sh
# The kernel level with root in the guest (HUM-445). Runs INSIDE a guest of
# the backend qemu, started by tests/escape/kernel-proof.sh through
# `escape-launch --test-guest-root-probe`.
#
# Under the kernel level the guarantees are carried on the host, so they must
# hold even when the agent's filter is gone and the agent is root in its own
# kernel (docs/SECURITY.md section 14, THREAT-MODEL K-30 to K-32). To measure
# that, the shim in the guest starts one command next to the agent that is
# root and has no seccomp filter (the plan's root_probe). Only the escape
# suite can ask for it: the option exists only with the cargo feature of the
# bypass, which neither humanitld nor humanitl ever enables.
#
#   guest-root.sh probe        the probes, as root without a filter
#   guest-root.sh listen       a second guest of another session: listens on
#                              vsock port 6000 and records every connection
#                              (HUM-442 Q9)
#   guest-root.sh wait SUITE   the agent's command: waits until the probe has
#                              written the last line of SUITE, so that the
#                              guest lives as long as the probe
#
# A probe on a vsock target that does not exist waits for the timeout of the
# connection (HUM-442); every attempt sets SO_VM_SOCKETS_CONNECT_TIMEOUT to
# half a second, so that the job does not wait.

set -u
ESC_LIB="${ESC_LIB:-$(dirname "$0")/lib.sh}"
# shellcheck source=tests/escape/lib.sh
. "$ESC_LIB"

# The door's vsock port on the host and the port the second guest listens on.
GUEST_DOOR_PORT=5000
GUEST_SIBLING_PORT=6000
GUEST_RESULTS=/work/results

# esc_vsock CID:PORT... — one connection attempt per target, each with a
# timeout of half a second. One line per target, `<cid>:<port> <ERRNO>` or
# `<cid>:<port> CONNECTED`; exit 0 when any target connected, 1 when none
# did, 3 when the timeout could not be set, 127 when this kernel or python
# knows no AF_VSOCK. The cases judge the lines, never the exit code: a helper
# that breaks prints no such line and is red, not a pass.
esc_vsock() {
    python3 -c '
import ctypes, errno, socket, struct, sys
AF_VSOCK = getattr(socket, "AF_VSOCK", 40)
# SO_VM_SOCKETS_CONNECT_TIMEOUT_OLD takes a struct timeval of two longs.
# Python insists on an integer for every option of AF_VSOCK, so libc sets it.
SO_TIMEOUT = 6
libc = ctypes.CDLL(None, use_errno=True)
timeout = ctypes.create_string_buffer(struct.pack("ll", 0, 500000))
connected = False
for target in sys.argv[1:]:
    cid, port = (int(part, 0) for part in target.split(":"))
    try:
        s = socket.socket(AF_VSOCK, socket.SOCK_STREAM)
    except OSError as exc:
        print("no AF_VSOCK here: %s" % exc)
        sys.exit(127)
    if libc.setsockopt(s.fileno(), AF_VSOCK, SO_TIMEOUT, timeout, len(timeout.raw)) != 0:
        print("SO_VM_SOCKETS_CONNECT_TIMEOUT: errno %d" % ctypes.get_errno())
        sys.exit(3)
    s.settimeout(3)
    try:
        s.connect((cid, port))
    except socket.timeout:
        print("%d:%d ETIMEDOUT" % (cid, port))
    except OSError as exc:
        print("%d:%d %s" % (cid, port, errno.errorcode.get(exc.errno, "errno=%s" % exc.errno)))
    else:
        print("%d:%d CONNECTED" % (cid, port))
        connected = True
    finally:
        s.close()
sys.exit(0 if connected else 1)
' "$@"
}

# esc_vsock_refused CID:PORT... — exit 0 only when every requested target
# has exactly one line, refused with an errno, and there is no other line:
# the set of targets and their count are compared, so that a helper which
# skipped a target or printed a stray line is red. The lines are the evidence.
esc_vsock_refused() {
    esc_vr_out=$(esc_vsock "$@")
    esc_vr_code=$?
    printf '%s\n' "$esc_vr_out"
    [ "$esc_vr_code" -eq 127 ] && return 127
    esc_vr_lines=$(printf '%s\n' "$esc_vr_out" | grep -c . || true)
    [ "$esc_vr_lines" -eq "$#" ] || { echo "expected $# lines, got $esc_vr_lines"; return 1; }
    for esc_vr_target in "$@"; do
        esc_vr_want=$(printf '%d:%d' "${esc_vr_target%%:*}" "${esc_vr_target#*:}")
        esc_vr_n=$(printf '%s\n' "$esc_vr_out" | grep -cE "^$esc_vr_want E[A-Z0-9]+$" || true)
        [ "$esc_vr_n" -eq 1 ] || { echo "$esc_vr_want: not refused exactly once"; return 1; }
    done
}

# esc_inet ADDRESS PORT — a connection out of the guest; exit 0 when it
# connected, 1 with the errno when not.
esc_inet() {
    python3 -c '
import errno, socket, sys
host, port = sys.argv[1], int(sys.argv[2])
family = socket.AF_INET6 if ":" in host else socket.AF_INET
s = socket.socket(family, socket.SOCK_STREAM)
s.settimeout(3)
try:
    s.connect((host, port))
except socket.timeout:
    print("ETIMEDOUT"); sys.exit(1)
except OSError as exc:
    print(errno.errorcode.get(exc.errno, "errno=%s" % exc.errno)); sys.exit(1)
print("CONNECTED")
' "$1" "$2"
}

# esc_whoami — what this process is: uid, seccomp mode and two capabilities
# that would matter (CAP_SYS_MODULE 16, CAP_NET_ADMIN 12).
esc_whoami() {
    python3 -c '
status = dict(line.split(":", 1) for line in open("/proc/self/status") if ":" in line)
uid = status["Uid"].split()[0]
mode = status["Seccomp"].strip()
eff = int(status["CapEff"].strip(), 16)
print("uid=%s seccomp=%s sys_module=%d net_admin=%d" % (uid, mode, eff >> 16 & 1, eff >> 12 & 1))
'
}

# esc_finit_module — finit_module(2) on a module of the guest kernel that is
# not loaded yet (dummy, else any), with MODULE_INIT_COMPRESSED_FILE for a
# compressed one. Prints `finit_module: <ERRNO>` when refused, for
# probe_syscall: EPERM is the module lock; EEXIST, ENOEXEC or a success mean
# the kernel would have taken a module.
esc_finit_module() {
    python3 -c '
import ctypes, errno, os, sys
root = "/run/humanitl/modules"
found = []
for base, dirs, files in os.walk(root):
    for name in files:
        if ".ko" in name:
            found.append(os.path.join(base, name))
if not found:
    print("no module file under %s" % root)
    sys.exit(3)
dummy = [path for path in found if os.path.basename(path).startswith("dummy.ko")]
path = (dummy or sorted(found))[0]
flags = 4 if not path.endswith(".ko") else 0
libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long
fd = os.open(path, os.O_RDONLY)
rc = libc.syscall(313, fd, b"", flags)
if rc == 0:
    print("finit_module loaded %s" % path)
    sys.exit(0)
print("%s" % path)
print("finit_module: %s" % errno.errorcode.get(ctypes.get_errno(), "errno=%d" % ctypes.get_errno()))
sys.exit(1)
'
}

# esc_reset_lock — kernel.modules_disabled is one-way; exit 0 (a leak) only
# when writing 0 succeeded and the value reads 0 afterwards.
esc_reset_lock() {
    if ! echo 0 2> /dev/null > /proc/sys/kernel/modules_disabled; then
        echo "writing 0 refused, value $(cat /proc/sys/kernel/modules_disabled)"
        return 1
    fi
    esc_rl_now=$(cat /proc/sys/kernel/modules_disabled)
    echo "writing 0 accepted, value $esc_rl_now"
    [ "$esc_rl_now" = 0 ]
}

guest_probe() {
    esc_begin esc-guest
    # The premise: without it every probe below would measure the agent's
    # filter and its missing capabilities, not the guest.
    expect_output root_without_filter '^uid=0 seccomp=0 sys_module=1 net_admin=1$' esc_whoami

    # Guarantee 1: no interface but lo, and nothing leaves the machine.
    expect_only only_lo '^lo$' sh -c 'tail -n +3 /proc/net/dev | cut -d: -f1 | sed "s/[[:space:]]//g"'
    # The errno is the verdict, not the exit code: a broken helper is red.
    expect_only no_route_out_inet  '^E[A-Z0-9]+$' esc_inet 1.1.1.1 443
    expect_only no_route_out_inet6 '^E[A-Z0-9]+$' esc_inet 2606:4700:4700::1111 443
    expect_only no_route_out_slirp '^E[A-Z0-9]+$' esc_inet 10.0.2.2 80

    # Guarantee 2: exactly one door. The positive control first: this
    # process reaches the host's door, so a refusal below is the host's
    # device saying no, not a guest without vsock.
    expect_output vsock_door_reaches_host "^2:$GUEST_DOOR_PORT CONNECTED$" esc_vsock "2:$GUEST_DOOR_PORT"
    # 5001 is the guest's own report and control port, which the host
    # connects to and never offers; 5002 to 5010 lie next to the door.
    expect_ok vsock_other_ports esc_vsock_refused 2:5001 2:5002 2:5003 2:5004 2:5005 2:5006 \
        2:5007 2:5008 2:5009 2:5010 2:22 2:3128 2:1024 "2:$GUEST_SIBLING_PORT" 2:65535 2:0x7fffffff
    expect_ok vsock_other_cids esc_vsock_refused "0:$GUEST_DOOR_PORT" "1:$GUEST_DOOR_PORT" "3:$GUEST_DOOR_PORT" \
        "4:$GUEST_DOOR_PORT" "5:$GUEST_DOOR_PORT" "100:$GUEST_DOOR_PORT" "0xfffffffe:$GUEST_DOOR_PORT"
    # A second guest of another session listens on port 6000 (HUM-442 Q9);
    # kernel-proof.sh starts it first and writes sibling-ready only once it
    # listens. Every guest has CID 3 behind its own device process, so the
    # sibling is tried on every CID a shared device could route to it.
    if [ -f "$GUEST_RESULTS/sibling-ready" ]; then
        expect_ok vsock_sibling_guest esc_vsock_refused "3:$GUEST_SIBLING_PORT" "4:$GUEST_SIBLING_PORT" \
            "2:$GUEST_SIBLING_PORT" "1:$GUEST_SIBLING_PORT" "5:$GUEST_SIBLING_PORT"
    else
        esc_record vsock_sibling_guest fail "no second guest was listening (no $GUEST_RESULTS/sibling-ready); the probe would prove nothing"
    fi

    # Guarantee 3: no new doors. The module lock holds against root.
    expect_output modules_locked '^1$' cat /proc/sys/kernel/modules_disabled
    probe_syscall finit_module_locked esc_finit_module
    probe modules_lock_one_way esc_reset_lock
    esc_end
}

guest_listen() {
    ESC_RESULTS="$GUEST_RESULTS/esc-guest-sibling.txt"
    esc_begin esc-guest
    esc_gl_out=$(python3 -c '
import os, socket, sys, time
AF_VSOCK = getattr(socket, "AF_VSOCK", 40)
results, port = sys.argv[1], int(sys.argv[2])
try:
    s = socket.socket(AF_VSOCK, socket.SOCK_STREAM)
    s.bind((0xFFFFFFFF, port))
    s.listen(8)
except OSError as exc:
    print("listening=no %s" % exc)
    sys.exit(0)
print("listening=yes port=%d" % port, flush=True)
open(os.path.join(results, "sibling-ready"), "w").write("%d\n" % port)
s.settimeout(0.5)
accepted = []
deadline = time.monotonic() + 240
stopped = False
while time.monotonic() < deadline:
    if os.path.exists(os.path.join(results, "sibling-stop")):
        stopped = True
        break
    try:
        conn, peer = s.accept()
    except socket.timeout:
        continue
    accepted.append("%s:%s" % peer)
    open(os.path.join(results, "sibling-accepted"), "a").write("ACCEPTED %s:%s\n" % peer)
    conn.close()
print("accepted=%d %s" % (len(accepted), " ".join(accepted)))
print("stopped=%s" % ("yes" if stopped else "no"))
' "$GUEST_RESULTS" "$GUEST_SIBLING_PORT" 2>&1)
    expect_output sibling_listening '^listening=yes' printf '%s\n' "$esc_gl_out"
    expect_output sibling_not_reached '^accepted=0 *$' printf '%s\n' "$esc_gl_out"
    expect_output sibling_outlived_the_probe '^stopped=yes$' printf '%s\n' "$esc_gl_out"
    esc_end
}

guest_wait() {
    esc_gw_file="$GUEST_RESULTS/$1.txt"
    esc_gw_left=600
    while [ "$esc_gw_left" -gt 0 ]; do
        if grep -q "^SUITE-DONE " "$esc_gw_file" 2> /dev/null; then
            echo "guest-root: $1 done"
            return 0
        fi
        sleep 0.5
        esc_gw_left=$((esc_gw_left - 1))
    done
    echo "guest-root: $1 did not finish within five minutes" >&2
    return 1
}

case "${1:-}" in
probe) guest_probe ;;
listen) guest_listen ;;
wait) guest_wait "${2:?wait needs the suite}" ;;
*)
    echo "usage: guest-root.sh probe | listen | wait SUITE" >&2
    exit 2
    ;;
esac
