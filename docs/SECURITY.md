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
([section 5.6](#56-other-known-limits), item 12). The claim rests on
three guarantees. They stand word for word in the isolation panel of the application (ARB keys
`isolationCheck1` to `isolationCheck3`) and in `BACKLOG.md` 4.1:

| # | Guarantee | Checked by |
|---|---|---|
| 1 | **No network interface. There is nowhere for traffic to go.** | [section 2](#2-guarantee-1-no-network-interface), ESC-1, ESC-3 |
| 2 | **Exactly one door: a socket that leads to Humanitl.** | [section 3](#3-guarantee-2-exactly-one-door), ESC-2 |
| 3 | **The kernel opens no new door (seccomp).** | [section 4](#4-guarantee-3-no-new-doors), ESC-1 |

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
| system keyring (Secret Service) | the LLM access key (HUM-140) | `secret-tool` from `libsecret-tools`; without an answer `$XDG_DATA_HOME/humanitl/keys/llm.key`, `0600` in `0700` | protects against other accounts, not against the same account; the agent reads the key inside the sandbox because it needs it |

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
[`CONFIG.md`](CONFIG.md).

Toolchain for reproducible builds: Rust 1.95.0 (pinned in `daemon/rust-toolchain.toml`,
minimum 1.88), Flutter 3.47.2 (pinned in `app/.fvmrc`), `bubblewrap` 0.8 or newer. `socat` is
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
argument list (`humanitl sandbox argv`). In short, from `profiles/sandbox/default.toml`:

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
   gate and the bridge listeners, creates the `socketpair` over which the filter's listener will
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
  agent ends, the shells die with the thread that forked them (`PR_SET_PDEATHSIG`) and with the
  PID namespace, and the daemon ends their terminals when its end of the pair closes.
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
(`LLM_005`, one per request, with count and host and never with the value found) but does not
hold. One exception: with `hold.hard_block_checksum_secrets` on, the passthrough blocks a request
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
(`secret-service:humanitl/llm` or `file:llm`). It is set through the RPC `LlmKey`, from the form
or with `humanitl llm key set`, which reads it from `stdin`; no answer of the daemon carries it
back.

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

*How it reaches the sandbox.* When a session starts, the daemon fetches the key and places it as
the file `/etc/humanitl/opencode/llm-api-key` in the sandbox, from a memfd through
`--ro-bind-data`. `opencode.json` refers to it with `{file:/etc/humanitl/opencode/llm-api-key}`.
With `agent.adapter = "pi"` the same mechanism places it at `/etc/humanitl/pi/llm-api-key`, and
pi's `models.json` refers to it with `"apiKey": "!cat /etc/humanitl/pi/llm-api-key"`, which pi
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
searched (HUM-247). A key written differently from literal or URL-encoded (a JSON escape, `+` as
a space in a form) is no hit.

*What stays open.* The agent can read the key; it needs it. A subverted agent can therefore put it
into a request to another destination. There it is a finding, except in the path before `?` and
in the host; the request is held only at a host without an allowing rule or with
`hold.hard_block_checksum_secrets`. A key that leaves re-encoded (Base64, split, encrypted) is no
literal hit. Whoever controls the same user reads the key with `secret-tool lookup` themselves; the
keyring protects against other users, not against this one. *What to do:* create a key with as
few rights as possible, revocable at the provider, and install `libsecret-tools` so it does not
end up in the file.

### 5.2 The project directory `/work`

*Why it exists.* An agent that may not write is a chat window.

*What Humanitl does.* Only a subpath is bound, never `$HOME`. The folder lies below `$HOME` or
is the one a person declared in `config.toml`; a stored folder gets this check at every start
against the path it resolves to then, and a declaration outside `$HOME` holds only for the path
its text really names, so a link put into a parent directory later cannot move the project
elsewhere (HUM-294). `sandbox.work_mode = "ro"` is
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
repositories with `--work-mode ro`.

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
registers no OSC handler of its own.

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

Nothing in this document has to be believed. Three ways to look, from the quickest to the most
thorough:

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
`escape-tests` on every issue branch and on `main`, and locally with `make escape`:

| ID | File | What it shows |
|---|---|---|
| ESC-1 | [`esc-1-sockets.sh`](../tests/escape/esc-1-sockets.sh) | socket families and types, `io_uring`, x32, interfaces, routing, capabilities, `Seccomp: 2` for every process, PID 1, refusals reported |
| ESC-2 | [`esc-2-mounts.sh`](../tests/escape/esc-2-mounts.sh) | mount surface, exactly one socket, no host `/proc`, hostname |
| ESC-3 | [`esc-3-egress.sh`](../tests/escape/esc-3-egress.sh) | no egress without the proxy; through the proxy every request no rule decides lands in the queue (`allow` rules and the LLM passthrough skip it); no DNS before the decision, proven on the host at the log of a recording name server |
| ESC-4 | [`esc-4-rules.sh`](../tests/escape/esc-4-rules.sh) | the rule table including homograph, IP literal, WebSocket and body cap |
| ESC-5 | [`esc-5-filesystem.sh`](../tests/escape/esc-5-filesystem.sh) | symlink marking, masked paths, OSC 52, audit tampering |

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
| 0.1.x | Further shells next to the agent: the daemon asks PID 1 over an unnamed `SOCK_SEQPACKET` pair only it and PID 1 hold; the shim forks the shell from inside with the agent's filter on top, proven by a refused `AF_UNIX`; at most four, ending with their process or the sandbox (HUM-299). |
