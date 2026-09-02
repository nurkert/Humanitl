# Sprint 0 · Fundament (M0)

Ziel des Sprints: Ein Monorepo, das leer baut, eine CI, die leer grün ist, ein Proto-Vertrag, ein Kerntypen-Crate mit Zustandsautomat, ein Fake-Daemon für die UI-Entwicklung, ein Escape-Test-Harness (noch rot), Sicherheits- und Design-Dokumente als Entwurf. Nach Sprint 0 kann ein zweiter Entwickler an UI und Daemon parallel arbeiten, ohne auf den jeweils anderen zu warten.

Voraussetzung für alle Issues: `BACKLOG.md` Abschnitte 2 bis 6 und `backlog/CONVENTIONS.md` gelesen.

| Reihenfolge | ID | Titel | Größe | Abhängigkeiten |
|---|---|---|---|---|
| 1 | HUM-001 | Monorepo anlegen | S | keine |
| 2 | HUM-002 | CI-Pipeline | M | HUM-001 |
| 3 | HUM-003 | Proto v1 definieren | M | HUM-001 |
| 4 | HUM-004 | core-types Crate | M | HUM-001 |
| 5 | HUM-063 | Diagnostic-Typ | S | HUM-004, HUM-003 |
| 6 | HUM-062 | config Crate mit Schema | M | HUM-004, HUM-063 |
| 7 | HUM-010 | Sandbox-Profil-Format | S | HUM-062 |
| 8 | HUM-005 | Fake-Daemon für UI-Entwicklung | M | HUM-003, HUM-004 |
| 9 | HUM-006 | Escape-Test-Harness | M | HUM-010 |
| 10 | HUM-007 | SECURITY.md und THREAT-MODEL.md Entwurf | S | keine |
| 11 | HUM-008 | Design-Tokens und `packages/ui` | M | HUM-001 |
| 12 | HUM-074 | Abhängigkeits-Lint | S | HUM-001 |
| HUM-009 | ADR-Verzeichnis | S | keine |

Sprint-Abschluss (Demo-Skript M0): `make check` läuft lokal und in CI grün durch: Rust-Workspace baut, Flutter-App baut, Proto-Codegen ohne Drift, `humanitl --fake`-Session spielt 20 Flows ab, Escape-Harness erzeugt JUnit-XML (Ergebnis rot ist erlaubt und erwartet).

---

> **Abgleich 2026-09-02** (gilt vor dem Text der Issues, Details in `CONVENTIONS.md` Abschnitt 4): `Rule`-Typen liegen in `humanitl-core::rule`; seccomp erlaubt `AF_INET`/`AF_INET6`; Escape-Test-Dateien heißen `esc-N-<name>.sh`; Diagnostic-Codes werden im Register `core-types/src/diagnostics/codes.rs` reserviert (Bereiche siehe CONVENTIONS 4.6); `packages/ui` enthält zusätzlich `HModal`; `daemon/xtask` ist eine Hilfs-Crate außerhalb der Abhängigkeitsregeln; Sandbox-Profil hat `[network].bridges` und `[seccomp].allow_families`.

## HUM-001 · Monorepo anlegen
Sprint: 0 · Größe: S · Abhängigkeiten: keine · Blockiert: alle anderen

### Kontext
Setzt BACKLOG.md 3.2 (Monorepo-Layout) und CONVENTIONS.md 3.1 (Crates) um. Alles Weitere baut auf dieser Struktur auf. Ein falsches Layout hier kostet in jedem späteren Issue Zeit.

### Ziel
Nach `git clone` bauen `cargo build --workspace` und `flutter build linux --debug` ohne Fehler, beide Toolchains sind versionsgepinnt, alle Verzeichnisse aus dem Layout existieren mit Platzhalter-Inhalt, und `make check` führt Format, Lint und Tests beider Welten aus.

### Nicht-Ziel
Keine Fachlogik. Keine Proto-Definition (HUM-003). Keine CI (HUM-002). Kein shadcn_flutter-Setup (HUM-008).

### Betroffene Pfade
- `Makefile` (neu)
- `.editorconfig` (neu)
- `.gitignore` (neu)
- `.gitattributes` (neu)
- `CONTRIBUTING.md` (neu)
- `README.md` (neu, Kurzfassung mit Verweis auf BACKLOG.md)
- `daemon/Cargo.toml` (neu, Workspace)
- `daemon/rust-toolchain.toml` (neu)
- `daemon/.cargo/config.toml` (neu)
- `daemon/deny.toml` (neu)
- `daemon/crates/{core-types,config,rules,findings,recorder,audit,sandbox,catalog,proxy,ipc}/` (neu, je `Cargo.toml` + `src/lib.rs`)
- `daemon/bin/{humanitld,humanitl,humanitl-shim}/` (neu, je `Cargo.toml` + `src/main.rs`)
- `app/` (neu, `flutter create`)
- `app/.fvmrc` (neu)
- `app/packages/ui/` (neu, leeres Dart-Package)
- `proto/humanitl/v1/.gitkeep` (neu)
- `profiles/sandbox/.gitkeep`, `agents/opencode/.gitkeep`, `catalog/.gitkeep`, `rules/.gitkeep`, `packaging/{systemd,deb,appimage}/.gitkeep`, `tests/{escape,e2e}/.gitkeep`, `docs/adr/.gitkeep` (neu)

### Spezifikation

`daemon/Cargo.toml`:

```toml
[workspace]
resolver = "3"
members = [
  "crates/core-types", "crates/config", "crates/rules", "crates/findings",
  "crates/recorder", "crates/audit", "crates/sandbox", "crates/catalog",
  "crates/proxy", "crates/ipc",
  "bin/humanitld", "bin/humanitl", "bin/humanitl-shim",
]

[workspace.package]
edition = "2024"
license = "GPL-3.0-only"
repository = "https://github.com/<owner>/humanitl"
rust-version = "1.85"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
thiserror = "2"
serde = { version = "1", features = ["derive"] }
uuid = { version = "1", features = ["v7", "serde"] }
tracing = "0.1"
# weitere Abhängigkeiten werden in den jeweiligen Issues ergänzt, immer hier zentral versioniert

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
```

Jede Crate: `[lints] workspace = true`, `[package] name = "humanitl-<name>"` (Ausnahmen: `humanitld`, `humanitl`, `humanitl-shim`). `humanitl-shim` bekommt `[dependencies]` nur `libc`; kein tokio. `lib.rs` enthält `//! Crate-Doku` und `#![forbid(unsafe_code)]` (Ausnahme: `humanitl-shim` darf `unsafe` für `prctl`/`seccomp`, dort `#![deny(unsafe_op_in_unsafe_fn)]`).

`daemon/rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.85.0"
components = ["rustfmt", "clippy"]
```

`daemon/.cargo/config.toml`: `[build] rustflags = ["-D", "warnings"]` nur unter `[target.'cfg(all())']` für CI via Env, lokal nicht erzwingen. Einfacher: leer lassen und `-D warnings` im Makefile setzen.

`daemon/deny.toml`: Lizenz-Allowlist `MIT`, `Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Unicode-3.0`, `MPL-2.0`, `Zlib`, `OpenSSL`; `GPL-3.0` erlaubt (eigener Code); `advisories.vulnerability = "deny"`; `bans.multiple-versions = "warn"`.

`app/`: `flutter create --platforms=linux --org dev.humanitl --project-name humanitl app`, danach `app/.fvmrc` mit `{"flutter": "3.47.0"}` und in `pubspec.yaml` `environment: sdk: ^3.13.0`, `flutter: ">=3.47.0"`. Verzeichnisse `lib/core/{ipc,domain,ui}`, `lib/features/{setup,intercept,editor,history,rules,sandbox,audit,settings}`, `l10n/`, `test/goldens/`, `integration_test/` mit `.gitkeep`. `app/lib/core/ipc/generated/` in `.gitignore`.

`app/packages/ui/pubspec.yaml`: `name: humanitl_ui`, Abhängigkeit `flutter`. In `app/pubspec.yaml`: `humanitl_ui: { path: packages/ui }`.

`Makefile` (Targets, alle phony):

```make
check: rust-fmt rust-clippy rust-test flutter-analyze flutter-test
rust-fmt:      ; cd daemon && cargo fmt --all --check
rust-clippy:   ; cd daemon && cargo clippy --workspace --all-targets -- -D warnings
rust-test:     ; cd daemon && cargo test --workspace
rust-deny:     ; cd daemon && cargo deny check
flutter-get:   ; cd app && flutter pub get
flutter-analyze: flutter-get ; cd app && flutter analyze
flutter-test:  flutter-get ; cd app && flutter test
proto:         ; ./scripts/gen-proto.sh        # ab HUM-003
escape:        ; ./tests/escape/run.sh         # ab HUM-006
```

`.editorconfig`: UTF-8, LF, `indent_style = space`, Rust 4, Dart 2, YAML/TOML 2, Markdown `trim_trailing_whitespace = false`.

`.gitattributes`: `*.arb linguist-language=JSON`, `app/lib/core/ipc/generated/** linguist-generated`.

`CONTRIBUTING.md`: Toolchain-Versionen, `make check` vor jedem Push, Commit-Präfixe aus CONVENTIONS.md 2, Verweis auf Definition of Done (CONVENTIONS.md 3.12), Regel „ein Issue, ein Branch `hum-xxx-kurztitel`".

### Schritte
1. Wurzeldateien anlegen (`Makefile`, `.editorconfig`, `.gitignore`, `.gitattributes`, `README.md`, `CONTRIBUTING.md`). `.gitignore` deckt `daemon/target/`, `app/build/`, `app/.dart_tool/`, `app/lib/core/ipc/generated/`, `*.db`, `*.db-wal`.
2. Cargo-Workspace mit allen 13 Mitgliedern anlegen. Jede Lib-Crate: `lib.rs` mit Crate-Doku-Kommentar und einem leeren `pub mod` Platzhalter ist nicht nötig, nur Doku. Jedes Bin: `main.rs` mit `fn main() { println!("humanitl <name> 0.0.0"); }`. Prüfen: `cd daemon && cargo build --workspace && cargo clippy --workspace -- -D warnings`.
3. `rust-toolchain.toml`, `deny.toml` anlegen. Prüfen: `cargo deny check` (Installation `cargo install cargo-deny` dokumentieren).
4. Flutter-App anlegen, Verzeichnisstruktur, `.fvmrc`, `packages/ui`. Prüfen: `cd app && flutter pub get && flutter analyze && flutter build linux --debug`.
5. Platzhalter-Verzeichnisse mit `.gitkeep`.
6. `make check` ausführen, muss grün sein.

### Tests
- Kein Fachtest. Verifikation ist `make check`.
- `daemon/crates/core-types/src/lib.rs` enthält einen `#[cfg(test)] mod tests { #[test] fn workspace_builds() {} }`, damit `cargo test` nicht mit „0 tests" verwirrt.

### Akzeptanzkriterien
- [ ] `cd daemon && cargo build --workspace` exit 0
- [ ] `cd daemon && cargo clippy --workspace --all-targets -- -D warnings` exit 0
- [ ] `cd daemon && cargo deny check` exit 0
- [ ] `cd app && flutter analyze` exit 0 und `flutter build linux --debug` exit 0
- [ ] `make check` exit 0
- [ ] `find . -path ./.git -prune -o -type d -print | sort` enthält jedes Verzeichnis aus „Betroffene Pfade"
- [ ] `git status` nach `make check` zeigt keine ungetrackten generierten Dateien

### Fallstricke
- Cargo-Resolver 3 braucht Rust ≥ 1.84. Toolchain-Pin muss dazu passen.
- `unwrap_used = deny` bricht Tests, die `unwrap()` nutzen. In Test-Modulen `#![allow(clippy::unwrap_used)]` setzen, nicht global lockern.
- `flutter create` erzeugt `test/widget_test.dart` mit Counter-Test; löschen, sonst schlägt er nach HUM-008 fehl.
- `flutter create` legt `linux/` mit CMake an. Nicht anfassen, bis HUM-053 (Packaging).
- `humanitl-shim` darf keine Workspace-Lints erben, die `unsafe_code = forbid` setzen. Für diese Crate `[lints.rust] unsafe_code = "allow"` lokal überschreiben.

### Referenzen
BACKLOG.md 3.2, CONVENTIONS.md 3.1, 3.9, 3.12.

---

## HUM-002 · CI-Pipeline
Sprint: 0 · Größe: M · Abhängigkeiten: HUM-001 · Blockiert: HUM-003 (Codegen-Drift-Job), HUM-006 (Escape-Job)

### Kontext
BACKLOG.md 8 legt fest: jeder Sprint endet mit einem grünen Demo-Skript in CI, sonst wird nichts gemerged. Die Pipeline muss also vor dem ersten Feature stehen. Jobs, deren Inhalt erst später kommt, werden als Platzhalter angelegt, die bewusst „skipped" melden, nicht grün lügen.

### Ziel
GitHub Actions führt bei jedem Push und Pull Request die Jobs `rust-check`, `rust-test`, `proto-lint-and-gen`, `flutter-analyze-test`, `goldens`, `escape-tests`, `e2e-xvfb` aus. Alle laufen auf leerem Stand grün oder als expliziter Skip durch. Ein tag-getriggerter Job `release` existiert als Gerüst.

### Nicht-Ziel
Kein Packaging-Job-Inhalt (HUM-053). Kein Fuzzing (HUM-056). Kein Release-Upload (HUM-060).

### Betroffene Pfade
- `.github/workflows/ci.yml` (neu)
- `.github/workflows/release.yml` (neu, Gerüst)
- `.github/actions/setup-rust/action.yml` (neu, Composite)
- `.github/actions/setup-flutter/action.yml` (neu, Composite)
- `scripts/ci/escape-placeholder.sh` (neu)
- `scripts/ci/e2e-placeholder.sh` (neu)

### Spezifikation

`.github/workflows/ci.yml`:

```yaml
name: ci
on:
  push: { branches: [main] }
  pull_request:
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true
env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-D warnings"
  FLUTTER_VERSION: "3.47.0"

jobs:
  rust-check:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - uses: ./.github/actions/setup-rust
      - run: cd daemon && cargo fmt --all --check
      - run: cd daemon && cargo clippy --workspace --all-targets -- -D warnings
      - run: cd daemon && cargo deny check

  rust-test:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - uses: ./.github/actions/setup-rust
      - run: cd daemon && cargo test --workspace --all-targets
      - run: cd daemon && cargo doc --workspace --no-deps
        env: { RUSTDOCFLAGS: "-D warnings" }

  proto-lint-and-gen:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - uses: bufbuild/buf-action@v1
        with: { setup_only: true }
      - uses: ./.github/actions/setup-flutter
      - run: buf lint proto
      - run: buf breaking proto --against ".git#branch=main,subdir=proto"
        if: github.event_name == 'pull_request'
      - run: ./scripts/gen-proto.sh
      - name: Fail on generated drift
        run: git diff --exit-code -- daemon/crates/ipc/src/generated app/lib/core/ipc/generated || (echo "::error::generated code drifted, run make proto" && exit 1)

  flutter-analyze-test:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - uses: ./.github/actions/setup-flutter
      - run: cd app && flutter pub get
      - run: cd app && dart run build_runner build --delete-conflicting-outputs
      - run: cd app && flutter analyze --fatal-infos
      - run: cd app && flutter test --coverage
      - run: cd app && flutter build linux --debug

  goldens:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - uses: ./.github/actions/setup-flutter
      - run: cd app && flutter pub get
      - run: cd app && flutter test --tags golden test/goldens
        # ab HUM-054; bis dahin gibt es keine Tests mit diesem Tag, flutter test exit 0

  escape-tests:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - uses: ./.github/actions/setup-rust
      - run: sudo apt-get update && sudo apt-get install -y bubblewrap socat python3
      - run: bwrap --version
      - run: ./scripts/ci/escape-placeholder.sh   # ab HUM-006: ./tests/escape/run.sh
      - uses: actions/upload-artifact@v4
        if: always()
        with: { name: escape-junit, path: target/escape/*.xml, if-no-files-found: ignore }

  e2e-xvfb:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
      - uses: ./.github/actions/setup-rust
      - uses: ./.github/actions/setup-flutter
      - run: sudo apt-get update && sudo apt-get install -y xvfb libgtk-3-dev ninja-build clang bubblewrap socat
      - run: ./scripts/ci/e2e-placeholder.sh      # ab HUM-021: xvfb-run -a ./tests/e2e/run.sh
```

`.github/actions/setup-rust/action.yml`: Composite mit `dtolnay/rust-toolchain@stable` (liest `rust-toolchain.toml`), `Swatinem/rust-cache@v2` mit `workspaces: daemon`, Installation von `cargo-deny` über `taiki-e/install-action@cargo-deny`, Installation von `protoc` über `arduino/setup-protoc@v3`.

`.github/actions/setup-flutter/action.yml`: Composite mit `subosito/flutter-action@v2`, `flutter-version: ${{ env.FLUTTER_VERSION }}` (Composite kann `env` nicht lesen, also Input `flutter-version` mit Default `3.47.0`), `cache: true`, dann `dart pub global activate protoc_plugin 22.0.0` und Pfad `~/.pub-cache/bin` an `$GITHUB_PATH`, `sudo apt-get install -y libgtk-3-dev ninja-build clang cmake pkg-config`.

`scripts/ci/escape-placeholder.sh`:

```sh
#!/usr/bin/env sh
set -eu
mkdir -p target/escape
cat > target/escape/placeholder.xml <<'EOF'
<testsuite name="escape" tests="0" skipped="0"><!-- HUM-006 not yet implemented --></testsuite>
EOF
echo "::notice::escape tests not yet implemented (HUM-006)"
```

`scripts/ci/e2e-placeholder.sh` analog mit Notice auf HUM-021.

`.github/workflows/release.yml`: `on: push: tags: ['v*']`, ein Job `build` der `make check` ausführt und einen Draft-Release ohne Assets anlegt (`softprops/action-gh-release@v2`, `draft: true`). Inhalt kommt mit HUM-053/060.

Branch-Schutz (manuell im Repo, in CONTRIBUTING.md dokumentieren): `main` verlangt alle sieben Jobs grün.

### Schritte
1. Composite-Actions anlegen. Lokal mit `act` nicht nötig; Syntax mit `actionlint` prüfen (`brew`/`go install`, oder `rhysd/actionlint@v1` Docker).
2. `ci.yml` mit allen Jobs anlegen. `proto-lint-and-gen` bis HUM-003 mit `if: false` deaktivieren und Kommentar `# enable in HUM-003`.
3. Platzhalter-Skripte anlegen, ausführbar machen (`chmod +x`).
4. `release.yml` Gerüst.
5. Push auf einen Branch, PR öffnen, alle Jobs müssen grün sein. Screenshot oder Link in PR-Beschreibung.
6. Branch-Schutz einrichten, in `CONTRIBUTING.md` dokumentieren.

### Tests
- `actionlint .github/workflows/*.yml` exit 0.
- PR gegen `main` mit leerer Änderung zeigt sieben grüne Jobs (bzw. `proto-lint-and-gen` als skipped).

### Akzeptanzkriterien
- [ ] Alle Jobs aus der Spezifikation existieren namentlich in `ci.yml`
- [ ] `actionlint` sauber
- [ ] Erster PR grün, Laufzeit gesamt < 15 min (Cache warm)
- [ ] `escape-tests` lädt ein JUnit-Artefakt hoch, auch beim Platzhalter
- [ ] `RUSTFLAGS=-D warnings` ist in CI aktiv, lokal nicht erzwungen
- [ ] `release.yml` triggert nur auf `v*`-Tags

### Fallstricke
- `subosito/flutter-action` mit `cache: true` und wechselnder Version erzeugt stale Caches; `cache-key` mit `FLUTTER_VERSION` versehen.
- `buf breaking` gegen `main` scheitert beim ersten PR, wenn `main` noch keine Proto hat. `continue-on-error: true` bis HUM-003 gemerged, danach entfernen.
- `flutter test --tags golden` mit null getaggten Tests ist exit 0, aber `flutter test test/goldens` mit nicht existierendem Verzeichnis ist exit 1. Verzeichnis mit `.gitkeep` muss existieren (HUM-001).
- `RUSTDOCFLAGS=-D warnings` macht fehlende Doku-Kommentare zum Fehler, sobald `missing_docs = warn` greift. Gewollt.
- bubblewrap auf GitHub-Runnern: `ubuntu-24.04` erlaubt unprivilegierte User-Namespaces; AppArmor-Profil `unprivileged_userns` kann seit 24.04 blockieren. Falls `bwrap` mit `Permission denied` scheitert: `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0` im Job vor dem Aufruf. In HUM-006 einbauen.

### Referenzen
BACKLOG.md 3.2, 8 (Sprint-Regel), Flutter Integration-Test-Docs (docs.flutter.dev/testing/integration-tests), buf (buf.build/docs).

---

## HUM-003 · Proto v1 definieren
Sprint: 0 · Größe: M · Abhängigkeiten: HUM-001 · Blockiert: HUM-005, HUM-018, HUM-019

### Kontext
ADR-003: gRPC über UDS, die Proto ist der versionierte Vertrag und später die Plugin-Schnittstelle. Alle Ereignisse und Kommandos zwischen UI, CLI und Daemon laufen ausschließlich über diesen Vertrag. Änderungen an der Proto brauchen ab jetzt `buf breaking`.

### Ziel
`proto/humanitl/v1/humanitl.proto` ist vollständig, lint-sauber, und `./scripts/gen-proto.sh` erzeugt deterministisch Rust-Code (prost/tonic in `humanitl-ipc`) und Dart-Code (in `app/lib/core/ipc/generated/`). Die Datei deckt alle RPCs aus BACKLOG.md 3.3 ab.

### Nicht-Ziel
Kein Server (HUM-018), kein Client (HUM-019), kein Mapping auf Domain-Typen (HUM-018).

### Betroffene Pfade
- `proto/buf.yaml` (neu)
- `proto/buf.gen.yaml` (neu)
- `proto/humanitl/v1/humanitl.proto` (neu)
- `scripts/gen-proto.sh` (neu)
- `daemon/crates/ipc/build.rs` (neu) oder `daemon/crates/ipc/src/generated/` (eingecheckt, siehe Fallstricke)
- `daemon/crates/ipc/Cargo.toml` (`tonic`, `prost`, `prost-types`, build-dep `tonic-build`)
- `app/lib/core/ipc/generated/` (gitignored, wird erzeugt)
- `app/pubspec.yaml` (`grpc`, `protobuf`)

### Spezifikation

`proto/buf.yaml`:

```yaml
version: v2
modules:
  - path: .
lint:
  use: [STANDARD]
  except: [PACKAGE_VERSION_SUFFIX]   # wir nutzen v1 als Verzeichnis, nicht als Suffix
breaking:
  use: [FILE]
```

`proto/humanitl/v1/humanitl.proto` (vollständig):

```proto
syntax = "proto3";
package humanitl.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/duration.proto";
import "google/protobuf/empty.proto";

option java_multiple_files = true;

// ---------- Service ----------

service Humanitl {
  // Version und Fähigkeiten des Daemons. UI verweigert höhere Proto-Major.
  rpc GetInfo(google.protobuf.Empty) returns (Info);
  // Ereignisstrom. Bei Lagged: Client synchronisiert über ListFlows(since).
  rpc Subscribe(SubscribeRequest) returns (stream FlowEvent);
  rpc ListFlows(ListFlowsRequest) returns (FlowPage);
  rpc GetFlow(FlowRef) returns (FlowDetail);
  rpc GetBody(BodyRef) returns (stream BodyChunk);
  rpc Decide(DecideRequest) returns (DecideResponse);
  rpc Rules(RulesRequest) returns (RulesResponse);
  rpc Sandbox(SandboxRequest) returns (stream SandboxEvent);
  rpc Terminal(stream TerminalInput) returns (stream TerminalOutput);
  rpc Audit(AuditRequest) returns (AuditResponse);
  rpc GetConfig(GetConfigRequest) returns (ConfigSnapshot);
  rpc SetConfig(SetConfigRequest) returns (ConfigSnapshot);
}

// ---------- Info ----------

message Info {
  string daemon_version = 1;   // semver
  uint32 proto_major = 2;      // 1
  uint32 proto_minor = 3;
  repeated string capabilities = 4;   // "sandbox.bwrap", "proxy.h1", "proxy.h2", "findings.regex", ...
  string session_id = 5;       // aktuelle Session, leer wenn keine läuft
}

// ---------- Kern ----------

enum Method {
  METHOD_UNSPECIFIED = 0;
  METHOD_GET = 1; METHOD_HEAD = 2; METHOD_POST = 3; METHOD_PUT = 4; METHOD_PATCH = 5;
  METHOD_DELETE = 6; METHOD_OPTIONS = 7; METHOD_CONNECT = 8; METHOD_TRACE = 9;
  METHOD_OTHER = 10;
}

enum Scheme { SCHEME_UNSPECIFIED = 0; SCHEME_HTTP = 1; SCHEME_HTTPS = 2; SCHEME_WS = 3; SCHEME_WSS = 4; }

enum FlowState {
  FLOW_STATE_UNSPECIFIED = 0;
  FLOW_STATE_RECEIVED = 1; FLOW_STATE_ANALYZED = 2; FLOW_STATE_HELD = 3; FLOW_STATE_DECIDED = 4;
  FLOW_STATE_FORWARDED = 5; FLOW_STATE_RESPONDED = 6; FLOW_STATE_RECORDED = 7;
}

enum DecisionKind {
  DECISION_KIND_UNSPECIFIED = 0;
  DECISION_KIND_ALLOW = 1; DECISION_KIND_ALLOW_EDITED = 2; DECISION_KIND_BLOCK = 3; DECISION_KIND_TIMED_OUT = 4;
}

enum BlockReason {
  BLOCK_REASON_UNSPECIFIED = 0;
  BLOCK_REASON_USER = 1; BLOCK_REASON_RULE = 2; BLOCK_REASON_TIMEOUT = 3; BLOCK_REASON_BODY_CAP = 4;
  BLOCK_REASON_AUTHORITY_MISMATCH = 5; BLOCK_REASON_NO_ROUTE = 6;
}

enum DecisionSource {
  DECISION_SOURCE_UNSPECIFIED = 0;
  DECISION_SOURCE_USER = 1; DECISION_SOURCE_RULE = 2; DECISION_SOURCE_TIMEOUT = 3; DECISION_SOURCE_PASSTHROUGH = 4; DECISION_SOURCE_SYSTEM = 5;
}

message Header { string name = 1; bytes value = 2; }   // bytes: Header dürfen non-UTF8 sein

message BodyRef {
  bytes sha256 = 1;
  uint64 size = 2;
  bool truncated = 3;
  string content_type = 4;
}

message Authority { string host = 1; uint32 port = 2; bool is_ip_literal = 3; string display_host = 4; }  // display_host: U-Label für UI

message HttpRequest {
  Method method = 1;
  string method_raw = 2;         // bei METHOD_OTHER
  Scheme scheme = 3;
  Authority authority = 4;
  string path_and_query = 5;
  repeated Header headers = 6;
  BodyRef body = 7;
  string version = 8;            // "HTTP/1.1", "HTTP/2"
}

message HttpResponseHead { uint32 status = 1; repeated Header headers = 2; string version = 3; }

enum FindingTier { FINDING_TIER_UNSPECIFIED = 0; FINDING_TIER_CHECKSUM = 1; FINDING_TIER_REGEX = 2; FINDING_TIER_USER_TERM = 3; }
enum FindingLocation { FINDING_LOCATION_UNSPECIFIED = 0; FINDING_LOCATION_HEADER = 1; FINDING_LOCATION_QUERY = 2; FINDING_LOCATION_BODY = 3; }

message Finding {
  string kind = 1;               // "api_key.github", "email", "iban", "user_term", ...
  FindingLocation location = 2;
  string header_name = 3;        // bei HEADER
  uint64 span_start = 4;
  uint64 span_end = 5;
  FindingTier tier = 6;
  bytes value_hash = 7;
  string display_prefix = 8;     // z. B. "ghp_ab…"
  bool resolved = 9;
}

enum Severity { SEVERITY_UNSPECIFIED = 0; SEVERITY_INFO = 1; SEVERITY_WARNING = 2; SEVERITY_ERROR = 3; SEVERITY_BLOCKING = 4; }

message FixAction {
  oneof action {
    SetEnv set_env = 1;
    Rule add_rule = 2;
    google.protobuf.Empty install_service = 3;
    ChangeSetting change_setting = 4;
    string copy_command = 5;
    string open_url = 6;
    string remount_read_only = 7;
  }
  message SetEnv { string key = 1; string value = 2; }
  message ChangeSetting { string key = 1; string value = 2; }
}

message Diagnostic {
  string code = 1;               // "SANDBOX_001"
  Severity severity = 2;
  string title = 3;
  string why = 4;
  FixAction fix = 5;
  string docs_url = 6;
}

// ---------- Flows ----------

message FlowRef { string flow_id = 1; }

message FlowSummary {
  string flow_id = 1;
  string session_id = 2;
  google.protobuf.Timestamp received_at = 3;
  Method method = 4;
  string method_raw = 5;
  Scheme scheme = 6;
  Authority authority = 7;
  string path = 8;
  FlowState state = 9;
  DecisionKind decision = 10;
  DecisionSource decision_source = 11;
  BlockReason block_reason = 12;
  string rule_id = 13;
  uint32 status = 14;
  uint64 request_size = 15;
  uint64 response_size = 16;
  google.protobuf.Duration duration = 17;
  uint32 finding_count = 18;
  bool edited = 19;
  bool passthrough = 20;
  google.protobuf.Timestamp deadline = 21;   // nur bei HELD
  string origin_tool = 22;                   // optional: "webfetch", "curl", leer wenn unbekannt
}

message FlowDetail {
  FlowSummary summary = 1;
  HttpRequest request = 2;
  HttpRequest edited_request = 3;       // nur bei ALLOW_EDITED
  HttpResponseHead response = 4;
  BodyRef response_body = 5;
  repeated Finding findings = 6;
  repeated Diagnostic diagnostics = 7;
}

message SubscribeRequest { string since_flow_id = 1; bool include_passthrough = 2; }

message FlowEvent {
  google.protobuf.Timestamp at = 1;
  oneof event {
    FlowSummary received = 2;
    Analyzed analyzed = 3;
    Held held = 4;
    Decided decided = 5;
    FlowRef forwarded = 6;
    ResponseHeaders response_headers = 7;
    ResponseChunk response_chunk = 8;
    FlowRef recorded = 9;
    FlowRef timed_out = 10;
    Lagged lagged = 11;
    Diagnostic diagnostic = 12;         // sessionweite Diagnostics (z. B. TLS-Ablehnung)
  }
  message Analyzed { string flow_id = 1; repeated Finding findings = 2; }
  message Held { string flow_id = 1; google.protobuf.Timestamp deadline = 2; }
  message Decided { string flow_id = 1; DecisionKind kind = 2; DecisionSource source = 3; BlockReason block_reason = 4; string rule_id = 5; }
  message ResponseHeaders { string flow_id = 1; HttpResponseHead head = 2; bool streaming = 3; }
  message ResponseChunk { string flow_id = 1; uint64 bytes_so_far = 2; }   // keine Daten, nur Fortschritt
  message Lagged { uint64 dropped = 1; }
}

message ListFlowsRequest {
  string filter = 1;             // Syntax wie History-Screen: "host:github.com state:blocked"
  string since_flow_id = 2;
  string cursor = 3;
  uint32 limit = 4;              // Default 200, Max 1000
  string order_by = 5;           // "received_at desc"
  bool include_passthrough = 6;
}
message FlowPage { repeated FlowSummary flows = 1; string next_cursor = 2; uint64 total = 3; }

message BodyChunk { bytes data = 1; uint64 offset = 2; bool last = 3; }

// ---------- Decide ----------

message DecideRequest {
  repeated string flow_ids = 1;   // Batch möglich
  oneof decision {
    google.protobuf.Empty allow = 2;
    HttpRequest allow_edited = 3;  // nur bei genau einer flow_id
    google.protobuf.Empty block = 4;
  }
  Rule remember = 5;             // optional: Regel, die zusätzlich angelegt wird
  bool acknowledge_findings = 6; // Nutzer hat offene Findings gesehen
}
message DecideResponse { repeated DecideResult results = 1; string created_rule_id = 2; }
message DecideResult { string flow_id = 1; bool applied = 2; Diagnostic diagnostic = 3; }

// ---------- Rules ----------

enum RuleAction { RULE_ACTION_UNSPECIFIED = 0; RULE_ACTION_ALLOW = 1; RULE_ACTION_BLOCK = 2; RULE_ACTION_ASK = 3; RULE_ACTION_REDACT = 4; }
enum Upgrade { UPGRADE_UNSPECIFIED = 0; UPGRADE_NONE = 1; UPGRADE_WEBSOCKET = 2; }

message RuleMatcher {
  string host = 1;                // Label-Glob, "ip:…", "cidr:…"
  repeated Method methods = 2;
  string path = 3;
  Scheme scheme = 4;
  uint32 port = 5;
  Upgrade upgrade = 6;
}
message RuleExpiry {
  oneof expiry {
    google.protobuf.Empty never = 1;
    google.protobuf.Empty session = 2;
    google.protobuf.Timestamp at = 3;
  }
}
message Rule {
  string rule_id = 1;
  RuleAction action = 2;
  RuleMatcher matcher = 3;
  RuleExpiry expires = 4;
  bool stream = 5;
  string created_from_flow_id = 6;
  bool bundled = 7;
  string note = 8;
  google.protobuf.Timestamp created_at = 9;
  uint32 position = 10;
  uint64 hit_count = 11;
}

message RulesRequest {
  oneof op {
    google.protobuf.Empty list = 1;
    Rule add = 2;
    Rule update = 3;
    string remove = 4;
    Reorder reorder = 5;
    DryRun dry_run = 6;
    string make_permanent = 7;     // rule_id einer Session-Regel
  }
  message Reorder { repeated string rule_ids_in_order = 1; }
  message DryRun { Rule rule = 1; uint32 limit = 2; }
}
message RulesResponse {
  repeated Rule rules = 1;
  repeated FlowSummary dry_run_matches = 2;
  Diagnostic diagnostic = 3;
}

// ---------- Sandbox ----------

enum SandboxState { SANDBOX_STATE_UNSPECIFIED = 0; SANDBOX_STATE_STOPPED = 1; SANDBOX_STATE_STARTING = 2; SANDBOX_STATE_RUNNING = 3; SANDBOX_STATE_STOPPING = 4; SANDBOX_STATE_FAILED = 5; }
enum IsolationCheck { ISOLATION_CHECK_UNSPECIFIED = 0; ISOLATION_CHECK_NO_NETWORK_INTERFACE = 1; ISOLATION_CHECK_SINGLE_SOCKET = 2; ISOLATION_CHECK_SECCOMP_ACTIVE = 3; }

message SandboxRequest {
  oneof op {
    Start start = 1;
    google.protobuf.Empty stop = 2;
    google.protobuf.Empty status = 3;
    google.protobuf.Empty isolation_check = 4;
    google.protobuf.Empty argv = 5;
  }
  message Start { string profile = 1; string work_dir = 2; string work_mode = 3; repeated string command = 4; }
}
message CheckResult { IsolationCheck check = 1; bool passed = 2; string evidence = 3; Diagnostic diagnostic = 4; }
message SandboxEvent {
  oneof event {
    Status status = 1;
    CheckResult check = 2;
    string argv_line = 3;
    Diagnostic diagnostic = 4;
    LogLine log = 5;
  }
  message Status { SandboxState state = 1; string sandbox_id = 2; string session_id = 3; string backend = 4; string llm_endpoint = 5; string work_dir = 6; string work_mode = 7; }
  message LogLine { google.protobuf.Timestamp at = 1; string line = 2; }
}

// ---------- Terminal ----------

message TerminalInput {
  oneof input {
    Open open = 1;
    bytes data = 2;
    Resize resize = 3;
    google.protobuf.Empty close = 4;
  }
  message Open { string sandbox_id = 1; uint32 cols = 2; uint32 rows = 3; }
  message Resize { uint32 cols = 1; uint32 rows = 2; }
}
message TerminalOutput {
  oneof output {
    bytes data = 1;
    Exit exit = 2;
    Diagnostic diagnostic = 3;
  }
  message Exit { int32 code = 1; }
}

// ---------- Audit ----------

message AuditRequest {
  oneof op {
    google.protobuf.Empty verify = 1;
    Export export = 2;
    google.protobuf.Empty head = 3;
  }
  message Export { string format = 1; string out_path = 2; bool redact_hosts = 3; }
}
message AuditResponse {
  bool ok = 1;
  uint64 entries = 2;
  bytes head_hash = 3;
  uint64 first_bad_seq = 4;
  string out_path = 5;
  Diagnostic diagnostic = 6;
}

// ---------- Config ----------

message GetConfigRequest { bool include_schema = 1; }
message ConfigSnapshot {
  string toml = 1;                       // effektive Config
  string json_schema = 2;                // bei include_schema
  repeated FieldOrigin origins = 3;
  repeated Diagnostic diagnostics = 4;
}
message FieldOrigin { string key = 1; string origin = 2; }   // "default" | "global" | "profile:global" | "profile:project" | "env" | "cli"
message SetConfigRequest { string key = 1; string value = 2; }
```

`proto/buf.gen.yaml`:

```yaml
version: v2
plugins:
  - remote: buf.build/protocolbuffers/dart:v22.0.0
    out: ../app/lib/core/ipc/generated
    opt: [grpc]
```

Rust-Seite über `tonic-build` in `daemon/crates/ipc/build.rs`:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["../../../proto/humanitl/v1/humanitl.proto"], &["../../../proto"])?;
    println!("cargo:rerun-if-changed=../../../proto/humanitl/v1/humanitl.proto");
    Ok(())
}
```

`scripts/gen-proto.sh`:

```sh
#!/usr/bin/env sh
set -eu
cd "$(dirname "$0")/.."
rm -rf app/lib/core/ipc/generated
mkdir -p app/lib/core/ipc/generated
( cd proto && buf generate )
( cd daemon && cargo build -p humanitl-ipc )
echo "proto generated"
```

Für den Drift-Check in CI (HUM-002) wird der Dart-Output in einem separaten Schritt erzeugt und mit einem eingecheckten Hash verglichen, weil `generated/` gitignored ist: `scripts/gen-proto.sh` schreibt zusätzlich `proto/generated.sha256` (Hash über alle erzeugten Dart-Dateien, sortiert). CI vergleicht diese Datei per `git diff --exit-code proto/generated.sha256`.

### Schritte
1. `buf.yaml`, `buf.gen.yaml`, Proto anlegen. `buf lint proto` sauber machen.
2. `humanitl-ipc` mit `build.rs`, `Cargo.toml` (`tonic = "0.13"`, `prost = "0.13"`, `prost-types`, build-dep `tonic-build`, `protoc` muss installiert sein). `lib.rs`: `pub mod v1 { tonic::include_proto!("humanitl.v1"); }`. Bauen.
3. Dart: `grpc: ^5.1.0`, `protobuf: ^4.0.0` in `pubspec.yaml`, `dart pub global activate protoc_plugin 22.0.0`. `scripts/gen-proto.sh` laufen lassen, `flutter analyze` sauber (evtl. `analysis_options.yaml` mit `exclude: [lib/core/ipc/generated/**]`).
4. `generated.sha256` erzeugen und einchecken. CI-Job `proto-lint-and-gen` in HUM-002 aktivieren (`if: false` entfernen, Drift-Check auf die Hash-Datei umstellen).
5. `docs/PROTOCOL.md` (neu, kurz): wie man die Proto ändert (additiv, Feldnummern nie recyceln, `buf breaking`), Versionsregel (Minor bei additiv, Major bei Break).

### Tests
- `daemon/crates/ipc/tests/proto_roundtrip.rs`: `fn flow_event_roundtrip()` baut ein `FlowEvent::Held`, encodiert mit prost, decodiert, vergleicht. `fn enums_have_unspecified_zero()` prüft per `Method::from_i32(0) == Some(Method::Unspecified)` für alle Enums.
- Dart: `app/test/core/ipc/generated_smoke_test.dart`: instanziiert `Info()`, `FlowEvent()`, setzt Felder, `writeToBuffer()`/`fromBuffer()` Roundtrip.

### Akzeptanzkriterien
- [ ] `buf lint proto` exit 0
- [ ] `cargo build -p humanitl-ipc` exit 0, `cargo doc -p humanitl-ipc` ohne Warnungen (generierten Code mit `#[allow(missing_docs)]` im Modul umschließen)
- [ ] `./scripts/gen-proto.sh` idempotent: zweiter Lauf ändert `proto/generated.sha256` nicht
- [ ] Jedes Enum hat `_UNSPECIFIED = 0`
- [ ] Jede Message aus BACKLOG.md 3.3 existiert; zusätzlich `GetConfig`/`SetConfig` für HUM-062/069
- [ ] CI-Job `proto-lint-and-gen` grün

### Fallstricke
- Proto-Feldnamen `snake_case`, Enum-Werte mit Typ-Präfix (`METHOD_GET`), sonst schlägt `buf lint` STANDARD an.
- `google/protobuf/*.proto` Imports brauchen bei `tonic-build` das Include-Verzeichnis von `protoc`; `protoc` aus `arduino/setup-protoc` bringt sie mit. Lokal `apt install protobuf-compiler` oder `protoc` aus Release.
- Header-Werte als `bytes`, nicht `string`. HTTP-Header sind nicht garantiert UTF-8; prost wirft sonst bei Decode.
- Nie Bodies in Events. `ResponseChunk` trägt nur `bytes_so_far`.
- `Decide.allow_edited` mit mehreren `flow_ids` ist ein Fehler: Server antwortet mit `DecideResult.diagnostic` Code `IPC_002`. Im Proto-Kommentar festhalten.
- Dart-Codegen-Plugin-Version und `protobuf`-Paketversion müssen zusammenpassen (22.x ↔ 4.x). Beide pinnen.
- `buf breaking` gegen `main` braucht `fetch-depth: 0` im Checkout.

### Referenzen
BACKLOG.md 3.3, ADR-003, CONVENTIONS.md 3.6, tonic (docs.rs/tonic), buf (buf.build/docs/lint), grpc-dart UDS (github.com/grpc/grpc-dart/issues/299).

---

## HUM-004 · core-types Crate
Sprint: 0 · Größe: M · Abhängigkeiten: HUM-001 · Blockiert: HUM-063, HUM-062, HUM-005, HUM-015, HUM-016

### Kontext
ADR-004: Der Request-Lebenszyklus ist ein Zustandsautomat mit expliziten, geprüften Übergängen. Aus jedem Übergang wird ein Event abgeleitet, das gRPC und Audit speist. Typisierte IDs verhindern das Vertauschen von Flow-, Rule- und Session-IDs. Diese Crate hat keine IO-Abhängigkeit und ist der Boden, auf dem `rules`, `findings`, `proxy`, `recorder` stehen.

### Ziel
`humanitl-core` exportiert die Typen aus CONVENTIONS.md 3.2 (ohne `Diagnostic`, das kommt in HUM-063 in dieselbe Crate), den Zustandsautomaten `FlowState::on`, die Event-Ableitung, und eine `HostName`-Normalisierung. Alle Übergänge sind tabellengetrieben getestet, erlaubte wie verbotene.

### Nicht-Ziel
Keine Regeln (HUM-022), keine Detektoren (HUM-025), kein Proto-Mapping (HUM-018), kein `Diagnostic` (HUM-063).

### Betroffene Pfade
- `daemon/crates/core-types/Cargo.toml` (`uuid` v7+serde, `bytes`, `http` 1.x, `thiserror`, `serde`, `idna`, `time` oder `jiff`)
- `daemon/crates/core-types/src/lib.rs`
- `daemon/crates/core-types/src/ids.rs` (neu)
- `daemon/crates/core-types/src/host.rs` (neu)
- `daemon/crates/core-types/src/http.rs` (neu)
- `daemon/crates/core-types/src/finding.rs` (neu)
- `daemon/crates/core-types/src/flow.rs` (neu)
- `daemon/crates/core-types/src/event.rs` (neu)
- `daemon/crates/core-types/tests/flow_state_table.rs` (neu)
- `daemon/crates/core-types/tests/host_normalize.rs` (neu)

### Spezifikation

`ids.rs`: Makro `typed_id!(FlowId)` erzeugt `#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)] pub struct FlowId(Uuid)` mit `pub fn new() -> Self { Self(Uuid::now_v7()) }`, `pub fn parse(s: &str) -> Result<Self, IdParseError>`, `Display` als Hyphenated, `FromStr`. Vier IDs: `FlowId`, `RuleId`, `SessionId`, `SandboxId`. `Ord` über die UUID-Bytes ergibt bei v7 Zeitordnung.

`host.rs`:

```rust
pub enum HostName { Dns(String), Ip(IpAddr) }
pub struct HostParseError { pub input: String, pub reason: &'static str }
impl HostName {
    /// Normalisiert: IDNA-to-ASCII (idna::domain_to_ascii_strict), lowercase, trailing dot entfernt,
    /// "[::1]" und "1.2.3.4" werden zu Ip. Nicht-kanonische IP-Formen ("0x7f.1", "0177.0.0.1") sind Fehler.
    pub fn parse(input: &str) -> Result<HostName, HostParseError>;
    pub fn labels(&self) -> Option<Vec<&str>>;           // None bei Ip
    pub fn display(&self) -> String;                     // U-Label für UI, bei Ip die IP
    pub fn is_private(&self) -> bool;                    // RFC1918, loopback, link-local, ULA, 169.254.169.254
}
```

`http.rs`: `HttpRequest`, `Authority`, `Scheme { Http, Https, Ws, Wss }`, `BodyRef { sha256: [u8;32], size: u64, inline: Option<Bytes>, content_type: Option<String>, truncated: bool }`, `Upgrade { WebSocket }`. `Method` und `HeaderMap` aus der `http`-Crate re-exportieren. `Authority::default_port(scheme)`.

`finding.rs`: `Finding`, `FindingKind`, `FindingLocation`, `Tier` exakt wie CONVENTIONS.md 3.2. `Finding::display_prefix` ist maximal 8 Zeichen des Originals plus `…`, nie mehr. `value_hash` = SHA-256 über die Bytes des Werts (Hashing selbst mit `sha2`).

`flow.rs`:

```rust
pub enum FlowState { Received, Analyzed { findings: Vec<Finding> }, Held { deadline: Instant }, Decided(Decision), Forwarded, Responded { status: u16 }, Recorded }
pub enum Decision { Allow, AllowEdited { request: Box<HttpRequest> }, Block { reason: BlockReason }, TimedOut }
pub enum BlockReason { User, Rule(RuleId), Timeout, BodyCap, AuthorityMismatch, NoRoute }
pub enum DecisionSource { User, Rule(RuleId), Timeout, Passthrough, System }

pub enum Transition {           // Eingaben des Automaten (nicht identisch mit FlowEvent)
    Analyze { findings: Vec<Finding> },
    Hold { deadline: Instant },
    Decide { decision: Decision, source: DecisionSource },
    Forward,
    Respond { status: u16 },
    Record,
    Timeout,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid transition from {from} on {input}")]
pub struct InvalidTransition { pub from: &'static str, pub input: &'static str }

impl FlowState {
    pub fn on(self, t: Transition) -> Result<(FlowState, FlowEvent), InvalidTransition>;
    pub fn name(&self) -> &'static str;   // "received", "analyzed", ...
    pub fn is_terminal(&self) -> bool;    // Recorded
}
```

Übergangstabelle (alles andere ist `InvalidTransition`):

| von | Eingabe | nach | Event |
|---|---|---|---|
| Received | Analyze | Analyzed | Analyzed |
| Analyzed | Hold | Held | Held |
| Analyzed | Decide (nur source Rule oder Passthrough) | Decided | Decided |
| Held | Decide (source User oder Rule) | Decided | Decided |
| Held | Timeout | Decided(TimedOut) | TimedOut |
| Decided(Allow \| AllowEdited) | Forward | Forwarded | Forwarded |
| Decided(Block \| TimedOut) | Record | Recorded | Recorded |
| Forwarded | Respond | Responded | ResponseHeaders |
| Responded | Record | Recorded | Recorded |
| Forwarded | Record (Upstream-Fehler ohne Antwort) | Recorded | Recorded |

`event.rs`: `FlowEvent` mit den Varianten aus CONVENTIONS.md 3.2, jede trägt `flow_id: FlowId` und `at: SystemTime`. `Lagged { n }` wird nicht vom Automaten erzeugt, sondern vom IPC-Layer. `ResponseChunk` ebenfalls nicht vom Automaten (kein Zustandswechsel), sondern vom Proxy direkt.

`Flow` (Aggregat): `pub struct Flow { pub id: FlowId, pub session: SessionId, pub received_at: SystemTime, pub request: HttpRequest, pub state: FlowState, pub history: Vec<(SystemTime, &'static str)> }` mit `pub fn apply(&mut self, t: Transition) -> Result<FlowEvent, InvalidTransition>`, das den Zustand ersetzt und `history` anhängt.

### Schritte
1. `ids.rs` mit Makro und vier IDs. Test: `FlowId::new()` zweimal hintereinander, zweite ist `>` erste.
2. `host.rs` mit `parse`, `labels`, `display`, `is_private`. Tests aus „Tests".
3. `http.rs`, `finding.rs`.
4. `flow.rs` mit Automat und `event.rs`. Tabellentest schreiben, bevor `on` implementiert wird; Test muss zuerst rot sein.
5. `Flow::apply`. Doku-Kommentare auf allen öffentlichen Items. `cargo doc` ohne Warnungen.

### Tests
`tests/flow_state_table.rs`:
- `fn allowed_transitions_table()`: Vektor aller Zeilen der Übergangstabelle, für jede: Startzustand bauen, `on` aufrufen, Zielzustand per `name()` prüfen, Event-Variante prüfen.
- `fn forbidden_transitions_are_errors()`: Kreuzprodukt aller sieben Zustände × sieben Eingaben, alle Paare, die nicht in der Tabelle stehen, müssen `Err(InvalidTransition)` liefern. Anzahl geprüfter verbotener Paare muss ≥ 35 sein (Assertion auf den Zähler, damit der Test nicht leer läuft).
- `fn analyzed_decide_only_by_rule_or_passthrough()`: `Analyzed` + `Decide{source: User}` ist `Err`.
- `fn recorded_is_terminal()`: `Recorded` + jede Eingabe ist `Err`.
- `fn flow_apply_appends_history()`.

`tests/host_normalize.rs`:
- `("GitHub.COM.", Dns("github.com"))`, `("münchen.de", Dns("xn--mnchen-3ya.de"))`, `("192.168.1.50", Ip)`, `("[::1]", Ip)`, `("0x7f.1", Err)`, `("0177.0.0.1", Err)`, `("", Err)`, `("a..b", Err)`, `("exa mple.com", Err)`.
- `fn display_returns_ulabel()`: `xn--mnchen-3ya.de` → `münchen.de`.
- `fn is_private_table()`: `10.0.0.1` true, `169.254.169.254` true, `8.8.8.8` false, `fc00::1` true, `::1` true.

### Akzeptanzkriterien
- [ ] `cargo test -p humanitl-core` grün, mindestens 12 Tests
- [ ] Verbotene Übergänge: Zähler ≥ 35 im Test
- [ ] `cargo doc -p humanitl-core` ohne Warnung
- [ ] Keine Abhängigkeit auf tokio, sqlite, tonic in `Cargo.toml` dieser Crate
- [ ] `FlowId::new()` ist zeitgeordnet (Test)

### Fallstricke
- `Uuid::new_v4()` statt `now_v7()`: verliert Zeitordnung, `ListFlows(since)` bricht später.
- `idna::domain_to_ascii` (nicht strict) akzeptiert Müll. `_strict` nehmen. IDNA-Crate-Version ≥ 1.0, API heißt dort `idna::domain_to_ascii_strict` oder über `idna::Uts46`.
- `Instant` ist nicht serialisierbar; `deadline` bleibt im In-Memory-Zustand, für IPC wird in `SystemTime` umgerechnet (Aufgabe von `ipc`).
- Tests nur für erlaubte Übergänge schreiben ist die typische Lücke. Das Kreuzprodukt ist Pflicht.
- `HeaderMap` aus `http` 1.x, nicht 0.2 (hudsucker 0.25 nutzt hyper 1 / http 1).
- `Decision::AllowEdited` hält einen ganzen Request; `Box` verhindert, dass `FlowState` riesig wird.

### Referenzen
ADR-004, CONVENTIONS.md 3.2, uuid v7 (docs.rs/uuid), idna (docs.rs/idna).

---

## HUM-063 · Diagnostic-Typ
Sprint: 0 · Größe: S · Abhängigkeiten: HUM-004, HUM-003 · Blockiert: HUM-062, alle Issues mit Fehlerpfaden

### Kontext
ADR-012 und Prinzip 7: Jeder nicht-grüne Zustand trägt Grund und Fix. Damit das nicht Prosa bleibt, ist `Diagnostic` ein Typ mit einer Code-Registry, und CI lehnt `Err(String)` in öffentlichen Daemon-Pfaden ab.

### Ziel
`humanitl-core::diag` exportiert `Diagnostic`, `Severity`, `FixAction`, `DiagnosticCode`, eine Registry aller Codes mit Titel-Vorlage und Doku-Anker, und eine Konvertierung nach Proto (in `ipc`, hier nur der Typ). Ein Lint-Skript in CI findet `Result<_, String>` und `anyhow::Error` in öffentlichen Signaturen der Daemon-Crates.

### Nicht-Ziel
Keine UI-Darstellung (HUM-019/068). Keine konkreten Diagnostics für Sandbox/TLS (kommen mit den jeweiligen Issues, hier nur der Rahmen und die ersten Codes).

### Betroffene Pfade
- `daemon/crates/core-types/src/diag.rs` (neu)
- `daemon/crates/core-types/src/diag_codes.rs` (neu, Registry)
- `docs/DIAGNOSTICS.md` (neu, generiert aus Registry per Test)
- `scripts/ci/lint-no-string-errors.sh` (neu)
- `.github/workflows/ci.yml` (Schritt im Job `rust-check`)

### Spezifikation

```rust
pub struct DiagnosticCode(pub &'static str);        // "SANDBOX_001"
pub enum Severity { Info, Warning, Error, Blocking } // Blocking = Aktion (z. B. Start) wird verweigert
pub enum FixAction {
    SetEnv { key: String, value: String },
    AddRule(Box<humanitl_core::rule::Rule>),   // Rule, Matcher, Action, Expiry, HostPattern liegen in core (Abgleich 2026-09-02)
    InstallService,
    ChangeSetting { key: String, value: String },
    CopyCommand(String),
    OpenUrl(String),
    RemountReadOnly(PathBuf),
}
pub struct Diagnostic { pub code: DiagnosticCode, pub severity: Severity, pub title: String, pub why: String, pub fix: Option<FixAction>, pub docs: Option<String> }

impl Diagnostic {
    pub fn new(code: DiagnosticCode, severity: Severity) -> DiagnosticBuilder;   // title aus Registry
}
pub struct DiagnosticBuilder { .. }
impl DiagnosticBuilder { pub fn why(self, s: impl Into<String>) -> Self; pub fn fix(self, f: FixAction) -> Self; pub fn build(self) -> Diagnostic; }

/// Registry: statische Tabelle. Jeder Code genau einmal.
pub struct CodeInfo { pub code: DiagnosticCode, pub area: &'static str, pub title: &'static str, pub docs_anchor: &'static str }
pub static CODES: &[CodeInfo] = &[ /* … */ ];
pub fn lookup(code: DiagnosticCode) -> Option<&'static CodeInfo>;
```

Bereiche und erste Codes (Registry initial, wird von späteren Issues erweitert, Nummern nie recycelt):

| Code | Bereich | Titel |
|---|---|---|
| DAEMON_001 | daemon | Daemon nicht erreichbar |
| DAEMON_002 | daemon | Proto-Version inkompatibel |
| IPC_001 | ipc | Ungültiges Token |
| IPC_002 | ipc | AllowEdited nur für genau einen Flow |
| IPC_003 | ipc | Flow nicht mehr gehalten |
| CONFIG_001 | config | Config-Datei ungültig |
| CONFIG_002 | config | Unbekannter Schlüssel |
| CONFIG_003 | config | Wert außerhalb des Bereichs |
| SANDBOX_001 | sandbox | bwrap nicht gefunden |
| SANDBOX_002 | sandbox | bwrap-Version zu alt |
| SANDBOX_003 | sandbox | User-Namespaces nicht erlaubt |
| SANDBOX_004 | sandbox | Isolation-Check fehlgeschlagen |
| SANDBOX_005 | sandbox | Projektordner nicht beschreibbar |
| PROXY_001 | proxy | Body über Cap |
| PROXY_002 | proxy | Authority-Mismatch |
| TLS_001 | tls | Client hat Humanitl-CA abgelehnt |
| LLM_001 | llm | LLM-Endpoint nicht erreichbar |
| LLM_002 | llm | LLM-Endpoint antwortet nicht als OpenAI-kompatible API |
| RULES_001 | rules | Regel-Datei ungültig |
| RULES_002 | rules | Host-Muster verdächtig (xn--, IP in Host-Glob) |
| AUDIT_001 | audit | Hash-Kette gebrochen |

`docs/DIAGNOSTICS.md` wird von einem Test erzeugt: `tests/diag_docs.rs` rendert die Registry als Markdown-Tabelle und vergleicht mit der Datei; bei Abweichung schlägt der Test fehl mit Hinweis `UPDATE_DIAG_DOCS=1 cargo test` zum Überschreiben.

`scripts/ci/lint-no-string-errors.sh`:

```sh
#!/usr/bin/env sh
set -eu
# Öffentliche Signaturen in Lib-Crates dürfen keine String- oder anyhow-Fehler tragen.
bad=$(grep -rnE 'pub (async )?fn [^{]*Result<[^,>]+, *(String|anyhow::Error|Box<dyn (std::error::)?Error[^>]*>)>' daemon/crates --include='*.rs' | grep -v '/tests/' || true)
if [ -n "$bad" ]; then echo "$bad"; echo "::error::public fns must return typed errors (see HUM-063)"; exit 1; fi
```

### Schritte
1. `diag.rs`, `diag_codes.rs` mit Registry-Tabelle und `lookup`.
2. Builder mit Titel aus Registry; unbekannter Code ist ein Compile-Zeit-Fehler nicht möglich, also Laufzeit-`debug_assert!` plus Test, der alle Codes in `CODES` auf Eindeutigkeit prüft.
3. `tests/diag_docs.rs` und `docs/DIAGNOSTICS.md` erzeugen.
4. Lint-Skript, in `rust-check` einhängen.
5. `lib.rs` re-exportiert `diag::*`.

### Tests
- `fn codes_are_unique()`: Set-Größe == Slice-Länge.
- `fn codes_follow_schema()`: Regex `^[A-Z]+_[0-9]{3}$`.
- `fn builder_uses_registry_title()`.
- `fn docs_in_sync()` (siehe oben).
- Lint-Skript: negatives Fixture in `scripts/ci/fixtures/bad_signature.rs.txt` wird vom Skript erkannt (Test im Skript selbst: `sh -c` gegen Fixture-Verzeichnis, erwartet exit 1).

### Akzeptanzkriterien
- [ ] `Diagnostic` ohne `why` lässt sich nicht bauen (Builder verlangt `why` vor `build`; typestate oder `build()` liefert `Result`)
- [ ] `docs/DIAGNOSTICS.md` existiert und Test `docs_in_sync` grün
- [ ] Lint-Skript in CI aktiv und grün auf aktuellem Stand
- [ ] Alle 21 initialen Codes in Registry, Test `codes_are_unique` grün

### Fallstricke
- `FixAction::AddRule` braucht den `Rule`-Typ. Entschieden: `Rule`, `Matcher`, `Action`, `Expiry`, `HostPattern` liegen als reine Werttypen in `humanitl-core::rule`; `humanitl-rules` enthält nur Parsen (YAML) und `RuleSet::evaluate`. Kein Zyklus, `catalog` kann `HostPattern` ebenfalls nutzen.
- Titel im Code, `why` zur Laufzeit: `why` muss konkrete Werte enthalten (Pfad, Port, Version), sonst ist es nutzlos. Im Doku-Kommentar von `why()` festhalten.
- Lint-Regex ist grob; falsche Positive in Kommentaren durch `grep -v '^\s*//'` reduzieren.

### Referenzen
ADR-012, BACKLOG.md 1.3 Prinzip 7, CONVENTIONS.md 3.2.

---

## HUM-062 · config Crate mit Schema
Sprint: 0 · Größe: M · Abhängigkeiten: HUM-004, HUM-063 · Blockiert: HUM-010, HUM-064, HUM-069

### Kontext
ADR-011: eine Konfigurationsquelle, drei Sichtbarkeitsstufen. Settings sind Rust-Typen; daraus entstehen TOML-Schema, CLI-Flags, Settings-Screen, Doku. Präzedenz über fünf Ebenen mit Herkunfts-Tracking pro Feld.

### Ziel
`humanitl-config` lädt eine `Config` aus Defaults, globaler TOML, globalem Profil, Projektprofil, Env, CLI-Overrides, in dieser Reihenfolge, und liefert neben dem Wert für jedes Blattfeld die Herkunft. `Config::json_schema()` gibt ein JSON-Schema mit `x-tier` und `description` pro Feld. Fehler sind `Diagnostic`s mit `CONFIG_*`-Codes.

### Nicht-Ziel
Kein Profil-Bündel mit Regeln/Agent (HUM-066), keine CLI (HUM-064), kein Settings-Screen (HUM-069). Nur die Config-Struktur, das Laden, das Schema.

### Betroffene Pfade
- `daemon/crates/config/Cargo.toml` (`serde`, `toml`, `schemars` 1.x, `figment` oder eigene Merge-Logik, `directories` für XDG, `thiserror`)
- `daemon/crates/config/src/lib.rs`
- `daemon/crates/config/src/model.rs` (neu, Config-Typen)
- `daemon/crates/config/src/tier.rs` (neu)
- `daemon/crates/config/src/load.rs` (neu, Präzedenz)
- `daemon/crates/config/src/origin.rs` (neu)
- `daemon/crates/config/src/paths.rs` (neu, XDG)
- `daemon/crates/config/tests/precedence.rs`, `tests/schema.rs`, `tests/fixtures/*.toml` (neu)
- `docs/CONFIG.md` (neu, aus Schema generiert per Test wie bei DIAGNOSTICS)

### Spezifikation

`model.rs` (vollständig für Sprint 0; spätere Issues fügen Felder hinzu, nie um):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub llm: LlmConfig,
    pub hold: HoldConfig,
    pub sandbox: SandboxRef,
    pub agent: AgentRef,
    pub recorder: RecorderConfig,
    pub preview: PreviewConfig,
    pub ipc: IpcConfig,
    pub ui: UiConfig,
    pub experimental: Experimental,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct LlmConfig {
    /// OpenAI-kompatibler Endpoint im LAN. Verkehr dorthin wird nicht angehalten, aber geloggt.
    #[schemars(extend("x-tier" = "basic"))]
    pub endpoint: Option<url::Url>,
    /// Pfadpräfixe, die als LLM-Passthrough gelten.
    #[schemars(extend("x-tier" = "advanced"))]
    pub passthrough_paths: Vec<String>,          // Default ["/v1/", "/api/"]
}
// HoldConfig { timeout_secs: u64 = 300 [basic], body_cap_bytes: u64 = 32 MiB [advanced], ask_mode: AskMode = Ui [advanced] }
// SandboxRef { profile: String = "default" [advanced], work_dir: Option<PathBuf> [basic], work_mode: WorkMode = Rw [basic] }
// AgentRef { adapter: String = "opencode" [advanced], command: Option<Vec<String>> [expert] }
// RecorderConfig { inline_max_bytes: u64 = 256 KiB [expert], retention_days: u32 = 90 [advanced] }
// PreviewConfig { cap_bytes: u64 = 8 MiB [expert], max_decompress_ratio: u32 = 100 [expert] }
// IpcConfig { event_buffer: usize = 1024 [expert] }
// UiConfig { language: Language = En [basic], theme: Theme = Dark [advanced], notifications: bool = true [advanced], sound: bool = false [advanced] }
// Experimental { h2_upstream: bool = false [expert], ws_hold: bool = false [expert] }
```

Tier: statt eines eigenen Attribut-Makros wird `schemars(extend("x-tier" = "…"))` verwendet. Ein Test prüft, dass jedes Blattfeld im erzeugten Schema `x-tier` und `description` trägt (Doku-Kommentar `///` wird von schemars zu `description`).

`origin.rs`: `pub enum Origin { Default, Global, ProfileGlobal(String), ProfileProject(PathBuf), Env(String), Cli }`, `pub struct Resolved { pub config: Config, pub origins: BTreeMap<String /* "hold.timeout_secs" */, Origin>, pub diagnostics: Vec<Diagnostic> }`.

`load.rs`:

```rust
pub struct Sources { pub global_toml: Option<PathBuf>, pub profile_global: Option<PathBuf>, pub profile_project: Option<PathBuf>, pub env_prefix: &'static str /* "HUMANITL" */, pub cli: Vec<(String, String)> }
pub fn load(sources: &Sources) -> Result<Resolved, Diagnostic>;
pub fn discover(cwd: &Path) -> Sources;   // XDG + <cwd>/.humanitl/profile.toml
```

Algorithmus: (1) `toml::Value` aus Defaults (`Config::default()` serialisiert), (2) für jede Ebene in Reihenfolge: Datei lesen, als `toml::Value` parsen, tief mergen (Tabellen rekursiv, Arrays und Skalare ersetzen), für jeden gesetzten Blattpfad `origins[path] = Origin::X`, (3) Env: alle `HUMANITL_*`-Variablen, Pfad durch `__` getrennt, Wert als TOML-Skalar parsen (Fallback String), (4) CLI-Paare gleich behandeln, (5) finales `Value` in `Config` deserialisieren, `deny_unknown_fields` liefert `CONFIG_002` mit Pfad, Bereichsverletzungen (`timeout_secs == 0`, `body_cap_bytes < 1024`) liefern `CONFIG_003` nach `validate()`.

Profil-Dateien enthalten in Sprint 0 nur einen `[config]`-Block, der wie die globale TOML gemergt wird; die weiteren Blöcke (`[rules]`, `[agent]`) kommen in HUM-066.

`paths.rs`: `config_dir()`, `data_dir()`, `runtime_dir()`, `profiles_dir()`, `rules_path()`, `db_path()`, `ca_dir()`, `audit_path()`, `daemon_socket()`, `proxy_socket()`, `token_path()` gemäß CONVENTIONS.md 3.4. `runtime_dir()` fällt auf `/run/user/<uid>` zurück, dann `$TMPDIR/humanitl-<uid>` mit Warnung `CONFIG_001`-Diagnostic (Info).

### Schritte
1. `model.rs` mit allen Typen, Defaults via `impl Default`, Doku-Kommentar auf jedem Feld.
2. `Config::json_schema()`, Test `every_leaf_has_tier_and_description`.
3. `paths.rs`.
4. `load.rs` mit Merge, Env, CLI, Origins. Fixtures anlegen.
5. `validate()` und Diagnostics.
6. `docs/CONFIG.md`-Generator-Test analog zu HUM-063.

### Tests
`tests/precedence.rs`:
- `fn defaults_when_nothing_set()`: `origins` alle `Default`.
- `fn global_overrides_default()`, `fn profile_project_overrides_profile_global()`, `fn env_overrides_files()` (`HUMANITL_HOLD__TIMEOUT_SECS=42`), `fn cli_overrides_env()`.
- `fn unknown_key_is_config_002()`: Fixture mit `[hold] timeoutt_secs = 1` → `Err(Diagnostic{code: CONFIG_002})`, `why` enthält den Pfad.
- `fn zero_timeout_is_config_003()`.
- `fn env_value_types()`: `"true"` → bool, `"42"` → int, `"abc"` → string.
`tests/schema.rs`:
- `fn every_leaf_has_tier_and_description()`: rekursiv durch `properties`, jedes Blatt hat `x-tier ∈ {basic, advanced, expert}` und nicht-leere `description`.
- `fn schema_is_stable()`: Snapshot in `tests/fixtures/config.schema.json`, Abweichung mit Hinweis auf `UPDATE_SNAPSHOTS=1`.

### Akzeptanzkriterien
- [ ] `cargo test -p humanitl-config` grün, ≥ 10 Tests
- [ ] `Config::json_schema()` enthält alle Schlüssel aus CONVENTIONS.md 3.7
- [ ] Jedes Blattfeld hat Tier und Beschreibung (Test)
- [ ] Präzedenz-Reihenfolge exakt: Default < Global < Profil global < Profil Projekt < Env < CLI (Tests)
- [ ] `docs/CONFIG.md` generiert und in sync

### Fallstricke
- `#[serde(default)]` auf Struct-Ebene plus `deny_unknown_fields` ist die richtige Kombination; nur eines von beiden lässt Tippfehler durch oder erzwingt alle Felder.
- Env-Parsing: `HUMANITL_LLM__ENDPOINT=http://…` enthält `://`, darf nicht als TOML geparst werden (wäre Fehler); Regel: erst als Bool, dann Int, dann Float versuchen, sonst String.
- Deep-Merge darf Arrays nicht zusammenführen, sondern ersetzen; sonst wachsen `passthrough_paths` bei jedem Layer.
- schemars 1.x hat andere API als 0.8 (`extend` statt `schema_with`). Version in Workspace pinnen.
- `url::Url` in JsonSchema braucht das `schemars`-Feature `url2` oder ein `#[schemars(with = "String")]`.

### Referenzen
ADR-011, CONVENTIONS.md 3.4, 3.7, schemars (docs.rs/schemars), XDG Base Directory Spec.

---

## HUM-010 · Sandbox-Profil-Format
Sprint: 0 · Größe: S · Abhängigkeiten: HUM-062 · Blockiert: HUM-006, HUM-011

### Kontext
ADR-002: Die Sandbox-Policy ist eine einzige lesbare bwrap-Kommandozeile. Damit sie deterministisch und prüfbar ist, wird sie aus einer deklarativen TOML-Datei erzeugt, nicht aus Code zusammengeklebt. Das Format muss vor dem Launcher (HUM-011) und dem Escape-Harness (HUM-006) stehen, weil beide es lesen.

### Ziel
`profiles/sandbox/default.toml` und `profiles/sandbox/test.toml` existieren, `humanitl-sandbox::profile` parst sie in `SandboxProfile`, validiert Mount-Allowlist gegen eine Denylist gefährlicher Pfade, und `SandboxProfile::to_bwrap_args(ctx)` erzeugt die vollständige Argument-Liste (noch ohne Ausführung).

### Nicht-Ziel
Kein Start der Sandbox (HUM-011), kein Shim (HUM-012), keine CA-Dateien (HUM-014). Env-Kit ist hier nur die Liste der Schlüssel; Werte für CA-Pfade kommen mit HUM-014.

### Betroffene Pfade
- `profiles/sandbox/default.toml` (neu)
- `profiles/sandbox/test.toml` (neu, minimal für Escape-Tests, `/tests/escape` ro gebunden)
- `daemon/crates/sandbox/Cargo.toml` (`serde`, `toml`, `schemars`, `humanitl-core`, `humanitl-config`)
- `daemon/crates/sandbox/src/lib.rs`
- `daemon/crates/sandbox/src/profile.rs` (neu)
- `daemon/crates/sandbox/src/bwrap_args.rs` (neu)
- `daemon/crates/sandbox/tests/profile_parse.rs`, `tests/bwrap_args_snapshot.rs`, `tests/snapshots/default.argv.txt` (neu)

### Spezifikation

`profiles/sandbox/default.toml` (vollständig):

```toml
version = 1
name = "default"
description = "bwrap sandbox: no network interface, one proxy socket, seccomp after bridge"

[sandbox]
backend = "bwrap"
hostname = "sandbox"
unshare = ["user", "pid", "net", "ipc", "uts", "cgroup"]   # entspricht --unshare-all
die_with_parent = true
new_session = true
min_bwrap_version = "0.8.0"

[mounts]
# src wird zur Laufzeit gesetzt (SessionContext.work_dir); mode aus Config sandbox.work_mode
work = { dst = "/work" }
ro = ["/usr", "/etc/ssl", "/etc/alternatives", "/etc/ld.so.cache", "/etc/localtime"]
symlinks = [["usr/lib", "/lib"], ["usr/lib64", "/lib64"], ["usr/bin", "/bin"], ["usr/sbin", "/sbin"]]
tmpfs = ["/tmp", "/var/tmp", "/dev/shm", "/home/agent", "/work/.git/hooks", "/work/.vscode", "/work/.idea"]
proc = "/proc"
dev = "/dev"
masked_files = ["/work/.envrc", "/work/.git/config"]      # werden als leere ro-Datei überdeckt
extra_ro = []                                              # Nutzer-Erweiterung, gegen Denylist geprüft
extra_rw = []

[network]
proxy_socket_dst = "/run/humanitl/proxy.sock"
proxy_port = 3128
ca_cert_dst = "/etc/humanitl/ca.crt"
shim_dst = "/usr/local/bin/humanitl-shim"

[env]
HOME = "/home/agent"
USER = "agent"
TERM = "xterm-256color"
LANG = "C.UTF-8"
PATH = "/usr/local/bin:/usr/bin:/bin"
HTTP_PROXY = "http://127.0.0.1:3128"
HTTPS_PROXY = "http://127.0.0.1:3128"
http_proxy = "http://127.0.0.1:3128"
https_proxy = "http://127.0.0.1:3128"
NO_PROXY = ""
no_proxy = ""
SSL_CERT_FILE = "/etc/humanitl/ca.crt"
SSL_CERT_DIR = "/etc/ssl/certs"
CURL_CA_BUNDLE = "/etc/humanitl/ca.crt"
REQUESTS_CA_BUNDLE = "/etc/humanitl/ca.crt"
NODE_EXTRA_CA_CERTS = "/etc/humanitl/ca.crt"
DENO_CERT = "/etc/humanitl/ca.crt"
GIT_SSL_CAINFO = "/etc/humanitl/ca.crt"
CARGO_HTTP_CAINFO = "/etc/humanitl/ca.crt"
PIP_CERT = "/etc/humanitl/ca.crt"
NPM_CONFIG_CAFILE = "/etc/humanitl/ca.crt"
NIX_SSL_CERT_FILE = "/etc/humanitl/ca.crt"
```

`profiles/sandbox/test.toml`: wie default, plus `extra_ro = ["/tests/escape"]` (Host-Pfad wird vom Runner ersetzt) und `[env] HUMANITL_TEST = "1"`.

Denylist (hart im Code, nicht konfigurierbar): Kein Mount, dessen Host-Quelle unter `/run/user`, `$XDG_RUNTIME_DIR`, `/tmp/.X11-unix`, `/var/run/docker.sock`, `/run/docker.sock`, `~/.ssh`, `~/.gnupg`, `~/.gitconfig`, `~/.netrc`, `~/.config/humanitl`, `~/.local/share/humanitl`, `/proc`, `/sys`, `/dev` (außer den eingebauten `--proc`/`--dev`) liegt. Verstoß ist `SANDBOX_006` (neu in Registry: „Mount verboten") mit `why` = Pfad und Grund.

```rust
pub struct SandboxProfile { pub version: u32, pub name: String, pub sandbox: SandboxSection, pub mounts: MountSection, pub network: NetworkSection, pub env: BTreeMap<String, String> }
pub struct SessionContext { pub session: SessionId, pub work_src: PathBuf, pub work_mode: WorkMode, pub proxy_socket_src: PathBuf, pub ca_cert_src: PathBuf, pub shim_src: PathBuf, pub command: Vec<OsString> }
impl SandboxProfile {
    pub fn load(path: &Path) -> Result<Self, Diagnostic>;
    pub fn validate(&self, home: &Path) -> Result<(), Diagnostic>;           // Denylist
    pub fn to_bwrap_args(&self, ctx: &SessionContext) -> Vec<OsString>;      // ohne "bwrap" selbst, endet mit "--" + shim + command
    pub fn argv_line(&self, ctx: &SessionContext) -> String;                 // shell-quoted, für UI
}
```

Reihenfolge der erzeugten Argumente (deterministisch, für Snapshot): `--unshare-*` gemäß Liste, `--die-with-parent`, `--new-session`, `--hostname`, `--ro-bind` je `ro`, `--symlink` je Eintrag, `--proc`, `--dev`, `--tmpfs` je Eintrag (in TOML-Reihenfolge; `/work/...`-tmpfs nach dem Work-Bind), `--bind`/`--ro-bind` für work, `--ro-bind` Proxy-Socket, `--ro-bind` CA, `--ro-bind` Shim, `--ro-bind-data`/`--file` Maskierungen (leerer FD), `--unsetenv`-Liste für alles aus dem Host (`--clearenv`), `--setenv` je Eintrag alphabetisch, `--chdir /work`, `--`, `<shim_dst>`, `--proxy-port 3128`, `--`, `command…`.

### Schritte
1. TOML-Dateien anlegen.
2. `profile.rs`: Typen, `load`, `validate` mit Denylist.
3. `bwrap_args.rs`: `to_bwrap_args`, `argv_line` (Quoting mit `shell-words` oder `shlex`).
4. Snapshot-Test für `default.toml` mit festem `SessionContext` (Pfade `/home/u/proj`, `/run/user/1000/humanitl/proxy/proxy.sock`, …).
5. `bwrap --help` gegen die Argument-Namen prüfen (lokal, manuell), Ergebnis in Fallstricke ergänzen falls Abweichung.

### Tests
- `fn parses_default_profile()`, `fn parses_test_profile()`.
- `fn rejects_docker_socket_mount()`: `extra_ro = ["/var/run/docker.sock"]` → `SANDBOX_006`.
- `fn rejects_runtime_dir_mount()`, `fn rejects_ssh_dir_mount()` (mit `~` expandiert).
- `fn bwrap_args_snapshot_default()`: Vergleich mit `tests/snapshots/default.argv.txt`, ein Argument pro Zeile.
- `fn work_ro_uses_ro_bind()`: `WorkMode::Ro` → `--ro-bind` für work.
- `fn env_is_cleared_then_set()`: `--clearenv` steht vor dem ersten `--setenv`.
- `fn unknown_field_is_diagnostic()`: `deny_unknown_fields`.

### Akzeptanzkriterien
- [ ] Beide Profile parsen, Snapshot-Test grün
- [ ] Denylist-Tests grün
- [ ] `argv_line` ist mit `sh -c` parsebar (Test: `shlex::split(line)` ergibt dieselbe Liste)
- [ ] Kein Mount der Denylist kann per Profil eingeschmuggelt werden, auch nicht über Symlink-Quelle (Quelle wird kanonisiert vor Prüfung)
- [ ] Alle Env-Schlüssel aus dem Env-Kit im Profil vorhanden

### Fallstricke
- `--unshare-all` ist Kurzform; explizite Liste ist für Snapshot und Anzeige besser, aber `--unshare-all` schließt zusätzlich `--unshare-cgroup` ein und ist zukunftssicher. Entscheidung: Liste explizit, plus `cgroup`.
- `--tmpfs /work/.git/hooks` funktioniert nur, wenn das Verzeichnis nach dem `/work`-Bind existiert; existiert es nicht, legt bwrap es an (in tmpfs über dem Bind). Reihenfolge einhalten.
- `--file FD /work/.envrc` braucht einen offenen FD (leere Datei); `LaunchPlan.fds` aus CONVENTIONS.md 3.4 trägt ihn. In Sprint 0 nur als Platzhalter `--ro-bind /dev/null /work/.envrc` verwenden; HUM-011 ersetzt bei Bedarf.
- `--clearenv` existiert ab bwrap 0.5; Debian trixie hat 0.11. `min_bwrap_version` im Profil, Prüfung in HUM-011.
- Host-Home-Pfade in der Denylist müssen mit dem echten `$HOME` expandiert werden, nicht mit `~`-String-Vergleich.

### Referenzen
ADR-002, BACKLOG.md 4.1, CONVENTIONS.md 3.4, bwrap Manpage (manpages.debian.org/trixie/bubblewrap/bwrap.1.en.html).

---

## HUM-005 · Fake-Daemon für UI-Entwicklung
Sprint: 0 · Größe: M · Abhängigkeiten: HUM-003, HUM-004 · Blockiert: HUM-019, HUM-020

### Kontext
UI und Daemon werden parallel entwickelt. Damit das UI ab Sprint 1 gegen realistische Ereignisse gebaut werden kann, gibt es einen Fake, der dieselbe gRPC-Schnittstelle implementiert und eine aufgezeichnete Session mit Timing abspielt. Der Fake ist zugleich das Werkzeug für Golden-Tests und e2e-Tests ohne echte Sandbox.

### Ziel
`humanitld --fake <session.jsonl> [--speed 10]` startet einen gRPC-Server auf dem normalen UDS-Pfad, spielt die Ereignisse der Datei mit relativen Zeitstempeln ab, hält Flows echt (Deadline-Timer läuft, `Decide` beendet den Hold, Timeout blockt), beantwortet `ListFlows`, `GetFlow`, `GetBody`, `Rules` (In-Memory), `Sandbox` (simulierter Status, Checks immer grün), `Terminal` (Echo), `Audit` (leer), `GetConfig` (Defaults). Zwei Sessions liegen bei: `fixtures/sessions/npm-install.jsonl` (15 Flows in 20 s, ein Host) und `fixtures/sessions/mixed.jsonl` (Findings, Passthrough, Timeout, Edited).

### Nicht-Ziel
Kein echter Proxy. Kein Recorder auf Platte. Kein Escape-Test. Der Fake ist Rust, damit er die echte Proto-Implementierung teilt; ein Dart-Fake existiert zusätzlich nur als `FakeDaemonClient` für Widget-Tests (HUM-019).

### Betroffene Pfade
- `daemon/bin/humanitld/src/main.rs` (Flag `--fake`)
- `daemon/crates/ipc/src/fake/mod.rs`, `fake/player.rs`, `fake/state.rs` (neu)
- `daemon/crates/ipc/src/server_stub.rs` (neu, `tonic`-Service-Impl, in HUM-018 durch echte ersetzt; Fake und echte teilen den Trait `DaemonApi`)
- `fixtures/sessions/npm-install.jsonl`, `fixtures/sessions/mixed.jsonl` (neu)
- `fixtures/sessions/README.md` (neu, Format)
- `daemon/crates/ipc/tests/fake_player.rs` (neu)

### Spezifikation

Session-Format (JSONL, eine Zeile pro Ereignis, `t_ms` relativ zum Start):

```json
{"t_ms":0,"type":"session","session_id":"018f0000-0000-7000-8000-000000000001","llm_endpoint":"http://192.168.1.50:11434","work_dir":"/home/u/proj"}
{"t_ms":120,"type":"request","flow_id":"018f0000-0000-7000-8000-000000000010","method":"GET","scheme":"https","host":"registry.npmjs.org","port":443,"path":"/lodash","headers":[["user-agent","npm/10.8.0 node/v22.4.0"],["accept","application/json"]],"body_b64":"","origin_tool":"npm"}
{"t_ms":130,"type":"findings","flow_id":"018f0000-0000-7000-8000-000000000010","findings":[]}
{"t_ms":135,"type":"hold","flow_id":"018f0000-0000-7000-8000-000000000010","timeout_ms":300000}
{"t_ms":9000,"type":"auto","flow_id":"018f0000-0000-7000-8000-000000000011","source":"rule","rule_id":"018f0000-0000-7000-8000-0000000000a1","kind":"allow"}
{"t_ms":9100,"type":"response","flow_id":"018f0000-0000-7000-8000-000000000011","status":200,"headers":[["content-type","application/json"]],"body_b64":"eyJuYW1lIjoibG9kYXNoIn0=","streaming":false}
{"t_ms":15000,"type":"passthrough","flow_id":"018f0000-0000-7000-8000-000000000020","method":"POST","host":"192.168.1.50","port":11434,"path":"/api/chat","body_b64":"...","response_status":200}
{"t_ms":20000,"type":"diagnostic","code":"TLS_001","severity":"warning","why":"curl 8.9 in sandbox rejected CA for host example.org","fix":{"set_env":{"key":"CURL_CA_BUNDLE","value":"/etc/humanitl/ca.crt"}}}
```

Regeln des Players:
- `request` erzeugt `Received`, dann sofort `Analyzed` (mit `findings`-Zeile, falls vorhanden, sonst leer), dann `hold` wenn eine `hold`-Zeile folgt, sonst wartet er auf `auto`.
- Gehaltene Flows werden nicht per Datei entschieden; die Entscheidung kommt vom Client (`Decide`) oder vom Timeout. Nach `Decide(Allow)` spielt der Player die `response`-Zeile des Flows ab, falls vorhanden, sonst synthetisiert er `200` mit leerem Body. Nach `Block` folgt `Recorded`.
- `--speed N` teilt alle `t_ms` durch N; `hold.timeout_ms` wird nicht skaliert (Default aus Config), außer `--scale-timeouts`.
- `--loop` startet die Datei nach Ende neu mit neuen Flow-IDs (Suffix inkrementiert), für Dauerbetrieb bei UI-Arbeit.

`DaemonApi`-Trait in `ipc`:

```rust
#[async_trait]
pub trait DaemonApi: Send + Sync + 'static {
    async fn info(&self) -> Info;
    fn subscribe(&self, req: SubscribeRequest) -> BoxStream<'static, FlowEvent>;
    async fn list_flows(&self, req: ListFlowsRequest) -> Result<FlowPage, Diagnostic>;
    async fn get_flow(&self, id: FlowId) -> Result<FlowDetail, Diagnostic>;
    fn get_body(&self, r: BodyRef) -> BoxStream<'static, BodyChunk>;
    async fn decide(&self, req: DecideRequest) -> Result<DecideResponse, Diagnostic>;
    async fn rules(&self, req: RulesRequest) -> Result<RulesResponse, Diagnostic>;
    fn sandbox(&self, req: SandboxRequest) -> BoxStream<'static, SandboxEvent>;
    fn terminal(&self, input: BoxStream<'static, TerminalInput>) -> BoxStream<'static, TerminalOutput>;
    async fn audit(&self, req: AuditRequest) -> Result<AuditResponse, Diagnostic>;
    async fn get_config(&self, req: GetConfigRequest) -> Result<ConfigSnapshot, Diagnostic>;
    async fn set_config(&self, req: SetConfigRequest) -> Result<ConfigSnapshot, Diagnostic>;
}
```

Die tonic-Service-Impl (`server_stub.rs`) ist generisch über `T: DaemonApi` und mappt `Diagnostic` auf `tonic::Status` mit `Status::with_details` (Diagnostic als Proto-Bytes in den Details). Der Fake ist `FakeDaemon: DaemonApi`. Token-Prüfung (`x-humanitl-token`) ist im Stub, Fake schreibt das Token-File genauso wie der echte Daemon.

Fixtures-Inhalt:
- `npm-install.jsonl`: 15 GET-Requests an `registry.npmjs.org` zwischen `t_ms` 100 und 20000, alle `hold`, keine Findings. Erwartete UI-Reaktion: eine Gruppe, Batch-Allow.
- `mixed.jsonl`: (1) POST `api.github.com/graphql` mit `authorization: Bearer ghp_…` und E-Mail im Body → Findings `api_key.github` (Checksum-Tier), `email` (Regex-Tier), hold; (2) GET `models.dev/api.json` → `auto` block via Regel `bundled`; (3) Passthrough an `192.168.1.50:11434`; (4) GET `example.org/` hold mit `timeout_ms: 5000` → läuft in Timeout, wenn der Nutzer nicht reagiert; (5) POST `httpbin.org/post` hold, gedacht für „Allow edited"-Test; (6) `diagnostic` TLS_001; (7) WebSocket-Upgrade `wss://ws.example.org/` hold mit `upgrade: websocket`.

### Schritte
1. `DaemonApi`-Trait und generischer tonic-Stub inklusive Token-Prüfung und Diagnostic→Status-Mapping.
2. `fake/state.rs`: In-Memory `Flows: BTreeMap<FlowId, Flow>`, `Rules: Vec<Rule>`, `broadcast::Sender<FlowEvent>` (cap aus Config `ipc.event_buffer`).
3. `fake/player.rs`: Datei lesen, sortieren nach `t_ms`, `tokio::time::sleep` bis zum nächsten Ereignis, Zustandsautomat aus `humanitl-core` benutzen (nicht umgehen: jeder Übergang über `Flow::apply`).
4. `main.rs`: `--fake PATH --speed N --loop --scale-timeouts`, Socket-Pfad aus `paths::daemon_socket()`, Token-File schreiben, `SIGTERM` sauber beenden und Socket-Datei löschen.
5. Fixtures schreiben, README mit Format.
6. Manuell: `grpcurl -unix -plaintext $XDG_RUNTIME_DIR/humanitl/daemon.sock humanitl.v1.Humanitl/Subscribe` zeigt Events (Token-Header mitgeben).

### Tests
`tests/fake_player.rs` (tokio, `start_paused` für Zeitkontrolle):
- `fn plays_npm_session_in_order()`: Subscribe, nach virtuellen 20 s sind 15 `Held`-Events angekommen, Reihenfolge der Flow-IDs monoton.
- `fn decide_allow_emits_forward_and_response()`: Ersten Flow allowen, erwartet `Decided`, `Forwarded`, `ResponseHeaders`, `Recorded` in dieser Reihenfolge.
- `fn timeout_blocks()`: `mixed.jsonl`, Flow 4 nach 5 s → `TimedOut`, dann `Recorded`, `ListFlows` zeigt `decision = TIMED_OUT`.
- `fn allow_edited_with_two_ids_is_ipc_002()`.
- `fn invalid_token_is_unauthenticated()`.
- `fn lagged_when_subscriber_slow()`: Kanal-Kapazität auf 4 setzen, 20 Events feuern, Subscriber liest erst danach, erhält `Lagged{n>0}`.

### Akzeptanzkriterien
- [ ] `humanitld --fake fixtures/sessions/mixed.jsonl` startet, `grpcurl` Subscribe liefert Events
- [ ] Alle sechs Tests grün
- [ ] Jeder Zustandswechsel im Fake geht durch `Flow::apply` (grep: kein direktes `state =` außerhalb von `flow.rs`)
- [ ] Fake beendet sich auf SIGTERM und räumt Socket und Token-File weg
- [ ] Fixtures haben gültige UUIDv7-Strings (Test: alle IDs parsen mit `FlowId::parse`)

### Fallstricke
- Zeitsteuerung in Tests ohne `tokio::time::pause()` macht Tests langsam und flaky. `#[tokio::test(start_paused = true)]` plus `tokio::time::advance`.
- Der Player darf Deadline-Timer nicht mit `t_ms` verwechseln: Deadline ist Wanduhr ab `Held`, unabhängig vom Abspieltempo (außer `--scale-timeouts`).
- `broadcast` liefert `RecvError::Lagged(n)`; das muss in ein `FlowEvent::Lagged` übersetzt werden, nicht in einen Stream-Abbruch.
- Body-Bytes in Fixtures sind Base64; `GetBody` liefert sie in Chunks von 64 KiB, `last = true` beim letzten. Leerer Body: genau ein Chunk mit `last = true`.
- Token-File und Socket unter `$XDG_RUNTIME_DIR`; in CI ohne Runtime-Dir greift der Fallback aus HUM-062.

### Referenzen
ADR-003, ADR-004, CONVENTIONS.md 3.6, 3.5 (Defaults), tonic Status Details (docs.rs/tonic/latest/tonic/struct.Status.html).

---

## HUM-006 · Escape-Test-Harness
Sprint: 0 · Größe: M · Abhängigkeiten: HUM-010, HUM-002 · Blockiert: HUM-012, HUM-013, HUM-021

### Kontext
BACKLOG.md 4.5 und Risiko 1: Die Escape-Tests werden vor dem Proxy geschrieben, damit die Sicherheitsaussage von Anfang an messbar ist. In Sprint 0 sind sie erwartet rot (kein Shim, kein Proxy-Socket). Das Harness liefert JUnit-XML, damit CI sie als Testfälle sieht.

### Ziel
`tests/escape/run.sh` startet bwrap mit `profiles/sandbox/test.toml` (über die Argument-Erzeugung aus HUM-010, aufgerufen durch ein kleines Rust-Test-Binary, weil die CLI erst in HUM-064 kommt), führt `esc-1.sh` bis `esc-3.sh` darin aus, sammelt Exit-Codes und Ausgaben in `target/escape/escape.xml`. ESC-4 (Regel-Tabelle) ist ein reiner Rust-Test und kommt mit HUM-022; ESC-5 kommt mit HUM-043/050. Hier werden Platzhalter-Testfälle mit `skipped` eingetragen.

### Nicht-Ziel
Kein Shim, kein seccomp (HUM-012). Kein Proxy (HUM-013/015). Die Tests sind grün, sobald diese Issues fertig sind; hier geht es um das Gerüst und die exakten Proben.

### Betroffene Pfade
- `tests/escape/run.sh` (neu)
- `tests/escape/lib.sh` (neu, Hilfsfunktionen `probe`, `expect_fail`, `expect_output`)
- `tests/escape/esc-1-sockets.sh`, `esc-2-mounts.sh`, `esc-3-egress.sh` (neu)
- `tests/escape/junit.sh` (neu, XML-Schreiber)
- `daemon/crates/sandbox/src/bin/escape-launch.rs` (neu, `[[bin]]` in sandbox-Crate: liest Profil, baut Argv, `exec bwrap`)
- `.github/workflows/ci.yml` (Job `escape-tests` auf `./tests/escape/run.sh` umstellen)

### Spezifikation

`lib.sh`:

```sh
# probe NAME CMD...      führt CMD aus; Erfolg der Probe = CMD scheitert (exit != 0)
# expect_ok NAME CMD...  Erfolg = CMD gelingt (exit == 0); für erlaubte Operationen wie socket(AF_INET)
# expect_output NAME PATTERN CMD...   Erfolg = Ausgabe matcht PATTERN (grep -E)
# Jede Probe schreibt eine Zeile "RESULT <name> <pass|fail> <detail>" nach $ESC_RESULTS
```

`esc-1-sockets.sh` (läuft in der Sandbox):

```sh
#!/bin/sh
. /tests/escape/lib.sh
expect_output ifaces_only_lo '^1: lo' sh -c 'ip -o link 2>/dev/null || cat /proc/net/dev'
expect_output no_ipv4_routes '^$' sh -c 'cat /proc/net/route | tail -n +2'
# AF_INET/AF_INET6 sind ERLAUBT (Loopback zum Proxy); die Garantie ist: kein Weg nach draußen.
expect_ok socket_af_inet   python3 -c 'import socket; socket.socket(socket.AF_INET, socket.SOCK_STREAM)'
expect_ok socket_af_inet6  python3 -c 'import socket; socket.socket(socket.AF_INET6, socket.SOCK_STREAM)'
probe connect_af_inet_lan  python3 -c 'import socket; s=socket.socket(); s.settimeout(2); s.connect(("10.0.0.1",80))'
probe socket_af_unix   python3 -c 'import socket; socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)'
probe socket_af_netlink python3 -c 'import socket; socket.socket(socket.AF_NETLINK, socket.SOCK_RAW)'
probe socket_af_packet python3 -c 'import socket; socket.socket(socket.AF_PACKET, socket.SOCK_RAW)'
probe socketpair       python3 -c 'import socket; socket.socketpair()'
probe dev_tcp          sh -c 'exec 3<>/dev/tcp/1.1.1.1/80'
expect_output seccomp_mode_2 '^Seccomp:[[:space:]]+2' cat /proc/self/status
expect_output seccomp_parent_mode_2 '^Seccomp:[[:space:]]+2' cat /proc/1/status
```

`esc-2-mounts.sh`:

```sh
#!/bin/sh
. /tests/escape/lib.sh
probe no_x11        test -e /tmp/.X11-unix
probe no_wayland    sh -c 'ls /run/user/*/wayland-* 2>/dev/null | grep -q .'
probe no_dbus       sh -c 'ls /run/user/*/bus /run/dbus/system_bus_socket 2>/dev/null | grep -q .'
probe no_docker     sh -c 'test -e /var/run/docker.sock -o -e /run/docker.sock'
probe no_host_home  sh -c 'ls /home | grep -vx agent | grep -q .'
probe no_runtime_dir test -d /run/user
expect_output exactly_one_socket '^1$' sh -c 'find / -xdev -type s 2>/dev/null | wc -l'
expect_output socket_is_proxy '^/run/humanitl/proxy.sock$' sh -c 'find / -xdev -type s 2>/dev/null'
probe host_pid1_environ cat /proc/1/environ   # in eigenem PID-NS ist PID 1 der Shim; Test prüft, dass dort kein HOST-Marker steht
expect_output hostname_sandbox '^sandbox$' hostname
probe machine_id     sh -c 'test -s /etc/machine-id'
expect_output shm_is_tmpfs 'tmpfs' sh -c 'grep " /dev/shm " /proc/self/mountinfo'
```

Der Runner setzt vor dem Start `HUMANITL_ESCAPE_MARKER=host-$RANDOM` in seiner eigenen Umgebung; `host_pid1_environ` ist zusätzlich `expect_output no_marker_leak` mit negiertem Grep über `/proc/*/environ`.

`esc-3-egress.sh` (braucht Proxy ab HUM-013; bis dahin scheitern die Proben an „no route", was für die ersten drei Zeilen bereits das gewünschte Ergebnis ist):

```sh
#!/bin/sh
. /tests/escape/lib.sh
probe direct_http    curl -s --max-time 3 --noproxy '*' http://example.com/
probe direct_https   curl -s --max-time 3 --noproxy '*' https://example.com/
probe direct_ip      curl -s --max-time 3 --noproxy '*' http://93.184.216.34/
probe dns_lookup     sh -c 'getent hosts example.com'
probe quic_udp       python3 -c 'import socket; s=socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.sendto(b"x",("1.1.1.1",443))'
# Ab HUM-013/015: über Proxy erreichbar, muss in der Hold-Queue landen (Runner prüft per gRPC ListFlows)
expect_output via_proxy_held 'Blocked by Humanitl' curl -s --max-time 10 http://blocked.example/
expect_output via_proxy_private_held 'Blocked by Humanitl' curl -s --max-time 10 http://10.0.0.1/
expect_output via_proxy_metadata_held 'Blocked by Humanitl' curl -s --max-time 10 http://169.254.169.254/
expect_output via_proxy_idn_held 'Blocked by Humanitl' curl -s --max-time 10 http://xn--80ak6aa92e.com/
expect_output host_mismatch_blocked 'authority_mismatch' curl -sk --max-time 10 -H 'Host: evil.io' https://github.com/
```

Der Runner setzt für ESC-3 die Hold-Timeout auf 2 s (`HUMANITL_HOLD__TIMEOUT_SECS=2`), sodass „held" zu „timed out, blocked" wird und `curl` den 403-Body sieht. Zusätzlich prüft der Runner host-seitig, dass vor dem Timeout kein DNS-Lookup für `blocked.example` stattfand: `resolvectl statistics` Differenz oder `tcpdump -i any port 53` im Hintergrund (in CI: `sudo tcpdump`; lokal optional). Bis ADR-006 umgesetzt ist (HUM-024), ist dieser Teil `skipped`.

`junit.sh`: liest alle `RESULT`-Zeilen und schreibt `<testsuite name="escape" tests=N failures=F skipped=S>` mit `<testcase classname="esc-N" name="…">` und `<failure message="…"/>` bzw. `<skipped/>`.

`run.sh`:

```sh
#!/usr/bin/env sh
set -eu
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/target/escape"; mkdir -p "$OUT"
export ESC_RESULTS="$OUT/results.txt"; : > "$ESC_RESULTS"
# AppArmor-Workaround für Ubuntu 24.04 Runner
if [ -f /proc/sys/kernel/apparmor_restrict_unprivileged_userns ] && [ "$(cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns)" = 1 ]; then
  sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0 || true
fi
export HUMANITL_ESCAPE_MARKER="host-$$"
LAUNCH="$ROOT/daemon/target/debug/escape-launch"
( cd "$ROOT/daemon" && cargo build -p humanitl-sandbox --bin escape-launch )
for n in 1 2 3; do
  "$LAUNCH" --profile "$ROOT/profiles/sandbox/test.toml" --tests-dir "$ROOT/tests/escape" -- /bin/sh "/tests/escape/esc-$n-"*.sh || true
done
sh "$ROOT/tests/escape/junit.sh" "$ESC_RESULTS" > "$OUT/escape.xml"
# Exit-Code: 0 nur, wenn keine failure; in Sprint 0 wird das Ergebnis per ESCAPE_ALLOW_FAIL=1 toleriert
if grep -q ' fail ' "$ESC_RESULTS" && [ "${ESCAPE_ALLOW_FAIL:-0}" != 1 ]; then exit 1; fi
```

`escape-launch.rs`: `--profile`, `--tests-dir` (wird als `extra_ro` nach `/tests/escape` gebunden), `--proxy-socket` (optional; fehlt er, wird ein leeres Dummy-Verzeichnis gebunden, damit ESC-2 „exactly_one_socket" korrekt rot ist), `--` Kommando. Baut `SessionContext`, ruft `to_bwrap_args`, `exec("bwrap", args)`. Ohne Shim (bis HUM-012) wird das Kommando direkt nach `--` gesetzt.

CI: `ESCAPE_ALLOW_FAIL=1` in Sprint 0 im Job setzen, mit Kommentar `# remove in HUM-021`. `ESC_RESULTS` wird als Artefakt hochgeladen.

### Schritte
1. `lib.sh`, `junit.sh` schreiben, lokal mit Dummy-Ergebnissen testen (`sh junit.sh fixture.txt`).
2. `escape-launch.rs` in der sandbox-Crate.
3. Drei Skripte schreiben. Lokal ausführen; Erwartung: ESC-1 Interface-Probe grün, Socket-Proben rot (kein seccomp), ESC-2 teilweise grün, ESC-3 Direktproben grün, Proxy-Proben rot.
4. `run.sh`, CI-Job umstellen, `ESCAPE_ALLOW_FAIL=1` setzen.
5. `docs/SECURITY.md` (HUM-007) verlinkt auf die Test-IDs.

### Tests
- Das Harness ist selbst Test-Infrastruktur. Selbsttest: `tests/escape/selftest.sh` führt `lib.sh`-Funktionen mit `true`/`false` aus und prüft, dass `probe x false` pass und `probe x true` fail erzeugt, sowie dass `junit.sh` wohlgeformtes XML liefert (`xmllint --noout`).
- Rust: `escape-launch` hat einen Test, der ohne `exec` nur die Argv baut und den Snapshot aus HUM-010 mit `--tests-dir` erweitert prüft.

### Akzeptanzkriterien
- [ ] `./tests/escape/run.sh` läuft lokal durch und erzeugt `target/escape/escape.xml` mit ≥ 25 Testcases
- [ ] `xmllint --noout target/escape/escape.xml` exit 0
- [ ] CI-Job `escape-tests` lädt das XML als Artefakt hoch und ist mit `ESCAPE_ALLOW_FAIL=1` grün
- [ ] Erwartetes Ergebnis in Sprint 0 dokumentiert in `tests/escape/README.md`: welche Proben rot sind und welches Issue sie grün macht
- [ ] Keine Probe nutzt Netzwerk auf dem Host außer der optionalen DNS-Beobachtung

### Fallstricke
- `ip` ist in einer minimalen Sandbox nicht vorhanden; Fallback `/proc/net/dev` einbauen (in ESC-1 vorgesehen).
- `python3` muss in der Sandbox verfügbar sein: `/usr` ist ro gebunden, also ja, wenn auf dem Host installiert. CI installiert `python3`.
- `curl` folgt `HTTP_PROXY` aus dem Env-Kit; für Direktproben `--noproxy '*'` setzen, sonst testet man den Proxy statt die Abwesenheit der Route.
- In der Sandbox ist PID 1 der Shim (später) oder `/bin/sh`; `/proc/1/status` zeigt dessen Seccomp. Beide müssen Mode 2 zeigen, sobald HUM-012 fertig ist.
- `find / -type s` traversiert `/proc`; `-xdev` verhindert das nicht vollständig, `-path /proc -prune` ergänzen.
- `sudo` ist auf GitHub-Runnern passwortlos, lokal nicht; `|| true` beim sysctl.

### Referenzen
BACKLOG.md 4.5, Risiko 1 (Abschnitt 10), CONVENTIONS.md 3.11.

---

## HUM-007 · SECURITY.md und THREAT-MODEL.md Entwurf
Sprint: 0 · Größe: S · Abhängigkeiten: keine · Blockiert: HUM-041 (Panel-Texte), HUM-059

### Kontext
Prinzip 2: Das Sicherheitsargument muss in drei Sätzen erklärbar und per Klick prüfbar sein. Das Dokument ist die Quelle für das Isolation-Panel, das README und externe Reviews. Es steht vor dem Code, damit der Code sich daran messen lässt.

### Ziel
`docs/SECURITY.md` und `docs/THREAT-MODEL.md` existieren als vollständige Entwürfe mit der Gliederung unten, jeder Abschnitt ausformuliert (kein „TODO"), Escape-Test-IDs referenziert, Seitenkanäle ehrlich benannt.

### Nicht-Ziel
Kein Responsible-Disclosure-Prozess (kommt mit HUM-059). Keine Bewertung von Docker/microVM (Post-MVP).

### Betroffene Pfade
- `docs/SECURITY.md` (neu)
- `docs/THREAT-MODEL.md` (neu)
- `README.md` (Abschnitt „Sicherheit in drei Sätzen" mit Link)

### Spezifikation

`docs/SECURITY.md` Gliederung:

1. **Die Garantie in drei Sätzen** (Wortlaut aus BACKLOG.md 0 und 4.1, identisch in DE und EN, weil das Panel beide zeigt).
2. **Was das konkret heißt**: pro Satz der Mechanismus (Namespace, Datei-Bind, seccomp), der Prüfbefehl, die Escape-Test-ID (ESC-1, ESC-2, ESC-3), und was ein Angreifer versuchen würde.
3. **Was nicht abgedeckt ist** (Tabelle aus BACKLOG.md 4.2, ausformuliert): LLM-Passthrough, `/work`, Terminal-Ausgabe, Hostnamen im Log, Caches. Für jeden: warum er existiert, was Humanitl tut, was der Nutzer tun sollte.
4. **Vertrauensbasis (TCB)**: Kernel, bwrap, seccomp-BPF, `humanitld` (Rust), `humanitl-shim`, hyper/rustls, der LLM-Host. Mit Versionsanforderungen.
5. **Der Proxy**: CA pro Installation, wo der Key liegt, dass er nie in den Host-Trust-Store kommt, was bei Certificate Pinning passiert (Tool scheitert sichtbar, kein Fallback), Body-Caps, Dekompressionslimit.
6. **Regeln und ihre Fallen**: Label-Globs, IDN, IP-Literale, private Bereiche, Authority-Konsistenz, WebSocket, Redirects (neuer Request wird erneut gehalten).
7. **DNS**: warum erst nach Freigabe aufgelöst wird, mit dem 63-Byte-Label-Beispiel.
8. **Aufzeichnung und Audit**: was die Hash-Kette beweist und was nicht (BACKLOG.md ADR-008, Security-Review §5).
9. **So prüfst du es selbst**: die exakte `bwrap`-Zeile anzeigen (UI oder `humanitl sandbox argv`), `humanitl sandbox check`, `./tests/escape/run.sh`.
10. **Bekannte Grenzen und offene Punkte**: Head-Anchoring des Audit-Logs, HTTP/2 zum Upstream, WebSocket-Frame-Hold, Tail-Truncation.

`docs/THREAT-MODEL.md` Gliederung:

1. **Schutzziel**: Vertraulichkeit von Projektdaten und Secrets; Integrität der Entscheidungshistorie. Nicht: Verfügbarkeit des Agenten.
2. **Angreifer**: (a) Prompt Injection über geladene Inhalte, (b) bösartige Abhängigkeit, (c) kompromittiertes Modell, (d) lokaler Nutzer mit Lesezugriff auf Logs (für Audit relevant). Nicht im Modell: Root auf dem Host, Kernel-Exploits, physischer Zugriff.
3. **Angriffsflächen** als Tabelle: Kanal, Angreifer a/b/c/d, gestoppt durch, Restrisiko, Escape-Test.
4. **Ausführliche Kanal-Liste** (aus Security-Review §1, alle zwölf): Projektverzeichnis, LLM-Passthrough, Socket-Verzeichnis, seccomp-Lücke, Unix-Sockets vom Host, PID/IPC-Namespace, /proc und /sys, Symlinks, Terminal-Escapes, DNS-Timing, Queue-Metadaten, Caches. Je: Schwere, Mitigation, Status (MVP / später).
5. **Annahmen**: bwrap ≥ 0.8 mit User-Namespaces, Kernel ≥ 5.x mit seccomp, der Nutzer liest, was er freigibt, der LLM-Host ist unter Kontrolle des Nutzers.
6. **Was passiert, wenn eine Annahme bricht**: pro Annahme ein Absatz.
7. **Änderungshistorie** des Modells.

### Schritte
1. THREAT-MODEL.md aus den Review-Ergebnissen ausformulieren (Tabellen, keine Stichpunkt-Fragmente).
2. SECURITY.md schreiben, Abschnitt 1 und 3 zuerst, weil UI-Texte daraus entstehen.
3. README-Abschnitt ergänzen.
4. Gegenlesen mit der Frage „Würde ein Nicht-Experte nach Abschnitt 1 bis 3 verstehen, was sicher ist und was nicht?" Falls nein, umschreiben.

### Tests
- `scripts/ci/lint-docs.sh` (neu, in `rust-check` einhängen): beide Dateien existieren, enthalten keine Zeile mit `TODO` oder `TBD`, alle referenzierten `ESC-N` existieren als Skript unter `tests/escape/`.

### Akzeptanzkriterien
- [ ] Beide Dateien vollständig nach Gliederung, keine leeren Abschnitte
- [ ] Die drei Garantie-Sätze stehen wortgleich in SECURITY.md Abschnitt 1, BACKLOG.md 4.1 und später im ARB (`isolation_guarantee_1..3`)
- [ ] Jede der zwölf Kanäle aus dem Review ist in THREAT-MODEL.md Abschnitt 4 mit Status
- [ ] `lint-docs.sh` grün

### Fallstricke
- Nicht „sicher" schreiben, wo „gemindert" gemeint ist. Der Abschnitt 3 ist der wichtigste; wer ihn weglässt, verspielt Vertrauen.
- Die Sätze müssen für Nicht-Experten funktionieren; Fachbegriffe in Klammern erklären (Namespace, seccomp).
- Escape-Test-IDs müssen mit den Dateinamen aus HUM-006 übereinstimmen.

### Referenzen
BACKLOG.md 4, Security-Review (interne Gremiums-Runde vom 2026-09-02), Claude Code Sandboxing (anthropic.com/engineering/claude-code-sandboxing), sandbox-runtime README.

---

## HUM-008 · Design-Tokens und `packages/ui`
Sprint: 0 · Größe: M · Abhängigkeiten: HUM-001 · Blockiert: HUM-019, HUM-020, HUM-054

### Kontext
BACKLOG.md 5 (Airlock) und ADR-009: shadcn_flutter hinter einem eigenen Wrapper-Package, exakt gepinnt, Tokens als Konstanten. Ohne Tokens vor dem ersten Screen wird jedes Widget seine eigenen Farben erfinden.

### Ziel
`app/packages/ui` exportiert `HTokens` (Farben, Typografie, Spacing, Radius, Motion), ein `HTheme` für Dark und Light auf Basis von shadcn_flutter, die Wrapper `HButton`, `HPill`, `HBadge`, `HPanel`, `HRow`, `HMethodBadge`, `HStateGlyph`, `HHairline`, sowie eine Galerie-Seite (`humanitl --gallery`), die jedes Element in jedem Zustand zeigt. Inter und JetBrains Mono sind gebündelt.

### Nicht-Ziel
Keine Screens. Kein Riverpod. Keine Icons außer Lucide (kommt mit shadcn_flutter). Keine Golden-Tests (HUM-054), aber die Galerie ist deren spätere Grundlage.

### Betroffene Pfade
- `app/packages/ui/pubspec.yaml` (`shadcn_flutter: 0.0.54` exakt, `flutter`)
- `app/packages/ui/lib/humanitl_ui.dart` (Barrel)
- `app/packages/ui/lib/src/tokens/colors.dart`, `typography.dart`, `spacing.dart`, `motion.dart` (neu)
- `app/packages/ui/lib/src/theme/h_theme.dart` (neu)
- `app/packages/ui/lib/src/widgets/{h_button,h_pill,h_badge,h_panel,h_row,h_method_badge,h_state_glyph,h_hairline}.dart` (neu)
- `app/packages/ui/lib/src/gallery/gallery_page.dart` (neu)
- `app/packages/ui/fonts/Inter-Variable.ttf`, `JetBrainsMono-Variable.ttf` (neu, OFL, Lizenzdateien daneben)
- `app/lib/main.dart` (Flag `--gallery` über `Platform.environment['HUMANITL_GALLERY']` oder `args`)
- `app/packages/ui/test/tokens_test.dart` (neu)

### Spezifikation

`colors.dart`:

```dart
abstract final class HColors {
  // Dark (Quelle: BACKLOG.md 5)
  static const bg0 = Color(0xFF0F1115);
  static const bg1 = Color(0xFF151821);
  static const bg2 = Color(0xFF1B1F2A);
  static const bg3 = Color(0xFF232838);
  static const line = Color(0xFF2A3040);
  static const lineStrong = Color(0xFF384056);
  static const fg0 = Color(0xFFE6E8EE);
  static const fg1 = Color(0xFFA3A9B8);
  static const fg2 = Color(0xFF6B7186);
  static const accent = Color(0xFF7C9CF5);
  // Light
  static const lBg0 = Color(0xFFFAFBFD);
  static const lBg1 = Color(0xFFFFFFFF);
  static const lBg2 = Color(0xFFF3F5F9);
  static const lBg3 = Color(0xFFE9ECF3);
  static const lLine = Color(0xFFE1E5EE);
  static const lLineStrong = Color(0xFFC9CFDC);
  static const lFg0 = Color(0xFF16181F);
  static const lFg1 = Color(0xFF4B5162);
  static const lFg2 = Color(0xFF7C8294);
  static const lAccent = Color(0xFF5B7FE6);
  // Zustände (dark); light = HSL-Lightness minus 12 %
  static const held = Color(0xFFE0B24A);
  static const allowed = Color(0xFF4FBF8C);
  static const blocked = Color(0xFFE5646E);
  static const timedOut = Color(0xFF8A90A2);
  static const passthrough = Color(0xFFB48AF0);
  static const secret = Color(0xFFF0784F);   // error / secret found
  static const tintAlpha = 0.10;             // max Flächen-Alpha für Zustandsfarben
  // Method-Hues (neutral, nie Zustand)
  static const methodGet = accent;
  static const methodPost = passthrough;
  static const methodPutPatch = held;
  static const methodDelete = Color(0xB3E5646E); // 70 %
}

enum HFlowState { held, allowed, allowedEdited, blocked, timedOut, autoRule, passthroughLlm, error }

extension HFlowStateColor on HFlowState {
  Color color(Brightness b);          // autoRule = allowed mit 0.6 Opazität
  IconData get icon;                  // Lucide: hourglass, arrowUpRight, arrowUpRight+pencil, shieldX/xCircle, clockX, bolt/shieldCheck, cpu/chevronsRight, triangleAlert
  String get l10nKey;                 // "state_held" …
}
```

`typography.dart`: `HType.ui11/ui12/ui13/ui14/ui16/ui20` (Inter, Zeilenhöhen 16/16/20/22/24/28, Gewichte 400/500/600 als Varianten `.medium`, `.semibold`), `HType.mono11/mono12/mono13/mono14` (JetBrains Mono, `fontFeatures: [FontFeature.disable('liga'), FontFeature.tabularFigures()]`). Inter mit `FontFeature('cv11')`, `tnum`.

`spacing.dart`: `HSpace.unit = 4`, `x1..x8`, `HRadius.control = 4`, `card = 6`, `panel = 0`, `HSize.headerBar = 40`, `statusBar = 24`, `row = 36`, `rowSelected = 56`, `hitMin = 28`, `paneMinQueue = 280`, `paneMinInspector = 480`, `paneMinContext = 260`, `paneRatio = (28, 44, 28)`.

`motion.dart`: `HMotion.enter = Cubic(0.2, 0, 0, 1)`, `exit = Cubic(0.4, 0, 1, 1)`, `arrive = 180ms`, `press = 120ms`, `sweep = 200ms`, `leave = 220ms`, `ruleDraw = 240ms`, `breathe = 1200ms`, `holdToConfirm = 400ms`.

`h_theme.dart`: `HTheme.dark()` / `HTheme.light()` liefern `ThemeData` von shadcn_flutter (`ColorScheme` mit den Tokens, `radius` 4, `Typography` mit Inter/Mono). `HThemeMode { dark, light, system }`.

Widgets (alle mit `key`, `semanticsLabel`, min. Hit-Target 28):
- `HButton(variant: primary|secondary|ghost|danger, size: sm|md, leading?: IconData, onPressed, child)`. `danger` nur für destruktive Aktionen, Farbe `blocked`.
- `HPill(left: Widget, right: Widget, onLeft, onRight, onLeftLongPress)`: geteilter Pill für Release-Valve (Hairline in der Mitte).
- `HBadge(text, color, mono: bool)`: 11/500, Radius 2, Tint 10 %.
- `HMethodBadge(method: String)`: Farbe nach Method-Tabelle, uppercase mono.
- `HStateGlyph(state: HFlowState, size: 16, progress?: double)`: Icon + optionaler Countdown-Ring (`CustomPainter`, Strichstärke 1.5), Breathing unter 20 %.
- `HPanel(title?, actions?, child)`: bg1, Padding 12, Hairline-Rand.
- `HRow(state, leading, title, subtitle, trailing, selected, onTap, onHover)`: 36/56 px, 4 px Zustands-Rail links, bei `selected` 2 px Akzent-Rail.
- `HHairline(vertical: bool)`.

Galerie: eine Seite mit Sektionen Farben (alle Swatches mit Hex-Label), Typo-Skala, Buttons in allen Varianten und Zuständen (enabled, hover, pressed, disabled), Pill, Badges, Method-Badges, StateGlyphs (alle acht, mit Ring bei 100/50/15 %), Row in drei Zuständen, Panel. Theme-Toggle oben rechts.

### Schritte
1. Fonts herunterladen (Inter 4.x, JetBrains Mono 2.x, OFL-Lizenzdateien), in `pubspec.yaml` unter `flutter: fonts:` registrieren.
2. Tokens-Dateien.
3. `HTheme` auf shadcn_flutter 0.0.54 API. Falls die API abweicht (pre-1.0), Anpassung nur in `h_theme.dart`, nie in Consumers.
4. Widgets in der Reihenfolge Hairline, Badge, MethodBadge, StateGlyph, Button, Pill, Row, Panel.
5. Galerie und `--gallery`-Einstieg. Manuell auf Wayland und X11 anschauen, Screenshot in PR.
6. `tokens_test.dart`.

### Tests
`tokens_test.dart`:
- `fn every_state_has_distinct_color_dark()` und `_light()`: acht Zustände, acht verschiedene Farben (autoRule zählt mit Opazität als verschieden).
- `fn state_colors_contrast_on_bg1()`: Kontrast (WCAG-Formel) jeder Zustandsfarbe gegen `bg1` ≥ 3.0, gegen `lBg1` ≥ 3.0 für die Light-Ableitung.
- `fn hit_targets_min_28()`: `HButton.sm` und `HBadge`-Tap-Fläche ≥ 28 logische Pixel (Widget-Test mit `tester.getSize`).
- `fn method_badge_colors_are_not_state_colors()`: Method-Farben ≠ `blocked` (außer Delete 70 %), ≠ `secret`.
- `fn mono_disables_ligatures()`.
- Widget-Test: Galerie baut ohne Exception in Dark und Light (`pumpWidget`, `expect(tester.takeException(), isNull)`).

### Akzeptanzkriterien
- [ ] `flutter analyze` in `app/packages/ui` sauber
- [ ] Galerie startet über `HUMANITL_GALLERY=1 flutter run -d linux`
- [ ] Alle Hex-Werte aus BACKLOG.md 5 exakt in `colors.dart`
- [ ] shadcn_flutter exakt gepinnt (kein `^`)
- [ ] Fonts gebündelt, Lizenzdateien im Repo
- [ ] Tests grün, Kontrast-Test bestanden

### Fallstricke
- shadcn_flutter 0.0.54 verlangt Flutter ≥ 3.47 und hat Material entfernt; kein `MaterialApp` verwenden, sondern `ShadcnApp`. Wer `material.dart` importiert, bekommt Konflikte bei `Colors`, `TextStyle`-Erweiterungen; in `packages/ui` nur `package:flutter/widgets.dart` plus shadcn.
- Variable Fonts: `fontWeight` funktioniert nur, wenn die Variable-Achse registriert ist; sonst drei statische Schnitte (400/500/600) bündeln.
- Light-Zustandsfarben nicht per Hand raten, sondern per HSL-Funktion ableiten und im Test die Kontrastgrenze prüfen.
- `Cubic(0.2, 0, 0, 1)` ist nicht `Curves.easeOut`; nicht ersetzen.
- Countdown-Ring: `CustomPainter` mit `shouldRepaint` nur bei Änderung von `progress`, sonst repaint bei jedem Frame.

### Referenzen
BACKLOG.md 5, ADR-009, shadcn_flutter (pub.dev/packages/shadcn_flutter), Inter (rsms.me/inter), JetBrains Mono (jetbrains.com/lp/mono), Lucide (lucide.dev).

---

## HUM-009 · ADR-Verzeichnis
Sprint: 0 · Größe: S · Abhängigkeiten: keine · Blockiert: keine

### Kontext
BACKLOG.md 2 sagt: jede Entscheidung bekommt eine eigene Datei. ADRs sind der Ort, an dem spätere Änderungen begründet werden, ohne den Backlog umzuschreiben.

### Ziel
`docs/adr/` enthält 0001 bis 0013 aus BACKLOG.md 2 als eigene Dateien im MADR-Format plus ein Template und einen Index.

### Nicht-Ziel
Keine neuen Entscheidungen.

### Betroffene Pfade
- `docs/adr/README.md` (neu, Index)
- `docs/adr/0000-template.md` (neu)
- `docs/adr/0001-rust-hudsucker.md` … `docs/adr/0013-cli-headless.md` (neu)

### Spezifikation

Template (MADR-light):

```markdown
# ADR-NNNN · Titel
Status: accepted | superseded by ADR-XXXX | deprecated
Datum: 2026-09-02

## Kontext
## Entscheidung
## Begründung
## Verworfene Alternativen
## Konsequenzen
## Betroffene Issues
```

Dateinamen: `0001-rust-hudsucker`, `0002-bwrap-first`, `0003-grpc-uds`, `0004-flow-state-machine`, `0005-buffer-request-body`, `0006-dns-after-allow`, `0007-rule-model`, `0008-storage`, `0009-ui-stack`, `0010-packaging`, `0011-single-config-source`, `0012-diagnostics-as-type`, `0013-cli-headless`. Inhalt jeweils aus BACKLOG.md 2, ausformuliert, „Betroffene Issues" mit HUM-IDs.

### Schritte
1. Template und Index.
2. 13 Dateien, Inhalt aus BACKLOG.md übertragen und um „Konsequenzen" ergänzen.
3. `scripts/ci/lint-docs.sh` (HUM-007) prüft zusätzlich: jede ADR-Datei hat alle sieben Überschriften.

### Tests
- `lint-docs.sh` grün.

### Akzeptanzkriterien
- [ ] 13 ADRs plus Template plus Index
- [ ] Jede ADR nennt mindestens ein Issue
- [ ] BACKLOG.md 2 verlinkt auf `docs/adr/`

### Fallstricke
- ADR-Nummern nie umbenennen, auch wenn Reihenfolge im Backlog anders wirkt (010 kommt im Backlog nach 013).

### Referenzen
BACKLOG.md 2, MADR (adr.github.io/madr).


## HUM-074 · Abhängigkeits-Lint
Sprint: 0 · Größe: S · Abhängigkeiten: HUM-001 · Blockiert: keine

### Kontext
ADR-015: Ports-and-Adapters funktioniert nur, wenn die Abhängigkeitsrichtung mechanisch erzwungen wird. Ein Skript prüft den Cargo-Graphen gegen die Tabelle in `CONVENTIONS.md` 3.1 und den Egress-Grundsatz (kein `TcpStream::connect` außerhalb von `Egress`-Implementierungen).

### Ziel
`tools/check-deps.sh` schlägt fehl, sobald eine Workspace-Crate eine nicht erlaubte interne Abhängigkeit hat, eine Bibliotheks-Crate `#![deny(missing_docs)]` nicht setzt, oder `TcpStream::connect` außerhalb von `daemon/crates/proxy/src/egress/` vorkommt. CI-Job `deps-lint` ruft es auf.

### Nicht-Ziel
Keine Lizenzprüfung (macht `cargo deny` in HUM-002). Keine Dart-Seite (Feature-Import-Regel wird in HUM-019 als `dart analyze`-Lint über `import_lint` oder ein kleines Skript ergänzt).

### Betroffene Pfade
- `tools/check-deps.sh` (neu)
- `tools/deps-allow.toml` (neu): Tabelle aus CONVENTIONS 3.1 als `[allow] "humanitl-proxy" = ["humanitl-core", "humanitl-rules", "humanitl-findings", "humanitl-recorder"]`
- `.github/workflows/ci.yml` (Job `deps-lint`)

### Spezifikation
```sh
#!/usr/bin/env sh
set -eu
cd "$(dirname "$0")/.."
cargo metadata --format-version 1 --no-deps > target/meta.json
python3 tools/check_deps.py target/meta.json tools/deps-allow.toml   # 0 = ok, 1 = Verstoß mit Liste
# missing_docs
for f in daemon/crates/*/src/lib.rs; do grep -q '#!\[deny(missing_docs)\]' "$f" || { echo "missing deny(missing_docs): $f"; exit 1; }; done
# Egress-Grundsatz
if grep -rn 'TcpStream::connect' daemon/crates daemon/bin --include='*.rs' | grep -v 'crates/proxy/src/egress/' | grep -v '/tests/'; then echo 'TcpStream::connect outside egress'; exit 1; fi
```
`check_deps.py`: liest `packages[*].name` und `dependencies[*].name` für Workspace-Mitglieder (Name beginnt mit `humanitl`), vergleicht mit `[allow]`. `humanitld`, `humanitl` (CLI) und `humanitl-xtask` sind von der Prüfung ausgenommen (`[exempt]`-Liste).

### Schritte
1. `deps-allow.toml` aus CONVENTIONS 3.1 abschreiben.
2. `check_deps.py` (30 Zeilen, nur stdlib + `tomllib`).
3. Shell-Skript, lokal grün auf dem leeren Workspace aus HUM-001.
4. CI-Job `deps-lint` (ubuntu-latest, `cargo metadata` braucht nur die Toolchain, kein Build).
5. Negativtest: temporär `humanitl-core` von `humanitl-rules` abhängen lassen, Skript muss mit Exit 1 und der Zeile `humanitl-core -> humanitl-rules not allowed` scheitern.

### Tests
- `tools/tests/check_deps_test.py`: Fixture-`meta.json` mit einem Verstoß ⇒ Exit 1 und Verstoß in stdout; ohne Verstoß ⇒ Exit 0.
- CI-Job grün auf `main`.

### Akzeptanzkriterien
- [ ] `tools/check-deps.sh` liefert Exit 0 auf dem aktuellen Workspace.
- [ ] Negativtest (Schritt 5) liefert Exit 1 mit sprechender Zeile.
- [ ] Job `deps-lint` in `ci.yml` vorhanden und grün.
- [ ] `grep -rn 'TcpStream::connect'` außerhalb `egress/` bricht den Job.

### Fallstricke
- `cargo metadata` ohne `--no-deps` listet alle externen Crates; die Prüfung wird dann langsam und falsch. Immer `--no-deps`.
- Dev-Dependencies zählen nicht als Verstoß (Tests dürfen alles); im Skript `dep.kind == null` filtern.
- `tomllib` gibt es erst ab Python 3.11; CI-Runner prüfen.

### Referenzen
BACKLOG.md ADR-015; `docs/ARCHITECTURE.md` 2, 4; CONVENTIONS.md 3.1, 3.10b.
