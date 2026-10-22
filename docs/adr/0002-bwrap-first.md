# ADR-0002 · bubblewrap first, Docker later as a second backend

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

The agent must run in an environment in which there is physically no way into
the network except the one socket that Humanitl controls. That is principle 1:
physics instead of trust. An agent that ignores `HTTP_PROXY` or brings its own
DNS resolution must not get through; it must fail (fail-closed).

Three guarantees follow from this, and they are meant to be verifiable live in
the UI (`BACKLOG.md` 4.1):

1. **No network interface.** Only `lo` exists in the sandbox. No IP, no DNS,
   no ICMP, no QUIC, no raw sockets.
2. **Exactly one door.** The daemon's proxy socket is bound into the sandbox
   as a single file; `find / -type s` returns exactly this one file.
3. **No new doors.** A seccomp filter prevents any other socket families or
   types from being opened in the sandbox at all.

The sandbox technology must carry these three guarantees, work rootless (the
audience installs a `.deb` and does not want to type `sudo`), get by without a
background service and express its policy in a form the UI can show to a
non-expert.

## Decision

The sandbox backend in the MVP is
[bubblewrap](https://github.com/containers/bubblewrap) (`bwrap`), addressed
through the port `SandboxBackend` in `daemon/crates/sandbox`. The policy is a
single readable `bwrap` command line, generated from a profile under
`profiles/sandbox/*.toml` and shown unabridged in the UI ("Show exact bwrap command line").

Core of the command line: `--unshare-all --new-session --die-with-parent
--cap-drop ALL`, plus `--unshare-pid --unshare-ipc --unshare-uts`, its own
`/proc`, `--tmpfs /dev/shm`, host name `sandbox`, and a mount allowlist.
`--cap-drop ALL` stands unconditionally in the argv, directly after
`--new-session`, and is not configurable (`backlog/CONVENTIONS.md` 4.11). Never
mounted are `$XDG_RUNTIME_DIR`, `/tmp`, `/run`, `~/.ssh`, `~/.gitconfig`,
`~/.netrc`, as well as X11, Wayland, dbus and docker sockets. `/work` is the
project folder, either `ro` or `rw`; `.git/hooks`, `.git/config`, `.envrc`,
`.vscode` and `.idea` are masked with tmpfs.

Docker comes after the MVP as a second implementation of the same trait
(`SandboxBackend`), for complete toolchain images. Core and application
remain untouched by it (ADR-0015).

## Rationale

`--unshare-all` gives the sandbox an empty network namespace: only `lo`
exists, the routing table is empty. Guarantee 1 is therefore not a question of
configuration but a kernel property. The side effect on abstract Unix sockets
matters: the abstract namespace is bound to the network namespace, so it is
empty in the sandbox as well. An agent therefore cannot attach to an abstract
socket of the host.

bwrap runs rootless via user namespaces and needs no daemon. The policy is an
`argv`, not a state object in a foreign process — it can be displayed, copied,
attached to a bug report and reproduced outside of Humanitl. For a tool whose
security promise the user is meant to be able to follow, this transparency is
the decisive argument.

bwrap is packaged in Debian and Ubuntu and used by Flatpak on a large scale,
so it is neither exotic nor unobserved.

The limits are not concealed: bwrap needs unprivileged user namespaces. On
Ubuntu 24.04 and newer an AppArmor profile restricts them;
`humanitl doctor` (HUM-075) checks `unprivileged_userns_clone` and the
AppArmor profile and, on failure, returns a concrete fix instead of an error
message.

## Rejected alternatives

- **Docker with `--network none` (in the MVP).** Also creates an empty network
  namespace, but still needs a bridge and a seccomp filter in the container,
  runs via a daemon as root, and `docker.sock` is an additional pitfall path:
  whoever reaches it is root on the host. For the audience, "install Docker"
  is moreover a hurdle before the first start. Comes after the MVP as a second
  backend, where toolchain images justify the effort.
- **gVisor.** Rootless without its own netstack unusable for our model, and the
  compatibility surface for arbitrary agent toolchains is hard to estimate.
- **nsjail.** Technically close to bwrap, but no Debian package. The delivery
  problem ("one package, everything included") weighs more than the
  difference in functionality.
- **Firecracker or Kata Containers.** Stronger isolation via KVM, but heavy on
  tooling, kernel image in the package, slow start, KVM access not available
  everywhere. Earmarked as v2 for a stronger threat model, not for the MVP.
- **seccomp only, without namespaces.** A seccomp filter alone holds back no
  file system and no process list; in our design it is the safety net
  (guarantee 3), not the load-bearing wall.

## Consequences

- The sandbox is Linux-specific. macOS would need its own backend
  (`seatbelt`), which the port `SandboxBackend` permits, but nobody builds.
- An agent in the sandbox reaches the proxy via `127.0.0.1:3128`, because only
  loopback exists in the empty network namespace. The translation from this
  TCP endpoint to the host's Unix socket is done by the shim (`humanitl-shim`).
  The process model originally did without `--as-pid-1`: PID 1 in the
  sandbox was bwrap's init, the shim ran beneath it. Since HUM-203 the
  addendum below applies: bwrap starts with `--as-pid-1`, the shim is PID 1. The
  isolation check still reads the filtered child. Details on the bridge list
  in ADR-0016.
- Because only `lo` exists in the namespace, seccomp may allow `AF_INET`/`AF_INET6` with
  `SOCK_STREAM` without weakening the guarantee. The filter behaves
  exactly like this (`backlog/CONVENTIONS.md` 4.10 and 4.11): `socket()` is only
  allowed for the cross product `allow_families` × `allow_types` from the profile;
  `arg1` is masked with `0xff` so that `SOCK_NONBLOCK|SOCK_CLOEXEC`
  pass. Every other family and every other type returns `EPERM`. An
  arch mismatch leads to `KillProcess`. x32 syscalls (`nr & 0x40000000`)
  return `EPERM` via a handwritten BPF prelude before the
  seccompiler program; that is part of the decision, not "if needed".
  `socketpair()` stays untouched by the filter: it only knows `AF_UNIX`, connects
  two descriptors of the same process tree and is not egress (Node and Bun
  need it for their child process IPC). The filter lives in
  `daemon/bin/humanitl-shim/src/seccomp.rs` and keeps, under `#[cfg(test)]`,
  a table of all rules; `docs/SECURITY.md` quotes the list from there.
  The profile `browser` later extends the families with `AF_UNIX` (ADR-0016) —
  the guarantee is then still carried by the network namespace plus the
  mount allowlist.
- The bwrap version and the availability of user namespaces become an
  installation requirement. `humanitl doctor` checks both and offers the
  package command for copying; on failure the sandbox screen disables the
  start with a concrete reason and never offers "start anyway".
- The three guarantees are testable and are tested: escape test 1
  (socket refusal, expects `socketpair()` = ok) and 2 (mount surface)
  check them in the running sandbox, the isolation panel shows the same
  checks live in the UI.

## Addendum 2026-09-23: The shim is PID 1 (HUM-203)

The security pass of 2026-09-23 (finding M1) showed that the process model
without `--as-pid-1` had a gap. bwrap's init carried no seccomp filter, was
"dumpable" and ran under the same UID in the same user namespace as the agent.
With `kernel.yama.ptrace_scope = 0` the agent opened `/proc/1/mem` with
`O_RDWR` (measured against bwrap 0.13.0) and could have executed code without
a filter. `/proc/<pid>/mem` is not a syscall, and `pidfd_getfd` was not in the
floor; both paths are checked only by `ptrace_may_access`.

Decision: `to_bwrap_args` sets `--as-pid-1` in every command line, without a
switch. The flag requires `--unshare-pid`, which every profile has to set
anyway, and does not combine with `--lock-file`, which Humanitl does not use.
`--as-pid-1` alone is not enough, because then the shim parent process would
be the writable PID 1. Therefore the parent process, as its first step after
the `fork`, sets `PR_SET_DUMPABLE` to 0 and opens the gate to the agent only
once `PR_GET_DUMPABLE` confirms it (already so since HUM-138, now load-bearing
for PID 1). `pidfd_getfd` goes into the floor, `pidfd_open` only into the list
of nameable syscalls, because asyncio, Go and libuv probe for it.

Consequences: as PID 1 the shim reaps every orphan (`waitpid(-1)`) and counts
only the status of its own child. Forwarding `SIGTERM` and `SIGHUP` is
load-bearing, because the kernel does not deliver signals without a handler to
a namespace init. When the shim ends, the sandbox ends. `/proc/1/status` now
shows `Seccomp: 2`, and ESC-1 checks PID 1 without exception.

## Affected issues

`HUM-010` (sandbox profile format), `HUM-011` (bwrap launcher), `HUM-012`
(humanitl-shim with seccomp), `HUM-013` (proxy socket bind), `HUM-006`
(escape test harness), `HUM-040` (sandbox screen), `HUM-041`
(isolation check panel), `HUM-043` (`/work` hardening), `HUM-075`
(`humanitl doctor`), `HUM-203` (`--as-pid-1`, addendum).
