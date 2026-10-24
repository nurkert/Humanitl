# Humanitl

Humanitl lets a sandboxed AI coding agent use the internet, one approved request at a time.

![The intercept screen in the dark theme: the queue of held requests on the left, the selected request with headers and body in the middle, the domain panel on the right. Rendered from a golden test, so text appears as blocks.](docs/img/intercept.png)

<p align="center">
  <a href="https://github.com/nurkert/Humanitl/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/nurkert/Humanitl/actions/workflows/ci.yml/badge.svg"></a>
  <a href="LICENSE"><img alt="License: GPL-3.0-only" src="https://img.shields.io/badge/license-GPL--3.0--only-blue.svg"></a>
  <img alt="Platform: Linux" src="https://img.shields.io/badge/platform-Linux-lightgrey.svg">
</p>

The agent runs in a sandbox that has no network interface. Its only way out is a moderating proxy
on the host. A request that no rule decides is held, shown to you with headers, parameters and
body, and the proxy forwards it upstream only after you allow it. Two kinds of traffic go without a
hold: requests an `allow` rule matches, and the declared passthrough to your language model.
Everything is recorded, except during a recording outage (disk full, database locked): then
traffic continues unrecorded and the app says so on every card.

## Why

Local language models are now good enough for real work, but they know less by heart and need to
look things up. An agent working on sensitive client data can put that data into a request, through
prompt injection, a malicious package or a manipulated model. Until now the only safe answer was no
internet at all; Humanitl makes internet access possible under human control, traceable and
recorded.

## The three guarantees

The security argument fits in three sentences, and each one can be checked from inside the running
sandbox (`BACKLOG.md` 4.1):

1. **No network interface. There is nowhere for traffic to go.** `bwrap --unshare-all`: only `lo`
   exists, no outside address, no route, no DNS, no ICMP, no QUIC, no raw sockets.

   ```sh
   ip link        # shows only lo
   ```

2. **Exactly one door: a socket that leads to Humanitl.** The daemon's proxy socket is bound into
   the sandbox as a single file.

   ```sh
   find / \( -path /proc -o -path /sys -o \( -path '/dev/*' ! -path /dev/shm ! -path '/dev/shm/*' \) \) -prune -o -xtype s -print 2>/dev/null
   # exactly one line: /run/humanitl/proxy.sock
   ```

   `-xtype s`, not `-type s`: bwrap binds the proxy socket over an empty regular file, and `find` trusts the directory entry for a bare `-type`; and no `-xdev`, because `/work`, `/tmp`, `/dev/shm` and `/home/agent` are separate mounts where a planted socket must show up.

3. **The kernel opens no new door (seccomp).** The shim installs a filter before it starts the
   agent: `socket()` only for `AF_INET`/`AF_INET6` with `SOCK_STREAM`, everything else `EPERM`,
   plus `ptrace`, `io_uring_*`, `process_vm_*`, `keyctl` and every x32 syscall refused.

   ```sh
   grep Seccomp /proc/self/status   # Seccomp: 2
   ```

`humanitl sandbox check` runs all three in a short-lived sandbox, and the application shows them as
the isolation ring. The full argument is in [`docs/SECURITY.md`](docs/SECURITY.md).

## What it does not protect against

Read [`docs/THREAT-MODEL.md`](docs/THREAT-MODEL.md) before relying on Humanitl. Five side channels
are declared, shown in the app and not closed. Two of them are open data paths out of the sandbox:

- **The LLM passthrough.** Your language model sees every prompt the agent sends it, without a hold;
  that host is part of what you trust.
- **The project directory `/work`.** It is writable; what the agent leaves there, a human may carry
  out later.

The other three carry no new data path, but you should know them: **terminal output** comes from the
agent and is filtered, not trusted; **host names** stand in plain text in the recording and in every
export; **package caches** are per project and carry data between sessions of that project.
Finally, **you** are the classifier: an approved request goes out, so read what you approve
([`docs/SECURITY.md`](docs/SECURITY.md#5-declared-side-channels)).

## Install

Releases are on the [releases page](https://github.com/nurkert/Humanitl/releases) as a `.deb`
and an AppImage. Linux with unprivileged user namespaces, `bubblewrap` 0.8 or newer and a systemd
user session are required.

```sh
# Debian 13 / Ubuntu 24.04
sudo apt install ./humanitl_<version>_amd64.deb
humanitl daemon install        # runs: systemctl --user enable --now humanitld.socket

# AppImage
chmod +x Humanitl-<version>-x86_64.AppImage
./Humanitl-<version>-x86_64.AppImage --cli daemon install

humanitl daemon status
humanitl sandbox check
```

Everything else, prerequisites, the agent, updates and removal, is in
[`docs/guides/install.md`](docs/guides/install.md).

## First session

1. Open the app; the setup screen checks the background service, the sandbox and the model server.
2. Enter your model server and test it ("Contact it").
3. Choose the project folder the agent may see.
4. Start the session and read the isolation check: three green lines and one amber line for the
   model.
5. Decide the first held request: allow, block, or remember it as a rule.

Step by step, with screenshots: [`docs/guides/first-session.md`](docs/guides/first-session.md).

## CLI

The command line is a thin client of the same daemon as the app:

```sh
humanitl run --profile llm-only                            # a session that may reach only the model
humanitl rules test https://registry.npmjs.org/left-pad    # which rule decides; exit 10 block, 11 ask
humanitl audit verify                                      # check the hash chain with key and anchors
```

Every command and flag: [`docs/reference/cli.md`](docs/reference/cli.md). Configuration keys:
[`docs/CONFIG.md`](docs/CONFIG.md). Rules: [`docs/reference/rules.md`](docs/reference/rules.md).
Diagnostic codes: [`docs/DIAGNOSTICS.md`](docs/DIAGNOSTICS.md). All documentation:
[`docs/README.md`](docs/README.md).

## How it works

```
+--------------------------------+      gRPC over UDS       +------------------------------------+
| humanitl (Flutter, GTK)        | <----------------------> | humanitld (Rust, systemd --user)   |
|  Intercept · History · Rules   |  Stream: FlowEvent       |  proxy     hold queue, MITM        |
|  Sandbox · Audit               |  Cmds: Decide, Rules,    |  rules     pure matching logic     |
|  Terminal (xterm via stream)   |        Sandbox, Query    |  findings  regex detectors         |
+--------------------------------+                          |  recorder  SQLite + blobs          |
                                                            |  audit     hash chain              |
                                                            |  sandbox   backend trait, bwrap    |
                                                            |  ipc       tonic server            |
                                                            +-----------------+------------------+
                                                                              | exactly one UDS (file bind)
                                                            +-----------------v------------------+
                                                            | sandbox: bwrap --unshare-all       |
                                                            |  shim: bridge, then seccomp,       |
                                                            |        then exec agent             |
                                                            |  agent (opencode) HTTP_PROXY=      |
                                                            |        127.0.0.1:3128              |
                                                            |  /work  (project, ro|rw)           |
                                                            |  CA only as .crt, env kit set      |
                                                            +------------------------------------+
                                                                              |
                                                     LAN LLM (passthrough rule, logged)  ·  Internet (only after approval)
```

The daemon resolves a host name only after a request was allowed, so not even a DNS lookup leaves
before you decide. The architecture is in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), the
decisions in [`docs/adr/`](docs/adr/README.md). The isolation model follows the Linux sandboxes of
Claude Code and the OpenAI Codex CLI; the proxy is an own accept loop on hyper 1 with
tokio-rustls and rcgen (`daemon/crates/proxy/src/core.rs`), the sandbox is
[bubblewrap](https://github.com/containers/bubblewrap).

## Status

| Milestone | Delivers | Status |
|---|---|---|
| M0 Foundation | monorepo, CI, protobuf contract, core types, fake daemon, escape-test harness | delivered |
| M1 Sealed box | sandbox provably closed, proxy holds a request | delivered |
| M2 First decision | rules, findings, recorder, intercept and history screens | delivered |
| M3 Agent inside | OpenCode against a LAN model, terminal, isolation check, `humanitl run` | delivered |
| M4 Trusted editor | pseudonymisation, audit chain, settings, German and English, packages | delivered |
| M5 Release 0.1 | fuzzing, limits, error paths, documentation, release | in progress |

A milestone counts as delivered when its demo script runs green in CI against a real daemon and a
real sandbox (`tests/e2e/`), not when its code exists.

Version 0.1.0 is the first release (milestone M5). In it: the sealed sandbox with the three
guarantees and five escape tests in CI; the intercept queue with allow, block, edit and rules; the
OpenCode and pi agents against a model in your network, with terminal and isolation check; history, export
and a hash-chained audit log; pseudonymisation in the editor; settings, German and English; `.deb`
and AppImage. Not in it: holding responses or WebSocket frames, HTTP/2 to the destination, a Docker
or micro-VM backend, a browser for the agent, macOS. What comes after the MVP is listed in
[`BACKLOG.md` section 9](BACKLOG.md#9-nach-dem-mvp-priorisiert).

## Contributing

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) for toolchain, workflow and the definition of done. The
plan lives in [`BACKLOG.md`](BACKLOG.md) and the issue specifications in [`backlog/`](backlog/);
names and types every part agrees on are in [`backlog/CONVENTIONS.md`](backlog/CONVENTIONS.md). The
most useful contribution is still to read [`docs/SECURITY.md`](docs/SECURITY.md) and tell us where
the reasoning is wrong. Security problems go through [`SECURITY.md`](SECURITY.md), not public issues.

## License

GPL-3.0-only. See [`LICENSE`](LICENSE). Domain data from the
[Public Suffix List](https://publicsuffix.org/) and the
[Majestic Million](https://majestic.com/reports/majestic-million) under CC BY 3.0
(`catalog/RANKS-LICENSE`).
