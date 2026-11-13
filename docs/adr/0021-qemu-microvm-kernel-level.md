# ADR-0021 · The kernel level runs as a QEMU micro-VM from Debian packages

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-11-13

## Context

ADR-0020 planned the kernel level as Podman with the OCI runtime `krun`
(crun built with libkrun). Neither libkrun nor a crun with libkrun support is
packaged in Debian; only the guest payload `libkrunfw` is. The level would
have meant downloading and building foreign code on every machine that wants
it, and keeping that build current ourselves. The owner decided on
2026-11-13 against that path: the kernel level uses only software the
distribution ships, and Podman stays what it is today, the engine of the
plain level and the source of OCI images.

The constraints of ADR-0020 stay in force without change: no setting weakens
the three guarantees from [`SECURITY.md`](../SECURITY.md#1-the-claim), a
project never lowers the isolation level, an unsafe setting is refused with a
`Diagnostic`, and no backend falls back silently to another one.

The spike HUM-442 measured the replacement on Debian forky/sid (kernel
7.3.4, QEMU 11.1.3, virtiofsd 1.14.0, vhost-device-vsock 0.3.0, bubblewrap
0.13.1). The guest ran with root and without any filter, so every result
below holds for an attacker who owns the guest:

| # | Question | Result |
|---|---|---|
| Q1 | Does the guest have a network interface? | Only `lo`; `connect` to `1.1.1.1:443` ends in `ENETUNREACH` |
| Q2 | Does exactly one vsock port reach the host? | CID 2, port 5000 reaches the Unix socket `<uds>_5000` and gets the host's answer; port 5001 and 22 on CID 2, CID 1 and CID 4 time out without a connection |
| Q3 | Which channels does the VMM hold? | `/dev/kvm`, the KVM VM and vCPU descriptors, eventfds, io_uring, a signalfd and the Unix sockets of the vhost-user devices (five for three devices in the final form), besides standard output and error; in the measuring runs also standard output as the serial console, which the final form (Q8) drops; no `/dev/vhost-vsock`, no `/dev/net/tun`, no tap |
| Q4 | Does a project folder swapped after the check reach the guest? | No: virtiofsd started in bubblewrap with `--bind-fd` on the opened folder serves the checked folder (same inode 377185) while the path already names another folder |
| Q5 | Do inode numbers pass through virtiofs? | Yes, for an entry and for the root of the mount (376298 on host and guest); the device number differs (43 on the host, 28 or 30 in the guest) |
| Q6 | Can root in the guest load kernel modules after the start? | No: the init loads the three vsock modules compressed from the host's `/usr/lib/modules` and then sets `kernel.modules_disabled = 1`; a later `finit_module` fails with `EPERM` |
| Q7 | Do all processes run under a hardened transient user unit? | Yes, in the final form, with `PrivateNetwork`, `PrivateUsers`, `NoNewPrivileges`, an empty capability bounding set, `ProtectSystem=strict`, `ProtectHome=read-only` and `RestrictAddressFamilies=AF_UNIX AF_NETLINK`; inside it each of QEMU, both virtiofsd and vhost-device-vsock runs in its own bubblewrap jail without network, and QEMU sees only the kernel file of `/boot` |
| Q8 | How long does a start take? | In the final form without a console, three runs: 2.20 s, 2.24 s and 2.16 s from calling `systemd-run` to the guest's first connection at the door (2.04 s, 2.03 s and 2.03 s of it from the start of QEMU), loading and locking the modules and mounting `/work` included (Debian's generic kernel, SeaBIOS); the same kind of unit running a bubblewrap jail with `true` takes 39 to 43 ms |
| Q9 | Can a guest reach another guest? | Measured under the unit of Q7. Not when each guest has its own vhost-device-vsock process: a guest dialling a listening second guest times out, even with the same group name. With one process for both guests in one group the connection succeeds (positive control), so groups act only within a process |
| Q10 | Can the host open the control channel? | Yes: the host connects to `<uds>`, writes `CONNECT 6000` and gets `OK 6000` from a guest listening on vsock port 6000; data flows both ways. The guest does not open this channel |

Whether hosted CI runners offer KVM is open (Q11). The commands and outputs stand in `backlog/sprint-7.md` under HUM-442.

## Decision

We add an adapter `qemu` behind the existing port `SandboxBackend`. It runs
each session as a QEMU micro-VM (`-M microvm`) with the host's own kernel,
built only from Debian packages: `qemu-system-x86`, `virtiofsd`,
`vhost-device-vsock` and `bubblewrap`. No new port, no abstraction over QEMU:
the names `qemu`, `virtiofsd`, `vhost-device-vsock` and `microvm` appear only
in `daemon/crates/sandbox/src/qemu/`, guarded by a new section in
`tools/coupling-baseline.toml`.

**Selection.** `sandbox.backend` gets the value `qemu`, isolation level
`kernel`. The runtime `krun` of `sandbox.podman.runtime` is removed; a
configuration that names it is refused with a `Diagnostic` whose fix names
`sandbox.backend = "qemu"`. It was never admitted, so no session depended on
it. `auto` stays bubblewrap. `sandbox.min_isolation = "kernel"` is satisfied
only by `qemu`.

**Processes.** Per session one transient unit of the user's service manager
(ADR-0010 addendum, HUM-434), hardened as in Q7. Inside it four processes,
each in a bubblewrap jail with `--unshare-all`, `--cap-drop ALL` and only the
paths it needs:

- **virtiofsd for the root**, read-only, serving `/` of its own jail. The jail
  is built from the same plan as the bubblewrap backend (the profile's
  mounts, the shim, the CA), so the guest's root is the plan and not a copy.
  Its socket arrives as a descriptor (`--fd`), so the session's runtime
  directory is not part of the guest's root.
- **virtiofsd for `/work`**, with the checked project folder bound by
  descriptor (`--bind-fd`, Q4) and `--sandbox none`, because the jail is the
  sandbox.
- **vhost-device-vsock**, one process per session and never shared between
  sessions, because its groups separate guests only within one process (Q9);
  with a group of its own and `--uds-path` in the session's directory, never
  `--forward-cid`.
- **QEMU** with `-nodefaults -no-user-config -nic none`, no serial, monitor
  or display, `-sandbox on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny`,
  shared guest memory through `memory-backend-memfd` (vhost-user needs it),
  the kernel file and nothing else from `/boot`, and `/dev/kvm` as its only
  device.

**The end of the daemon ends the session.** The unit is bound to the daemon
(`BindsTo` on the daemon's unit, or outside a unit a lifeline on the host
whose write end only the daemon holds). Root in the guest can ignore the end
of the control channel, so the lifeline inside the guest is only defence in
depth.

**The host checks the VMM.** Besides the `CHECK` lines from the guest, the
daemon checks from the host before the agent starts and refuses the session
with `SANDBOX_047` otherwise: every process of the unit has only `lo` in its
network namespace, QEMU holds only the descriptors of Q3 without a console,
no `/dev/vhost-vsock` and no `tun`, the runtime directory holds only the
declared sockets, and exactly one vhost-device-vsock process with one device
belongs to the session.

**The door.** vhost-device-vsock maps a connection from the guest to CID 2,
port P onto the Unix socket `<uds>_P`. The daemon creates exactly one such
socket, for the proxy, so exactly one port leads anywhere (Q2). The control
channel of HUM-443 goes the other way: the daemon connects to `<uds>` and
names the shim's port, so the guest cannot open it (Q10). No other socket exists
in the directory, and the directory is the session's own.

**The guest.** The shim is PID 1 (`init=`). Before anything else it loads
`vsock`, `vmw_vsock_virtio_transport_common` and
`vmw_vsock_virtio_transport` from the host's modules (compressed, through
`finit_module` with `MODULE_INIT_COMPRESSED_FILE`) and sets
`kernel.modules_disabled = 1` (Q6). It then listens on `127.0.0.1:3128` and
forwards each connection to the vsock door, mounts `/work`, compares the
mount table with the plan, installs the same seccomp filter as under the
other backends and starts the agent. For the agent nothing changes.

**The kernel.** The guest boots the running host kernel
(`/boot/vmlinuz-$(uname -r)`) with its modules from `/usr/lib/modules`, so
security updates of the kernel arrive through the distribution. Where the
image is not readable by the user (Ubuntu installs it with mode `0600`), or
where the kernel builds virtiofs as a module (`CONFIG_VIRTIO_FS=m`, as Ubuntu's
`linux-azure` does, so it cannot boot a virtiofs root without an initramfs),
the level is unavailable and `humanitl doctor` names the reason; a key
`sandbox.qemu.kernel` takes another image, with its modules directory. The CI
job of HUM-465 boots Debian's cloud kernel for this reason.

**Admission.** As in ADR-0020: the level is refused with `SANDBOX_048` until
its mount checks and the escape suite against it are green in CI (HUM-445,
HUM-446); HUM-449 admits it.

## Rationale

- Every component is a Debian package with its own security updates. The
  daemon builds nothing and downloads nothing; `humanitl doctor` checks that
  the packages are installed.
- The guarantees no longer rest on the guest. Root in the guest owns a kernel
  without a network device and with module loading switched off; its only way
  out is vsock to CID 2, where only the ports the daemon chose lead to a
  socket. Q1, Q2 and Q6 measured this with root in the guest.
- vhost-device-vsock keeps the vsock device out of the host kernel: no
  `vhost-vsock` module, no global CID space that other VMs or services of the
  host share. Its reach is the directory of the session.
- The root of the guest comes from the same plan as under bubblewrap. The
  mount checks, the pinning by descriptor and the profiles stay one code
  path, and the guest sees what the plan says (Q4, Q5).
- Every process runs in a bubblewrap jail inside a hardened unit. A flaw in
  QEMU, virtiofsd or vhost-device-vsock reaches a process without network,
  without capabilities and with only its own paths, before it reaches the
  user's account.

## Rejected alternatives

- **Podman with `krun`** (ADR-0020). libkrun is not packaged in Debian; the
  level would have needed a build of libkrun, libkrunfw and crun from foreign
  sources on every machine.
- **systemd-vmspawn.** Packaged and rootless, but its vsock uses the host
  kernel's `vhost-vsock` with the global CID space, so the guest reaches every
  vsock service of the host, and it expects systemd in the guest with several
  channels (journal, notification, credentials).
- **crosvm.** Packaged, with a stronger process model (one jailed process per
  device), but Debian ships a git snapshot, and whether its jails work
  without root is open. It stays the fallback if QEMU's surface proves too
  large.
- **gVisor (`runsc`).** A kernel in user space, not a guest kernel; it would
  be a level between `namespaces` and `kernel`, not the kernel level.
- **Incus virtual machines.** Need a daemon running as root and an agent in
  the guest as a second channel.
- **virtio-serial instead of vsock.** Built into QEMU and the guest kernel,
  but one byte stream per port; every proxy connection would need a
  multiplexing protocol of our own on both sides.

## Consequences

- A new adapter `daemon/crates/sandbox/src/qemu/` and a new section
  `[qemu]` in `tools/coupling-baseline.toml`. The Podman adapter loses the
  runtime `krun` and its code paths.
- The shim gains a guest mode: module loading and locking, the TCP to vsock
  forwarder and the control channel as a stream (HUM-443). It is PID 1 of a
  kernel, so a panic in it ends the VM, which is the lifeline's intent.
- `humanitl doctor` checks per prerequisite: `/dev/kvm` readable and writable,
  the four programs and the options this decision uses (from their help, not
  from version numbers), the kernel image readable, the
  modules directory of that kernel present, bubblewrap with `--bind-fd`.
- The trusted computing base of the level is the host kernel's KVM, QEMU's
  micro-VM machine with its virtio and vhost-user frontends, virtiofsd,
  vhost-device-vsock, bubblewrap and the shim. `docs/SECURITY.md` gets a
  section for the level and `docs/THREAT-MODEL.md` K entries for the VMM, the
  vsock door, the control channel, virtiofs and the guest kernel, each with
  the code it describes.
- A start costs about two seconds more than under bubblewrap (Q8).
- CI needs KVM and the Debian packages. The plan is a job in a `debian:sid`
  container on a hosted runner with `/dev/kvm` passed through (HUM-465); if
  no hosted runner offers KVM, the level stays unadmitted and the owner
  decides the replacement (HUM-445).
- The level is Linux on x86-64 for now. macOS can carry the same guest under
  Virtualization.framework, which has vsock and virtiofs devices of its own;
  that is a later adapter and a later decision.
- vhost-device-vsock is packaged in Debian testing and unstable, not yet in a
  stable release or in Ubuntu 24.04. Until it is, the level is unavailable
  there, and `humanitl doctor` says which package is missing.

## Affected issues

`HUM-442` (the spike and this decision), `HUM-443` to `HUM-449`
(`backlog/sprint-7.md`, replanned for this adapter), `HUM-145` (the coupling
ratchet), `HUM-263` (the pinned project folder), `HUM-350` (the disposable
sandbox). ADR-0020 gets an addendum that points here for the kernel level.
