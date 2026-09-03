#!/bin/sh
# ESC-1 — sockets, interfaces, routing, capabilities, seccomp.
# Runs INSIDE the sandbox: humanitl sandbox run --profile test -- /tests/escape/esc-1-sockets.sh
#
# RED IS THE CORRECT STATE UNTIL SPRINT 1 CLOSES. Everything that depends on the
# shim's seccomp filter (HUM-012) fails here today, on purpose: the harness is
# written before the thing it guards, so that the guarantee is measurable from
# the first line of the filter onwards. tests/escape/README.md lists which probe
# turns green with which issue.
#
# The claim under test (CONVENTIONS.md 4.10, SECURITY.md, THREAT-MODEL K-04/K-13):
# AF_INET and AF_INET6 SOCK_STREAM must KEEP WORKING, because that is how the
# agent reaches the proxy on 127.0.0.1:3128, and so must socketpair(), which is
# AF_UNIX between two descriptors of the same process tree and no egress at all
# (4.11: Node and Bun use it for child-process IPC). The guarantee is not "no
# sockets", it is "no way out": the network namespace holds nothing but lo, the
# routing table is empty, and every other family and type is refused, and
# refused with EPERM, the filter's own answer (4.10). Any other errno means
# something else said no, and the case does not pass on it.

set -u
ESC_LIB="${ESC_LIB:-$(dirname "$0")/lib.sh}"
# shellcheck source=tests/escape/lib.sh
. "$ESC_LIB"

esc_begin esc-1

# Interface names out of /proc/net/dev. `ip` is not in a minimal image, and
# /sys is not mounted, so procfs is the only source that is always there.
# sed instead of `tr -d`, because tr would eat the newlines and glue a second
# interface onto the first.
ESC_IFACES='tail -n +3 /proc/net/dev | cut -d: -f1 | sed "s/[[:space:]]//g"'
# The device is the last column of an IPv6 routing entry.
ESC_ROUTE6='sed "s/.*[[:space:]]//" /proc/net/ipv6_route'

# esc_socket FAMILY TYPE [PROTOCOL] — create one socket. Exit 0 with
# `<label> created` when the kernel allowed it; exit 1 with
# `<label> refused: <ERRNO>` when it did not, and probe_eperm in lib.sh reads
# that errno as the verdict: EPERM passes, EAFNOSUPPORT is a skip (this kernel
# has no such family), anything else fails. Exit 127 when this python cannot
# even name a constant, so that "untestable" is recorded as skip and never as
# a pass.
#
# TYPE may carry flags, ORed with `|` the way C spells them:
# `SOCK_STREAM|SOCK_NONBLOCK|SOCK_CLOEXEC`. The filter masks arg1 with 0xff
# (CONVENTIONS 4.10) so that exactly these flags pass on an allowed type, and
# a forbidden type stays forbidden whatever flags ride along. Both halves of
# that rule get a probe below; without the first, every non-blocking runtime
# (node, tokio, asyncio) would lose the proxy on the day the filter lands.
#
# The protocol matters for SOCK_RAW. With the default 0 the kernel answers
# EPROTONOSUPPORT before any capability or filter is consulted, and the probe
# would be green whatever the sandbox does. IPPROTO_ICMP asks for a real raw
# socket: today that fails on the dropped CAP_NET_RAW, from HUM-012 on at the
# type mask of the filter, and both are the sandbox doing its job.
esc_socket() {
    python3 -c '
import errno, socket, sys
FALLBACK = {"AF_NETLINK": 16, "AF_PACKET": 17, "AF_VSOCK": 40,
            "SOCK_STREAM": 1, "SOCK_DGRAM": 2, "SOCK_RAW": 3,
            "IPPROTO_ICMP": 1}
def const(name):
    value = getattr(socket, name, None)
    return FALLBACK.get(name) if value is None else int(value)
def word(text):
    # "A|B|C" is one argument whose parts are ORed. SOCK_NONBLOCK and
    # SOCK_CLOEXEC have no fallback on purpose: their values differ between
    # architectures, and a guessed one would probe the wrong bits.
    parts = [const(name) for name in text.split("|")]
    if any(part is None for part in parts):
        return None
    value = 0
    for part in parts:
        value |= part
    return value
names = sys.argv[1:]
label = "/".join(names)
values = [word(name) for name in names]
if any(value is None for value in values):
    print("this python does not know %s" % label)
    sys.exit(127)
try:
    handle = socket.socket(*values)
except OSError as exc:
    print("%s refused: %s" % (label, errno.errorcode.get(exc.errno, "errno=%s" % exc.errno)))
    sys.exit(1)
handle.close()
print("%s created" % label)
' "$@"
}

# esc_socketpair — AF_UNIX by definition, and ALLOWED: the filter matches
# socket(), not socketpair() (CONVENTIONS 4.10 allow_families x allow_types is a
# rule on socket(); 4.11 keeps socketpair() untouched, because it connects two
# descriptors of the same process tree and is no egress, and Node/Bun need it
# for child-process IPC). Exit 0 when the pair was created.
esc_socketpair() {
    python3 -c '
import socket, sys
try:
    left, right = socket.socketpair()
except OSError as exc:
    print("socketpair refused: %s" % exc)
    sys.exit(1)
left.close()
right.close()
print("socketpair created")
'
}

# esc_connect ADDRESS PORT — print the errno name the kernel answered with.
# Always exit 0: the verdict is the errno, not the exit code.
esc_connect() {
    python3 -c '
import errno, socket, sys
handle = socket.socket()
handle.settimeout(2)
try:
    handle.connect((sys.argv[1], int(sys.argv[2])))
except OSError as exc:
    print(errno.errorcode.get(exc.errno, "errno=%s" % exc.errno))
else:
    print("CONNECTED")
' "$1" "$2"
}

# esc_io_uring — io_uring_setup(2), syscall 425 on every architecture we build for.
# Prints `io_uring_setup: <ERRNO>` when refused, or the descriptor when a ring
# was handed out. The errno is the verdict: the case wants EPERM, the answer of
# deny_syscalls (CONVENTIONS 3.4), and ENOSYS would only mean this kernel was
# built without io_uring, which is not the sandbox doing anything.
esc_io_uring() {
    python3 -c '
import ctypes, errno, sys
libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long
libc.syscall.argtypes = [ctypes.c_long, ctypes.c_long, ctypes.c_void_p]
params = (ctypes.c_uint32 * 32)()
fd = libc.syscall(425, 8, ctypes.byref(params))
if fd >= 0:
    print("io_uring_setup returned fd %d" % fd)
    sys.exit(0)
print("io_uring_setup: %s" % errno.errorcode.get(ctypes.get_errno(), "errno=%d" % ctypes.get_errno()))
sys.exit(1)
'
}

# esc_x32 — an x32 syscall (bit 0x40000000) with the getpid number.
# Prints the errno name. CONVENTIONS 4.10 demands EPERM from the shim prelude;
# ENOSYS would only mean this kernel was built without CONFIG_X86_X32, and a
# guarantee that rests on someone else's kernel config is not a guarantee.
esc_x32() {
    python3 -c '
import ctypes, errno, sys
libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long
libc.syscall.argtypes = [ctypes.c_long]
value = libc.syscall(0x40000000 | 39)
if value >= 0:
    print("RETURNED %d" % value)
else:
    print(errno.errorcode.get(ctypes.get_errno(), "errno=%d" % ctypes.get_errno()))
'
}

# esc_syscall NAME — dial one syscall from deny_syscalls (CONVENTIONS 3.4) raw,
# with arguments that let the call go through on an unfiltered kernel, and
# report what the kernel said: `<name>: <ERRNO>` and exit 1 when it was
# refused, `<name> returned <value>` and exit 0 when it went through. The
# verdict is probe_syscall's in lib.sh: EPERM passes, ENOSYS and EINVAL are a
# skip (the kernel has no such syscall, the filter was never asked), any other
# errno is red because the kernel answered, not the filter.
#
# Numbers are per architecture: x86_64 has its own table, aarch64 and riscv64
# share asm-generic, and the io_uring family (425 to 427) is the same
# everywhere. On any other machine the helper exits 3 before the call; the
# case is turned into a skip by denied_syscall below, on the machine name,
# not on that exit code, so that a table that is wrong for a new architecture
# shows up as a loud "no verdict" and never as a quiet skip.
#
# The arguments, and why these:
#   ptrace             PTRACE_TRACEME: the one request that needs no target and
#                      no capability, so a bare kernel lets it through. Once it
#                      has, this process is a tracee of its parent (the shell)
#                      and leaves through os._exit at once, before anything
#                      could stop it under a signal the shell would never resume.
#   process_vm_readv   16 bytes from this process into this process: the pid is
#   process_vm_writev  our own, so nothing but a filter has a reason to refuse.
#   keyctl             KEYCTL_GET_KEYRING_ID of the process keyring, created on
#                      demand: a serial on a bare kernel.
#   add_key            a "user" key in the process keyring, which dies with us.
#   request_key        a key that does not exist and no callout: ENOKEY on a
#                      bare kernel. A callout would run /sbin/request-key in
#                      the host's namespace; that is why the syscall is on the
#                      list, and no reason to try it from a test.
#   io_uring_enter     on a pipe of our own, which is a descriptor but not a
#   io_uring_register  ring: ENOTSUP on a bare kernel, before any sysctl is
#                      consulted (kernel.io_uring_disabled only guards setup).
#                      Not fd -1: a 7.x kernel answers that with EINVAL for
#                      io_uring_register, and EINVAL is the skip bucket.
#   kexec_load         no segments and an architecture value the kernel does
#   kexec_file_load    not know, respectively descriptor -1 and an undefined
#                      flag: the capability check (CAP_SYS_BOOT) comes first,
#                      so an unprivileged caller never reaches the arguments,
#                      and a privileged one gets EINVAL. Nothing here can load
#                      a kernel.
#   init_module        a NULL image of length zero, descriptor -1, and a module
#   finit_module       name nothing has ever registered: the capability check
#   delete_module      (CAP_SYS_MODULE) comes first, and behind it EFAULT,
#                      EBADF and ENOENT. Nothing here can load or drop a module.
#   bpf                BPF_MAP_CREATE with a NULL attribute of length zero:
#                      EPERM where unprivileged BPF is off, EINVAL otherwise.
#   perf_event_open    a software CPU clock on this process (see perf_attr
#                      below). A bare kernel at perf_event_paranoid <= 2 hands
#                      out a descriptor and a stricter one answers EACCES;
#                      neither is EPERM, so only the filter makes this case
#                      green.
#   userfaultfd        flags zero: a bare kernel with
#                      vm.unprivileged_userfaultfd = 1 hands out a descriptor,
#                      which is exactly the leak the filter closes.
esc_syscall() {
    python3 -c '
import ctypes, errno, os, sys
GENERIC = {"ptrace": 117, "process_vm_readv": 270, "process_vm_writev": 271,
           "add_key": 217, "request_key": 218, "keyctl": 219,
           "kexec_load": 104, "kexec_file_load": 294, "init_module": 105,
           "finit_module": 273, "delete_module": 106, "bpf": 280,
           "perf_event_open": 241, "userfaultfd": 282}
TABLE = {
    "x86_64": {"ptrace": 101, "process_vm_readv": 310, "process_vm_writev": 311,
               "add_key": 248, "request_key": 249, "keyctl": 250,
               "kexec_load": 246, "kexec_file_load": 320, "init_module": 175,
               "finit_module": 313, "delete_module": 176, "bpf": 321,
               "perf_event_open": 298, "userfaultfd": 323},
    "aarch64": GENERIC,
    "riscv64": GENERIC,
}
COMMON = {"io_uring_setup": 425, "io_uring_enter": 426, "io_uring_register": 427}
name = sys.argv[1]
machine = os.uname().machine
numbers = dict(COMMON)
numbers.update(TABLE.get(machine, {}))
nr = numbers.get(name)
if nr is None:
    print("%s: no syscall number known for %s" % (name, machine))
    sys.exit(3)

class iovec(ctypes.Structure):
    _fields_ = [("iov_base", ctypes.c_void_p), ("iov_len", ctypes.c_size_t)]
source = ctypes.create_string_buffer(b"humanitl-escape", 16)
target = ctypes.create_string_buffer(16)
local = iovec(ctypes.addressof(source), 16)
remote = iovec(ctypes.addressof(target), 16)
PTRACE_TRACEME = 0
KEYCTL_GET_KEYRING_ID = 0
KEY_SPEC_PROCESS_KEYRING = -2
IORING_REGISTER_PROBE = 8
not_a_ring, _ = os.pipe()
# A perf_event_attr the kernel accepts: type PERF_TYPE_SOFTWARE, config
# PERF_COUNT_SW_CPU_CLOCK, size 128 (PERF_ATTR_SIZE_VER5), everything else
# zero. With pid 0 (this process) and cpu -1 a bare kernel at
# perf_event_paranoid <= 2 hands out a descriptor; a stricter sysctl answers
# EACCES. Neither answer is EPERM, so the case measures the filter.
perf_attr = ctypes.create_string_buffer(128)
ctypes.memmove(perf_attr, (1).to_bytes(4, "little"), 4)
ctypes.memmove(ctypes.byref(perf_attr, 4), (128).to_bytes(4, "little"), 4)
O_NONBLOCK = 0x800
KEXEC_BOGUS_ARCH = 0x0F000000
KEXEC_FILE_BOGUS_FLAG = 0x40
CALLS = {
    "ptrace": (PTRACE_TRACEME, 0, 0, 0),
    "process_vm_readv": (os.getpid(), ctypes.byref(local), 1, ctypes.byref(remote), 1, 0),
    "process_vm_writev": (os.getpid(), ctypes.byref(local), 1, ctypes.byref(remote), 1, 0),
    "keyctl": (KEYCTL_GET_KEYRING_ID, KEY_SPEC_PROCESS_KEYRING, 1),
    "add_key": (b"user", b"humanitl-escape", b"x", 1, KEY_SPEC_PROCESS_KEYRING),
    "request_key": (b"user", b"humanitl-escape-missing", None, 0),
    "io_uring_enter": (not_a_ring, 0, 0, 0, None, 0),
    "io_uring_register": (not_a_ring, IORING_REGISTER_PROBE, None, 0),
    "kexec_load": (0, 0, None, KEXEC_BOGUS_ARCH),
    "kexec_file_load": (-1, -1, 0, None, KEXEC_FILE_BOGUS_FLAG),
    "init_module": (None, 0, b""),
    "finit_module": (-1, b"", 0),
    "delete_module": (b"humanitl_escape_missing", O_NONBLOCK),
    "bpf": (0, None, 0),
    "perf_event_open": (ctypes.byref(perf_attr), 0, -1, -1, 0),
    "userfaultfd": (0,),
}
args = CALLS.get(name)
if args is None:
    print("%s: esc_syscall has no argument recipe for it" % name)
    sys.exit(3)
def carg(value):
    # syscall(2) is variadic: every argument travels as a machine word, and
    # ctypes would hand a bare int over as a 32-bit C int.
    if value is None:
        return ctypes.c_void_p(None)
    if isinstance(value, bytes):
        return ctypes.c_char_p(value)
    if isinstance(value, int):
        return ctypes.c_long(value)
    return value
libc = ctypes.CDLL(None, use_errno=True)
libc.syscall.restype = ctypes.c_long
value = libc.syscall(ctypes.c_long(nr), *[carg(arg) for arg in args])
if value < 0:
    code = ctypes.get_errno()
    print("%s: %s" % (name, errno.errorcode.get(code, "errno=%d" % code)))
    sys.exit(1)
print("%s returned %d" % (name, value))
sys.stdout.flush()
os._exit(0)
' "$1"
}

# denied_syscall CASE NAME — one probe_syscall, or a skip on a machine whose
# syscall numbers esc_syscall does not carry.
ESC_MACHINE=$(uname -m)
case "$ESC_MACHINE" in
x86_64 | aarch64 | riscv64) ESC_SYSCALL_TABLE=1 ;;
*) ESC_SYSCALL_TABLE=0 ;;
esac
denied_syscall() {
    if [ "$ESC_SYSCALL_TABLE" = 1 ]; then
        probe_syscall "$1" esc_syscall "$2"
    else
        skip "$1" "esc_syscall has no syscall numbers for $ESC_MACHINE; add them to its table"
    fi
}

# --- the network namespace holds nothing but lo -------------------------------

expect_output iface_lo_present '^lo$' sh -c "$ESC_IFACES"
expect_only   ifaces_only_lo   '^lo$' sh -c "$ESC_IFACES"
expect_empty  no_ipv4_routes         sh -c 'tail -n +2 /proc/net/route'
expect_only   ipv6_routes_only_lo '^lo$' sh -c "$ESC_ROUTE6"
expect_empty  no_arp_entries         sh -c 'tail -n +2 /proc/net/arp'

# --- what the agent needs stays open ------------------------------------------

expect_ok socket_af_inet  esc_socket AF_INET  SOCK_STREAM
expect_ok socket_af_inet6 esc_socket AF_INET6 SOCK_STREAM
expect_ok socketpair      esc_socketpair
# The flags an event loop sets in the type word. The filter compares
# `arg1 & 0xff` (CONVENTIONS 4.10) precisely so that these two survive; a
# filter that matched the whole word would cut every non-blocking client off
# the proxy while every blocking one still worked, and nobody would notice
# until an agent hung.
expect_ok socket_inet_stream_flags  esc_socket AF_INET  'SOCK_STREAM|SOCK_NONBLOCK|SOCK_CLOEXEC'
expect_ok socket_inet6_stream_flags esc_socket AF_INET6 'SOCK_STREAM|SOCK_NONBLOCK|SOCK_CLOEXEC'

# --- every other family is refused, with EPERM --------------------------------
#
# probe_eperm, not probe: a plain "it failed" would be green on a kernel that
# simply has no vsock (EAFNOSUPPORT), and green for the wrong errno. EPERM is
# what the filter answers (CONVENTIONS 4.10), and today it is also what the
# dropped CAP_NET_RAW answers for AF_PACKET, which is why that one holds already.

probe_eperm socket_af_unix    esc_socket AF_UNIX    SOCK_STREAM
probe_eperm socket_af_netlink esc_socket AF_NETLINK SOCK_RAW
probe_eperm socket_af_packet  esc_socket AF_PACKET  SOCK_RAW
probe_eperm socket_af_vsock   esc_socket AF_VSOCK   SOCK_STREAM

# --- every other type is refused, even on the allowed families ----------------

probe_eperm socket_inet_dgram esc_socket AF_INET SOCK_DGRAM
probe_eperm socket_inet_raw   esc_socket AF_INET SOCK_RAW IPPROTO_ICMP
# The other half of the 0xff rule: a flag must not rescue a forbidden type.
# The mask strips SOCK_CLOEXEC before the comparison, so SOCK_DGRAM stays
# SOCK_DGRAM and stays refused.
probe_eperm socket_inet_dgram_flags esc_socket AF_INET 'SOCK_DGRAM|SOCK_CLOEXEC'

# --- an open socket still reaches nowhere -------------------------------------

expect_output connect_lan_enetunreach '^ENETUNREACH$' esc_connect 10.0.0.1 80
expect_output connect_wan_enetunreach '^ENETUNREACH$' esc_connect 1.1.1.1 443
# /bin/sh is dash on Debian and Ubuntu, and dash has no /dev/tcp at all: under
# it the probe is green because the shell cannot parse the redirection, not
# because the sandbox stopped it. bash implements it; without bash the case is
# a skip, never a pass.
if command -v bash > /dev/null 2>&1; then
    probe dev_tcp bash -c 'exec 3<>/dev/tcp/1.1.1.1/80'
else
    skip dev_tcp "no bash on this image; /bin/sh has no /dev/tcp, so the probe would be green for the wrong reason"
fi

# --- the syscalls that would sidestep the filter ------------------------------
#
# EPERM specifically, never "any error": ENOSYS says this kernel has no
# io_uring and is a fail, because the guarantee names EPERM and nobody
# answered it. A kernel with kernel.io_uring_disabled set answers EPERM by
# itself, before any filter is asked, and that would be the same false pass
# in the other direction; so that case is a skip that says so.
ESC_IO_URING_SYSCTL=$(cat /proc/sys/kernel/io_uring_disabled 2>/dev/null || echo 0)
if [ "${ESC_IO_URING_SYSCTL:-0}" != 0 ]; then
    skip io_uring_setup "kernel.io_uring_disabled=$ESC_IO_URING_SYSCTL answers EPERM itself; the filter was never asked"
else
    expect_output io_uring_setup '^io_uring_setup: EPERM$' esc_io_uring
fi
if [ "$ESC_MACHINE" = x86_64 ]; then
    expect_output x32_syscall_eperm '^EPERM$' esc_x32
else
    skip x32_syscall_eperm "the x32 ABI only exists on x86_64, this is $ESC_MACHINE"
fi

# The rest of deny_syscalls, one probe per name, each wanting EPERM and
# nothing else. A bare kernel answers most of these on its own: it lets
# ptrace(PTRACE_TRACEME), process_vm_readv/writev, keyctl and add_key through
# (a LEAK), and says ENOTSUP for a descriptor that is no ring and ENOKEY for a
# key nobody added. Every one of those is red for the same reason: the kernel
# answered, the filter did not. ENOSYS and EINVAL are the kernel saying it has
# no such syscall at all (no CONFIG_KEYS, no io_uring); the filter was never
# asked, so the case is a skip that says so, never a pass. Note that
# io_uring_setup above keeps its stricter reading of ENOSYS.
#
# Yama at ptrace_scope 2 or 3 refuses PTRACE_TRACEME by itself, with EPERM,
# before any filter is consulted: the same false pass as io_uring_disabled, and
# the same skip.
ESC_PTRACE_SCOPE=$(cat /proc/sys/kernel/yama/ptrace_scope 2>/dev/null || echo 0)
if [ "${ESC_PTRACE_SCOPE:-0}" -ge 2 ] 2>/dev/null; then
    skip ptrace_traceme "kernel.yama.ptrace_scope=$ESC_PTRACE_SCOPE answers EPERM itself; the filter was never asked"
else
    denied_syscall ptrace_traceme ptrace
fi
denied_syscall process_vm_readv     process_vm_readv
denied_syscall process_vm_writev    process_vm_writev
denied_syscall keyctl_get_keyring_id keyctl
denied_syscall add_key              add_key
denied_syscall request_key          request_key
denied_syscall io_uring_enter       io_uring_enter
denied_syscall io_uring_register    io_uring_register

# The hardening list of the specification (backlog/sprint-1.md, the table of
# HUM-012), the same names the Docker default profile refuses: a new kernel,
# kernel modules, BPF programs, other processes' event counters, and page
# faults served from user space. They are part of seccomp::FLOOR, so no
# profile can drop them.
#
# Read these verdicts with the same care as the ones above: a kernel answers
# most of them with EPERM by itself as soon as the caller has no capability,
# and inside the sandbox nobody has one (CapEff is empty, see above). The
# probes therefore cannot tell "the filter refused" from "the capability was
# missing" - they hold the line at the point where a capability would ever be
# granted again. The two that do measure the filter alone are perf_event_open,
# which an unprivileged caller is allowed to point at itself (or is refused
# with EACCES, which is red here), and userfaultfd, which a kernel with
# vm.unprivileged_userfaultfd = 1 hands out.
denied_syscall kexec_load           kexec_load
denied_syscall kexec_file_load      kexec_file_load
denied_syscall init_module          init_module
denied_syscall finit_module         finit_module
denied_syscall delete_module        delete_module
denied_syscall bpf                  bpf
denied_syscall perf_event_open      perf_event_open
denied_syscall userfaultfd          userfaultfd

# --- no capabilities, no way to gain one --------------------------------------
#
# capsh --print says the same thing in prose, but it is not in a minimal image;
# /proc is mounted in every profile, so the status file is the reliable source.
expect_output caps_effective_empty '^CapEff:[[:space:]]+0+$' sh -c 'grep ^CapEff /proc/self/status'
expect_output caps_bounding_empty  '^CapBnd:[[:space:]]+0+$' sh -c 'grep ^CapBnd /proc/self/status'
expect_output no_new_privs         '^NoNewPrivs:[[:space:]]+1$' sh -c 'grep ^NoNewPrivs /proc/self/status'

# --- the filter is on -----------------------------------------------------------
#
# Mode 2 is SECCOMP_MODE_FILTER. The second case walks every process, because a
# filter that only the first one carries is not a boundary. PID 1 is the one
# exception: with --unshare-pid and no --as-pid-1 that is bwrap itself, which
# stays behind to reap children and never installs a filter of its own. The
# carve-out is keyed on the PID, not on /proc/PID/comm: comm is writable by the
# process itself (prctl PR_SET_NAME), so a name-based exception would let
# anything that escaped the filter hide by calling itself bwrap. The name is
# still printed, as evidence only. Once HUM-011/012 make the shim PID 1 the
# carve-out goes, and the issue's seccomp_parent_mode_2 on /proc/1/status
# comes back in its place.
ESC_SECCOMP_ALL='for p in /proc/[0-9]*; do
  [ "$p" = /proc/1 ] && continue
  c=$(cat "$p/comm" 2>/dev/null) || continue
  s=$(sed -n "s/^Seccomp:[[:space:]]*//p" "$p/status" 2>/dev/null) || continue
  [ -n "$s" ] && echo "${p#/proc/}:$c=$s"
done'
expect_output seccomp_mode_2 '^Seccomp:[[:space:]]+2$' sh -c 'grep ^Seccomp /proc/self/status'
expect_only   seccomp_every_process '=2$' sh -c "$ESC_SECCOMP_ALL"

esc_end
