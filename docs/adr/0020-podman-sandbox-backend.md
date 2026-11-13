# ADR-0020 · Podman as a selectable second sandbox backend on Linux

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-11-11

## Context

On Linux the sandbox is bubblewrap (ADR-0002). Namespaces, the shim's
seccomp filter and the host kernel form the whole boundary: a kernel exploit
from inside the sandbox reaches the host directly. The owner decided on
2026-11-11 that after 0.1.0 Humanitl gets Podman as a second backend that the
user selects next to bwrap, first as a plain rootless container and later with
a kernel boundary through a micro-VM runtime. bwrap stays the default.

The same decision sets two limits that every part of the design has to keep:

1. **No setting weakens the three guarantees** of
   [`SECURITY.md`](../SECURITY.md#1-the-claim): no network interface, exactly
   one door to the proxy, no new doors. A project's own configuration can
   never lower the isolation level (`backlog/CONVENTIONS.md` 4.11).
2. **An unsafe setting is refused with a `Diagnostic`** (ADR-0012), at load and
   at start. It is never applied silently, and a backend whose prerequisites
   are missing never falls back to another one silently.

Apart from these limits everything is meant to be configurable: the backend
globally, per configuration profile, per session from the command line and
in the interface.

The port `SandboxBackend` (ADR-0015) already has two adapters, bwrap and
Seatbelt (ADR-0019), but only one of them exists per build: `PlatformBackend`
is a compile-time alias, and the daemon, the CLI and the preview reach past the
port into bwrap's builder methods and into the bwrap command line
(`backlog/sprint-5.md`, HUM-292, section "Result (desk part, 2026-11-11)").
`tools/coupling-baseline.toml` records 33 files outside the adapter that name
bwrap. A second Linux backend that the user picks at run time cannot be a
`cfg` branch; it forces the port to become neutral.

Hard constraints: rootless, no service running as root, Debian and Ubuntu as
primary targets, the daemon runs as a hardened systemd user unit
(ADR-0010), the shim (`humanitl-shim`) is the only component inside the
sandbox and stays without tokio.

## Decision

We add a Podman adapter behind the existing port `SandboxBackend` and make the
port backend-neutral first. No new port, no abstraction over Podman: the names
`podman`, `crun`, `krun`, `libkrun` and `conmon` appear only in
`daemon/crates/sandbox/src/podman/`, guarded by a new section in
`tools/coupling-baseline.toml`.

**Selection.** A new configuration key `sandbox.backend` with the values
`auto`, `bwrap`, `podman` and, on macOS only, `seatbelt`. `auto` means bwrap
on Linux and Seatbelt on macOS. It is set in `config.toml`, in a configuration
profile (`[config.sandbox]`), per session with `--backend` on
`humanitl run` and `humanitl sandbox run`, and in the interface. The key is
`x-project-scope = "denied"`. The existing field `sandbox.backend` of a
sandbox profile (`profiles/sandbox/*.toml`) stays as a requirement: if it names
a backend and the effective choice is a different one, the start is refused.

**Levels.** Every backend reports an isolation level, an ordered enumeration
`IsolationLevel`:

| Level | Backends | Boundary |
|---|---|---|
| `policy` | `seatbelt` | the macOS kernel holds the process to a profile |
| `namespaces` | `bwrap`, `podman` with runtime `crun` | Linux namespaces and seccomp, shared host kernel |
| `kernel` | `podman` with runtime `krun` | a guest kernel inside a micro-VM, then the VMM process |

A second key, `sandbox.min_isolation`, takes the same values and is the one
backend key a project may set (`x-project-scope = "allowed"`). For this key
alone the strictest value of all layers wins, so a project can raise the floor
and never lower it. A start whose backend is below the floor is refused.

**Podman, plain level (`sandbox.podman.runtime = "crun"`).** One `podman run`
per session, rootless, started by the daemon (under its hardened unit as a
transient unit of the user's service manager, ADR-0010 addendum, HUM-434); the
shim takes the descriptors of the plan from a handover socket of the session
that accepts exactly one connection, no longer through `--preserve-fds`:

- `--network none`, so the container has a network namespace with only `lo`;
- the proxy socket bound as a single file to the same path as under bwrap;
- `--cap-drop all`, `--security-opt no-new-privileges`, `--read-only` root,
  `--userns keep-id`, `--pids-limit`, `--memory`, `--cpus`,
  `--log-driver none`, `--pull never`, `--rm`, host name `sandbox`,
  `--read-only-tmpfs=false` (no engine tmpfs on `/run`, `/tmp`, `/var/tmp`),
  `--env-host=false` and `--http-proxy=false` (Podman otherwise copies the
  host's proxy variables, credentials included, into the container);
- every configuration source of Podman that the daemon does not write is cut
  off: `CONTAINERS_CONF` names a `containers.conf` the daemon writes (with
  `default_mounts_file` pointing at an empty file as a second line of
  defense, an empty `hooks_dir`, and our own `seccomp_profile`),
  `CONTAINERS_STORAGE_CONF` and
  `CONTAINERS_REGISTRIES_CONF` name files the daemon writes,
  `CONTAINERS_CONF_OVERRIDE`, `CONTAINER_HOST` and `CONTAINER_CONNECTION` are
  removed from the environment, and the `podman run` line names an empty
  directory with `--hooks-dir` and the empty file with
  `--default-mounts-file`, which alone keeps out mounts such as
  `/run/secrets`, because Podman reads the
  user's `~/.config/containers/mounts.conf` in addition to the configured
  `default_mounts_file` (HUM-454);
- the shim compares the mount table inside the container
  (`/proc/self/mountinfo`) with the plan as a multiset of entries (mount point,
  type, source, options, count), including the engine's own entries, before it
  starts the agent; any extra, missing, different or stacked entry refuses the
  start. This holds for bwrap as well;
- the shim as PID 1 (no `--init`), with the same seccomp filter, the same
  five `CHECK` lines and the same control channel for further shells as under
  bwrap;
- the environment of the agent through an inherited sealed memfd that the
  shim reads, never as `-e` values, so that no secret stands in the process
  list or in the container's stored configuration;
- a lifeline pipe whose write end only the daemon holds; when the daemon
  ends, the shim ends the container. Podman detaches the container from the
  process that starts it, so `--die-with-parent` has no direct counterpart.

**Admission.** A level is selectable only when it is admitted. Since HUM-448
the plain level (`crun`) is admitted and `--backend podman` starts it; `auto`
stays bubblewrap. A level not yet admitted (today `krun`):
`sandbox.backend = "podman"` with that runtime, `--podman-runtime krun` and
the interface refuse it with a diagnostic "not yet admitted" (`SANDBOX_048`); only the escape
suite and the tests can start it, through an argument in code, not through a
configuration key; that path is compiled only into the escape-suite launcher
and the tests, never into the daemon or the CLI. The plain level was admitted
by HUM-448 after the mount checks (HUM-437) and the escape suite against
Podman (HUM-435) were green; the kernel level likewise after HUM-445 and HUM-446.

**Podman, kernel level (`sandbox.podman.runtime = "krun"`).** The same
adapter with the OCI runtime `krun` (crun built with libkrun). The guest gets
no network device and no transparent socket forwarding. The door is exactly
one vsock port that libkrun maps to the session's Unix socket on the host;
the host side never uses `vhost-vsock`. Terminal, further shells and the
lifeline run over a second, declared vsock port to a daemon socket, because
descriptors cannot cross into a VM. The guarantees are stated so that they
hold with root inside the guest; the checks in the guest become defence in
depth, and a host-side check of the VMM process carries them. This level is
gated: it ships only after the spike measurements K1 to K3 and the escape suite
against it are green in CI. If crun cannot express "no forwarding, one vsock
port", the fallback is libkrun used directly from the adapter, started inside
the existing bwrap jail; that would be a follow-up decision recorded here.

**The door per level.**

| Backend | Door | Seen by the agent |
|---|---|---|
| bwrap | proxy socket bound as one file | `127.0.0.1:3128`, served by the shim |
| Seatbelt | proxy socket plus one loopback port per session | the session port |
| Podman `crun` | proxy socket bound as one file | `127.0.0.1:3128`, served by the shim |
| Podman `krun` | one vsock port mapped to the session's proxy socket | `127.0.0.1:3128`, served by the shim |

**Configuration shape.** All keys have a default and a tier (ADR-0011), all
are `x-project-scope = "denied"` except `sandbox.min_isolation`:

```toml
[sandbox]
backend = "auto"              # auto | bwrap | podman | seatbelt (macOS)
min_isolation = "policy"      # policy | namespaces | kernel; strictest layer wins;
                              # "namespaces" is an explicit Linux preference

[sandbox.podman]
program = ""                  # empty: podman from PATH
runtime = "crun"              # crun | krun
rootfs = "host"               # host | image
image = ""                    # name@sha256:digest, required for rootfs = "image"
memory_mib = 4096
cpus = 2.0
pids_limit = 1024
start_timeout_secs = 30
```

There is no key for extra Podman arguments, for a network mode, for added
capabilities, devices or volumes outside the sandbox profile, and no switch for
the forwarding of the micro-VM. A value that would need one of them does not
exist, so it cannot be set.

**Neutral port.** `SandboxBackend::plan` takes a `PlanRequest` with profile,
session and `LaunchOptions` (the options that today are builder methods of
`BwrapBackend` and `SeatbeltBackend`); the backend also returns a side-effect
free `PlanPreview` with the exact command line, a neutral policy table
(generalised from Seatbelt's `PolicyRow`), the door and the limits. The daemon
and the CLI hold an enum `Backend` with one variant per adapter instead of the
alias `PlatformBackend`. The sketch is in the HUM-292 result; HUM-428
implements it.

## Rationale

Podman is rootless without a service running as root, it is packaged in
Debian and Ubuntu, and it ships on the GitHub Ubuntu runners, so the escape
suite can run against it in CI without our own runner. `--network none`
gives the same empty network namespace as `--unshare-all`, so guarantee 1 is
again a kernel property and not configuration; guarantee 2 is again one bound
socket file; guarantee 3 is the same shim binary with the same filter on the
same kernel. The plain level therefore needs no restated guarantees, only a
second way to express the same mounts. It is falsifiable by the same
`tests/escape/` run with the backend switched.

What Podman adds over bwrap is the image path: a session can run on an OCI
image with a toolchain or an agent installed, and the same adapter reaches a
kernel boundary by changing the OCI runtime. The kernel level is the reason the
owner wants a second backend at all; the plain level is the step that makes
the port neutral and proves the adapter before a VM adds its own questions.

The isolation floor as a separate key keeps both limits mechanical: the
backend key is never read from a project, and the one key a project may set
can only move one way. A test per direction proves it.

The environment through a descriptor instead of `-e`, the log driver `none`
and the lifeline are consequences of Podman's design, not taste: Podman
writes the container configuration including every `-e` value to disk, logs
container output by default, and runs the container under `conmon`, which
does not end with the process that started it.

Where an assumption is open, the HUM-292 result names the measurement and the
command; the issues in `backlog/sprint-7.md` start only after the plain-level
measurements P1 to P9 are recorded.

## Rejected alternatives

- **Incus.** Its VMs and system containers run under `incusd` as root, and a
  VM has an agent channel next to any door, so there would be two channels and
  a privileged service.
- **microsandbox.** The same micro-VM technique as libkrun, but with its own
  server process and a permanently open console channel to an agent in the
  guest, still beta when last checked (BACKLOG.md 9.12).
- **Kata Containers.** A VM per container, but usually behind a containerd
  running as root, with `vhost-vsock` on the host side, which lets a guest
  reach other vsock listeners of the host.
- **gVisor (`runsc`).** No `/dev/kvm` needed, but no guest kernel either, and
  its compatibility with arbitrary agent toolchains is hard to predict; kept
  as a candidate for hosts without KVM, not as the kernel level.
- **Docker.** A root daemon whose socket is a root shell on the host
  (ADR-0002); the rootless variant brings nothing that rootless Podman does not
  already have, without the packaging advantage.
- **A second Linux backend as a `cfg` branch like Seatbelt.** Cheap for one
  build, but the user is meant to switch per session, so the selection has to
  happen at run time.
- **Do nothing.** bwrap stays the only boundary on Linux; the owner's request
  for a kernel boundary stays open.

## Consequences

- HUM-428 makes the port neutral before any Podman code lands: `PlanRequest`,
  `LaunchOptions`, `PlanPreview`, `IsolationLevel`, the enum `Backend`, and the
  preview and mount table built from the plan rather than parsed from bwrap
  flags. The coupling numbers in `tools/coupling-baseline.toml` fall.
- `tests/escape/` becomes parametrised by backend (`HUMANITL_ESCAPE_BACKEND`),
  and a backend is admissible only when the whole suite is green against it in
  CI. The kernel level adds probes with root in the guest and a host-side
  check of the VMM.
- The daemon's user unit (`packaging/systemd/humanitld.service`) blocks rootless
  Podman as it stands: `PrivateUsers`, `ProtectSystem=strict` without the
  container storage, `ProtectControlGroups` and `NoNewPrivileges` with
  `newuidmap`. Which hardening stays is measurement P8; every relaxation is
  stated in `docs/SECURITY.md` "Service hardening" in the same commit.
- The security documents get a Podman section and new K entries in
  `docs/THREAT-MODEL.md` for the container engine and, at the kernel level,
  for the VMM, the vsock door, the control channel, virtiofs and the guest
  kernel, each in the commit of the code it describes.
- The configuration grows by one group, `sandbox.podman`, and the protocol by
  an RPC that lists the backends with level, availability and missing
  prerequisites (ADR-0018 parity: CLI `humanitl sandbox backends`).
- New risk: the user's own `containers.conf`, `mounts.conf`, `storage.conf`
  and hook directories can add mounts (host secrets under `/run/secrets`),
  devices or hooks to every container. The adapter starts Podman with
  configuration files of its own for every source, and the shim's checks from
  inside stay the last word: an extra mount point, interface or socket stops
  the session before the agent starts.
- New risk: the kernel level depends on libkrun, which Debian does not
  package. Until it does, the level is experimental, and `humanitl doctor`
  says what is missing.

**Amendments this decision needs**, written when the issues landed: the unit
in ADR-0010 with HUM-434, the rest with HUM-441:

- **ADR-0002.** "Docker later as a second backend" becomes "Podman as the
  second Linux backend (ADR-0020)"; the command line as the readable policy
  stays, but the table the interface shows comes from the neutral plan.
- **ADR-0010.** The `.deb` lists `podman` (and later the krun runtime) under
  `Recommends`, never `Depends`; the user unit gets the hardening that P8
  shows rootless Podman needs, or a documented way to start Podman outside it.
- **ADR-0015.** The port table names `bwrap`, `seatbelt` and `podman` as the
  adapters of `SandboxBackend`; `docker` and `microvm` as later adapters are
  replaced by `podman` (with the runtimes `crun` and `krun`).

## Addendum 2026-11-13: The kernel level moves to ADR-0021 (HUM-442)

The kernel level of this decision, Podman with the runtime `krun`, is
replaced. libkrun is not packaged in Debian, and the owner decided that the
level is built only from packages of the distribution. ADR-0021 decides the
replacement: an adapter `qemu` of its own, a QEMU micro-VM with virtiofsd and
vhost-device-vsock, with the measurements of the spike HUM-442. Everything
this decision says about the plain level (`crun`), the selection, the levels,
`sandbox.min_isolation` and the admission stays in force. Where it names
`krun`, `libkrun` or the kernel level of Podman, ADR-0021 applies instead;
the runtime `krun` leaves `sandbox.podman.runtime` with HUM-464.

## Affected issues

`HUM-292` (the spike and its result, `backlog/sprint-5.md`), `HUM-428` to
`HUM-449` (`backlog/sprint-7.md`), `HUM-145` (the coupling ratchet),
`HUM-263` (the pinned project folder), `HUM-350` (the disposable sandbox),
`HUM-448` (the plain level admitted, status Accepted), `HUM-441` (the
amendments to ADR-0002, ADR-0010 and ADR-0015), `HUM-442` (the kernel level
moves to ADR-0021, addendum).
