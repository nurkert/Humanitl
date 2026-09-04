# Contributing

## Toolchain

| Tool | Version | Note |
|---|---|---|
| Rust | 1.88+ (pinned 1.95.0 in `daemon/rust-toolchain.toml`) | needs `rustfmt` and `clippy` |
| Flutter | 3.47.2 (pinned in `app/.fvmrc`) | Dart 3.13+ |
| bubblewrap | 0.8+ | runtime dependency of the sandbox |
| socat | not required | the shim carries its own bridge |

A local toolchain installed without `rustup` has no `rustfmt` and no `clippy`.
If a rustup toolchain exists under `~/.rustup/toolchains/` (even without
`rustup` itself on PATH), the Makefile puts its `bin` directory first for the
fmt and clippy targets. Otherwise `make check` skips those two steps and says
so. Continuous integration installs both and runs with `STRICT=1`, so nothing
merges unformatted or with clippy warnings.

`make proto` generates the Dart side of the contract only when `protoc` and
`protoc-gen-dart` (`dart pub global activate protoc_plugin`, exact version
pinned in `scripts/gen-proto.sh`) are on PATH; without them the Flutter gate
cannot resolve the generated code. Add `~/.pub-cache/bin` to PATH.

Optional: `cargo install cargo-deny` for `make rust-deny`.

## Working on an issue

One issue, one branch, named `hum-042-short-title`. The specification of every
issue is in `backlog/sprint-N.md`; read `BACKLOG.md` sections 2 to 6 and
`backlog/CONVENTIONS.md` before the first one.

`make check` has to pass before every push.

## Sprint gate

Every milestone ends with a demo script, and the scripts of the milestones
already reached stay green (`BACKLOG.md` section 8). From the end of sprint 2
on, nothing merges unless both are green in continuous integration:

| Milestone | Script | CI job |
|---|---|---|
| M1, the sealed box | `tests/e2e/m1_sealed_box.sh` | `e2e` |
| M2, the first decision | `tests/e2e/m2_first_decision/run.sh` | `e2e-xvfb` |

`make e2e` runs both, in that order; `E2E_ONLY=m1` or `E2E_ONLY=m2` picks one.
Both scripts print one line per assertion they checked, whether it held or not.
The M2 script also counts them and fails when fewer ran than it expects, so a
run that skipped a branch cannot report success; the M1 script carries the same
counter but does not yet check it.

**The M2 gate is half built, and the half that is missing is named.** HUM-036
asks for the whole loop — real daemon, real sandbox **and the real screen under
xvfb**, ending in a valid HAR file. What runs today is the daemon half: request
grouping, a batch release with a session rule, block with a note, the hold
deadline, the recorded history and the set the export is built from. Not
covered, and therefore not vouched for by a green run:

- **the screen.** Queue, action bar, rules screen and history are never driven;
  no HAR file is written or validated. That is HUM-097.
- **the MITM path.** Sixteen of the seventeen requests are plain HTTP, so leaf
  minting from Humanitl's own CA, the handshake with the agent and the upstream
  TLS session run for no released or blocked flow at all. That is coverage the
  product does not have right now, and it comes back with HUM-087.

Until both land, a green `e2e-xvfb` means "the daemon half of M2 holds", not
"M2 holds". Say so when you lean on it.

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
