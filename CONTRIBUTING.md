# Contributing

## Toolchain

| Tool | Version | Note |
|---|---|---|
| Rust | 1.85+ (pinned 1.95.0 in `daemon/rust-toolchain.toml`) | needs `rustfmt` and `clippy` |
| Flutter | 3.44+ (pinned in `app/.fvmrc`) | Dart 3.12+ |
| bubblewrap | 0.8+ | runtime dependency of the sandbox |
| socat | not required | the shim carries its own bridge |

A local toolchain installed without `rustup` has no `rustfmt` and no `clippy`.
`make check` then skips those two steps and says so. Continuous integration
installs both and runs with `STRICT=1`, so nothing merges unformatted.

Optional: `cargo install cargo-deny` for `make rust-deny`.

## Working on an issue

One issue, one branch, named `hum-042-short-title`. The specification of every
issue is in `backlog/sprint-N.md`; read `BACKLOG.md` sections 2 to 6 and
`backlog/CONVENTIONS.md` before the first one.

`make check` has to pass before every push.

## Commit messages

Prefix with `feat`, `fix`, `test`, `docs`, `chore` or `refactor`, followed by the
scope in parentheses:

```
feat(rules): label glob matching
fix(proxy): answer 100-continue before buffering
```

## Definition of done

From `backlog/CONVENTIONS.md` section 3.12, in short:

- Acceptance criteria of the issue ticked, its tests present and green.
- New error paths return a `Diagnostic` with `why` and, where possible, `fix`.
- New settings carry tier, description and default in the schema.
- New user-visible strings exist in `app/l10n/app_en.arb` and `app_de.arb`.
- No `unwrap()` or `expect()` outside tests and `main`, no `Err(String)`.
- Every public type and function has a documentation comment.

## Architecture rules

Dependencies point inward only; `make deps-lint` enforces it. The core crates
carry no IO, no async and no protobuf. Every capability is an RPC first: the
desktop application and the command line are thin clients of the same service.
Details in `docs/ARCHITECTURE.md`.
