# ADR-0019 · Seatbelt is the sandbox on macOS, a Linux VM comes later

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-10-30

## Context

Humanitl is to run on macOS as cleanly as on Linux, and every release is to
carry a Mac app (owner's request of 2026-10-29). The sandbox of ADR-0002 rests
on Linux kernel features that macOS does not have: an empty network namespace,
mount namespaces, seccomp. Without them the three guarantees of
[`SECURITY.md`](../SECURITY.md) cannot be stated as they are.

The port `SandboxBackend` (ADR-0015) names Seatbelt and a micro-VM as
candidate second backends. On macOS three ways were open:

- **Seatbelt.** The sandbox of the macOS kernel: `sandbox-exec -f profile.sb`
  sets an SBPL profile that the kernel enforces on the process and every child.
  Chrome, OpenAI Codex and Claude Code `sandbox-runtime` use it. It needs no
  privileges and no download.
- **A Linux VM** through `Virtualization.framework`, with the bwrap backend
  inside and the door as a vsock port. The same three guarantees as on Linux,
  at the price of a guest image in the release, a boot per session and a
  second kernel to keep current.
- **Docker Desktop or a similar VM tool.** A dependency the user installs and
  licenses separately, and a daemon that runs as a privileged helper.

## Decision

On macOS the sandbox backend is Seatbelt (`SeatbeltBackend` in
`daemon/crates/sandbox/src/seatbelt.rs`), chosen at compile time through the
type alias `PlatformBackend`. Daemon and command line name only that alias.

- The plan writes one SBPL profile per session into a private session
  directory and starts `/usr/bin/sandbox-exec -f <profile> <shim> …`. The
  profile denies by default; what it allows is listed in
  [`SECURITY.md` section 12](../SECURITY.md#12-macos-seatbelt-instead-of-bubblewrap).
- Without mounts, the sandbox paths of the profile (`/work`, `/home/agent`,
  `/tmp`, `/etc/humanitl`, `/run/humanitl`) are mapped onto the project and the
  session directory, in the environment, the command and the adapter files.
- The shim runs on macOS too, without seccomp. It serves the door on a free
  loopback port of the session, measures the guarantees from inside and writes
  the same five `CHECK` lines; the host maps them to the three guarantees as on
  Linux.
- The guarantees are restated for macOS: no way out (every connection but the
  door ends in `EPERM`), exactly one door, and the kernel holds the process to
  the profile (`sandbox_check`).

A backend with a Linux VM follows as its own issue (HUM-318), decided by the
owner on 2026-10-29.

## Rationale

Seatbelt carries the part of the claim that matters for exfiltration: the
shim asserts `EPERM` for a public TCP and UDP target, IPv6, foreign Unix and
loopback targets and raw, routing and system sockets, and the macOS escape
suite (`tests/escape/macos/run.sh`) checks from inside a real sandbox that
direct TCP, TLS, DNS and these sockets fail, that the home directory, masks,
other terminals and the profile itself are out of reach, and that a request
through the door arrives at the daemon. A mutation that opens the network turns guarantees 1
and 2 red. The suite runs in the CI job `macos` and against the bundled
programs in every release.

It costs nothing to install and starts in milliseconds, so a Mac user gets the
same workflow as a Linux user.

## Rejected alternatives

- **The Linux VM now.** It would keep the Linux guarantees word for word, but
  it needs a guest image, its updates and a vsock door before the first Mac
  release; the owner put it after Seatbelt.
- **Docker Desktop.** A separately licensed dependency with a privileged
  helper, and `docker.sock` is a further way in (ADR-0002).
- **No sandbox on macOS.** The proxy alone would rest on the agent honouring
  `HTTP_PROXY`; that is trust instead of physics (principle 1).

## Consequences

- Guarantee 1 changes its wording on macOS: interfaces exist, but no process
  in the sandbox may use them. Guarantee 3 is the profile, not seccomp.
- Loopback is shared with the host, so the door listens on a host-visible
  port. Any local process of any account can reach it and sends with the
  session's rules (K-19).
- There is no PID namespace. The shim ends the command's session at the end
  of the sandbox, in repeated passes; a process that starts a session of its
  own outlives it, still under the profile, and the profile allows it the
  daemon's proxy socket for as long as the daemon runs (K-20). HUM-321
  restricts that socket to the session's shim.
- Reading is a deny list, not the allow list of mounts: everything outside
  `/Users`, `/Volumes` and the temp trees is readable, `/etc` and
  `/opt/homebrew` included. The project is not pinned by descriptor as on
  Linux (K-21).
- Apple marks `sandbox-exec` as deprecated and documents SBPL only in
  examples. Every release runs the escape suite against the macOS of the
  `macos-15` runner, so a change in that version shows up as a red release
  run; one that only a newer macOS brings shows up when the runners move
  (K-22).

## Affected issues

`HUM-313` (the backend), `HUM-317` (this record and the security documents),
`HUM-318` (the VM backend). See [`backlog/sprint-6.md`](../../backlog/sprint-6.md).
