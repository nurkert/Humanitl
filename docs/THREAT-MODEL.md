# Threat model

Applies to: Humanitl 0.1

This document says against whom the mechanisms of [`SECURITY.md`](SECURITY.md) work and where
they do not. A reader without a security background should know after the first three sections
what Humanitl prevents and what it does not. A reviewer should be able to check every claim: for
each channel the mechanism, the file that implements it and the test that observes it are named.

Humanitl protects two things. **Confidentiality of project data and credentials:** nothing from
the project directory, the agent's environment or the agent's memory leaves the machine over the
network unless a rule or a human approved that concrete request; without a matching rule a human
sees it first, method, destination, headers, parameters and the full request body. **Integrity of
the decision history:** what was decided, when and why cannot be rewritten later unnoticed by
anyone who lacks the HMAC key of the audit chain. A process running as the same user can read that
key and rebuild chain and anchors; against it the chain proves nothing (K-14).
**Availability of the agent is explicitly not a goal:** when an isolation or decision mechanism
fails, the agent stops, and a timeout ends in `block`, never in `allow`. One failure does not stop
the agent at all: a recording outage stops the recording, not the traffic (HUM-250, K-16). And a
red isolation check does stop the agent, but only after it has started, not before (K-15).

Two exceptions are declared and visible: the passthrough to the language model (streamed, fully
recorded, scanned, not held) and the project directory `/work`. A third the human writes into
their own rules: a rule with `stream: true` lets a request whose body is above
`limits.hold_body_cap_bytes` leave streamed, decided on its head, the body unscanned; it never
applies while `hold.hard_block_checksum_secrets` is on.

## Assets

| Asset | Where it lives | Why it matters | Protected by |
|---|---|---|---|
| Project data | the project directory, mounted at `/work` | client code and data under confidentiality and GDPR obligations | the three guarantees; no egress except approved requests and the declared LLM host |
| Credentials | the host's environment, `~/.ssh`, `~/.netrc`, keyrings, tokens in other processes | a stolen token opens systems far beyond the project | PID namespace, mount allowlist, clean environment (`--clearenv`), the LLM key only as a sandbox file (for Claude Code moved into the agent's environment inside the sandbox, HUM-514) |
| LLM prompts | in flight to the LLM host, in the recording | they carry whatever code the agent shows the model | declared side channel; full recording and findings scan; the user chooses the host |
| Audit log | `$XDG_DATA_HOME/humanitl/audit/audit.jsonl`, anchors in `humanitl.db` | evidence of what an agent actually sent out | hash chain with HMAC, anchors in file and database |
| CA key | `$XDG_DATA_HOME/humanitl/ca/ca.key`, `0600` | whoever has it can forge TLS for every connection that trusts the Humanitl CA | never in the sandbox, never in the host trust store; trusted only inside the sandbox |

## Attacker

In every case the attacker is **code running as the agent, with subverted intent**. It has the
agent's rights: it can read and write in `/work`, start processes and talk to the proxy. It does
not have the kernel's rights and not the host's.

| ID | Attacker | How they get in | What they can do | What they cannot do |
|---|---|---|---|---|
| **a** | **Prompt injection** through loaded content | a README, an issue text, a web page, an error message of a package server contains instructions the model reads as a task | steer the agent's tool calls: read files, formulate requests, send text to the model | bring their own binaries; they work with the tools the agent has |
| **b** | **Malicious dependency** | a package the agent installs or runs (`npm install`, `pip install`, a build script) | arbitrary code as the agent user in the sandbox: syscalls, own network attempts, reading every visible file | anything the kernel refuses; no new mounts, no capabilities |
| **c** | **Compromised model** | the local model itself is manipulated (poisoned weights, a substituted endpoint) | everything of (a), and it sees every prompt, potentially the whole source the agent shows it | change anything about the sandbox; it can only steer the agent |
| **d** | **Local reader of the records** | another process or person with file access to `$XDG_DATA_HOME/humanitl` | read recordings, delete or change entries, truncate the file | rebuild the chain unnoticed without the HMAC key |

(a) and (b) differ a lot in power: (a) has to work through the agent, (b) runs code. Every
statement here holds for (b) unless it says otherwise, and therefore for (a) as well. (c) is the
most unpleasant, because the model channel is open; Humanitl reduces it to "the user chose to trust
this host" and makes that choice visible (K-02).

## Trust boundaries

```
 +--------------------------- host, one user account -----------------------------+
 |                                                                                 |
 |  +------------------------+   gRPC over UDS    +------------------------------+ |
 |  | humanitl app / CLI     | <----------------> | humanitld (systemd --user)   | |
 |  | shows, decides         |   daemon.sock 0600 | proxy, rules, recorder,      | |
 |  | (trusted for the       |   + token          | audit, sandbox control       | |
 |  |  decision, not for     |                    | CA key, audit key            | |
 |  |  isolation)            |                    +---------+---------+----------+ |
 |  +------------------------+                       (B2)    |         |  (B3)      |
 |                                       proxy.sock (file)   |         |            |
 |  - - - - - - - - - - - - - - - - - - - - - - - - - - - - -|- - - - -|- - - - - - |
 |  (B1) sandbox: bwrap, own net/pid/ipc/uts/user/cgroup ns  |         |            |
 |  +--------------------------------------------------------v--+      |            |
 |  | shim (PID 1): bridge 127.0.0.1:3128 -> proxy.sock          |      |            |
 |  | agent: seccomp filter, no interface but lo                 |      |            |
 |  | /work (project, ro|rw) <---- shared with the host --------------------- (B4)   |
 |  +------------------------------------------------------------+      |            |
 +----------------------------------------------------------------------|------------+
                                                                        |
                     LLM host (LAN, passthrough rule, recorded)  <------+------>  internet
                                                                   (only after approval)
```

- **B1, sandbox to host.** The kernel boundary: namespaces, seccomp, no capabilities. Everything
  inside is attacker-controlled, including the agent's own permission prompts; they are no
  boundary, and Humanitl switches them off for fetching and commands
  ([`SECURITY.md`](SECURITY.md#the-agents-own-permission-prompts-hum-308)).
- **B2, the one door.** The proxy socket file. Every byte through it is parsed by the daemon,
  which is why the daemon is memory-safe Rust and hardened by systemd.
- **B3, daemon to the network.** Only after a rule or a human allowed the request, resolved once
  after the decision, pinned to the address found.
- **B4, `/work`.** Shared by design. What the agent writes there, the host's tools may later run
  or a human may carry out.
- **The window and the CLI** sit on the host side. They cannot weaken the sandbox, but they show
  the human what they decide about.
- **The daemon and the user's service manager (HUM-434).** Under its hardened user unit the daemon
  runs in a user namespace of its own, where rootless Podman cannot map its IDs. Instead of
  loosening that hardening, the daemon starts every Podman call as a transient unit `humanitl-*`
  of the user's service manager (`systemd-run --user`), outside its own unit. A compromised daemon
  therefore gains **"start a transient user unit"**: any program as the same user, without the
  restrictions of `humanitld.service`. The user account could do that before; the hardened daemon
  could not. The code builds only unit names with the prefix `humanitl-` and one fixed line
  (`--pipe --wait --collect`, `KillMode=mixed`, the program behind `env -i`), which guards against
  mistakes, not against a daemon that is taken over. Under bubblewrap nothing of this is used.
  The sandbox's descriptors reach the shim through a socket of the session that takes exactly one
  connection of the same uid with a one-time token, then refuses every other; only its directory
  is bound, and the daemon removes the socket before sending, so the agent finds nothing there. The token stands in the unit's command line (`systemctl --user show`, D-Bus),
  so a process of the same uid can read it and race the shim; that ends in a failed session, never
  in the descriptors reaching another uid.

## Attack surface

The twelve channels of the security review of 2026-09-02. Severity is the impact **if** the
channel is used, not its likelihood. Status is `mitigated` (a mechanism in 0.1 closes or contains
it), `declared` (deliberately open, shown in the application, not closed) or `open` (a known gap
without a mechanism yet).

| ID | Channel | Attackers | Severity | Mitigation in 0.1 | Status | Test |
|---|---|---|---|---|---|---|
| K-01 | project directory `/work` | a, b, c | high | only a subpath; hooks, CI and editor directories and secret files masked; session summary with touched files, outward symlinks and a secret scan; the mounted folder is the checked one (bound by descriptor under bubblewrap, device and inode checked by the shim under every backend, `SANDBOX_042`) | declared | ESC-5, ESC-2 (`swapped_folder`) |
| K-02 | LLM passthrough | a, b, c | high | exact rule on host, port, scheme, methods and exact inference endpoints; full recording; findings warn | declared | - |
| K-03 | socket directory of the proxy | b | medium | the socket **file** is bound, never its directory | mitigated | ESC-2 |
| K-04 | TCP sockets allowed by seccomp | b | low | empty network namespace and routing table; parent filtered and not dumpable | mitigated | ESC-1 |
| K-05 | host Unix sockets (X11, Wayland, D-Bus, Docker) | b | critical | mount allowlist, denylist in the argument builder (`SANDBOX_006`), the mount table inside equals the plan (`SANDBOX_049`), `AF_UNIX` refused | mitigated | ESC-2 |
| K-06 | PID and IPC namespace | b | high | own PID, IPC and UTS namespaces, own `/proc`, `--new-session`, shim as non-dumpable PID 1 | mitigated | ESC-1, ESC-2 |
| K-07 | `/proc` and `/sys` | b | low | the namespace's own `procfs`, `/sys` not bound | mitigated | ESC-2 |
| K-08 | symlinks out of `/work` | b | medium | resolved inside the mount namespace; marked in the session summary (`SANDBOX_022`) | mitigated | ESC-5 |
| K-09 | terminal escapes (OSC 52, OSC 8, titles) | a, b, c | medium | allowlist filter in the daemon before any client; no OSC handler in the widget | mitigated | ESC-5 |
| K-10 | DNS and the time of resolution | a, b, c | high | resolution only after approval (ADR-006), pinned address | mitigated | ESC-3 |
| K-11 | queue and rule metadata | a, b, c | low | `humanitl.internal` is read-only, `/ask` never creates a rule; the policy oracle stays | declared | ESC-4 |
| K-12 | package caches | b | medium | caches per project, not per trust level; none unless configured | declared | - |

Five further channels complete the model:

| ID | Channel | Attackers | Severity | Mitigation in 0.1 | Status | Test |
|---|---|---|---|---|---|---|
| K-13 | direct network egress | a, b, c | critical | network namespace, seccomp, no capabilities | mitigated | ESC-1, ESC-3 |
| K-14 | audit file on the host | d | high | hash chain with HMAC, anchors in file and SQLite; mitigated only against an attacker without the HMAC key: not against a same-user process that has the key, not a cut behind the last anchor | declared | ESC-5 |
| K-15 | the window between `exec` and `SIGKILL` | b | high | report read at once, `SIGKILL` without grace | open | - |
| K-16 | recording outage | a, b, c | medium | outage reported to every client once per cause, cards marked "not recorded", first approval paused | declared | `daemon/crates/recorder` health tests, golden `states_recorder_failure` |
| K-18 | the door for further shells (`humanitl sandbox exec`) | b | high | unnamed socket pair held only by the daemon and PID 1; the shim forks the shell from inside with the agent's filter on top, proven by a refused `AF_UNIX` and a refused `AF_VSOCK`; at most four; dies with the sandbox; in the stream mode (K-29) the request is a frame on the control stream and the shim opens the shell's pseudo-terminal itself, under the same limits | mitigated | `daemon/crates/ipc/tests/terminal.rs` (HUM-299), `daemon/crates/ipc/tests/terminal_stream.rs` (HUM-443) |
| K-24 | the disposable sandbox: the upper layer and the way back into the project (`humanitl sandbox apply`) | b | high | project folder read-only as the lower layer of an overlay, upper layer in RAM under the daemon's runtime directory with a size cap, `mounts.extra_rw` read-only; applied only on a human's choice, checked before the first write, never through or as a symlink out of the project, no device, FIFO, socket or setuid bit, nothing on, under or above a mask of the profile (HUM-446), no overwrite of a file the human changed since the start unless that change is confirmed one by one (`ApplyChanges.overwrite_ids` from the app, HUM-351) or every selected change is (`--overwrite`); a container-engine backend refuses `disposable` (`SANDBOX_030`) because its lower layer cannot be bound to the checked folder, and `auto` runs direct there (HUM-438); the kernel level builds the overlay in the guest over the read-only project and a layer it gets as a descriptor, and the daemon reads that layer as written by the attacker (K-34, HUM-446) | mitigated | `daemon/crates/sandbox/src/disposable/tests.rs`, `daemon/crates/sandbox/tests/launcher.rs`, `daemon/crates/ipc/tests/sandbox_start.rs` (HUM-350); `daemon/crates/sandbox/src/podman/tests.rs`, `daemon/crates/sandbox/tests/escape_worktree.rs` (HUM-438) |
| K-27 | the container engine under Podman (Podman, `conmon`, `crun`, the user's container storage, `containers.conf`) | b, d | high | admitted at the plain level (`crun`) only after the escape suite against it was green on `main` (HUM-448), the kernel level (backend `qemu`, ADR-0021) a backend of its own (K-30 to K-34, admitted with HUM-449), `auto` never Podman; every configuration source the daemon does not write cut off; fixed flags without keys; the environment only through the sealed memfd; ended through the engine and the lifeline; leftovers removed at daemon start (`SANDBOX_041`) | mitigated | ESC-1, ESC-2, ESC-5 (`escape-tests-podman`, HUM-435); `daemon/crates/sandbox/src/podman/` (HUM-433) |
| K-28 | the image as the root of the sandbox under Podman, and pulling it (origin, digest, the network while pulling) | b, d | high | pinned by digest, checked before every start, never pulled by a start (`SANDBOX_043`); pulled only on request, by the engine directly and not through the proxy, from the registry the name names, said before it runs; registry credentials only from the adapter's own file, handed to the engine as a private copy, and a credential helper there or in a registry drop-in refused (HUM-456); project, door, CA, shim and session files from the host, never from the image; user, entry point, volumes and health check of the image overridden, an image setting `HUMANITL_*` refused | open | ESC-2 `ca_is_the_hosts` (`HUMANITL_ESCAPE_IMAGE`); `daemon/crates/sandbox/src/podman/image.rs` (HUM-439) |
| K-29 | the control stream of the terminals (the stream mode: the kernel level always, bwrap when a caller asks for it) | b | high | one stream socket held only by the daemon and PID 1, opened before the agent's filter, which refuses `AF_VSOCK` and `AF_UNIX`; in a guest the host opens it as the second connection on the shim's one listening port, the guest never opens one; terminal bytes and control only, never a byte to or from the proxy; frames of at most 16 KiB, oversized frames, unknown kinds and unknown ids dropped by both ends without ending the stream; its end is a lifeline | mitigated | ESC-1 `socket_af_vsock` in `daemon/crates/ipc/tests/terminal_stream.rs`, fuzz target `control_stream_reader` with `daemon/crates/sandbox/tests/regressions/`, `daemon/crates/sandbox/src/qemu/kvm_tests.rs` (HUM-443) |
| K-30 | the virtual machine monitor of the kernel level (QEMU `microvm`) | b | high | admitted only after `kvm-tests` and `escape-tests-qemu` were green on `main` (HUM-449), chosen only by a person and never by `auto`; no network device, console, monitor or display; full `-sandbox`; in a bubblewrap jail without capabilities with `/dev/kvm` and the kernel file only; inside a hardened transient unit of the user with `PrivateNetwork`; the host checks its descriptors and its own command line after the start (`SANDBOX_047`) | mitigated | `daemon/crates/sandbox/src/qemu/host_check.rs`, `daemon/crates/sandbox/src/qemu/kvm_tests.rs` (HUM-444); CI jobs `kvm-tests` and `escape-tests-qemu` (ESC-1 to ESC-5 and `tests/escape/guest-root.sh` with root in the guest, HUM-445, HUM-449) |
| K-31 | the vsock door of the kernel level | b | high | one vsock device process per session with one device and its own group, never `--forward-cid`; on the host exactly one listener `<uds>_5000`, forwarded to the session's proxy socket; every other port finds no listener; at most 64 open connections through the door per session, one above that closed at once and the first such noted in the evidence of `single_socket` (HUM-467) and in the audit log as `sandbox.door_capped` (HUM-477); the host check refuses any other socket in the runtime directory and a second device | mitigated | `daemon/crates/sandbox/src/qemu/kvm_tests.rs` (`an_extra_listener_beside_the_door_turns_the_host_check_red`, HUM-444); `daemon/crates/sandbox/src/qemu/session.rs` (`the_connection_above_the_cap_is_closed_and_the_others_keep_working`, HUM-467); CI jobs `kvm-tests` and `escape-tests-qemu` (ESC-1 to ESC-5 and `tests/escape/guest-root.sh` with root in the guest, HUM-445, HUM-449) |
| K-32 | the guest kernel (root in the guest) | b | high | the guarantees are carried on the host, so they hold with root in the guest; automatic module loading off, then the plan's vsock modules loaded and module loading locked before the agent; the guest root a read-only share without the runtime directory and without the jail's `/proc`; writable only `/work`, the profile's tmpfs and `/run`, which the shim mounts as tmpfs in the guest with `/run/humanitl` read-only on top (HUM-466), `mounts.extra_rw` refused (`SANDBOX_051`) | mitigated | `daemon/crates/sandbox/src/qemu/kvm_tests.rs` (HUM-465, HUM-444); escape suite with root in the guest (HUM-445); CI jobs `kvm-tests` and `escape-tests-qemu` (ESC-1 to ESC-5 and `tests/escape/guest-root.sh` with root in the guest, HUM-445, HUM-449) |
| K-33 | the four helper processes of a session (two file system daemons, the vsock device, the emulator) and the unit's first process | b, d | high | each in its own bubblewrap jail with every namespace unshared and no capability, only `/usr` of the host, what it serves and its own socket directory; the root daemon read-only with its socket as a descriptor, the `/work` daemon on the checked descriptor with the profile's masks; nothing inherited from the daemon but three descriptors over a one-connection handover; the unit ends with the daemon through the lifeline, also on `SIGKILL`, and with the daemon's unit through `BindsTo`; each file system daemon at most 65,536 descriptors (`LimitNOFILE` of the unit), so a guest cannot pin host memory without bound (HUM-469); the guest's file cache on (`--cache auto`) and trimmed by PID 1 in the guest whenever more than 32,768 entries are unused (checked every 100 ms), so a walk over the root no longer exhausts a daemon (HUM-476), and whenever the guest holds more than 49,152 inodes and 8,192 more than after the last trim, at 57,344 at the latest, so directory chains let go after a trim leave as well (HUM-483); what the guest holds in use still counts against the limit; the three files of the trim only PID 1 holds, never the agent (HUM-483); the host check refuses every process of the unit that runs another program than the script's shell, bubblewrap, a file system daemon, the vsock device or the emulator, by `/proc/<pid>/exe` against the configured and the resolved paths, a deleted or unreadable program included (`SANDBOX_047`, HUM-468), or that runs one of them more often than the script starts it (HUM-480); the unit runs no file of the host but its programs, their loaders and libraries (`NoExecPaths=/`, `ExecPaths=`, also after admission, HUM-480); each jail hides the unit's other programs, allows no further user namespace, keeps no writable mount without `noexec` and refuses `execveat` and an executable `memfd` (HUM-557); the whole unit binds at most `MemoryMax` (the machine's memory, 512 MiB for the helpers and the size of every tmpfs it writes to, 64 MiB per mask over the project), as much swap again (`MemorySwapMax`) and `TasksMax` (the vCPUs and 128) processes and threads; without the cgroup controllers `memory` and `pids`, or when the running unit's cgroup does not carry the planned limits, the session is refused (`SANDBOX_052`, HUM-485) | mitigated | `daemon/crates/sandbox/src/qemu/kvm_tests.rs` (`a_killed_daemon_ends_the_unit_within_five_seconds`, HUM-444; `an_extra_sleep_in_the_unit_turns_the_host_check_red`, HUM-468; `a_helper_that_starts_a_foreign_program_fails_at_exec`, HUM-480; `own_code::a_helper_reaches_no_code_of_its_own_choosing`, HUM-557; `a_process_in_the_unit_cannot_start_more_tasks_than_tasks_max`, `a_unit_without_its_limits_is_refused_with_sandbox_052`, `a_write_beyond_a_mask_s_size_ends_with_enospc`, HUM-485; `a_walk_over_deep_directory_chains_stays_below_the_descriptor_limit`, `the_agent_holds_none_of_pid_1s_cache_descriptors`, HUM-483), `daemon/crates/sandbox/src/qemu/host_check.rs` (`every_process_of_the_unit_runs_one_of_its_programs`, HUM-468; `every_program_runs_as_often_as_the_script_starts_it`, HUM-480); CI jobs `kvm-tests` and `escape-tests-qemu` (ESC-1 to ESC-5 and `tests/escape/guest-root.sh` with root in the guest, HUM-445, HUM-449) |
| K-34 | the shared file systems of the kernel level (virtiofs): the guest root, the project folder at `/work` and the layer of a disposable session | b | high | every shared folder is opened and checked by the daemon and reaches its file system daemon only as that descriptor (`--bind-fd`), never as a path; the shim compares the inode of `/work`, or of the overlay's lower layer, with the plan; the host check finds the opened folder itself at the one daemon that shares it, a descriptor that names the share and carries the plan's device and inode (`SANDBOX_047`), which root in the guest cannot forge; in a disposable session the project is read-only twice (its daemon with `--readonly`, its jail binding the checked folder with `--ro-bind-fd`), the layer comes through a third daemon, and the daemon reads it only after every process of the unit has ended, with the scan of bubblewrap's disposable sandbox plus a refusal of every change on, under or above a mask of the profile; the shim lays the profile's masks over the overlay as bubblewrap does, and the layer's daemon takes only `user.overlay.*` attributes (`--xattrmap`, HUM-481) | mitigated | `daemon/crates/sandbox/src/qemu/kvm_tests.rs` (`a_folder_swapped_after_the_plan_stays_the_checked_folder`, `a_disposable_session_never_writes_to_the_project`), `daemon/crates/sandbox/src/qemu/kvm_tests/masks.rs` (HUM-481), `daemon/crates/sandbox/src/qemu/host_check.rs`, `daemon/crates/sandbox/tests/escape_worktree.rs` (ESC-5 against `qemu`, HUM-446); CI jobs `kvm-tests` and `escape-tests-qemu` (ESC-1 to ESC-5 and `tests/escape/guest-root.sh` with root in the guest, HUM-445, HUM-449) |
| K-35 | a decision proxy of the user behind Humanitl (`egress.proxy`, opt-in), and with `hold.ask_mode = "upstream"` the one who decides requests without a rule | a, b, c | high | off by default; `upstream` only with a proxy the running daemon uses (`CONFIG_003` otherwise), `block` never reaches it (the declared LLM passthrough, ranked ahead of block rules, does, as any allow), `redact` and the hard block on verified secrets stay in Humanitl, recorded as `forward` by `upstream` with the proxy's status (HUM-512); only `http://` to it, from the daemon on the host, never reachable from the sandbox; every allowed request of the agent leaves through it or not at all (`PROXY_012` to `PROXY_017`, never a direct fallback); its CA trusted only on that way, in a `ClientConfig` of its own; with `egress.proxy_resolve = "proxy"` (default of the opt-in) Humanitl resolves nothing and refuses private literals and local names before forwarding, with `"humanitl"` it resolves and checks every address, which binds only a proxy that connects to the address in `CONNECT` (not Burp Suite 2026.11.2, HUM-524) (ADR-0022); all keys closed to project profiles; credentials in the keyring, or the `0600` fallback file (`CONFIG_019`), never in `config.toml` | declared | `daemon/crates/proxy/tests/decision_proxy.rs` (HUM-511), `daemon/crates/proxy/tests/upstream_ask_mode.rs` (HUM-512) |
| K-36 | the agent depot (`agents.source = "depot"`, opt-in, or a company lock), first part: the agent's origin, its mounts and the masks of host installs | b, d | high | off by default; all keys closed to project profiles; a lock may only name `depot` and binds every way of writing the source (`AGENT_010`); the depot owned by the daemon's user, `0700`/`0600`, checked on every open (`AGENT_013`), nothing of it run on the host; one guard on the final plan request in every backend (`AGENT_017`); the resolved version directory read-only, trust-checked entry by entry before the bind; the configuration as a writable scratch copy outside the depot, deleted with the session; host install locations masked (empty read-only mounts under Linux only where they exist, deny rules `file-read*` and `process-exec` after the read rule under Seatbelt); the kernel level refuses a depot session (`AGENT_015`); second part: install and update only as a maintenance run in a sandbox without a project (its own work directory as `HOME`, no `/work`, no profile extension), only the recipe's hosts without a hold as rules of that run, the result checked, verified and switched by the daemon, the version pinned; third part: login and configuration runs of the depot program without a project, only the credential file goes back, through a guard (one file by `openat` with `O_NOFOLLOW`, regular, at most 64 KiB, the adapter's format, against the stored file no key more or fewer, never older, never another account and never one without a named account, values and modes of the format kept (HUM-515), the daemon's own serialisation written atomically under a lock, every outcome in the audit log with SHA-256 only); configuration changes only after confirmation; optionally every credential file sealed at rest (`agents.encrypt_credentials`, ChaCha20-Poly1305, the key only in the system keyring, refused without one, HUM-523) | mitigated | `daemon/crates/sandbox/src/agent/depot/`, `daemon/crates/sandbox/tests/agent_depot.rs`, `daemon/crates/config/src/agents.rs`, `daemon/crates/ipc/src/agents/tests.rs` (HUM-518), `daemon/crates/sandbox/src/agent/depot/maintenance/tests.rs` (HUM-519), `daemon/crates/ipc/tests/depot_maintenance.rs` (HUM-563, HUM-520), `daemon/crates/sandbox/src/agent/depot/credential/tests.rs`, `daemon/crates/sandbox/src/agent/depot/account/tests.rs` (HUM-520), `daemon/crates/sandbox/src/agent/depot/seal/tests.rs` (HUM-523) |
| K-38 | the daemon's memory over a long session | b | medium | at most `limits.registry_max_finished_flows` (default 10,000, closed to project profiles) finished flows stay in the flow registry, the oldest leaves first and only once the recording has had all its events; held and running flows never leave and are bounded by `limits.hold_max_flows` and `limits.max_client_connections`; the queue to the audit writer holds at most 4,096 records, and a full queue makes the sender wait instead of dropping, so a sandbox notice (HUM-477) is never lost there (HUM-556); flow events the audit sink loses behind the broadcast leave an `audit.events_dropped` record with their number in the chain, and the sink closes only flows it has not heard from since its previous gap, so its table holds at most the open flows of two gaps (HUM-564) | mitigated | `daemon/crates/proxy/src/registry.rs` (`finished_flows_above_the_cap_are_evicted_and_a_held_flow_stays`), `daemon/crates/proxy/tests/registry_memory.rs`, `daemon/crates/audit-log/tests/writer.rs` (`a_burst_against_a_stalled_writer_is_bounded_and_loses_no_sandbox_record`, `a_record_queued_behind_stop_is_written_before_the_end`), `daemon/bin/humanitld/src/audit_sink.rs` (`a_forced_lag_leaves_exactly_one_record_with_the_number_of_lost_events`, `a_lagging_sink_closes_only_flows_it_no_longer_hears_from`) |

K-13 is the obvious channel and the only one considered closed outright. K-14 is here so that
attacker (d) appears in a row. K-15 came with HUM-041: the check that proves the guarantees runs
when the agent has already started. K-16 came with HUM-250: an attacker who can fill the disk can
make approved traffic leave without a record.

### K-01 The project directory `/work`

The agent needs write access to the project. Whatever it leaves there is on the user's disk
afterwards: a file with credentials in plain text, or a commit that pushes data into a foreign
repository the next time the human pushes. Only a subpath is bound, never `$HOME`;
`work_mode = "ro"` is the suggestion for analysis sessions. Directories like `.git/hooks`,
`.vscode`, `.idea` and `.github/workflows` are covered with an empty `tmpfs`, files like `.envrc`,
`.env`, `.git/config`, `.npmrc` and `Jenkinsfile` with an empty read-only file from a sealed memfd
(full list in `profiles/sandbox/default.toml`), so a written hook does not run on the host at the
next commit. The project directory and `mounts.extra_ro`/`mounts.extra_rw` are opened once as
descriptors, checked at the location `/proc/self/fd/N` names and bound by descriptor (HUM-263), so
swapping a directory for a symlink between check and mount no longer works; the descriptor never
reaches the agent. Under Podman, which takes no descriptor as a mount source, such a swap does
reach the container: before the agent exists the shim compares device and inode of `/work`, of
every entry of `mounts.ro`, `mounts.extra_ro` and `mounts.extra_rw` and of the agent program with
the pair the launcher saw when it opened the source, under every backend, and refuses the start
with `SANDBOX_042` (HUM-437). Under Podman the swap is therefore detected and refused, under
bubblewrap it is prevented. *Residual risk:* the agent can rearrange, disguise or plant data in the project
that a human later carries out; Humanitl sees that only if the human reads the diff.

### K-02 The LLM passthrough

The agent must send code to the model; these requests are not held. The model host is therefore
the second open channel and, unlike `/work`, one over the network. The rule is as narrow as
possible: one host, one port, one scheme, and thirteen exact inference endpoints instead of the
surfaces `/v1/` and `/api/` (HUM-261), each with only the method it needs: `POST` on the
endpoints that compute, `GET` on those that give information, one rule each (HUM-568); a path
with `..` segments is held (HUM-204).
Traffic is recorded and scanned (`LLM_005`; every finding stays on its request, the warning
repeats only for findings new in the run or last warned about 15 minutes or more ago; an expired
or evicted fingerprint warns again, HUM-354), and with
`hold.hard_block_checksum_secrets` a checksum-confirmed secret is blocked (`HOLD_004`). The access key of a provider (HUM-140) is entered only through the RPC
`LlmKey` (the form of the app, `humanitl llm key set` from `stdin`, or the settings view
`humanitl settings` without echo, sent once; never `SetConfig`), lives in
the system keyring (on macOS the login Keychain, HUM-349; the Seatbelt profile reaches no
`securityd` and no `~/Library/Keychains`, so the agent cannot ask for it) and reaches the sandbox
only as a sandbox file. For Claude Code, which reads its key only from its environment, a shell
inside the sandbox reads that file and exports `ANTHROPIC_API_KEY` (or `ANTHROPIC_AUTH_TOKEN`
towards a gateway) before it starts the agent; the key never stands in the plan's environment or
in an `argv` (HUM-514). Codex gets the same shell, which exports `OPENAI_API_KEY` for the provider
the adapter declares in its `config.toml` (HUM-515), and so does the Antigravity CLI, which
exports `GEMINI_API_KEY` (HUM-517); the host's keyring, where `agy` keeps a Google sign-in, and
the session bus never enter a sandbox. It is masked in the recording and a
finding everywhere except in `Authorization`, or `X-Api-Key` for Claude Code, towards the agent's
inference endpoint (`llm.endpoint`, for Claude Code without it `api.anthropic.com:443`, whose
passthrough covers only `/v1/messages` and `/v1/messages/count_tokens`, for Codex with a key and
without it `api.openai.com:443`, whose passthrough covers only `/v1/responses` and
`/v1/models`, for the Antigravity CLI with a key and without it `x-goog-api-key` towards
`generativelanguage.googleapis.com:443`, whose passthrough covers only `/v1beta/models` and
`/v1/models` and what lies below them). With several saved model
servers (HUM-289) each has its own keyring item, and only the active server's origin gets a key and
is the passthrough of the next session; a probe sends a saved server's key only to that server's own
origin, never to another's; an endpoint from the environment or the command line above the active
server, a shared reference or a missing active server leaves the key unsent. Since HUM-400 the
boundary is the saved server the session resolved to: `llm.server` from the run, the project
profile or a session profile beats `llm.active`, only that server is the passthrough and gets its
key, and a name without a saved entry refuses the start (`CONFIG_021`) instead of falling back.
*Residual risk:*
complete.
Whoever steers the agent through prompt injection can put project content into a prompt. If
`llm.endpoint` is a name, whoever controls that name can redirect the passthrough between two
requests, and because the rule carries `allow_private`, the router or `169.254.169.254` are valid
targets; use an address or `resolver.overrides`. The agent can read the key and send it elsewhere,
where it is a finding but held only at a host without an allowing rule. A cloned repository can
choose another of the user's saved servers with `llm.server` in its project profile and so route
the model traffic of its own session there, for example to a company server instead of a local
one; it cannot define a server, an endpoint or a key, and the choice is shown with its
source in the sandbox header, in `humanitl sandbox plan` and at the start of `humanitl run`. Since
HUM-405 a choice of a server other than the default starts only after the human confirmed that
server once for that project folder (`CONFIG_022`); the confirmation lives in the daemon's data
directory, outside every project, so the agent, which can rewrite the project profile, cannot
issue it. A bundled profile the project names with `name`, whose copy carries a server the human
assigned, counts as the project's choice and asks the same question. The confirmation names a folder
and a server, not the content of the repository: a repository that changes under a confirmed folder
keeps the confirmation for the same name; removing, renaming or re-adding a server, or a new origin
of its endpoint, drops it. An edit that moves a server to another origin drops its key, so a stored
key never follows an address it was not entered for. Do not save a server that no repository may
reach.

With a model in the cloud the second channel ends at a provider outside this machine (HUM-516): the
files the agent reads and the output of its tools travel to it as part of the requests, by design.
Humanitl derives the case from the host of the model endpoint the adapter names, without resolving
anything, and names it in the session header, at the start of `humanitl run` and `go`, in `humanitl
doctor` and when the agent or the endpoint is chosen; the first start per agent and provider asks
once, the daemon refuses an unanswered first start (`LLM_011`) from the fully resolved start
configuration, and the confirmation lives in the daemon's data directory, outside every project. A saved
server marked `local = true` counts as local only for its own origin. *Residual risk:* the notice
informs and blocks nothing; whoever confirms it accepts that the provider sees what the agent reads.
A public name that points into the own network counts as outside until it is marked.

### K-03 The socket directory of the proxy

If Humanitl bound the directory holding the socket, every file created there later would be visible
in the sandbox, and the agent could create sockets there through which two sandbox processes talk
past the recording. The socket **file** is bound, the host directory has mode `0700` and is
separate from the daemon's gRPC socket, which does not exist in the sandbox. The isolation check
counts the sockets in the sandbox. *Residual risk:* a bug in the proxy itself; whoever takes over
the proxy process has the host's network, which is why the daemon is Rust and hardened.

### K-04 TCP sockets allowed by seccomp

The filter must allow `socket(AF_INET|AF_INET6, SOCK_STREAM)`, because that is how the agent
reaches the proxy. Guarantee 3 is therefore not "no sockets" but "no others". The safety of this
socket is carried by the namespace: only `lo`, an empty routing table, no capabilities, so
`connect()` to a LAN address ends in `ENETUNREACH`. The shim parent holding the bridge is the only
process allowed `AF_UNIX`; it carries a filter with the same deny list plus that one family, and is
not dumpable since HUM-138, so `ptrace`, `/proc/<pid>/mem`, `/proc/<pid>/fd` and `pidfd_getfd(2)`
are refused to the agent even with the same UID. Both filters refuse `seccomp()` with
`SECCOMP_FILTER_FLAG_NEW_LISTENER`. Fixed with HUM-203 (finding M1 of 2026-09-23): PID 1 used to be
bwrap's unfiltered, dumpable init, writable through `/proc/1/mem` at `ptrace_scope = 0`; the shim is
now PID 1, filtered and non-dumpable, and refuses to start the agent if the flag cannot be set.
*Residual risk:* a loopback service the agent starts itself is reachable for it; that is
communication between two processes that both belong to the attacker, not egress.

### K-05 Host Unix sockets

One wrong mount voids everything: `/tmp/.X11-unix` gives keyboard and screen, the Wayland socket
the same, D-Bus starts host processes, `/var/run/docker.sock` is root on the host. The profile is an
allowlist; the full command line of the sandbox (bwrap on Linux, `sandbox-exec` with its Seatbelt
profile on macOS) is visible in `humanitl sandbox argv`; the argument builder refuses
`$XDG_RUNTIME_DIR`, `/tmp`, `/run`, `/home`, `~/.ssh`, `~/.gitconfig`, `~/.netrc` and these sockets
(`SANDBOX_006`); the isolation check looks for unexpected sockets in the running sandbox; before
the agent starts the shim compares the mount table inside with the one the plan expects, entry by
entry, so a mount the plan does not hold (with or without a socket in it) refuses the start with
`SANDBOX_049` (HUM-437); and after the filter `AF_UNIX` is refused anyway. *Residual risk:* a user
writing their own profile that adds a file mount (not a socket) such as `~/.ssh` read-only; that
mount is in the plan, and neither check refuses it.

### K-06 PID and IPC namespace

Without its own PID namespace the agent would see every process of the user and read tokens from
`/proc/<pid>/environ`; without its own IPC namespace it would reach shared memory and message
queues. The sandbox has its own PID, IPC and UTS namespaces, a fresh `/proc`, `--new-session` (no
shared controlling terminal, so no `TIOCSTI` injection) and `--die-with-parent`. `--new-session`
stays although a session at the daemon's pseudo-terminal now has a controlling terminal
(HUM-280): it detaches the sandbox from the terminal that started bwrap, and only then the shim's
child takes the daemon's own pseudo-terminal with `setsid` and `TIOCSCTTY` (argument 0, never
stealing). The user's terminal is never handed to such a session, so `TIOCSTI` can only write
into the input of the sandbox's own pseudo-terminal, whose master the daemon holds; a sandbox with
inherited descriptors never asks for a controlling terminal, and a terminal that already is a
session's controlling terminal cannot be taken without `CAP_SYS_ADMIN` in the initial user
namespace. PID 1 is the shim
(HUM-203), which reaps orphans and relays `SIGTERM` and `SIGHUP`; it is not dumpable, so
`/proc/1/environ` and `/proc/1/mem` are unreadable for the agent. Where a backend starts the
sandbox detached from the daemon and `--die-with-parent` has no counterpart (Podman, whose
container runs on under `conmon` when the client is killed), the shim's lifeline takes its place
(HUM-430): the read end of a pipe whose only write end the daemon holds with `CLOEXEC`; when the
daemon ends, also through `SIGKILL`, PID 1 sees `POLLHUP` and ends the sandbox as on `SIGTERM`,
with `SIGKILL` to every process of the namespace after `KILL_GRACE`. The agent never holds the
lifeline. *Residual risk:* none known, as long as bwrap implements the options as documented.

### K-07 `/proc` and `/sys`

Even an own `procfs` reveals kernel version, modules and CPU details; `/sys` would reveal hardware
and, if bound, write paths into the kernel. `/proc` is the new namespace's, `/sys` is not bound.
*Residual risk:* kernel information stays readable; for an attacker who already runs code that is
no gain.

### K-08 Symlinks out of `/work`

The agent creates `/work/x -> /home/user/.ssh`. Inside the sandbox it leads nowhere, because `/home`
does not exist there. It becomes dangerous **after** the session, on the host, when a tool of the
user follows it. The session summary marks every new symlink whose target lies outside `/work`
(`SANDBOX_022`), and the daemon reads the tree with `openat2` and `RESOLVE_BENEATH |
RESOLVE_NO_SYMLINKS`, never following a link. *Residual risk:* the user overlooks the mark.

### K-09 Terminal escapes

The agent's output is attacker-controlled. OSC 52 writes to the host clipboard, OSC 8 hides a
foreign URL behind harmless text, title escapes change the window. The daemon filters before any
byte reaches a client (`humanitl_core::TerminalFilter`, policies `ColourOnly` and `FullScreen`), in
three spellings of every introducer and only as well-formed UTF-8; the ring buffer for late clients
holds filtered bytes; the widget registers no clipboard handler and refuses OSC 52 writes and
reads explicitly, as a second wall behind the filter (HUM-313); colour queries that pass the filter
(OSC 4, 10, 11, `CSI ? 996 n`) the emulator answers with the colours of the theme. *Residual risk:* a bug in filter or
widget, and text that merely looks like a Humanitl message, which is why the "a request waits"
statement stands in a strip above the terminal drawn from the event stream.

### K-10 DNS and the time of resolution

A host name carries up to 63 bytes per label. Whoever asks for `<encoded data>.attacker.example`
has exfiltrated as soon as anyone resolves it. Humanitl resolves only **after** approval, once,
through the `Resolver` port, and pins the address; private, loopback, link-local and CGNAT
addresses are refused unless the rule carries `allow_private`. *Residual risk:* after approval the
name leaves by nature, and the *time* of an approval stays observable, a slow one-bit signal if the
human approves readily.

### K-11 Queue and rule metadata

The agent learns from behaviour what is allowed: an immediate answer means a rule, a `403` with
`reason: rule` an explicit block, a long delay a human looking. The proxy also answers the virtual
host `humanitl.internal` with status and rules. That host is read-only and answered without
upstream; `/ask` creates a card in the window, never a rule. *Residual risk:* the oracle stays, by
design: an agent that knows PyPI is allowed and GitHub blocked bothers the human less.

### K-12 Package caches

A cache shared across projects (`~/.npm`, `~/.cache/pip`, `~/.cargo`) carries data from one project
into another. Caches are per project, and whoever configures none gets none. *Residual risk:*
whoever adds a shared cache to save time rebuilds the channel.

### K-13 Direct network egress

A TCP connection to the internet, a UDP packet, a QUIC handshake, an ICMP tunnel, a raw socket: none
exists. There is no interface but `lo`, no route, no capability to create one, and the filter
refuses `SOCK_DGRAM`, `SOCK_RAW` and every family except `AF_INET`/`AF_INET6`. Refused `socket()`
calls are counted and shown since HUM-138; a `connect()` failing at the empty routing table is not.
*Residual risk:* a kernel bug.

### K-14 The audit file on the host

Whoever changes the JSONL file changes the answer to "what went out back then". Every record
carries `seq`, `ts`, the predecessor's hash and its own over canonical JSON, sealed with HMAC; every
`audit.anchor_every` records and at shutdown an anchor stands in the file and in `audit_anchors`.
*Residual risk:* an attacker running as the same user has key, file and database and can rebuild
everything; truncating behind the last anchor is not detected; retention deletes the start of the
chain, documented by a sealed `audit.pruned` (details in
[`SECURITY.md`](SECURITY.md#8-the-audit-log)).

### K-15 The window between `exec` and `SIGKILL`

The shim reports the three guarantees and starts the agent right after; it enforces nothing
(`CONVENTIONS.md` 4.12). Between the agent's `exec` and the daemon's `SIGKILL` the agent runs in a
sandbox whose isolation is not proven yet: the bridge stands, the proxy accepts, an `allow` rule
forwards without a human, the passthrough streams. In the case of `SANDBOX_015` the second socket
the check complains about is usable in that window. The daemon reads the report at once and kills
without grace; the upper bound is 10 s in the daemon with bubblewrap or Seatbelt and 15 s for
`humanitl sandbox run` and `escape-launch`. At the kernel level and with the container backend the
budget is 40 s and about 95 s, the sum of their command timeouts; it is not a bound, because a
`systemctl` or engine command stuck in uninterruptible sleep extends it
([`SECURITY.md`](SECURITY.md#the-window-between-exec-and-sigkill)). Every request no rule decides still waits for a human. Closing it means the proxy
refuses connections until the isolation is proven, a separate issue. `a_second_socket_in_the_project_stops_the_start`
in `daemon/crates/ipc/tests/sandbox_start.rs` proves the session is stopped and never reported as
running; that nothing leaves in the window is **not** proven. The verdict and the stop do not
depend on the starting client: the daemon stops the sandbox before it sends the results, so a
client that closes its event stream after `Status(starting)` leaves no sandbox running with a red
result, and the finding is recorded in the session summary (`stopped_by`)
(`a_client_that_leaves_does_not_keep_a_red_sandbox_running` in the same file). If the stopping
thread fails, a second `SIGKILL` goes through the kept handle. On both paths, a process without an
exit status that `/proc` still lists, or whose state cannot be read, is reported as a blocking
`SANDBOX_029`, not as stopped, and stays registered, so no new start is accepted until a stop.

### K-16 Recording outage

If the recorder cannot open or write its database (disk full, database locked, read-only file),
approved requests and the passthrough still leave, but are not recorded. Attackers (a) to (c) can
provoke this by filling the disk from `/work`. The daemon sends `RECORDER_001` or `RECORDER_003` to
every client once per cause, and first to a client that connects during the outage; the status bar
turns red, every new card says "not recorded", and the first approval of the session pauses for a
confirmation. `RECORDER_005` ends the outage. Lost flows are not written afterwards. *Residual
risk:* traffic during the outage, above all through `allow` rules and the passthrough, which no
human sees, has no record; the claim "both are recorded" is visibly false for that time.

### K-17 The agent program from a per-user directory

When the agent lives in `~/.opencode/bin`, `~/.local/bin` or `~/bin` (or an absolute
`agent.command` points outside the binds), the daemon binds that one file read-only at
`/run/humanitl/agent/<name>` (HUM-278). From inside, attackers (a) to (c) can read and run the
file, which they already run as the agent; they cannot write it (read-only bind), and they see no
other file of the directory. The daemon binds only a native ELF executable owned by the user or
root whose file and directory nobody else can write (group write only for a group shown to be the
user's private group). The launcher opens the file once with `O_PATH | O_NOFOLLOW`, checks owner,
mode and ELF header through that descriptor and binds the descriptor, so a swap of the path or of
a parent directory after the check does not change what the sandbox gets. A command given after
`--` is never bound this way. Since HUM-280 a session with such a command (`humanitl run -- bash`)
still gets the configured agent's bind beside it, by the same rules, and `/run/humanitl/agent`
first on `PATH`; that is the same file a daemon-started agent would get, and the caller's own
command stays unbound. *Residual risk:* whoever can write as the user, or holds write
access to an ancestor directory before the check, can substitute another file owned by the user
or root; that is outside the threat model (see [Out of scope](#out-of-scope)).

### K-18 The door for further shells

`humanitl sandbox exec` (RPC `Exec`, HUM-299) opens a shell next to the agent. The daemon does not
enter the sandbox's namespaces; it asks PID 1 over a `SOCK_SEQPACKET` pair the launcher creates
only for a session at the daemon's pseudo-terminal, and the shim forks the shell from inside. That
pair is a second channel between the sandbox and the host next to the proxy socket, so attacker
(b) would try three things. (1) *Ask for a shell from inside, or send the daemon something on the
pair.* The pair has no name in any filesystem; inside, only PID 1 holds its end (the agent's child
closes it, it carries `CLOEXEC`, every further shell closes all descriptors above 2), and PID 1 is
not dumpable, so `/proc/1/fd` and `pidfd_getfd` do not reach it. Even a request from inside would
start nothing the agent cannot already run itself. The daemon reads only nine-byte answers for ids
it asked for and never accepts a descriptor from the pair. (2) *Get a shell with fewer limits than
the agent.* The shell inherits PID 1's filter, which allows `AF_UNIX`, and installs the agent's
filter on top before `exec`; the shim checks that `socket(AF_UNIX)` is refused and does not start
the shell otherwise. Mounts, namespaces, environment and the proxy are the agent's, because the
shell is forked inside the same sandbox. (3) *Outlive the session or exhaust the host.* At most
four shells live at a time (shim and daemon both count); PID 1 kills them when the agent ends,
they die with the forking thread and the PID namespace, and the daemon ends their terminals when
its end of the pair closes. The only caller on the host side is the `Exec` RPC, behind the
daemon's token on the daemon's socket, which is never bound into the sandbox; shell ids carry a
part of the sandbox id, so an id from an earlier session reaches nothing. *Residual risk:* refused
`socket()` calls of a further shell are refused but not counted (K-13 stays visible for the
agent only), because the filters refuse a second listener. The shell is not confidential against
the agent: it runs with the agent's UID in the same PID namespace without `hidepid`, so the agent
can read what is typed into it and write into its terminal. Never type secrets there. The default
shell runs no start files (`bash --norc --noprofile`, `sh` without `ENV`), so an rc file the agent
planted does not run when it opens. Isolating further shells from the agent (a sub-user namespace
with another UID, or `hidepid`) is a candidate for a later issue.

*In the stream mode (HUM-443, K-29)* no descriptor crosses: the daemon sends an open frame with
the command and the size on the control stream, the shim opens the shell's pseudo-terminal itself
and forks the shell as above, and the shell's bytes, size, end and hang-up travel as frames of its
id. The limits are the same: the agent's filter on top, proven by refused `AF_UNIX` and
`AF_VSOCK` sockets, at most four, killed when the stream or the sandbox ends. A frame for an id
the daemon did not open changes nothing on either side.

The channels K-19 to K-23 exist only on macOS, where the sandbox is Seatbelt instead of bwrap
([ADR-0019](adr/0019-seatbelt-on-macos.md), [`SECURITY.md` section 12](SECURITY.md#12-macos-seatbelt-instead-of-bubblewrap)).

### K-19 The shared loopback of macOS

There is no network namespace on macOS, so the shim's door listens on a loopback port of the
host, chosen free per session. Attacker (b) would try to reach other loopback services of the
host from inside; the profile allows outbound loopback only to the session's own ports, and
`tests/escape/macos/run.sh` checks that another port is refused. In the other direction any
local process of any account can connect to the door and reaches the proxy as if it were the
agent. *Residual risk:* such a process sends with the session's rules: what an `allow` rule or the
passthrough lets through goes out without a hold, the rest lands in the owner's queue, and the
human sees it as the agent's. HUM-331 restricts the proxy to the session's shim.

The profile also allows a second random port of the session, on which an agent may run its own
local server (HUM-365): OpenCode 2 talks to one, and a fixed port would let the sandbox reach a
service of the host. Seatbelt can name only `localhost`, which covers both addresses, so a session
port is one that is free on `127.0.0.1` and `[::1]` and never OpenCode's fixed service ports.
*Residual risk:* any local process reaches that server too, so the adapter guards it with a
password per session. It protects against other local processes, not against the agent: the
value is in the agent's environment and can end up in a recording. A host process that binds that port on the other address while the session
runs becomes reachable from inside; it would have to pick that random port during the session.

### K-20 No PID namespace on macOS

The agent runs in a session of its own (the shim calls `setsid` for it). At the end of the
command, and three seconds after a stop, the shim kills the processes of that session, in passes
that repeat until one finds nobody, at most ten. A process that calls `setsid` itself leaves that
session and outlives the sandbox, and so could one that keeps forking faster than the passes. It
keeps the profile, and the profile allows it the daemon's proxy socket directly, not only the
shim's door. *Residual risk:* such a process can keep sending requests for as long as the daemon
runs, through launchd and across logins; `allow` rules and the passthrough apply, everything else
is held in the queue after the session ended. It can also keep changing the project directory,
which the summary of the session does not see; in a disposable run the project is out of its
reach, and the clone only through a descriptor it already held (K-24). HUM-331 restricts the proxy socket to the shim of
the running session; until then a restart of the daemon ends such a process's way out.

### K-21 Reading by deny list, paths instead of descriptors on macOS

On Linux the sandbox sees only the mounted trees. On macOS the profile allows reading everything
outside `/Users`, `/Volumes`, `/private/var/folders`, `/private/tmp` and `/private/var/tmp`, so
attacker (b) reads all of `/etc` (the real `passwd` and `hosts`), `/Library`, `/Applications`, `/opt/homebrew` and
`/usr/local/etc`. *Residual risk:* a credential in a system-wide configuration file reaches the
agent and, through the passthrough, the model host without a hold. Keep secrets in the home
directory, which stays unreadable.

The profile names paths. The project directory is resolved and checked against the policy before
the start, but not pinned by descriptor (HUM-263 on Linux). Seatbelt matches resolved paths, so a
symbolic link swapped in leads nowhere; only a local process of the same account, outside the
attackers of this model, could swap in another real directory under the same name between the
check and the start. Attacker (b)
cannot do this from inside: the profile lets the agent write only inside the resolved project, its
session home and tmp.

### K-22 Seatbelt is deprecated

Apple marks `sandbox-exec` as deprecated and documents the profile language only by example; it
the same kernel mechanism carries the sandboxes of Chrome, Codex and Claude Code, and the escape
suite passed on macOS 15 (CI) and macOS 27 (development). A future macOS
could change its behaviour without notice. The macOS escape suite runs on every commit in the CI
job `macos` and against the bundled programs in every release, on the macOS version of GitHub's
`macos-15` runners. A change in that version shows up as a red run before a release. On any other
version the shim measures the guarantees at every start, and a red line stops the session
(K-15). *Residual risk:* a regression the shim's probes do not cover reaches users of an
untested version unnoticed.

### K-23 Host-local side channels on macOS

The profile allows POSIX semaphores and shared memory and the Mach services for user data,
notifications, the system log and preferences, because ordinary tools need them. Each is a channel
to processes on the same machine, not to the network. Attacker (b) could hand data to a
cooperating host process through them; that process would then need its own way out, which the
sandbox does not give it. *Residual risk:* entries in the system log are readable by the user's
other processes.

### K-24 The disposable sandbox

On macOS (Seatbelt) there is no overlay; the layer is an APFS clone of the project (HUM-374,
below). The way back is the same code.

*Who is covered (HUM-357).* The disposable sandbox is the default where the system allows it:
`sandbox.mode = "auto"`, the default, resolves to disposable with bubblewrap 0.11 or newer (not
setuid), Linux 5.11 or newer, a runtime directory on tmpfs with a private layer directory and a
writable project; a setup that still fails before bubblewrap starts falls back too, never one
after. Where one is missing, `auto` runs direct and
the start warns with `SANDBOX_033`; such a run has none of the protections of this section, and
the agent writes to the project folder directly, as before HUM-350. An explicit `disposable` never
falls back; it refuses with `SANDBOX_030`. A project profile cannot set `sandbox.mode`
(CONVENTIONS 4.11), so (b) cannot move a run from disposable to direct through a cloned
repository; what decides is the system the human runs on and the human's own choice. Pending
changes of an earlier run refuse every start except an explicit `direct`, checked before the
mode is resolved, instead of letting a default fall back to direct over a layer nobody
reviewed. A run without changes discards its layer at once (`no_changes`), but only after a complete
scan; a directory that cannot be read marks the list incomplete, and the layer waits.

*macOS (HUM-374).* In direct runs a mask behind a symbolic link is denied on its resolved target
too, with the folders above it locked. The daemon clones the opened project to a sibling of the
session directories, removes every present mask from the clone before the start and denies every
configured mask in the profile, present or not; the scan finds a mask by looking its name up in
the clone and comparing device and inode, so a spelling that APFS folds to a mask (Unicode case
folding, normalization) is refused on apply like the mask itself (HUM-491); the root of the clone
is locked and opened only by descriptor against the device and inode taken after cloning; the real
project is denied for reading and writing, also outside the private trees, and `mounts.extra_rw`
is read-only. `auto` resolves to disposable there too, and a clone that fails runs direct with
`SANDBOX_033`, without the protections of this section; an explicit `disposable` refuses with
`SANDBOX_030`. Before anything reads the clone the daemon renames it, which ends path access for a
process that outlived the sandbox (K-20). *Residual risk:* a descriptor such a process opened for
writing before the end stays usable, so it can change the bytes of that one file after the human
reviewed it; there is no size cap, so the agent can fill the volume as in direct mode; the clone
lies on disk until apply, discard or the next daemon start; a file whose kind, size, `mtime` and
mode the agent restored is not listed, and is then not applied either; a volume that folds
differently and is mounted inside the project is not seen by the clone, so for a mask present
there only the ASCII comparison applies, and for a missing one nothing (HUM-491). Measured with a
real `sandbox-exec` run, see
[`SECURITY.md` section 12](SECURITY.md#the-disposable-sandbox-on-macos-hum-374).

With a disposable start (HUM-350) the project folder is the read-only lower layer of an
overlay at `/work`, and the agent's changes go to an upper layer the daemon created under
`$XDG_RUNTIME_DIR/humanitl/disposable/<id>/` (mode `0700`, tmpfs). After the run a human reviews
the diff and applies all, some or none of it. The upper layer is written by attacker (b) and
read by the daemon; applying it is a write path from the sandbox back to the host, so (b) tries
four things. (1) *Write to the project or the host during the run.* The project is the lower
layer and never written; `mounts.extra_rw` is mounted read-only in this mode; the path of the
upper layer is not visible in the sandbox (`/run` never comes from the host, and bubblewrap opens
the overlay directories by descriptor). (2) *Keep writing while the daemon reads.* The daemon
reads and applies only after bubblewrap is reaped and the sandbox's first process is gone; with
PID 1 the kernel ends the whole PID namespace, and the overlay's mount namespace ends with its
last process. (3) *Fill the host's memory.* The tmpfs of the runtime directory is the hard
limit; the daemon measures that file system's growth every 250 ms and stops the sandbox above
2 GiB or 200 000 entries (or half of what was free), `SANDBOX_032`. Between two measurements
the agent can write past the cap up to the hard limit. (4) *Make the apply write somewhere
else or something else.* Every access to the layer and to the project goes through
`openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)`; a symlink the agent planted in the layer is
never followed, a symlink whose target leaves the project is never written, and a symlink that
appeared in the project meanwhile stops the write (`ELOOP`). A symlink target must be a plain
relative path (leading `..` only, no more than the link's own depth), so a chain such as
`sub/d -> ..` and `e -> sub/d/..` cannot lead above the project. Files are created with
`O_CREAT | O_EXCL | O_NOFOLLOW` under a name of their own and moved into place with `renameat`,
which replaces a symlink at the target instead of following it. Devices, FIFOs and sockets are
never written, setuid, setgid and sticky bits are dropped, hard links become separate files,
whiteouts and opaque directories only delete inside the project, and entries with overlay
attributes an unprivileged overlay never writes (`redirect`, `metacopy`) are refused. The project folder is opened once
before the start; the baseline is taken on that descriptor before the sandbox starts, the
launcher mounts only a folder with the same inode, and diff and apply never reopen the path. A file the human changed since the start is a conflict and is written only
when it was confirmed for exactly that change (`overwrite_ids`, or `--overwrite` for all selected,
HUM-351), and the target is compared again directly before each write (a remaining window of
a few system calls, no lock on the folder); every selected change is
checked before the first write, and one failing check writes nothing. Paths and diffs are agent
data and are shown sanitised. *Residual risk:* what the human applies is the agent's content, as
in a direct session; review the diff, in particular files the host runs by itself (hooks,
`Makefile`, `package.json` scripts, which the summary's finding `SANDBOX_026` names for direct
sessions only so far). Requests the agent sent through the proxy are out, whatever happens to the
layer. The conflict check compares type, size, `mtime` and mode, not content: a human edit that
keeps all four is not noticed. A daemon that ends discards a pending layer; its changes are lost
then, and the recording says so.

*Podman (HUM-438).* The engine's overlay volume with its own upper layer is a second form of the
upper layer, measured and not used. It takes the project as a path the engine resolves later; a
descriptor is no mount source (HUM-292 P13, and P10 with `/proc/self/fd/N` as the lower layer),
and inside the container the overlay cannot be tied to the checked folder by device and inode.
An attacker of kind (b), a process of the same user, or anything else that can rename the
folder could swap it between the check and the mount
and make an unchecked folder the lower layer. Under Podman `disposable` therefore refuses with
`SANDBOX_030` and `auto` runs direct with `SANDBOX_033`; such a run has none of the protections of
this section. ESC-5 checks the refusal under Podman.

### K-25 Updates from inside the app

The daemon fetches the latest release from GitHub once a day and installs it on a click (HUM-382).
An attacker who controls the network path, GitHub's storage or the GitHub account could try to
ship code that runs as the user with the daemon's rights, outside every sandbox. The binding is
the Ed25519 signature over the update manifest, checked against a public key compiled into the
daemon before any field is read; the manifest binds version, URL prefix, size and SHA-256 of the
file, and the daemon refuses an older or equal version, a URL outside the release downloads, a
redirect off GitHub and any byte that does not match. The agent cannot reach this path: it runs
over its own connection stack, never through the proxy. Packages from APT and dnf are upgraded by
their package manager through `pkexec`, with the repository's signatures. *Residual risk:* a stolen
release key ships malware to every installation that clicks; the check tells GitHub that this
address runs Humanitl and which version. Turned off with `update.check = false`.

### K-26 Pausing intercept

A person can pause intercepting (HUM-407). While the pause lasts, requests that would wait for a
person leave without a hold, with the decision source `paused` (a `redact` match un-pseudonymised
until HUM-079; without a hold deadline nothing changes); `block` rules, the hard block on verified
secrets, the sandbox guarantees and the recording stay. An agent (attackers a to c) that learned of
a pause could time its exfiltration into it, and one that could set the pause would have removed
the human from the loop. The binding: only the RPC `Intercept` on the daemon socket sets it; the
proxy and `humanitl.internal` hold no handle to it, and no answer text the agent reads names it
(HUM-364, HUM-401). The agent cannot set it; it can infer it from the missing wait. The
pause lives in memory only and ends on resume, with every new session and with the daemon; start
and end are audit records, and the app shows a running caution tape in every section while it
lasts. *Residual risk:* what the agent sends during a pause leaves unseen by a person; the
recording and the audit log show it afterwards, each request marked `paused`. An agent can infer
from the missing wait, and from requests that now succeed, that holds stopped, and time what it
sends accordingly.

### K-27 The container engine

Under the Podman backend (HUM-433, [SECURITY.md section 13](SECURITY.md#13-linux-podman)) the
sandbox is a rootless container, and the engine that builds it joins the trusted computing base:
Podman, `conmon`, which stays as the container's parent, the OCI runtime `crun`, the user's
container storage under `~/.local/share/containers` and the configuration Podman reads. An agent
(attacker b) that found a weakness in any of them would leave the sandbox as it would through a
weakness of bubblewrap; a process of the same user (attacker d) could change the user's own
`containers.conf`, `mounts.conf` or hooks to add a mount, a device, a network or a hook to every
container. The configuration sources of Podman are `containers.conf` in every layer
(`/usr/share/containers`, `/etc/containers`, `~/.config/containers`, `CONTAINERS_CONF_OVERRIDE`),
`mounts.conf` in the same three places and through `--default-mounts-file`, the OCI hook
directories, `storage.conf`, `registries.conf`, `policy.json` and the remote connections
(`CONTAINER_HOST`, `CONTAINER_CONNECTION`). The binding: the daemon starts Podman with its own `containers.conf`,
`storage.conf` and `registries.conf` and an empty hook directory, and with an empty environment
except `HOME`, the user's `XDG_RUNTIME_DIR`, `PATH`, the user's session bus and those three paths; the line carries every flag that
the three guarantees rest on, with no key to change it; the agent's environment goes only through
the sealed memfd and never into the stored container configuration; the shim's descriptors (report,
lifeline, environment, mount table, shell door, terminal) reach it through a socket of the session
that accepts exactly one connection of the daemon's uid naming a one-time token and is closed
before the agent starts, so the engine's client never holds them (HUM-434); under the daemon's
hardened unit every Podman call runs as a transient unit `humanitl-*` of the user's service
manager, and stopping a session also stops its unit; the container ends through the
engine by name and, when the daemon dies, through the lifeline; whatever a configuration source
still adds, the shim finds in the mount table before the agent starts and refuses with
`SANDBOX_049`, whether it carries a socket or not (HUM-437). *Residual risk:* the plain level
shares the host kernel as bubblewrap does; mounts go by path, so a swapped path is detected and
refused by the shim's check of device and inode after the mount (`SANDBOX_042`, HUM-437), not
prevented; the engine's own files `/run/.containerenv` and `/etc/hostname` are matched by mount
point, type, mode and flags, not by their backing file (the masks under `/proc` and the device
nodes are pinned to the host's `/dev/null` and devices by device and inode); Podman
reads `/etc/containers/policy.json` and the storage it shares with the user. HUM-448 admitted the level after the CI job below was green on `main`; a person starts it only by choosing it, and `auto` stays bubblewrap; the kernel level is a backend of its own (K-30 to K-34), admitted with HUM-449. Checked by ESC-1, ESC-2 and ESC-5 in the CI job `escape-tests-podman` (HUM-435): the
same probes as under bubblewrap, plus an empty `/run/.containerenv`, no socket of `conmon` or the
engine, the shim as PID 1 with the filter, and a `mounts.conf` entry, a volume and a hook planted
in the runner's own engine configuration that reach a plain container and not the sandbox.

### K-28 The image as the root of the sandbox

Under Podman with `rootfs = "image"` (HUM-439, [SECURITY.md section 13](SECURITY.md#an-image-as-the-root-hum-439))
the root of the sandbox is an OCI image, and three things come with it. **Origin:** the image is
code someone else built, like a dependency (attacker b), and it runs as the agent; whoever controls
its registry or its name controls what a later pull brings. **Digest:** a name can point at another
image tomorrow; a start that took the name would run something nobody chose. **The network while
pulling:** the engine fetches the layers itself, over the daemon's network, outside the proxy, its
rules and its recording. The binding: an image is only ever named with its digest
(`name@sha256:...`, `CONFIG_003` otherwise); before every start the plan asks the engine for
exactly that digest and refuses a missing image or another digest under the same name with
`SANDBOX_043`; no start pulls (`--pull=never`); only `humanitl sandbox image pull`, on the request
of a person, pulls, and the command line and the interface say before it runs that it uses the
network directly; a name without a registry is refused, and the daemon's own `registries.conf`
holds no search list and no mirror; project folder, door, CA certificate and bundle, shim, the
adapter's files and the session directory come from the host, never from the image, so a CA, a key
or a session directory never stands in an image; the line sets user, entry point,
`--image-volume=ignore` and `--no-healthcheck` itself, and an image whose environment sets a
`HUMANITL_` name, which would win over the shim's variables in the engine, is refused. Registry
credentials for a pull come only from `<config>/humanitl/podman/auth.json`: the daemon opens it
without following links, checks owner and mode on the open file, refuses `credHelpers` and
`credsStore`, and hands the engine a private copy under `--authfile`, removed after the pull; a
registry drop-in that sets `credential-helpers` refuses the pull (HUM-456). *Residual
risk:* the signature policy accepts any image, the digest is the only anchor; the drop-in check runs
before the engine reads the drop-ins, so a process of the same account could add one in between,
but such a process can read the keyring and the credential files directly, and the check guards
against a helper set up by accident; the digest check
reads the engine's metadata and does not hash the unpacked layers again, so a process of the same
account (attacker d) that writes into the user's container storage can change the content under an
unchanged digest; the image's annotations reach the engine; the pull itself is not recorded by
the proxy, only by the daemon's log. Checked by ESC-2 `ca_is_the_hosts` (the CA in the sandbox is
the daemon's file by device and inode), which runs under every backend and, with
`HUMANITL_ESCAPE_IMAGE`, under an image; by the unit and live tests in
`daemon/crates/sandbox/src/podman/` (another digest under the same name refused, nothing pulled).

### K-29 The control stream of the terminals

In a micro-VM no descriptor crosses the boundary: the daemon cannot hand the agent a
pseudo-terminal, `SCM_RIGHTS` cannot carry the follower of a further shell (K-18), and no signal of
the host reaches a process of the guest. In the stream mode (HUM-443) the shim opens every
pseudo-terminal itself, and terminal bytes, sizes, further shells, exits, interrupts and
heartbeats travel as frames over one stream socket (`daemon/bin/humanitl-shim/src/frame.rs`, the
one file both ends compile). The kernel level always uses it; under bwrap a caller can ask for it,
so that both modes face the same terminal tests. It is a second channel between sandbox and host
next to the door, so attacker (b) would try four things. (1) *Use it as a way out.* It carries
terminal bytes and control and nothing else; the bridge is the only way to the proxy, and the
daemon never forwards a byte of the stream to the proxy or from it. Inside, only PID 1 holds the
socket and the masters of the pseudo-terminals: the agent's child closes them before its filter,
every further shell closes every descriptor above 2, all of them carry `CLOEXEC`, and PID 1 is not
dumpable. In a guest the shim listens on exactly one vsock port and takes exactly two connections
from the host, the report first and this stream second, then closes the listener; the guest never
opens a connection for either. (2) *Open a vsock or Unix socket of its own.* The agent's filter
refuses `AF_VSOCK` and `AF_UNIX` with `EPERM`, and so does the filter every further shell installs
before `exec`, proven before the shell starts; ESC-1 checks it in the stream mode. (3) *Feed the
daemon hostile frames* (root in a guest can write anything to the stream). Every frame has a
nine-byte header with a length of at most 16 KiB; a longer frame is counted away without being
kept, a frame of an unknown kind is skipped the same way, a frame of the wrong shape or of the
other direction is dropped, and an answer for an id nobody opened changes nothing. The reader holds
at most one frame. The fuzz target `control_stream_reader` runs this reader nightly, and its
regression test fails when the length check is removed. (4) *Outlive the daemon.* The end of the
stream is a lifeline: PID 1 sends `SIGTERM` to the agent, kills every further shell and, after the
grace period, every process. PID 1 never writes to a terminal itself and sends the signal before
any message, so a full terminal cannot hold it up; after the end it reads every terminal empty and
keeps nothing. In a guest this is defence in depth; the host stops the VM itself (K-33, HUM-444).
*Residual risk:* root in a guest can end or stall the stream. A guest that stops reading makes
the daemon's next frame miss its deadline of ten seconds, and the daemon then shuts the stream:
the session ends, no daemon thread waits longer. A daemon that stops reading stops the shim's
reading of the terminals, not of the control frames. Input for a terminal travels under credit
of 64 KiB (HUM-497): a write to a terminal whose process does not read waits and fails with
`TERM_002` after ten seconds without credit, the stream stays open, and no byte is dropped while
the writer is told it arrived, except in the moment in which the terminal's process ends; root in a guest that withholds credit only stalls input to its
own sandbox, and a credit beyond the bytes sent and not yet granted back adds nothing. This
protects against an agent that does not read, not against a hostile guest: delivery to the agent
depends on the guest's PID 1, and root in a guest owns the terminal and can lose or alter its
input anyway, which harms only its own sandbox. Output of a further
shell whose reader stalls is dropped beyond 256 pieces on the daemon's side, counted; neither
holds up another channel.

### K-30 The virtual machine monitor of the kernel level

The kernel level (ADR-0021, HUM-444; admitted since HUM-449) puts the agent in a QEMU micro-VM
with a kernel of its own. The emulator is the code that stands between a guest that attacker (b)
may own completely and the host. It gets no network device (`-nic none`, no `-netdev`), no
console, monitor, display or QMP, and so no channel a guest could talk through except the three
vhost-user sockets of its devices. It runs under its full `-sandbox` (no obsolete system calls, no
privilege change, no `fork`/`exec`, no resource control), in a bubblewrap jail without capabilities
that holds only `/usr`, `/dev/kvm`, the one kernel file and, read-only, the three directories of
its vhost-user sockets (never the door, the handover socket or the guest's files), and in a
transient unit of the user with
`PrivateNetwork`, `PrivateUsers`, `NoNewPrivileges`, no capability and a read-only file system.
After the start the daemon checks it from the host: its descriptors may only be KVM, event and
signal descriptors, `io_uring`, its memory, `/dev/null`, pipes and sockets; its own command line
may hold no network option and no console; a finding is `SANDBOX_047` and stops the session.
When the guest has already refused on its own, its diagnosis is the one shown and the host's
findings stand in the evidence; the session is refused either way (HUM-470).
The agent in the guest waits at its exec gate for the daemon's `ADMIT`, sent only after the
guest's lines and this host check passed, so a finding stops the session before the agent ever
runs.
*Residual risk:* a flaw in KVM or in the emulator's device code that lets a guest execute code in
the emulator; the jail and the unit then limit what that code reaches, and it still has no network.

### K-31 The vsock door of the kernel level

The only way from the guest to the host is vsock: one device per session, served by its own
`vhost-device-vsock` process with its own group (two guests in one process could reach each other,
HUM-442 Q9). A connection of the guest to CID 2 and port `P` becomes a connection to the Unix
socket `<uds>_P` in the session's runtime directory. The daemon binds exactly one such socket,
`<uds>_5000`, and forwards every connection to the session's proxy socket; for any other port
there is no listener, and the guest's connection times out (Q2). `--forward-cid` is never set. The
host side opens two connections into the guest itself (`CONNECT` on the shim's report port): the
report and the control stream (K-29). It never gives up a pending `CONNECT` to ask again, so the
guest, which takes exactly two connections in their order, cannot take an abandoned one for the
report (HUM-470). It asks only after the shim has said on the door, with one fixed line
on a connection of its own, that it listens (HUM-471): a `CONNECT` that reaches
`vhost-device-vsock` 0.3.0 while the guest's driver is still setting the device up stops the
device for good, and only a connection the guest makes itself proves that the device runs. Until
that line the daemon hands no connection of the door to the proxy; the agent cannot run before it
anyway, since it waits for the admission on the report connection. Each forwarded connection
binds two threads and four descriptors of the daemon, so the door keeps at most 64 of them open per
session (HUM-467): a connection above that is closed at once, before anything is read and without a
thread, and never reaches the proxy. The first one per session is noted and stands in the evidence
of `single_socket` in every later check of the session and, once per sandbox launch, in the audit
log as `sandbox.door_capped` (HUM-477; a daemon that launches the sandbox again writes one more
under the same audit session); later ones are not noted. A guest with root
can therefore bind at most 128 threads and 256 descriptors of the daemon through the door. The host
check refuses any socket in the runtime directory besides the declared ones, and more than one
vsock device process or device. *Residual risk:* a
flaw in the vsock device process; it runs jailed and without network like the other helpers. The
cap is per session and there is no shared one (HUM-479): the daemon runs exactly one session
(`CLI_005`), so no guest can lock another session's door, and one full door with its proxy
connections holds about 384 descriptors, well below the soft limit of 1024
descriptors that systemd gives the daemon's user service. Running
several sessions at once (HUM-549) must decide a shared budget first.

### K-32 The guest kernel

The agent in the guest is no root: before its filter it gives up every capability, the bounding
set included, and runs as the user that bwrap maps into its sandbox, so it cannot write into PID 1
(`/proc/1/mem`) or load a module (HUM-445). The guest's `/sys` is an empty read-only tmpfs, as under
bubblewrap, which mounts no sysfs. Still, a kernel bug can make the agent root, and root controls
the guest kernel. Nothing of the three guarantees rests on it: no network device exists, the door is decided on the host, and the
emulator cannot gain a device after the start. Before the agent runs the shim, as PID 1, empties
`kernel.modprobe` (no module loads on demand), loads exactly the plan's vsock modules and locks
module loading (`kernel.modules_disabled`); the guest root is a read-only share that holds neither
the runtime directory nor the jail's `/proc`. The shim's `CHECK` lines from the guest are defense in
depth, and a guest that ends without them is a failed check. The escape suite measures the guest
as root without a seccomp filter (`tests/escape/guest-root.sh`, HUM-445): only `lo`, no route out,
the host's door the only vsock target, no second guest reachable, no module. *Residual risk:* root
in the guest can forge the guest's lines; the host check is what holds then.

### K-33 The helper processes of a session

A disposable session has one helper more, the file system daemon of the layer (K-34), in a jail of
the same kind; it alone gets `CAP_DAC_OVERRIDE`, inside its own user namespace, over the layer and
a read-only `/usr`, because the overlay in the guest works as root.

A session of the kernel level is one transient unit of the user with five processes of its own:
the unit's first process (the shim with `--fds-then-exec`, then a fixed `sh` script), the file
system daemon of the guest root, the file system daemon of `/work`, the vsock device and the
emulator. Each helper runs in its own bubblewrap jail with every namespace unshared and every
capability dropped, sees only `/usr` of the host, what it serves and its own socket directory in
the runtime directory (no jail binds the runtime directory itself), and the unit itself has no
network, no capability and a read-only file system except the session's runtime directory and the
project folder. The unit is no child of the daemon and inherits nothing from it but three
descriptors over a handover socket that accepts one connection with a one-time token: the root
daemon's listening socket, the checked project folder and the read end of the lifeline. When the
daemon ends, by `SIGKILL` too, the script sees the end of the lifeline and ends, and the service
manager ends every process of the unit within seconds, without the guest; a daemon that runs as a
service also binds the unit to its own (`BindsTo`). A file system daemon keeps one descriptor per
inode the guest has looked up and one more per open file, so a guest decides how many it holds: a
walk over the guest root drove the root daemon to about 520,000 until it ran out and the guest saw
`ENFILE` (found in HUM-445), and root in the guest could pin host kernel memory that way. That
finding is closed (HUM-469): the unit carries `LimitNOFILE=65536`, soft and hard, for every
process, which no jail can raise; past it the daemon answers the guest
with `ENFILE` and keeps serving. The guest's file cache is on (`--cache auto`, HUM-476) and PID 1
in the guest trims it: every 100 ms, above 32,768 unused dentries and inodes, it makes the guest kernel drop
them (`vm.drop_caches=2`), which makes the daemons close their descriptors, so a walk over the
whole root ends without `ENFILE` (measured: at most 37,566 descriptors). Since HUM-483 PID 1 also
trims when the guest holds more than 49,152 inodes in all and 8,192 more than after the last
trim, at 57,344 at the latest (a trim there that frees little is repeated at most once a second, unless the guest holds more than it left: 8,192 more below 57,344, 512 more above), because the kernel no longer counts a directory as unused once a trim met it in use: a
chain held during a trim and let go afterwards would otherwise stay (measured: without that
trigger `ENFILE` after 64,924 directories, with it at most 45,814 descriptors). The trim is a
trigger, not a cap: what the guest holds in use (open files, working directories and the
directories above them) stays, so a guest can still reach the limit on purpose; PID 1 opens the files for that before the agent exists and holds
them alone, and the bound is the guest's comfort, not the guarantee, which is the limit. The KVM
test
`a_guest_cannot_drive_the_root_file_daemon_over_its_descriptor_limit` holds the root open as root
in the guest and counts the daemon's descriptors from the host. Memory and processes are bounded
for the unit as a whole (HUM-485): `MemoryMax` is the machine's memory plus a fixed 512 MiB for the
helpers plus the size of every tmpfs the unit writes to (the project folder when the user put it
on one, which makes the bound that tmpfs's size rather than a fixed amount; always the layer of a
disposable session, which lives in RAM; 64 MiB for each mask over the project, a tmpfs of that
fixed size in the `/work` jail, `ENOSPC` beyond it), `MemorySwapMax` as much again, and
`TasksMax` the vCPUs plus 128. Measured on real sessions: the guest's memory is a memfd and counts
up to the machine's size, the helpers themselves at most 59 MiB of anonymous and 96 MiB of kernel
memory, page cache of what the daemons read is reclaimed at the limit instead of killing anything,
and 28 to 34 tasks besides the vCPUs. A compromised helper that allocates or forks without end
stops at the limit (`EAGAIN`, or the memory controller reclaims and finally kills inside the
unit); the host keeps the rest. Without the controllers `memory` and `pids` systemd would drop the
limits silently, so the start is refused with `SANDBOX_052` before the unit exists, and because a
slice in between could still withhold them, the host check compares `memory.max`,
`memory.swap.max` and `pids.max` of the running unit with the plan and refuses the session with
`SANDBOX_052` before the agent starts. The KVM test
`a_process_in_the_unit_cannot_start_more_tasks_than_tasks_max` lets a process in the unit fork
until it fails. Only the unit's programs can run (HUM-480): the unit carries `NoExecPaths=/` and
`ExecPaths=` with exactly the files its processes start or map as code (`env`, the shim, the
script's shell, `sleep`, bubblewrap, the file system daemons, the vsock device, the emulator, and
for each its loader and the libraries the loader lists), single files rather than the library
directory, which holds many other programs; every writable path of the unit is named in
`NoExecPaths=` too, since `ReadWritePaths=` mounts it anew beyond the reach of `NoExecPaths=/`.
Every other file reachable through the unit's mounts, a copy the unit wrote into the project
folder or its runtime directory included, fails at `execve` (`EACCES`) and at a mapping as code
(`EPERM`), the loader as a detour included; the bubblewrap
jails inherit this locked, and it holds after admission, which the one-time host check could not
see. The host check also counts the processes per program: as many shells, bubblewraps and file
system daemons as the script starts, so a shell or a second bubblewrap in place of a helper is a
finding (`SANDBOX_047`). The KVM test `a_helper_that_starts_a_foreign_program_fails_at_exec` starts
`/usr/bin/id` and copies of it in writable places in the emulator's jail, beside it in the unit
and, after admission, once more from a process waiting in the jail. *Residual risk:* a compromised
daemon can start such units (as under Podman, see the trust boundaries); a flaw in a file system
daemon reaches what it serves, the read-only guest root or the project folder; root in the guest
can exhaust a daemon's descriptors for its own session, never past the limit. The program list
binds files of the host; code a taken-over process brings along is closed off in each jail
(HUM-557): every other program of the unit under `/usr` is hidden behind `/dev/null`, so the
shell and bubblewrap of the list can no longer be used together; no further user namespace
(`--disable-userns`), so nothing in a jail mounts a tmpfs without `noexec`, and a nested
bubblewrap fails with `ENOSPC` (measured with bubblewrap made reachable: without the option it ran
a copy of a program from its own tmpfs); the mounts bubblewrap creates, its root, `/dev` and the
empty tmpfs of the guest root, are remounted read-only, because bubblewrap 0.13 has no `noexec`
for them; the masks over the project are tmpfs mounts of the unit with `noexec`
(`TemporaryFileSystem=`); a seccomp filter on each helper refuses `execveat` and, for every helper
but the emulator, `memfd_create` without `MFD_NOEXEC_SEAL`, so no executable `memfd` exists
(`vm.memfd_noexec` is 0 by default and only a privileged process may raise it). The emulator needs
its `memfd` for the guest's memory; `execve` of it stays closed by its own `-sandbox spawn=deny`,
`execveat` by the filter. The KVM test `a_helper_reaches_no_code_of_its_own_choosing` tries each
way from the emulator's and the vsock device's jail with a process that runs code of its own
(Python, admitted only in the test), also after admission; each fails. What stays open: a process
that already runs code of its own choice can still run it in its own memory (`mprotect`), which
starts no program and which no list of files can see; and the file system daemon of the guest root
sees the unit's programs under `/guest/usr`, the guest's `/usr`, which it serves and cannot hide;
there it can start them, but no program of the list gives it more: bubblewrap creates no
namespace, and no writable mount in its jail lets anything run.

### K-34 The shared file systems of the kernel level

The guest reaches files of the host only through virtiofs: the read-only guest root, the project
folder at `/work` and, in a disposable session, the session's layer. Each share is a folder the
daemon opened and checked before the start; it reaches its file system daemon as that descriptor
(`--bind-fd` in the daemon's jail), never as a path, so a folder swapped after the check does not
reach the guest (HUM-442 Q4). Two checks follow, one on each side. The shim compares the inode of
`/work` (virtiofs keeps the host's inodes, not its device numbers, Q5) with the plan, and in a
disposable session the inode of the overlay's lower layer; a mismatch is `SANDBOX_042`. Root in the
guest could forge that line, so the host check also looks at the daemon that shares `/work`, and at
the one that shares the layer: exactly one of each, holding a descriptor that names its share and
carries the device and inode of the opened folder; a daemon that holds the checked folder only
somewhere below its share does not pass (`SANDBOX_047`). Measured (HUM-446): with a path instead of
`--bind-fd`, a folder swapped after the plan is refused with `SANDBOX_042`, and a plan that expects
another inode turns the host check red.

*A disposable session.* The project's daemon runs with `--readonly`, its jail binds the checked
folder read-only (`--ro-bind-fd`), and the unit gets only the layer as a writable path, so neither the guest nor a compromised daemon writes the
project. The shim loads `overlay` before the module lock and builds `/work` as an overlay of the
project and the layer with the options of bubblewrap's unprivileged overlay. Root in the guest can
mount the layer again and write anything into it: whiteouts, extended attributes, modes, hard
links, symbolic links; device nodes other than the whiteout `0:0` the layer's daemon cannot create
without `CAP_MKNOD`. The daemon therefore reads the layer only after every process of the unit has
ended (`ensure_ended` asks the service manager), with the scan of bubblewrap's disposable sandbox
(K-24), which never applies a device, FIFO, socket, setuid bit, foreign overlay attribute or
symbolic link out of the project. The profile's masks lie in the lower layer (in the jail of the
project's daemon) and, since HUM-481, on top of the overlay as under bubblewrap: the shim mounts
an empty tmpfs over every masked folder and an empty read-only file over every masked file that
exists, opened beneath `/work` without following a symbolic link, and the expected mount table
names each. A write to a masked file fails in the guest, and what the agent writes into a masked
folder stays in the mask's tmpfs. Root in the guest could still remove a mask or write the layer
past it; the scan refuses every change on, under or above a mask, so neither a hook nor a secret
reaches the project through the layer, and nobody in the guest sees what a mask hides, because
the mask in the lower layer lies in the jail of the project's daemon. The layer's daemon takes
only the overlay's own attributes, `user.overlay.*` (`--xattrmap`); every other name fails with
`ENOTSUP` and is hidden from listings, so no attribute of the guest's choosing lands on the host
(measured with Linux 7.3 in the guest: the overlay writes `user.overlay.opaque`,
`user.overlay.impure` and `user.overlay.uuid`, nothing else). The comparison with the masks is
exact, and the guest cannot see whether the host folds case: its virtiofs passes each name to the
daemon on the host, whose `openat` folds in a casefolded folder, so `.GIT/hooks` in the layer would
be `.git/hooks` in the project. A project whose root or a folder on the way to a mask folds case
(the casefold attribute, vfat, exFAT, HFS+, SMB, or an existing mask entry that resolves under the
other ASCII case) is therefore refused before the start (`SANDBOX_030`, HUM-482), and again before
the layer is read and right before an apply writes (`SANDBOX_031`); under bubblewrap
overlayfs refuses a casefolded folder itself (observed on Linux 7.3), which Humanitl does not rely
on. Root in the guest can also rename or replace `upper` inside the layer directory; the daemon
records its device and inode when it creates the layer and refuses a swapped or missing `upper`
with `SANDBOX_031` (HUM-482). *Residual risk:* a file system that folds case without the attribute
and outside the list (FUSE) passes when the mask entry does not exist yet; a flaw in a file
system daemon reaches what it serves, the layer with the rights of `CAP_DAC_OVERRIDE` in its own
namespace included; a scan flaw would show attacker-written bytes to the human, who still decides
each apply.

### K-35 A decision proxy behind Humanitl

With `egress.proxy` a person puts an intercepting proxy of their own (mitmproxy, Burp, a company
tool) behind Humanitl (ADR-0022, HUM-511). Humanitl still decides first; what it allows leaves
through that proxy, re-encrypted with the proxy's CA, so the proxy sees the decrypted traffic,
including secrets Humanitl did not block. The trust boundary for content moves to that proxy; the
isolation does not: the connection to it starts on the host from the daemon, the sandbox has no
route to it, and a failing proxy never makes a request leave directly. Its CA (`egress.proxy_ca`)
is trusted only on that way. Who resolves the target's name is the person's choice:
`egress.proxy_resolve = "proxy"`, the default of the opt-in, keeps half one of ADR-0006 (no DNS
from Humanitl before the decision there) and refuses only private address literals and names that
are local by definition (`localhost`, `*.localhost`, `*.local`, `*.internal`, `*.home.arpa`, names
without a dot), which are refused before any resolution with either value; `"humanitl"` keeps half two (every address resolved and checked by Humanitl, sent
as `CONNECT IP:PORT`) only with a proxy that connects to that address, and the query leaves before the proxy decides. Burp Suite 2026.11.2 resolves the name from `Host` again and ignores the address (observed 2026-11-18, HUM-524); with Burp only `"proxy"` and its residual risk apply. Every key is closed to
project profiles, and the credentials live in the keyring, or in the `0600` fallback file when the keyring does not answer (`CONFIG_019`), never in `config.toml`. Requests the daemon makes on its own
(model search, update check) stay direct. *Residual risk:* with `"proxy"`, a public name whose DNS
answer points to `127.0.0.1`, the LAN or a metadata address reaches that address through the
proxy if the person there allows it; with `"humanitl"`, the name leaves as a DNS query before the
person at the proxy decides.

With `hold.ask_mode = "upstream"` (HUM-512) that proxy also decides what no rule decides: a
request without a rule or with `ask` is forwarded instead of held, with the decision source
`upstream`. The content trust moves further, from reading to deciding; the isolation stays.
Valid only with `egress.proxy` set (`CONFIG_003` at load) and only while the running daemon goes
through it (the session start refuses otherwise), so no such request leaves undecided. A `block`
rule stops a request before any resolution and before the proxy, with one exception in every ask
mode: the declared LLM passthrough is a bundled `allow` ranked ahead of the person's block rules,
so a request it matches reaches the proxy like any allowed request (recorded as `passthrough`);
`redact` keeps holding in Humanitl unless intercepting is paused; the hard block on verified secrets turns the forward into a block of the system, and the
proxy sees no byte. The audit records `decision: forward`, `decided_by: upstream` and the status of
the proxy's answer, never an allow by a person. *Residual risk:* the proxy's decision is only as
good as the person and the tool there; Humanitl cannot tell a refusal from an allow except by the
status, and a slow decision there holds the agent up to `egress.proxy_response_timeout_secs`.

### K-36 The agent depot

With the agent depot (ADR-0023, HUM-518) the agent comes from a store per agent under the daemon's
data directory instead of from the host; HUM-519 and HUM-563 add the maintenance runs with their
signature checks below, and HUM-520 completes this entry with login, configuration and the guarded
credential write-back (third part). **Origin:** the program is the
resolved version directory `program/<version>/` of the depot, never the `current` link, so an
update never changes a running session; the daemon reads the version from `manifest.json` and
never executes anything from the depot on the host. **Mounts:** that directory read-only at
`/run/humanitl/agent/`, every entry checked with the trust rules of `locate.rs` before each bind,
and a scratch copy of `config/` and the credential file, writable at `/run/humanitl/agent-config`,
under the session's runtime directory and never under the depot, deleted with the session or at the
next start after a crash; a compromised agent can change its configuration for the rest of its own
session only. **One guard** on the final plan request in every backend refuses a host program of a
depot agent (`AGENT_017`). **Masks:** the host install locations of a depot agent's recipe are
covered by empty read-only mounts under Linux, only where they exist on the host (a missing one is
skipped, since nothing is there and no mount point can be made under the read-only `/usr`), and
under Seatbelt by deny rules for `file-read*` and `process-exec` placed after the rule that allows
reads outside the private trees, because in SBPL the last matching rule wins. **Limit of the
lock:** the lock keeps Humanitl from starting or exposing a host agent. It is no defence against a
user who deliberately copies an agent binary to a path no recipe lists: on their own machine the
user controls that, and the company policy does not claim otherwise. *Residual risk:* other
processes of the same user can read and change the depot as they can any file under the data
directory; the kernel level does not carry the depot yet (`AGENT_015`); under Podman, which mounts by path, a
session bind or a profile extension (`mounts.extra_ro`, `mounts.extra_rw`) through a symbolic link
owned by a user (not root) is refused while the project directory and the profile's `mounts.ro` are checked on their resolved
places, and for every bind a part of the path could still be replaced between the check
and the engine's mount by someone who can write its parent.

**Maintenance runs (second part, HUM-519, HUM-563).** *Attacker:* a vendor host or a network path
that serves something else than the vendor's release, and the downloaded installer itself.
*Defence:* install and update run only in a sandbox of the daemon without a project: its own work
directory beside the depots as `HOME` at `/run/humanitl/agent-home`, no `/work`, the sandbox profile
without its extensions, no session bind, no depot mount, the agent's host installs masked; the guard
admits that directory as the work place only for a session the daemon marked, and only resolved to
itself (`SANDBOX_006`). Only the recipe's hosts pass without a hold, as `allow` rules of this
session that go when the run ends; the stored rules are never written, and for the run the bundled
group has no LLM passthrough. The run only downloads; the daemon reads no verdict from it, checks
the tree on its own copy, verifies a signing vendor's manifest with the host's `gpg` against the
fingerprint fixed in the adapter and the program's checksum against it, or, for a vendor that
publishes a list of checksums instead (Codex, HUM-566), its own copy of the chain from release
metadata over the list to the archive, and only then switches `current` with a journal; a run with a
status other than 0 or a red guarantee changes nothing. The version is pinned, sessions keep theirs,
and a session's read-only program directory makes a self-update fail. *Residual risk:* an unsigned
recipe's version is the run's word; behind a checksum list stands only TLS to the vendor's host, and
the unpacked tree is bound to the checked archive by the run, not by the daemon; the person's own
rules apply to the run as to any session; the kernel level and Seatbelt refuse a maintenance run for
now (`AGENT_015`).

**Login, configuration and the credential file (third part, HUM-520).** *Attacker:* a compromised
agent in any session of a depot agent, which can write its writable copy of the configuration at
will and wants to reach later sessions. *Assets:* the integrity of the depot's configuration and
login, and the person's account. Four ways, each with its defence:

- **Persistence.** Without a guard, anything a session writes would act in every later one. Only
  one file goes back, the credential file the adapter names, opened relative to the session's copy
  with `openat` and `O_NOFOLLOW` (`O_NONBLOCK` against a FIFO), a regular file of at most 64 KiB, a
  JSON object with only the keys of the adapter's format, each value of its type and form, every
  required key present; later write-backs also with exactly the stored file's keys. The daemon
  writes its own serialisation of the checked value, not the session's bytes, to a temporary file
  `0600` and puts it in place with `rename` under a lock per agent. Nothing else of the copy is
  read; settings, a second file, anything in the program's place are discarded with the session
  (`AGENT_012` for a refused file, the stored one unchanged). An adapter without a declared format
  gets nothing back. Configuration changes only through a configuration run whose changed files,
  with the keys that run commands highlighted, the person confirms; a login takes over single
  named keys of another file at most, read and checked by the daemon, never that file.
- **Account swap.** A session that logs in to another account (the attacker's) would make every
  later session send the person's work there. A session never stores the first credential file
  (`AGENT_012`), and where the format names the account, a write-back of another account is
  refused (`AGENT_016`); only a login run, which the person starts, sets or changes it. A login
  checks the credential file and the named keys before it writes either.
- **Covert channel through the credential file.** Within the format, the values are the
  session's: a compromised session can encode a few bytes in a token of the right form, or write
  one that does not work, and a later session reads it. Bounded by the format and 64 KiB, every
  write-back stands in the audit log as `agent.credential_writeback` with the SHA-256 of the old
  and new file, never their content; `agents.<id>.credential_writeback = false` closes it at the
  price of a new login whenever the token expires.
- **Parallel sessions.** Two sessions renewing the same login must not set the depot back. Where
  the format carries an expiry, a file that expires earlier than the stored one is refused
  (`AGENT_016`), so is one with the same expiry from a session that started before the stored
  file was written, and an expiry more than 400 days ahead; without an expiry, only a session
  that started on the stored file writes back. A
  vendor that rotates its refresh token can still make two sessions invalidate each other; then a
  new login is needed, and the status names the session that wrote last.

The login run reaches only the adapter's login hosts without a hold, as rules of that run; its URLs
are shown, and a client opens one on the host only after a click or `--open` and only for those
hosts. A browser callback that the sandbox cannot receive is pasted by the person, accepted only
as exactly `http://localhost:<port><path>?<query>` and delivered by one process inside the run; no
port of the host is ever forwarded into the sandbox, and under Seatbelt, where the sandbox shares
the host's loopback, only the device flow works (`AGENT_014`). The host's session bus is never
bound, also not for a login. With `agents.encrypt_credentials = true` (HUM-523) every credential
file lies sealed with ChaCha20-Poly1305 (`ring`), a fresh random nonce per write, the key only in
the system keyring and never in a file next to the data; without a keyring that answers the setting
is refused (`AGENT_020`), the switch is one transaction with a journal under one lock together with
`config.toml`, finished forward after a crash before a status answer or a start, and a file that
does not open with the key, or lacks the header in a sealed depot, is a finding and stays as it is. *Residual
risk:* the covert channel above; without `agents.encrypt_credentials` the credential file lies
unencrypted, `0600`, in the depot, and with it the daemon still decrypts for every session, so a
process of the same user that reaches the keyring or the daemon's memory reads the login, and the
encryption protects only the files at rest (a copy, a backup, a disk image); the key does not
rotate; processes of the same user can change the depot; the formats of the real agents' credential files are declared only once measured, and
until then their login answers `AGENT_014` and nothing is taken back.

### K-37 The project choice and the project register

The daemon keeps a register of known projects and, per project, the **project choice** (HUM-532,
ADR-0024): a configuration layer above the project profile and below environment and command line.
**Attacker:** a cloned repository and the agent, which can write `/work` and therefore the project
profile `<project>/.humanitl/profile.toml`. **Defence:** the choice is not in the project folder; it
lives in the daemon's SQLite store under its data directory and is written only through
`Projects.save_choice` on the daemon socket, which the sandbox cannot reach (guarantee 2). Its keys
are a fixed allow-list (`CONFIG_024` for any other), it never lowers `sandbox.min_isolation`, and a
company lock wins over it; the same checks run when a start loads it, so a choice that no longer
holds stops the start instead of vanishing. The register recognises a folder by its canonical path
together with device and inode: another folder at the same path is a new project without the old
choice. A start without a project folder is refused (`SANDBOX_053`) instead of mounting the
daemon's working directory, which a desktop launcher may have set to anything. *Residual risk:*
other processes of the same user can change the store, as they can `config.toml`; a client on the
daemon socket can save any choice the allow-list admits, as it can call `SetConfig`.

**Cleanup and forgetting** (HUM-533, ADR-0024 point 14). The register can delete recorded flows
and bodies of a project, the waiting changes of a disposable run (only on explicit choice, with the
warning that changes not applied are lost for good), and the project choice with its register entry;
forgetting a project is such a plan limited to choice and entry. **Attacker:** a client or a race
that would delete more than the person saw, or a stale confirmation. **Defence:** nothing is deleted
that the daemon did not list item by item first (`Projects.cleanup_plan`) and the person confirmed;
`Projects.cleanup` takes the digest of that plan, computes the plan again inside the deleting
transaction and deletes only when the digest is the same. The digest binds identity and version of
every item (session and flow ids with the rows of each flow, blob hashes and whether each falls,
the id and listing of the waiting layer,
project id with version, the hash of the choice), not sizes or counts; a new session, a changed
choice of the same length or a session that started makes it `SANDBOX_054`, and nothing is deleted.
The running session is never part of a plan. The project folder, the audit chain (`audit.jsonl`,
`audit_anchors`), rules, the global configuration and the agent depot are never part of one; each
run leaves one `projects.cleaned` record with kinds and amounts only. *Residual risk:* a client on
the daemon socket can request and confirm a plan without a human, as it can call `SetConfig`; the
plan still lists what goes, and the audit chain records that it went.

### K-38 The daemon's memory over a long session

**Attacker:** a process in the sandbox (b) that sends a great many small requests over a single
connection. The caps per connection and per session (HUM-120, HUM-467) bound only what runs at the
same time, not the sum over time. Before HUM-556 every request left a record in the flow registry
for the life of the daemon, about 2.3 to 2.6 KiB of heap each (measured with
`daemon/crates/proxy/tests/registry_memory.rs`), and the queue to the audit writer had no bound;
filling the daemon's memory would take gRPC, the recording and the audit log down with it.
**Defence:** the registry keeps at most `limits.registry_max_finished_flows` finished flows (default
10,000, `1..=1,000,000`, closed to project profiles); above that the oldest finished flow leaves. A
flow reaches `Recorded` through the hold queue, which hands each event to the registry and then to
the recorder, so the oldest finished flow has already passed its `Recorded` to the recorder; the
gRPC layer's transitions only broadcast, but they drive held flows, which never leave. Held and
running flows never count and never leave; their number is bounded by `limits.hold_max_flows` and
`limits.max_client_connections`. With the default the registry levels out at about 36 MiB (37.6 MB
after 80,000 and after 1,280,000 requests alike). The queue to the audit writer holds at most 4,096
records; a full queue makes the sender wait until the writer has made room, on a worker of the
runtime through `block_in_place`, rather than drop a record or grow. A notice of the sandbox
(HUM-477) goes straight into that queue and therefore always arrives; when the writer stops it first
takes no more and writes everything already queued before `daemon.stopped`, so an accepted record is
never discarded. When the audit sink falls behind the broadcast it writes `audit.events_dropped`
with the number of lost events and of the flows it held open at that moment, as soon as it notices
the gap, so at about the point in the chain where the lost events would have stood; a
`flow.received` of a flow already open before may follow it (HUM-564); the record names no flow,
host or path. So that its own table does not grow with every lost `Recorded`, the sink then closes
the flows it has not heard from since its previous gap and marks the rest; a flow whose end was lost
hears nothing again and leaves at the next gap at the latest, while a flow that keeps running keeps
its decision, status, size and duration. Between two gaps only flows whose `Received` the sink read
come in, and each of them is either still running at that point of the stream (bounded by the
proxy's caps on concurrent flows) or loses its end in the next gap and leaves one gap later, so the
table never holds more than the open flows of two gaps. *Residual risk:* while the audit sink waits
on a full queue it falls behind the event broadcast and loses flow events with `Lagged`, the
documented limit "events never written" (`docs/SECURITY.md`, section 8); the chain shows where and
how many, never what. A flow without a single event across two consecutive gaps, for example one
held or silent in its response that long, counts as lost: if it ends after all, its `flow.responded`
is missing, and a retraction after that point counts once more in `session.ended`. A flow whose
`Failed` event falls into a gap has no `flow.failed` (HUM-565): the `Recorded` after it names only
the flow, not how it ended, so the chain shows the decision and the gap but neither a failure nor
its reason, which stays in the recording. A finished flow
that has left the registry loses what only the registry knew (whether the scan saw the whole
request, the hard-block finding of its row), and without a recording `ListFlows` shows only the last
finished flows.

### K-39 A session without a terminal

**Attacker:** an agent (b) that waits until nobody watches. Since GitHub #13 closing the terminal
of `humanitl go` leaves the session running (`terminal.on_close = "detach"`, default), as closing
the app always did. Without a terminal nobody may notice a held request for a while, and a session
nobody sees could run on unnoticed. **Defence:** a held request leaves only after a person allows
it; when its deadline expires it is blocked (`hold.timeout_secs`, `BlockReason::Timeout`), so an
unwatched session cannot turn waiting into an allow. The three guarantees, the door and the filter
are the daemon's and stay in force whoever is attached; the check verdict was never a client's
(HUM-585). The running session stays visible: `humanitl sessions` says when no terminal hangs on it
(the daemon counts the clients of the agent's terminal, `Status.terminal_clients`, contract 1.62),
the start window marks it under "Running", the tray shows it, and a second start in another folder
is refused with `CLI_005`, whose fix leads to it. The key is closed to project profiles, so a
cloned project cannot keep its own sessions alive behind a closed terminal; `humanitl run` and
`SIGTERM` still stop. After `SIGHUP` the command writes nothing into the closed terminal and ends;
nothing it started keeps it alive. *Residual risk:* a detached session keeps its agent working, and
what it does within its allowed rules (the LLM passthrough, `allow` rules, the project folder) goes
on without anyone watching, until someone stops it or the agent ends. That is the same as a session
whose app window was closed before #13.

## Assumptions

| ID | Assumption | How to check it | If it breaks |
|---|---|---|---|
| A-1 | `bubblewrap` 0.8 or newer is installed and may create unprivileged user namespaces | `bwrap --version`; `sysctl kernel.unprivileged_userns_clone` or `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` | no session starts; no weaker fallback, a diagnostic with the fixing command |
| A-2 | the kernel is 5.4 or newer (the minimum `humanitl doctor` checks, tested from 5.10) with seccomp-BPF | `uname -r`; `grep Seccomp /proc/self/status` | the shim child cannot install the filter and stops before `exec` (`SANDBOX_016`, or `SANDBOX_013` without a report) |
| A-3 | the human reads what they approve | not technically checkable; the window makes reading cheap: findings first, full body, destination as identity | Humanitl is a recorder, not a protection; the history still shows what was approved |
| A-4 | the LLM host is under the user's control | `humanitl config get llm.endpoint`; line 4 of the isolation panel | the model channel is full access to everything the agent sends the model |
| A-5 | the host is not compromised | the download against `SHA256SUMS` of its release ([verify-download](guides/verify-download.md); releases are signed from 1.0.0 on), `systemctl --user status humanitld` | every statement here is void |
| A-6 | the sandbox profiles are the shipped ones or deliberately changed ones | `humanitl sandbox argv` | an extra socket fails the isolation check; an extra file mount is visible only in the bwrap line |

## Out of scope

These attackers are not defended against, and Humanitl does not claim to:

- **Host compromise.** Root on the host reads the CA key, the keyring and the daemon's memory, and
  can place a CA in the system store the daemon trusts towards destinations (HUM-257). Kernel
  exploits that break namespaces or seccomp belong here too; the countermeasure is the choice of
  well-used components and timely kernel updates.
- **Processes of the same account outside the sandbox** that change the daemon's setup: its
  environment (for example `SSL_CERT_FILE`, `SSL_CERT_DIR`) or `config.toml`. That is not attacker
  (d), who changes records; whoever configures the daemon is trusted.
- **Physical access**, an unlocked session, cold-boot attacks, memory dumps.
- **A malicious user.** Whoever approves a request approves it. Humanitl informs the decision, it
  does not make or overrule it. Manipulated Humanitl binaries belong here too. Releases before
  1.0.0 are not signed: `SHA256SUMS` detects a damaged download, not a tampered release, and
  signed releases come with 1.0.0 (HUM-060). Updates from inside the app check a signed manifest
  (K-25).
- **Microarchitectural and timing side channels**, electromagnetic emanation.
- **Denial of service** against the agent or the daemon.

## Residual risks

- **Unrecorded traffic during a recording outage** (K-16). Traffic continues while the recorder
  cannot write; the window says so, but what leaves through rules and the passthrough in that time
  has no record.
- **The audit chain against the same user** (K-14). A process of the same account has the HMAC key
  and the database and can rewrite history; only external anchoring (post-MVP) would change that.

- **The LLM host is shared trust.** Everything the agent shows the model, the model host sees. If
  the host is shared, foreign or compromised, no sandbox helps; the channel is open by design.
  Whoever has no machine of their own switches the passthrough off and lets inference be held.
- **Exfiltration through `/work`.** All three attackers can write into the project. Humanitl shows
  the touched files, symlinks and secrets at the end of a run, but whatever a human later commits,
  pushes or mails leaves without Humanitl.
- **The release key** (K-25). Whoever holds the private key that signs the update manifests can
  ship code that installations accept on a click.
- **A pause of intercepting** (K-26). For as long as a person keeps it on, requests that would wait
  leave without being seen first; they are recorded and marked `paused`. The agent can notice the
  missing wait.
- **The human as classifier.** A base64 body to `api.github.com` looks legitimate. Humanitl puts
  findings first, groups by host, removes noise with rules, asks back before approving whole
  groups and lets a timeout end in `block`. It cannot read the human's attention.

## Change history

| Date | Change | Reason |
|---|---|---|
| 2026-09-02 | First version: four attackers, fourteen channels, six assumptions | HUM-007, security review of the same day |
| 2026-09-04 | K-15 added: a red isolation check stops the session instead of preventing it | HUM-041 |
| 2026-09-23 | K-02: `allow`/`redact` path patterns never match `..` segments; `block`/`ask` also check the resolved path | HUM-204 |
| 2026-09-23 | K-04, K-06: the shim is PID 1, filtered and not dumpable; `pidfd_getfd` in the floor | HUM-203 |
| 2026-09-23 | K-13 visible: refused `socket()` calls counted through a seccomp listener in the shim parent | HUM-138 |
| 2026-09-24 | Hard block: a checksum-confirmed secret is held for editing; `Allow` refused with `HOLD_004` | HUM-159 |
| 2026-10-06 | K-02: the provider access key in the system keyring, to the agent only as a sandbox file, masked in the recording | HUM-140 |
| 2026-10-19 | `stream: true` as the declared exception to buffering, never under the hard block | HUM-057 |
| 2026-10-20 | The daemon trusts the system certificate store towards destinations; opt-out `tls.trust_system_store` | HUM-257 |
| 2026-10-20 | K-02: generated passthrough endpoints match exactly, not as prefixes | HUM-261 |
| 2026-10-21 | K-01: project and extra mounts bound by descriptor | HUM-263 |
| 2026-10-21 | Recording outage reaches every client and the window | HUM-250 |
| 2026-10-21 | Translated to English and brought to the state of the code for 0.1 | HUM-059 |
| 2026-10-22 | K-17: agent program from a per-user directory bound read-only as one file | HUM-278 |
| 2026-10-23 | K-02: several saved model servers, one keyring item each; only the active origin gets a key and the passthrough | HUM-289 |
| 2026-10-24 | K-06: the session's first process takes the daemon's pseudo-terminal as controlling terminal, `--new-session` stays; K-17: a caller's command gets the configured agent bound beside it, never itself | HUM-280 |
| 2026-10-29 | B1: the agent's own permission prompts are no boundary; OpenCode's for `webfetch` and `bash` are off | HUM-308 |
| 2026-10-29 | K-18: further shells next to the agent through a socket pair only the daemon and PID 1 hold, forked inside with the agent's filter on top | HUM-299 |
| 2026-10-29 | B1: a globally installed pi extension (`pi-web-access`) runs inside the agent with all its restrictions; its requests pass the proxy and are held there when no rule decides them; `agent.pi.web_access` closed to project profiles | HUM-309 |
| 2026-10-30 | K-19 to K-23: macOS with Seatbelt; shared loopback, no PID namespace, paths instead of descriptors, a deprecated interface, host-local side channels | HUM-323, HUM-327, HUM-331 |

| 2026-10-30 | K-24: the disposable sandbox, an overlay over the read-only project with the upper layer in RAM and a size cap; changes reach the project only through a checked apply | HUM-350 |
| 2026-10-30 | K-02: on macOS the provider access key lives in the login Keychain through `/usr/bin/security`, over `stdin` and read back; same class as the Secret Service: readable by every process of the account outside the sandbox, out of reach of the agent | HUM-349 |
| 2026-10-30 | K-24: overwriting a conflict is confirmed per change (`overwrite_ids`); a confirmation for one file never covers another | HUM-351 |
| 2026-10-31 | K-02: the boundary is the saved server the session resolved to; `llm.server` from run, project or profile chooses among the user's saved servers, never defines one; an unknown name refuses the start | HUM-400 |
| 2026-11-02 | K-02: a project's choice of a server other than the default is confirmed once per project folder and name, stored in the daemon's data directory; the daemon refuses the start until then and writes the project file without following links | HUM-405 |
| 2026-10-30 | K-24: the disposable sandbox becomes the default where the system allows it (`sandbox.mode = "auto"`); without the overlay the default runs direct with `SANDBOX_033`, an explicit `disposable` still refuses | HUM-357 |
| 2026-11-01 | K-26: pausing intercept, set only over the daemon socket, never from the sandbox; held nothing while on, block rules and the hard block stay; ends on resume, with the next session and with the daemon; logged and shown as a caution tape | HUM-407 |
| 2026-11-01 | K-25: updates from inside the app, bound to an Ed25519 release key built into the daemon; daily check of GitHub on by default, `update.check` turns it off; packages through `pkexec` and their package manager | HUM-382 |
| 2026-10-31 | K-24: the disposable sandbox on macOS, an APFS clone with the masks removed, the real project sealed and the clone renamed before it is read; a held write descriptor and the missing size cap stay as residual risks | HUM-374 |
| 2026-11-01 | The reserved name `llm.humanitl.internal` for a model on the loopback: mapped by the proxy only to the configured endpoint, scheme and port, before the rules, never resolved; every other use refused with `PROXY_003`. No new door, the sandbox profile is unchanged | HUM-387 |
| 2026-11-04 | A-5 and manipulated binaries: releases before 1.0.0 carry `SHA256SUMS` only, no package signature; signed releases come with 1.0.0. The earlier text named a package signature that never shipped | HUM-060 |
| 2026-11-12 | K-06: the shim's lifeline (`--lifeline-fd`) as the counterpart of `--die-with-parent` for a backend that detaches the sandbox from the daemon; the agent's environment through a sealed memfd (`--env-fd`) instead of `argv`; the agent inherits neither; K-15 unchanged | HUM-430 |
| 2026-11-12 | K-27 added: the container engine under the Podman backend, plain level, not admitted until HUM-448 | HUM-433 |
| 2026-11-12 | K-27: ESC-1 and ESC-2 run against Podman in the CI job `escape-tests-podman`, with a planted hostile engine configuration of the user (`mounts.conf`, volume, hook) that must not reach the sandbox | HUM-435 |
| 2026-11-12 | K-24: no disposable sandbox under Podman; the engine's overlay volume was measured (P10) and is not used, because its lower layer cannot be bound to the checked folder; `disposable` refuses with `SANDBOX_030`, `auto` runs direct with `SANDBOX_033` | HUM-438 |
| 2026-11-12 | K-01, K-05, K-27: the shim compares the mount table with the plan and the device and inode of every checked folder before the agent starts, under every backend (`SANDBOX_049`, `SANDBOX_042`); under Podman a swapped folder is detected and refused instead of prevented; K-27 names the configuration sources of Podman | HUM-437 |
| 2026-11-13 | K-28 added: an image as the root of the sandbox under Podman, pinned by digest and checked before every start (`SANDBOX_043`), pulled only on request and directly, not through the proxy; ESC-2 checks the CA by device and inode | HUM-439 |
| 2026-11-13 | Trust boundaries: under its hardened unit the daemon starts Podman as transient units `humanitl-*` of the user's service manager instead of loosening its hardening; a compromised daemon gains "start a transient user unit". K-27: the shim's descriptors through a one-connection handover socket instead of `--preserve-fds` | HUM-434 |
| 2026-11-13 | K-28: registry credentials for a pull only from the adapter's own file, as a private copy under `--authfile`; credential helpers in that file or in a registry drop-in refused (`SANDBOX_043`) | HUM-456 |
| 2026-11-13 | K-27 `mitigated`: the plain level of Podman (`podman/crun`) admitted after `escape-tests-podman` (ESC-1, ESC-2, ESC-5) was green on `main` with HUM-434, HUM-436, HUM-437 and HUM-438; chosen only by a person, never by `auto`; the kernel level stays refused with `SANDBOX_048` until HUM-449 | HUM-448 |
| 2026-11-14 | K-29 added: the control stream of the terminals, the stream mode of the kernel level and of bwrap on request; K-18 in the stream mode; further shells prove a refused `AF_VSOCK` as well | HUM-443 |
| 2026-11-14 | K-30 to K-33 added: the monitor, the vsock door, the guest kernel and the helper processes of the kernel level; one hardened unit per session, four jails, the checked project folder by descriptor, a host check of the monitor (`SANDBOX_047`) before the agent is admitted, each jail binding only its own socket directory, the end of the daemon ends the unit; not admitted until HUM-449 | HUM-444 |
| 2026-11-14 | K-30, K-31: when the guest has already refused, its diagnosis is shown and the host check's findings stand in the evidence, the session is refused either way; the host never gives up a pending `CONNECT` into the guest | HUM-470 |
| 2026-11-14 | K-32: the profile's tmpfs (`/tmp`, `/var/tmp`, the agent's home) and `/run` are writable tmpfs in the guest, `/run/humanitl` read-only on top, mounted by the shim, in the expected mount table and in the socket walk; `mounts.extra_rw` refused at the kernel level (`SANDBOX_051`) instead of mounted read-only | HUM-466 |
| 2026-11-14 | K-33: each file system daemon of the kernel level holds at most 65,536 descriptors (`LimitNOFILE` of the unit) and runs with `--cache never --allow-mmap` (guest kernel 6.6 or newer, `SANDBOX_045` below); closes the finding of HUM-445, where a walk over the guest root drove the root daemon to about 520,000 descriptors | HUM-469 |
| 2026-11-14 | K-31: the host opens the report and control connections only after the shim's ready line on the door; until then the door forwards nothing to the proxy | HUM-471 |
| 2026-11-14 | K-31: at most 64 open connections through the door per session; one above that is closed at once, and the first is noted in the evidence of `single_socket` | HUM-467 |
| 2026-11-14 | K-33: the file system daemons run with `--cache auto`; PID 1 in the guest trims unused dentries and inodes above 32,768, checked every 100 ms (`vm.drop_caches=2`), so a walk over the whole root no longer ends with `ENFILE`; `LimitNOFILE=65536` stays the guarantee | HUM-476 |
| 2026-11-14 | K-34 added: the shared file systems of the kernel level; the host check finds the opened project folder (and the layer) at the daemon that shares it; the disposable sandbox at the kernel level, an overlay in the guest over the read-only project and a layer from a third daemon, read only after the unit has ended, with every change on, under or above a mask refused; K-24 and K-33 follow | HUM-446 |
| 2026-11-14 | K-33: the host check knows the programs of the unit; a process that runs anything but the script's shell, bubblewrap, a file system daemon, the vsock device or the emulator, or whose program is deleted or unreadable, refuses the session with `SANDBOX_047`, also when it holds no network and no device; the check rereads the unit's processes until no new one appears, but a process started after admission is not seen (the check runs once, before the agent) | HUM-468 |
| 2026-11-15 | K-33: the unit of a kernel-level session binds at most `MemoryMax` (machine memory, 512 MiB for the helpers, the size of every tmpfs it writes to), as much swap and `TasksMax` (vCPUs plus 128); 64 MiB tmpfs per mask over the project; without the cgroup controllers `memory` and `pids`, or when the running unit's cgroup lacks the planned limits, the session is refused (`SANDBOX_052`) | HUM-485 |
| 2026-11-15 | K-33: the file system daemons run without `--allow-mmap`, and no guest kernel version is refused any longer; both belonged to `--cache never` (HUM-469), under `--cache auto` shared mappings go through the guest's page cache on any kernel (measured on 6.12 and 7.3, directly and in a disposable session); a kernel too old for the guest's other needs fails its boot or check and the session is refused | HUM-484 |
| 2026-11-15 | K-33: PID 1 in the guest also trims when the guest holds more than 49,152 inodes and 8,192 more than after the last trim, at 57,344 at the latest, so directory chains that a trim met in use and that were let go afterwards no longer stay until the limit; a KVM test shows that the agent holds none of the three files of the trim | HUM-483 |
| 2026-11-15 | K-34, K-24: a disposable sandbox refuses a project whose root or a folder on the way to a mask folds case (casefold attribute, vfat, exFAT, HFS+, SMB, or a mask entry that resolves under the other ASCII case), because the scan compares masks exactly and the guest's virtiofs hides the host's folding (`SANDBOX_030`, `auto` runs direct); the daemon records the device and inode of `upper` at creation and refuses a swapped or missing `upper` with `SANDBOX_031` | HUM-482 |
| 2026-11-15 | K-30 to K-34 `mitigated`: the kernel level (backend `qemu`) admitted after `kvm-tests` and `escape-tests-qemu` were green on `main` with HUM-446 and the probes with root in the guest of HUM-445, in ten consecutive runs in which both jobs ran (43624997550, 43630993303, 43638237420, 43641769840, 43646599744, 43648968241, 43656202366, 43661904978, 43669712307 and 43673927540, the rule written at HUM-471); chosen only by a person with `sandbox.backend = "qemu"` or `--backend qemu`, never by `auto`; a missing prerequisite refuses the start with `SANDBOX_045` | HUM-449 |
| 2026-11-16 | K-35 added: a decision proxy of the user behind Humanitl (`egress.proxy`, opt-in), its CA only on that way, no direct fallback, who resolves chosen with `egress.proxy_resolve` | HUM-511 |
| 2026-11-16 | K-35: with `hold.ask_mode = "upstream"` the decision proxy decides requests without a rule; only with a proxy in use, `block` never reaches it, `redact` and the hard block stay in Humanitl, the audit says `forward` by `upstream` with the proxy's status | HUM-512 |
| 2026-11-17 | K-02: cloud inference named; an endpoint outside this machine and its network shows an orange tag, a start line and a doctor line, and asks once per agent and provider, confirmed in the daemon's data directory; blocks nothing | HUM-516 |
| 2026-11-17 | K-36 added (first part): the agent depot as the agent's origin, the read-only version directory and the scratch copy of its configuration, one guard on every backend's plan (`AGENT_017`), masks of host installs (empty read-only mounts only where they exist, Seatbelt deny rules), the limit of the company lock | HUM-518 |
| 2026-11-18 | K-37 added: the project choice as a host-side layer that neither a clone nor the agent writes, its allow-list, the isolation floor only raised, company locks, recognition by device and inode; a start without a project folder is refused | HUM-532 |
| 2026-11-18 | K-37: cleanup and forgetting only through a listed, confirmed plan whose digest binds identity and version of every item and is checked again in the deleting transaction (`SANDBOX_054`); never the project folder, the audit chain or the running session | HUM-533 |
| 2026-11-18 | K-31: the first connection above the door's cap reaches the audit log once per sandbox launch (`sandbox.door_capped`), written straight to the audit writer and not through the bounded event broadcast, and every isolation check of the kernel level, the host check included, stands there as `isolation.check`; a channel from the sandbox backend into the session's event stream carries both | HUM-477 |
| 2026-11-18 | K-31: no shared cap on door connections; the per-session cap suffices while the daemon runs one session (`CLI_005`), and several sessions (HUM-549) must decide a shared budget first | HUM-479 |
| 2026-11-19 | K-33: the unit of a kernel-level session runs only its programs, their loaders and libraries (`NoExecPaths=/`, `ExecPaths=` with single files), in every bubblewrap jail and also after admission; the host check counts shells, bubblewraps and file system daemons; residual: code in a `memfd` or in a mount bubblewrap creates inside a jail still runs | HUM-480 |
| 2026-11-19 | K-34: in a disposable session of the kernel level the shim lays the profile's masks over the overlay in the guest as bubblewrap does, each in the expected mount table; the layer's file system daemon takes only `user.overlay.*` attributes (`--xattrmap`), any other fails with `ENOTSUP` | HUM-481 |
| 2026-11-19 | K-24: on macOS the scan of the clone finds a mask by looking its name up in the clone itself and comparing device and inode, so no spelling that APFS folds to a mask (Kelvin sign, long s, another normalization) is applied; a mask the human spelled otherwise in the project is no deletion; not covered: a differently folding volume mounted inside the project (ASCII comparison only for a mask present there, nothing for a missing one) | HUM-491 |
| 2026-11-19 | K-38: the flow registry keeps at most `limits.registry_max_finished_flows` finished flows (default 10,000), held and running flows never leave; the audit writer's queue holds at most 4,096 records and a full queue makes the sender wait, so no sandbox notice is dropped | HUM-556 |
| 2026-11-19 | K-33: each jail of the kernel level hides the unit's other programs behind `/dev/null`, allows no further user namespace (`--disable-userns`), remounts its root, `/dev` and the empty tmpfs of the guest root read-only, binds the masks over the project from tmpfs mounts of the unit with `noexec`, and lays on a seccomp filter that refuses `execveat` and, except for the emulator, `memfd_create` without `MFD_NOEXEC_SEAL`; residual: code a taken-over process runs in its own memory, and the unit's programs under the guest's `/usr` in the root daemon's jail | HUM-557 |
| 2026-11-19 | K-36 second part: install and update as a maintenance run without a project, only the recipe's hosts without a hold as rules of the run, the takeover checked, verified and switched by the daemon, the version pinned | HUM-563 |
| 2026-11-20 | K-36 complete (third part): login and configuration runs of the depot program; only the credential file goes back, through a guard against persistence, account swap and older tokens of parallel sessions, every outcome in the audit log as `agent.credential_writeback`; the covert channel through the credential file named; configuration only after confirmation; a pasted callback delivered inside the run, never a forwarded port | HUM-520 |
| 2026-11-20 | K-02: Claude Code (HUM-514) gets the provider key from its sandbox file through a shell inside the sandbox that exports `ANTHROPIC_API_KEY` or `ANTHROPIC_AUTH_TOKEN`, never through the plan's environment or `argv`; the key is no finding in `X-Api-Key` towards the agent's inference endpoint, `api.anthropic.com:443` for Claude Code without `llm.endpoint`, whose passthrough covers only the Messages endpoints | HUM-514 |
| 2026-11-20 | K-02: Codex (HUM-515) gets the provider key from its sandbox file through the same shell inside the sandbox, which exports `OPENAI_API_KEY` for the provider the adapter declares; the key is no finding in `Authorization` towards `api.openai.com:443` with a key and without `llm.endpoint`, whose passthrough covers only `POST`/`GET` on `/v1/responses` and `/v1/models`. Codex's own sandbox (`sandbox_mode`) is `danger-full-access`, because its bubblewrap needs a user namespace that the outer sandbox refuses (`--disable-userns`); the outer boundary stays Humanitl. |
| 2026-11-20 | K-36: the credential files of the depot optionally sealed at rest (`agents.encrypt_credentials`, ChaCha20-Poly1305 from `ring`, a random nonce per write, the key only in the system keyring, the setting refused without one, `AGENT_020`); the switch converts all files atomically under the depots' locks; the limit named: the daemon decrypts for every session | HUM-523 |
| 2026-11-20 | K-36: the write-back guard never takes two files without a named account for the same account; wherever the object holding the account is present (Codex: `tokens`), the account must be a non-empty string, else the file is refused at the login too; a format may require one of several keys to hold a value (`one_of`), restrict a key to an enumeration (`OneOf`), demand material for a value (`requires`) and keep a key as stored (`fixed`; Codex: `auth_mode` only `apikey` or `chatgpt`, never changed by a session) | HUM-515 |
| 2026-11-20 | K-02: the Antigravity CLI (HUM-517) gets a Gemini API key from its sandbox file through the same shell inside the sandbox, which exports `GEMINI_API_KEY` and writes `modelProvider = "gemini"` into its settings on the sandbox's own home; the key is no finding in `x-goog-api-key` towards `generativelanguage.googleapis.com:443` with a key and without `llm.endpoint`, whose passthrough covers only `POST`/`GET` on `/v1beta/models` and `/v1/models` and below. Its Google sign-in lives in the host's keyring over D-Bus, which never enters a sandbox; the depot supports `agy` with an API key only. Its own terminal sandbox cannot start inside ours (`--disable-userns`); the outer boundary stays Humanitl. | HUM-517 |
| 2026-11-20 | K-38: flow events the audit sink loses behind the broadcast stand in the chain as `audit.events_dropped` with their number; after a gap the sink closes only flows it has not heard from since the previous gap, residual: a flow silent across two gaps loses its `flow.responded` | HUM-564 |
| 2026-11-20 | K-38: a flow whose `Failed` event falls into a gap of the audit sink has no `flow.failed`; `Recorded` names only the flow, the reason stays in the recording | HUM-565 |
| 2026-11-20 | K-36: an install may take a vendor's package of several files (Codex): the recipe lists the files it expects, the daemon starts the program through a link of its own and checks a vendor's list of checksums on its own copy (release metadata, list, archive); residual: only TLS to the vendor's host vouches for the metadata, and the unpacked tree is bound to the archive by the run, not the daemon. `agy` gets no install: its manifest offers no chosen version, points at a shared storage host and carries its only checksum itself | HUM-566 |
| 2026-11-20 | K-02: the passthrough pairs each method with its paths: an adapter gives one rule per set of methods (fixed ids `…00ff`, `…00fe`, `…00fd`). Codex passes `POST` only on `/responses` and `GET` only on `/models`; `agy` `POST` only on `models/{model}:generateContent` and `:streamGenerateContent` and `GET` only on the model list; Claude Code `POST` only on the Messages endpoints; OpenCode and pi `POST` on the endpoints that compute and `GET` on `/v1/models`, `/api/tags`, `/api/ps`, `/api/version`. `POST /models`, `GET /responses` and `GET` on an inference endpoint are now held. A path a human writes into `llm.passthrough_paths` keeps both methods. Narrows, weakens nothing | HUM-568 |
| 2026-11-21 | K-15: the verdict on the three guarantees and the stop after a red or missing result no longer depend on the starting client. Before, a client that closed its event stream after `Status(starting)` ended the check before the verdict, and a sandbox with a red result kept running; the folder check and the register entry of a green start were skipped the same way. The daemon now stops the sandbox before it sends the results, records the finding in the session summary (`stopped_by`) and stops it a second time if the stopping thread fails | HUM-585 |
| 2026-11-22 | K-02: a third way to enter the provider access key and the proxy credentials: the settings view `humanitl settings`, typed without echo, sent once through `LlmKey` or `Egress(SetAuth)`, never through `SetConfig`, never shown; the input buffer is overwritten on every delete, cancel, growth and drop, a pasted text once taken over, and the secret stays under `Zeroizing` until the request is built; not overwritten: the request field handed to tonic, the prost/tonic encode buffers and crossterm's read buffers | GitHub #7 |
| 2026-11-23 | K-39: closing the terminal of `humanitl go` detaches it (`terminal.on_close`, default `detach`, closed to project profiles); held requests wait for a person or their deadline, which blocks; the daemon counts the clients of the agent's terminal and `humanitl sessions` and the start window show a session without one; `humanitl run` and `SIGTERM` still stop | GitHub #13 |

Every security-relevant change to the shim, the mount allowlist, the filter or the passthrough gets
a line here. Vulnerabilities are reported as described in
[`SECURITY.md`](SECURITY.md#10-reporting-a-vulnerability).
