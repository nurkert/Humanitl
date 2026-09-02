# Humanitl

Humanitl lets you give an AI coding agent internet access without giving up
control. The agent runs in a sandbox that has no network interface. Its only way
out is a moderating proxy: every request is held, shown to you with headers,
parameters and body, and leaves only after you allow it. Rules take care of the
traffic you already trust. Everything is recorded.

The agent talks to a local language model on your own network. Nothing else
reaches the internet unless you release it.

## Why it holds

1. **No network interface.** The sandbox runs in its own network namespace with
   only a loopback device. No address, no DNS, no ICMP, no QUIC.
2. **One door.** A single Unix socket, bind-mounted into the sandbox, leads to
   the proxy on the host.
3. **No new doors.** A seccomp filter allows only loopback TCP sockets and
   refuses every other socket family.

The same approach is used by the sandboxes of Claude Code and the OpenAI Codex
CLI. You can verify all three checks from the application at any time, and the
exact sandbox command line is one click away.

Two channels are deliberately open and shown as such: the project directory the
agent works in, and the connection to your language model. Both are documented
in `docs/SECURITY.md`.

## Status

Early development. The plan from nothing to a working first version is in
[`BACKLOG.md`](BACKLOG.md); the architecture guideline is in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md); the issue specifications are
under [`backlog/`](backlog/).

## Getting started

```sh
make check   # format, lint, build and test both toolchains
make help    # all targets
```

Requirements: Rust 1.85 or newer, Flutter 3.44 or newer, `bubblewrap` on Linux.
See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Licence

GPL-3.0-only. See [`LICENSE`](LICENSE).
