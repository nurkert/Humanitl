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

## When CI is red

The `rust-test` job pipes its output to a file. A red run therefore names the
tests that failed as annotations on the run — one line per test, at most ten
lines in all, and from the eleventh test on the tenth line collects the rest —
and uploads the whole output as the artifact `rust-test-log`. Both are
visible without write access to the repository; the job log itself is not.

`scripts/ci/test-report.sh <log>` is the same evaluation, and
`scripts/ci/test-report.sh --self-test` (part of `make check`) keeps it honest.

## Disk

A fresh build tree of the daemon weighs about 6 GiB after
`cargo build --workspace --all-targets`, and it grows with every change of a
dependency or a feature set: Cargo writes a new set of artefacts and never
removes an old one. On 2026-09-06 the tree of this repository stood at 103 GiB
after roughly a week of work. The dev profile therefore carries
`debug = "line-tables-only"` (`daemon/Cargo.toml`, measured there), which halves
each set while keeping file and line in every backtrace.

When the tree has grown past what the machine can spare, reset it:

```sh
cargo clean --manifest-path daemon/Cargo.toml
```

The Flutter side is small by comparison (`app/build`, a few hundred MiB);
`flutter clean` resets it.

## Working on an issue

One issue, one branch, named `hum-042-short-title`. The specification of every
issue is in `backlog/sprint-N.md`; read `BACKLOG.md` sections 2 to 6 and
`backlog/CONVENTIONS.md` before the first one.

`make check` has to pass before every push.

`make flutter-test-dbus` is not part of it: the D-Bus protocol tests
(`app/test/features/tray/dbus_live_test.dart`) need a session bus that CI does
not have, and only on the private bus of `dbus-run-session` are the names
`org.kde.StatusNotifierWatcher` and `org.freedesktop.Notifications` free, so
that the test can hold them itself and read what its adapters put on the wire.
Run it after touching the tray or the notification adapter. On the bus of a
real desktop the same tests skip with a reason instead of registering against
the panel of the person sitting there.

`make flutter-test-integration` is not part of it either: it runs the real
application on a screen (`Xvfb :99` is enough) against a real daemon, one file
at a time -- several files in one invocation start the app twice on the same
device and the second start fails. Run it after touching the shell, the queue
or anything the app shows while a daemon answers.

`make flutter-test-daemon` is not part of it either: the sandbox screen against
a real daemon (`app/test/features/sandbox/daemon_live_test.dart`) starts a real
`humanitld` in its own XDG root and a real sandbox with `bwrap`, and neither
belongs in a gate that has to be green in seconds on every machine. Run it
after touching the sandbox screen, its providers or the `Sandbox` RPC. Without
`HUMANITL_DAEMON_TESTS=1` the file registers a single skipped test with the
reason, so a normal `flutter test` run is unaffected.

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

**The M2 gate runs the whole loop.** HUM-036 asks for real daemon, real
sandbox **and the real screen under xvfb**, ending in a valid HAR file, and
since HUM-097 that is what happens: the screen driver
(`app/integration_test/m2_first_decision_test.dart`) starts before the agent
and takes the decisions of sections 2 to 4 while the requests are held — the
batch release with a session rule, the block, the single allow — then filters
the history and writes the HAR file that step 10 reads back. A green
`e2e-xvfb` means "M2 holds".

Two paths used to be on the list of things a green run did not vouch for and
no longer are. HUM-087 gave the daemon `--allow-test-ca`; every one of the
seventeen requests now goes over `https://`, so leaf minting from Humanitl's
own CA, the handshake with the agent and the upstream TLS session run for
every released and blocked flow, and both findings are made in bodies the
proxy decrypted itself. HUM-097 added the screen half.

`M2_UI=0` switches the screen off, for a machine without `flutter` or
`xvfb-run`. Such a run says nothing about the screen and nothing about the HAR
format, and it says so in its own output; CI never takes that branch.

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

## Vorabversionen 0.0.x

Bis zum ersten richtigen Release 0.1.0 gibt es Vorabversionen mit der Nummer
0.0.N. Sie entstehen aus einem Tag und dem Workflow
`.github/workflows/release.yml`. Das Release enthält das Paket
`humanitl_0.0.N_amd64.deb`, das Archiv `humanitl-0.0.N-linux-x86_64.tar.gz`
und `SHA256SUMS`. Es ist auf GitHub als Vorabversion markiert, nicht signiert
und nicht für den produktiven Einsatz gedacht.

So wird eine Vorabversion geschnitten:

1. Auf GitHub nachsehen, dass der CI-Lauf des Commits auf `main` grün ist.
   Am sichersten ist der jüngste Commit von `main`: `ci.yml` bricht den Lauf
   eines älteren Commits ab, sobald ein neuerer kommt, und ein abgebrochener
   Lauf zählt nicht als grün.
2. Den Tag auf genau diesen Commit setzen und nur den Tag pushen:

   ```sh
   git fetch origin
   git tag -a v0.0.3 -m "Humanitl 0.0.3" origin/main
   git push origin v0.0.3
   ```

3. Den Lauf beobachten. Der Job `guard` bricht ab, wenn der Tag nicht die Form
   `v0.0.N` hat, der Commit nicht auf `main` liegt oder sein CI-Lauf nicht mit
   `success` endete; läuft CI noch, wartet er bis zu 45 Minuten. `check-deb`
   installiert das Paket in einem frischen Ubuntu-24.04-Container, prüft es und
   entfernt es wieder, und erst danach legt `release` das Release an.

Ein Tag wird nie verschoben oder neu gesetzt. Schlägt ein Lauf fehl, wird der
Fehler auf `main` behoben und die nächste Nummer getaggt; der misslungene Tag
kann gelöscht werden, seine Nummer wird nicht wieder vergeben.

Die Versionsnummer steht im Repository weiter als `0.0.0`. Der Workflow setzt
sie nur in seinem eigenen Auscheckstand in `daemon/Cargo.toml` und
`daemon/Cargo.lock` (`packaging/release/stamp-version.sh`) und übergibt sie
der App mit `flutter build linux --build-name`. Dass `humanitl --version`,
`humanitld --version` und die App dieselbe Nummer tragen, prüft der Lauf und
scheitert sonst.

**Probelauf.** Unter „Actions", Workflow „release", „Run workflow" mit einer
Version wie `0.0.3` baut alles, prüft das Paket im Container und lädt die
Dateien als Workflow-Artefakt `humanitl-dry-run-<commit>` hoch (ein Tag-Lauf
nennt sein Artefakt `humanitl-release`), legt aber kein Release
an. Der Probelauf darf auch einen Zweig bauen; dass der Commit nicht auf
`main` liegt oder CI nicht grün ist, steht dann als Warnung im Lauf statt als
Fehler. Die Schritte lassen sich lokal einzeln nachfahren, die Skripte liegen
unter `packaging/release/` und `packaging/deb/`. Das Paket wird dabei nie auf
dem eigenen Rechner installiert, sondern nur in einem Wegwerf-Container, so wie
es `packaging/deb/check-install.sh` verlangt.

**0.1.0 ist etwas anderes.** Der erste richtige Release folgt HUM-060 in
`backlog/sprint-5.md`: eine `VERSION`-Datei als einzige Quelle aller
Versionsstellen, `CHANGELOG.md`, minisign-Signaturen, AppImage und die
Abnahme-Checkliste. Dieser Workflow reagiert nur auf `v0.0.*`; ein Tag
`v0.1.0` löst hier nichts aus, bis HUM-060 den Workflow erweitert.
