# Security

Applies to: Humanitl 0.1

This document explains why an agent running under Humanitl cannot reach the network unseen,
where that statement ends, and how to check both yourself. It is written for two readers: a
professional without a security background who wants to know what they can rely on, and a
reviewer who checks every claim against a command or a file. Who attacks what is described in
[`THREAT-MODEL.md`](THREAT-MODEL.md); this document describes the mechanisms.

Where this document and the code disagree, the code is right and this document has a bug. The
seccomp rule table, the sandbox profile and the configuration schema are the sources; the
places that cite them are checked by tests (`security_doc_mentions_syscalls` in
`daemon/bin/humanitl-shim/src/seccomp.rs`, the generated [`CONFIG.md`](CONFIG.md)).

## 1. The claim

> The sandbox has no network interface, there is no way out, and the kernel refuses every
> socket family except loopback TCP. Its only ways in and out are the project directory `/work`
> and exactly one Unix socket to the proxy. Every byte on that socket is either an HTTP request
> you approved or a call to the declared LLM host, and both are recorded.

"You approved" means: you, at the moment the request was held, or in advance through a rule in
your rule set (your own rules, or the bundled ones you left switched on). Approval governs what the
proxy forwards upstream. Requests an `allow` rule matches and calls to the declared LLM host go
without a hold. "Both are recorded" holds while the recorder works: during a recording outage
(HUM-250) traffic continues unrecorded, and every client is told so
([section 5.6](#56-other-known-limits), item 12). The one deliberate exception to "you, at the
moment the request was held" is the pause of intercepting (HUM-407,
[section 6](#pausing-intercept)): for as long as you keep it on, a request that would wait for you
goes out at once. You switch it on over the daemon socket only, it is visible and logged,
and it ends by itself. The claim rests on
three guarantees. They stand word for word in the isolation panel of the application (ARB keys
`isolationCheck1` to `isolationCheck3`) and in `BACKLOG.md` 4.1:

| # | Guarantee | Checked by |
|---|---|---|
| 1 | **No network interface. There is nowhere for traffic to go.** | [section 2](#2-guarantee-1-no-network-interface), ESC-1, ESC-3 |
| 2 | **Exactly one door: a socket that leads to Humanitl.** | [section 3](#3-guarantee-2-exactly-one-door), ESC-2 |
| 3 | **The kernel opens no new door (seccomp).** | [section 4](#4-guarantee-3-no-new-doors), ESC-1 |

On macOS the sandbox is Seatbelt instead of bwrap, and the three guarantees read differently
there; [section 12](#12-macos-seatbelt-instead-of-bubblewrap) states them and what changes.

### What the claim does not say

It does **not** say "the agent cannot exfiltrate data". Two channels are deliberately open,
shown in the application and described in [section 5](#5-declared-side-channels):

1. **The project directory.** The agent writes to `/work`. Whatever it leaves there is on your
   disk afterwards, including something a human later carries out themselves.
2. **The connection to the language model.** The agent sends code to the model without every
   inference being confirmed. Whoever controls the model host sees this traffic.

The precise statement is: the agent cannot exfiltrate **over the network** except through
requests that were approved, by a human or by a rule, and through the declared LLM host. The
sentence "the LLM host is part of the trusted computing base" is the condition under which
everything else holds.

Guarantee 3 does **not** mean "no sockets at all". The filter has to allow TCP over IPv4 and
IPv6, because that is how the agent reaches the proxy on `127.0.0.1:3128`. That an allowed TCP
socket still leads nowhere is the work of the empty network namespace from guarantee 1, not of
the filter: only `lo` exists there, and the routing table is empty. The three guarantees hold
together; none of them holds alone.

### The agent's own permission prompts (HUM-308)

Agents bring their own permission prompts: OpenCode asks "allow webfetch?" before it fetches a
page and can ask before a shell command or an edit. Inside the sandbox Humanitl switches these off
for fetching and running commands (`"webfetch": "allow"`, `"bash": "allow"` in the bundled
`opencode.json`, also in `OPENCODE_PERMISSION`); `edit` stays at OpenCode's own default, which
already allows it without a prompt. pi 1.4.0 has no such prompts. None of the guarantees above
rests on these prompts, and switching them off weakens none of them:

- **They are not a boundary.** The prompt runs inside the agent process, which sits on the
  untrusted side of B1 ([`THREAT-MODEL.md`](THREAT-MODEL.md#trust-boundaries)). A steered or
  compromised agent does not ask; a request it sends anyway is held at the proxy all the same.
- **The network question is asked once, at the door.** Every request leaves only through the
  proxy and is approved there, by a rule or by you. A second prompt before the same request
  asked you twice for one decision and taught you to click through.
- **Shell and edits stay inside the sandbox.** Commands run under the same namespaces and filter;
  writes that outlive the session reach only `/work`
  ([section 5.2](#52-the-project-directory-work)), which you review afterwards in the run
  summary, plus any `mounts.extra_rw` a profile declares and configured package caches
  ([section 5.5](#55-package-caches)). `/tmp` and the agent's home are a `tmpfs` and end with the
  session. `external_directory` stays `deny` and `websearch` stays `deny`.

What stays with the agent is the question it may still ask about its own work, such as OpenCode's
prompt before reading a `.env` file or after the same call repeated three times. Neither is a
network decision, and both are left at OpenCode's default.

### Third-party agent extensions (HUM-309)

With `agent.pi.web_access` at `auto` (the default) or `on`, pi loads the npm package
`pi-web-access` when you have installed it globally. That is third-party code running inside the
agent process, so it sits on the untrusted side of B1 like the agent itself and gets no right the
agent does not have: the same namespaces, seccomp filter and mounts; its directory lies in a
read-only mount (normally `/usr`); it leaves only through the proxy, and every request it makes is
decided by a rule or, when no rule decides it, held for you, like any other (measured: a fetch arrived at the proxy as `CONNECT`). Its own
SSRF guard is told to let the proxy resolve names (`ssrf.trustEnvProxy`), because the sandbox has
no resolver; that changes no guarantee, since the proxy is the only door anyway. No API key of a
search provider reaches the sandbox. What the claim does not cover is what you install: Humanitl
does not review the package, so a malicious version can do inside the sandbox what a steered
agent could do there. `agent.pi.web_access = off` keeps it out, and a project profile cannot
switch it.

### Trusted computing base

The trusted computing base is the set of components whose failure voids the guarantees. It is
deliberately small and consists of components that many other projects use as well.

| Component | Role | Requirement | Why we trust it |
|---|---|---|---|
| Linux kernel | namespaces, seccomp-BPF, capabilities | 5.4 or newer (the minimum `humanitl doctor` checks), tested from 5.10; `CONFIG_SECCOMP_FILTER`, unprivileged user namespaces allowed | the same mechanisms carry containers, Flatpak and the sandboxes of Claude Code and the OpenAI Codex CLI |
| `bubblewrap` | builds the sandbox | 0.8 or newer (`min_bwrap_version` in `profiles/sandbox/default.toml`); 0.10 or newer for descriptor binding | small, setuid-free in rootless mode, packaged by Debian and Fedora, under scrutiny for years |
| seccomp-BPF filter | refuses further socket families and dangerous syscalls | generated by `seccompiler` plus a hand-written prelude, source `daemon/bin/humanitl-shim/src/seccomp.rs` | a few dozen rules, enumerated in full by a test |
| `humanitl-shim` | holds the bridge, forks, installs the filter in the child, `exec` | Rust, no tokio, only `libc` and `seccompiler` | the process model is its whole content; six files |
| `humanitld` | proxy, hold queue, rules, recording, sandbox control | Rust; `unsafe` is forbidden in the library crates except one pre-exec hook in `humanitl-sandbox/src/inherit.rs`, and in the binary only in `bin/humanitld/src/systemd.rs` (removing the `LISTEN_*` variables) | memory safe, no interpreter, no runtime package tree |
| `hyper` 1, `tokio-rustls`/`rustls`, `rcgen`, with an own accept loop (`daemon/crates/proxy/src/core.rs`) | TLS termination and HTTP handling | versions locked in `daemon/Cargo.lock`, updated regularly | a Rust TLS stack without the OpenSSL class of memory bugs; the Rapid Reset and CONTINUATION flood history is followed |
| system certificate store | trust anchors for the daemon's own connections to destinations (HUM-257) | `/etc/ssl/certs`, written only by root; `webpki-roots` when it holds no usable root | the store `curl`, `apt` and the browser trust; whoever changes it is root and outside the model |
| the LLM host | sees every prompt | must belong to the user | a declared side channel, not a technical safeguard ([section 5.1](#51-the-llm-passthrough)) |
| audit key file | HMAC key of the audit chain | `$XDG_DATA_HOME/humanitl/keys/audit.key`, `0600` in `0700` | protects against other accounts, not against the same account |
| the image of the sandbox (Podman with `rootfs = "image"`, HUM-439) | the root file system the agent and its tools run from | pinned by digest (`name@sha256:...`) and present in the local storage of the engine; checked before every start (`SANDBOX_043`), never pulled by a start | the person chose it by its digest; what it holds runs as the agent, under the agent's filter, and never holds the CA, a key or the session directory, which come from the host ([section 13](#an-image-as-the-root-hum-439)) |
| system keyring (Secret Service, on macOS the login Keychain) | the LLM access key (HUM-140, HUM-349) | `secret-tool` from `libsecret-tools`, on macOS `/usr/bin/security`; without an answer `$XDG_DATA_HOME/humanitl/keys/llm.key`, `0600` in `0700` | protects against other accounts, not against the same account; the agent reads the key inside the sandbox because it needs it |

**Not part of the isolation TCB: the Flutter application.** It is a gRPC client like the CLI
and cannot weaken the sandbox. It is trusted for the *decision*, though: it shows the human
what they decide about. An application that renders a body wrongly leads to a wrong approval,
which is why bodies are rendered as text and never in a web view.

**Not part of the TCB either: the project profile** `<project>/.humanitl/profile.toml`. It
lives in the cloned repository. A key that would lift a protection of the user is refused there
with `CONFIG_003`, and the daemon does not start. This covers `hold.hard_block_checksum_secrets`
(HUM-208) and `limits.recorder_max_body_bytes` (HUM-209): before, a project profile could switch
off the hard block for IBANs and card numbers, or shrink the recording of every body to 1 KiB
while an approval sent the whole body. The full list is the column "Projekt" in
[`CONFIG.md`](CONFIG.md). Under `llm` a project profile may set exactly one key, `llm.server`
(HUM-400): it chooses one of the model servers the user saved and can never define an endpoint,
a model or a key (section 5.1).

Toolchain for reproducible builds: Rust 1.95.0 (pinned in `daemon/rust-toolchain.toml`,
minimum 1.89), Flutter 3.47.2 (pinned in `app/.fvmrc`), `bubblewrap` 0.8 or newer. `socat` is
**not** needed: the bridge lives in the shim, so that no further program runs in the sandbox.

## 2. Guarantee 1: No network interface

**Mechanism.** The sandbox is started with `bwrap` and the namespaces of `--unshare-all`
spelled out in the profile (`unshare = ["user", "pid", "net", "ipc", "uts", "cgroup"]`; a
profile that leaves one out is refused). Bubblewrap is a small, widely used program that
creates new *namespaces* without root: separate views of the kernel on network, processes,
mounts, IPC and hostname. In the new network namespace exactly one device exists, the loopback
interface `lo`, which bwrap brings up so that the bridge to the proxy can listen on
`127.0.0.1`. There is no outside address, no route, no DNS server, no ICMP, no QUIC, no raw
socket. bwrap runs with `--cap-drop ALL` and `--disable-userns` (`bwrap_args.rs`): the agent
holds no capabilities and cannot create a further user namespace in which it would get them back,
so nobody can add a device: `CAP_NET_ADMIN` is missing.

What that means in the kernel: a network namespace is a complete, separate network stack. A
socket created inside it can only reach interfaces of that namespace. With only `lo` and an
empty routing table, `connect()` to any address that is not loopback ends with
`ENETUNREACH`. An abstract Unix socket needs no path in the filesystem, but abstract sockets are
bound to the network namespace, and that namespace is empty.

**Check (inside the sandbox).**

```sh
ip link                 # only "lo"; without iproute2: cat /proc/net/dev
ip route                # empty
cat /proc/net/route     # only the header line
capsh --print           # Current: = (empty)
```

**Automated.** `tests/escape/esc-1-sockets.sh` (ESC-1) and `tests/escape/esc-3-egress.sh`
(ESC-3). At run time the shim reports the same as `IsolationCheck::NoNetworkInterface`
(`CHECK no_interfaces`); if it fails, the host stops the sandbox, reports `SANDBOX_014` and
never treats it as running. That is **not** "the agent does not start": see
[the window between `exec` and `SIGKILL`](#the-window-between-exec-and-sigkill).

**What an attacker would try.** Create a `veth` pair or a TUN device: fails without
`CAP_NET_ADMIN`. Send a UDP packet to a DNS server: fails at the filter already, at the latest
at the missing route. Use a descriptor inherited from the host: the daemon passes only the
descriptors in `LaunchPlan::fds`, and the shim additionally closes everything a launcher handed
down without `CLOEXEC` before it forks.

## 3. Guarantee 2: Exactly one door

**Mechanism.** The daemon creates its proxy socket on the host at
`$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock`, in a directory of its own with mode `0700`. The
**file** is bound into the sandbox at `/run/humanitl/proxy.sock`, never the directory. The
difference matters: a bound directory would show every file created there later, and the agent
could create sockets in it itself. The daemon's gRPC socket
(`$XDG_RUNTIME_DIR/humanitl/daemon.sock`, mode `0600`) does not exist in the sandbox; only the
application and the CLI on the host steer the daemon through it.

Inside the sandbox the shim listens on `127.0.0.1:3128` and forwards every accepted TCP
connection to that one socket file. The agent's environment points `HTTP_PROXY`, `HTTPS_PROXY`
and `ALL_PROXY` there and sets `NO_PROXY` empty; `NODE_USE_ENV_PROXY=1` makes Node's built-in
`fetch` and the global agents of `node:http` and `node:https` follow the same variables. The profile allows exactly one bridge: a second
one would be a second listener to a second Unix socket and is refused with `CONFIG_003`, a
bridge in the direction `out` with `SANDBOX_007`.

Everything else the agent needs comes from an allowlist whose complete form is the bwrap
argument list on Linux and the `sandbox-exec` line with its generated `sandbox.sb` on macOS
(`humanitl sandbox argv` prints the one that runs). In short, on Linux, from
`profiles/sandbox/default.toml`:

- read-only binds of `/usr`, `/etc/ssl`, `/etc/alternatives`, `/etc/ld.so.cache`,
  `/etc/localtime`, and the symlinks `/lib`, `/lib64`, `/bin`, `/sbin` into `/usr`;
- the Humanitl CA certificate as `/etc/humanitl/ca.crt`, generated minimal `/etc/passwd`,
  `/etc/group` and `/etc/hosts` (from the daemon's memory, not from the host), and the shim at
  `/run/humanitl/humanitl-shim`;
- created fresh: an own `/proc`, a minimal `/dev`, `tmpfs` on `/tmp`, `/var/tmp`, `/dev/shm` and
  an empty `/home/agent`;
- the project at `/work`, with the masks of [section 5.2](#52-the-project-directory-work);
- the proxy socket file;
- the agent program, when it lies outside these binds (HUM-278): one native executable, bound
  read-only at `/run/humanitl/agent/<name>`, see below.

There is no `/etc/resolv.conf`. What is **not** bound stands in the code as a denylist the
argument builder enforces: `$XDG_RUNTIME_DIR`, `/run`, the host's `/tmp`, `/home`, `~/.ssh`,
`~/.gitconfig`, `~/.netrc`, X11, Wayland, D-Bus and Docker sockets. A profile naming one of them
is refused with `SANDBOX_006`. The shim and the proxy socket live under `/run/humanitl`, the one
directory that never comes from the host.

**The agent program (HUM-278).** Agents install per user (`~/.opencode/bin`, `~/.local/bin`,
`~/bin`), which the sandbox does not see. When the command is not startable inside, the daemon
searches its `PATH` and these directories and binds the file it found read-only at
`/run/humanitl/agent/<name>`; the command then names that path. Only the configured agent
(`agent.command` or the adapter's default) is bound this way, never a command given after `--`.
This bind is checked by its own rules, not by the `SANDBOX_006` mount denylist, which would
refuse everything under `/home`: what is bound is one regular file, never its directory, and only
when it is a native ELF executable of type `EXEC` or `DYN` (a script would need files beside
it), owned by the user or root, and neither it nor its directory is writable by anyone else.
Group write counts only for the user's private group: named like the user, no listed members,
and no other account with it as primary group; where that cannot be shown, group write is
refused. Like the `mounts.extra_ro` sources (HUM-263), the file is opened once with
`O_PATH | O_NOFOLLOW`, checked through that descriptor (owner, mode, ELF header), and bound by
the descriptor (`--ro-bind-fd`, or `/proc/self/fd/N` below bubblewrap 0.10), so a swap of the path
after the check does not reach the sandbox. The agent is untrusted anyway; it receives read access
to the code it already runs, nothing more. An absolute `agent.command` follows the same rules.

**A session with the caller's command (HUM-280).** `humanitl run -- bash` gets the same adapter
files and environment as a daemon-started agent, and the configured agent is bound by exactly the
rules above, beside the caller's command; `/run/humanitl/agent` then stands first on the
sandbox's `PATH`, so the shell finds it by name. The caller's own command is still never bound:
it stays as written and must be startable from the profile's mounts. Nothing new reaches the
sandbox this way. The bind is the same one file a daemon-started agent would get, the key file is
the one that agent would read, and the directory on `PATH` holds that one bind and nothing else.
`PATH` gets that directory exactly once and first, so a same-named program in a directory such as
`/work/bin` cannot stand in front of the bind. When the key store does not give out the key
(`LLM_009`), a session with the caller's command starts without it and reports `LLM_009` as a
warning; no key file is written then. A session without a command still refuses to start.

**Bound is what was checked (HUM-263).** The project directory and every entry of
`mounts.extra_ro` and `mounts.extra_rw` are opened exactly once while planning
(`O_PATH | O_NOFOLLOW`, the project directory also with `O_DIRECTORY`). The mount policy then
checks the location `/proc/self/fd/N` names for the opened descriptor, and bwrap binds exactly
that descriptor: from bubblewrap 0.10 with `--bind-fd` and `--ro-bind-fd`, below that through
`/proc/self/fd/N` with an inherited descriptor. Before, the launcher checked the path and bwrap
resolved it a second time when mounting; whoever could write to a parent directory could swap
the directory for a symlink to `~/.ssh` in between. A swap after opening no longer reaches the
sandbox; a swap between resolving and opening fails the comparison and stops the start with
`SANDBOX_006`. A profile source that is missing at planning time is refused the same way.

The descriptor itself must never reach the agent: `openat(fd, "..")` from a host directory
descriptor would walk up the host's tree past every mount namespace. With `--bind-fd`, bwrap
closes it. In the `/proc/self/fd/N` fallback the shim closes it before it starts the agent,
together with everything else it inherited without `CLOEXEC`; the escape harness launcher
(`escape-launch`) therefore refuses to start without a usable shim when bwrap is older than
0.10. In the daemon the descriptor carries `FD_CLOEXEC` all the time; only the bwrap child
loses it, between `fork` and `exec`. The preview (`humanitl sandbox argv`) has no descriptors
and keeps showing paths.

**The mount table equals the plan (HUM-437).** The socket walk sees a second door, not a second
mount: a directory of secrets a configuration source adds to the sandbox holds no socket. Every
backend therefore hands the shim, as a sealed memfd behind `--pins-fd`, the mount table the plan
expects and the device and inode of every checked target. Before anything else of the start the
shim compares `/proc/self/mountinfo` with that table as a multiset: entry by entry with mount
point, file system type, source, read-only or writable, `nosuid`, `nodev` and `noexec`. The source
of a bind is the device and inode of what was checked, read with `lstat` on the mount point and
counted only for the topmost mount there; the source of every other mount is its type. The table
holds every bind, tmpfs and mask of the plan and the backend's own entries (bwrap's root, `/proc`
and `/dev`; Podman's, see [section 13](#13-linux-podman)). An extra entry, a missing one, a
different one, or a second mount stacked onto a planned point is `CHECK mounts_match fail`, and the
start ends with `SANDBOX_049` before the agent exists. Then `lstat` of `/work`, every entry of
`mounts.ro`, `mounts.extra_ro`, `mounts.extra_rw` and the agent program must give the device and
inode the launcher saw when it opened the source; a different pair is `CHECK mounts_pinned fail`
and `SANDBOX_042` with the path, the expected and the found pair. Both lines belong to the second
guarantee: exactly one door also means that nothing is mounted outside the plan. A shim on Linux
that writes neither line leaves the guarantee unproven. A host source with mounts of its own
beneath it fails this check too: bwrap and Podman bind recursively, and what comes along is not in
the plan. Masks and tmpfs under `/work` are planned, and mounted, only on paths that exist in the
project.

**Check (inside the sandbox).**

```sh
find / \( -path /proc -o -path /sys -o \( -path '/dev/*' ! -path /dev/shm ! -path '/dev/shm/*' \) \) -prune -o -xtype s -print 2>/dev/null
                                                    # exactly one line: /run/humanitl/proxy.sock
cat /proc/self/mountinfo                            # no /run/user, no X11, no docker.sock
hostname                                            # "sandbox"
```

**Automated.** `tests/escape/esc-2-mounts.sh` (ESC-2). At run time `IsolationCheck::SingleSocket`,
whose evidence comes from inside the sandbox: before `exec` the shim walks the sandbox
filesystem for Unix sockets and reports every one it finds (`CHECK single_socket`), next to
`CHECK bridge_listening`, which shows that the one door is open and answers. The walk has a
budget (without `/proc`, `/sys` and `/dev` except `/dev/shm`, no symlinks followed, depth 3,
2000 entries; if a limit hit, the evidence says so). ESC-2 runs the unbounded `find` command
above (`esc_find_sockets` in `tests/escape/lib.sh`). `-xtype s`, not `-type s`: bwrap binds the proxy socket over an empty regular file, and `find` trusts the directory entry for a bare `-type`; and no `-xdev`, because `/work`, `/tmp`, `/dev/shm` and `/home/agent` are separate mounts where a planted socket must show up.
Two cases of ESC-2 run on the host against each backend (`tests/escape/mount-proof.sh`,
HUM-437): the project folder swapped between the plan and the start (`esc-2/swapped_folder`: the
start is refused with `SANDBOX_042`, or, under bwrap, the sandbox still sees the checked folder),
and a mount in the line that the plan does not expect (`esc-2/unplanned_mount`: refused with
`SANDBOX_049`).

**What an attacker would try.** Look for the D-Bus or Docker socket: not there. Reach another
mount namespace through `/proc/<pid>/root`: the own PID namespace shows only sandbox
processes. Create a second socket next to the proxy socket to connect two sandbox processes
past the recording: the directory is not bound, and after the filter `AF_UNIX` is refused
anyway.

## 4. Guarantee 3: No new doors

**Mechanism.** `humanitl-shim` is a small program without a runtime (only `libc` and
`seccompiler`) that starts as PID 1 of the sandbox: the launcher calls bwrap with `--as-pid-1`,
so no init of bwrap sits above it (HUM-203). Its process model is the actual trick (HUM-012):
the bridge lives in the parent, the agent is a child that carries the filter before `exec`.
Both carry a filter; the parent's allows exactly one more family, `AF_UNIX`, because the bridge
needs it. As PID 1 the parent reaps every orphan of the sandbox (`waitpid(-1)`), counts only the
status of its own child and relays `SIGTERM` and `SIGHUP` to the agent; without a handler the
kernel would drop them at a namespace init.

The order, as `daemon/bin/humanitl-shim/src/main.rs` runs it:

1. **Start the bridge.** For every `in` bridge of the profile the parent binds the TCP listener
   on `127.0.0.1:3128` itself, without `socat`, and connects to it once (`bridge_listening`).
   The listener exists before the fork, so the agent never sees `ECONNREFUSED`, and carries
   `CLOEXEC`, so the agent does not inherit it. Then it reads the interface list
   (`no_interfaces`) and walks the filesystem for sockets (`single_socket`).
2. **Close and fork.** It closes every inherited descriptor except 0, 1, 2, the report, the exec
   gate, the bridge listeners and, when the daemon gave them, its door for further shells and its
   lifeline (HUM-430), creates the `socketpair` over which the filter's listener will
   travel (HUM-138), and forks. The parent's first step after the fork is
   `prctl(PR_SET_DUMPABLE, 0)`, checked back with `PR_GET_DUMPABLE` before the gate opens; it
   then installs its own filter, the agent's policy plus `AF_UNIX`.
3. **`PR_SET_NO_NEW_PRIVS` in the child.** The child dies with the parent
   (`PR_SET_PDEATHSIG`), closes every inherited descriptor except 0, 1, 2, the report, the gate
   and its end of the channel, and sets `PR_SET_NO_NEW_PRIVS`. From then on no later `exec` can
   gain privileges, for example through a setuid program; without the flag an unprivileged
   process may not install a seccomp filter at all.
4. **seccomp.** The child loads the filter with a listener
   (`SECCOMP_FILTER_FLAG_NEW_LISTENER`): a refused `socket()` call is handed to the parent, which
   counts it and answers `EPERM` (see [what becomes visible](#what-becomes-visible-and-what-does-not)).
   The child passes the listener to the parent and closes its own copy before the agent starts.
   This load runs without `TSYNC`, because kernels before 5.7 refuse `TSYNC` together with a
   listener, and the child has exactly one thread right after the fork. If the kernel refuses
   the listener (for example `EBUSY`, because Humanitl itself runs inside a sandbox that has
   one), the child loads the same filter without a listener and **with**
   `SECCOMP_FILTER_FLAG_TSYNC`, and the host reports `SANDBOX_019`: refused exactly the same,
   only nobody counts. Either way every thread and process the agent starts later inherits the
   filter.
5. **`exec`.** The child reports `seccomp_applied` and `families`, waits at the gate until the
   parent is non-dumpable and filtered, then calls `execvp` on the agent. The agent inherits the
   filter and cannot remove it.

The parent carries its own version of the filter because a filter covers threads, not parent
processes: the agent cannot pass its filter up, and the parent cannot carry the agent's
narrower filter because it needs `AF_UNIX`. Both filters refuse the same syscalls, so every
process in the sandbox carries a filter, PID 1 included (ESC-1 `seccomp_every_process`,
`seccomp_parent_mode_2`). Until HUM-203, PID 1 was bwrap's init: unfiltered, dumpable and under
the agent's UID. With `kernel.yama.ptrace_scope = 0` the agent could open `/proc/1/mem` for
writing and run code there without a filter (security pass of 2026-09-23, finding M1).

### Further shells in a running sandbox (HUM-299)

`humanitl sandbox exec` opens a shell next to the agent, like `docker exec`. Nobody enters the
sandbox from outside: there is no `setns`, no `nsenter`, and the daemon starts no process in the
sandbox's namespaces itself. It asks PID 1, the shim's parent, which forks the shell from
inside. The shell therefore inherits what every process of the sandbox carries:
`PR_SET_NO_NEW_PRIVS`, the parent's filter, the same mounts, namespaces, environment (without
the shim's own variables) and `/work` as its directory. Before `exec` the shell's child installs
the agent's filter on top, in the variant that answers `EPERM` itself, and proves it:
`socket(AF_UNIX, SOCK_STREAM)`, which the parent's filter still allows for the bridge, must
answer `EPERM`, or the shell never starts. Filters only stack, so the shell carries at least
everything the agent carries. It runs on a pseudo-terminal of its own that the daemon opened,
takes it as controlling terminal like the agent (`setsid`, `TIOCSCTTY` with argument 0), closes
every descriptor above 2, and resets signal mask and dispositions. Its output runs through the
same `FullScreen` filter as the agent's ([section 5.3](#53-the-agents-terminal-output)).

**The door, and who holds it.** The request travels over a `SOCK_SEQPACKET` pair the launcher
creates only for a start at the daemon's own pseudo-terminal. The daemon keeps one end
(`CLOEXEC`); bwrap inherits the other and passes it to the shim as `--control-fd`. This is a new
way into the sandbox, so its limits are part of the claim:

- Only the daemon can ask. The pair has no name in any filesystem, so there is nothing to
  connect to, and `single_socket` still finds the proxy socket alone. Inside the sandbox only
  PID 1 holds its end: the agent's child closes it before its filter, the end carries `CLOEXEC`,
  every further shell closes all descriptors above 2, and PID 1 is not dumpable, so neither
  `/proc/1/fd` nor `pidfd_getfd(2)` (which the floor refuses anyway) reaches it. On the daemon's
  side the only caller is the `Exec` RPC on the gRPC socket, which needs the daemon's token and is
  never bound into the sandbox.
- A shell id is `<session>-<n>`: the number the shim counts from 1 per sandbox, with eight hex
  characters of the sandbox id in front, so an id from an earlier session reaches nothing.
- A request carries a command and exactly one descriptor, the follower side of the shell's
  pseudo-terminal; anything else is refused. What comes back are nine-byte answers (started,
  refused, ended) for ids the daemon asked for; the daemon reads them without a control buffer,
  so a descriptor sent the other way is closed by the kernel, never accepted.
- At most four further shells live at a time; the shim counts, and the daemon counts again
  (`TERM_002` for the fifth).
- A shell ends when its process exits, and with the sandbox: PID 1 kills every shell when the
  agent ends or the daemon closes its end of the pair, the shells die with PID 1, whose main
  loop forked them (`PR_SET_PDEATHSIG`, HUM-430), and with the PID namespace, and the daemon
  ends their terminals when its end of the pair closes.
- A sandbox started without the daemon's pseudo-terminal (`humanitl sandbox run`, the escape
  suites) has no door.

*What it costs.* A refused `socket()` call in a further shell is refused like the agent's but
not counted: the listener of the reported gate belongs to the agent's filter, and the filters
refuse a second one. The shell also starts with the parent's filter underneath, which allows
`AF_UNIX`; the agent's filter on top refuses it, and the shell does not start unless that
refusal is proven.

*Not confidential against the agent.* A further shell runs with the agent's UID in the same PID
namespace, and the sandbox's `/proc` is not mounted with `hidepid`. The agent can therefore see
the shell's processes, read what the shell's programs read from their terminal (the keystrokes
typed there) and write output into that terminal, which reaches the human through the same
filter as the agent's own output. The shell guards the host against the agent, not the human's
input against the agent: never type a secret into it. The default shell starts without start
files (`bash --norc --noprofile -i`, or `sh -i` with `ENV` unset), so a `~/.bashrc` or
`~/.profile` the agent planted in the sandbox's home does not run when it opens; a command the
caller names after `--` decides that for itself.

**The stream mode (HUM-443).** In a micro-VM no descriptor crosses the boundary, so the kernel
level carries the terminals another way, and under bwrap a caller can ask for the same
(`TerminalTransport::Stream`), so that both modes face the same terminal tests. The shim then opens
the agent's pseudo-terminal and every further shell's itself, and the daemon talks to them over one
stream socket in frames (`daemon/bin/humanitl-shim/src/frame.rs`, compiled by both ends): terminal
bytes per id, sizes, open and close of a shell, exits, interrupts and heartbeats. Under bwrap the
socket is an unnamed pair the launcher creates and passes as `--control-stream-fd`; in a guest the
host opens it with `CONNECT` at the vsock device as the second connection on the shim's one
listening port, after the report, and the guest never opens one. The claims of the door hold
unchanged, and these are added:

- The stream carries terminal bytes and control, never a byte to or from the proxy; the bridge
  stays the only way out.
- Only PID 1 holds the socket and the masters, all with `CLOEXEC`; the agent's child closes them
  before its filter, every further shell closes every descriptor above 2. The agent's filter,
  and the one every further shell installs and proves before `exec`, refuses `AF_VSOCK` and
  `AF_UNIX` with `EPERM`; ESC-1 checks it in the stream mode.
- Every frame has a nine-byte header (kind, id, length) and at most 16 KiB of payload. Both ends
  count a longer frame away without keeping it, skip unknown kinds, drop frames of the wrong shape
  or direction and answers for ids nobody opened; none of it ends the stream, and no reader holds
  more than one frame (fuzz target `control_stream_reader`).
- The end of the stream ends the sandbox like the lifeline. PID 1 never writes to a terminal
  and signals before any message, so a full terminal cannot hold up the end; a frame the shim
  does not take within ten seconds makes the daemon shut the stream. Interrupts come from the
  shim (`SIGINT` to the foreground group of the terminal), because no signal of the host reaches
  a guest.
- No channel holds up another: input for a terminal whose queue is full (64 KiB) is dropped,
  output of a further shell whose reader stalls is dropped beyond 256 pieces, both counted.

### The lifeline and the environment through descriptors (HUM-430)

Two things bwrap does with flags need a descriptor once a backend starts the sandbox detached from
the daemon and stores every value of its command line on disk (Podman, HUM-292 measurements P7
and P11). The shim takes both, the same way under every backend; bwrap itself keeps its flags.

- **The lifeline** (`--lifeline-fd`). The read end of a pipe whose only write end the daemon
  holds, with `CLOEXEC`, so no child of the daemon inherits it. The shim, PID 1, waits on it in
  the same `poll` loop as on its children and the door for further shells. When the daemon ends,
  also through `SIGKILL`, the kernel closes the write end, the read end reports `POLLHUP`, and
  the shim ends the sandbox as on `SIGTERM`: `SIGTERM` to the agent, after `KILL_GRACE` (5 s)
  `SIGKILL` to the agent, every further shell and every process of the PID namespace. It takes
  the place of `--die-with-parent`. The read end may be open in other processes too (the
  `podman run` client inherits it); what counts is only that the write end lies in the daemon
  alone. A number that names no pipe refuses the start.
- **The environment** (`--env-fd`). The agent's environment as one JSON object, `String` to
  `String`, in a memfd sealed against writing, growing and shrinking. The shim reads it before
  anything else of the start, makes it the agent's whole environment (what the engine put there
  on its own does not pass) and closes it. No value stands in an argument vector, in the process
  list or in the engine's stored configuration. The rules of `--setenv` hold: the five variables
  of the launcher contract and the loader variables `LD_PRELOAD`, `LD_AUDIT` and
  `LD_LIBRARY_PATH` are refused, as is a descriptor that is not fully sealed; a
  refusal is a failed `seccomp_applied` line and exit 126, before the agent exists. The
  memfd must be sealed against writing, shrinking and growing; one sealed against writing alone
  could still be emptied with `ftruncate(2)`. The five variables of the launcher contract
  (`HUMANITL_BRIDGES`, `HUMANITL_REPORT_FD`, `HUMANITL_SECCOMP_FAMILIES`,
  `HUMANITL_SECCOMP_TYPES`, `HUMANITL_SECCOMP_DENY`) still reach the shim through its own
  process environment, under a container engine through its command line. That is fine: they
  hold a socket path inside the sandbox, a descriptor number and the names of socket families,
  types and syscalls, all of which this document states openly, and no secret; and the shim
  needs the report descriptor before it reads the memfd, so that a broken memfd becomes a
  failed check line.

**The agent inherits neither.** The shim reads and closes the environment's memfd before it
forks. The lifeline stays with PID 1 alone, like the door: the parent's close before the fork
keeps it with `CLOEXEC`, the agent's child closes it explicitly and does not keep it in its own
close, and every further shell closes all descriptors above 2. PID 1 is not dumpable, so
`/proc/1/fd` does not reach it either. The agent holds nothing of the lifeline and cannot read
the memfd again.

### What the filter allows and refuses

`socket()` passes only for a family in `allow_families` (`AF_INET`, `AF_INET6`) with a type in
`allow_types` (`SOCK_STREAM`). Both lists are not a setting but a floor in both directions: a
profile naming `AF_UNIX` or `SOCK_DGRAM` is refused with `CONFIG_003`, and the launcher passes
exactly this floor to the shim. The one planned exception is the later `browser` profile (M7),
which needs `AF_UNIX` for Chromium IPC; it is `SocketFloor::BrowserUnixIpc` in the launcher's
code and cannot be set from any profile file, because such a file can come from a cloned
repository. The type argument is masked with `0xff`, so `SOCK_NONBLOCK` and `SOCK_CLOEXEC` pass.
Everything else, `AF_UNIX`, `AF_NETLINK`, `AF_PACKET`, `AF_VSOCK`, `SOCK_DGRAM`, `SOCK_RAW`,
gets `EPERM`.

`socketpair()` has no rule and is allowed: it knows only `AF_UNIX`, connects two descriptors of
the same process tree and is no egress (Node and Bun need it for child-process IPC,
`CONVENTIONS.md` 4.11; `filter_allows_socketpair` in `seccomp.rs` and ESC-1 prove it).

The program the kernel runs has two parts. A hand-written prelude comes first:

| Subject | Condition | Verdict |
|---|---|---|
| every syscall | architecture is not the one the shim was built for | kill the process |
| every syscall | number has the x32 bit `0x40000000` set | `EPERM` |
| `seccomp` | flag `SECCOMP_FILTER_FLAG_NEW_LISTENER` set | `EPERM` |
| `socket` | family not in `allow_families` | `EPERM` (agent: via the listener) |
| `socket` | type (`arg1 & 0xff`) not in `allow_types` | `EPERM` (agent: via the listener) |

Then the seccompiler program refuses every syscall of the deny list with `EPERM` and allows the
rest. The floor, `FLOOR` in `seccomp.rs`, is refused in every profile, even when a profile
forgets a name:

| Syscall | Why it is refused |
|---|---|
| `ptrace` | taking over a neighbouring process |
| `io_uring_setup`, `io_uring_enter`, `io_uring_register` | queueing syscalls past the filter |
| `process_vm_readv`, `process_vm_writev` | reading and writing another process's memory |
| `pidfd_getfd` | copying a descriptor out of another process (HUM-203) |
| `keyctl`, `add_key`, `request_key` | the kernel keyring |
| `kexec_load`, `kexec_file_load` | loading a new kernel |
| `init_module`, `finit_module`, `delete_module` | swapping kernel modules |
| `bpf` | loading BPF programs |
| `perf_event_open` | opening performance counters |
| `userfaultfd` | serving page faults in user space, the classic lever to stretch a race in the kernel |

The last eight are the standard hardening that the Docker default profile applies as well.
`pidfd_open` stays allowed, because asyncio, Go and libuv probe it, and without `pidfd_getfd` a
process descriptor opens nothing a `kill` cannot reach anyway.

A profile may refuse more, but only names from `SYSCALLS` in `seccomp.rs`; an unknown name ends
the shim with exit 126 rather than being dropped silently. Beyond the floor these are:
`mount`, `umount2`, `pivot_root`, `chroot`, `setns`, `unshare`, `reboot`, `swapon`, `swapoff`,
`acct`, `settimeofday`, `clock_settime`, `clock_adjtime`, `adjtimex`, `open_by_handle_at`,
`name_to_handle_at`, `personality`, `kcmp`, `quotactl`, `mbind`, `set_mempolicy`,
`get_mempolicy`, `migrate_pages`, `move_pages`, `fsopen`, `fsconfig`, `fsmount`, `fspick`,
`move_mount`, `open_tree`, `mount_setattr`, `vhangup`, `pidfd_open` and `socket` itself. The
bundled default profile denies exactly the floor.

The authoritative list is the source, not this page: `humanitl-shim --rules` prints the rule
table the current environment would produce, and the test `rule_table_lists_every_rule` prints
every row. The test `security_doc_mentions_syscalls` fails when a syscall name of `seccomp.rs`
is missing on this page.

**Check (inside the sandbox).**

```sh
grep Seccomp /proc/self/status                       # "Seccomp: 2" (filter mode)
grep NoNewPrivs /proc/self/status                    # "NoNewPrivs: 1"
cat /proc/1/comm                                     # "humanitl-shim": PID 1 is the shim parent
grep Seccomp /proc/1/status                          # "Seccomp: 2"
stat -c %u /proc/1/mem; id -u                        # different: PID 1 is not dumpable
python3 -c 'import os; os.open("/proc/1/mem", os.O_RDWR)'                 # PermissionError
python3 -c 'import socket; socket.socket(socket.AF_UNIX)'                 # PermissionError
python3 -c 'import socket; socket.socket(socket.AF_INET, socket.SOCK_DGRAM)'  # PermissionError
python3 -c 'import socket; socket.socket(socket.AF_INET, socket.SOCK_STREAM)' # works
python3 -c 'import socket; socket.create_connection(("10.0.0.1",80),2)'   # ENETUNREACH
```

**Automated.** ESC-1 covers exactly these probes, including `io_uring_setup`, an x32 syscall
and the probes on PID 1 and the shim parent (`pid1_*`, `shim_parent_*`, `pidfd_getfd_self`;
with `ptrace_scope` 1 or higher Yama already refuses `mem` and `pidfd_getfd`, those cases are
`skip` there, and `pid1_not_dumpable` carries the claim). At run time
`IsolationCheck::SeccompActive`: the shim writes check lines from the running sandbox on its own
descriptor (`HUMANITL_REPORT_FD`), the launcher reads them and turns them into the three results.
The proof comes from the sandbox the agent really runs in, not from a second one started for
testing.

**What an attacker would try.** Run a setuid program to get rid of the filter:
`NO_NEW_PRIVS` prevents it. Take over a neighbour with `ptrace`: `EPERM`. Queue syscalls past
the filter through `io_uring`: the three `io_uring_*` calls are refused. Call the same syscall
under its x32 number, a classic filter bypass: the prelude catches it. Start a thread without
the filter: there is none, threads inherit it. Write into PID 1 through `/proc/1/mem` or
`pidfd_getfd(2)` to run code without the agent's filter: PID 1 is the shim, carries the bridge
filter and is not dumpable, and `pidfd_getfd` is in the floor (ESC-1 `pid1_mem_rdwr`,
`pid1_pidfd_getfd`). Take the filter's listener to answer one's own calls with "continue"
(`SECCOMP_USER_NOTIF_FLAG_CONTINUE`): the listener lives only in the parent, which is not
dumpable, so `/proc/<pid>/fd`, `/proc/<pid>/mem` and `pidfd_getfd(2)` are refused to the agent
even with the same UID and `kernel.yama.ptrace_scope = 0`
(`the_agent_cannot_reach_the_descriptors_of_the_parent` in
`daemon/bin/humanitl-shim/tests/shim.rs`). The parent never answers anything but `EPERM`, and if
it dies, the kernel answers `ENOSYS`: the door stays shut whatever happens to the listener. Get
an own listener for a newer filter: both filters refuse `seccomp()` with
`SECCOMP_FILTER_FLAG_NEW_LISTENER`, and the parent never lets go of its listener, not even
after errors the agent provokes (`a_dropped_listener_does_not_free_the_agent_to_take_one` in
`seccomp.rs`, ESC-1 `seccomp_no_own_listener`). Hijack the parent that holds the bridge: see
K-04 in [`THREAT-MODEL.md`](THREAT-MODEL.md#k-04-tcp-sockets-allowed-by-seccomp); nothing is
gained, because it carries a filter too and the bridge knows exactly one target, the recording
proxy.

### What becomes visible and what does not

Being blocked does not mean being seen. Whatever tries to get past the one door, an `ssh` to a
bare address, a UDP packet, `git://`, a Unix socket, never reaches the proxy: it becomes no flow
and stands neither in the queue nor in the history. So that the window still shows what the
agent **tried**, the shim counts what the filter refuses (HUM-138):

- **Visible:** every refused `socket()` call of the agent and its children, counted per pair of
  family and type, with count, first and last time and which half of the gate refused (family or
  type). The shim writes the state at most every two seconds and once after the agent ends; the
  daemon passes it on as `SandboxEvent.refusals`, the window shows it in the isolation tab,
  `humanitl sandbox run` after the run. Counted, not logged: at most sixteen pairs plus one
  bucket for the rest, so an agent in a loop creates no flood. The answer costs one more context
  switch, measured at about 5 to 30 µs per refused call; allowed calls never reach the listener.
- **Not visible:** a `connect()` to an outside address. It is not a refused call (the
  `socket(AF_INET, SOCK_STREAM)` before it is allowed) but fails at the empty routing table with
  `ENETUNREACH`. Calls the kernel refuses without asking are not counted either: the deny list
  and every x32 call.
- **A report, not a guarantee.** An attacker (b) who wants to hide their attempts can load a
  further filter of their own that answers `socket()` with `EPERM` itself; the kernel takes that
  answer, and the listener hears nothing. The call is refused all the same. None of the three
  guarantees depends on a line of this report.

Automated: `esc-1/refusals_reported` (`tests/escape/refusals-proof.sh`) has a sandbox try
`socket(AF_UNIX)` three times and `socket(AF_INET, SOCK_DGRAM)` twice and requires exactly these
counts in the report.

### The window between `exec` and `SIGKILL`

The shim enforces nothing. It writes its five `CHECK` lines and starts the agent right after;
enforcement happens on the host (`CONVENTIONS.md` 4.12). The host therefore reads the report
when the agent is already running, and stops the sandbox afterwards. In between lies a window
in which the bridge stands, the proxy accepts, and an `allow` rule forwards without a human.

Normally that is milliseconds. The **upper bound** is the sum of the deadlines:

| Path | Report missing after | Stopping | Bound |
|---|---|---|---|
| daemon (`SandboxService`) | `REPORT_TIMEOUT` 5 s | `SIGKILL` at once, `KILL_GRACE` 5 s to reap | **10 s** |
| `humanitl sandbox run`, `escape-launch` | `REPORT_TIMEOUT` 5 s | `SIGTERM`, `KILL_GRACE` 5 s, then `SIGKILL`, `KILL_GRACE` 5 s | **15 s** |

No path into the sandbox is exempt, and the escape tests say nothing about this window: they
measure from inside. Closing it means the proxy refuses connections while the isolation of the
running session is not proven; that is a separate issue and not built. Until then: a red
isolation check **stops** the session, it does not **prevent** it (K-15 in
[`THREAT-MODEL.md`](THREAT-MODEL.md#k-15-the-window-between-exec-and-sigkill)).

## 5. Declared side channels

This is the most important section. A security promise that hides its exceptions is none. All
channels below are shown in the application, not hidden.

| Channel | Why it exists | What Humanitl does in 0.1 | Later |
|---|---|---|---|
| LLM passthrough | the agent has to send code to the model | a rule exactly on host, port, scheme, methods and exact inference endpoints; streamed, fully recorded, scanned for findings with a warning; amber line in the isolation panel | holding on findings, rate limits |
| `/work` (read-write) | the agent has to write files | only a subpath is bound; hooks, CI and editor directories and secret files masked; session summary with touched files, outward symlinks and a secret scan over the diff | git operations from the window on the host side |
| terminal output | the human reads what the agent writes | the daemon filters every byte stream before a client sees it (allowlist); banner "agent output is not trustworthy" | |
| host names in the log | blocked requests stay visible | documented; the export carries hosts, paths, headers and bodies in plain text and says so before writing | optional host redaction |
| package caches | repeated installs | caches per project, never shared across trust levels | read-only seed caches |

### 5.1 The LLM passthrough

*Why it exists.* The agent has to send code to the model, otherwise it does not work. If every
inference were held, a human would have to confirm several times a minute; nobody keeps that up,
and after two hours they click everything away blindly.

*What Humanitl does.* The passthrough is a rule, not a hole in the filter: exactly one host, one
port, one scheme, the methods `POST` and `GET` (the model list is a `GET`) and a list of
endpoints. An entry names an endpoint and never a whole API surface. The default of
`llm.passthrough_paths` is `["/v1/", "/api/"]`, and Humanitl does not pass those two through as
prefixes: both would cover calls that change the server (`POST /api/pull`, `POST /api/create`,
`DELETE /api/delete`, `POST /v1/files`, `/v1/uploads`, `/v1/vector_stores`,
`/v1/fine_tuning/jobs`, vLLM's `POST /v1/load_lora_adapter`). It expands them into the thirteen
endpoints that run inference or give information: `/v1/chat/completions`, `/v1/completions`,
`/v1/responses`, `/v1/embeddings`, `/v1/models`, and `/api/chat`, `/api/generate`, `/api/embed`,
`/api/embeddings`, `/api/tags`, `/api/show`, `/api/ps`, `/api/version`. If `llm.endpoint` has a
path prefix (`https://host/api/v1/`), the five OpenAI-compatible endpoints sit under that base
and the eight Ollama endpoints are dropped, because the native Ollama API exists only at the
root of a server (HUM-261).

Each of these generated endpoints matches exactly, not as a prefix: `/v1/models` matches
`/v1/models` and `/v1/models?x=1`, but not `/v1/models/delete`, `/v1/models/` or
`/v1/models%2Fdelete`; nothing is decoded, every other spelling is held. A path the human
writes into `llm.passthrough_paths` themselves stays a prefix and covers everything below it. In
a rule an exact entry ends with `?` (`/v1/models?`). A path with a `..` segment never matches a
prefix, because only the server resolves it; the same holds for every rule that lets a request
through (`allow`, `redact`) and carries a path pattern or prefixes, also when disguised as
`%2e%2e` or `..;` (HUM-204). Every other request to the same host is held normally.

The traffic is fully recorded and runs through the findings detectors; a hit produces a warning
(`LLM_005`, with host and counts and never with the value found) but does not hold. Every
finding stays recorded with its request and visible in the history. Agents resend the whole
conversation with every turn, so the warning is raised only for a request that carries at least
one finding not warned about at this endpoint (`host:port`) in this run, or last warned about 15
minutes or more ago; it says how many are new and how many the request carries in total
(HUM-354). The daemon remembers what it warned about as detector kind plus the value hash the
finding already carries, in memory only and capped per endpoint. The memory is cleared whenever
a sandbox run starts, an entry expires 15 minutes after its warning, and a fingerprint that
expired or fell out of the cap warns again. A warning that reached no open window is therefore
repeated by the next request carrying the same finding once the entry has expired or a new run
has started; without such a request no card follows, and the finding stays visible only in the
history of its request. One exception: with `hold.hard_block_checksum_secrets` on, the passthrough blocks a request
carrying a checksum-confirmed secret (API key, JWT, IBAN, credit card) as the system with
`HOLD_004`, instead of letting it pass (HUM-159). Because the model usually sits in the local
network, the rule carries `allow_private: true`; it is the only rule that deliberately points
into private address ranges. The right is bound to this one rule and this one request. In the
isolation panel the channel is a fourth, amber line with the concrete endpoint.

*What stays open.* If `llm.endpoint` holds a name instead of an address, the resolver decides
where the passthrough leads, and it decides anew for every request. Whoever controls that one
name (a DHCP-announced DNS in a foreign Wi-Fi, a compromised router) can redirect the passthrough
between two requests to another machine, and because the rule carries `allow_private`, the router
itself or `169.254.169.254` are valid targets. The remedy is an address instead of a name, or a
fixed entry under `resolver.overrides` that binds the name to exactly one address before anyone
is asked.

*What the user should do.* Enter only a machine they control. No shared Ollama server without
authentication in a company network. Prefer the IP address, or pin the name under
`resolver.overrides`. Whoever has no own machine switches the passthrough off and lets inference
be held as well: inconvenient, but honest. Look at line 4 of the isolation panel once per start.

#### The reserved name `llm.humanitl.internal` (HUM-387)

*Why it exists.* OpenCode 2.2.5 sends its model requests to a loopback `baseURL`
(`http://127.0.0.1:8080/v1`) directly, past `HTTP_PROXY`, even with an empty `NO_PROXY`. On macOS
the Seatbelt profile refuses that connection (`deny network-outbound`), on Linux `127.0.0.1` of
the sandbox has nothing behind it, and the agent cannot reach a model on the user's own machine.
A name goes through the proxy. pi tunnels a loopback address through the proxy itself and keeps it.

*What Humanitl does.* When the host of `llm.endpoint` is a loopback address (`127.0.0.0/8`,
`::1`, or `localhost` itself), the OpenCode adapter writes the reserved name
`llm.humanitl.internal` into the `baseURL` instead, with the same scheme, port and path. The
proxy maps exactly that name, with exactly that scheme and port, to the configured endpoint, and
does so before body, detectors and rules. The name is never resolved and never handed to a
resolver or upstream as a name. A request to it with another port or scheme, or in a session whose
`llm.endpoint` is not on the loopback (or is not set), is refused at once with `PROXY_003` and
`no_route`: it is never held, no human can approve it, and nothing is connected. The name
therefore reaches the one model endpoint the user configured and no other service of the host;
the sandbox profile and its one door are unchanged.

Because the mapping comes before the rules, the request then runs under the real target: the LLM
passthrough of section 5.1 decides it as it decides a request to `127.0.0.1:8080`, with
`allow_private` from that one rule only, the access key is no finding at its own target, and a
`block` rule of the user for the target applies exactly as it does for the real address; for
the passthrough endpoints the passthrough still ranks first. The recording and
`humanitl flows list` show the real target (`127.0.0.1:8080`), which is where the bytes went;
the `Host` header the agent sent (`llm.humanitl.internal:8080`) is recorded unchanged, and the
upstream gets the `Host` of the real target. A refused request went nowhere and is recorded under
the name itself.

#### The access key of the model (HUM-140)

*Where it lives.* In the system keyring through the Secret Service (`gnome-keyring`, `kwallet`),
addressed with `secret-tool` from `libsecret-tools`, under the attributes `service=humanitl` and
`account=llm`. The key travels in through `stdin` and out through `stdout`, never on
`secret-tool`'s command line. If the Secret Service does not answer (no `secret-tool`, no session
bus, an error, or 20 seconds without an answer), it lives in
`$XDG_DATA_HOME/humanitl/keys/llm.key` with `0600` in a `0700` directory; that reports
`CONFIG_019`, and the form says which of the two applies. The file is read only when it belongs
to this account, is no symlink and is readable by nobody else (otherwise `LLM_009`).
`config.toml` never holds the key, only its reference `llm.api_key_ref`
(`secret-service:humanitl/llm`, `keychain:humanitl/llm` or `file:llm`). It is set through the RPC
`LlmKey`, from the form or with `humanitl llm key set`, which reads it from `stdin`; no answer of the daemon carries it
back.

*On macOS (HUM-349).* The keyring is the login Keychain, addressed with the system tool
`/usr/bin/security`, with the same service `humanitl` and account `llm`, the reference
`keychain:humanitl/llm` and the same 20 seconds. There is no client library: the app is only
ad-hoc signed, so an item whose access list trusted our binary would ask after every update.
An item created by `security` trusts that Apple-signed tool, and only it; no `-A`, no `-T`. The
key goes to `security -i` over `stdin`, never on its command line, and comes back over `stdout`.
Because the exit code of `security -i` proves nothing (a line it cannot process can end with 0),
the daemon reads the item back and compares it before it calls the key stored. A key `security`
cannot carry unchanged (characters outside printable ASCII, or a command over 4000 bytes) never
reaches the tool. If the Keychain does not take the key, for example because it is locked in a
session without a screen, the key lives in the `0600` file as on Linux and `CONFIG_019` says so,
with the reason. A key already in that file keeps working through its `file:` reference; setting
a key again moves it to the Keychain and removes the file. An item of the Keychain is exactly as
open to the account as the Secret Service: **every process of this account that runs outside the
sandbox can read it with `security find-generic-password -s humanitl -a llm -w`**, without a
prompt, because the access list trusts that tool and not a caller. Against the agent the Keychain
holds as the Secret Service does: the Seatbelt profile reaches no `securityd` (Mach services are
an allow list without it), and `~/Library/Keychains` lies in `/Users`, which is not readable.
`/usr/bin/security` itself may be started inside the sandbox; it finds nothing there. The agent
gets the key only as the sandbox file of the session.

*Several saved servers (HUM-289).* `config.toml` may hold several model servers under
`[llm.servers.<name>]`, each with its own endpoint and its own key reference; a new server's key
lives under `account=<name>` (`secret-service:humanitl/<name>`), the server taken over from an
older configuration keeps `account=llm`, and no two servers share an item. `llm.active` names the
one that applies, and the flat keys `llm.endpoint` and `llm.api_key_ref` are its view: only the
active server's origin gets the stored key, at the start of a session and in the probe, and only it
is the passthrough of the next session. A probe that names a saved server (`ProbeLlmRequest.server`)
sends that server's key, and only when the probed address has that server's origin (scheme, host,
port); the key of one server never goes to the origin of another. One rule holds at load and again
at every use (the agent's key at the start, the probe without a server): a key reference is only
effective together with the endpoint of the same entry. If the environment or the command line sets
`llm.endpoint` or `llm.api_key_ref` above the active server, or, without saved servers, the two flat
keys come from different layers, the stored key is withheld (`CONFIG_021`). If saved servers exist
and `llm.active` names none of them, endpoint, models and key are all unset (`CONFIG_021`). Two
servers that share one reference get the key for neither, and no key is ever written to a shared
reference. A key is attached to a server only if the entry is unchanged (same name, same endpoint)
after the key was stored; otherwise the stored item is deleted again. A switch applies to the next
session; a running session keeps the server, the key and the passthrough it started with, and the
active server cannot be removed while a session runs or starts; no switch and no removal that deletes a key happens while a start is resolving its configuration, and endpoint or key reference of a server never change while its key is being stored (one lock for the daemon and the command line). Removing a server deletes its
keyring item on request, and only when no other server refers to it.

*Editing a server (HUM-405).* The address and models of every saved server can be changed without
making it the default (`LlmServers(Edit)`). A stored key belongs to the origin (scheme, host, port)
it was stored for: when an edit gives the endpoint another origin, the daemon drops the server's key
reference and deletes the keyring item unless another server refers to it, and the settings say so
before Save; the key has to be entered again for the new address. A path on the same origin keeps
the key. A stored key therefore never moves to a new origin, and never to the origin of another
saved server, without being entered again.

*A server per project, profile or run (HUM-400).* The key `llm.server` names one of the saved
servers for a session, and it beats `llm.active`: `humanitl run --llm NAME` (or the start field,
`Start.llm_server`) beats the project profile, the project profile beats a session profile, and a
session profile beats `llm.active`. **The trust boundary of the passthrough is therefore the saved
server the session resolved to**, no longer "the active server": only that server's endpoint is
the session's passthrough, only its origin gets its key, and the mask of the recording and
`http://humanitl.internal/` name that server. The check of the key at the start compares with the
session's choice, never with the globally active server, and the daemon builds the passthrough,
the key file and the mask from the configuration resolved for that start (one session per daemon:
the next start replaces them). `humanitl sandbox run`, which starts the sandbox in its own process
and cannot set the daemon's session, refuses to start whenever the passthrough its own configuration
would get (built with the daemon's function and compared as scheme, host, port, methods and exact
inference paths) is not the one the daemon holds, for every choice of a server and for an endpoint
without one (`CONFIG_021`, with `humanitl run` as the way), so an agent never
talks to one server while the proxy lets another through. `llm.server` only chooses: endpoint, models and key always come from the entry
the user saved under `[llm.servers.<name>]`, every other key under `llm` stays closed to a project
profile, and a name without a saved entry applies no endpoint and no key at all. Such a name never
falls back to `llm.active`: the start is refused with `CONFIG_021`, which names the place that chose
it, the saved servers and the fix (save the server or remove the key). An endpoint from the
environment or the command line above the choice still wins, and then the stored key is withheld
as before.

**A deliberate widening.** A project file can now choose among the user's saved servers. A
hostile repository can thereby route the model traffic of its own session (prompts, the code the
agent reads) to another of the user's saved servers than the active one, for example to a company
server instead of a local one. It cannot add a server, change an endpoint, read a key or send one
server's key to another server's origin. The choice is limited to servers the user saved on
purpose, each with its own key, and it is visible before and at every start. The sandbox header of
the app shows the server with its source (the badge turns to the colour of a hold when the project
chose), the hover names the project file, `humanitl sandbox plan` and `humanitl run` print the
server and where the choice comes from, and `SandboxEvent.Status.llm_server` carries both to every
client.

*Confirmed once per project folder (HUM-405).* HUM-400 left open whether a project's first choice
needs a question; HUM-405 answers it with yes. The project profile lies in the folder the agent
writes, so a running agent can change `llm.server` there for the next session. **When the server a
project chooses is not the default (`llm.active`) and the human has not confirmed this server for
this project folder, the session does not start** (`CONFIG_022`, checked by the daemon in the
resolution of every start, and by `humanitl sandbox run`); no passthrough is set up and no key is
fetched. The app asks once before the start, with Cancel preselected; the command line names
`humanitl llm server confirm NAME --project DIR`. The confirmation is stored per resolved project
folder and server name in `$XDG_DATA_HOME/humanitl/project-servers.toml`, in the daemon's data
directory and never in the project, so the agent cannot issue it itself. A choice of the default
needs no confirmation: the traffic then goes where it would go without the project; without saved
entries the server `default` derived from the flat values counts as the default. A choice the
project makes indirectly counts as the project's: when the project profile names a bundled profile
with `name` and that profile (or the user's own copy of it, which shadows it) carries `llm.server`,
the start asks the same question, also when the human names the same profile at the same time. A
confirmation covers one server under one name: a project that switches to another non-default
server asks again, and removing, renaming or re-adding a server, renaming another server to that
name, or giving it an endpoint of another origin drops every confirmation of that name, before the
change is written. Assigning
a server to a project through the daemon (`LlmServers(Assign)`, the settings of the app or
`humanitl llm server assign`) is the human's choice and counts as the confirmation for that folder.
The daemon writes the project file relative to the opened project folder with `O_NOFOLLOW`, as a
new file renamed into place within the same opened `.humanitl` folder: a link the agent put at
`.humanitl` or `.humanitl/profile.toml` refuses the write (`CONFIG_015`) and its target is never
touched. Whoever keeps a server that no repository should reach should still not save it: a
confirmation is a decision about a folder, and a later change of the repository's content under
the same folder and name is not asked again.

*How it reaches the sandbox.* When a session starts, the daemon fetches the key and places it as
the file `/etc/humanitl/opencode/llm-api-key` in the sandbox, from a memfd through
`--ro-bind-data`. `opencode.json` refers to it with `{file:/etc/humanitl/opencode/llm-api-key}`.
With `agent.adapter = "pi"` the same mechanism places it at `/etc/humanitl/pi/llm-api-key`, and
pi's `models.json` refers to it with `"apiKey": "!cat \"$HUMANITL_PI_KEY_FILE\""`, which pi
runs through `/bin/sh -c`; the path is a constant of the adapter, never a configured value, and
only the path stands in the `argv` of that `cat` (HUM-274).
It is deliberately not an environment variable: bwrap sets the agent's environment with
`--setenv NAME VALUE` on its own command line, which every process of the same user can read in
`/proc/<pid>/cmdline`. If the key is missing from the store, the session does not start and
names `LLM_009` with `humanitl llm key set`. A plan (`Sandbox(Plan)`) never reads the key; the
file is empty there, so plan and start have the same command line.

*Where it does not stand.* Not in `argv`, not in the environment, not in the window's snapshot,
not in the log: the type `ApiKey` shows as `••••` in every `Debug` form and is overwritten when
dropped. In the recording it stands as `••••`: before the agent sees it, the start adds it to the
recording's masks, and every recorded header, the `content_type` and `content_encoding` columns,
every recorded path with query, the host and the `Host` header carry `••••` instead of the value,
also URL-encoded, wholly or partly, in either case of the hex digits. Bodies are not masked:
their checksum is the address of the blob. A key shorter than 16 characters is not stored at all
(`CONFIG_020`): it could be neither masked nor found reliably.

*Where it is a finding.* The detector `known-secret` searches for the key literally and
URL-encoded in every header, in the query (raw and decoded) and in the decoded body of every
request, to any host. Every occurrence is a finding `api_key:llm` of level `checksum`. It is not
a finding in exactly one place: the `Authorization` header of a request to host and port of
`llm.endpoint`. A finding does not hold a request by itself; it is held anyway when no rule
allows its host, and with `hold.hard_block_checksum_secrets` on the finding holds it because of
its level. In the path before `?` and in the host no detector searches; there only the recording
masks.

The body is searched as far as every scan goes: up to `limits.preview_cap_bytes` (beyond that the
scan reports a gap, `FINDINGS_002`), decoded only for a `Content-Encoding` the scanner knows. A
gzip body is decoded across all members (RFC 1952 allows several in a row, and the destination
reads them all) under one shared budget of `limits.preview_cap_bytes` and
`limits.max_decompress_ratio`. Bytes after the end of a `deflate` or `br` stream, or after the
last complete gzip member, that cannot be read as another member make the scan search the decoded
part before them and report the rest as a gap (`FINDINGS_002`) instead of calling the body fully
searched (HUM-247). A `br` body that announces large-window brotli, an extension outside RFC 7932,
is not decoded and is reported as a gap (`FINDINGS_002`): its decoder would reserve a window of
up to 1 GiB before the budget applies (HUM-425). A `deflate` body is read with the zlib frame of
RFC 1950 first and as a raw RFC 1951 stream only when the frame is broken; when the frame reads
completely, the raw reading is compared against it under the same limits. A body whose raw reading
differs from the zlib reading is reported as a gap (`FINDINGS_002`) with the zlib reading
searched, since a receiver that only knows the raw stream would get content the scan never saw;
this holds when the raw stream ends cleanly, has bytes behind its end, or stops short in the
middle after it has already diverged, because a streaming receiver has delivered that content by
then. A valid zlib header that asks for a preset dictionary (`FDICT`) is a gap as well: the scan
has no dictionary, searches the raw reading and says that the zlib reading was not searched
(HUM-266). A key written differently from literal or URL-encoded (a JSON escape, `+` as a space in
a form) is no hit.

*What stays open.* The agent can read the key; it needs it. A subverted agent can therefore put it
into a request to another destination. There it is a finding, except in the path before `?` and
in the host; the request is held only at a host without an allowing rule or with
`hold.hard_block_checksum_secrets`. A key that leaves re-encoded (Base64, split, encrypted) is no
literal hit. Whoever controls the same user reads the key with `secret-tool lookup`, on macOS with
`security find-generic-password`, themselves; the keyring protects against other users, not
against this one. *What to do:* create a key with as few rights as possible, revocable at the
provider, and install `libsecret-tools` so it does not end up in the file; on macOS keep the
login Keychain unlocked when you set the key.

### 5.2 The project directory `/work`

*Why it exists.* An agent that may not write is a chat window.

*What Humanitl does.* Only a subpath is bound, never `$HOME`. The folder lies below `$HOME` or
is the one a person declared in `config.toml`; a stored folder gets this check at every start
against the path it resolves to then, and a declaration outside `$HOME` holds only for the path
its text really names, so a link put into a parent directory later cannot move the project
elsewhere (HUM-294). For the daemon only its own `config.toml` declares; a file a client names
with `--config` declares nothing, and its folder outside `$HOME` is refused with the command
that declares it in the daemon's file (HUM-296). `sandbox.work_mode = "ro"` is
possible and the suggestion for analysis-only sessions. Paths through which written files would
later be **executed** on the host are covered (full list in `profiles/sandbox/default.toml`):

- as an empty `tmpfs`: `.git/hooks`, `.vscode`, `.idea`, `.fleet`, `.github/workflows`,
  `.gitlab-ci.yml.d`, `.direnv`, `.humanitl`;
- as an empty, read-only file: `.envrc`, `.env`, `.env.local`, `.git/config`, `.npmrc`,
  `.yarnrc`, `.yarnrc.yml`, `.pypirc`, `.gitlab-ci.yml`, `Jenkinsfile`,
  `.pre-commit-config.yaml`.

A mask is a `--ro-bind-data` from a sealed, empty memfd, not `/dev/null`: binding a device onto a
`nodev` mount answers `EACCES`. The agent sees an empty file, and writes end in `EROFS`. bwrap
mounts only over a mount point that exists. If one of these directories or files is missing in
the project, nothing lies over it, and what the agent writes there lands in the project. Humanitl
does **not** create it: the daemon writes nothing into the project directory. Instead the gap is
named: the affected paths stand in the run summary, and once the agent has written below one of
them, `SANDBOX_025` (Warning) lists them. A profile can lift a mask with `mounts.unmask`, which
shows as `SANDBOX_020` at start; `/work/.envrc` and `/work/.git/config` are exempt, a profile
trying to unmask them is refused with `CONFIG_003`.

At the end of a sandbox run Humanitl shows the touched files, marks new symlinks whose target
lies outside `/work` (`SANDBOX_022`), runs the secret scan over the diff (`SANDBOX_023`), names
every file this machine runs by itself, a git hook, a `Makefile`, a `package.json` with
`postinstall`, a workflow file (`SANDBOX_026`), and names every changed file it could not look
into (`SANDBOX_028`). Both snapshots walk a tree the agent wrote to until a moment ago; every
access goes through `openat2` with `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS`,
and a symlink target is only read, never followed. Every name in the summary comes from the agent
and is sanitised before display; each row carries a hash of its real name, and a copy command is
built only from a path that can be quoted literally and that sanitising would not have changed.
The summary is kept in the recording (`session_summaries`); `humanitl sessions summary <id>`
fetches it long after the run. With `work_mode = "ro"` there is no summary: nothing can change
under `--ro-bind`.

*What the user should do.* Read the diff before committing, especially new files and everything
under `.github/`, `Makefile`, `package.json` (`scripts`) and build files. Start foreign
repositories with `--work-mode ro`, or disposable.

#### The disposable sandbox (HUM-350)

This section describes the overlay on Linux. On macOS the same mode runs on an APFS clone of the
project instead; [section 12](#the-disposable-sandbox-on-macos-hum-374) says what differs.

**Who gets it (HUM-357).** It is the default where the system allows it: `sandbox.mode` defaults
to `auto`, and the daemon resolves `auto` to a disposable sandbox when bubblewrap is 0.11 or
newer and not installed setuid, the kernel is Linux 5.11 or newer (unprivileged overlayfs), the
runtime directory is on tmpfs with a private layer directory (or one that can be created), and
the project is mounted writable. The probe only reads and creates nothing. Should the overlay
setup still fail before bubblewrap starts, `auto` falls back the same way with the real reason;
once the sandbox has started, nothing falls back any more. When one of these is missing, `auto`
runs **direct**, exactly as before HUM-350: the agent writes to the project
folder, and none of the protections below apply. The start says so with the warning
`SANDBOX_033`, which names the missing condition and its fix; the app shows the same reason at the
switch and in Settings, and the snapshot of the sandbox names the mode the next start gets
(`SandboxEvent.Status.sandbox_mode`). Only the default falls back. An explicit choice,
`sandbox.mode = "disposable"`, `humanitl run --disposable` or the switch in the app, refuses
such a start with `SANDBOX_030` and never writes to the folder. The distinction lives in the value,
not in the knowledge of which layer set it: `auto` is the default and may fall back, `disposable`
never does. A project profile cannot set `sandbox.mode` at all (CONVENTIONS 4.11), so a cloned
repository can neither switch back to direct nor turn an explicit `disposable` into `auto`. While changes of
an earlier disposable run wait, every start except an explicit `direct` is refused
(`SANDBOX_030`), checked before the mode is resolved, so a default that would fall back never
writes directly under changes nobody has looked at; the snapshot names that refusal. A disposable
run that changed nothing leaves nothing waiting: its layer is removed at once and the recording
says `no_changes`. That holds only for a complete scan: any directory of the layer or of the
project that cannot be read (a project folder with mode `030` under an opaque directory hides
its deletions) marks the list incomplete, and the layer waits for review as before.
`humanitl sandbox run`, which keeps no layer after it ends, treats `auto` as direct.

With a disposable start the agent writes nothing to the
project folder while it runs. The folder is the read-only lower layer of an overlay at `/work`;
every change goes to an upper layer the daemon creates before the start under
`$XDG_RUNTIME_DIR/humanitl/disposable/<id>/` (mode `0700`, on tmpfs, so in RAM; the start is
refused with `SANDBOX_030` when that directory is not on tmpfs, when bubblewrap is older than
0.11, or when the project is mounted read-only). The opened project folder reaches bubblewrap as
`/proc/self/fd/N`, so the folder that was checked is the lower layer (HUM-263). The daemon
opens the project folder once before the start and keeps that descriptor: the baseline for the
conflict check is taken on it before the sandbox starts, the launcher mounts only a folder with
the same device and inode (a rename plus a new folder under the same path is refused with
`SANDBOX_006`), and the diff and every write of an apply go through it, never through the path
again. Entries of
`mounts.extra_rw` are mounted read-only in this mode, so nothing writable from the host reaches
the sandbox; masks and `tmpfs` under `/work` lie on the overlay as before.

This is a new mount mode and a new write path back to the host, and its limits are part of the
claim:

- **Where the upper layer lives.** The sandbox never sees the path of the upper layer: `/run`
  never comes from the host, and bubblewrap opens the overlay directories before it mounts. The
  daemon reads the layer only after the sandbox has ended: bubblewrap is reaped and the
  sandbox's first process is gone from `/proc` (compared with its start time, so a reused process
  number does not count). When PID 1 of the sandbox ends, the kernel ends
  every process in its PID namespace, and the overlay lives only as long as a process in its
  mount namespace, so nothing writes into the layer while the daemon reads it.
- **Its size.** The hard limit is the tmpfs of the runtime directory (logind sets it, by default
  10 % of RAM). Below it the daemon measures the growth of that file system with `statvfs(2)`
  every 250 ms against its state when the layer was created, before the start, and stops the
  sandbox when it grows by more than 2 GiB or 200 000 entries, or by half of what was free at the
  start if that is less (`SANDBOX_032`); `humanitl sandbox run --disposable` runs the same guard. Counting the file system
  and not the tree means a directory with `chmod 000` hides nothing. Between two measurements an
  agent can write past the cap, up to the hard limit; what other programs write to the same
  tmpfs counts too and can only stop the sandbox earlier.
- **Reading it.** The diff walks the upper layer with the same `openat2(RESOLVE_BENEATH |
  RESOLVE_NO_SYMLINKS)` as the summary and never follows a symlink. Whiteouts (character device
  `0:0`) are deletions, `user.overlay.opaque = "y"` hides the lower directory, and below an
  opaque directory every directory hides the lower one too, so `rm -rf build && mkdir -p build/a`
  deletes what lay in `build/a`; a file copied up but unchanged is no change. Files and
  directories carrying `user.overlay.redirect` or `user.overlay.metacopy`, which an unprivileged
  overlay never writes, are listed and never applied. Paths and diffs are agent data: paths are sanitised and carry a hash of their real
  name, control and bidi characters in a diff are shown escaped.
- **Applying.** Only what a human selects is written (`humanitl sandbox apply`, which asks
  first). Every selected change is checked before the first write, and if one fails nothing is
  written. The parent of each change is opened with `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`;
  missing directories are created one by one with `O_NOFOLLOW`, so a symlink that appeared in the
  project meanwhile stops the write instead of leading out. A file is written under its own name
  with `O_CREAT | O_EXCL | O_NOFOLLOW` and moved into place with `renameat`, which replaces a
  symlink at the target instead of following it; a change of mode only is written the same way,
  as a new inode, so a hard link to the old file elsewhere keeps its mode. Directories are created
  with owner `rwx` and get their final mode last, deepest first. Never written: a symlink whose
  target is not a plain relative path inside the project (absolute, more leading `..` than its own
  depth, or `..` after a name such as `sub/d/..`, which could resolve through another symlink and
  leave the project), a device, FIFO or socket, setuid, setgid and sticky bits (dropped, and
  said), and a hard link (each name becomes a file of its own). A file or directory the human
  changed in the project since the start (type, size, `mtime` or mode differ from the snapshot
  taken before the start) is a conflict and written only when it is confirmed for exactly that
  change: the app names every confirmed change in `ApplyChanges.overwrite_ids`, and a conflict of
  any other selected change, also one that appeared after the selection, stops the apply before
  the first write; `--overwrite` on the command line confirms every selected change (HUM-351).
  Directly before each
  `renameat`, `unlinkat` or `mkdirat` the target is compared again with what was there at the
  check, when a directory is deleted every entry below it is compared before it goes (a changed or
  newly created entry stops the removal, and a directory whose tree cannot be read completely is
  refused before anything is written), and a deferred directory mode is set only on the same
  inode with the same mode as when it was noted; a change in between stops the apply with
  `partial`. A window of a few system calls stays
  between that comparison and the write: Humanitl does not lock the human's folder. A removal that
  stops midway can leave directories with mode `0700` and directories named `.humanitl-flat-*` in
  the project, and the finding says so.
- **Discarding.** `humanitl sandbox discard`, the rest after an apply, and the end of the daemon
  remove the layer: without following a symlink, giving directories without permissions back
  their mode, and flattening trees deeper than 64 levels instead of recursing. A layer a crashed
  daemon left behind is removed at the next start. The recording keeps the decision (applied or
  discarded, and which files) in the run's summary.

**Not under Podman (HUM-438).** The Podman backend has no disposable sandbox: `disposable`
refuses with `SANDBOX_030`, and `auto` runs direct with `SANDBOX_033`, both naming the reason.
Podman's overlay volume with its own upper layer (`-v <project>:/work:O,upperdir=,workdir=`)
works, but it takes the project as a path that the engine resolves later, and a descriptor is no
mount source there (measured in HUM-292, P13 for a bind and P10 for the overlay's lower layer:
`crun: ... Invalid argument`). Unlike a bind, the overlay cannot be tied to the checked folder
from inside the container either: it has a device of its own, and the `lowerdir` in its mount
options is only text. A folder swapped between the check and the mount could become the lower
layer, and the promise above (the folder that was checked is the lower layer, HUM-263) would not
hold. That is the deciding reason. A second, separate obstacle came out of the same measurement:
the engine writes the whiteouts and the overlay's work directory as its first subordinate ID, and
the daemon could not remove them when discarding the layer.

What it does not change: requests the agent sent through the proxy are out, whatever happens to
the layer. A disposable run says nothing about the agent's network traffic, and it is not a
second line of defence for the masks above; it removes the write path to the project for the
duration of the run, nothing more.

### 5.3 The agent's terminal output

*Why it exists.* The human has to see what the agent does. This output comes from the agent and
so potentially from the attacker.

*What Humanitl does.* The output runs through `humanitl_core::TerminalFilter` in the daemon before
it reaches any client: on the way to the terminal widget and on the way to `humanitl run`, which
writes it into the user's terminal. The filter is an allowlist with two policies.

- **`ColourOnly`, the stream without a pseudo terminal** (`humanitl run`, `SandboxEvent.output`):
  of all control sequences exactly one passes, `ESC [ … m` for colour and attributes. Dropped are
  OSC 52 (clipboard), OSC 8 (hyperlinks hiding a foreign target behind harmless text), window
  titles, every cursor movement, erase and scroll, terminal reset, and the string sequences DCS,
  SOS, PM and APC (`ESC P tmux; …` would otherwise smuggle a forbidden sequence through tmux).
- **`FullScreen`, the session terminal** (`Terminal` RPC, HUM-042): a full-screen agent draws with
  absolute addressing, so this policy lets every CSI sequence and every escape sequence except
  `ESC c` pass. It drops every string sequence except a short list of OSC numbers, colour (4, 10,
  11) with their resets (104, 110, 111) and prompt marks (133), whose payload must be printable
  ASCII; `CSI … t` (XTWINOPS, which sets and reads back the window title past OSC 0 and resizes
  the window); and `ESC c`.

Both policies recognise every sequence in three spellings: introduced with `ESC`, as a single C1
byte (`0x9b` is CSI, `0x9d` is OSC), and as its well-formed UTF-8 encoding (`C2 9B`, `C2 9D`),
because VTE-based terminals decode UTF-8 before parsing and execute `U+009B` as CSI. A sequence
that starts with a C1 introducer never passes. Only well-formed, shortest-form UTF-8 leaves the
daemon: an overlong form such as `E0 82 9B` is `U+009B`, and surrogates and code points above
`U+10FFFF` are dropped too. The ring buffer that serves a late client holds filtered bytes. Above
the terminal widget a permanent banner says that agent output is not trustworthy, and the widget
registers no clipboard handler: it refuses the clipboard requests of OSC 52 (and of the iTerm2
`CopyToClipboard` sequence) explicitly, writing nothing and answering nothing, because the emulator
would otherwise install handlers of its own that write and read the clipboard while the terminal
has the focus (HUM-313). What the emulator does answer are colour queries: OSC 4, 10 and 11 and the
colour-scheme query `CSI ? 996 n` get the colours of the theme back through the keyboard stream,
and only those of them that passed the daemon filter reach it. Text reaches the clipboard only by an action of the user: `Ctrl+Shift+C`
or the context menu; pasting is `Ctrl+Shift+V`, the menu or the middle button, bracketed while the
program asked for it, with every escape sequence of the pasted text removed.

**The controlling terminal (HUM-280).** bubblewrap runs with `--new-session` in every command
line, and that stays: `setsid` before the shim detaches the whole sandbox from whatever terminal
started bwrap, so `TIOCSTI` (which writes into a terminal's input queue and is allowed only on the
caller's own controlling terminal) can never reach the user's terminal. A session the daemon
starts sits on a pseudo-terminal the daemon opened for it; the user's terminal is never handed to
the sandbox, and the daemon opens the follower side with `O_NOCTTY`, so it never becomes the
daemon's controlling terminal either. For such a start, and only for it, the launcher passes
`--controlling-terminal` to the shim. The shim's child, the session's first process, then calls
`setsid` and `TIOCSCTTY` with argument 0 on descriptor 0, before the seccomp filter and the
`exec`. Argument 0 never steals a terminal that is already some session's controlling terminal,
and stealing would need `CAP_SYS_ADMIN` in the initial user namespace, which the sandbox never
has. An interactive user's terminal is the controlling terminal of the session of the shell
that runs in it, so a launcher bug that passed the switch with inherited descriptors would, in
that usual case, end in `EPERM`, which the shim reports and survives; the guarantee itself is
that the switch is never passed then. With its own pseudo-terminal as controlling terminal, `Ctrl+C` in an
interactive shell interrupts the foreground program and job control works. `TIOCSTI` inside the
sandbox now reaches only that pseudo-terminal's own input, which the sandbox already writes; the
master side stays in the daemon (`CLOEXEC`, never inherited), and what comes back out runs
through the filter above like every other byte. A sandbox started with inherited descriptors
(`humanitl sandbox run`, the escape suites) never gets the switch. `SandboxHandle::interrupt`
signals the terminal's foreground group (`tcgetpgrp` on the master) in addition to the group of
the sandbox's init, both of which lie in the sandbox.

*What the filter does not cover.* A C1 byte **inside** a longer printable UTF-8 character stays
text; a terminal that does not work in UTF-8 still reads it as a control. Dropping every such
byte would mean showing no text outside ASCII; this is a decision, not a gap to be closed. The
filter also does not cover what **text** is: the agent can print `[humanitl] request allowed: …`
itself. That is why the statement that matters, a request waits for a decision, stands in the
strip **above** the terminal that the window draws from the event stream, and in a separate frame
(`TerminalOutput.notice`), never inside the agent's bytes. *What the user should do:* never paste
displayed commands into a host shell without reading them.

### 5.4 Host names in the recording

*Why it exists.* A blocked request is recorded too, otherwise nobody would know what the agent
tried.

*What Humanitl does.* The recording lives locally under `$XDG_DATA_HOME/humanitl`; nothing is
uploaded. The export writes everything that was recorded: host, full path with query, all headers
and both bodies in plain text, and the export dialog says so in one sentence before the file is
written. Optional host redaction is planned after the MVP. *What the user should do:* treat an
export like the requests themselves; it carries tokens in headers and secrets in bodies. Host
names alone can reveal client projects.

### 5.5 Package caches

Without a cache every install costs network traffic and therefore an approval. Caches are kept
per project, never shared across trust levels; whoever configures none gets none. Do not share a
cache between a client project and an experiment. Read-only seed caches are planned after the
MVP.

### 5.6 Other known limits

An honest list of what is missing in 0.1 or weaker than one might assume:

1. **No external anchoring of the audit log.** The anchors live in the file and in SQLite on the
   same machine ([section 8](#8-the-audit-log)).
2. **No HTTP/2 to the upstream.** The proxy speaks HTTP/1.1 on both sides; ALPN offers only
   `http/1.1`. gRPC over TLS fails visibly with `PROXY_007`, not with a silent fallback.
3. **WebSocket frames are not held.** After an approved upgrade, frames flow and are recorded,
   but not confirmed one by one. Whoever does not want that blocks the upgrade.
4. **Responses are not held.** The MVP moderates requests. Responses are streamed and recorded; a
   malicious server can deliver content that acts as prompt injection. That is why the outgoing
   direction is strict.
5. **No repeated isolation check.** The three checks run at start; a periodic re-run is post-MVP.
6. **The window between `exec` and `SIGKILL`** ([section 4](#the-window-between-exec-and-sigkill)).
7. **Pseudonyms in responses.** The pseudonym mapping lives per session in memory; swapping
   pseudonyms back in responses is post-MVP.
8. **One sandbox technique.** bubblewrap is the only backend in 0.1. Other backends are planned
   behind `SandboxBackend` and are admissible only when `tests/escape/` passes against them.
9. **No `ProtectHome` in the systemd unit** ([section 6](#service-hardening)).
10. **Refused attempts are not in the history** (HUM-138). The isolation tab and
    `humanitl sandbox run` show them while the session runs; they are not recorded yet.
11. **The `/tmp` fallback of the runtime directory** (HUM-212). Without `XDG_RUNTIME_DIR` and
    without `/run/user/<uid>`, daemon and clients put socket and token under
    `$TMPDIR/humanitl-<uid>`. Another account can create that directory first or put a symlink
    there. Both are refused: the daemon takes over an existing directory only when it is no
    symlink and belongs to it (`DAEMON_004`), and clients read the token only when directory and
    token file belong to the own account and are closed to group and others (`DAEMON_001`). A
    stranger can still prevent the start while their directory is there. The remedy is an own
    `XDG_RUNTIME_DIR` under the home directory for the whole session
    ([`INSTALL.md`](INSTALL.md)). It is an availability weakness, not a confidentiality one.
12. **A recording outage stops the recording, not the traffic** (HUM-250). If the disk is full or
    the database locked, approved requests still leave, but are not recorded. The daemon sends
    `RECORDER_001` or `RECORDER_003` to every client, once per cause and again to a client that
    connects during the outage; the status bar turns red, every new card says it will not be
    recorded, and the first approval of the session pauses for a confirmation. `RECORDER_005`
    says when recording works again. Lost flows are not written afterwards.

### Out of scope

Humanitl does not claim to protect against: an attacker with root on the host or a kernel
exploit; physical access; manipulated Humanitl packages; the human who approves; the LLM host;
what the agent writes into `/work` and a human later passes on; microarchitectural and timing
side channels; availability (a blocked agent is the intended outcome); other processes of the same
account; network attacks on the host itself. Details in
[`THREAT-MODEL.md`](THREAT-MODEL.md#out-of-scope).

## 6. The proxy

**Own CA per installation.** To show the content of HTTPS requests, Humanitl has to terminate
TLS ("MITM"). The daemon creates its own certificate authority at first start. The private key
lives at `$XDG_DATA_HOME/humanitl/ca/ca.key` with mode `0600`, next to `ca.crt`. Only the public
certificate goes into the sandbox, as `/etc/humanitl/ca.crt`, and the environment kit points
`SSL_CERT_FILE`, `CURL_CA_BUNDLE`, `REQUESTS_CA_BUNDLE`, `NODE_EXTRA_CA_CERTS`, `GIT_SSL_CAINFO`,
`CARGO_HTTP_CAINFO`, `PIP_CERT`, `NPM_CONFIG_CAFILE` and the other variables of the profile at it.

**Never in the host trust store.** The certificate is not added to the system or browser trust
store. A CA there would mean that whoever gets the key can forge every TLS connection of the user,
including online banking that has nothing to do with Humanitl. Trust ends at the sandbox border:
whoever has `ca.crt` can forge nothing, and whoever has `ca.key` already has the host. *Check:*
`ls -l $XDG_DATA_HOME/humanitl/ca/` shows `0600` on `ca.key`; `trust list | grep -i humanitl`
(or `ls /usr/local/share/ca-certificates/`) finds nothing.

**Trust towards destinations: the system certificate store (HUM-257).** The other direction is
whom the daemon believes when it connects to a destination itself: in the proxy's forwarding path
and in the endpoint probe behind `ProbeLlm`. By default these are the roots of the system store
(`/etc/ssl/certs`, maintained with `update-ca-certificates`), read with `rustls-native-certs`,
the same as for `curl` and the browser, so a company CA works for a model server in the company
network.

- *Opt-out.* `tls.trust_system_store = false` (switch "System certificates" in the settings, or
  `humanitl config set tls.trust_system_store false`, effective after a daemon restart) uses only
  `webpki-roots`. The key is locked in the project profile.
- *Fallback.* If the store yields no usable root, the daemon takes `webpki-roots` instead of
  failing every connection; a single certificate rustls rejects costs a log warning. The start log
  states once how many roots came from where (`upstream trust anchors loaded`, with `anchors`,
  `source`, `skipped` and `env`).
- *What an attacker gains.* Whoever can put a CA into the system store is root, and root is
  outside the model. A process of the same account outside the sandbox could point the daemon at
  its own store through `SSL_CERT_FILE` or `SSL_CERT_DIR`; it could just as well change
  `config.toml`, and whoever configures the daemon is not covered. **These variables must never
  point at a path the sandbox can write**, such as the project directory. The daemon warns at
  start when such a path is writable by others or owned by the account, and the start line names
  the redirection in `env`.
- *What does not change.* The sandbox still gets only `ca.crt`, and the decision about a request
  falls before any connection; an additional trusted CA opens no destination that no rule and no
  human approved.

**The test root and its flag (`--allow-test-ca`).** For test runs the destination often sits on
the same machine with a self-signed certificate. `humanitld --allow-test-ca` reads the file in
`resolver.test_ca` as PEM and adds every certificate in it as a trust anchor for the proxy's (and
`ProbeLlm`'s) connections to destinations. The path must be absolute (otherwise `CONFIG_012`
before the file is touched); with the flag and an unusable file the daemon does not start
(`CONFIG_010`); the key without the flag, or the flag without the key, trusts nothing and warns
(`CONFIG_011`). It is a flag and not a key because configuration comes from files a project can
bring, and the command line belongs to the human who starts the daemon. The flag is in no systemd
unit and no package start command. It is visible in `ps -o args= -p <pid>` and as a `WARN` line at
start; it is not visible through the daemon's interface (`GetInfo` carries no capability for it
yet). There is no second lever of this kind: `experimental.upstream_port_map` was removed
(HUM-088), because silently mapping 443 to another port would move what a human approved.

**Certificate pinning.** Programs that accept only a built-in certificate fail visibly with a TLS
error. Humanitl offers **no** fallback that passes the connection through unencrypted: that would
be a tunnel for arbitrary data, and the recording would be a lie.

**Limits and timeouts.** The request body is fully buffered before the human is asked; whoever
shows only headers and streams the body hides exactly the part where data leaves. The full list
with codes is in [`reference/limits.md`](reference/limits.md):

| Limit | Default | When exceeded |
|---|---|---|
| `limits.hold_body_cap_bytes` | 32 MiB | `413`, `BlockReason::BodyCap` and `LIMIT_001` suggesting a rule `ask` + `stream: true`, even if a rule allows the host; except a matching `allow` or `ask` rule with `stream: true` |
| `limits.preview_cap_bytes` | 8 MiB | the preview is cut; the decision stays possible |
| `limits.max_decompress_ratio` | 100 | decompression stops while decoding (zip bomb), `LIMIT_003` |
| `limits.hold_max_bytes` | 256 MiB | sum of all bodies in progress, reserved while buffering; a request that no longer fits gets `503`, `BlockReason::HoldMemory`, `LIMIT_002`; held requests are never dropped |
| `limits.hold_max_flows` | 200 | a new request gets `503`, `BlockReason::HoldMaxFlows`, `LIMIT_002` |
| `hold.timeout_secs` | 300 | `504`, `BlockReason::Timeout`: expiry means **block**, never allow |
| `limits.connect_timeout_secs` | 10 s | connecting to the destination stops, the flow is `Failed` |
| `limits.header_timeout_secs` | 30 s | client request head and the gap before the next request; also the TLS handshake after `CONNECT` and, for a streamed upload, the time from the end of the body to the response headers (`LIMIT_004`) |
| `limits.body_timeout_secs` | 300 s | silence **between two chunks** of a body, both directions: request body `408` with `PROXY_011`; streamed response cut and marked truncated (`LIMIT_004`); never a limit on total duration |
| `limits.max_client_connections` | 256 | the accept loop answers `503` with `Connection: close`; `PROXY_010`, summarised once per minute |

The timeouts measure silence, not slowness, on purpose: a total duration of 300 seconds would cut
the model's stream, the declared side channel, and every long download. A slow but never silent
response stream is limited only by the number of connections; a bandwidth limit does not exist
yet, and a total deadline for buffering is HUM-258. `Expect: 100-continue` is answered by the
proxy itself; before the decision no byte reaches the upstream. A cut response body stays
recognisable in every export (`response_body_truncated`).

**`stream: true`: the one exception to buffering (HUM-057).** A request above
`limits.hold_body_cap_bytes` cannot be held like any other. It is blocked, unless a rule matches
that carries `stream: true` and says `allow` or `ask`, **and** `hold.hard_block_checksum_secrets`
is off. Then the decision is made on the head: `allow` passes without asking, `ask` holds and the
human sees method, destination and headers, but no body; only the approval reads the rest of the
line. Head, query and headers are scanned; the body leaves **unscanned**, with `LIMIT_001` as
`Info` on the flow, and is recorded like a streamed response up to
`limits.recorder_max_body_bytes`. A `block` or `redact` rule with `stream: true` does not stream.
The suggestion `LIMIT_001` offers at a block is an `ask` rule with `stream: true`, never `allow`.
With the hard block on, `stream: true` never applies and `LIMIT_001` suggests raising the cap
instead.

### Pausing intercept

Since HUM-407 a person can pause intercepting: from the header pill of the app, after a question
whose preselected answer is Cancel, or with `humanitl intercept pause`. While the pause lasts, the
statement "every new request waits for a human" is suspended, and only that one:

- **What changes.** A request that would wait for a person is not held: one no rule decides, one an
  `ask` rule matches, and, until the pseudonymiser of HUM-079 exists, one a `redact` rule matches,
  which then goes out **un-pseudonymised**. The proxy forwards it at once with the decision source
  `paused` (`DecisionSource::Paused`). The pause sits at exactly the place where the request would
  otherwise be held (`AskPipeline`), so everything before it still applies. Where nothing would be
  held, nothing changes: with `ask_mode = none` or `hold.timeout_secs = 0` a request still runs into
  its deadline and is refused (`504`), pause or not.
- **What stays.** `block` rules block. The hard block on checksum-confirmed secrets (HUM-049,
  HUM-159) blocks as the system. Authority consistency, private-address refusal, body caps, DNS
  after the decision and the three sandbox guarantees are untouched. Every request is recorded as
  before, each passed one with its own reason `paused` (history column, `flows.paused`, audit
  `flow.decided` with `decision: paused`, `decided_by: paused`).
- **Who can set it.** Only a client of the daemon socket (`0600`, with the session token), through
  the RPC `Intercept`. The proxy and the meta endpoint `humanitl.internal` hold no handle to the
  pause: nothing that arrives through the one door of the sandbox can set, extend or read it.
  `daemon/crates/proxy/tests/intercept_pause.rs` sends every plausible path and header from the
  sandbox side and finds the pause unchanged.
- **How long it lasts.** It lives only in the daemon's memory and ends when the person resumes,
  when a new session starts (`SessionSettings::set`) and when the daemon stops or restarts. No
  setting, file or profile makes it permanent. An orderly stop writes `intercept.resumed` with
  `daemon_stopping`; after `SIGKILL` or a crash the audit log holds `intercept.paused` without an end
  record, and the next `daemon.started` is the end, because the pause did not survive the process.
  A request that read the pause just before it was resumed may be recorded `paused` after
  `intercept.resumed`: at most the requests in flight at that moment.
- **How it shows.** The header pill reads "Intercept PAUSED" in the warning colour, and a running
  caution tape spans the window in every section (`docs/UX.md` 3.6). The app shows a pause at the
  latest one heartbeat (two seconds) after it began, the caution tape included; the daemon's audit
  log records it immediately. Start and end are audit
  records (`intercept.paused`, `intercept.resumed` with the reason `requested`,
  `session_started` or `daemon_stopping`), and every client sees the change on the event stream.
- **What the agent reads.** No answer text of the proxy or of `humanitl.internal` names the pause
  (HUM-364; that no answer names a person either is HUM-401): a passed request gets the answer of its destination without an added
  header, and `/why` gives a passed request the reason `user`, the same line a request a person
  allowed gets: the pause stands in for exactly that approval (HUM-401 later makes `user` neutral
  for every flow). The pause itself is not hidden by that: an agent can infer from the
  missing wait, and from requests that now succeed, that requests are not being held.

The pause is a decision of the person, made in advance for every request until it ends, like an
`allow` rule for `**` that cannot be saved. Whoever pauses accepts that what the agent sends in
that time leaves unseen; the recording shows afterwards what it was.

### Updates from inside the app

Since HUM-382 the daemon asks GitHub at most once a day whether a newer release exists, and
installs one when a person clicks "Update" (or runs `humanitl update apply`). This is the one
connection the daemon makes for itself. It is not part of the sandbox's door: it uses its own
connection stack with the egress `Direct` (the same construction as the endpoint probe), never the
proxy the agent talks to, and no request of the agent can trigger it.

**What is contacted.** `https://api.github.com/repos/nurkert/Humanitl/releases/latest` for the
latest release, then the release's files under
`https://github.com/nurkert/Humanitl/releases/download/`, and the storage GitHub redirects a
download to (`objects.githubusercontent.com`, `release-assets.githubusercontent.com`; a redirect
anywhere else, or over plain HTTP, is refused with `UPDATE_001`). The requests carry the address,
`Accept` and `User-Agent: Humanitl/<version>`, nothing else: no identifier, no configuration, no
statistics. GitHub learns the IP address and the installed version, as with any download.

**How the signature binds an update to the key.** The release pipeline writes one manifest per
installable file (`update-macos.json` for the zip, `update-linux-x86_64.json` for the archive,
`update-linux-x86_64-appimage.json` for the AppImage) with `version`, `url`, `sha256`, `size` and
`min_os`, and signs its exact bytes with Ed25519 (`openssl pkeyutl -sign -rawin`) using the
repository secret `HUMANITL_UPDATE_SIGNING_KEY`
([`packaging/release/sign-update.sh`](../packaging/release/sign-update.sh)). The step checks that
the secret matches the public key built into the daemon before it signs; a tag build without the
secret fails. The daemon verifies the signature against `RELEASE_KEY` in
`daemon/crates/proxy/src/self_update.rs` (raw 32 bytes, base64) with the Ed25519 implementation of
the TLS stack it already ships (ring through rustls) before it reads a single field. Then:

- the version must be of the form `MAJOR.MINOR.PATCH`, equal to the release tag, and newer than
  the highest of three: the running daemon, what is installed at the target now (the bundle's
  `Info.plist` on macOS) and the highest version this daemon ever installed there, kept in
  `update-installed.json` under the data directory. The record is written before the swap; if it cannot be written, nothing is swapped, and a failed swap resets it. A replayed older
  manifest is refused, also while the restart after an update still waits for a session to end;
- the URL must lie under `https://github.com/nurkert/Humanitl/releases/download/` after `..`
  segments are resolved, without user info, query or fragment, and it must be exactly
  `…/download/v<version>/<file>` with the file name of the manifest's kind
  (`Humanitl-<version>-macos.zip`, `humanitl-<version>-linux-x86_64.tar.gz`,
  `Humanitl-<version>-x86_64.AppImage`), so a signed manifest of one kind or release never stands
  in for another;
- the download is counted and hashed while it is written; one byte more than `size` aborts, and a
  different SHA-256 refuses (`UPDATE_003`);
- `Apply` checks the manifest afresh and refuses when it now names another version than the one the
  person was offered.

**How it is installed.** All work happens in a `0700` directory under the runtime directory, which
is removed afterwards. On macOS the zip is unpacked with `ditto`, the app must pass
`codesign --verify --deep --strict`, carry the entitlement
`com.apple.security.files.user-selected.read-only` and the manifest's version in its `Info.plist`,
like `packaging/macos/build-app.sh` checks; the copy placed next to the target is checked again.
An AppImage is replaced or restored only at a path the daemon trusts: the one `humanitl daemon install` recorded from the AppImage itself (`<data_dir>/appimage-path`, `0600`) or one the daemon installed before; any other path in a request is refused with `UPDATE_004` before anything on disk changes. An unpacked archive must hold exactly `humanitl-<version>-linux-x86_64/` with the three programs
and the app; an AppImage is taken as the signed bytes. Then the target is renamed to
`.previous`, the new one moved into place, and `.previous` removed only after that succeeded; if
the second step fails, `.previous` is moved back (`UPDATE_005`). A swap that stopped halfway
(target missing, `.previous` present) is restored before anything else, under the lock of `Apply`
and before any request to GitHub, with the target taken from the path of the running installation; `.previous` is never
deleted while the target is missing, and if it cannot be moved back the update refuses. A bundle under `/Volumes`, in App
Translocation, a build tree or a folder this user cannot write is never touched (`UPDATE_004`).
The download by the daemon carries no quarantine flag, so Gatekeeper does not stop the new app;
the signature check above replaces it for this one path, and the ad-hoc code signature only proves
that the bundle is intact, not who built it.

**Packages.** A package from the APT or dnf repository is never replaced by the daemon. The app
runs a fixed argument vector without a shell, `pkexec apt-get install --only-upgrade -y humanitl`
or `pkexec dnf upgrade -y humanitl`, and only if the daemon named exactly this vector; polkit asks
for the administrator password and the package manager verifies its repository signatures. The
daemon does not run `pkexec` itself: as a user service it has no polkit agent of the session. The
AUR, a package installed from a file and a machine without `pkexec` get the command to copy
(`UPDATE_004`); no AUR helper is ever run.

**The restart.** After a swap the app restarts the daemon through the existing retire-if-idle path:
the daemon ends only when no start holds a claim and no sandbox runs, so an agent session is never
cut off without the existing second answer. A restart that fails is shown in the status bar and
nothing is relaunched; a session that holds the daemon is no failure, the new app then offers the
existing restart button. An AppImage refreshes its copy of the daemon on every start
(`humanitl daemon install --refresh`); since HUM-382 that switches the copy and restarts only after
`retire-if-idle` answered `retired` (or no daemon runs; a daemon older than contract 1.19 only when
a fresh `Sandbox(Status)` shows no session), so relaunching the new AppImage never ends
a session. The app of an unpacked archive finds its command line at `<root>/bin/humanitl` beside
`<root>/app`, with the same ownership check as the other places.

**What this does not cover.**

- A stolen release key signs malicious updates that every installation accepts. The key lives only
  as a repository secret, is visible only to the two signing steps of `release.yml`, and is never
  printed. A new key needs a release that ships the new public key, signed with the old one.
- A GitHub account taken over without the key cannot ship an update of its own. What it can do:
  withhold updates or break the check (`UPDATE_001`); serve an older signed manifest (refused as not
  newer than the installed version floor); serve a signed manifest under the wrong tag (refused,
  version and tag must match) or under the wrong kind (refused, the file name is bound to the
  manifest); delete a release so the previous one becomes "latest" again (refused as not newer);
  and learn which addresses run which version.
- The check reveals to GitHub that a Humanitl installation exists at this address and which
  version it runs.
- A package from the APT or dnf repository is as trustworthy as that repository's signing key;
  the app only starts the package manager.

**How to turn it off.** `update.check = false` in `config.toml`, or the switch "Check for updates
automatically" in the settings. Then the daemon connects only when someone asks explicitly
(`humanitl update check`, "Check now"). The key is closed to project profiles.

### Service hardening

The systemd unit (`packaging/systemd/humanitld.service`) runs as a user service, never as a system
unit, and is hardened as far as the sandbox backend allows (HUM-053): `NoNewPrivileges`,
`PrivateTmp`, `ProtectSystem=strict` with `ReadWritePaths` for the data, config and runtime
directories, `PrivateIPC`, `PrivateMounts`, `PrivateUsers`, `ProtectClock`,
`ProtectControlGroups`, `ProtectKernelModules`, `ProtectProc=invisible`, `KeyringMode=private`,
`LockPersonality`, `RestrictRealtime`, `RestrictFileSystems=~@privileged-api`,
`CapabilityBoundingSet=CAP_SYS_ADMIN`, `SystemCallFilter=@system-service @mount @sandbox sethostname`
and `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK`. Every child of the daemon
inherits these restrictions, the sandbox and the agent included; they add to the three guarantees
and replace none of them.

Deliberately **not** set, each measured: `RestrictNamespaces` (the sandbox needs namespaces),
`ProtectKernelTunables`, `ProtectKernelLogs` and `ProtectHostname` (the sandbox could no longer
mount its own `/proc`), `RestrictSUIDSGID`, `MemoryDenyWriteExecute` (inherited by the agent; node
and every JIT runtime dies at start), `SystemCallArchitectures=native` (the shim dies when it
checks its own filter), `PrivateDevices` (no terminal for the agent) and `ProtectHome`.
`ProtectHome=read-only` does not fit projects under `$HOME` that are mounted writable; the daemon
runs as the same user anyway. `ProtectSystem=strict` makes everything outside `$HOME` and the
runtime directory read-only: a project outside `$HOME` is read-only in the sandbox until the human
adds an own `ReadWritePaths` line ([`INSTALL.md`](INSTALL.md)).

**Podman under the unit (HUM-434).** No property of the unit is loosened for Podman, and none for
bubblewrap. Measurement P8 (2026-11-13, Podman 5.8.6) showed that every property with a mount
namespace of its own (`PrivateTmp`, `ProtectSystem`, `PrivateMounts`, `ProtectClock`,
`ProtectControlGroups`, `ProtectKernelModules`, `PrivateIPC`, `PrivateUsers`; in a user unit each
implies a private user namespace) breaks `newuidmap` ("write to uid_map failed"), so rootless
Podman cannot start as a child of the daemon; loosening them all would have undone most of the
hardening. Instead, when the daemon runs outside the first user namespace (that is, under this
unit), it starts every Podman call as a transient unit of the user's service manager:
`systemd-run --user --pipe --wait --collect --service-type=exec --expand-environment=no` with
`KillMode=mixed`, `Delegate=yes` (so that the engine gets the cgroup controllers the user has) and `TimeoutStopSec` equal to the kill grace, named `humanitl-<session>` for a
session and `humanitl-q-<id>` with `RuntimeMaxSec` for a query. Podman runs there behind
`/usr/bin/env -i` with exactly the environment the daemon gives it, never the service manager's.
Stopping a session stops its unit as well; no unit lingers after logout, and the daemon never
enables linger. A user service manager that does not answer, is older than systemd 254 or lacks
`systemd-run` stops the start with `SANDBOX_050`. What this costs: a compromised daemon can start
a transient unit of the user, outside its own hardening ([`THREAT-MODEL.md`](THREAT-MODEL.md)).
Because the engine's client is then no child of the daemon, it inherits nothing: the shim's
descriptors travel through a socket of the session, `0600` in a `0700` directory of its own, that
accepts exactly one connection, of the daemon's uid (`SO_PEERCRED`) naming a one-time token of 128
bits, and closes before anything is checked. Only that directory is bound read-only into the
sandbox, at `/run/humanitl-handover`, and the daemon removes the socket file before it sends any
descriptor: when the shim has them, the sandbox holds no socket but the proxy's, and
`single_socket` needs no exception. When the engine runs as a unit, it sees the mount flags of
the sources from another mount namespace than the daemon (the unit's `ProtectSystem` and
relatives remount the host's file systems `nosuid`); the mount table then leaves the inherited
flags of bind mounts open and keeps point, type, read-only or writable, count, and device and inode
strict. The same path serves a Podman started
from a terminal, so one way is tested everywhere. Writable access to `~/.local/share/containers`
belongs to the transient unit, not to the daemon's unit, which keeps its `ReadWritePaths`.

**Socket activation.** The package ships `humanitld.socket`: systemd binds
`$XDG_RUNTIME_DIR/humanitl/daemon.sock` with mode `0600` in a `0700` directory and hands the
descriptor to the daemon. The daemon takes it only when `LISTEN_PID` names itself, exactly one
socket was passed, and it is a listening Unix stream socket at exactly the path the clients look
for; otherwise it does not start (`DAEMON_013`). It removes `LISTEN_PID`, `LISTEN_FDS` and
`LISTEN_FDNAMES` from its environment before the first thread, and neither the sandbox backend nor
the agent inherits the listening socket. The token is still written by the daemon itself, and
without the token the service accepts no call.

**Fuzzing.** Six cargo-fuzz targets run nightly for ten minutes each
(`.github/workflows/fuzz-nightly.yml`, HUM-056): the HTTP request path, the body decoder (gzip,
deflate, brotli), the chunked decoder, the rules YAML parser, the flow filter language and the
audit verifier.

## 7. The rules engine

Rules are an ordered list; the first match wins; the default is `ask`. Session rules (in memory,
`expires: session`) are evaluated before persistent ones, so a decision just made applies at once.
The field reference is [`reference/rules.md`](reference/rules.md). The traps are all known and all
tested (ESC-4):

- **Normalisation before comparison.** Host names are converted to A-labels (Punycode) with
  `idna::domain_to_ascii`, lower-cased and stripped of a trailing dot. `API.GITHUB.COM.` matches
  `*.github.com`.
- **Globs work on labels, not characters.** `*` is exactly one label, `**` one or more.
  `*.github.com` matches `api.github.com`, but **not** `github.com` itself and **not**
  `a.b.github.com`. A substring comparison would be the classic hole: `evil-github.com` and
  `github.com.evil.io` never match.
- **Internationalised names look like others.** `xn--80ak6aa92e.com` renders in many fonts as
  `аpple.com` with a Cyrillic "а". The window always shows the A-label form and the identity from
  the bundled catalogue next to it.
- **IP literals never match a host glob.** They need an explicit rule with `host: "ip:192.168.1.50"`
  or `host: "cidr:192.168.0.0/16"`.
- **Private ranges are refused.** If a name resolves to RFC 1918, `127/8`, `169.254/16` (including
  `169.254.169.254`, the cloud metadata address), `100.64/10`, `0/8`, `192.0.0/24`, `198.18/15`,
  `240/4`, `::`, `::1`, `fc00::/7`, `fe80::/10` or `fec0::/10`, the connection is refused unless the
  matching rule carries `allow_private: true`. The same holds for IPv6 addresses that embed such an
  IPv4 address in a fixed pattern (IPv4-mapped, IPv4-compatible, IPv4-translated, NAT64
  `64:ff9b::/96`, 6to4 `2002::/16`, ISATAP) and for Teredo (`2001::/32`) and local NAT64
  (`64:ff9b:1::/48`), whose embedded address cannot be read unambiguously. 6rd has no fixed prefix
  and is not recognised.
- **Authority consistency.** After TLS termination the `Host` header or `:authority` of every
  request is compared with the `CONNECT` target and the SNI. A mismatch is blocked without asking
  (`BlockReason::AuthorityMismatch`, `403`): the defence against domain fronting.
- **DNS after allow.** See below.
- **WebSocket.** An upgrade matches only a rule that carries `upgrade: websocket`; otherwise `ask`.
  The window says before approval that frames are recorded, not held.
- **Redirects inherit nothing.** A `301` or `302` makes the client send a new request to a new
  destination, which runs through the rules again.
- **Unknown methods lead to `ask`**, never to `allow`.
- **No rule lifts the hard block.** With `hold.hard_block_checksum_secrets` on, a request carrying a
  checksum-confirmed secret does not leave unchanged: an `allow` rule and the LLM passthrough block
  it as the system (`BlockReason::Secret`, `403`); without a rule it is held, and `Decide(Allow)`
  ends with `HOLD_004`. It leaves only edited, and only when a second scan of the edited version
  finds no such secret any more (HUM-049, HUM-159). The check sits in the daemon, not in the window.

**DNS after allow (ADR-006).** A host name carries up to 63 bytes per label and 253 in total.
Whoever has `ZGllcyBpc3QgZWluIGdlaGVpbW5pcw.attacker.example` resolved has transmitted the data as
soon as any resolver asks: the attacker's authoritative server sees the query even if no HTTP
connection follows. A proxy that resolves **before** a human decided is the exfiltration channel
itself. Humanitl therefore holds requests on the host name as a string. It resolves only after
`allow`, exactly once, through the `Resolver` port, never through the system resolver of an HTTP
connector; behind the port the system's name service answers or, when `resolver.nameserver` is
set, hickory asks exactly that server. The address found is pinned to the connection
(`Egress::connect(authority, Some(ip))`), so a second lookup cannot return another target (DNS
rebinding). The domain preview in the window fetches nothing from the network; it comes from a
bundled catalogue.

*Check.* `humanitl rules test https://api.github.com/repos/x/y` prints the matching rule and exits
0 (allow), 10 (block) or 11 (ask). ESC-4 runs the full table. ESC-3 observes on the host that no
lookup happens for a held name before the decision: the run's daemon asks a recording name server
(`tests/escape/dns-stub.py`) through `resolver.nameserver`, and `dns_not_before_decision`,
`dns_after_allow_once` and `meta_no_dns_lookup` read its log (HUM-115). The system name service
hangs off the same single call site after the decision; `daemon/crates/proxy/tests/dns_after_allow.rs`
proves it in the daemon.

## 8. The audit log

Recording is local: flows, decisions and bodies in SQLite at `$XDG_DATA_HOME/humanitl/humanitl.db`,
large bodies content-addressed under `$XDG_DATA_HOME/humanitl/blobs/<hex[0..2]>/<sha256>`, and in
addition the log `$XDG_DATA_HOME/humanitl/audit/audit.jsonl` (HUM-050). Every record is one line of
canonical JSON with `seq`, `ts`, `session`, `kind`, `data`, the predecessor's hash (`prev`) and its
own `hash`, sealed with HMAC-SHA256. Bodies, headers, plain-text values of findings and the note of
a block never stand in the log; the path of a request only as SHA-256; the host in plain text.

The HMAC key lives at `$XDG_DATA_HOME/humanitl/keys/audit.key` (`0600` in a `0700` directory,
refused with `AUDIT_005` if others can read it or it belongs to another user). **Head anchoring:**
every `audit.anchor_every` records (default 100) and when the daemon stops, an anchor stands in the
file **and** the same one in the table `audit_anchors` of `humanitl.db`. Whoever changes only the
file fails at the database; whoever changes both also needs the key. Both places are on the same
machine. If `audit.jsonl` ends before an anchor from `audit_anchors` at start, the daemon starts
anyway, reports `AUDIT_007`, and continues the chain behind the last anchor with `audit.resumed`;
`verify` keeps reporting the gap. If the end itself does not fit (a last record that does not match
hash or key), the daemon does not append (`AUDIT_001`).

### What the chain proves

It proves that since the last anchor was set, no record up to that anchor was changed, removed or
reordered without `verify` reporting it, as long as the attacker does not have the HMAC key.

It does **not** prove:

1. that the daemon wrote honestly. A manipulated daemon logs what it wants;
2. that no events are missing that were never written. A daemon killed hard can write half a last
   line; the next start puts it aside (`AUDIT_002`);
3. that the last up to `anchor_every` records after the last anchor were not truncated. A cut
   exactly on an anchor leaves a valid, shorter chain; `verify` warns only about unanchored records
   behind it (`UnanchoredTail`). A daemon that stops in order anchors its last record;
4. anything against an attacker who has the key. A process running as the **same user** has key,
   file and database and can rebuild chain and anchors.

Stronger guarantees need external anchoring (post-MVP).

**Retention: delete, never rewrite in the middle (HUM-157, HUM-194).** `audit.retention_days` and
`audit.max_bytes` default to `0`, and then the chain keeps every record. With either set, a daily
run deletes records at the **start** of the chain, oldest first, after verifying the whole chain
(**a chain that does not hold is not shortened**, `AUDIT_001`), appends a sealed `audit.pruned`
record naming number and hash of the last deleted record followed by an anchor, and rewrites the
file. `audit_anchors` is never shortened. `verify` accepts a start other than 1 only when an
`audit.pruned` names exactly that cut; otherwise it is a `SeqGap`. What stood in deleted records
the chain no longer proves; an export is the only copy. With a byte limit, the **amount** of records
decides how long a record stays, so an agent with many requests can push earlier records out
faster, but never unnoticed.

**Recording outage (HUM-250).** If the recorder cannot open or write its database (disk full,
locked, read-only), every client gets `RECORDER_001` or `RECORDER_003` once per cause, a client that
connects during the outage gets them first, and the window marks every card that is not recorded
and pauses the first approval of the session. Traffic is not stopped by the outage; the statement
"both are recorded" is then visibly false, not silently false. `RECORDER_005` ends the outage.

*Check.* `humanitl audit verify` asks the daemon, which verifies with key and anchors (HUM-156); the
CLI and the audit screen see the same chain with the same strength and head hash. Without a
daemon, `humanitl audit verify --file` checks chain and canonical form **without key and anchors**
and says so (`no HMAC key (file mode)`, `no anchors (file mode)`). `humanitl audit export` checks
nothing; it copies or formats, so that even a broken log can be handed over as evidence (HUM-214).
ESC-5 runs `audit_delete_is_detected` and `audit_truncate_is_detected` against a real daemon; limit
3 stands as its own test (`truncate_above_last_anchor_not_detected_documented`).

## 9. What we verify in CI

The guarantees in this document can be checked, not only read. Three ways to look, from the
quickest to the most thorough:

**1. The exact command line.** The complete isolation policy is one bwrap call. It is shown in the
window (Sandbox, Isolation, "Show command line") and on the command line:

```sh
humanitl sandbox argv
```

Whoever wants to know what is bound reads the `--bind` and `--ro-bind` arguments; whoever wants to
know what is not bound looks in vain for `/run/user`, `.ssh` and `docker.sock`.

**2. The checks.**

```sh
humanitl sandbox check           # three checks, evidence from a running sandbox
humanitl sandbox check --json    # the same, machine-readable
```

The checks read `/sys/class/net` and `/proc/net/dev`, walk the filesystem for sockets **inside**
the sandbox, and read the `Seccomp:` line from a child that carries the filter. If a check fails, the session is stopped
and its state is `failed`; there is no "start anyway".

**3. The escape tests** ([`tests/escape/`](../tests/escape/)). They run in the CI job
`escape-tests` on every issue branch and on `main`, and locally with `make escape`. The same
suites run against Podman in the CI job `escape-tests-podman` (HUM-435,
`HUMANITL_ESCAPE_BACKEND=podman`), on a runner where a hostile engine configuration of the user
was planted first and proven to reach a plain container; a backend is admissible only when every
probe is green against it, and a run that cannot start the backend is an error, never a pass or a
skip:

| ID | File | What it shows |
|---|---|---|
| ESC-1 | [`esc-1-sockets.sh`](../tests/escape/esc-1-sockets.sh) | socket families and types, `io_uring`, x32, interfaces, routing, capabilities, `Seccomp: 2` for every process, PID 1, refusals reported |
| ESC-2 | [`esc-2-mounts.sh`](../tests/escape/esc-2-mounts.sh) | mount surface, exactly one socket, no host `/proc`, hostname; under a container engine also an empty `/run/.containerenv`, no socket of `conmon` or the engine, the shim as PID 1 with `Seccomp: 2`, and no mount, volume or hook from the user's engine configuration |
| ESC-3 | [`esc-3-egress.sh`](../tests/escape/esc-3-egress.sh) | no egress without the proxy; through the proxy every request no rule decides lands in the queue (`allow` rules and the LLM passthrough skip it); no DNS before the decision, proven on the host at the log of a recording name server |
| ESC-4 | [`esc-4-rules.sh`](../tests/escape/esc-4-rules.sh) | the rule table including homograph, IP literal, WebSocket and body cap |
| ESC-5 | [`esc-5-filesystem.sh`](../tests/escape/esc-5-filesystem.sh) | symlink marking, masked paths, OSC 52, audit tampering, a disposable run never writing to the project (HUM-350) |

Besides the escape tests, CI runs the conformance matrix of real clients through the proxy
(`daemon/crates/proxy/tests/conformance.rs`), the backpressure load test (1000 flows in 10 s), the
check that the shim is static and under 2 MB, and nightly fuzzing.

## 10. Reporting a vulnerability

Please do **not** report a security problem as a public issue.

Contact: see GitHub security advisories for this repository (private vulnerability reporting).

Report form: <https://github.com/nurkert/Humanitl/security/advisories/new>

- **What helps:** the affected version, the sandbox profile (`humanitl sandbox argv`), a minimal
  reproduction and, if possible, the observation as an escape-test probe in the style of
  `tests/escape/`.
- **Response:** acknowledgement within 72 hours, assessment within seven days.
- **Disclosure:** coordinated disclosure after 90 days or when a fix is available, whichever comes
  first. If you want to publish earlier, please say so in the report.
- **No bug bounty.** This is a single-maintainer project under GPL-3.0. Credit in the changelog
  below is gladly given, by name; money is not.

## 11. Changelog of security-relevant changes

Every security-relevant change to the shim, the mount allowlist, the seccomp filter, the
passthrough or the audit chain gets a line here and in the change history of
[`THREAT-MODEL.md`](THREAT-MODEL.md#change-history).

| Version | Change |
|---|---|
| 0.1.0 | Initial release. |
| 0.1.0 | Several saved model servers with one keyring item each; only the active origin gets a key and the passthrough (HUM-289). |
| 0.1.0 | The session's first process takes the daemon's pseudo-terminal as controlling terminal; `--new-session` stays. A caller's command gets the adapter's contribution and the configured agent on `PATH`, never a bind of its own (HUM-280). |
| 0.1.0 | OpenCode no longer asks before `webfetch` and `bash`; the proxy is the one place that asks. Its `bash` waits as long as a held request (HUM-308). |
| 0.1.0 | A `br` body with the large-window marker (outside RFC 7932) is not decoded but reported as a gap; five bytes of it made the decoder reserve 512 MiB, up to 1 GiB, outside the decompression budget (HUM-425). |
| 0.1.x | Further shells next to the agent: the daemon asks PID 1 over an unnamed `SOCK_SEQPACKET` pair only it and PID 1 hold; the shim forks the shell from inside with the agent's filter on top, proven by a refused `AF_UNIX`; at most four, ending with their process or the sandbox (HUM-299). |
| 0.1.x | The stream mode of the terminals: the shim opens the pseudo-terminals of the agent and of every further shell, and the daemon talks to them over one stream socket in frames with fixed maximum lengths, oversized frames, unknown kinds and unknown ids dropped; the stream carries no byte for the proxy, the agent's filter refuses `AF_VSOCK` and `AF_UNIX`, and the end of the stream ends the sandbox (HUM-443). |
| 0.1.x | pi loads the globally installed extension `pi-web-access` (`agent.pi.web_access`, default `auto`, closed to project profiles); its requests go through the proxy like every other (HUM-309). |
| 0.1.x | macOS: the disposable sandbox on an APFS clone, masks removed from the clone and denied in it, the real project sealed, the clone renamed before it is read, and a failed clone that is never silent: `auto` warns with `SANDBOX_033`, an explicit `disposable` refuses with `SANDBOX_030` (HUM-374, K-24). |
| 0.1.x | Updates from inside the app: a daily check of GitHub (`update.check`, default on), manifests signed with Ed25519 and checked against a built-in key, size and SHA-256 checked while downloading, code signature and version checked before an atomic swap with restore; APT and dnf packages through `pkexec` and their package manager (HUM-382, [Updates](#updates-from-inside-the-app), K-25). |
| 0.1.x | Pausing intercept: requests that would wait for a person pass with the reason `paused` while a person keeps the pause on (no change without a hold deadline); block rules, the hard block, the sandbox and the recording stay; settable only over the daemon socket, never from the sandbox; ends on resume, with the next session and with the daemon; start and end in the audit log (HUM-407, [Pausing intercept](#pausing-intercept), K-26). |
| 0.1.x | The shim takes a lifeline (`--lifeline-fd`, the read end of a pipe only the daemon writes; `POLLHUP` ends the sandbox as on `SIGTERM`) and the agent's environment as a sealed memfd (`--env-fd`, never in `argv`); the agent inherits neither (HUM-430, [section 4](#the-lifeline-and-the-environment-through-descriptors-hum-430)). |
| 0.1.x | macOS: Seatbelt as the sandbox backend, a generated SBPL profile per session, the three guarantees restated and measured by the shim; the daemon as a launchd agent; socket and token under `~/Library/Application Support/Humanitl/run` (HUM-323, HUM-327, [section 12](#12-macos-seatbelt-instead-of-bubblewrap)). |
| 0.1.x | Linux: Podman as a second sandbox backend, plain level (rootless, runtime `crun`), not admitted: every start refuses it with `SANDBOX_048` until HUM-448; only the escape suite and the tests start it. Every configuration source the daemon does not write is cut off, the environment goes only through the sealed memfd, the container ends through the engine and the lifeline (HUM-433, [section 13](#13-linux-podman), K-27). |
| 0.1.x | The escape suite runs against every backend; the CI job `escape-tests-podman` runs it against Podman through `escape-launch --backend podman`, with a hostile engine configuration planted on the runner (HUM-435, [section 9](#9-what-we-verify-in-ci), K-27). |
| 0.1.x | Linux, Podman: an image as the root of the sandbox, pinned by digest and checked before every start (`SANDBOX_043`), never pulled by a start; `humanitl sandbox image pull` pulls on request, directly and not through the proxy, and says so first; project, door, CA, shim and session files come from the host, and ESC-2 checks the CA by device and inode (HUM-439, [section 13](#an-image-as-the-root-hum-439), K-28). |
| 0.1.x | Linux: Podman admitted at the plain level (`podman/crun`) after `escape-tests-podman` was green on `main`; a person chooses it with `sandbox.backend = "podman"` or `--backend podman`, `auto` stays bubblewrap, and the kernel level (backend `qemu`, ADR-0021) stays refused with `SANDBOX_048` until HUM-449 (HUM-448, [section 13](#13-linux-podman), K-27). |

## 12. macOS: Seatbelt instead of bubblewrap

macOS has no network namespace, no mount namespace and no seccomp. There the sandbox is
Seatbelt, the sandbox of the macOS kernel: the daemon writes one SBPL profile per session and
starts the shim under `/usr/bin/sandbox-exec -f <profile>`. The kernel holds the shim and every
process it starts to that profile; no child can drop it. The decision and the alternatives,
among them a Linux VM with the bwrap backend inside (HUM-328), are in
[ADR-0019](adr/0019-seatbelt-on-macos.md).

### The three guarantees on macOS

| # | On Linux | On macOS | Measured by the shim as |
|---|---|---|---|
| 1 | No network interface | **No way out.** The interfaces exist, but every `connect`, `bind` and `sendto` except the three below ends in `EPERM`; there is no name resolution. | a TCP connection to a public IPv4 and IPv6 address and a UDP datagram, all refused (`no_interfaces`) |
| 2 | Exactly one door | **Exactly one door.** Allowed are the proxy socket of the daemon, the loopback port of the session on which the shim relays to it, and a second random port of the session for the agent's own local server (HUM-365). | the door answers, `/var/run/syslog` and a foreign loopback port do not (`single_socket`, `bridge_listening`) |
| 3 | seccomp | **The kernel holds the process to the profile.** Routing, system and driver sockets are refused at `socket(2)`; unprivileged ICMP has no remote address the profile allows. | `sandbox_check(getpid())` reports the process as sandboxed; `AF_ROUTE`, `AF_SYSTEM`, `AF_NDRV` and `AF_INET SOCK_RAW` end in `EPERM` (`seccomp_applied`, `families`). The raw socket is refused to any unprivileged process anyway; it stays in the list as a floor. |

The check names are the ones of Linux, so the isolation panel and `humanitl sandbox check` read
the same report; the evidence column says what was measured on which system.

### The profile

The profile is generated by `SeatbeltPolicy::render` in
`daemon/crates/sandbox/src/seatbelt.rs` and lies, read-only for the agent, as `sandbox.sb` in the
session directory. It denies by default. In SBPL the last matching rule wins, so the denials of
masks come last.

- **Reading:** the whole system except `/Users`, `/Volumes`, `/private/var/folders`,
  `/private/tmp` and `/private/var/tmp`; from these only the project, the session directory, the extra folders of the
  profile, and single files: the shim and the agent program the start resolved. The folders
  above them may be listed by name (`file-read-metadata`), not read.
- **Writing:** the project (when it is mounted `rw`), the session's own `home` and `tmp`, the
  extra writable folders, `/dev/null` and the session's terminal. Never the session directory
  itself: that is where the profile of further shells lies.
- **Network:** outbound only to the proxy socket and to `localhost:<port>` of the session; bind
  and inbound only on that port.
- **Masks:** the `masked_files` and the `tmpfs` paths under `/work` that exist in the project
  are neither readable nor writable, and the folders between the project and a mask may not be
  renamed or removed, so `mv .git .gitx` cannot lift the mask of `.git/hooks`. A mask whose path
  does not exist is reported as unprotected, as on Linux.
- **Terminals:** other terminals of the user (`/dev/ttys*`) can be neither read nor written. The
  agent keeps the terminal it inherited, reaches it as `/dev/tty`, and may create new
  pseudo-terminals; their follower side carries the extension `com.apple.sandbox.pty` that only
  it gets.
- **Services:** only the Mach services ordinary tools need for user data, notifications, logging
  and preferences. Not reachable: `mDNSResponder` (DNS), `trustd`, `securityd` (so no Keychain),
  the pasteboard and launchd.

Without mounts the sandbox paths of a profile are mapped: `/work` is the project itself,
`/home/agent`, `/tmp`, `/etc/humanitl` and `/run/humanitl` lie in the session directory under the
runtime directory. The mapping applies to the environment, the command and the text of the
adapter files, so the agent finds its configuration where the adapter put it. It does not reach
into a running program: a `/tmp` that a program hardcodes or a tool call of the agent builds is the
real `/private/tmp`, and reading or writing there ends in `EPERM`. Temporary files belong in
`$TMPDIR`, which is the session's `tmp`. The real `/tmp` is never opened, because it is shared with
every other program of the machine.

### What is different, and what follows

These are the points where macOS gives less than Linux. Each is a channel in
[`THREAT-MODEL.md`](THREAT-MODEL.md) with its consequence.

0. **Session files lie on disk** (HUM-368). On Linux the files of a session are memfds. On macOS
   they lie in the session directory, `0700` under `~/Library/Application Support/Humanitl/run/sessions`,
   the model's access key included, and the directory is removed when the sandbox ends. After a crash
   of the daemon it stays until a later plan clears it, at most seven days, and a backup taken during a
   session can contain it.
0. **A project cannot configure OpenCode** (HUM-371). The managed configuration directory of
   OpenCode lies outside any session on macOS, so the adapter turns the project's own OpenCode
   configuration off with `OPENCODE_DISABLE_PROJECT_CONFIG`. An `opencode.json` in the project has
   no effect in the sandbox there, and neither has its `.opencode` directory or `AGENTS.md`; on
   Linux they count, below the managed file.
1. **Loopback is shared with the host** (K-19). The door listens on a loopback port that every
   local process of every account can reach. Such a process reaches only the proxy, but with the
   session's rules: what an `allow` rule or the LLM passthrough lets through goes out without a
   hold, everything else lands in your queue. The agent in turn reaches no other loopback port than
   the door and the one random port of the session on which it may run its own server (HUM-365),
   so it cannot talk to databases or other services of the host.
2. **There is no PID namespace** (K-20). The command runs in a session of its own; at its end, and
   three seconds after a stop, the shim kills the processes of that session, in passes that repeat
   until one finds nobody (at most ten). This is best effort, not a namespace: a process that
   starts another session outlives the sandbox, and so would one that forks faster than ten
   passes take. Such a process stays under the profile, which allows it the proxy socket of the
   daemon directly: as long as the daemon runs, it can still send requests, the passthrough and
   `allow` rules apply, and everything else is held in the queue after the session has ended.
   On Linux the PID namespace ends every process and seccomp refuses `AF_UNIX`. Restricting the
   proxy socket to the shim of the running session is HUM-331. Until then, after a session on
   macOS that you do not trust, restart the daemon (`humanitl daemon restart`).
3. **A deny list instead of an allow list for reading** (K-21). On Linux the sandbox sees `/usr`,
   `/etc/ssl` and a few files, with a generated `passwd` and `hosts`. On macOS it reads everything
   outside `/Users`, `/Volumes`, `/private/var/folders`, `/private/tmp` and `/private/var/tmp`: all
   of `/etc`,
   `/Library`, `/Applications`, `/opt/homebrew` and `/usr/local/etc` among them. A credential in a
   configuration file there (a database or a Homebrew service) is readable by the agent, and it
   could send it to the LLM passthrough without a hold. Keep secrets in your home directory. The
   profile also names paths: the project directory is resolved and checked before the start, but
   not pinned by descriptor as on Linux (HUM-263).
4. **Seatbelt is deprecated and documented only by example** (K-22). The macOS escape suite runs
   in the CI job `macos` on every commit and against the programs in every release, on the macOS
   of GitHub's `macos-15` runners; it was also run on macOS 27 during development. Other versions
   are not tested. On every version the shim measures the three guarantees at each start, and a
   red line stops the session as on Linux (K-15), so a macOS that no longer refuses what the
   profile denies shows a red isolation check instead of a running agent.
5. **Side channels inside the host that carry no new way out** (K-23): POSIX semaphores and shared
   memory, the system log. None of them leaves the machine.

Two more differences are no channels; they change what a program inside sees, and the briefing
names both:

- **Masks answer `EPERM`.** On Linux a masked file is an empty file; on macOS opening it fails, so
  `cat .git/config` is refused. `git` in the project works all the same: the agent uses the
  session's own `git`, first on `PATH`, while `.git/config` stays masked (HUM-366). The profile is
  not opened for this.
- **No nested sandbox, and a real identity.** `sandbox-exec` inside the sandbox fails, so a tool
  that wants to set up its own sandbox cannot. `whoami` and `hostname` show the real account and
  machine; there is no generated `passwd` or `hosts`.

### The disposable sandbox on macOS (HUM-374)

macOS has no overlay. At the start the daemon clones the opened project folder with
`fclonefileat(2)` to `<runtime directory>/disposable/<id>/upper` (`~/Library/Application
Support/Humanitl/run`, mode `0700`). The clone shares its blocks with the project until something
writes. It sits beside the session directories, not in one, because it has to outlive the sandbox
until a human applied or discarded the changes. The profile makes the clone the writable project;
the real project is denied for reading and writing at the very end of the profile, also where it
lies outside the private trees (such as `/opt`), and `mounts.extra_rw` is read-only as on Linux.
`sandbox changes`, `apply` and `discard`, the changes panel, the conflict check and every rule of
the way back are the same code as on Linux.

**Masks.** `clonefile` copies everything, also `.env` and `.git/config`. So the daemon removes
every present mask from the clone before the sandbox starts: what is not there cannot leak through
a gap in the profile. The profile denies every configured mask in the clone, present at the start
or not, so the agent cannot create anything in its place, and the existing folders above a mask
can be neither renamed nor removed. So in a project without `.git` a disposable run has no working
`git init`. The absence of a mask is not listed as a deletion. Anything found on a masked path, in
any ASCII case, and anything but a directory on the way to one, is refused on apply. A mask that is
a symbolic link takes its target along. The real files stay in the real project, which the agent
cannot reach. A configured mask reached through a symbolic link inside the project cannot be held
in a clone, so the clone counts as failed. In a direct run the profile denies the resolved target
of such a mask as well and locks the folders above it, which also holds when `auto` falls back.

The root of the clone can be neither removed nor renamed, and the daemon opens it only by
descriptor, without following a symbolic link, and compares device and inode with the values taken
right after cloning. Anything else in its place ends in `SANDBOX_031`. Dropping a layer clears file
flags and ACLs first, because the agent can set them and `clonefile` carries them along. A layer
that still cannot be removed is reported, never counted as discarded.

**When the clone fails.** `auto` resolves to disposable on macOS as well, with a writable project.
A clone can still fail: a volume that is not APFS, a project and runtime directory on different
volumes, a file in the project the daemon cannot read (`clonefile` takes the whole tree or
nothing), no room for the clone, a runtime directory inside the project, or a mask behind a
symbolic link. Then the rule of Linux holds: `auto` runs **direct** with the warning `SANDBOX_033`,
which names the reason, and the agent writes to the project folder with none of the protections
of this section. An explicit `disposable` refuses with `SANDBOX_030` and never writes to the
folder. A read-only project also runs direct with `SANDBOX_033`.

**What differs from Linux.**

- **After the end.** There is no PID namespace, so a process can outlive the sandbox (K-20). Before
  anyone reads the clone the daemon renames it (`sealed`). The profile names paths, so the
  survivor loses access through relative paths, absolute paths and the new name alike
  (measured). The profile denies the new name from the start, so this holds wherever the runtime
  directory lies. A descriptor it opened for writing before the end stays usable: such a process can
  still change the bytes of that one file, also after you looked at the diff.
- **No size cap.** `SANDBOX_032` does not exist here. The clone lies on the same volume as the rest
  of the machine, and a guard that measures a whole file system would stop the sandbox for what
  other programs write. The agent can fill the volume, as it can in direct mode.
- **On disk, not in RAM.** The clone survives a crash of the daemon until its next start, which
  removes it. Blocks of the project and of the agent's changes stay on disk until then.
- **What counts as a change.** A file whose kind, size, `mtime` and mode still match the baseline
  counts as untouched and is not read. An agent that restores all of them hides its edit from the
  list; the edit is then not applied, so this loses work and writes nothing. A file the human
  changes during the run is not an agent's change, and an agent's change over it is a conflict.
- **Paths.** The agent works under the path of the clone, not of the project; absolute paths into
  the project, such as those in build caches, do not resolve. A rename that changes only the case
  of a name on a case-insensitive volume is not seen.

### What we verify for macOS

`tests/escape/macos/run.sh` starts a daemon in an empty home and runs `humanitl sandbox check`,
whose shim lines assert `EPERM` for the probes listed in the table above. Then it tries every way
out the profile must refuse from inside a real sandbox and counts a probe as refused when the
command fails (the shim's lines are the ones that name the errno): direct TCP and TLS,
DNS, UDP, IPv6, raw and routing sockets, `syslog` and `mDNSResponder`, another loopback port,
reading the home directory, `/Users`, `/private/var/tmp`, the masked `.env` and `.git/config`,
writing `.git/hooks`, `/private/tmp`, the real `/tmp` and `/Library`, a nested `sandbox-exec`,
rewriting or removing the profile, writing the adapter files, renaming `.git`,
opening another terminal. It checks that a background process ends with the sandbox, that the
agent can still create a pseudo-terminal and write to `$TMPDIR`, that `git` works in a repository
created inside the sandbox, that `.git/config` of a real repository stays refused, that the adapter
files arrive,
that a read-only project can be read and not written, and that a request through the door arrives at the
daemon as a flow (that the proxy holds it is the subject of the Linux suite, ESC-4). It runs in the CI job `macos` and in the release job `check-macos` against
the binaries inside the disk image.

| 0.1.0 | The disposable sandbox: the project folder as the read-only lower layer of an overlay, the upper layer in RAM under the runtime directory with a size cap, `mounts.extra_rw` read-only; changes are written back only on apply, never through a symlink, never as a symlink out of the project, a device or with setuid bits, and never over a file the human changed since the start without `--overwrite` (HUM-350, K-24). |
| 0.1.0 | The changes panel of the app: a conflict is overwritten only when it was confirmed for exactly that change (`ApplyChanges.overwrite_ids`); a conflict that appears after the selection stops the apply (HUM-351, K-24). |
| 0.1.0 | A project profile, a session profile or one run (`humanitl run --llm NAME`) chooses one of the saved model servers with `llm.server`; only the chosen server is the session's passthrough and gets its key; an unknown name refuses the start with `CONFIG_021` and never falls back to `llm.active`; the choice and its source stand in the sandbox header and in `humanitl sandbox plan` (HUM-400, K-02). |
| 0.1.0 | A project that chooses a saved server other than the default starts only after the human confirmed that server for that project folder once (`CONFIG_022`); the confirmation lives in the daemon's data directory, never in the project; the daemon writes the project file without following a link (HUM-405, K-02). |
| 0.1.0 | The disposable sandbox is the default where the system allows it: `sandbox.mode = "auto"` resolves to disposable with bubblewrap 0.11 or newer, a runtime directory on tmpfs and a writable project, and otherwise runs direct, as before, with the warning `SANDBOX_033`; an explicit `disposable` still refuses with `SANDBOX_030` and never falls back (HUM-357, K-24). |
| 0.1.x | OpenCode reaches a model on the loopback through the proxy: the adapter writes the reserved name `llm.humanitl.internal` into the `baseURL`, and the proxy maps that name only to the configured loopback endpoint with its scheme and port, before the rules; any other use of the name is refused with `PROXY_003` and never resolved ([section 5.1](#the-reserved-name-llmhumanitlinternal-hum-387), HUM-387). |

## 13. Linux: Podman

**Admitted since HUM-448.** Podman is a second sandbox backend on Linux (ADR-0020), first as the
plain level: rootless Podman with the OCI runtime `crun`, namespaces over the shared host kernel,
the same isolation level as bubblewrap. It was admitted only after the escape suite against it
(the CI job `escape-tests-podman`, ESC-1, ESC-2 and ESC-5) was green on `main` together with the
mount check (HUM-437) and the user's service manager (HUM-434). A start with
`sandbox.backend = "podman"` runs a session, whether the choice comes from `config.toml`, a
profile, the environment, `--backend` or the start of a session; a project profile cannot choose
the backend. `auto` stays bubblewrap: Podman runs only when a person chooses it. The kernel level,
the backend `qemu` (ADR-0021), is not admitted yet and every start with it refuses with `SANDBOX_048` until
HUM-449. This section states what the adapter (`daemon/crates/sandbox/src/podman/`, HUM-433)
does.

### The three guarantees under Podman

| # | Guarantee | How it holds under Podman | Checked by the shim as |
|---|---|---|---|
| 1 | No network interface | `--network=none`: a network namespace with only `lo`, no `pasta` or `slirp4netns` | `no_interfaces`, unchanged |
| 2 | Exactly one door | the proxy socket bound as one file to `/run/humanitl/proxy.sock`, read-only; `conmon`'s attach socket and exit file stay on the host side; the mount table equals the plan | `single_socket` and `bridge_listening`, unchanged, and `mounts_match` and `mounts_pinned` as under bubblewrap |
| 3 | No new doors | the same shim is PID 1 and installs the same filter before the agent's `exec`; Podman's own seccomp profile (the daemon's copy of the default profile of `containers-common`) stays as an outer layer, with `--cap-drop=all` and `no-new-privileges` | `seccomp_applied` and `families`, unchanged |

Fixed in every line, with no key to change it: `--rm`, private network, IPC, UTS, PID and cgroup
namespaces, `--userns=keep-id`, `--read-only` without the engine's tmpfs on `/run`, `/tmp` and
`/var/tmp`, `--log-driver=none` (the agent's output never reaches a log), `--pull=never`,
`--no-hosts` and `--passwd=false` (the profile's identity files instead), `--env-host=false` and
`--http-proxy=false` (without it Podman copies the daemon's proxy variables, credentials included,
into the container), `--memory`, `--cpus` and `--pids-limit` from `sandbox.podman`. No `-e`, no
`-t`, no `--privileged`, no other network, no added capability, no device, no relabelling.
Podman mounts the host's `/sys` read-only into every container, with its own `/sys/fs/cgroup`
beneath; bwrap gives none. The adapter lays an empty read-only tmpfs over both (the cgroup limits
still hold, they do not depend on the mount), so the agent sees no more of the host than under bwrap
(`esc-2/no_host_sys`, HUM-453).

### Configuration the daemon does not write is cut off

Podman reads much more than its command line: `containers.conf` in every layer, `mounts.conf`
(which mounts into every container, for example host secrets under `/run/secrets`), OCI hooks,
`storage.conf`, `registries.conf`, and remote connections. The adapter starts Podman with an empty
environment except `HOME`, the user's own `XDG_RUNTIME_DIR` (never Humanitl's), `PATH`, the user's session bus if there is one, and three files it writes itself under the
runtime directory (`CONTAINERS_CONF` with an empty `default_mounts_file`, an empty `hooks_dir` and
its own `seccomp_profile`; `CONTAINERS_STORAGE_CONF`; `CONTAINERS_REGISTRIES_CONF`). The line
that starts a container (`podman run`) also carries `--hooks-dir` on an empty directory and
`--default-mounts-file` on an empty file. Only that global flag keeps the user's
`~/.config/containers/mounts.conf` out: Podman reads that file in addition to the
`default_mounts_file` of the configuration, which is defense in depth only (measured on Podman 4.9.3
and 5.8.6, HUM-454). `CONTAINERS_CONF_OVERRIDE`, `CONTAINER_HOST` and `CONTAINER_CONNECTION` never
reach it. A Podman that runs as root, or that is only the client of a remote service such as a
Podman machine, is refused with `SANDBOX_038`.

### What is different from bubblewrap

- **Mounts by path: a swap is detected and refused, not prevented.** Podman takes no descriptor
  as a mount source (measurement P13 in HUM-292: `crun: mount /proc/self/fd/3 to work: Invalid
  argument`; measured again with the adapter's own line in HUM-437: no container starts, the shim
  reports nothing, the agent never runs). The plan still opens and checks every source once, as under bubblewrap, and mounts
  the place the opened object lies at. A swap of that path between the plan and the mount does
  reach the container; the shim's check of device and inode after the mount
  ([section 3](#3-guarantee-2-exactly-one-door), HUM-437) detects it before the agent exists and
  refuses the start with `SANDBOX_042`. At that moment only the shim runs in the container. Bind
  mounts keep the device number of their source (P14). A path with a comma, a quote or a line
  break is refused (`SANDBOX_006`): mount options are separated by commas.
- **The engine's own mounts are part of the expected table.** With the fixed flags above Podman
  adds `/proc` with its read-only self-binds and masked paths, `/dev` with `pts`, `mqueue` and the
  host's device nodes, `/run/.containerenv` and `/etc/hostname`, and no `/etc/resolv.conf`
  (measurement P17; the list in `podman/mounts.rs` is the measured one). Everything else a
  configuration source adds, from `mounts.conf`, `containers.conf` or a newer Podman, is an extra
  entry and refuses the start with `SANDBOX_049`, whether it carries a socket or not. The cut-off
  configuration below keeps such entries out; the table check is the barrier when it does not.
- **Masks and files on disk.** Masks, the identity files and the adapter's files are files in the
  session directory under the runtime directory, not memfds. The adapter's files are mode `0400`
  and may carry the model's access key; the plan refuses to write them when the runtime directory
  is not on tmpfs, and the directory goes with the sandbox. Masks and tmpfs under `/work` are
  mounted only on paths that exist, as under bubblewrap: a missing mount point would be created in
  the project.
- **The environment through a descriptor only.** Podman stores every `-e` value in the container's
  configuration on disk (P11). The agent's environment therefore travels only in the sealed memfd
  of HUM-430 (`--env-fd`). The five variables of the shim's own contract (bridges, seccomp lists,
  the report's descriptor number) carry no secret and stand in the session's `containers.conf`,
  and with it in the stored container configuration.
- **Ending the sandbox.** The container runs detached from the `podman run` the daemon starts
  (P7), and PID 1 ignores a signal without a handler (P5, P6). The daemon ends it through the
  engine by name: `podman kill --signal TERM`, after the grace `--signal KILL` and
  `podman rm --force`. When the daemon dies, the lifeline of HUM-430 ends PID 1. When
  `podman run` ends without a requested stop, its `--rm` no longer applies: the lifeline ends PID 1
  within `KILL_GRACE`, and the daemon removes the ended container through the engine right away,
  with a warning `SANDBOX_041` if that fails; a concurrent stop and this removal never both remove
  (HUM-452). A container a crash still leaves behind carries the label of the daemon's instance and is removed at the next
  daemon start, with a warning `SANDBOX_041`.
- **The trusted computing base grows** by Podman, `conmon`, `crun`, the user's container storage
  and the daemon's `containers.conf` (K-27 in [`THREAT-MODEL.md`](THREAT-MODEL.md)).
- **No disposable sandbox.** Its lower layer cannot be bound to the checked project folder;
  `disposable` refuses with `SANDBOX_030`, `auto` runs direct with `SANDBOX_033`
  ([section 5.2](#the-disposable-sandbox-hum-350), HUM-438).

### An image as the root (HUM-439)

With `sandbox.podman.rootfs = "image"` the root of the sandbox is the image named in
`sandbox.podman.image`, always with its digest (`name@sha256:` and 64 hex digits; without it the
configuration is refused with `CONFIG_003`).

- **No start pulls.** The line keeps `--pull=never`, and before every start the plan asks the
  engine for exactly that digest (`podman image inspect`). A missing image, or another digest under
  the same name, refuses the start with `SANDBOX_043`, and its fix is the command that pulls. The
  container starts from the reference with its digest, so the engine resolves it by content, not
  by a name under which tomorrow another image could stand.
- **Pulling only on request.** `humanitl sandbox image pull [REFERENCE]` (RPC
  `SandboxRequest.image_pull`) is the only way the adapter pulls. The engine then uses the network
  of the daemon directly: not through the Humanitl proxy, without its rules, its hold and its
  recording, and without the proxy variables of the host. The command line and the interface say
  so before it runs. The registry is the one in the name: a short name without a registry is
  refused, and the daemon's own `registries.conf` has no search list and no mirror. The signature
  policy is the daemon's own file (`--signature-policy`) and accepts any image: the trust anchor is
  the digest the person named, not a signature. Under the kernel level, which is not admitted
  yet, the pull is refused with `SANDBOX_048`, like every start.
- **The image is part of the trusted computing base.** Whatever it holds runs as the agent, under
  the agent's filter. The shim runs from the host's file, statically linked in a release, so no
  library of the image runs before the filter is in place. Project folder, door, CA certificate and
  bundle, shim, the adapter's files and the session directory come from the host as without an
  image, never from the image: a CA, a key or a session directory never stands in an image. ESC-2
  checks that the CA in the sandbox is the daemon's file by device and inode
  (`esc-2/ca_is_the_hosts`), under an image too (`HUMANITL_ESCAPE_IMAGE`).
- **What the image does not decide.** The line sets the user (`--user` with the user's own IDs, as
  `--userns=keep-id` does without an image), the shim as the entry point, `--image-volume=ignore`
  (a `VOLUME` of the image would be a mount the plan does not know) and `--no-healthcheck`. The
  engine lets the image's environment win over the daemon's `containers.conf`, where the shim's
  variables stand; an image whose environment sets a name that starts with `HUMANITL_` is refused
  with `SANDBOX_043`. The agent's environment comes from the memfd as always, and the shim clears
  everything the engine put there.
- **The profile's `mounts.ro` and symbolic links do not apply**: the image brings its own `/usr`
  and `/etc`. The expected mount table holds the root as exactly one read-only `overlay`; its flags
  stay open, because the overlay has no counterpart on the host to read them from. The preview
  shows the reference with its digest, and the mount table names the root "image".
- **Residual risk.** The digest check reads the engine's metadata; it does not hash the unpacked
  layers again, so a process of the same account that writes into the user's container storage can
  change the content under an unchanged digest (K-28). The image's annotations reach the engine.
  An agent from the host bound into an image may miss its libraries (`AGENT_005`).

### Prerequisites

Podman 4.9 or newer with `crun` (`SANDBOX_037`), rootless and local (`SANDBOX_038`), subordinate
IDs for `--userns=keep-id` (`SANDBOX_039`), the cgroup controllers `memory`, `pids` and `cpu`
delegated (`SANDBOX_040`), no enforcing SELinux (`SANDBOX_044`, a mount would need relabelling the
project). A missing prerequisite refuses the start with its code; the start never falls back to
bubblewrap.

## 14. Linux: QEMU micro-VM (kernel level)

**Not admitted yet.** The kernel level, the backend `qemu` (ADR-0021), runs the agent in a micro-VM
with a guest kernel of its own. It is not in the list of admitted levels: every start with
`sandbox.backend = "qemu"` refuses with `SANDBOX_048` until HUM-449, after the escape suite against
it with root in the guest (HUM-445) is green on `main`. `auto` never chooses it. Until then only
the escape suite and the tests reach the adapter (`daemon/crates/sandbox/src/qemu/`, HUM-444),
through a bypass that the shipped daemon and command line do not contain. This section states what
the adapter does, so that the claim can be checked before the level is offered.

### The three guarantees in the micro-VM

The guarantees have to hold with root in the guest: the agent's kernel is not the host's, and
whoever gets root in the guest controls that kernel. The checks inside the guest are therefore
defense in depth; what carries each guarantee lies on the host.

| # | Guarantee | How it holds on the host | Checked by |
|---|---|---|---|
| 1 | No network interface | the machine has no network device (`-nic none`, no `-netdev`); the session's unit runs with `PrivateNetwork=yes`, so every helper process sees only `lo`; no process of the unit holds `/dev/net/tun`, `/dev/vhost-net` or `/dev/vhost-vsock` | the shim's `no_interfaces` in the guest, and the host check: the network namespace of every process of the unit, the descriptors of every process, the running emulator's own command line |
| 2 | Exactly one door | one vsock device process per session (never shared, HUM-442 Q9) with one device, `--uds-path` in the session's runtime directory and never `--forward-cid`; on the host exactly one listener `<uds>_5000`, which the daemon forwards to the session's proxy socket; every other port of the host finds no listener; the guest root is a read-only share in which no runtime directory and no host socket stands | the shim's `single_socket`, `bridge_listening`, `mounts_match` and `mounts_pinned` in the guest, and the host check: only the declared sockets in the runtime directory, exactly one vsock device process with exactly one device, the emulator's descriptors limited to those of the plan without a console |
| 3 | No new doors | the guest kernel loads only the plan's modules and then locks module loading (`kernel.modules_disabled = 1`, automatic loading off before that); no device can be added to a running machine (no monitor, no QMP); the emulator runs under its full `-sandbox` and in a jail without capabilities | the shim's `seccomp_applied` and `families` in the guest; the agent's filter refuses `AF_VSOCK` and `AF_UNIX` |

**The agent starts only when the host admits it.** In the guest, PID 1 writes every `CHECK` line
and then holds the agent at its exec gate until the daemon answers on the report connection, which
the host opened. The daemon reads the guest's lines, runs the host check and sends `ADMIT` only when
both pass; otherwise it sends `DENY` and stops the unit. Without an answer within 120 seconds the
shim refuses on its own. So no failed check, from the guest or from the host, ever leaves an agent
running even for a moment. This gate exists only in the guest; under bubblewrap and Podman the
agent starts as before and a red check stops the session afterwards (K-15).
A guest that ends without the shim's report is a failed check with its own reason, never a pass.
A failed host check is `SANDBOX_047`: the agent is not admitted and the session is stopped.
When the guest has already refused on its own (a red line in its report, or no report at all), its
diagnosis stays the one the start shows, and the host's findings stand in the evidence: the shim
then stops and powers the guest off, the unit ends with it, and a host check that finds a unit
without a cgroup or without its emulator sees the consequence of that refusal, not its cause
(HUM-470). The verdict never depends on this order; only the cause shown does. The host opens the
two connections into the guest only after the shim has said on the door, with one fixed line on a
connection of its own, that it listens on the report port (HUM-471). A `CONNECT` that reaches the
vsock device while the guest's driver is still setting it up (the guest's memory known, its queues
not yet ready) stops `vhost-device-vsock` 0.3.0 for good: no later connection gets an answer, and
the guest powers off without its report. Only a connection the guest makes itself proves that the
device runs. Until that line the daemon takes the door's connections for itself and hands none to
the proxy; no agent can run before it, since the agent waits for the admission on the report
connection. When the report is missing, the evidence names how far the way to the guest got (no
ready line, or no answer to the report or the control connection). The host opens each
connection into the guest once and waits for its answer until the session's deadline: a
`CONNECT` it gave up could still be accepted by the guest later and take the place of the report
(HUM-470, a precaution: the vsock device refuses within milliseconds while the guest does not
listen).

### How a session is built

- **One hardened unit per session.** The session starts as a transient unit of the user's service
  manager (`systemd-run --user`, HUM-434) with the properties of HUM-442 Q7: `PrivateNetwork`,
  `PrivateUsers`, `NoNewPrivileges`, an empty capability bounding set, `ProtectSystem=strict`,
  `ProtectHome=read-only` with only the runtime directory of the session and the project folder
  writable, `RestrictAddressFamilies=AF_UNIX AF_NETLINK`, `LockPersonality`, `RestrictRealtime`,
  native system calls only.
- **Four processes, four jails.** The file system daemon of the guest root, the file system daemon
  of `/work`, the vsock device and the emulator each run in their own bubblewrap jail with every
  namespace unshared and every capability dropped. No jail binds the session's runtime directory
  itself: each socket lives in a subdirectory of its own (`root/`, `work/`, `vsock/`, `door/`),
  and each jail binds only what it needs. The `/work` daemon binds `work/`, the vsock device
  binds `vsock/` and `door/`, and the root daemon binds none, since it gets its socket as a
  descriptor. The emulator binds `root/`, `work/` and `vsock/` read-only, enough to connect to the
  three vhost-user sockets; it never sees the door, the handover socket or the guest's files. Beyond
  those it holds only `/usr`, `/dev/kvm` and the one kernel file from `/boot`; it has no console,
  monitor or display.
- **The guest root comes from the same plan as under bubblewrap.** The profile's mounts, the shim,
  the CA certificate and bundle, the identity files and the adapter's files are built into a
  subdirectory of the root daemon's jail and shared read-only; the jail's own `/proc` is outside
  that subdirectory. The root daemon gets its socket as an inherited descriptor, so the runtime
  directory never stands in the guest root. Mount points inside the share are not announced to the
  guest (`--no-announce-submounts` where the file system daemon knows it), and the shim's expected
  mount table holds exactly one virtiofs mount at `/` and one at `/work`.
- **The file system daemons hold a fixed number of descriptors** (HUM-469). The unit carries
  `LimitNOFILE=65536`, soft and hard, for every process of the session. A file system daemon
  tries to raise its soft limit, fails at that hard limit and takes it; raising the hard limit
  needs `CAP_SYS_RESOURCE`, which no jail has. Above the limit the daemon answers the guest with
  `ENFILE` and keeps running, and what the guest releases it can take again. The daemons do not
  get `--rlimit-nofile`: systemd lowers `LimitNOFILE` to the service manager's own hard limit
  when that is smaller, and the unit starts with the smaller, still fixed limit, whereas a daemon
  asked for more than it inherited would exit and leave the guest without a root (and the start
  with a report-less check instead of the cause). Both daemons run with `--cache never`: the
  guest drops every entry as soon as nothing holds it and tells the daemon to forget it, so a walk
  over the whole root leaves only the descriptors of what the guest still holds open (measured:
  98 after a walk over 735,790 entries; before, a walk drove the root daemon to about 520,000,
  HUM-445). Copying, reading, searching and `git` over 13,475 files and 300 program starts held at
  most 131, so the limit leaves ample room, and it is an eighth of the hard limit the service
  manager otherwise hands out, so the host memory a guest can pin through a daemon stays bounded.
  The price is speed on metadata-heavy work: without the guest's cache such work took two to
  sixteen times as long in that measurement (`ls -lR` over `/usr/share/doc` 33 s instead of 2 s).
- **Shared mappings without the guest's page cache** (HUM-469). `--allow-mmap` keeps
  `mmap(MAP_SHARED)` working on `/` and `/work` (rustc and cargo need it); the guest kernel must
  be 6.6 or newer, and a kernel whose configuration names an older version is refused with
  `SANDBOX_045`. A configuration without the usual version header starts, and `humanitl doctor`
  says the version is unknown. Caveat on data coherency, not a security boundary: virtiofsd
  advises this option only when the guest has the directory to itself. When the user edits a
  file under the project on the host while the guest has it mapped shared, the mapping in the
  guest can show stale pages. The guest sees nothing it was not given either way.
- **The project folder is the checked one.** The daemon opens and checks the folder, hands the
  descriptor to the unit and the `/work` daemon's jail binds exactly that descriptor
  (`--bind-fd`); a swap of the path after the check does not reach the guest (HUM-442 Q4). The
  profile's masks under `/work` lie over that share in the jail, so the guest sees them as under
  bubblewrap. The shim in the guest compares the inode of `/work` with the plan.
- **Nothing is inherited.** The unit is no child of the daemon. Its first process is the shim on
  the host (`--fds-then-exec`): it takes three descriptors over a one-connection handover socket of
  the session (the root daemon's socket, the project folder, the read end of the lifeline) and runs
  the fixed script that starts the four jails.
- **The end of the daemon ends the session without the guest.** The script waits on the
  lifeline, whose write end only the daemon holds; when the daemon ends, also by `SIGKILL`, the
  script ends and the service manager stops every process of the unit (`KillMode=control-group`).
  When the daemon runs as a service, the unit is also bound to it (`BindsTo`). Root in the guest
  can ignore the end of the control stream; that lifeline inside the guest is defense in depth
  only. A stop of a session stops the unit and removes its runtime directory; what a crash leaves
  behind is stopped and removed at the next daemon start (`SANDBOX_041`).
- **Inside, the guest looks like a bubblewrap sandbox** (HUM-445). PID 1 is root in its own
  kernel; the agent and every further shell are not. Before the filter each gives up every
  capability, the bounding set and the ambient set included, sets `no_new_privs` and becomes the
  user that bubblewrap maps into its sandbox (the user of the daemon, as the guest's
  `/etc/passwd` says), and the shim proves the result from the kernel before it goes on. PID 1 is
  not dumpable, so the agent can neither read nor write its memory. After the module lock `/sys`
  is an empty read-only tmpfs instead of the guest kernel's sysfs, `/dev/shm` is a tmpfs as
  under bubblewrap, and the host name is the profile's (`sandbox`). Every other tmpfs of the
  profile outside `/work` (`/tmp`, `/var/tmp`, the agent's home) is a tmpfs in the guest as well,
  and so is `/run`, which bubblewrap creates writable on its own root; the shim mounts them before
  the agent starts, with mode 0755, owned by the agent's user, `nosuid` and `nodev` (HUM-466).
  Each holds at most the kernel's default of half the guest's memory, less than under bubblewrap,
  where the default is half the host's. A file of the agent's adapter under the home lies
  read-only on top of its tmpfs as under bubblewrap, and `/run/humanitl` (shim, guest plan,
  modules) stays read-only on top of `/run`: the shim takes a copy of the mount in the read-only
  root before the tmpfs hides it and moves the copy back on top. The expected mount table holds
  all of these, and the socket walk of `single_socket` covers every tmpfs.

### What is different from bubblewrap

- **The trusted computing base grows** by the emulator, the guest kernel, the two file system
  daemons, the vsock device and the user's service manager (K-30 to K-33 in
  [`THREAT-MODEL.md`](THREAT-MODEL.md)).
- **The guest root is read-only; writable extensions are refused.** The project at `/work`, the
  tmpfs of the profile and `/run` are the only writable places in the guest. A profile with
  `mounts.extra_rw` does not start (`SANDBOX_051`, HUM-466) rather than getting the folder
  read-only without a word: a writable extension would be another writable way to the host that
  nothing reviews. The rest of the root, such as `/etc` beside its binds, stays read-only in the
  guest, where bubblewrap's own root tmpfs is writable; that is stricter, not a gap: nothing
  written there reaches the host or outlives the session under either backend.
- **No disposable sandbox yet.** `disposable` refuses with `SANDBOX_030` until HUM-446.
- **The terminal travels as a stream** (K-29); a signal of the host reaches no process of the
  guest, an interrupt is a frame.

### Prerequisites

`/dev/kvm` readable and writable, the emulator with `microvm` and `vhost-user-vsock-pci`, the file
system daemon with `--readonly`, `--fd`, `--cache` and `--allow-mmap`, the vsock device with
`--uds-path` and `--groups`, bubblewrap with `--bind-fd`, a readable kernel image with virtiofs
built in (`CONFIG_VIRTIO_FS=y` in the configuration beside the image) of version 6.6 or newer
and the module directory of that kernel. A missing prerequisite refuses the start with `SANDBOX_045`; the start never falls back to
another backend. `humanitl doctor` shows one line per prerequisite.
