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
| Credentials | the host's environment, `~/.ssh`, `~/.netrc`, keyrings, tokens in other processes | a stolen token opens systems far beyond the project | PID namespace, mount allowlist, clean environment (`--clearenv`), the LLM key only as a sandbox file |
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
  inside is attacker-controlled.
- **B2, the one door.** The proxy socket file. Every byte through it is parsed by the daemon,
  which is why the daemon is memory-safe Rust and hardened by systemd.
- **B3, daemon to the network.** Only after a rule or a human allowed the request, resolved once
  after the decision, pinned to the address found.
- **B4, `/work`.** Shared by design. What the agent writes there, the host's tools may later run
  or a human may carry out.
- **The window and the CLI** sit on the host side. They cannot weaken the sandbox, but they show
  the human what they decide about.

## Attack surface

The twelve channels of the security review of 2026-09-02. Severity is the impact **if** the
channel is used, not its likelihood. Status is `mitigated` (a mechanism in 0.1 closes or contains
it), `declared` (deliberately open, shown in the application, not closed) or `open` (a known gap
without a mechanism yet).

| ID | Channel | Attackers | Severity | Mitigation in 0.1 | Status | Test |
|---|---|---|---|---|---|---|
| K-01 | project directory `/work` | a, b, c | high | only a subpath; hooks, CI and editor directories and secret files masked; session summary with touched files, outward symlinks and a secret scan | declared | ESC-5 |
| K-02 | LLM passthrough | a, b, c | high | exact rule on host, port, scheme, methods and exact inference endpoints; full recording; findings warn | declared | - |
| K-03 | socket directory of the proxy | b | medium | the socket **file** is bound, never its directory | mitigated | ESC-2 |
| K-04 | TCP sockets allowed by seccomp | b | low | empty network namespace and routing table; parent filtered and not dumpable | mitigated | ESC-1 |
| K-05 | host Unix sockets (X11, Wayland, D-Bus, Docker) | b | critical | mount allowlist, denylist in the argument builder (`SANDBOX_006`), `AF_UNIX` refused | mitigated | ESC-2 |
| K-06 | PID and IPC namespace | b | high | own PID, IPC and UTS namespaces, own `/proc`, `--new-session`, shim as non-dumpable PID 1 | mitigated | ESC-1, ESC-2 |
| K-07 | `/proc` and `/sys` | b | low | the namespace's own `procfs`, `/sys` not bound | mitigated | ESC-2 |
| K-08 | symlinks out of `/work` | b | medium | resolved inside the mount namespace; marked in the session summary (`SANDBOX_022`) | mitigated | ESC-5 |
| K-09 | terminal escapes (OSC 52, OSC 8, titles) | a, b, c | medium | allowlist filter in the daemon before any client; no OSC handler in the widget | mitigated | ESC-5 |
| K-10 | DNS and the time of resolution | a, b, c | high | resolution only after approval (ADR-006), pinned address | mitigated | ESC-3 |
| K-11 | queue and rule metadata | a, b, c | low | `humanitl.internal` is read-only, `/ask` never creates a rule; the policy oracle stays | declared | ESC-4 |
| K-12 | package caches | b | medium | caches per project, not per trust level; none unless configured | declared | - |

Four further channels complete the model:

| ID | Channel | Attackers | Severity | Mitigation in 0.1 | Status | Test |
|---|---|---|---|---|---|---|
| K-13 | direct network egress | a, b, c | critical | network namespace, seccomp, no capabilities | mitigated | ESC-1, ESC-3 |
| K-14 | audit file on the host | d | high | hash chain with HMAC, anchors in file and SQLite; mitigated only against an attacker without the HMAC key: not against a same-user process that has the key, not a cut behind the last anchor | declared | ESC-5 |
| K-15 | the window between `exec` and `SIGKILL` | b | high | report read at once, `SIGKILL` without grace | open | - |
| K-16 | recording outage | a, b, c | medium | outage reported to every client once per cause, cards marked "not recorded", first approval paused | declared | `daemon/crates/recorder` health tests, golden `states_recorder_failure` |

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
reaches the agent. *Residual risk:* the agent can rearrange, disguise or plant data in the project
that a human later carries out; Humanitl sees that only if the human reads the diff.

### K-02 The LLM passthrough

The agent must send code to the model; these requests are not held. The model host is therefore
the second open channel and, unlike `/work`, one over the network. The rule is as narrow as
possible: one host, one port, one scheme, `POST` and `GET`, and thirteen exact inference endpoints
instead of the surfaces `/v1/` and `/api/` (HUM-261); a path with `..` segments is held (HUM-204).
Traffic is recorded and scanned (`LLM_005`), and with `hold.hard_block_checksum_secrets` a
checksum-confirmed secret is blocked (`HOLD_004`). The access key of a provider (HUM-140) lives in
the system keyring and reaches the agent only as a sandbox file; it is masked in the recording and a
finding everywhere except in `Authorization` towards `llm.endpoint`. With several saved model
servers (HUM-289) each has its own keyring item, and only the active server's origin gets a key and
is the passthrough of the next session; a probe sends a saved server's key only to that server's own
origin, never to another's; an endpoint from the environment or the command line above the active
server, a shared reference or a missing active server leaves the key unsent. *Residual risk:*
complete.
Whoever steers the agent through prompt injection can put project content into a prompt. If
`llm.endpoint` is a name, whoever controls that name can redirect the passthrough between two
requests, and because the rule carries `allow_private`, the router or `169.254.169.254` are valid
targets; use an address or `resolver.overrides`. The agent can read the key and send it elsewhere,
where it is a finding but held only at a host without an allowing rule.

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
allowlist; the full bwrap line is visible in `humanitl sandbox argv`; the argument builder refuses
`$XDG_RUNTIME_DIR`, `/tmp`, `/run`, `/home`, `~/.ssh`, `~/.gitconfig`, `~/.netrc` and these sockets
(`SANDBOX_006`); the isolation check looks for unexpected sockets in the running sandbox; and after
the filter `AF_UNIX` is refused anyway. *Residual risk:* a user writing their own profile that adds
a file mount (not a socket) such as `~/.ssh` read-only; the check does not see file mounts.

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
`/proc/1/environ` and `/proc/1/mem` are unreadable for the agent. *Residual risk:* none known, as
long as bwrap implements the options as documented.

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
holds filtered bytes; the widget registers no OSC handler. *Residual risk:* a bug in filter or
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
without grace; the upper bound is 10 s in the daemon and 15 s for `humanitl sandbox run` and
`escape-launch`. Every request no rule decides still waits for a human. Closing it means the proxy
refuses connections until the isolation is proven, a separate issue. `a_second_socket_in_the_project_stops_the_start`
in `daemon/crates/ipc/tests/sandbox_start.rs` proves the session is stopped and never reported as
running; that nothing leaves in the window is **not** proven.

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

## Assumptions

| ID | Assumption | How to check it | If it breaks |
|---|---|---|---|
| A-1 | `bubblewrap` 0.8 or newer is installed and may create unprivileged user namespaces | `bwrap --version`; `sysctl kernel.unprivileged_userns_clone` or `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` | no session starts; no weaker fallback, a diagnostic with the fixing command |
| A-2 | the kernel is 5.4 or newer (the minimum `humanitl doctor` checks, tested from 5.10) with seccomp-BPF | `uname -r`; `grep Seccomp /proc/self/status` | the shim child cannot install the filter and stops before `exec` (`SANDBOX_016`, or `SANDBOX_013` without a report) |
| A-3 | the human reads what they approve | not technically checkable; the window makes reading cheap: findings first, full body, destination as identity | Humanitl is a recorder, not a protection; the history still shows what was approved |
| A-4 | the LLM host is under the user's control | `humanitl config get llm.endpoint`; line 4 of the isolation panel | the model channel is full access to everything the agent sends the model |
| A-5 | the host is not compromised | package signature, `systemctl --user status humanitld` | every statement here is void |
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
  does not make or overrule it. Manipulated Humanitl binaries belong here too; their
  countermeasure is the package signature (HUM-060).
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

Every security-relevant change to the shim, the mount allowlist, the filter or the passthrough gets
a line here. Vulnerabilities are reported as described in
[`SECURITY.md`](SECURITY.md#10-reporting-a-vulnerability).
