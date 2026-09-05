# Konventionen für Issue-Spezifikationen

Dieses Dokument ist die gemeinsame Sprache aller Issue-Dateien unter `backlog/`. Jede Spezifikation verwendet ausschließlich die hier festgelegten Namen. Wer ein Issue umsetzt, liest zuerst `BACKLOG.md` (Abschnitte 2 bis 6), dann dieses Dokument, dann das Issue.

## 1. Issue-Template

Jedes Issue hat exakt diese Abschnitte in dieser Reihenfolge. Leere Abschnitte werden mit „keine" gefüllt, nicht weggelassen.

```
## HUM-xxx · Titel
Sprint: N · Größe: S|M|L · Abhängigkeiten: HUM-aaa, HUM-bbb · Blockiert: HUM-ccc

### Kontext
Warum es dieses Issue gibt, welche Entscheidung aus BACKLOG.md es umsetzt (ADR-Nummer), welches Nutzerproblem es löst.

### Ziel
Ein Absatz, der beschreibt, was nach Abschluss existiert und funktioniert. Aus Sicht eines Nutzers oder eines aufrufenden Moduls.

### Nicht-Ziel
Was ausdrücklich nicht Teil dieses Issues ist, auch wenn es naheliegt. Verweis auf das Issue, das es später macht.

### Betroffene Pfade
Liste der Dateien und Verzeichnisse, die angelegt oder geändert werden. Neue Dateien mit (neu).

### Spezifikation
Der technische Kern. Signaturen, Typen, Proto-Messages, CLI-Syntax, Config-Schlüssel, Dateiformate, Zustandsübergänge, Fehlercodes. Code-Blöcke in der Zielsprache. Keine Prosa, wo ein Typ reicht.

### Schritte
Nummerierte Reihenfolge der Umsetzung. Jeder Schritt endet in einem prüfbaren Zwischenzustand (kompiliert, Test grün, Befehl liefert X).

### Tests
Konkrete Testfälle mit Name, Eingabe, erwartetem Ergebnis. Unit, Integration, Escape, Widget, Golden, e2e je nach Ebene.

### Akzeptanzkriterien
Checkliste. Jeder Punkt ist mit einem Befehl oder einem Blick verifizierbar. Kein „funktioniert gut".

### Fallstricke
Bekannte Fehlerquellen, Randfälle, Sicherheitsfallen, Dinge, die ein Modell typischerweise falsch macht.

### Referenzen
BACKLOG.md-Abschnitte, ADRs, externe Quellen mit URL.
```

## 2. Sprache und Stil

- Prosa Deutsch. Bezeichner, Dateinamen, Befehle, Fehlermeldungen im Code Englisch.
- UI-Strings werden nie im Code hart verdrahtet, sondern als ARB-Schlüssel (`en` Quelle, `de` Übersetzung) benannt.
- Commit-Präfixe: `feat`, `fix`, `test`, `docs`, `chore`, `refactor`, gefolgt von Scope in Klammern, z. B. `feat(rules): label glob matching`.

## 3. Kanonische Namen

### 3.1 Crates (Cargo-Workspace unter `daemon/`)

| Crate | Pfad | Darf abhängen von |
|---|---|---|
| `humanitl-core` | `daemon/crates/core-types` | nichts intern |
| `humanitl-config` | `daemon/crates/config` | core |
| `humanitl-rules` | `daemon/crates/rules` | core |
| `humanitl-findings` | `daemon/crates/findings` | core |
| `humanitl-recorder` | `daemon/crates/recorder` | core |
| `humanitl-audit` | `daemon/crates/audit` | core |
| `humanitl-sandbox` | `daemon/crates/sandbox` | core, config |
| `humanitl-catalog` | `daemon/crates/catalog` | core |
| `humanitl-proxy` | `daemon/crates/proxy` | core, rules, findings, recorder |
| `humanitl-ipc` | `daemon/crates/ipc` | alle oben; einzige Crate mit Protobuf |
| `humanitld` | `daemon/bin/humanitld` | alle |
| `humanitl` (CLI) | `daemon/bin/humanitl` | core, config, ipc (als Client) |
| `humanitl-xtask` | `daemon/xtask` | Hilfs-Crate (`cargo xtask docs`), außerhalb der Abhängigkeitsregeln, nie Laufzeitcode |
| `humanitl-shim` | `daemon/bin/humanitl-shim` | nur libc, seccompiler; kein tokio. Startet die Bridges aus dem Profil, wendet dann seccomp an (`socket`/`socketpair` nur für `allow_families`, sonst EPERM; `deny_syscalls` EPERM; `PR_SET_NO_NEW_PRIVS`; TSYNC), dann `execvp` |

Rust: stable, Edition 2024, `tokio` (multi-thread), `tonic` + `prost`, `hudsucker` 0.25 mit Features `rcgen-ca`, `rustls-client`, `http2`, `rusqlite` (bundled) + `refinery`, `seccompiler`, `clap` (derive), `serde` + `toml` + `serde_yaml`, `schemars`, `thiserror`, `tracing` + `tracing-subscriber` (json), `uuid` (v7), `dashmap`, `globset`, `regex`, `idna`, `psl`, `sha2`, `hmac`, `zeroize`.

### 3.2 Kerntypen (`humanitl-core`)

```rust
pub struct FlowId(Uuid);      // v7
pub struct RuleId(Uuid);
pub struct SessionId(Uuid);
pub struct SandboxId(Uuid);
pub struct BodyRef { pub sha256: [u8; 32], pub size: u64, pub inline: Option<Bytes> }

pub enum FlowState {
    Received,
    Analyzed { findings: Vec<Finding> },
    Held { deadline: Instant },
    Decided(Decision),
    Forwarded,
    Responded { status: u16 },
    Failed { error: UpstreamError },   // nach Decided(Allow*) oder Forwarded: DNS, Connect, TLS, PrivateAddress, UpstreamTimeout
    Recorded,
}
pub enum UpstreamError { Dns, Connect, Tls, PrivateAddress(IpAddr), Timeout }
pub enum Decision { Allow, AllowEdited { request: HttpRequest }, Block { reason: BlockReason }, TimedOut }
pub enum BlockReason { User, Rule(RuleId), Timeout, BodyCap, AuthorityMismatch, NoRoute, HoldMemory, HoldMaxFlows, ClientTimeout, PrivateAddress }
// HTTP-Status je Grund: User/Rule/AuthorityMismatch/PrivateAddress 403, BodyCap 413, Timeout 504, HoldMemory/HoldMaxFlows 503, Upstream-Fehler (Failed) 502
pub enum Decision { Allow, AllowEdited { request: HttpRequest }, Block { reason: BlockReason, note: Option<String> }, TimedOut }   // ersetzt die Zeile oben

pub enum FlowEvent { Received{..}, Analyzed{..}, Held{..}, Decided{..}, Forwarded{..}, ResponseHeaders{..}, ResponseChunk{..}, Recorded{..}, TimedOut{..}, Lagged{ n: u64 } }

pub struct HttpRequest { pub method: Method, pub scheme: Scheme, pub authority: Authority, pub path_and_query: String, pub headers: HeaderMap, pub body: BodyRef, pub version: Version }
pub struct Authority { pub host: HostName, pub port: u16 }   // HostName ist normalisiert (A-Label, lowercase, ohne trailing dot) oder IpLiteral
pub enum HostName { Dns(String), Ip(IpAddr) }

pub struct Finding { pub kind: FindingKind, pub span: Range<usize>, pub location: FindingLocation, pub tier: Tier, pub value_hash: [u8; 32], pub display_prefix: String }
pub enum FindingLocation { Header(HeaderName), Query, Body }
pub enum Tier { Checksum, Regex, UserTerm }
pub enum FindingKind { ApiKey(String), Jwt, Email, Iban, CreditCard, Phone, Ipv4, UserTerm(String), Custom(String) }

pub struct Diagnostic { pub code: DiagnosticCode, pub severity: Severity, pub title: String, pub why: String, pub fix: Option<FixAction>, pub docs: Option<Url> }
pub enum Severity { Info, Warning, Error, Blocking }
pub enum FixAction { SetEnv { key, value }, AddRule(Rule), InstallService, ChangeSetting { key, value }, CopyCommand(String), OpenUrl(Url), RemountReadOnly(PathBuf) }
pub struct DiagnosticCode(pub &'static str);   // Schema: BEREICH_NNN, z. B. SANDBOX_001, TLS_003, LLM_002, DAEMON_001
```

Übergänge: `impl FlowState { pub fn on(self, t: Transition) -> Result<(FlowState, FlowEvent), InvalidTransition> }`. `Transition` ist die Eingabe (Analyze, Hold, Decide, Forward, Respond, Record, Timeout), `FlowEvent` die daraus abgeleitete Ausgabe (Abgleich: Event ist Output, nicht Input). Erlaubt genau: Received→Analyzed→Held→Decided→Forwarded→Responded→Recorded, Held→Decided(TimedOut)→Recorded, Decided(Block)→Recorded, Analyzed→Decided (Regel-Auto-Entscheidung, überspringt Held), Decided(Allow|AllowEdited)→Failed, Forwarded→Failed, Failed→Recorded. Alles andere `InvalidTransition { from, transition }`.

### 3.3 Regeln (Typen in `humanitl-core::rule`, Parser und Auswertung in `humanitl-rules`)

```yaml
# rules.yaml
version: 1
rules:
  - id: 018f...            # UUIDv7, optional beim Anlegen
    action: allow | block | ask | redact
    match:
      host: "*.github.com"   # Label-Glob; "**.github.com" = Apex + alle Subdomains; ohne Stern = exakt
      method: [GET, HEAD]    # optional, Liste
      path: "/repos/**"      # optional, Glob; "~^/v[0-9]+/" = Regex
      scheme: https          # optional
      port: 443              # optional
      upgrade: websocket     # optional; nur dann matcht ein WebSocket-Upgrade
    expires: never | session | "2026-09-03T10:00:00Z"
    stream: false            # nur relevant für Bodies über Cap
    allow_private: false     # true erlaubt Zieladressen in RFC-1918/Loopback/Link-Local/CGNAT; LLM-Passthrough setzt es
    created_from: <FlowId>   # optional
    bundled: false
    note: "…"
```

```rust
pub struct Rule { pub id: RuleId, pub action: Action, pub matcher: Matcher, pub expires: Expiry, pub stream: bool, pub created_from: Option<FlowId>, pub bundled: bool, pub note: Option<String> }
pub enum Action { Allow, Block, Ask, Redact }
pub enum Expiry { Never, Session(SessionId), At(DateTime<Utc>) }
pub struct RuleSet { rules: Vec<Rule> }        // geordnet, first match wins
impl RuleSet { pub fn evaluate(&self, req: &RequestKey, now: DateTime<Utc>, session: SessionId) -> Verdict }
pub struct RequestKey<'a> { pub host: &'a HostName, pub method: &'a Method, pub path: &'a str, pub scheme: Scheme, pub port: u16, pub upgrade: Option<Upgrade> }
pub enum Verdict { Matched { rule: RuleId, action: Action }, Default /* = Ask */ }
```

Glob-Semantik auf Labels: `*` genau ein Label, `**` ein oder mehr Labels, `**.example.com` matcht zusätzlich `example.com` selbst. Vergleich nach Normalisierung (`idna::domain_to_ascii`, lowercase, trailing dot entfernt). `HostName::Ip` matcht nie einen Host-Glob, nur eine Regel mit `host: "ip:192.168.1.50"` oder `host: "cidr:192.168.0.0/16"`. Unbekannte Methoden führen zu `Ask`. `upgrade: Some(WebSocket)` matcht nur Regeln mit `upgrade: websocket`, sonst `Ask`.

### 3.4 Sandbox (`humanitl-sandbox`)

```rust
pub trait SandboxBackend: Send + Sync {
    fn name(&self) -> &'static str;                                   // "bwrap", später "docker"
    fn plan(&self, profile: &SandboxProfile, session: &SessionContext) -> Result<LaunchPlan, Diagnostic>;
    fn launch(&self, plan: &LaunchPlan) -> Result<SandboxHandle, Diagnostic>;
    fn isolation_check(&self, handle: &SandboxHandle) -> Vec<CheckResult>;
}
pub struct LaunchPlan { pub argv: Vec<OsString>, pub env: Vec<(String, String)>, pub fds: Vec<(RawFd, RawFd)> }  // argv ist vollständig, wird im UI angezeigt
pub struct CheckResult { pub check: IsolationCheck, pub passed: bool, pub evidence: String, pub diagnostic: Option<Diagnostic> }
pub enum IsolationCheck { NoNetworkInterface, SingleSocket, SeccompActive }
```

Profil `profiles/sandbox/default.toml`:

```toml
[sandbox]
backend = "bwrap"
hostname = "sandbox"
[mounts]
work = { dst = "/work", mode = "rw" }          # src kommt aus Session
ro = ["/usr", "/etc/ssl", "/etc/alternatives"]
tmpfs = ["/tmp", "/dev/shm", "/work/.git/hooks", "/work/.vscode", "/work/.idea"]
masked_files = ["/work/.envrc", "/work/.git/config"]
[network]
proxy_socket_dst = "/run/humanitl/proxy.sock"
proxy_port = 3128
# Bridges, die der Shim vor seccomp startet. Richtung "in": Sandbox-TCP -> Host-UDS (Proxy).
# Richtung "out": Host verbindet UDS in der Sandbox -> Sandbox-TCP (später Browser-CDP, Profil browser).
bridges = [ { name = "proxy", dir = "in", listen = "127.0.0.1:3128", socket = "/run/humanitl/proxy.sock" } ]
[seccomp]
# Erlaubte Socket-Familien nach dem Bridge-Start. AF_INET/AF_INET6 sind immer nötig (Loopback zum Proxy);
# das Netz-Namespace hat nur lo, darum ist das sicher. AF_UNIX nur im Profil "browser" (Chromium-IPC).
allow_families = ["AF_INET", "AF_INET6"]
allow_types = ["SOCK_STREAM"]          # SOCK_DGRAM/SOCK_RAW immer EPERM (arg1 & 0xff); Profil browser ergänzt AF_UNIX und SOCK_DGRAM für Chromium
deny_syscalls = ["ptrace", "io_uring_setup", "io_uring_enter", "io_uring_register", "process_vm_readv", "process_vm_writev", "keyctl", "add_key", "request_key"]
[env]
HTTP_PROXY = "http://127.0.0.1:3128"
HTTPS_PROXY = "http://127.0.0.1:3128"
NO_PROXY = ""
SSL_CERT_FILE = "/etc/humanitl/ca.crt"
# ... vollständiges Env-Kit siehe HUM-014
```

Pfade in der Sandbox: Proxy-Socket `/run/humanitl/proxy.sock`, CA `/etc/humanitl/ca.crt`, Projekt `/work`, Shim `/usr/local/bin/humanitl-shim`. Auf dem Host: Proxy-Socket `$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock` (Verzeichnis 0700), gRPC-Socket `$XDG_RUNTIME_DIR/humanitl/daemon.sock` (0600), CA `$XDG_DATA_HOME/humanitl/ca/ca.key` (0600) und `ca.crt`, Datenbank `$XDG_DATA_HOME/humanitl/humanitl.db`, Blobs `$XDG_DATA_HOME/humanitl/blobs/<hex[0..2]>/<sha256-hex>`, Audit `$XDG_DATA_HOME/humanitl/audit/audit.jsonl`, Config `$XDG_CONFIG_HOME/humanitl/config.toml`, Regeln `$XDG_CONFIG_HOME/humanitl/rules.yaml`, Profile `$XDG_CONFIG_HOME/humanitl/profiles/*.toml`, Projekt-Profil `<projekt>/.humanitl/profile.toml`.

### 3.5 Proxy (`humanitl-proxy`)

Defaults: Hold-Timeout 300 s (`hold.timeout_secs`), Request-Body-Cap 32 MiB (`hold.body_cap_bytes`), Vorschau-Cap 8 MiB (`preview.cap_bytes`), Inline-Blob-Grenze 256 KiB (`recorder.inline_max_bytes`), Broadcast-Kapazität 1024 (`ipc.event_buffer`), Dekompressions-Ratio-Limit 100 (`preview.max_decompress_ratio`). Bei Block antwortet der Proxy an den Client mit `403` und Body `text/plain`:

```
Blocked by Humanitl.
reason: <BlockReason als snake_case>
flow: <FlowId>
host: <host>
```

```rust
pub struct HoldQueue { map: DashMap<FlowId, oneshot::Sender<Decision>> }
impl HoldQueue { pub fn hold(&self, id: FlowId, deadline: Instant) -> impl Future<Output = Decision>; pub fn decide(&self, id: FlowId, d: Decision) -> Result<(), NotHeld> }
```

### 3.6 IPC (`proto/humanitl/v1/humanitl.proto`)

Package `humanitl.v1`, Service `Humanitl`. RPC-Namen und Messages wie BACKLOG.md 3.3. Feld-Konventionen: IDs als `string` (UUID-Text), Zeitstempel `google.protobuf.Timestamp`, Bodies nie inline in Events, nur `BodyRef { bytes sha256; uint64 size; bool truncated }`, Enums mit `_UNSPECIFIED = 0`. Metadata-Header `x-humanitl-token` mit Session-Token aus `$XDG_RUNTIME_DIR/humanitl/token` (0600).

### 3.7 Konfiguration (`humanitl-config`)

```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Config { pub llm: LlmConfig, pub hold: HoldConfig, pub sandbox: SandboxRef, pub agent: AgentRef, pub recorder: RecorderConfig, pub preview: PreviewConfig, pub ipc: IpcConfig, pub ui: UiConfig, pub experimental: Experimental }
```

Jedes Feld hat `#[schemars(description = "…")]` und `#[humanitl(tier = "basic|advanced|expert")]` (eigenes Attribut-Makro oder `#[schemars(extend("x-tier" = "…"))]`). Präzedenz, niedrig nach hoch: eingebaute Defaults, `config.toml` global, Profil global, Profil Projekt, Env `HUMANITL_*` (Pfad mit `__`, z. B. `HUMANITL_HOLD__TIMEOUT_SECS`), CLI-Flag. Jede Auflösung merkt sich die Herkunft (`Origin`) pro Feld für die UI.

Config-Schlüssel (Auszug, verbindlich): `llm.endpoint` (URL), `llm.passthrough_paths` (Default `["/v1/", "/api/"]`), `hold.timeout_secs`, `hold.body_cap_bytes`, `hold.ask_mode` (`ui|terminal|none`), `sandbox.profile`, `sandbox.work_dir`, `sandbox.work_mode` (`ro|rw`), `agent.adapter` (`opencode`), `agent.command` (Override), `recorder.inline_max_bytes`, `recorder.retention_days`, `preview.cap_bytes`, `ipc.event_buffer`, `ui.language` (`en|de`), `ui.theme` (`dark|light|system`), `ui.notifications`, `ui.sound`, `experimental.h2_upstream`, `experimental.ws_hold`.

### 3.8 CLI (`humanitl`)

```
humanitl run [--profile NAME] [--work DIR] [--ask ui|terminal|none] [--llm URL] [-- CMD...]
humanitl sandbox run [--profile NAME] -- CMD...
humanitl sandbox argv [--profile NAME]
humanitl sandbox check
humanitl sandbox attach [--read-only]
humanitl rules list|add|remove|test URL [--json]
humanitl flows list [FILTER] | show ID [--json]
humanitl sessions summary ID [--json]
humanitl audit verify|export [--format jsonl|csv] [--out FILE]
humanitl config get KEY | set KEY VALUE | schema | edit
humanitl daemon install|status|logs
```

`humanitl sessions summary ID` nennt die Sandbox-Kennung eines Laufs, nicht die Sitzung des Daemons: Ein Daemon-Prozess hat genau eine `SessionId`, startet darin aber beliebig viele Sandboxen, und die Zusammenfassung gehört dem Lauf (HUM-043). `ID` ist die Kennung aus der Log-Zeile des Laufs.

Exit-Codes: 0 ok, 1 Nutzerfehler (mit Diagnostic), 2 Daemon nicht erreichbar, 3 Sandbox-Check fehlgeschlagen, 4 Sicherheitsverletzung (z. B. Authority-Mismatch im Test), 10 `rules test` ⇒ block, 11 `rules test` ⇒ ask.

### 3.9 Flutter (`app/`)

Flutter 3.47.2 gepinnt in `app/.fvmrc`; das ist der Pin, den der CI-Job liest (`.github/actions/setup-flutter`). `app/pubspec.yaml` führt die Untergrenze zusätzlich als eigenen Constraint (`flutter: ">=3.47.2"`, `sdk: ^3.13.0`). Der Pin folgt dem neuesten stabilen Flutter; wo ein Paket die Anhebung blockiert, wird das Paket genannt und nicht die Anhebung vertagt. Die Komponentenbibliothek ist `shadcn_flutter`, exakt gepinnt und ausschließlich in `app/packages/ui` importiert (ADR-0009, Abschnitt „Revidiert am 2026-09-04 durch den Projekteigentümer"). Pakete: `flutter_riverpod` 3.x + `riverpod_annotation` + `riverpod_generator`, `freezed` 4.x + `json_serializable`, `grpc` 5.x + `protobuf`, `re_editor`, `xterm2`, `diff_match_patch`, `window_manager`, `file_picker`, `dbus` (Tray und Notification ohne Plugin, HUM-034), `flutter_localizations` + `intl`, `alchemist` (dev). `two_dimensional_scrollables` steht noch aus und wird erst mit dem JSON-Baum entschieden (HUM-030); die History-Tabelle braucht es nicht.

Struktur:

```
app/lib/
  main.dart
  app.dart                         Root, Theme, Router, Shortcuts
  core/ipc/                        DaemonClient (Interface) + GrpcDaemonClient + FakeDaemonClient
  core/ipc/generated/              gitignored
  core/domain/                     Dart-Spiegel der Kerntypen (freezed): Flow, FlowState, Decision, Rule, Finding, Diagnostic
  core/ui/                         Re-Export von packages/ui
  features/<feature>/
    <feature>_screen.dart
    providers/                     riverpod, @riverpod-Generator
    widgets/
    l10n-Schlüssel mit Präfix <feature>_
app/packages/ui/                   Widget-Vokabular auf reinem Flutter: HTokens, HTheme, HButton, HRow, HBadge, HModal, HSheet, HTextField, HSegmented ...
app/l10n/app_en.arb, app_de.arb
app/test/, app/test/goldens/, app/integration_test/
```

Provider-Namen (verbindlich): `daemonClientProvider`, `daemonInfoProvider`, `flowEventsProvider` (StreamProvider), `flowsProvider` (Notifier, Map<FlowId, Flow>), `heldFlowsProvider` (abgeleitet, sortiert nach Deadline), `selectedFlowIdProvider`, `flowBodyProvider(BodyRef)`, `rulesProvider`, `sandboxStatusProvider`, `isolationCheckProvider`, `diagnosticsProvider`, `configProvider`, `catalogProvider`, `draftProvider(FlowId)` (Editor-Entwurf), `pseudonymMapProvider`.

Design-Tokens als `HTokens` (Farben, Typo, Spacing) exakt nach BACKLOG.md Abschnitt 5. Zustandsfarben über `FlowStateColor.of(state)`.

Shortcuts (verbindlich, als `Intent`-Klassen): `AllowIntent` Enter/`A`/Ctrl+F, `BlockIntent` `B`/Ctrl+L, `EditIntent` `E`, `ScopeIntent` `R`, `AllowGroupIntent` Ctrl+Shift+F, `BlockGroupIntent` Ctrl+Shift+L, `NextFlowIntent` `J`/↓, `PrevFlowIntent` `K`/↑, `DurationIntent(n)` `1`/`2`/`3`, `TargetIntent(n)` Shift+1..4, `FilterIntent` `/`, `PaletteIntent` Ctrl+K, `NavIntent(n)` Ctrl+1..5, `ToggleDomainPanelIntent` Ctrl+D, `AddRuleIntent` Ctrl+B.

### 3.10 Agent-Adapter

```rust
pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &'static str;                          // "opencode"
    fn command(&self, ctx: &SessionContext) -> Vec<OsString>;
    fn env(&self, ctx: &SessionContext) -> Vec<(String, String)>;
    fn files(&self, ctx: &SessionContext) -> Vec<(PathBuf, Vec<u8>)>;   // in Sandbox anzulegen, z. B. opencode.json
    fn default_rules(&self) -> Vec<Rule>;
    fn llm_passthrough(&self, llm: &LlmConfig) -> Rule;
}
```

### 3.10b Egress (Proxy-Upstream)

```rust
pub trait Egress: Send + Sync { async fn connect(&self, authority: &Authority, resolved: Option<IpAddr>) -> Result<Box<dyn AsyncStream>, Diagnostic>; }
pub struct Direct;   // MVP: TcpStream zu gepinnter IP; später HttpProxy(Url), Socks5h(Url)
```

Kein `TcpStream::connect` im Proxy außerhalb der `Egress`-Implementierungen (CI-Grep in `tools/check-deps.sh`).

### 3.11 Escape-Tests (`tests/escape/`)

Shell-Skripte plus ein Rust-Testrunner. Jeder Test hat eine ID `ESC-1` bis `ESC-5` (BACKLOG.md 4.5), Dateien `esc-1-sockets.sh`, `esc-2-mounts.sh`, `esc-3-egress.sh`, `esc-4-rules.sh`, `esc-5-filesystem.sh`, läuft via `humanitl sandbox run --profile test -- /tests/escape/esc-N-<name>.sh`, schreibt JUnit-XML nach `target/escape/`. Ein Test ist grün, wenn jede Exfiltrationsprobe fehlschlägt und die erwartete Beobachtung eintritt.

### 3.12 Definition of Done (für jedes Issue)

- Alle Akzeptanzkriterien abgehakt, Tests aus dem Issue vorhanden und grün, CI grün.
- Neue Fehlerpfade liefern `Diagnostic` mit `code`, `why`, wo möglich `fix`.
- Neue Config-Felder haben Tier, Beschreibung, Default, stehen in der Schema-Ausgabe.
- Neue UI-Strings in `app_en.arb` und `app_de.arb`.
- Keine `unwrap()`/`expect()` außerhalb von Tests und `main`, keine `Err(String)`.
- `cargo clippy -- -D warnings`, `cargo fmt --check`, `flutter analyze` sauber.
- Doku-Kommentar auf jedem öffentlichen Typ und jeder öffentlichen Funktion.


## 4. Abgleich-Entscheidungen (2026-09-02)

Die Sprint-Files wurden parallel geschrieben und haben an einigen Stellen die Abschnitte 3.x erweitert. Hier die verbindliche Auflösung. Wo ein Sprint-File abweicht, gilt dieser Abschnitt.

### 4.1 Typ-Heimat
- `Rule`, `Matcher`, `Action`, `Expiry`, `HostPattern`, `Upgrade` liegen in `humanitl-core::rule` als reine Werttypen. `humanitl-rules` enthält YAML-Parser, Normalisierung und `RuleSet::evaluate`. `humanitl-catalog` nutzt `HostPattern` aus core.
- `FixAction::AddRule(Box<Rule>)`, kein `RuleStub`.

### 4.2 Zustandsautomat
- Signatur `on(self, Transition) -> Result<(FlowState, FlowEvent), InvalidTransition>` (HUM-004).
- `Held`-Event trägt zusätzlich `queue_bytes`, `queue_count` (HUM-057).

### 4.3 Proto-Erweiterungen gegenüber BACKLOG.md 3.3
`GetConfig`/`SetConfig`; `FlowEvent` hat die Varianten `Diagnostic`, `RulesChanged`, `AgentAsk`; `Received` trägt `DomainInfo`; `DecideRequest.remember: Rule`, `DecideRequest.block.note`; `DecideResponse.created_rule`; `RulesRequest.make_permanent`; `SandboxRequest.argv`; eigene Datei `rules.proto`, importiert von `humanitl.proto`.

### 4.4 Config-Schlüssel (Ergänzung zu 3.7)
- Gruppe `limits` (HUM-057) ist die Heimat aller Caps und Timeouts: `limits.hold_body_cap_bytes` (Alias `hold.body_cap_bytes`), `limits.preview_cap_bytes` (Alias `preview.cap_bytes`), `limits.event_buffer` (Alias `ipc.event_buffer`), `limits.max_decompress_ratio`, `limits.hold_max_flows`, `limits.hold_max_bytes`, `limits.connect_timeout_secs`, `limits.header_timeout_secs`, `limits.body_timeout_secs`, `limits.recorder_max_body_bytes` (Alias `recorder.max_body_bytes`).
- `resolver.nameserver`, `resolver.overrides`, `resolver.cache_ttl_secs`, `resolver.prefer` (`ipv4|ipv6`), `resolver.test_ca` (nur Tests).
- `upstream.connect_timeout_secs` ist Alias von `limits.connect_timeout_secs`.
- `findings.enabled`, `findings.user_terms`, `findings.email_allow_domains`, `findings.ignored_hashes`.
- `agent.briefing.enabled` (HUM-071).
- `experimental.upstream_port_map` (nur Tests).

### 4.5 Regeln
- **Die Auswertungsreihenfolge, vier Ränge (entschieden in HUM-104, 2026-09-04):**

  1. die **mitgelieferte** Durchreiche zum Sprachmodell (`passthrough_llm` **und** `bundled`),
  2. die Sitzungsregeln **des Nutzers** (In-Memory, `expires: session`, nicht mitgeliefert),
  3. die dauerhaften Regeln des Nutzers aus `rules.yaml`,
  4. alles übrige Mitgelieferte (`rules/default.yaml`, Agent-Adapter, Profile).

  Innerhalb eines Rangs gilt die Reihenfolge der Liste; die erste passende Regel gewinnt. Passt nichts, gilt `ask`.

  Alle vier Ränge macht `RuleSet::evaluate` in eigenen Durchgängen (`Tier`). Sie hängen an der Regel selbst, nicht an ihrem Platz in der Liste; die Liste ordnet nur innerhalb eines Rangs. `RulesStore::snapshot_of` hängt die mitgelieferten Regeln trotzdem ans Ende — die Anzeige soll die Ordnung nicht anders herum behaupten als die Auswertung.

  **`bundled` schlägt `expires`.** Eine mitgelieferte Regel mit `expires: session` ist keine Sitzungsregel des Nutzers und fällt in Rang 4, nicht in Rang 2. Sonst überholte sie seine dauerhaften Regeln, und HUM-027 hinge daran, welche Gültigkeit `rules/default.yaml` gerade schreibt.

  **Die Lehre aus HUM-104, für jedes Feld, das jemand später hinzufügt: Ein Feld, an dem eine Rangordnung hängt, muss dem Lader gehören und darf nicht aus einer Datei kommen, die von der Ordnung betroffen ist.** Antigravity und Codex fanden am 2026-09-04 unabhängig je eine Hälfte derselben Lücke — erst `passthrough_llm`, dann `bundled`. Wer ein solches Feld einführt, schreibt zugleich hin, wo es gesetzt wird, und einen Test, der belegt, dass eine Datei es nicht setzen kann.

  **Der Vermerk `bundled` sagt, woher eine Regel kommt, und keine Datei setzt ihn.** Gesetzt wird er allein in `RuleSet::add_bundled`, also für die Regeln des Agent-Adapters und aus `rules/default.yaml`. `parse_rules` verwirft ihn und meldet `RULES_010` als Warnung — das gilt für die `rules.yaml` des Nutzers wie für `[rules].inline` und `[rules].files` eines Profils, denn beide gehen durch denselben Parser. `humanitl_ipc::convert::rule_from_proto` verwirft ihn für die Leitung. `rules/default.yaml` schreibt ihn deshalb nicht mehr hin.

  Das ist die Bedingung dafür, dass Rang 1 und Rang 4 eine Ordnung und keine Bitte sind: Rang 1 lässt eine Anfrage **ungehalten** hinaus und warnt bei Funden nur (`LLM_005`). Hinge er an `passthrough_llm` allein, stellte sich jede Datei den Rang selbst aus — und wer `passthrough_llm: true` schreibt, weil er ein zweites Modell durchreichen will, überholte damit unbemerkt seine eigenen Block-Regeln. Eine Durchreiche aus einer Datei behält alles andere: Sie wird nicht gehalten, sie warnt mit `LLM_005`, sie trägt `DecisionSource::Passthrough`. Sie steht nur an ihrem Platz in der Liste, und innerhalb derselben Datei bestimmt ihr Verfasser den ohnehin selbst.

  Begründungen, Rang für Rang:

  - **Rang 1.** Die Durchreiche ist der eine erklärte Seitenkanal (BACKLOG.md 4.2, `docs/SECURITY.md`). Erklärt ist er nur, solange er als solcher erkennbar bleibt: `DecisionSource::Passthrough` in der Aufzeichnung und die Warnung `LLM_005` vor Funden hängen daran, dass **diese** Regel entschieden hat (`pipeline.rs`). Entschiede eine breitere `allow`-Regel des Nutzers zuerst über denselben Host, sähe der Kanal aus wie jede andere Freigabe — und eine `block`-Regel wie die des Profils `llm-only` (`host: "**"`) blockte das eigene Modell. Deshalb steht die Durchreiche vor allem anderen, und ihr Vorrang hängt an der Regel, nicht an der Zusammenbau-Reihenfolge; eine Reihenfolge, die nur „meistens" stimmt, wäre für einen protokollierten Seitenkanal keine.
  - **Rang 2 vor Rang 3.** Was der Nutzer gerade entschieden hat, soll sofort gelten, auch wenn eine ältere persistente Regel breiter matcht.
  - **Rang 3 vor Rang 4.** Eine eigene Regel überstimmt eine mitgelieferte (HUM-027). Mitgelieferte Regeln gehören nicht dem Nutzer und lassen sich nicht löschen (`RULES_010`); der Fix, den dieser Befund vorschlägt, ist eine eigene Regel mit demselben Muster. Stünden die mitgelieferten vorn, verspräche er etwas Unmögliches.

  Verworfen wurde der zweite Weg aus der Spezifikation, „mitgeliefert ganz nach vorn" (wie `docs/profiles.md` es vor HUM-104 beschrieb). Er hätte Rang 3 gegen Rang 4 getauscht und damit HUM-027 aufgehoben, und er hätte den stillen Fall nicht gelöst: Eine Sitzungsregel `allow host "**"` hätte die Durchreiche weiterhin überdeckt, denn eine mitgelieferte Regel steht nicht vor einer Sitzungsregel.

- `match.upgrade: websocket` ist Teil des Schemas.

### 4.6 Diagnostic-Register
Datei `daemon/crates/core-types/src/diagnostics/codes.rs` hält alle Codes als Konstanten mit Doc-Kommentar. Reservierte Bereiche: `DAEMON_001..019`, `SANDBOX_001..029` (001–006 Launcher/Profil, 007 Bridge-Richtung, 010–012 Start-Fehler), `TLS_001..009`, `LLM_001..009`, `RULES_001..019` (001–008 Datei und Muster, 009–011 Regelspeicher aus HUM-027), `TERM_001..009`, `RECORDER_001..009`, `LIMIT_001..009`, `AUDIT_001..009`, `CONFIG_001..009`. Ein Code wird nie wiederverwendet; entfernte Codes bleiben als `#[deprecated]` stehen. CI-Test: jeder im Code verwendete Code ist im Register.

`CONFIG_005` („Veralteter Schlüssel") trägt mit HUM-101 zwei Fälle, und die Severity trennt sie: **`Info`** heißt Alias — der Schlüssel steht unter seinem alten Namen da, sein Wert **gilt**, und der Befund nennt den heutigen Namen; **`Warning`** heißt ersatzlos entfallen (`alias::RETIRED`) — der Wert **verfällt**, und der Befund nennt das Issue, das ihn entfernt hat, samt Grund. Beide Fälle stehen im Doc-Kommentar des Codes, damit ein Konsument nicht raten muss. Ein eigener Code wäre das Sauberere, aber der reservierte Bereich `CONFIG_001..009` ist voll; ihn zu erweitern ist eine Entscheidung über das Register und gehört dem, der den nächsten Config-Code braucht — dann bekommt der entfallene Schlüssel den neuen Code, und diese Zeile fällt weg. Bis dahin gilt: Wer `CONFIG_005` auswertet, verzweigt über die Severity, nie über den Titel.

### 4.7 Fake-Modus
- Daemon: `humanitld --fake <session.jsonl> [--speed N]`.
- Flutter: `--dart-define=HUMANITL_FAKE=<scenario>`; `1` oder `default` = `mixed.jsonl`-Szenario über den Fake-Daemon, andere Werte = `FakeDaemonClient`-Szenarien in Dart (HUM-058).

### 4.8 Sandbox
- Profil hat `[network].bridges` (Liste, `dir = "in" | "out"`) und `[seccomp].allow_families` (Default `AF_INET`, `AF_INET6`), `[seccomp].deny_syscalls`.
- seccomp-Filter liegt in `daemon/bin/humanitl-shim/src/seccomp.rs`; `SECURITY.md` zitiert die Liste von dort.
- Escape-Test-Dateien wie in 3.11.

### 4.9 Egress
Alle Upstream-Verbindungen über `Egress` (3.10b). `tools/check-deps.sh` (HUM-074) prüft `TcpStream::connect` außerhalb `proxy/src/egress/`.

### 4.10 Aus den Reviews von Antigravity und Codex (2026-09-02)
- `FlowState::Failed` und `UpstreamError` wie in 3.2; HUM-024 verbucht DNS-/Connect-Fehler als `Failed`, nie als `Responded{502}`.
- Private Zieladressen nach Auflösung: verweigern (`PrivateAddress`), außer Regel hat `allow_private: true`. LLM-Passthrough-Regel setzt es; `localhost` funktioniert damit.
- **`allow_private` einer treffenden Regel wirkt bei jeder Aktion, nicht nur bei `allow` (HUM-102).** `RulesPipeline::decide` setzt `flow.allow_private` vor der Fallunterscheidung über die Aktion. Das Recht öffnet ein Ziel, es entscheidet nichts: Erst dadurch gibt es überhaupt einen Weg, ein privates Ziel zu öffnen und trotzdem jede Anfrage dorthin einem Menschen zu zeigen (`action: ask`), und genau diese Regel schlägt der Befund `PROXY_008` vor. Für `action: block` bleibt es folgenlos — was geblockt wird, wird nicht verbunden; ein Test hält das fest (`daemon/crates/proxy/tests/private_address.rs`).
- **Die abgelehnte private Adresse hat einen Befund: `PROXY_008` (HUM-102).** Er entsteht an genau einer Stelle, in `FlowHandler::record_failure` beim Match auf `UpstreamError::PrivateAddress(ip)`, und trägt einen `FixAction::AddRule(Box<Rule>)` mit `action: ask` und `allow_private: true`, zugeschnitten auf Host, Port, Schema, Methode, Protokollwechsel und Pfadpräfix der gescheiterten Anfrage. Die Adresse steht im Befund und in `resolved_ip`, **nie** im Rumpf der Blockantwort und nie in einer Kopfzeile: Die Sandbox hat keinen Resolver, und die Zuordnung von Name zu privater Adresse wäre für den Agenten neue Information über das lokale Netz. Aus demselben Grund nennt die Notiz der vorgeschlagenen Regel die Adresse nicht — `rules.yaml` ist über den Meta-Endpunkt (HUM-073) für den Agenten lesbar. ADR-0006 nannte bis dahin `PROXY_005`; diese Nummer bedeutet im Code „Ungültiger Übergang im Flow".
- **Ein Regelvorschlag ist ein Versprechen, und es hat zwei Hälften: Er muss durch `parse_rules` passen, und er muss die Anfrage danach treffen (HUM-102).** Ein Klick auf ein `FixAction::AddRule` schreibt die Regel in die `rules.yaml` des Nutzers. Ein einziger Wert außerhalb des Wertebereichs, den `parse_rules` kennt, lehnt **die ganze Datei** ab — der Nutzer verlöre alle seine Regeln, ausgelöst von einer Anfrage, die der Agent frei formt. Und eine Regel, die zwar parst, aber die gescheiterte Anfrage nicht trifft, ist die Falle, wegen der dieses Issue umgedreht wurde: Der Mensch klickt und bekommt beim nächsten Versuch dieselbe Ablehnung ohne neue Erklärung.

  Deshalb gilt für **jedes** Feld, das ein Vorschlag aus einer Anfrage überträgt: Der Wertebereich von `parse_rules` steht im Doc-Kommentar der bauenden Funktion, und ein Tabellentest schickt die vorgeschlagene Regel für eine Liste feindlicher Anfragen durch `serialize_rules` und `parse_rules` und danach gegen den `RequestKey` der Anfrage (`daemon/crates/proxy/tests/private_address.rs`). Wer ein Feld hinzufügt, hängt seine Zeile an beide Tabellen. Vier Löcher dieser Art fanden sich so: unbekannte Methode und Port `0` (Datei abgelehnt), Punktsegmente im Pfad und wiederum die unbekannte Methode (Regel wirkungslos) — die letzten beiden hat der Tabellentest selbst gefunden, nicht ein Reviewer.

  Für `PROXY_008` heißt das im Einzelnen: Port `0` und eine Methode außerhalb von `is_known_method` bekommen **keinen** Vorschlag (`NoRule::PortZero`, `NoRule::UnknownMethod`); der `why` nennt dann den Grund statt eines Knopfes. Ein Pfadpräfix steht nur in der Regel, wenn es `path_prefix_is_valid` besteht **und** `prefix_matches` gegen die Anfrage selbst zutrifft — ein Pfad mit `..`-Segment trifft nie ein Präfix, auch verschleiert nicht. Der Protokollwechsel wird übernommen, weil die Auswertung die Upgrade-Dimension beidseitig prüft: Eine Regel ohne `upgrade` träfe genau den gescheiterten WebSocket nicht und öffnete stattdessen gewöhnliches HTTP.
- **Ein `FixAction::AddRule` landet beim Klick heute am Ende der Nutzerregeln und wirkt dann nicht, sobald eine ältere Regel denselben Host trifft (offen, aus dem Review zu HUM-102).** `humanitl_ipc::convert::rule_to_proto` sendet `position: 0`, `position_of` liest das als „ans Ende", `RulesStore::add` hängt an, und `RuleSet::evaluate` nimmt den **ersten** Treffer eines Rangs. Der Mensch klickt, versucht es erneut und bekommt denselben Befund ohne neue Erklärung — dieselbe Falle, wegen der HUM-102 umgeschrieben wurde, eine Ebene höher. Das betrifft **jeden** Regelvorschlag im Produkt, nicht nur `PROXY_008`, und die Behebung fasst die Wire-Form an; sie bekommt ein eigenes Issue. Wer bis dahin ein `FixAction::AddRule` baut, schreibt die Bedingung in `why` und in die Notiz der Regel, und ein Test misst beide Reihenfolgen (`the_suggestion_only_takes_effect_in_front_of_the_rule_that_decided`). Gegen eine **Sitzungsregel** hilft auch die Position nicht: Rang `Session` liegt vor Rang `User` (4.5), eine dauerhafte Regel überholt sie nie — dann ist die Sitzungsregel selbst zu ändern.
- **Ein Regelvorschlag, der zwar in der Datei steht, aber nie an die Reihe kommt, ist derselbe Fehler wie einer, der nicht parst.** Beide Hälften des Versprechens gehören geprüft: Rundlauf durch `parse_rules` **und** ein Treffer gegen den `RequestKey`, und der Rundlauf-Test holt die Regel aus dem `fix` des echten Befunds, nicht aus der bauenden Hilfsfunktion — sonst bleibt er grün, wenn die Kopplung bricht.
- **Fällt das Pfadpräfix weg, gilt die Regel für jeden Pfad des Hosts, und der Befund sagt das (HUM-102).** `CompiledPrefixes::Any` ist der Rückfall einer leeren Präfixliste. Der Vorschlag bleibt trotzdem, denn die Wurzel eines Dienstes (`GET /`) ist der Normalfall und nicht der Ausnahmefall, und die Regel gibt nichts frei: Sie öffnet ein Ziel, und jede Anfrage dorthin wird weiterhin gehalten. Verschwiegen wird die Weite nicht — sie steht im `why`. Wer künftig ein `FixAction::AddRule` baut, dessen Zuschnitt an der Anfrage scheitert, schreibt die verbleibende Weite ebenso hin, statt sie dem Klick zu überlassen.
- HTTP-Status je `BlockReason` wie in 3.2 kommentiert; Body-Format bleibt einheitlich.
- seccomp (HUM-012): `socket()` nur `allow_families` × `allow_types` (arg1 maskiert mit `0xff`, damit `SOCK_NONBLOCK|SOCK_CLOEXEC` durchgehen); Arch-Mismatch ⇒ `KillProcess`; x32-Syscalls (`nr & 0x40000000`) ⇒ `EPERM` über ein handgeschriebenes BPF-Präludium vor dem seccompiler-Programm, nicht „falls nötig"; bwrap-Argv enthält `--cap-drop ALL`. Filter-Datei `daemon/bin/humanitl-shim/src/seccomp.rs` mit `#[cfg(test)]`-Tabelle aller Regeln.
- Resolver (HUM-015): kein `GaiResolver`, kein `HttpConnector`-DNS; Auflösung ausschließlich über den `Resolver`-Port nach Allow, Verbindung über `Egress::connect(authority, Some(ip))`.
- Listener (HUM-015 Schritt 0): Spike, ob hudsucker einen generischen Accept-Stream annimmt; sonst Fork der Accept-Schleife auf `UnixListener`. Kein Loopback-TCP-Port auf dem Host.
- Hold-Budget (HUM-016): `limits.hold_max_bytes` und `limits.hold_max_flows` als atomare Zähler in der `HoldQueue`; Überschreiten ⇒ `503`, `BlockReason::HoldMemory`/`HoldMaxFlows`. HUM-057 tunt nur.
- `Expect: 100-continue` (HUM-015): sofort `100 Continue`, Body in den Hold-Puffer; nichts erreicht den Upstream vor der Entscheidung.
- Protokoll-Ziel M1: h1 beidseitig, ALPN bietet dem Client nur `http/1.1`; gRPC-Zeile der Matrix ist in M1 „erwartet: fehlschlägt mit `PROXY_007 h2 not available`", grün ab M6.
- Terminal (HUM-042/067): genau ein schreibender Client, beliebig viele lesende (`read_only: true`); Geometrie ist die des Schreibers, Leser erhalten `Resize`-Events und rendern letterboxed. `TERM_001` nur bei zweitem Schreiber.
- `--ask terminal` verweigert Vollbild-TUI-Agenten (`AgentAdapter::is_fullscreen_tui()`), Diagnostic `CLI_002`.
- Rücktausch von Pseudonymen in nicht-gestreamten Text-Antworten ist MVP (HUM-079, Sprint 4).
- M1 zeichnet noch nicht dauerhaft auf; der Recorder (HUM-026) ist Bedingung für die Aussage „alles wird aufgezeichnet".

### 4.11 Aus der Umsetzung von Sprint 0 (2026-09-02)

Entscheidungen, die beim Bauen fielen und ab jetzt gelten. Wo 3.x anderes sagt, gilt dieser Abschnitt.

**Zustandsautomat (HUM-004).**
- `Transition` ist ein Umschlag `{ flow_id: FlowId, at: SystemTime, input: TransitionInput }`, weil der reine Automat weder IDs erfinden noch eine Uhr lesen darf. `TransitionInput` hat die Varianten `Analyze`, `Hold { deadline, queue_bytes, queue_count }`, `Decide { decision, source }`, `Forward`, `Respond { status }`, `Record`, `Timeout`, `Fail { error }`. Konstruktoren `Transition::analyze(..)` usw.
- `Flow::apply(&mut self, input: TransitionInput, at: SystemTime) -> Result<FlowEvent, InvalidTransition>`; der Flow kennt seine ID selbst. Bei ungültigem Übergang bleiben Zustand und Historie unverändert.
- `DecisionSource::System` darf aus `Analyzed` und `Held` nur ablehnen (`Block`, `TimedOut`), nie erlauben. Nur so sind `HoldMemory`, `HoldMaxFlows`, `BodyCap`, `ClientTimeout` ausdrückbar.
- HTTP-Status: zusätzlich `NoRoute` 502, `ClientTimeout` 408.

**Diagnostics (HUM-063).**
- Modul `humanitl_core::diagnostics` mit `codes.rs`; `humanitl_core::diag` ist ein Re-Export. Register umfasst zusätzlich den Bereich `IPC_001..009`. Reserviert und belegt: `SANDBOX_006` Mount verboten, `SANDBOX_007` Bridge-Richtung, `PROXY_007` HTTP/2 nicht verfügbar, `TERM_001` zweiter schreibender Terminal-Client, `CLI_002` Vollbild-TUI mit `--ask terminal`, `CONFIG_004` Runtime-Verzeichnis-Fallback (Info), `CONFIG_005` veralteter Schlüssel in Gebrauch (Info), `CONFIG_006` alter und neuer Schlüssel gleichzeitig gesetzt (Warning).
- `Diagnostic.docs` und `FixAction::OpenUrl` tragen `String`, nicht `Url`; die `url`-Crate bleibt aus dem Kern.
- `sanitize_note` entfernt zusätzlich unsichtbare Zeichen (Zero-Width, Bidi-Overrides, BOM).

**Konfiguration (HUM-062).**
- Aliase werden beim Laden auf Pfaden aufgelöst (`alias::ALIASES`), nicht per `#[serde(alias)]`, weil jeder Alias Gruppen überschreitet. Die Gruppen `ipc`, `preview`, `upstream` existieren im Schema nicht mehr; ihre Felder liegen unter `limits`, die alten Schlüssel bleiben als Aliase gültig. Alter und neuer Schlüssel gleichzeitig in derselben Ebene ⇒ `CONFIG_006`, der kanonische gewinnt. Über Ebenen hinweg gilt die Präzedenz der Ebene: Ein Alias in einer höheren Ebene (z. B. `HUMANITL_HOLD__BODY_CAP_BYTES`) überschreibt den kanonischen Schlüssel einer niedrigeren Ebene, weil beide dasselbe Feld meinen; `CONFIG_006` nennt dann Gewinner und Ebene. Eine unbekannte `HUMANITL_*`-Variable ist ein `CONFIG_002` mit `Severity::Warning`, nie `Error`.
- Unbekannter Schlüssel in Datei oder auf der Kommandozeile ⇒ harter Fehler `CONFIG_002`; unbekannte `HUMANITL_*`-Variable ⇒ nur Diagnostic.
- `Sources { env: Env, .. }` und `discover_with(env, cwd, profile)`: `env.rs` ist die einzige Stelle, die die Prozessumgebung liest; `paths.rs` nutzt denselben Typ.
- Weitere Defaults: `limits.header_timeout_secs` 30, `limits.body_timeout_secs` 300, `limits.recorder_max_body_bytes` 32 MiB, `resolver.cache_ttl_secs` 300, `resolver.prefer` ipv4, `pseudonyms.max_response_bytes` 1 MiB, `pseudonyms.translate_responses` true, `findings.enabled` true, `agent.briefing.enabled` true, `hold.hard_block_checksum_secrets` false.
- Freiform-Tabellen (`resolver.overrides`, `experimental.upstream_port_map`) sind im Merge ein Blatt: eine höhere Ebene ersetzt die ganze Tabelle.
- `llm.endpoint` ist `Option<url::Url>`, im Schema als String.

**Sandbox-Profil (HUM-010).**
- `--cap-drop ALL` wird unbedingt nach `--new-session` ausgegeben, nicht konfigurierbar.
- `argv_line()` liefert nur die Argumente ohne führendes `bwrap`; welcher `bwrap` läuft, entscheidet der Launcher (HUM-011).
- `mounts.work.mode` ist im Profil optional; effektiv gilt der STRENGERE Wert aus Profil und Session (`effective_work_mode`).
- Mount-Verbotsliste: ganze Bäume (`/proc`, `/sys`, `/dev`, `/run`, `/var/run`, `/tmp/.X11-unix`, alles unter `$HOME` außer dem Projektverzeichnis) und Nur-Wholesale-Einträge (`/tmp`, `/var/tmp` als exakter Treffer). Relative Quellen werden abgelehnt. `load_validated(path, home)` ist der Einstieg, den HUM-011 nutzen muss.
- Bridge mit `dir = "out"` wird beim Laden abgelehnt (`SANDBOX_007`), nicht erst beim Start.
- Profil-Fehler nutzen `CONFIG_001` (Datei, TOML, unbekanntes Feld) und `CONFIG_003` (inkonsistente Werte). `min_bwrap_version` bleibt String, Vergleich in HUM-011.

**Proto (HUM-003).**
- Drei Dateien: `common.proto` (Method, Scheme, Upgrade), `rules.proto`, `humanitl.proto`; ein Paket `humanitl.v1`, ein Rust-Modul `humanitl_ipc::v1`.
- Rust-Codegen über `protox` + `tonic-prost-build` nach `OUT_DIR`; `cargo xtask proto` schreibt nur `proto/descriptor.binpb` (committet, deterministisch). Dart-Codegen nur wenn `protoc` und `protoc-gen-dart` vorhanden; Plugin 25.x zu `protobuf` 6.x.
- `FlowEvent` hat die Variante `Failed { flow_id, error, resolved_ip }`. `FlowEvent.Received` ist `{ summary, domain }`. `DecideRequest.block` ist `Block { note }`. `DecideResponse.created_rule` (volle Regel) ersetzt `created_rule_id`.
- Konstanten `PROTO_MAJOR`, `PROTO_MINOR`, `TOKEN_METADATA_KEY` in `humanitl-ipc`.
- `DecideRequest.allow_edited` ist `EditedRequest { method, method_raw, url, headers, body: bytes }` (Feld 7; Feld 3, das alte `HttpRequest`, ist `reserved`): die einzige Stelle, an der ein Body als Inhalt zum Daemon reist, und von `FlowEvent` aus unerreichbar (`proto_contract.rs` prüft das). Eine unlesbare Methode oder URL ist `IPC_004` (bis zum 2026-09-03 `IPC_002`; siehe 4.12), nie stillschweigend `allow`. `FlowDetail.body_preview` zeigt den Anfang des Request-Bodys als verlustbehaftetes UTF-8 mit höchstens 4096 Zeichen (Unicode-Skalare); Ereignisse tragen weiterhin nur `BodyRef`.

**Weitere Entscheidungen aus dem Fix-Durchgang (2026-09-02).**
- Diagnostic-Register, Ergänzungen: `DAEMON_003` Socket bereits belegt, `DAEMON_004` Laufzeitverzeichnis oder Socket nicht anlegbar; `SANDBOX_010..012` sind Starter-Fehler (Argumentliste, Platzhalter, Kommandozeile), `SANDBOX_013..016` die Isolation-Check-Diagnostics (kein Bericht, Check 1 bis 3 fehlgeschlagen; HUM-041). Die Tabelle `AREAS` in `codes.rs` ist die verbindliche Liste der Bereiche, inklusive `IPC`, `PROXY`, `DOCTOR`, `CLI`.
- `socketpair()` bleibt vom seccomp-Filter unberührt: es kennt nur `AF_UNIX`, verbindet zwei Deskriptoren desselben Prozessbaums und ist kein Egress (Node/Bun-Kindprozess-IPC). Der Halbsatz in 3.1 („`socket`/`socketpair` nur für `allow_families`") wird zu „`socket()` nur für `allow_families` × `allow_types`". Belegt wird es von `filter_allows_socketpair` (`seccomp.rs`) und von ESC-1; `probe_families` im Shim probt `socketpair` nicht, also sagt Check 3 von HUM-041 nichts darüber (korrigiert 2026-09-04, HUM-041).
- Prozessmodell ohne `--as-pid-1`: PID 1 in der Sandbox ist das Init von bwrap; der Shim läuft darunter, hält die Brücke ohne Filter, nur sein Kind (der Agent) trägt `Seccomp: 2`. Isolation-Check 3 liest deshalb ein gefiltertes Kind, nie `/proc/1/status`. Bekannter Befund aus ESC-2 (HUM-011 muss ihn schließen): `/proc/1/environ` ist bwraps eigene Umgebung; der Launcher startet bwrap mit gesäuberter Umgebung.
- Escape-Harness: Ergebniszeile `RESULT <suite> <name> <pass|fail|skip|error> <detail>`; `run.sh` endet mit 0/1/2 (1 = Probe durchgekommen, 2 = Sandbox nicht gestartet). Helfer-Exit 127 (Werkzeug fehlt) zählt als skip, nie als pass. ESC-4/ESC-5 sind bis HUM-022 bzw. HUM-043 skip-Platzhalter. Das Profil `test` nennt `/tests/escape` als Platzhalter-Bind, den der Runner ersetzt.
- Versions-Pins liegen in genau einer Datei je Werkzeug: `daemon/rust-toolchain.toml`, `app/.fvmrc`, `scripts/gen-proto.sh` (`PLUGIN_VERSION`, gepaart mit `protobuf` in `app/pubspec.yaml`). CI, Makefile und Doku lesen sie. Dart: `grpc` ^5.1, `protobuf` ^6.0, `protoc_plugin` exakt 25.0.0.
- Fremde GitHub Actions werden per Commit-SHA mit dem Release-Tag als Kommentar referenziert. Ein CI-Job, dessen Inhalt später kommt, darf nur mit `::notice::` und einem `### <job>: skipped`-Block in `$GITHUB_STEP_SUMMARY` grün enden.
- Generator-Skripte enden ohne Werkzeug mit 0 und Installationshinweis; mit `STRICT=1` oder `CI=true` mit 1.
- `DecideResponse`: `created_rule` (volle Regel) ist die einzige Darstellung; `created_rule_id` (Feld 2) wird bei der nächsten Proto-Änderung `reserved`. Bis dahin füllt der Fake beide.
- `DaemonApi` enthält jede RPC des Vertrags, auch `doctor` und `discover_llm`. `humanitl_ipc::BoxStream<T>` ist `Pin<Box<dyn Stream<Item = T> + Send + 'static>>`. `humanitl-ipc` darf von `humanitl-config` abhängen (`GetConfig`/`SetConfig`); `tools/deps-allow.toml` erlaubt es.
- Fake-Modus: `humanitld --fake <session.jsonl> [--speed N] [--loop] [--scale-timeouts] [--hold-timeout-secs N] [--event-buffer N] [--socket PATH]`. `--loop` ersetzt in jeder Flow-Id die 48-Bit-Zeit durch die des Durchlaufs, einmal je Durchlauf. Sitzungsformat: `body_b64` verbindlich, `body` Klartext für Handgeschriebenes; höchstens 2000 Flows, abgeschlossene werden zuerst entfernt. Alle Pfade aus `humanitl_config::Paths`.
- Fehlt eine Fremd-Crate in `[workspace.dependencies]`, trägt der Subagent sie nicht als nackte Version ein, sondern meldet sie im Handoff; der Parent ergänzt `daemon/Cargo.toml` vor dem Commit.
- Sandbox-Profil: `sandbox.unshare` ist die ausgeschriebene Form von `--unshare-all`; fehlt einer der sechs Namensräume ⇒ `CONFIG_003`. `sandbox.new_session` und `sandbox.die_with_parent` dürfen nie `false` sein (`CONFIG_003`). `seccomp.deny_syscalls` darf wachsen, nie unter die Liste aus 3.4 fallen. Die Mount-Verbotsliste verbietet bei Bäumen alles darunter und immer alles darüber (`/`, `/home`, `/var`), zusätzlich `/root`, das aufgelöste `$HOME`, `$XDG_RUNTIME_DIR`, `$XDG_CONFIG_HOME/humanitl`, `$XDG_DATA_HOME/humanitl` und jede Host-Quelle, die ein Unix-Socket ist. `humanitl-sandbox` hat kein JSON-Schema für Profile.
- Profil-Format (HUM-062 vs HUM-066): Konfigurationswerte stehen in Profilen ausschließlich unter `[config.<gruppe>]`, daneben `name`, `description`, `[rules]`, `[agent]`. Die flache Darstellung in sprint-3.md HUM-066 wird darauf angepasst.
- Env und CLI: Werte werden nach dem Typ des Zielfeldes gelesen; Variablen ohne `__` im Namen sind keine Konfigurationsschlüssel (`HUMANITL_GALLERY`, `HUMANITL_ESCAPE_MARKER`). Laufzeitverzeichnis: `$XDG_RUNTIME_DIR/humanitl`, sonst `/run/user/<uid>/humanitl`, sonst `$TMPDIR/humanitl-<uid>` mit `CONFIG_004`.
- `limits.hold_max_flows` Default ist 200 (nicht 500); sprint-5.md HUM-057 wird angepasst. `pseudonyms.max_response_bytes` Default 8 MiB.
- ARB-Schlüssel sind camelCase mit Feature-Präfix (`stateHeld`, `interceptAllowButton`), wie sprint-4.md; `HFlowState.l10nKey` liefert entsprechend. Die drei Garantiesätze heißen `isolationCheck1..3`; der Wortlaut aus sprint-3.md HUM-041 und `docs/SECURITY.md` Abschnitt 1 ist kanonisch. `packages/ui` enthält keinen Nutzer-String; Gallery-Beschriftungen sind englische Literale.
- `app/packages/ui` baut auf `shadcn_flutter`, exakt gepinnt (ADR-0009, Abschnitt „Revidiert am 2026-09-04 durch den Projekteigentümer"; HUM-035 hatte das am selben Tag verworfen, der Projekteigentümer hat es zurückgenommen). Features importieren `lib/core/ui/ui.dart`, nie das Paket; `tools/check-deps.sh` beanstandet jeden `package:shadcn_flutter` außerhalb von `app/packages/ui`. Die Bibliothek bündelt 8,3 MB eigener Assets in jeden Build — 4,4 MB Geist-Schriften in 33 Schnitten, 1,25 MB Icon-Schriften, 3,3 MB Länderflaggen aus `country_flags`; Flutter nimmt Paket-Schriften unabhängig von Importen mit. Wir bündeln darüber hinaus keine eigenen Fonts; `HType` nennt Familien mit Fallback-Stack, Bündelung ist ein eigenes späteres Issue. Light-Zustandsfarben sind abgeleitet (HSL-Lightness −12 %, dann abdunkeln bis Kontrast ≥ 3:1 auf allen hellen Flächen); `allowedEdited` hat die eigene Dark-Farbe `#57B99F`.
- `dart format --set-exit-if-changed app/lib app/test app/packages/ui` wird Teil von `make flutter-analyze`, sobald die Sprint-0-Dart-Dateien einmalig umformatiert sind (eigener Commit `chore(app): dart format`).
- ADR-Dateien heißen `NNNN-kebab-titel.md`, Status `Accepted | Superseded by ADR-NNNN | Deprecated`, zitiert als `ADR-0007`; die dreistellige Form `ADR-007` in BACKLOG.md, ARCHITECTURE.md und hier meint dieselbe Datei. `docs/adr/check.sh` läuft in `scripts/ci/lint-docs.sh`.
- Sicherheitsdokumente bleiben bis HUM-086 deutsch; dann wird die englische Fassung verbindlich.
- Sandbox-Profil, Nachtrag aus dem Review von HUM-010: `SandboxProfile::load_validated(path, &MountPolicy)` ist der Einstieg für HUM-011; die Politik entsteht mit `MountPolicy::from_paths(&humanitl_config::Paths)` (schützt `$HOME`, das ganze `$XDG_RUNTIME_DIR` samt Ersatzverzeichnis, `$XDG_CONFIG_HOME/humanitl`, `$XDG_DATA_HOME/humanitl`), nie aus dem Heimatverzeichnis allein. Jede Bind-Quelle (`ro`, `extra_ro`, `extra_rw`) wird mit `SANDBOX_006` abgelehnt, wenn sie ein Unix-Socket ist oder eine begrenzte Breitensuche (Tiefe 3, 2000 Einträge, Symlinks nicht verfolgt) einen darunter findet; der einzige Socket in der Sandbox ist der vom Launcher eingehängte Proxy-Socket. `die_with_parent`, `new_session`, `hostname = "sandbox"`, `tmpfs ⊇ {/tmp, /dev/shm}` und die Pflicht-Masken `/work/.envrc`, `/work/.git/config` sind nicht abschaltbar; `deny_syscalls` wird mit der Grundliste vereinigt; `unshare` wird in fester Reihenfolge gerendert.
- Konfiguration, Nachtrag aus dem Review von HUM-062 (Codex): Das Projekt-Profil `<projekt>/.humanitl/profile.toml` liegt im geklonten Repository und ist damit Angreifer-beeinflusst. Vertrauensrelevante Schlüssel dürfen deshalb nur aus eingebauten Defaults, globaler Config, globalem Profil, Umgebung oder Kommandozeile kommen, nie aus dem Projekt-Profil: `llm.*`, `sandbox.work_dir`, `sandbox.work_mode`, `sandbox.profile`, `sandbox.env`, `agent.adapter`, `agent.command`, `hold.ask_mode`, `findings.enabled`, `findings.ignored_hashes`, `findings.email_allow_domains`, `pseudonyms.*`, `resolver.*`, `experimental.*`, `recorder.retention_days`. Jedes Schema-Feld trägt dafür `x-project-scope: allowed | denied`; der Loader lehnt ein Projekt-Profil mit gesperrtem Schlüssel mit `CONFIG_003` ab und nennt Schlüssel und Ebene. Zusätzlich: `llm.endpoint` nur mit Schema `http` oder `https`; `sandbox.work_dir` muss absolut sein, wird kanonisiert und muss ein existierendes Verzeichnis sein; `..`-Segmente sind `CONFIG_003`.

### 4.12 Aus der Umsetzung von Sprint 1 (2026-09-03)

Entscheidungen, die beim Bauen fielen und ab jetzt gelten. Wo 3.x oder 4.11 anderes sagt, gilt dieser Abschnitt.

**Shim (HUM-012).**
- Der Shim wird als `humanitl-shim --proxy-port <port> -- <kommando...>` aufgerufen. Brücken (`HUMANITL_BRIDGES`, JSON) und Filtertabelle (`HUMANITL_SECCOMP_FAMILIES`, `HUMANITL_SECCOMP_TYPES`, `HUMANITL_SECCOMP_DENY`) kommen aus der Umgebung, nicht aus der Kommandozeile, damit keine Sicherheitsentscheidung in `/proc/<pid>/cmdline` für jeden Prozess lesbar steht. Der Shim liegt in der Sandbox unter `/usr/local/bin/humanitl-shim`.
- Auch der Elternprozess trägt einen seccomp-Filter (`SandboxSeccomp::for_bridge`): dieselbe Sperrliste wie beim Agenten, nur zusätzlich `AF_UNIX`, weil die Brücke den Proxy-Socket öffnen muss. Das weicht bewusst von 3.11 ab („Brücke ohne Filter"), ist strenger und macht ESC-1 `seccomp_every_process` grün. `docs/SECURITY.md` und `docs/THREAT-MODEL.md` K-04 sind entsprechend nachgezogen.
- `--check` gibt es nicht. Stattdessen schreibt der Shim vor dem `exec` Zeilen der Form `CHECK <name> ok|fail <evidence>` auf den Deskriptor aus `HUMANITL_REPORT_FD`; `--rules` gibt die Filtertabelle für Menschen aus. Exit-Codes: 125 Aufruf falsch, 126 Aufbau fehlgeschlagen, 127 `exec` fehlgeschlagen, sonst der Code des Kindes beziehungsweise `128 + Signal`.
- `libc` und `seccompiler` stehen in `[workspace.dependencies]`; der Shim ist in `tools/check-deps.sh` von der Regel „`TcpStream::connect` nur im Egress-Port" ausgenommen, weil er die Loopback-Brücke in der Sandbox ist und kein Upstream-Ziel kennt.

**Isolation-Check (HUM-011).**
- `BwrapBackend::isolation_check` liest die Prüfzeilen des Shims aus dem `SandboxHandle` der laufenden Sandbox statt eine zweite Sandbox zu starten: `no_interfaces` ergibt `NoNetworkInterface`, `single_socket` zusammen mit `bridge_listening` ergibt `SingleSocket`, `seccomp_applied` zusammen mit `families` ergibt `SeccompActive`. Fehlt der Bericht, ist das `SANDBOX_013`; eine fehlgeschlagene Prüfung wird `SANDBOX_014`, `SANDBOX_015` oder `SANDBOX_016`.
- `CHECK_NAMES` hat damit fünf Namen, und ein Bericht ist erst mit allen fünf vollständig.

**Nachtrag 2026-09-03 (externer Review, vier blockierende Befunde).**
- Garantie 2 ruhte auf `bridge_listening` allein, und das belegt nur, dass die eine Tür offen ist. Der Shim läuft deshalb vor dem `exec` einen begrenzten Suchlauf über das Dateisystem der Sandbox und meldet `CHECK single_socket ok|fail sockets=…;unexpected=…;entries=N;limit=none|entries|depth`: ohne `/proc`, `/sys` und `/dev` (Ausnahme `/dev/shm`, die eine beschreibbare tmpfs darunter), ohne Symlinks zu folgen, Tiefe und Budget wie `SOCKET_WALK_MAX_DEPTH` (3) und `SOCKET_WALK_MAX_ENTRIES` (2000), Verzeichnisse breitenweise und sortiert, damit `/run/humanitl` vor `/usr/bin` kommt. Der Typ eines Eintrags kommt aus `lstat`, nie aus `readdir`: für den überbundenen Proxy-Socket meldet `d_type` „reguläre Datei" (derselbe Fallstrick wie `-xtype` in `tests/escape/lib.sh`). Der Suchlauf ist eine Prüfung mit Budget; der erschöpfende Beweis bleibt ESC-2. `bridge_listening` bleibt eigene Evidenz in derselben Zeile des Befunds.
- Der Socket-Boden wirkt in beide Richtungen: `seccomp.allow_families` ist genau `AF_INET`, `AF_INET6`, `seccomp.allow_types` genau `SOCK_STREAM` (`REQUIRED_SOCKET_FAMILIES`, `REQUIRED_SOCKET_TYPES`). Jede Abweichung, auch eine engere, ist `CONFIG_003` mit Profil, Schlüssel und beanstandetem Wert; nach `parse` stehen die Listen kanonisch da, also kann `bridge_env::shim_env` nichts Weiteres mehr an den Shim reichen. Die einzige Ausnahme heißt `SocketFloor::BrowserUnixIpc` (`AF_UNIX` zusätzlich, für das Profil `browser` aus M7) und ist ein Argument von `SandboxProfile::parse_with_floor`, kein Schlüssel im Profil: eine Datei aus einem geklonten Repository darf die Grenze nicht verschieben können. `SOCK_DGRAM` bleibt auch dort gesperrt.
- `network.bridges` ist genau die Proxy-Bridge (`Bridge::proxy_on(network.proxy_port)`): eine Bridge, Name `proxy`, Richtung `in`, `127.0.0.1:<proxy_port>`, Ziel `/run/humanitl/proxy.sock`, und `network.proxy_socket_dst` derselbe Pfad. Alles andere ist `CONFIG_003` (Richtung `out` weiterhin zuerst `SANDBOX_007`), denn der Shim öffnet jede Bridge, die er bekommt. Der Shim lehnt eine Liste mit mehr als einer Bridge zusätzlich selbst ab (Exit 126).
- `escape-launch` beendet die Sandbox, sobald eine der drei Garantien rot ist oder der Bericht fehlt, und endet mit dem Befund (Exit 3). Ein Escape-Test in einer Sandbox ohne belegte Isolation misst nichts. **Der Befehl läuft dabei schon**: Der Shim `exec`t ihn unmittelbar nach seiner letzten Prüfzeile, geprüft wird auf dem Wirt danach (korrigiert 2026-09-04, HUM-041). Dasselbe gilt für `humanitl sandbox run` und für `SandboxService::start`; keiner der drei Wege verhindert den Befehl. Das Fenster steht in `docs/THREAT-MODEL.md` K-15.
- `SessionContext` trägt zwei weitere Felder: `ca_bundle_src` (das erzeugte Bundle, vom Aufrufer gereicht wie `ca_cert_src`, ohne Abhängigkeit auf `humanitl-proxy`) wird nach `CA_BUNDLE_DST` = `/etc/ssl/certs/ca-certificates.crt` gebunden, nach dem `--ro-bind /etc/ssl` des Profils, sonst verdeckte dieses die Überdeckung; und `session_env` (üblicherweise `humanitl_proxy::ca::env_kit(session)` mit `HUMANITL_SESSION`). Rangfolge der Umgebung: Profil-`[env]` < Sitzung < die fünf Variablen des Shims (`RESERVED_ENV`).

**CA und Zertifikate (HUM-014).**
- `CaStore::open` schreibt `ca-bundle.crt` bei jedem Start neu, damit Aktualisierungen der System-Wurzeln in der Sandbox ankommen. Der Launcher hängt genau diese Datei über `/etc/ssl/certs/ca-certificates.crt`; ohne diesen Bind lehnt jeder TLS-Client in der Sandbox das Leaf des Proxys ab.
- Als System-Bundle gilt der erste Kandidat aus `SYSTEM_BUNDLE_CANDIDATES`, der mindestens ein gültiges Zertifikat enthält, nicht der erste lesbare. Ein abgeschnittenes Bundle wird übersprungen.
- Dateien der CA werden nie über einen Symlink angefasst: `symlink_metadata` beim Prüfen, `create_new` mit `O_NOFOLLOW` und unvorhersagbarem Namen beim Schreiben.
- Scheitert ein einzelnes Leaf, ist das `TLS_005` ohne den Vorschlag, die CA zu löschen; der Vorschlag gehört nur zu einer wirklich unbrauchbaren CA.
- Der PKCS#12-Truststore für die JVM ist aus M1 herausgenommen (siehe `backlog/sprint-1.md`, HUM-014).

**IPC (HUM-018, Nachtrag 2026-09-03).**
- `IPC_004 Decide-Anfrage ungültig` deckt jede unvollständige oder unlesbare `Decide`-Anfrage: keine Flow-Id, keine Entscheidung, unlesbare Flow-Id, unlesbare `EditedRequest`, Body über `limits.hold_body_cap_bytes`. `IPC_002` bleibt allein für `AllowEdited` mit mehr als einem Flow. Der Fake-Daemon meldet dieselben Codes wie der echte, damit die Oberfläche nicht gegen ein Verhalten übt, das der Daemon ablehnt; eine leere Entscheidung ist auch im Fake keine Freigabe.
- `FlowRecord` führt `decision_source`, `response_bytes` und `finished`; `ListFlows` füllt `decision_source`, `response_size` und `duration` daraus und lässt leer, was der Daemon nicht weiß.


**Proxy-Kern (HUM-015, Nachtrag aus dem Review von Antigravity, 2026-09-03).**
- Der Zustandsautomat hat eine weitere Zeile: `Decided(Allow | AllowEdited)` + `Decide { Block, source: System }` ⇒ `Decided(Block)`. Das System darf eine Freigabe vor dem Weiterleiten in eine Sperre verwandeln, nie umgekehrt, nie aus `Forwarded`, und keine andere Quelle darf eine Entscheidung nachträglich ändern. Der Proxy nutzt das, wenn eine bearbeitete Anfrage ein anderes Ziel trägt als das, für das entschieden wurde: Das ist `403 authority_mismatch`, kein erfundener Upstream-Fehler.
- `AllowEdited` darf Methode, Pfad, Kopfzeilen und Body ändern, aber nie die Authority. Entschieden wurde für genau diesen Host und Port; alles andere wäre ein Egress, den kein Mensch freigegeben hat.
- `fail_closed` bringt den Flow aus jedem Zustand auf dem legalen Weg nach `Recorded`: aus `Received` über `Analyze` und `Decide(Block)`, aus `Decided(Allow)` und `Forwarded` über `Fail`. Vorher blieb ein Flow, dessen Übergang scheiterte, für immer in der Registry hängen.

**IPC (HUM-018, Nachtrag aus dem Review von Antigravity, 2026-09-03).**
- Token und Socket werden nie über einen Symlink angefasst: `read_token` und `file_mode` lesen `symlink_metadata` und lehnen alles ab, was keine reguläre Datei ist; `free_socket` erkennt auch einen hängenden Symlink als belegten Eintrag und räumt ihn weg, statt ihn zu übersehen.
- `Subscribe` filtert Durchreich-Flows (LLM-Passthrough) genauso wie `ListFlows`: nur wer `include_passthrough` setzt, sieht sie, im Rückstand wie im laufenden Strom. In M1 trägt noch kein Flow das Merkmal; der Filter steht trotzdem, damit HUM-024 nichts nachrüsten muss.
- `ListFlows.filter` kennt `session:<id>`, `host:<text>` und `state:<name>`.
- Der Test `subscribe_filters_session` aus der Spezifikation entfällt in M1: `SubscribeRequest` trägt keine Session, weil der Daemon genau eine Sitzung hat. Er kommt mit der zweiten Sitzung (Sprint 3) zurück, zusammen mit dem Feld in der Proto.

**CLI (HUM-064).**
- `humanitl sandbox run`, `sandbox argv` und `sandbox check` laufen in M1 im Prozess der CLI gegen die Sandbox-Crate, nicht über den Daemon: Der `Sandbox`-RPC kommt erst in Sprint 3. Das ist eine bewusste, auf genau diese drei Unterkommandos begrenzte Abweichung von ADR-018; die Naht ist in `daemon/bin/humanitl/src/cmd/sandbox.rs` markiert, und mit dem RPC wandert die Logik hinter ihn, ohne dass sich die Kommandozeile ändert. `daemon status`, `flows list|show|decide` und `config get|schema` gehen schon jetzt über den gRPC-Client.
- `flows decide ID allow|block [--note TEXT]` bleibt im Produkt: Es ist der Vorläufer von `--ask terminal` (HUM-067) und das Werkzeug, mit dem das Demoskript und die Escape-Tests entscheiden.

**Oberfläche, Nachtrag 2026-09-03.**
- `shadcn_flutter` war bis zum 2026-09-04 nie in einer Pubspec-Datei, obwohl ADR-0009 es von Anfang an als gesetzt beschrieb. HUM-035 hat auf diesem tatsächlichen Stand entschieden, es nicht aufzunehmen (88,3 % der gewichteten Punkte für die eigene Schicht gegen 48,3 % und 51,7 %, ohne den vorgesehenen Prototyp-Branch, siehe 4.20). Der Projekteigentümer hat das am selben Tag zurückgenommen; seither steht die Bibliothek exakt gepinnt in `app/packages/ui/pubspec.yaml` (ADR-0009, Abschnitt „Revidiert am 2026-09-04 durch den Projekteigentümer"). Der Wrapper bleibt Pflicht: ein Feature importiert `lib/core/ui/ui.dart`, nie ein fremdes Widget-Paket, und `tools/check-deps.sh` erzwingt das.
- Bewegung erklärt oder entfällt. Ein geteilter Übergang (`Hero`) wird nicht erzwungen: Er gehört an die zwei Stellen, an denen der Blick sonst springt, nämlich von der Karte in der Warteschlange in die Detailansicht und von einem entschiedenen Fluss in die History. Überall sonst genügen die Tokens aus `HMotion`. Eine Animation, die keine Frage des Nutzers beantwortet, ist ein Fehler, kein Schmuck.

### 4.13 Vertrauen als Gestaltungsauftrag (2026-09-03)

Humanitl entscheidet mit dem Nutzer darüber, was sein Rechner verlässt. Wer so
etwas benutzt, muss dem Werkzeug glauben können, und zwar ab der ersten Minute
der ersten Fassung. Ein Werkzeug wirkt nicht seriös, weil es das behauptet,
sondern weil jede Kleinigkeit dieselbe Sorgfalt zeigt. Die folgenden Punkte
sind deshalb Akzeptanzkriterien jedes UI-Issues, nicht Geschmack.

**Nie mehr behaupten als bewiesen ist.** Jede grüne Aussage der Oberfläche
zeigt auf einen Beleg, den man anschauen kann; die drei Isolationsprüfungen
tun das bereits mit ihrer `evidence`-Zeile. Was der Daemon nicht weiß, steht
als unbekannt da, nie als grün und nie als Strich, hinter dem man Grün vermuten
könnte. Eine Zahl, die geschätzt ist, wird als geschätzt gekennzeichnet.

**Genauigkeit, wo sie zählt.** Zeitpunkte, Größen und Zähler stehen exakt, mit
Einheit; „vor ein paar Sekunden" ist erlaubt, wo die Zeit nur Kontext ist, und
verboten, wo sie Teil des Belegs ist. Bytes und Hashes stehen in Monospace,
damit man sie vergleichen kann.

**Zurückhaltung.** Farbe bedeutet Zustand, sonst nichts. Kein Emoji, kein
Ausrufezeichen, keine Werbesprache im Produkt. Kein Verlauf, kein Schlagschatten
als Dekor, keine Animation ohne Aussage (4.12). Der Rahmen tritt zurück, der
Inhalt trägt.

**Vorhersagbarkeit.** Dieselbe Handlung liegt immer an derselben Stelle. Nichts
verschiebt sich unter dem Zeiger. Ein Bildschirm, der lädt, behält sein Gerüst,
statt zu springen. Was der Nutzer gerade liest, wird nicht animiert.

**Keine dunklen Muster, in beide Richtungen.** Freigeben darf nie leichter aus
Versehen passieren als Blocken. Eine Entscheidung, die man nicht zurücknehmen
kann, sagt das vorher. Voreinstellungen stehen auf der sicheren Seite, und wo
eine Einstellung Sicherheit kostet, sagt der Text das in einem Satz.

**Fehler sind Teil der Oberfläche.** Jeder nicht-grüne Zustand nennt Grund und
Abhilfe (`Diagnostic` mit `why` und `fix`), in der Sprache des Nutzers, nie nur
einen Code. Ein leerer Bildschirm erklärt, was als Nächstes passieren wird.

**Handwerk sichtbar machen.** Grundlinien liegen auf einem Raster, Radien und
Haarlinien sind überall gleich, Symbole haben eine optische Größe statt einer
gemessenen. Ein Bildschirm, der ruckelt, wirkt unfertig: die Budgets aus
`docs/UX.md` sind deshalb Teil dieser Aussage und nicht nur Technik.

Die Prüffrage vor jedem „fertig": Würde ein Sicherheitsverantwortlicher, der
dieses Fenster zum ersten Mal sieht, ihm die Entscheidung über seinen
Netzverkehr anvertrauen? Wenn eine Stelle daran zweifeln lässt, ist sie der
nächste Arbeitsschritt.

**Testdaten der Findings-Detektoren (2026-09-03).** Ein Detektor für Geheimnisse
lässt sich nur an echt geformten Werten prüfen, und genau diese Form blockiert
der Push-Schutz von GitHub im Quelltext. Solche Werte werden deshalb zur
Laufzeit aus zwei Teilen zusammengesetzt (`"ghp" + "_0123…"`), mit einem
Kommentar an der Stelle. Ein Wert nachträglich freizuschalten wäre der falsche
Weg: Der Schutz soll anschlagen, wenn wirklich einmal ein Schlüssel in einen
Commit rutscht.

### 4.14 Aus der Umsetzung des Recorders (HUM-026, 2026-09-03)

Abweichungen von `backlog/sprint-2.md`, die dauerhaft gelten. Wo die
Spezifikation anderes sagt, gilt dieser Abschnitt.

**Migrationen ohne `refinery`.** Die Spezifikation nennt `refinery`; die Crate
steht nicht in `[workspace.dependencies]`, und ein Subagent trägt keine nackte
Version ein (4.11). `daemon/crates/recorder/src/schema.rs` führt den Stand
stattdessen in `PRAGMA user_version` und wendet die Dateien aus `migrations/`
der Reihe nach an, jede in einer Transaktion zusammen mit dem Fortschreiben des
Standes. Verzeichnis und Dateinamen sind genau die, die `refinery` erwartet
(`V<n>__<name>.sql`), damit ein späterer Wechsel dieses eine Modul betrifft und
nicht das Schema. Eine Datenbank, deren `user_version` höher ist als das Binary
kennt, wird abgelehnt (`RECORDER_001`), nie halb gelesen.

**`V1__init.sql` bleibt wortgleich, Änderungen kommen als neue Migration.** Das
Akzeptanzkriterium verlangt, dass die Schema-Datei exakt dem `SQL` der
Spezifikation entspricht. Was danach nötig wurde, steht deshalb in
`V3__host_suffix.sql`: die Spalte `flows.host_rev` samt Index (der Filter
`host:` ist ein Suffix-Vergleich, und den kann kein Index beantworten; umgekehrt
geschriebene Labels machen daraus einen Bereich), `flows_ts` neu als
`(ts DESC, id DESC)` statt `(ts DESC, id)` (sonst legt `SQLite` für die zweite
Spalte einen temporären B-Baum an) und drei Indizes für die Sortierung nach
Host, Dauer und Größe. Bestandszeilen bekommen ihren `host_rev` beim nächsten
Start nachgetragen (`schema::backfill_host_rev`), weil eine Migration die
Umkehrung der Labels nicht ausdrücken kann; ohne das Nachtragen verschwänden
ältere Flows aus jedem `host:`-Filter.

**`state = 'failed'`.** Der Spaltenkommentar in `V1__init.sql` zählt sieben
Zustände auf und stammt wortgleich aus der Spezifikation. `FlowState::Failed`
kam mit 4.10 dazu und wird geschrieben wie jeder andere Zustand; `failed` ist
damit ein gültiger Wert der Spalte und ein gültiger Wert für den Filter
`state:`. Der Kommentar bleibt, wie er ist, die Aufzählung steht im
Doc-Kommentar von `FlowSummary::state`.

**`Cursor` trägt drei Felder.** Die Skizze der Spezifikation hat `{ ts, id }`.
Damit lässt sich nur nach Zeit blättern; nach Host, Dauer oder Größe sortiert
wären die Seiten lückenhaft. `Cursor` hat deshalb zusätzlich
`sort: Option<CursorKey>`, das bei `SortKey::Ts` leer bleibt. Ein Cursor, der
nicht zur Sortierung passt, wird abgelehnt (`RECORDER_002`), statt still falsche
Seiten zu liefern.

**Grenzen kommen als eigener Typ, nicht als `RecorderConfig`.** `humanitl-recorder`
darf nur von `humanitl-core` abhängen (3.1). Die Signatur `Recorder::open(db,
blobs, &RecorderConfig)` aus der Spezifikation ist deshalb
`Recorder::open(db, blobs, RecorderSettings)`; der Daemon rechnet
`recorder.inline_max_bytes`, `limits.recorder_max_body_bytes` und
`recorder.retention_days` beim Start um. Aus demselben Grund nimmt
`snapshot_rule` die Regel als `YAML`-Text und nicht als `Rule`, und Apex sowie
Katalog-Kennung kommen über `set_domain` von außen.

**Jeder Weg, der Bytes annimmt, gibt ein `Result` zurück.** `store_message` und
`ResponseSink::finish` liefern in der Skizze `BodyRef` beziehungsweise nichts.
Beide können am Blob-Speicher scheitern, und ein verlorener Body ist genau die
Art stiller Lücke, die 4.13 verbietet; sie liefern deshalb
`Result<BodyRef, RecorderError>`. Ein `ResponseSink`, den niemand abschließt,
schreibt beim Fallenlassen selbst, was bis dahin durchlief, mit `truncated = 1`.

**Gezählte Zahlen sagen, wenn sie geschätzt sind.** `FlowPage::total_estimate`
zählt höchstens bis 10 000. Ob die Zahl exakt ist, steht in
`FlowPage::capped`; `FlowPage::total_text` schreibt `10000+`, wo sie nur eine
Untergrenze ist (4.13).

**`ANALYZE` gehört zum Aufräumen.** Das gebündelte `SQLite` ist mit
`SQLITE_ENABLE_STAT4` übersetzt. Erst mit Stichproben unterscheidet der Planer
beim Filter `host:` den häufigen Host vom seltenen; ohne sie ist einer der
beiden Fälle um ein Vielfaches langsamer. Jeder Aufbewahrungslauf
(`Recorder::purge_expired`) erhebt die Statistiken deshalb neu.

**Proxy, Nachtrag aus HUM-023 (2026-09-03).**
- Das Schema der Anfrage muss zum Schema der Verbindung passen: im CONNECT-Tunnel `https`, ohne Tunnel `http`. Beides andere ist `403 authority_mismatch`. Ohne diese Prüfung konnte ein Client im Tunnel eine Anfrage mit `http://` schicken, und der Proxy hätte sie im Klartext weitergeleitet, obwohl der Mensch über eine TLS-Verbindung entschieden hat.
- Genau ein Fall bleibt `400` statt `403`: Origin-Form ohne Tunnel und ohne `Host`. Dort ist kein Ziel bekannt; ein Flow bräuchte eine erfundene Authority, und der Block-Body müsste `host: unknown` behaupten. Weitergeleitet wird auch dann nichts.
- `AllowEdited` darf weder Authority noch Schema ändern (4.12 nannte bisher nur die Authority).
- `BlockReason::Secret` (kurz `secret`, HTTP 403) gehört zu `hold.hard_block_checksum_secrets`. Vorher stand dort `user`, und eine Antwort, die einen Menschen nennt, den es nicht gab, ist eine Unwahrheit gegenüber dem Agenten und dem Protokoll (4.13).
- Der Flow-Datensatz trägt `findings_truncated`. Ein nur teilweise durchsuchter Body darf in der Oberfläche nie wie eine leere Fundliste aussehen; zusätzlich steht `FINDINGS_002` als Diagnostic im Ereignisstrom.

### 4.15 Aus der Umsetzung der Warteschlange (HUM-029, HUM-072, 2026-09-04)

Abweichungen von `backlog/sprint-2.md`, die dauerhaft gelten. Wo die
Spezifikation anderes sagt, gilt dieser Abschnitt.

**Gruppiert wird nach registrierbarer Domain, nicht nach Host.** `HeldGroup`
trägt deshalb `apex`, `display`, `hosts` und `rows` statt `host` und
`hostDisplay`: ein Schwall verteilt sich auf `registry.npmjs.org` und
`codeload.github.com`, und eine Gruppe je Host läse ihn wieder als zwölf
Dinge. `display` ist der eine Host der Gruppe und sonst leer; der Kopf
schreibt dann „Host und n weitere". Eine Domäne steht nur dort, wo der Daemon
sie geliefert hat.

**Die Tabelle in `psl.dart` ist ein Rat und wird nie zu einer Regel.** Sie
gruppiert die Ansicht, mehr nicht. Was eine Regel trifft, kommt aus
`selectedApexProvider`, also aus dem Katalog des Daemons; ohne dessen Antwort
ist der Domain-Scope ausgegraut und nennt beim Anklicken den Grund. Restrisiko
der Gruppierung: außerhalb der Tabelle kann der Rat ein Public Suffix als
Domäne nehmen (`a.foo.com.pl` und `b.evil.com.pl` fallen beide auf `com.pl`),
also zwei fremde Registranten in eine Gruppe legen. Deshalb fragt jede
Entscheidung, die mehrere Hosts umfasst, immer erst im Modal, das die Hosts
auflistet — unabhängig davon, wie wenige Anfragen sie umfasst. Mit dem Katalog
aus HUM-031 entfällt der Rat und mit ihm diese Sonderregel.

**Der Kopf zählt nur gehaltene Anfragen.** Eine entschiedene Zeile ruht drei
Sekunden an ihrem Platz (`docs/UX.md` 2.8), gehört aber zu keiner Entscheidung
mehr: `HeldGroup.flows` sind die gehaltenen, `HeldGroup.rows` alle
gezeichneten. Zählte der Kopf die ruhenden mit, wiese `Block {n}` die ganze
Gruppe ab, weil eine davon nicht mehr entscheidbar ist.

**Erlauben einer Gruppe geht nur über die Aktionsleiste.** Der Kopf zeigt im
28-px-Slot nur das Blockieren, und dort steht das Schild-Glyph mit der Zahl im
Semantics-Label; die Ziffer selbst steht im Zähler-Chip derselben Zeile.
`Ctrl+Shift+F` wählt die Gruppe zuerst aus und entscheidet erst beim zweiten
Druck, damit die Karte daneben zeigt, worüber entschieden wird.

**Die eingefrorene Reihenfolge lebt im `State` des Panes.** Provider ist nur
`pendingArrivalsProvider`, und der trägt die `FlowId` der wartenden
Ankünfte, nicht bloß ihre Zahl: eine Anfrage, die noch nie auf dem Schirm
stand, gehört in keine Reichweite einer Taste (`docs/UX.md` 2.8).

**`blockNoteProvider` hat keinen `FlowId`-Parameter.** Die Notiz setzt sich
mit jeder neuen Auswahl zurück, was strenger ist als eine Familie je Flow. Sie
reist mit jeder Entscheidung der Aktionsleiste mit, auch mit einem Stapel, und
das Label sagt es („Block 3 selected with note"); aus einer Zeile heraus reist
sie nie mit, weil dort die Gruppe unter dem Zeiger entschieden wird und nicht
die ausgewählte Anfrage.

**Ein Halten behält seine Zeit, auch wenn Animationen aus sind.** Jeder
`AnimationController`, dessen Dauer eine Sicherung ist, läuft mit
`AnimationBehavior.preserve`; sonst skaliert Flutter 400 ms auf 20 ms, sobald
die Plattform `disableAnimations` meldet, und ein gewöhnlicher Klick wäre eine
Bestätigung.

**`Edit + Allow` fehlt bis HUM-047.** Das Control konnte nie gedrückt werden;
ein toter Zustand ohne Grund ist schlimmer als ein fehlender (4.13). Der Body
steht bis dahin read-only in der Karte. Aus demselben Grund fehlen der
Katalogname („Looks like: npm install", `intercept_group_looks_like`) und
`CatalogEntry`, bis HUM-031 den Katalog liefert.

### 4.16 Aus der Umsetzung des Regel-Bildschirms (HUM-033, 2026-09-04)

Abweichungen von `backlog/sprint-2.md`, die dauerhaft gelten. Wo die
Spezifikation anderes sagt, gilt dieser Abschnitt.

**Die Herkunft einer Regel führt dorthin, wo die Anfrage steht, nicht in ein
eigenes History-Blatt.** Die Spezifikation schreibt „Klick auf `from #41`
öffnet das History-Detail-Blatt des Flows". Ein Feature greift nicht in ein
anderes (ARCHITECTURE 5), und für genau diese Übergabe gibt es bereits einen
Weg: `flowHandoffProvider` in `app/lib/core/ipc/flow_handoff.dart`. Das
Abzeichen bittet, die Shell führt aus. Einen zweiten Mechanismus daneben zu
bauen, wäre teurer als die Abweichung. Restrisiko, das damit bewusst
angenommen wird: die Shell wählt den Flow aus und wechselt in die
Warteschlange; kennt der Intercept-Screen den Flow nicht mehr, steht dort
keine Karte. Wer die Übergabe später auf die History erweitert, ändert die
Shell, nicht dieses Abzeichen — es bittet nur.

**Die Stream-Checkbox bleibt aus, bis es einen Tier-Begriff gibt.** Die
Spezifikation zeigt das Feld „nur bei Expert-Tier sichtbar". In `app/lib`
existiert kein Tier; er kommt mit HUM-069. Bis dahin steht das Control nicht
im Formular: es ist das eine Control dieses Bildschirms, das einen Rumpf über
der Kappe ungelesen hinauslässt, also die eine Weitung des offenen Datenpfads
(BACKLOG.md 4.2). Ein sichtbares Feld ohne den Schutz wäre die falsche
Zwischenlösung, weil es die Weitung leichter macht als die Einengung (4.13).
Das Feld der Regel selbst bleibt unangetastet: der Entwurf trägt weiter, was
der Daemon geliefert hat, und speichert es unverändert zurück. Die
ARB-Schlüssel `rulesFieldStream`, `rulesStream` und `rulesStreamHelp` bleiben
stehen, damit HUM-069 nur das Control und seine Tier-Prüfung nachzuliefern
hat.

**Ein Regelsatz, ein Generator.** `app/lib/core/text/rule_sentence.dart` ist
die einzige Stelle, die aus einer `Rule` einen Satz macht; die Aktionsleiste
(HUM-028) baut erst die Regel und liest dann denselben Generator. Vorher
standen zwei Generatoren nebeneinander, und dieselbe Sitzungsregel las sich
vor dem Anlegen „this session" und danach „until the session ends". Daraus
folgt dreierlei: `rulesExpirySession` heißt jetzt „this session" / „diese
Session"; die Schlüssel `interceptSentence*` sind entfallen; und der Satz
einer `url`-Regel nennt Schema und Port, weil ihr Matcher beides festnagelt.
Eine Stundenregel liest sich als Restlaufzeit („expires in 60 min"), vor dem
Anlegen wie danach.

**Rückgängig nimmt genau das zurück, was der Knopf getan hat.** „Dauerhaft
machen" ändert die Frist, also schreibt „Undo" die Frist zurück und nicht den
ganzen Zustand von vorhin (`Rules.restoreExpiry`). Und jedes Angebot verfällt,
sobald dieselbe Regel danach angelegt, geändert oder gelöscht wird, ebenso beim
Reload der Datei: der Streifen spräche sonst über einen Zustand, den es nicht
mehr gibt, und aus einer inzwischen engeren Regel machte ein Knopf mit der
Aufschrift „Undo" wieder die weitere. Ein Rückgängig, dessen Reichweite jemand
falsch rät, ist schlimmer als keines (`docs/UX.md` 4.5).

**Eine Zählweise je Bildschirm.** Die Tab-Beschriftungen und die
Ketten-Hinweise zählen dasselbe: was der Tab hält. Zwei Zahlen über dieselbe
Menge im selben Bild sind ein Fehler, auch wenn beide für sich stimmen. Der
Hinweis, der eine Auswertung behauptet (`rulesChainSessionFirst`), bleibt
trotzdem wahr, weil nur Sitzungsregeln temporär sind und eine Sitzungsregel nie
abläuft.

**Der Schalter mitgelieferter Regeln (HUM-105, 2026-09-05).** Eine
mitgelieferte Regel trägt an der Stelle, an der eine eigene ihren Papierkorb
hat, den Schalter „Ausschalten" beziehungsweise „Einschalten"; welche der
beiden Handlungen eine Zeile anbietet, entscheidet der Aufrufer, indem er genau
einen der beiden Rückrufe reicht. Der Slot fasst kein beschriftetes Control:
`HSize.rowActionSlot` ist 28 Pixel breit, und ein `HButton` der Größe `sm`
verbraucht davon 20 allein für seine waagerechte Polsterung. Das Wort steht
deshalb als Hover-Label und als Semantik am Knopf, im Slot steht `HGlyph.bolt`
— der Blitz heißt in diesem Vokabular „eine Regel hat entschieden", und genau
das schaltet der Knopf an und aus.

**Eine abgeschaltete Regel wird über drei Kanäle als abgeschaltet erkennbar,
und Farbe ist nur einer davon.** Die Zeile wird gedämpft wie eine abgelaufene
(`HFlowState.timedOut`); ihr Zustands-Glyph ist das Kreuz `HGlyph.close` statt
des Handlungs-Glyphs, in jeder Breite; und das Herkunftswort lautet
„mitgeliefert, ausgeschaltet", unterhalb von `ruleRowOriginBelow` gekürzt auf
das eine Wort, das den Zustand trägt. Die Kürzung ist keine Sparsamkeit,
sondern gemessen: das volle Wort verbrauchte in der üblichen Panebreite mehr
Platz als der Regelsatz selbst, und der Satz ist die Regel (`docs/UX.md` 3.4).
Das Kreuz ist bewusst nicht die Uhr: abgelaufen und ausgeschaltet sind zwei
Dinge — eine mitgelieferte Regel läuft nie ab, eine eigene wird nie
abgeschaltet —, sie teilen die Dämpfung und sonst nichts.

**Der Aufruf gehört dem Widget, das seine Antwort noch erlebt.** Der
Aktionsslot einer Zeile wird nur bei Hover und Fokus gebaut. Läge der laufende
Aufruf im Slot, nähme ein Zeiger, der die Zeile verlässt, den Befund einer
Ablehnung mit ins Nichts: der Mensch klickte, sähe nichts geschehen und
erführe nicht, dass der Daemon abgelehnt hat. Aufruf, laufender Zustand und
Befund liegen deshalb in der Liste, wie beim Löschen seit HUM-033. Die Merkung
„unterwegs" wird in einem `finally` gelöst: bricht der Aufruf mit etwas ab, das
kein `Diagnostic` ist, bliebe der Schalter sonst für immer tot, und ein Knopf,
den nur ein Neustart wiederbelebt, ist schlimmer als ein sichtbarer Fehler. Ein
solcher Wurf ist nach dem Vertrag des Ports ein Programmfehler und bekommt
keinen erfundenen Code; er geht an `FlutterError.reportError`, statt in einem
Future zu verschwinden, das niemand beobachtet. Der Zustand selbst kommt
ausschließlich aus der Antwort: der Schalter ist deaktiviert, solange die
Anfrage unterwegs ist, und nichts nimmt vorweg, was der Daemon noch nicht
bestätigt hat (4.13).

**Was die Zeile sagt, sagt der Editor auch.** Ein Klick auf eine Zeile öffnet
die Regel im Editor, und der nimmt mehr Fläche ein als die Zeile. Ein
Zustand, den nur die Zeile trägt, wird von der größeren Hälfte des Bildschirms
daneben stillschweigend bestritten: `_BundledNotice` trägt für eine
ausgeschaltete Regel dieselben drei Kanäle wie die Zeile — Kreuz statt
Schloss, die Farbe einer Regel, die nichts entscheidet, und das Wort als
Feststellung. Ein Satz, der das Abschalten anbietet, ist keine Auskunft
darüber, ob schon abgeschaltet wurde.

**Das Zeichen für „ausgeschaltet" ist ein eigenes und fällt mit keinem
Aktions-Zeichen zusammen.** Es ist das Kreuz `HGlyph.close`; `allow` trägt
`arrowUpRight`, `block` `shieldX`, `ask` `hourglass`, `redact` `redactBar`, und
eine abgelaufene Regel die Uhr `clockX`. Diese Zusicherung hängt nicht an der
Regel, die eine Vorrichtung gerade anlegt: sie wird über alle Aktionen geprüft
(`rules_a11y_test.dart`, „the switched-off glyph is no action glyph"), damit
der zweite Kanal nicht eines Tages still verlorengeht, weil jemand ein
Aktions-Zeichen auf dieselbe Form legt. Treffen „ausgeschaltet" und
„abgelaufen" je zusammen, gewinnt das Ausschalten: das ist die Entscheidung
eines Menschen, das Ablaufen das Ausbleiben einer.

**Eine Zeile bietet den Papierkorb oder den Schalter, nie beides.** Welche der
beiden Handlungen es gibt, sagt der Aufrufer; damit die Trennung nicht an der
Reihenfolge zweier Zweige hängt, hält ein `assert` im Konstruktor von
`RuleRow` sie fest. Der Daemon lehnt beide Handlungen an der falschen Regel
mit `RULES_010` ab, und eine Oberfläche, die eine davon anbietet, verspräche
etwas Unmögliches.

### 4.17 Aus der Umsetzung des Agent-Adapters (HUM-037, HUM-038, 2026-09-04)

Abweichungen von `backlog/sprint-3.md`, die dauerhaft gelten. Alle sind an der
installierten Fassung OpenCode 1.18.25 gemessen, nicht aus der Dokumentation
übernommen.

**Der Modellkatalog kommt aus einer Datei, nicht aus einer Adresse.**
`OPENCODE_MODELS_URL` ist eine Basis-Adresse, an die OpenCode `/api.json`
anhängt und die es über seinen HTTP-Client abruft; ein `file://`-Schema kommt
dort nicht an. Der Adapter setzt stattdessen
`OPENCODE_MODELS_PATH=/etc/humanitl/opencode/models.json` und
`OPENCODE_DISABLE_MODELS_FETCH=true`, das auch die stündliche Aktualisierung
abschaltet. Damit entfällt Fallstrick 1 aus HUM-037: die zweite Bridge auf Port
3129 wird nicht gebraucht, und die Sandbox behält ihre eine Tür.

**Zehn mitgelieferte Regeln statt acht.** Zwei kamen aus dem Binary dazu: der
Katalog liegt auf `models.opencode.ai` und nicht auf `models.dev`
(`…0009`), und das Teilen einer Sitzung geht an `opncd.ai`
(`POST /api/share`, dann `/api/share/<id>/sync`), nicht an `opencode.ai/share`.
Die Regel `…0005` trägt deshalb `opncd.ai`, und `opencode.ai` mit
`/share/**` steht als `…000a` daneben.

**Drei Orte für dieselbe Konfiguration.** OpenCode mergt seine Quellen so,
spätere gewinnen: Konfigurationsverzeichnis, `OPENCODE_CONFIG`, dann die
`opencode.json` des Projektbaums, `.opencode`-Verzeichnisse,
`OPENCODE_CONFIG_CONTENT`, Organisationskonfiguration, zuletzt das verwaltete
Verzeichnis (Linux `/etc/opencode`); danach `OPENCODE_PERMISSION` über den
Block `permission`. Ein geklontes Repository steht damit über
`OPENCODE_CONFIG`. Der Adapter legt seine Datei deshalb nach
`/etc/humanitl/opencode/opencode.json`, `$HOME/.config/opencode/opencode.json`
und `/etc/opencode/opencode.json` und setzt zusätzlich `OPENCODE_PERMISSION`.
`provider` bleibt additiv: ein Projekt kann einen Provider hinzufügen, dessen
Verkehr gehalten wird.

**Adapter-Dateien kommen als `--ro-bind-data`, nicht als `--file`.** Der
Mechanismus ist derselbe wie bei den Masken und Identitätsdateien: ein
versiegeltes memfd je Datei, ohne `CLOEXEC` an `bwrap` vererbt, gerendert nach
den Masken und vor `--clearenv`. `SandboxFile::mode` dokumentiert nur die
Absicht; den Modus bestimmt `bwrap` (gemessen 0600, nur lesbar eingehängt).
Kein Ziel darf unter `/work` liegen oder eines der Ziele belegen, die die
Sandbox selbst setzt (Proxy-Socket, CA, Bündel, Shim, `/etc/passwd`,
`/etc/group`, `/etc/hosts`, `/proc`, `/sys`, `/dev`, `/run/humanitl`); beides
ist `SANDBOX_006`.

**Die Sandbox wird aus drei Quellen gefüllt**, und der Kopf von
`profiles/sandbox/default.toml` sagt das: die Profildatei, der Beitrag des
Agent-Adapters (Umgebung und Dateien) und `sandbox.env`. Ein Host-Pfad kommt
weiterhin nur über die Profildatei herein.

**Mitgelieferte Regeln schaltet man ab, statt sie zu löschen.**
`Rule.disabled` liegt in `humanitl-core`, die Liste `disabled_bundled` in der
`rules.yaml` des Nutzers. Sie steht am `RuleSet`, nicht an der Regel, weil die
Nutzerdatei gelesen wird, bevor `RuleSet::add_bundled` die mitgelieferten
Regeln dazunimmt; eine Id darin darf eine Regel benennen, die es in dieser
Fassung nicht gibt. `RulesStore::set_bundled_disabled` ist der einzige Weg,
`RulesRequest.set_disabled` die RPC, `humanitl rules disable|enable ID` die
Kommandozeile. Eine eigene Regel abzuschalten ist `RULES_010`: die löscht man.

**Der Adapter bestimmt das Kommando.** `AgentAdapter::command` liefert
`agent.command`, sonst das eigene Standardkommando; die Shell bleibt nur, wo
gar kein Adapter beiträgt. Findet die Vorprüfung das Programm auf dem Host,
aber außerhalb der nur lesbaren Einhängungen des Profils, ist das `AGENT_004`:
in der Sandbox gäbe es das Programm nicht, und das `exec` scheiterte erst nach
dem Start.

**Neue Diagnose-Codes.** Bereich `agent` (`AGENT_001` bis `AGENT_009`):
`AGENT_001` Kommando nicht gefunden, `AGENT_002` nicht ausführbar, `AGENT_003`
gebündelte Vorlage unbrauchbar, `AGENT_004` in der Sandbox nicht erreichbar.
Dazu `LLM_004` für „kein Modell konfiguriert".

### 4.18 Aus der Umsetzung des History-Bildschirms (HUM-032, 2026-09-04)

Abweichungen von `backlog/sprint-2.md`, die dauerhaft gelten. Wo die
Spezifikation anderes sagt, gilt dieser Abschnitt.

**Keine `seq`-Spalte.** Die Spezifikation nennt sie als erste Spalte der
Tabelle. Der Recorder führt `flows.seq` (laufende Nummer je Sitzung), die
Proto-`FlowSummary` trägt sie aber nicht, also hat die Oberfläche nichts
anzuzeigen. Eine über die geladene Seite gezählte Nummer wäre keine: sie
änderte sich mit jedem Filter und behauptete eine Reihenfolge, die der Daemon
nie vergeben hat. Wer die Spalte will, nimmt das Feld in die Proto auf.

**Die Zustandsspalte gehört `HRow`.** Sie steht nicht in der Spaltenliste des
Features, weil `HRow` einen Slot für das Zustands-Glyph hat und ihn links von
allem anderen zeichnet. Kopf und Zeile rechnen deshalb dieselbe Einrückung aus
`HSize.stateRail`, `HSpace.x2` und `HSize.rowHistory`; ändert sich das Layout
der Zeile, folgt der Kopf an genau einer Stelle.

**Zustandsfarbe: ein Flow sieht auf beiden Bildschirmen gleich aus.** Die
Ableitungstabelle der Spezifikation bildet jede Blockierung auf `blocked` ab.
Eine Blockierung durch eine Regel ist in der Warteschlange `autoRule`, und zwei
Bildschirme, die einen Flow verschieden einfärben, kosten mehr, als die Tabelle
gewinnt. `historyVisualState` prüft deshalb der Reihe nach: gehalten,
Durchreichen, Zeitablauf, `no_route`, Blockierung, dann Fehler, dann die
gemeinsame Ableitung. Eine Entscheidung wird nie von dem Status überstimmt, den
sie selbst erzeugt hat: die 504 eines Timeouts ist keine Upstream-Störung.

**Text nie in der Flächenfarbe.** `HStateColors` ist auf 3:1 geklemmt und damit
Flächenfarbe; als Text misst sie im hellen Thema 3,17:1 bis 3,75:1. Jede
Textfarbe dieses Bildschirms kommt deshalb aus `tokens.stateTextColor` bzw.
`tokens.colors.accentText`, auch die, die `HDiagnosticCard` an `HBadge`
weiterreicht. Der Kontrasttest zählt jede Farbe auf, die der Bildschirm
wirklich auf einen `TextStyle` setzt, nicht nur `fg0` und `fg1`.

**`HistoryChip.blocked` filtert die Entscheidung, nicht den Zustand.** Der Chip
setzt `decision:block` und nicht `state:blocked`; `state:` kennt die sieben
Zustände des Automaten, und `blocked` ist keiner davon (3.2). Die Chips
schreiben ihren Term sichtbar ins Feld, damit die Grammatik dabei gelernt wird.

**`FlowPage.capped` kommt vom Draht.** Die Oberfläche schließt nicht mehr aus
`total == 10000` auf eine Untergrenze, seit die Proto das Feld trägt (4.14).
Der Fake füllt es genauso.

**HAR: `timings.wait` ist 0.** Das Mapping der Spezifikation setzt
`wait = held_ms`. Die Proto-`FlowSummary` trägt keine Haltezeit, also wird die
ganze Dauer auf `receive` gebucht. Eine geratene Aufteilung stünde als Zahl in
einer Datei, die als Beleg weitergegeben wird (4.13). `content.text` fehlt, wo
keine Bytes aufgezeichnet wurden; `content.comment` sagt das, denn eine leere
Zeichenkette neben `size: 900` läse sich als leere Antwort.

**Kein Schema-Test gegen `har-1.2.json`.** Die Spezifikation verlangt eine
Validierung gegen das Schema; die Datei liegt nicht im Repository, und ein
Schema, das niemand sehen kann, ist keine Prüfung. Der Test zählt statt dessen
jedes Pflichtfeld von `log`, `entry`, `request` und `response` einzeln auf.

**Der Export schreibt die Datei selbst.** Der Fallstrick der Spezifikation
(„`file_picker.saveFile` gibt unter Linux den Pfad zurück, schreibt nicht
selbst") gilt für die alte Schnittstelle. `file_picker` 12 nimmt die Bytes
entgegen und liefert die `Uri`, unter Linux über den XDG-Desktop-Portal; die
Naht ist ein Aufruf. Bis es eine Host-Redaktion gibt (nach dem MVP), nennt das
Export-Fenster in einem Satz, was die Datei trägt: Hosts, vollständige Pfade,
alle Kopfzeilen und beide Rümpfe im Klartext (`docs/SECURITY.md`).

**CSV ist ein vierter Export.** Die Spezifikation nennt HAR, JSONL und curl.
CSV kam dazu, weil eine Historie in einer Tabellenkalkulation gelesen wird und
die Spaltenliste dafür schon da war; Rümpfe trägt es bewusst nicht, dafür sind
die anderen drei da. Der `curl`-Export schreibt zwei Dateien: den Befehl und
`request.body` daneben. Der Name der zweiten steht im Export-Fenster, bevor
irgendetwas geschrieben wird, und eine vorhandene Datei wird nummeriert statt
überschrieben — nach ihr hat niemand gefragt.

**Der Umfang „Auswahl" ist eine Zeile.** Die Spezifikation meint die
Mehrfachauswahl aus HUM-029; die History hat bisher eine einzelne Auswahl
(`historySelectionProvider` trägt eine `FlowId`). Der Umfang heißt deshalb „die
gewählte Anfrage" und nicht „die Auswahl". Mit der Mehrfachauswahl wird daraus
eine Menge, ohne dass der Rest des Exports sich ändert.

**Die Filterleiste lehrt eine Grammatik, die trifft.** Der Platzhalter zeigt
`host:github.com decision:block since:1h`. `state:` vergleicht gegen die sieben
Zustände des Automaten, und `blocked` ist keiner davon — der Hinweis der
Spezifikation (`state:blocked`) fände nichts, und ein Beispiel, das null Treffer
liefert, lehrt das Gegenteil von dem, wofür es da ist.

**Ein `Lagged` lädt die Seite neu.** Der Ereignisstrom erzeugt bei jedem
Wiederverbinden ein synthetisches `Lagged`; die History lädt daraufhin dieselbe
Abfrage neu und behält dabei die sichtbaren Zeilen, bis die Antwort da ist.
Ohne das bliebe eine gehaltene Zeile stehen, die die Warteschlange längst nicht
mehr hält, und die Fußzeile zählte eine Menge, die es nicht mehr gibt.

**Eine Ankunft unter einem Filter wird gezählt, nicht verworfen.** Nur die
Aufzeichnung weiß, ob sie den Filter trifft und wohin sie gehört. Die Pille
bietet deshalb unter einem Filter ein Neuladen an und am ungefilterten Kopf ein
Zusammenführen; verworfen wird nichts.

**Der leere Bildschirm unterscheidet drei Fälle.** Nichts aufgezeichnet, ein
Filter trifft nichts, und: alles aufgezeichnete ist Durchreich-Verkehr und
damit ausgeblendet. Der dritte ist der Normalfall einer jungen Sitzung, weil
der Agent zuerst das Modell ruft; er nennt den Chip als Rückweg.

**Ereignisstrom und Übergabe liegen in `core`.** `flowEventsProvider` samt
`reconnectBackoffProvider` und `connectFlowEvents` sind nach
`core/ipc/flow_events.dart` gezogen: jeder Bildschirm ist eine Projektion
derselben `Subscribe`, und ein zweiter Provider wäre ein zweites Abonnement.
Eine gehaltene Anfrage reicht die History über `core/ipc/flow_handoff.dart` an
die Warteschlange weiter; ausgeführt wird die Übergabe von der Shell, die die
Sektionen ohnehin zusammensetzt (ARCHITECTURE 5).

### 4.19 Aus der Umsetzung von Notification und Tray (HUM-034, 2026-09-04)

**Die stehende Meldung wird einmal je Bündelungsfenster nachgezogen, nicht bei
jeder Ankunft.** HUM-034 und `docs/UX.md` 4.9 verlangen, dass weitere Ankünfte
dieselbe Meldung aktualisieren. Das tut sie, aber am Ende des Fensters von fünf
Sekunden (`notificationBundle`), nicht im Augenblick der Ankunft. Solange das
Fenster offen ist, kann also „1 request held" stehen, während schon fünfzehn
warten; nach spätestens einem Fenster stimmt die Zahl wieder. Das Protokoll
verlangt für `replaces_id` ausdrücklich, dass der Dienst die stehende Meldung
atomar und ohne Flackern ersetzt, eine stille Änderung ist also vorgesehen;
unzuverlässig ist die Umsetzung. GNOME Shell und Plasma heben eine ersetzte
Meldung neu hervor, und ein Dienst, der die vorige Meldung hat ablaufen lassen,
kennt ihre Kennung nicht mehr, so dass die Ersetzung zu einer zweiten Meldung
wird. Sofort nachziehen hieße auf diesen Arbeitsumgebungen, bei fünfzehn
Ankünften fünfzehnmal aufzupoppen. Tragbar ist die Verzögerung, weil das
Anzeigesymbol und der Fenstertitel währenddessen die wahre Zahl mitführen: der
Mensch wird einmal angesprochen und liest die genaue Zahl dort, wo sie immer
steht. Die Schranke ist ein Fenster und wird von
`the_standing_message_lags_at_most_one_window` festgehalten.

**Angekündigt wird eine Ankunft, keine Änderung.** Der Zustandsautomat merkt
sich die Ids der gehaltenen Anfragen und meldet nur, wenn eine Id dazukommt.
Die Länge der Liste taugt dafür nicht: `heldFlowsProvider` rechnet bei jeder
Änderung von `flowsProvider` neu und liefert jedes Mal eine neue Liste, und
eine einzige durchlaufende Anfrage erzeugt sechs solcher Änderungen.

**Angekündigt wird jede Ankunft, während das Fenster nicht vorn ist, nicht nur
der Übergang von null auf eins.** HUM-034 nennt wörtlich den Übergang;
`docs/CONFIG.md` beschreibt die Bedingung bei `ui.notifications` als „eine
Anfrage wartet und das Fenster ist nicht vorn", und das gilt. Unter der
wörtlichen Lesart erführe ein Mensch, der das Fenster mit einer wartenden
Anfrage verlässt, von den nächsten fünfzehn nichts, weil die Warteschlange nie
leer wurde. Beim Verlassen des Fensters wird deshalb die Warteschlange dieses
Augenblicks als gesehen verbucht; danach hält allein das Bündelungsfenster eine
Ankunft zurück.

**Nach einem Verbindungsabriss und nach jeder Lücke im Ereignisstrom ist die
Zahl unbekannt, bis die Warteschlange selbst wieder etwas sagt.** `GetInfo` und
der Ereignisstrom verbinden sich unabhängig voneinander neu, jeder mit eigenem
Backoff bis 30 Sekunden. Eine zurückgekehrte Verbindung sagt deshalb nichts
über die Warteschlange; erst die erste Meldung des Stroms nach der Lücke tut
das. Der Strom markiert jede Verbindung mit einem `Lagged`, und dieses
Ereignis, nicht der Herzschlag von `GetInfo`, setzt den Merker. Bis zur
Antwort zeigt das Anzeigesymbol den Offline-Zustand, nicht die Zahl von vorher
(4.13).

**Auch beim Start ist die Zahl zuerst unbekannt.** `Subscribe` ohne
`since_flow_id` heißt „ab jetzt". Ein Client, der startet, während der Daemon
drei Anfragen hält, erführe davon sonst nie. Deshalb fährt seit HUM-034 auch
die **erste** Verbindung in `core/ipc/flow_events.dart` mit `afterGap: true`,
also mit einem synthetischen `Lagged` und der `ListFlows`-Neusynchronisation,
die es auslöst. Die Änderung gehört sachlich zu HUM-020; sie steht hier, weil
das Anzeigesymbol der erste Ort war, an dem die Lücke sichtbar wurde. Der
Zustandsautomat startet entsprechend im Zustand „unbekannt" und nicht bei null,
und die Naht zur Shell speist beim Start nur die Verbindung, nicht die noch
leere lokale Karte.

**Jeder Aktionsschlüssel der Meldung trägt die Flow-Id** (`allow:<flowId>`,
`block:<flowId>`, `show:<flowId>`), und die Kennung der Meldung wird beim
Empfang eines `ActionInvoked` bewusst nicht verglichen. Ein Dienst, der
`replaces_id` ignoriert, lässt die vorige Meldung stehen; ein Druck darauf muss
die Anfrage nennen, um die es in **jener** Meldung ging. Eine Anfrage, die die
Warteschlange nicht mehr hält, wird mit `IPC_003` und dem Fenster beantwortet,
nie mit Schweigen.

**Das `dbus`-Paket wirft nackte Zeichenketten.** `on Exception` fängt sie
nicht; jeder Fang um einen Aufruf dieses Pakets ist `on Object catch`. Dazu
setzt das Paket seinen Verbindungs-Completer, bevor es den Socket öffnet, und
schließt ihn bei einem Wurf nie: nach dem ersten fehlgeschlagenen Socket wartet
jeder weitere Aufruf für immer. Ein Adapter darüber merkt sich den ersten
Fehlschlag und kehrt danach sofort zurück. Aus demselben Grund fragt der
Notification-Adapter zuerst nach den Fähigkeiten des Dienstes und abonniert die
Signale erst danach: ein Signal-Abo auf einem Bus, der sich nicht öffnen lässt,
vergiftet den Completer, bevor der erste Methodenaufruf überhaupt läuft.

**Der Weg über „Beenden" räumt auf, bevor er das Fenster zerstört.** Zwei
D-Bus-Verbindungen und ein Busname bleiben sonst offen; das Paket warnt selbst,
dass der Prozess dann womöglich nicht endet.

#### Abweichungen von der Spezifikation des Issues

Die „Betroffene Pfade" von HUM-034 sind vollständig ersetzt; die genannten
Dateien gibt es nicht. Was statt dessen steht, und warum:

| Spezifikation | Umgesetzt | Grund |
|---|---|---|
| `tray_manager` | eigener StatusNotifierItem-Adapter über `dbus` | `tray_manager` bindet `libayatana-appindicator3`; ohne dessen Entwicklungspaket scheitert schon `flutter build linux`, auf einer Maschine, die nie ein Tray wollte. |
| `flutter_local_notifications` | eigener `org.freedesktop.Notifications`-Adapter über `dbus` | dasselbe Protokoll hinter einem Plugin; ein Plugin heißt ein Eintrag in `linux/flutter/generated_plugin_registrant.cc` und eine Systembibliothek zur Bauzeit. `dbus` ist reines Dart. |
| PNG-Symbole `idle`, `held_1..9`, `held_9plus`, `alert` in 22 px und 44 px, erzeugt von `tools/gen_tray_icons.dart` | in `tray_icon.dart` gezeichnet, aus `HColors`, in denselben zwei Größen | kein Generator, keine Assets, kein Auseinanderdriften von Symbol und Gestaltungstoken. |
| feste Notification-`id = 1` | `replaces_id` | die Kennung vergibt der **Dienst**, nicht der Client; `replaces_id` tut das, was die feste Kennung tun sollte. |
| Prüfung auf `libayatana-appindicator3` über `Process.run('ldconfig -p')` | Registrierung beim Wächter entscheidet | es wird keine Bibliothek geladen, also sagt ihre Anwesenheit nichts; ob ein Wächter antwortet, sagt alles. Ein Prozessaufruf beim Start entfällt. |
| drei Symbolzustände (`docs/UX.md` 4.9) | vier: dazu `offline` | was der Daemon nicht bestätigt hat, steht als unbekannt (4.13). Ein Symbol, das die letzte bekannte Zahl weiterzeigt, behauptet etwas. |
| `UI_002` „in der Diagnostics-Ansicht" | Hinweiskarte im Meldungsplatz unter der Kopfzeile | die Ansicht gibt es noch nicht. Die Karte ist dieselbe `HDiagnosticCard` wie im Setup-Bildschirm und verschwindet nach einmaligem Schließen für immer. |
| `attentionProvider` unter `features/intercept/`, Plattformcode unter `core/platform/` | `features/tray/` mit `platform/` darunter, Naht in `features/shell/widgets/tray_host.dart` | ein Feature darf kein anderes importieren (ARCHITECTURE 5); die Shell ist der Rahmen, der Warteschlange und Schreibtisch kennt. |
| `ui.notifications` Stufe `basic` (HUM-034) | Stufe `advanced` | `docs/CONFIG.md` führt den Schlüssel mit `advanced`, und das Register gewinnt gegen die Angabe im Issue. Gebunden ist der Schlüssel noch nicht: dem Client fehlt `GetConfig`. |

### 4.20 Entscheidung ohne Prototyp bei HUM-035 (2026-09-04)

HUM-035 sah einen Prototyp-Branch `spike/forui` mit einer Zeitbox von einem Tag
vor, dazu ein Akzeptanzkriterium („Spike-Branch ist gepusht oder gelöscht mit
Vermerk"). Der Branch ist nicht gebaut worden. Das ist eine bewusste Abweichung
und steht hier, weil ein unerfülltes Akzeptanzkriterium sonst als erledigt
durchginge.

**Grund.** Die Bedingung, an der die Entscheidung hängt, steht ohne Prototyp
fest und wäre durch ihn nicht anders ausgefallen: `shadcn_flutter` 0.0.54 und
forui 0.26.0 verlangen beide Flutter ≥ 3.47.0 und Dart ≥ 3.13.0, während der
Pin in `app/.fvmrc` am Tag dieser Entscheidung auf 3.44.0 stand; er steht seit demselben Tag auf 3.47.2, siehe 3.9. Ein Port hätte mit dieser Anhebung
begonnen, und die Zeitbox wäre vor der ersten portierten Zeile verbraucht
gewesen. Der Fallstrick des Issues, ein halbfertiger Port sei kein Argument,
zeigt in dieselbe Richtung.

**Preis.** Vier Kriterien der Matrix bleiben für beide Bibliotheken Schätzung
statt Messung: 2 (getroffene Bugs), 4 (Theming-Passung), 5 (Tastatur und
Fokus), 6 (Performance in der History), zusammen 11 der 20 Gewichtspunkte.
`docs/adr/0009-ui-stack.md` rechnet unter „Wie belastbar das Ergebnis ist" vor,
wie weit sie das Ergebnis tragen könnten, und nennt unter „Entschieden ohne
Prototyp" die drei Beobachtungen, an denen ein Fehlurteil auffiele.

**Regel für künftige Entscheidungs-Issues.** Eine Bewertungsmatrix braucht
neben Kriterien und Gewichten einen Punkte-Maßstab: je Kriterium eine Schwelle
für 0, 1, 2 und 3 Punkte, für alle Kandidaten dieselbe. Ohne ihn ist jede
Einzelwertung ein Prosa-Urteil, und dieselbe Lücke lässt zwei Maßstäbe zu — im
ersten Entwurf von HUM-035 wurde ein Element, das dem Bestand erlassen war,
einem Bewerber angerechnet. Wird eine im Issue vorgeschriebene Messung
weggelassen, steht die Abweichung in diesem Abschnitt, mit Grund, Preis und
einer nachprüfbaren Bedingung, unter der die Entscheidung wieder aufgemacht
wird.

### 4.21 Aus der Umsetzung der LLM-Durchreiche (HUM-039, Daemon-Hälfte, 2026-09-04)

Abweichungen von `backlog/sprint-3.md`, die dauerhaft gelten. Wo die
Spezifikation anderes sagt, gilt dieser Abschnitt. Die Oberflächen-Hälfte des
Issues (Endpunkt-Feld und Test-Knopf im Setup) steht noch aus.

**Ein Präfix benennt einen Endpunkt, keine API-Fläche.** Die Vorgabe von
`llm.passthrough_paths` ist `["/v1/", "/api/"]`, und beide Einträge sind
Flächen. Unter `/api/` liegt bei Ollama neben der Inferenz auch
`POST /api/pull`, `POST /api/create`, `POST /api/copy`, `POST /api/push`,
`POST /api/blobs/…` und `DELETE /api/delete`; unter `/v1/` liegen bei OpenAI
`POST /v1/files`, `/v1/uploads`, `/v1/vector_stores` und `/v1/fine_tuning/jobs`,
bei vLLM `POST /v1/load_lora_adapter` und `/v1/unload_lora_adapter`. Ein Agent
könnte damit ungefragt am Bestand des Servers arbeiten, und zwar über die eine
Regel, die nicht gehalten wird und private Adressen erlauben darf.

`OpenCodeAdapter::passthrough_prefixes` ersetzt deshalb **beide** Flächen durch
Endpunkte: `/api/` durch `OLLAMA_INFERENCE_PATHS` (`/api/chat`,
`/api/generate`, `/api/embed`, `/api/embeddings`, `/api/tags`, `/api/show`,
`/api/ps`, `/api/version`) und `/v1/` durch `OPENAI_INFERENCE_PATHS`
(`/v1/chat/completions`, `/v1/completions`, `/v1/responses`, `/v1/embeddings`,
`/v1/models`; das letzte deckt über das Präfix auch `/v1/models/<id>`). Mit und
ohne abschließenden `/` gilt dasselbe — ohne ihn wäre die Fläche sogar breiter,
weil `/api` auch `/apifoo` träfe.

Die Regel ist syntaktisch und erklärbar. Wer `POST /api/pull` oder
`POST /v1/files` ohne Rückfrage will, schreibt den Pfad selbst in
`llm.passthrough_paths`; ein Präfix, das mehr nennt als das nackte `/api/` oder
`/v1/`, bleibt unverändert stehen. Die Vorgabe ist damit die sichere Seite, und
was sie nicht deckt, wird gehalten statt geblockt — der Mensch sieht es also,
statt es zu vermissen. Der Test `no_mutating_path_is_covered_by_the_default`
hält die Zusage gegen eine Liste echter verändernder Pfade beider APIs.

**`Matcher.path_prefixes` steht neben `Matcher.path`, nicht an seiner Stelle.**
Beide schränken ein; sind beide gesetzt, müssen beide zutreffen. Eine leere
Liste heißt „egal". Jeder Eintrag muss `humanitl_core::path_prefix_is_valid`
bestehen (beginnt mit `/`, mindestens zwei Zeichen); `rules.yaml` lehnt einen
anderen mit `RULES_005` ab, und eine im Programm gebaute Regel, von der kein
gültiges Präfix übrig bleibt, trifft nichts statt alles.

**Ein `..`-Segment trifft nie ein Präfix.** `/api/chat/../pull` beginnt mit
`/api/chat` und meint `/api/pull`; der Server dahinter löst das auf, bevor er
antwortet. `humanitl_rules::path::prefix_matches` prüft deshalb auf einer
Kopie, in der `%2e` zu `.` und `%2f`, `%5c` sowie `\` zu `/` werden, und lehnt
jeden Pfad mit einem `..`-Segment ab. Der Vergleich selbst läuft danach wieder
auf dem unveränderten Pfad. Die Prüfung gilt nur für `path_prefixes`; Glob und
regulärer Ausdruck bleiben, wie sie waren.

**`Rule.passthrough_llm` kommt nie von der Leitung.** `rule_from_proto` setzt
das Feld immer auf `false`, aus demselben Grund wie bei `bundled`: Ein Client,
der sich eine Durchreichregel anlegen könnte, könnte Verkehr an der
Warteschlange und an der voreingestellten Ansicht vorbeiführen. Gesetzt wird es
vom Agent-Adapter und von `rules.yaml`; in der Antwort steht es, damit die
Oberfläche die Regel als das zeigen kann, was sie ist. In `rules.yaml` gilt es
nur zusammen mit `action: allow`, und eine Durchreiche ohne jede Pfadbedingung
ist `RULES_008` (Warnung, keine Ablehnung).

**Der Fluss trägt die Durchreiche als `DecisionSource::Passthrough`, nicht als
`FlowEvent::Decided.passthrough`.** Die Spezifikation skizziert ein neues Feld
am Ereignis. Die Herkunft gibt es schon, sie bedeutet genau dasselbe, der
Zustandsautomat lässt sie nur aus `Analyzed` zu (eine Durchreiche wird also nie
aus dem Halten heraus entschieden), und der Recorder schreibt daraus
`flows.passthrough`. Ein zweites Feld daneben hätte dieselbe Aussage zweimal
gemacht.

**Preis:** `flows.rule_id` bleibt für einen durchgereichten Fluss leer.
`DecisionSource::Passthrough` trägt keine Regel-Id, und
`humanitl_recorder::writer::rule_of` füllt die Spalte nur aus
`DecisionSource::Rule` oder aus einem `BlockReason::Rule`. Die Zeile ist
trotzdem eindeutig zuzuordnen: `flows.passthrough = 1`, und die Durchreichregel
hat die feste Id `01920000-0000-7000-8000-0000000000ff`. Wer die Spalte
braucht, erweitert `DecisionSource::Passthrough` um die Id; das berührt
`humanitl-core`, `humanitl-recorder`, die Proto und den Fake und gehört
deshalb in ein eigenes Issue.

**`LLM_002` hat seinen Titel geschärft.** Er lautete „LLM-Endpoint antwortet
nicht als OpenAI-kompatible API"; diese Bedeutung trägt jetzt `LLM_003`, und
`LLM_002` steht für „verlangt eine Anmeldung" (401/403), wie die Tabelle in
HUM-039 es vorgibt. Die Nummer wird damit nicht wiederverwendet: Bis HUM-039
hat kein Codepfad sie je ausgegeben.

**`llm.probe_timeout_ms` gibt es nicht; die Frist steht in der Anfrage.**
`ProbeLlmRequest.timeout_ms` trägt sie, `0` heißt die Vorgabe von 3000 ms
(`humanitl_proxy::llm_probe::DEFAULT_TIMEOUT_MS`). Grund: Die Probe ist eine
Handlung im Setup und keine Dauereinstellung, und der einzige Aufrufer kennt
seine eigene Geduld besser als eine Datei. Wer den Schlüssel später doch will,
legt ihn in `humanitl-config` an und lässt den Dienst ihn als Vorgabe
einsetzen.

**Die Probe bekommt einen eigenen Resolver-Stapel.** `IpcServer::new` baut ihn
aus derselben Konfiguration wie der Proxy (`resolver.*`, `limits.*`), aber als
eigene Instanz. Der Zähler des Proxy-Resolvers ist der Zeuge dafür, dass vor
einer Freigabe kein Name aufgelöst wird (ADR-006, Escape-Test 3); eine
Auflösung, die ein Mensch im Setup selbst angestoßen hat, gehört nicht in
diesen Beweis. Dasselbe Verhalten gegenüber `ollama.lan` haben beide trotzdem,
weil die Einstellungen dieselben sind. Der Daemon braucht dafür keine
zusätzliche Verdrahtung; `IpcServer::with_llm_probe` gibt es nur für Tests.

**`Upstream` hat `resolve` und `forward_to` getrennt.** `forward` ist beides
nacheinander. Die Probe braucht die aufgelöste Adresse, um über `LLM_006`
etwas sagen zu können, und darf dafür nicht ein zweites Mal fragen. Die
Anheftung bleibt: Zwischen der Prüfung und dem Verbinden kommt keine zweite
Auflösung dazwischen.

**Die Probe fragt die API-Wurzel, nicht den Endpunkt-Pfad.** Ein abschließendes
`/v1` gehört zur OpenAI-kompatiblen Oberfläche und wird abgeschnitten:
`http://host:1/v1` und `http://host:1` fragen beide `…/api/tags` und
`…/v1/models`. Das ist das Gegenstück zu `OpenCodeAdapter::base_url`, das an
dieselbe Wurzel wieder ein `/v1` hängt. Ein anderer Pfadanteil (`/openai`)
bleibt erhalten.

**`ProbeResult` trägt eine Liste von Befunden, nicht nur einen.** `LLM_003`
(keine bekannte API) und `LLM_006` (nicht im eigenen Netz) stehen **neben**
einem Ergebnis und nicht an seiner Stelle; nur `LLM_001` und `LLM_002` sind
ein `Err`. `ProbeLlmResponse` hat deshalb `diagnostics` und daneben
`diagnostic` mit dem ersten Eintrag, wie `RulesResponse` es schon tut.

**Die Regel muss auch im laufenden Daemon stehen.** `llm_passthrough()` allein
tut nichts: `humanitld::load_rules` stellt das Ergebnis vor die mitgelieferten
Regeln, und ohne diese Zeile hält der Proxy jede Inferenz an, während der
Durchreich-Zweig, `DecisionSource::Passthrough` und `LLM_005` toter Code
bleiben — in den Crate-Tests grün, im Programm wirkungslos. Die Reihenfolge ist
Teil davon: Die Durchreiche steht **vor** allen anderen mitgelieferten Regeln,
sonst stünde die Blockregel `host: "**"` des Profils `llm-only` davor. Der
Beleg ist ein Test am gestarteten Binary
(`the_configured_llm_endpoint_becomes_a_passthrough_rule`), nicht am Adapter;
ein Adapter-Test hätte diese Lücke nie gesehen. `humanitld` hängt dafür an
`humanitl-sandbox`, was `tools/deps-allow.toml` für die Gruppe `bins` erlaubt.

**Ein Befund fällt nie unter den Durchreich-Filter.** `Subscribe` versteckt ohne
`include_passthrough` jedes Ereignis eines durchgereichten Flusses. Ein
`FlowEvent::Diagnostic` ist davon ausgenommen, im echten Daemon wie im Fake:
`LLM_005` warnt vor genau der Anfrage, die der Filter versteckt, und mit ihr zu
verschwinden kehrte die Zusage aus `docs/SECURITY.md` 3.1 um. Eingeklappt heißt
nicht stumm.

**Ein Befund mit Fluss reist als `FlowEvent.flow_diagnostic` (Feld 16).** Der
Strom ist sitzungsweit; der alte Arm `diagnostic` (12) trägt keine `flow_id`,
und ohne sie könnte kein Client die Meldung ihrem Fluss zuordnen. Der alte Arm
bleibt für Befunde, die zu keinem Fluss gehören — ein `ClientHello` ohne SNI
etwa (`TLS_003`), aus dem noch gar kein Fluss geworden ist.

**Eine Durchreiche muss genau ein Ziel nennen.** `too_broad` warnt mit
`RULES_008`, sobald `passthrough_llm` ohne exakten Host, ohne Port, ohne Schema
oder ohne Pfadbedingung dasteht. Der Host wiegt am schwersten: Eine Durchreiche
mit `host: "**"` reichte jeden Host der Welt ungehalten durch und bliebe dabei
aus der voreingestellten Ansicht heraus. `too_broad` gibt deshalb eine Liste
zurück statt eines einzelnen Befunds; vorher übersprang ein früher `return` die
Prüfung auf „alles".

**Was privat heißt, steht im Register.** Für `LLM_006` zählen die aufgelöste
Adresse (RFC 1918, Loopback, Link-Local, CGNAT) und der Name: `localhost` sowie
`.local`, `.lan`, `.home.arpa` und `.internal`. Die Spezifikation nennt nur die
ersten drei Suffixe; `localhost` und `.internal` kamen bei der Umsetzung dazu,
weil beide dasselbe meinen und ein Mensch sie tippt. Der Doc-Kommentar an
`LLM_006` in `codes.rs` ist die verbindliche Liste.

**Eigene Codes für eigene Zustände.** `LLM_007` steht für eine Adresse, die sich
gar nicht als HTTP-Adresse lesen lässt — nicht `LLM_001` und nicht `LLM_003`,
weil beide eine Beobachtung am Endpunkt behaupten würden und hier weder
aufgelöst noch verbunden wurde. `IPC_006` steht für „diesen RPC gibt es, aber
dieser Daemon hat nicht, was er dafür braucht"; vorher lieh sich die fehlende
Probe `IPC_005`, das „Rules-Anfrage ungültig" heißt.

**Eine Frist über der Obergrenze wird geklemmt.** `ProbeLlmRequest.timeout_ms`
gilt bis `MAX_TIMEOUT_MS` (30 000 ms). Eine Probe hält einen Task und eine
ausgehende Verbindung fest, solange sie läuft; ohne Obergrenze band ein
Aufrufer beides für bis zu 49 Tage.

**Eine Zeitüberschreitung nach einer Antwort ist keine Unerreichbarkeit.** Hat
der Server schon einmal geantwortet und läuft die Frist erst beim zweiten Pfad
ab, sagt der Befund das auch. Derselbe Code `LLM_001`, weil der Agent so oder so
nicht arbeiten kann, aber ein `why`, das die Beobachtung nicht überzeichnet
(4.13).

**`latency_ms` misst die Probe, nicht das Modell.** Gezählt wird vom Beginn der
Auflösung bis zur Antwort, aus der das Ergebnis stammt; bei einem
OpenAI-kompatiblen Server zählt der erste Umlauf nach `/api/tags` mit, der ins
Leere ging. Das ist die Zahl, die ein Mensch am Testknopf erlebt.

**Die Probe nutzt den mitgelieferten Wurzelsatz, nicht den System-Trust-Store.**
Die Spezifikation verlangt den des Hosts. Umgesetzt ist `webpki-roots`, also
derselbe Satz, den `Upstream` für jede Verbindung nach draußen benutzt. Der
Grund ist die Gleichheit mit dem Proxy-Pfad: Ein zweiter Vertrauensweg neben
dem einen, der zählt, wäre eine Aussage mehr, die niemand prüft. Der Kern der
Vorgabe bleibt erfüllt — es ist nicht die Humanitl-CA.

**Das Verhalten des Resolvers ist über `resolver.overrides` belegt, nicht über
`/etc/hosts`.** Der Fallstrick von HUM-039 nennt einen `/etc/hosts`-Eintrag im
CI. Der Test `probe_and_proxy_resolve_a_lan_name_the_same_way` gibt beiden
Seiten dieselbe `ResolverConfig` mit einer festen Zuordnung für `ollama.lan`
und lässt den Namensdienst darunter für diesen Namen scheitern; kommt trotzdem
eine Verbindung zustande, hat die Zuordnung geantwortet, und zwar auf beiden
Wegen. Ein `/etc/hosts` täte dasselbe, verlangte aber Schreibrechte am System
und wäre keine Prüfung mehr, sondern eine Umgebung.

**Der Name ist das Restrisiko der Durchreiche.** Steht in `llm.endpoint` ein
DNS-Name, entscheidet der Resolver bei jeder Anfrage neu, wohin sie führt, und
`allow_private` macht auch den Router und `169.254.169.254` zu gültigen Zielen.
Das ist DNS-Rebinding an der einen Stelle, an der ADR-0006 es nicht verhindern
kann, weil dort niemand gefragt wird. `docs/SECURITY.md` 3.1 und
`docs/THREAT-MODEL.md` K-02 nennen es und empfehlen eine IP-Adresse oder einen
festen Eintrag unter `resolver.overrides`.

**Offen und bewusst nicht in dieser Hälfte gebaut.** `docs/PROTOCOL.md` 4.9
verlangt zu jedem neuen RPC ein CLI-Subkommando im selben Issue. `ProbeLlm` hat
noch keines: `humanitl rules test` selbst antwortet bis heute mit `CLI_003`
(HUM-065), und ein zweites Kommando neben einer fehlenden Grundlage hilft
niemandem. Wer HUM-065 baut, nimmt `humanitl llm test URL` mit; die RPC steht
dafür bereit. Ebenso offen: `app/lib/core/ipc/proto_version.dart` steht noch
auf Minor `1`, während der Daemon `2` meldet — das ist verabredetermaßen keine
Störung (`docs/PROTOCOL.md` 5), und die Oberflächen-Hälfte zieht die Zahl nach.

### 4.22 Aus der Umsetzung des M2-Demoskripts (HUM-036, Daemon-Hälfte, 2026-09-04)

Abweichungen von `backlog/sprint-2.md`, die dauerhaft gelten. Wo die
Spezifikation anderes sagt, gilt dieser Abschnitt. Die Oberflächen-Hälfte des
Issues (`app/integration_test/m2_first_decision_test.dart` und der HAR-Export
aus dem Lauf) steht noch aus; `tests/e2e/m2_first_decision/run.sh` überspringt
sie mit einer ausdrücklichen Meldung, solange die Datei fehlt, und prüft sie,
sobald es sie gibt.

**Fake-Upstream und Fake-Agent sind Python, keine Rust-Binaries.** Die
Spezifikation nennt einen axum-Server und ein statisch gelinktes
musl-Binary. Beide wären zwei weitere Workspace-Member samt Bauzeit in jedem
Lauf, für ein Ziel, das zurückmeldet, wonach gefragt wurde, und einen Agenten,
der eine Liste von Zeitpunkten abarbeitet. Der M1-Lauf hat für sein Ziel
denselben Weg gewählt und ihn dort begründet. In der Sandbox liegen `python3`
und `curl` unter `/usr`, das jedes Profil ohnehin nur lesbar einhängt; der Lauf
belegt vor dem ersten Schritt, dass beide da sind, und der Agent bricht mit
einer Meldung ab, wenn `curl` fehlt. `daemon/Cargo.toml` und
`tools/deps-allow.toml` bleiben damit unberührt.

**Der Agent spricht über `curl`, nicht über eine eigene HTTP-Bibliothek.**
Derselbe Grund, aus dem das Demoskript über `humanitl` fährt und nicht über
einen eigenen gRPC-Klienten (3.11): Gemessen werden soll der Weg, den ein
echter Agent nimmt. `curl` liest `HTTP_PROXY` und `CURL_CA_BUNDLE` aus dem
Umgebungs-Kit des Profils, löst deshalb selbst keinen Namen auf und spricht
ausschließlich mit dem Proxy auf `127.0.0.1:3128`.

**Das Ziel liegt auf einer Adresse aus TEST-NET-2, nicht auf `127.0.0.1`.** Die
Spezifikation stellt den Fake-Upstream auf `127.0.0.1:8443` und lenkt den Port
mit `experimental.upstream_port_map` um. Beides geht nicht: Der Proxy weist
jede aufgelöste Adresse in einem privaten Bereich ab (`ip_is_private`, ADR-006),
und `127.0.0.1` ist eine; eine Freigabe wäre dort immer `502
upstream_private_address` statt einer Antwort. Ausserdem liest den Schlüssel
`experimental.upstream_port_map` heute niemand — er steht im Schema, wird
validiert und hat im Proxy keinen Aufrufer. Der Lauf nimmt deshalb denselben
Weg wie M1: ein eigener Netz-Namensraum, in dem `198.51.100.7` (RFC 5737) auf
`lo` liegt, und `resolver.overrides` zeigt die drei Hosts dorthin. Im eigenen
Namensraum ist der Lauf root und bindet die Ports 80 und 443 direkt, also
braucht er gar keine Umlenkung.

**Der Verkehr des Laufs ist Klartext-HTTP, nicht HTTPS — und damit fehlt
Abdeckung, nicht nur eine Variante.** Sechzehn der siebzehn Anfragen laufen
über Klartext; die einzige verschlüsselte existiert, um zu scheitern. Für
keinen einzigen freigegebenen oder geblockten Fluss werden deshalb ausgeführt:
das Prägen eines Blatts aus der eigenen CA, der TLS-Handschlag mit dem Agenten
hinter seinem `CONNECT`, die TLS-Sitzung nach oben und der Fund im
entschlüsselten Rumpf. Das ist die Hauptbauart des Produkts, und der einzige
vollständige Lauf, den es gibt, geht sie nicht. Kein grüner M2-Lauf darf als
Beleg für den MITM-Pfad gelesen werden, solange dieser Absatz hier steht; die
Abdeckung kommt mit **HUM-087** zurück, das `--allow-test-ca` nachliefert, den
Lauf auf `https://` stellt und Schritt 7 umdreht. Bis dahin belegen ihn allein
die Integrationstests in `daemon/crates/proxy/tests`, und die fahren keine
Sandbox.

Der Fake-Upstream zeigt auf 443 bereits ein Testzertifikat für die drei Hosts,
das bei jedem Lauf neu entsteht (`tests/e2e/fake-upstream/gen-test-ca.sh`,
`umask 077` vor der ersten Datei) und nie im Repository liegt. Der Daemon nimmt
es nicht an: `resolver.test_ca` steht im Schema und in
`docs/CONFIG.md`, aber `humanitld` liest den Schlüssel nicht (`ClientTls::new`
bekommt eine leere Wurzelliste) und meldet das beim Start als Warnung; das
Flag `--allow-test-ca`, das die Spezifikation nennt, gibt es nicht. Der Lauf
belegt deshalb die andere Richtung und macht sie zu einer Zusicherung: Schritt
7 („a test CA in the configuration is not trusted on its own") schickt eine
TLS-Anfrage an dasselbe Ziel, die die Sitzungsregel ohne Rückfrage erlaubt, und
erwartet `502` mit `error = upstream_tls`. Eine fremde Wurzel, die ohne Flag
gälte, wäre ein Loch in `docs/SECURITY.md`, und deshalb ist die heutige Lage
die sichere Seite.

Der Schritt hält damit eine **Abwesenheit** fest, keine Verweigerung: Der
Daemon lehnt die Wurzel nicht ab, er sieht sie nicht an. Zwei Vorkehrungen
halten die Aussage ehrlich, und beide gehören dazu, wenn jemand den Schritt
anfasst. Erstens ein Stolperdraht auf der Kommandozeilen-Fläche statt auf dem
Ergebnis: Der Lauf prüft, dass `humanitld --help` das Flag **nicht** kennt, und
stirbt mit der Anweisung, was umzudrehen ist, sobald es da ist. Ohne ihn bliebe
Schritt 7 grün, nachdem der Mangel behoben wäre, weil `start_daemon` den Daemon
weiter ohne das Flag startete. Dazu liest der Lauf die Zeile aus dem
Daemon-Protokoll, in der der Daemon selbst sagt, dass er den Schlüssel nicht
liest — die Behauptung stammt damit von ihm und nicht aus einem Ausbleiben.
Zweitens eine positive Kontrolle: `502 upstream_tls` entsteht genauso, wenn das
Blatt für die falschen Hosts gälte, abgelaufen wäre oder von einer fremden
Wurzel stammte. Ein `curl` im selben Namensraum, mit `--cacert` auf dieselbe
Wurzel, am Proxy vorbei und auf einem eigenen Pfad (`/tls-control`, damit die
Null-Zählung für `/tls-probe` unberührt bleibt), schafft den Handschlag — und
derselbe Aufruf ohne die Wurzel scheitert. Erst dieses Paar belegt die
Voraussetzung des Schritts: Das Material ist gültig, und was ihm fehlt, ist
allein das Vertrauen des Daemons.

Wer das Flag nachliefert, dreht den Schritt um und stellt die URLs in
`script.json` auf `https://`; alles andere am Lauf bleibt, wie es ist.

**Der Daemon warnt nur für `resolver.test_ca`, nicht für die anderen
Test-Hebel.** Der Fallstrick der Spezifikation verlangt eine Warnung beim
Start, damit `resolver.overrides` und `experimental.upstream_port_map` „nie
unbemerkt in Produktion landen". `humanitld` meldet beim Start nur
`resolver.nameserver` (ungenutzt) und `resolver.test_ca` (ungelesen); eine
nicht leere Zuordnungstabelle und eine gesetzte Portumlenkung gehen still
durch. Der Demolauf lebt davon, also fällt es dort nicht auf; im Alltag ist es
eine fehlende Warnung an genau der Stelle, an der die Spezifikation eine
verlangt. Gehört zum selben Bündel wie das fehlende `--allow-test-ca`.

**Die Stapel-Freigabe geht über zwei Aufrufe, nicht über einen.**
`DecideRequest` trägt `repeated flow_ids` und `remember`, kann eine Gruppe also
in einem Zug freigeben und die Regel dabei anlegen. `humanitl flows decide`
kennt weder mehrere Ids noch `--remember`; die Kommandozeile hat für die
Fähigkeit, die es im RPC und in der Oberfläche gibt, kein Gegenstück. Der Lauf
legt deshalb erst die Sitzungsregel über `humanitl rules add --expires session`
an und entscheidet dann die zwölf wartenden Anfragen einzeln. Die Wirkung ist
dieselbe — entschieden wird beim Eintreffen, die zwölf gehen also über die
Entscheidung und alles Spätere über die Regel —, der Preis ist größer als er
zunächst aussieht: Die Regel trägt kein `created_from_flow_id`, das Abzeichen
„from #n" des Regel-Bildschirms hat für sie nichts anzuzeigen, und damit ist
der Akzeptanzschritt 8 der Spezifikation („Rules-Screen: Temporär-Tab zeigt die
Session-Regel mit `from #1`") auch für die Oberflächen-Hälfte unerfüllbar,
solange die Regel neben der Entscheidung entsteht statt in ihr. Das ist eine
Lücke in der Parität von Oberfläche und Kommandozeile (ADR-018,
`docs/ARCHITECTURE.md` 3b); sie wird in **HUM-095** geschlossen, das
`humanitl flows decide <id> allow --remember <PATTERN>` nachliefert und den
M2-Lauf die Sitzungsregel über die Entscheidung anlegen lässt. Bis dahin gilt
Schritt 8 als offen und nicht als erfüllt.

**Die Haltefrist des Laufs ist 10 Sekunden, nicht 8, und sie ist zweierlei.**
Sie ist die Zeit, nach der die eine unentschiedene Anfrage 504 bekommt — dafür
soll sie kurz sein —, und zugleich das Budget, in dem die Kommandozeile die
zwölf gehaltenen Anfragen entscheiden muss, bevor die erste von ihnen verfällt.
Ein eigener Prozess je Entscheidung braucht davon gemessen 21 bis 30 Prozent;
auf einem langsamen Läufer wird der Lauf rot, ohne dass am Produkt etwas falsch
wäre. Das ist der eine Punkt, an dem dieses Gate an fremder Hardware wackeln
kann, und er verschwindet mit HUM-095: Mit einem einzigen `Decide` für den
Stapel ist das Budget kein Faktor mehr, und 8 Sekunden reichen wieder.

**`state:blocked` und `findings:>0` sind nicht dasselbe Paar wie in der
Spezifikation.** `state:` vergleicht gegen die sieben Zustände des Automaten,
und `blocked` ist keiner davon (4.18). Eine Zeitüberschreitung ist ausserdem
`decision = timed_out` mit `block_reason = timeout`, nicht `decision = block`:
Der Lauf zählt deshalb `decision:block` (eine Zeile, der Block eines Menschen)
und `decision:timed_out` (eine Zeile) getrennt und prüft die Gründe über
`reason:user` und `reason:timeout`.

**Der Export wird ohne die Oberfläche als Menge geprüft, nicht als Datei.** Das
HAR entsteht in `app/lib/features/history/export/har.dart`, also in der
Oberfläche; ohne sie gibt es keine Datei. Der Lauf prüft statt dessen die
Menge, aus der der Export entsteht: siebzehn Flüsse, fünfzehn `allow`, ein
Block eines Menschen, eine Zeitüberschreitung, zwei mit Funden, zwei durch die
Sitzungsregel. Sobald die Oberflächen-Hälfte da ist, prüft Schritt 10
zusätzlich die geschriebene Datei.

**`tests/e2e/lib.sh` zählt die Behauptungen.** `e2e_check` erhöht
`E2E_ASSERTIONS`, durch das jede Behauptung geht. Der M2-Lauf vergleicht den
Zähler am Ende mit einer festen Zahl im Skript und scheitert, wenn weniger
gelaufen sind: Ein Demoskript, das grün ist, weil ein Zweig übersprungen wurde,
ist schlimmer als keines. Beim M1-Lauf läuft der Zähler mit, ohne dass er ihn
prüft.

**Der Einstieg `tests/e2e/run.sh` fährt beide Demos.** Ein Meilenstein, ein
Skript, und jedes bleibt stehen (BACKLOG.md 8). `E2E_ONLY=m1` oder `m2` fährt
eines davon; die CI nutzt das, weil M1 im Job `e2e` und M2 im Job `e2e-xvfb`
läuft. Der zweite Lauf baut nicht noch einmal.

**Jede Demo hat ihr eigenes Artefakt-Verzeichnis.** M1 schreibt nach
`target/e2e/m1`, M2 nach `target/e2e/m2`, und jede räumt vor dem Lauf nur ihr
eigenes leer. Vorher lag M1 direkt in `target/e2e` und begann mit `rm -rf`
darauf — wer die Demos in der anderen Reihenfolge fuhr, verlor damit die
Artefakte der anderen. Die CI lädt weiter `target/e2e` als Ganzes hoch.

**Ein Abbruch bricht ab.** `e2e_trap` in `lib.sh` hängt den Aufräumer nicht nur
an `EXIT`, sondern auch an `INT`, `TERM` und `HUP`, und endet dort mit 130. Der
Grund ist gemessen: Ein Demoskript wartet die meiste Zeit in `wait` auf einen
Hintergrundprozess; trifft `SIGINT` die Shell dort und ist kein eigener Handler
gesetzt, bricht nur der Wartelauf ab, das Skript läuft weiter, entscheidet
weiter und meldet am Ende „OK". Wer abgebrochen hat, bekäme einen grünen
Bericht über einen Lauf, den er beendet zu haben glaubte; und wer statt dessen
hart tötet, lässt Daemon, Ziel, Sandbox-Baum, Shim, Agent und die privaten
Schlüssel des Laufs stehen. Beide Demos benutzen `e2e_trap`, und beide räumen
ihren Baum unter `/tmp` am Ende weg — auch nach einem Abbruch.

**Der Isolationsbericht wird gelesen, nicht nur geschrieben.** `humanitl
sandbox run -v` schreibt die drei Zeilen `check <name> pass|FAIL: <evidence>`
nach stderr, und `sandbox run` beendet die Sandbox mit Exit 3, sobald eine
davon rot ist. Beendet, nicht verhindert: Der Befehl ist zu diesem Zeitpunkt
schon gestartet (korrigiert 2026-09-04, HUM-041; `docs/THREAT-MODEL.md` K-15).
Der M2-Lauf prüft die Zeilen deshalb ausdrücklich: Er ist der
einzige Lauf, in dem die Sandbox echten Verkehr trägt, und ein Bericht, in den
niemand sieht, ist kein Beleg. Drei Zusicherungen, aus derselben Datei, die
auch der Agent beschrieben hat.

**Zusicherungen, die aus zwei Gründen leer sein könnten, kommen paarweise.**
Eine Zahl null am Ziel („die geblockte Anfrage kam nie an") ist ohne
Gegenstück auch dann grün, wenn das Ziel gar nicht lief; ein leeres Feld in der
Ausgabe des Agenten auch dann, wenn er nie gestartet ist. Deshalb hängt in
diesem Lauf jede solche Null an einer positiven Zahl aus derselben Quelle: die
Null-Treffer im Protokoll des Ziels an den sechzehn bedienten Anfragen darin,
die leeren Agentenfelder an den siebzehn Zeilen seiner Ausgabe, der
gescheiterte Handschlag an dem, der mit derselben Wurzel gelingt. Wer eine
weitere Zusicherung dieser Form hinzufügt, bringt ihr Gegenstück mit.

**Heute steht nur die Daemon-Hälfte des M2-Gates.** HUM-036 verlangt den vollen
Kreislauf mit echtem Daemon, echter Sandbox **und echtem UI unter xvfb**, samt
gültiger HAR-Datei, und `CONTRIBUTING.md` erklärt M2 zur Voraussetzung für
jeden Merge ab Sprint 3. Gebaut ist die Hälfte, die ohne Oberfläche prüfbar
ist; `run.sh` überspringt seinen Schritt 10 mit einer ausdrücklichen Meldung
und meldet trotzdem Erfolg. Damit gälte das Gate als erfüllt, ohne es zu sein —
genau die Sorte Behauptung, die 4.13 verbietet. Die Lücke hat deshalb eine
Nummer (**HUM-097**), und drei Stellen sagen sie laut: der Kopf von `run.sh`,
der Abschnitt „Stand" in HUM-036 und der Absatz „The M2 gate is half built" in
`CONTRIBUTING.md`. Ein grünes `e2e-xvfb` heißt bis dahin „die Daemon-Hälfte von
M2 hält", nicht „M2 hält". Wer sich darauf beruft, sagt dazu, worauf.

**Was ein grüner M2-Lauf trägt, und was nicht.** Ein Gate ist nur so viel wert,
wie ein späterer Leser über seine Reichweite weiß; der Kopf von `run.sh`
wiederholt das, damit niemand dafür diesen Abschnitt suchen muss. Er sagt
nichts über den Bildschirm — Warteschlange, Aktionsleiste, Regel-Bildschirm und
Historie werden nicht bedient (HUM-097). Er sagt nichts über das HAR-Format;
geprüft wird die Menge, aus der der Export entsteht, nicht eine Datei und kein
Feld darin. Er übt den MITM-Pfad nicht (HUM-087, Absatz oben). Er sagt nichts
über eine zweite Sitzung, über Neustarts (das prüft M1), über OpenCode
(HUM-046) und über Benachrichtigungen (abgeschaltet). Und Schritt 7 hält eine
Abwesenheit fest, keine Verweigerung.

### 4.23 Aus der Umsetzung der Profile (HUM-066, 2026-09-04)

Abweichungen von `backlog/sprint-3.md`, die dauerhaft gelten. Wo die
Spezifikation oder 4.4 anderes sagt, gilt dieser Abschnitt.

**Sieben Ebenen statt sechs.** Die Präzedenz aus 3.7 und 4.4 bekommt eine
zweite Profil-Ebene: eingebaute Vorgabewerte, `config.toml`, Profil `default`,
gewähltes Profil, Projekt-Profil, Umgebung, Kommandozeile. `default` gilt
immer, auch wenn ein anderes Profil gewählt wurde; das gewählte liegt darüber.
Beide Ebenen nehmen die Datei aus `$XDG_CONFIG_HOME/humanitl/profiles/`, sonst
die eingebettete Fassung. `docs/CONFIG.md` und `docs/profiles.md` sind die
Beschreibung.

**Das mitgelieferte Profil `default` setzt keinen Wert.** Es liegt über
`config.toml`; ein Wert darin — auch einer, der nur den Vorgabewert wiederholt,
wie ihn das Beispiel im Issue zeigt — machte die Datei des Nutzers für diesen
Schlüssel wirkungslos, ohne dass er es sähe. Der gängige Fall steht deshalb als
Kommentar in `profiles/default.toml`, und der Test
`the_default_profile_sets_no_value` hält es fest. `llm-only` setzt sehr wohl
Werte: es zu wählen ist eine ausdrückliche Entscheidung und soll `config.toml`
überstimmen.

**Ein Profil hat vier Schlüssel, nicht fünf.** `name`, `description`,
`[config]` und `[rules]`. Der Block `[agent]` aus 4.11 entfällt: `agent.adapter`
und `agent.command` sind Konfigurationsschlüssel und stehen unter
`[config.agent]`. Ein `[agent]` auf der obersten Ebene ist damit `CONFIG_002`
mit dem Hinweis auf `[config.agent]`, statt still übergangen zu werden;
`PROFILE_PASSTHROUGH` gibt es nicht mehr, an seine Stelle tritt
`humanitl_config::PROFILE_KEYS`.

**`ConfigOverlay` ist eine Menge von Blattpfaden, kein `Option`-Spiegel von
`Config`.** Die Spezifikation skizziert „`Config` mit allen Feldern
`Option<T>`". Gemischt wird aber ohnehin auf Blattpfaden, und ein zweiter, von
Hand gepflegter Typ mit denselben vierzig Feldern wäre die doppelte Pflege, die
ADR-011 ausschließt. `ConfigOverlay` hält deshalb `BTreeMap<Blattpfad, Wert>`;
feldweise wirkt es dadurch von selbst, und eine freie Tabelle bleibt ein Blatt.

**`EffectiveConfig` heißt weiter `Resolved`.** Der Typ aus HUM-062 hat schon
`config` und `origins`; dazu kommen `profiles: Vec<Profile>` und die Methode
`profile_chain() -> Vec<Origin>`. Ein zweiter Name für dieselbe Sache hätte
jeden Aufrufer in `humanitl-ipc`, `humanitl-proxy` und beiden Binaries berührt,
ohne etwas auszusagen. `Origin` bekommt die Variante `ProfileBuiltin(String)`
(`kind()` = `profile_builtin`), die Ränge rücken auf 0..6.

**`resolve()` nimmt `Env` und `&[(String, String)]`.** Die Signatur der
Spezifikation (`env: &[(String,String)]`, `cli: &CliOverrides`) hätte zwei neue
Typen neben `Env` und `Sources::cli` gestellt. Es gilt
`resolve(&ProfileSelection, work_dir: Option<&Path>, &Env, &[(String, String)])`;
daneben steht `sources_for(..)` mit denselben Argumenten für Aufrufer, die vor
dem Laden noch an den Quellen etwas ändern (die Kommandozeile mit `--config`).

**`--profile` benennt zwei Dinge, und eine Regel unterscheidet sie.**
**Verworfen am 2026-09-04, siehe den Nachtrag am Ende dieses Abschnitts. Die
Fassung hier ist nicht mehr gültig und steht nur noch da, damit die Begründung
der Rücknahme einen Bezug hat.** 3.8 nennt
`--profile NAME` sowohl für `humanitl run` (Sitzungsprofil) als auch für
`humanitl sandbox run|argv` (bwrap-Profil unter `profiles/sandbox/`). Es ist
deshalb kein Zweitname von `--sandbox-profile` mehr, sondern ein eigenes
globales Argument: Gibt es ein Sitzungsprofil dieses Namens, ist es gemeint;
sonst benennt der Name das bwrap-Profil und landet als `sandbox.profile` auf der
Kommandozeilen-Ebene. `--profile test` verhält sich damit wie bisher (die
Escape-Tests und `tests/e2e` bleiben unberührt), und `--profile llm-only` lässt
`sandbox.profile` in Ruhe, statt eine Datei `profiles/sandbox/llm-only.toml` zu
suchen, die es nicht gibt. `humanitl run` ist die Ausnahme und immer streng:
dort ist `--profile` das Sitzungsprofil, ein unbekannter Name ist `CONFIG_001`,
und wer das bwrap-Profil meint, schreibt `--sandbox-profile`.
`humanitl_config::discover_with` bleibt entsprechend nachsichtig,
`humanitl_config::resolve` ist streng.

**Drei Grenzen für das Projekt-Profil statt einer.** Zu `x-project-scope` aus
4.11 kommen zwei dazu: Ein `[rules]`-Block im Projekt-Profil ist `CONFIG_003` —
ein geklontes Repository entscheidet nicht, was die Sandbox verlassen darf —,
und `sandbox.mounts.extra_ro` / `sandbox.mounts.extra_rw` aus dieser Ebene sind
`CONFIG_003` mit dem Satz, dass nur globale Profile Host-Pfade einhängen dürfen.
Die Schlüssel stehen nicht im Schema (Einhängungen wohnen im Sandbox-Profil);
ohne diese Ausnahme wäre die Antwort „unbekannter Schlüssel, meintest du
`sandbox.profile`?" und damit die irreführendste, die möglich ist.
`hold.ask_mode` bleibt entgegen dem Text des Issues gesperrt: 4.11 führt es als
`denied`, und Abschnitt 4 geht vor.

**Zwei neue Diagnose-Codes.** `CONFIG_007` (Warning) für ein Projekt-Profil, das
einem anderen Konto gehört — das Issue nennt dafür `CONFIG_004`, die Nummer ist
seit 4.11 für den Ersatz des Laufzeitverzeichnisses vergeben, und eine Nummer
wird nie wiederverwendet. `CONFIG_008` (Info), wenn eine eigene Profildatei ein
mitgeliefertes Profil desselben Namens verdeckt und sich von ihm unterscheidet;
eine wortgleiche Kopie wird nicht gemeldet. Der Bereich `CONFIG` hat damit noch
`009` frei und muss für den übernächsten Code wachsen.

**`Env::default()` trägt die Nutzerkennung des Prozesses.** Bisher war sie 0,
weil `Default` abgeleitet war, während `Env::from_pairs` schon `current_uid()`
setzte. Seit `CONFIG_007` die Kennung gegen den Besitzer einer Datei hält, machte
die 0 aus jeder Datei die eines Fremden. Wer eine feste Kennung braucht, nimmt
`Env::with_uid`.

**Regeln aus einem Profil reisen als Dokument, nicht als `Rule`.**
`humanitl-config` darf nicht auf `humanitl-rules` zeigen (`deps-allow.toml`).
`Profile::rules_document()` liefert die Regeln aus `[rules].inline` deshalb als
JSON, das gültiges YAML ist und das `humanitl_rules::parse_rules` unverändert
liest; `Profile::rule_files()` liefert die Pfade aus `[rules].files`, relativ zur
Profildatei aufgelöst. Die Test-Abhängigkeit auf `humanitl-rules` steht unter
`[dev-dependencies]` und zeigt damit nicht nach außen.

**`humanitl run` löst auf und startet nichts.** Bis HUM-067 endet es weiter mit
`CLI_003`; neu ist, dass es vorher `Context::session()` ruft. Ein Projekt-Profil,
das Host-Pfade einhängen will oder einen gesperrten Schlüssel setzt, verweigert
damit schon hier den Start mit `CONFIG_003`. Die aufgelöste Sitzung steht unter
`-v` auf `stderr`; `stdout` bleibt leer, weil es kein Ergebnis gibt.

#### Nachtrag aus dem Review von HUM-066 (2026-09-04)

Sechs Befunde, alle übernommen. Wo dieser Nachtrag dem Text darüber
widerspricht, gilt der Nachtrag.

**Ein Projekt darf mit `name` nur ein mitgeliefertes Profil wählen.** Vorher
setzte der `name` des Projekt-Profils jedes Profil des Nutzers als Ebene 4 ein.
Ein geklontes Repository kam damit über den Umweg an jeden Schlüssel, den ihm
die Projekt-Ebene verwehrt: `.humanitl/profile.toml` mit `name = "loose"`, und
`agent.command` und `sandbox.profile` aus `loose.toml` galten — der Prozess in
der Sandbox und ihre Einhängefläche. `name` wählt jetzt nur unter
`BUILTIN_PROFILES`; ein anderer Wunsch wird übergangen und mit `CONFIG_009`
(Warning) gemeldet. Wer sein eigenes Profil meint, nennt es mit `--profile`.
Damit hat das Projekt-Profil vier Grenzen statt drei, und die erste trägt die
anderen drei.

**Das Projektverzeichnis ist `sandbox.work_dir`, nicht das aktuelle
Verzeichnis.** Vorher wurde das Projekt-Profil immer unter `cwd` gesucht: Mit
`--work` von außen sah niemand das Profil des Projekts, und stand die Shell in
einem feindlichen Repository, wirkte dessen Profil auf eine Sitzung, die
woanders arbeitete. `sources_for` lädt deshalb die Ebenen 1 bis 4 samt Umgebung
und Kommandozeile einmal vorab, nimmt `sandbox.work_dir` (sonst `cwd`) und sucht
erst dort. Zirkelfrei, weil der Schlüssel auf der Projekt-Ebene gesperrt ist.

**Die elfte Abweichung ist zurückgenommen: `--profile` bekommt seine Bedeutung
vom Unterkommando, nicht von der Platte.** Das ist keine Behebung im Rahmen der
alten Regel, sondern deren Verwerfung — wer oben die elfte Abweichung liest,
liest eine Fassung, die nicht mehr gilt.

Verworfen wurde die Regel „gibt es ein Sitzungsprofil dieses Namens, ist es
gemeint; sonst das bwrap-Profil". Sie machte die Bedeutung eines Arguments von
der Anwesenheit einer Datei abhängig, und das hat zwei Folgen, die eine Regel
nicht haben darf. Erstens kippte sie im Betrieb: Legt jemand ein
`profiles/test.toml` an — und `docs/profiles.md` lehrt genau das —, dann meint
`sandbox run --profile test` plötzlich etwas anderes, die `/tests/escape`-
Einhängung verschwindet lautlos und `sandbox.profile` bleibt still auf
`default`. Zweitens verschluckte sie einen Fehler: Ein Profil, das existiert,
sich aber nicht lesen lässt, galt als „kein Sitzungsprofil"; der Aufruf lief mit
Exit 0, ohne Befund und mit der anderen Bedeutung weiter — im Widerspruch zu dem
Satz, den derselbe Code schreibt („Nothing starts while it does not parse").

Es gilt stattdessen: Unter `humanitl sandbox` benennt `--profile` das
bwrap-Profil unter `profiles/sandbox/`, überall sonst das Profil der Sitzung.
Zwei Bedeutungen, eine Zuordnung, und die steht am Kommando. `cmd::ProfileMeaning`
trägt sie, `main::profile_means` trifft sie am `Cmd`. `Context::session`
entfällt; `Context::config` ist der eine, strenge Weg, und
`profile_is_a_session_profile` gibt es nicht mehr.

**`profile_exists` fragt nach der Datei, nicht nach ihrem Inhalt.** Ein Profil,
das sich nicht lesen lässt, ist ein Profil und wird beim Laden zu `CONFIG_001`.

**`discover_with` ist so streng wie `sources_for` und gibt ein `Result`.** Zwei
Auflösungen mit verschiedener Strenge nebeneinander waren die eigentliche
Fehlerquelle; es gibt nur noch eine. Zusätzlich prüft die Stelle, die aus einem
Namen einen Pfad macht (`resolve::layer`), den Namen selbst: Ein `../` erreicht
keine Datei außerhalb des Profilverzeichnisses, unabhängig davon, ob ein Mensch
ihn tippen könnte.

**Die Rangfolge der Herkunft ist ein Band, kein Vergleich.**
`Origin::ProfileBuiltin` und `Origin::ProfileGlobal` teilen sich Rang 2; die
Ränge darüber rücken auf 3, 4, 5. Welche von zwei Ebenen gewonnen hat,
entscheidet der Ebenen-Index, den `load::Merge` mitführt, nicht `rank()`. Vorher
benannte `alias_diagnostics` in der Mischung „eigenes `default.toml` auf Ebene 3
plus eingebettetes `llm-only` auf Ebene 4" den falschen Gewinner und hätte dem
Nutzer gesagt, er solle die falsche Zeile löschen.

**Der Bereich `CONFIG` reicht bis 019.** `CONFIG_009` ist der Befund für einen
übergangenen Profilwunsch des Projekts; mit `007` und `008` wäre der alte
Bereich bis `009` sonst voll gewesen. Die Tabelle `AREAS` und
`docs/DIAGNOSTICS.md` sind nachgezogen.

**Ein Name wird geprüft, wo er entsteht, nicht wo er dasteht.** `Profile::parse`
prüfte `check_name` nur am ausdrücklich gesetzten `name`. Fehlt der Schlüssel —
was erlaubt ist und wozu die Meldung bei einem Namenskonflikt sogar rät —, kam
der Name aus dem Dateistamm und blieb ungeprüft; ein `Work.Profile.toml` trug
seinen Stamm durch Auflösung, Herkunftsanzeige und Profil-Kette. Die Prüfung
steht jetzt hinter der Auswahl, also am Namen, der gilt.

**Eine Liste, die ein kaputtes Profil als brauchbar ausgibt, lädt zum Fehlschlag
ein.** Verdeckte eine unlesbare Datei ein mitgeliefertes Profil, zeigte
`humanitl config schema --profiles` weiter die Einbettung mit `bundled`, obwohl
`resolve::layer` die Datei nimmt und jeder Aufruf mit `CONFIG_001` endet.
`ProfileSummary` trägt deshalb `broken`, und ein Fehlschlag bekommt seine Zeile
an derselben Stelle wie ein Erfolg; die Kommandozeile schreibt `(does not load)`
hinter die Herkunft. Die Befunde stehen weiterhin daneben.

### 4.24 Aus der Umsetzung des Meta-Endpunkts (HUM-073, 2026-09-04)

Abweichungen von `backlog/sprint-3.md`, die dauerhaft gelten. Wo die
Spezifikation anderes sagt, gilt dieser Abschnitt.

**`FlowEvent::AgentAsk` liegt in `event.rs`, nicht in `flow.rs`.** Die
Spezifikation nennt `daemon/crates/core-types/src/flow.rs`; der Ereignistyp
wohnt aber seit HUM-004 in `event.rs`, und `flow.rs` trägt den
Zustandsautomaten. Die Variante trägt `ask_id`, `at`, `text` und
`suggested_host` — nicht `{ text, ts }` wie im Ziel des Issues —, weil die
Proto-Nachricht `FlowEvent.AgentAsk` diese drei Felder schon vorsah. `AskId`
ist eine neue typisierte Id in `ids.rs`: Eine Bitte ist kein Flow, hat aber
eine Kennung, damit die Oberfläche zwei gleichlautende Bitten auseinanderhält.

**Die Weiche liegt hinter der Authority-Prüfung und vor allem anderen.** Sie
steht in `FlowHandler::handle_request` unmittelbar nach
`connect::check_authority` und vor dem Puffern des Bodys, den Detektoren, der
Regelauswertung und jeder Namensauflösung. Hinter der Prüfung, damit sie auf
dem geprüften Ziel arbeitet: Ein `CONNECT github.com:443` mit
`Host: humanitl.internal` darin ist ein Widerspruch und wird geblockt, nicht
beantwortet; sonst wäre die Weiche über eine Kopfzeile steuerbar. Über CONNECT
zum reservierten Namen selbst kommt die Anfrage über denselben Weg an.

**Der reservierte Name ist der Name, nicht ein Dienst auf einem Port.**
`is_meta_host` vergleicht den normalisierten `HostName`. `HUMANITL.INTERNAL`
und `humanitl.internal.` sind derselbe Name und lösen die Weiche aus — genau
dafür normalisiert `HostName::parse`. Jeder Port desselben Namens gehört
ebenfalls dem Endpunkt: Ginge `humanitl.internal:8080` durch die Regeln, könnte
eine Freigabe dafür eine Namensauflösung auslösen, und ADR-014 schließt aus,
dass der Name je aufgelöst wird. Ein Name, der nur so *aussieht* —
`evil-humanitl.internal`, `sub.humanitl.internal`,
`humanitl.internal.evil.io` —, ist ein gewöhnlicher Host und läuft durch die
Regeln.

**Meta-Anfragen erzeugten zunächst keinen Flow; seit HUM-103 tun sie es, ohne
Entscheidung.** Die Spezifikation von HUM-073 wollte sie „im Recorder als Flow
mit `state=Recorded`, `decision=meta`" sehen. Der Zustandsautomat kannte keinen
Weg von einer Nicht-Sperre nach `Recorded`, und eine erfundene Entscheidung wäre
eine Behauptung über einen Menschen, der nichts getan hat (4.13); bis HUM-103
entstand für `/` und `/why` deshalb gar kein Flow, und sichtbar wurde allein
`/ask` als `FlowEvent::AgentAsk` und als Karte. Seit HUM-103 gibt es die Zeile,
und sie trägt **kein** `decision=meta`, sondern die Spalte `flows.meta` neben
der Entscheidung; wie das gebaut ist, steht in 4.27.

**Das Ratenlimit ist ein gleitendes Fenster.** Zehn angenommene Bitten je
Sitzung in sechzig Sekunden, gezählt über die Zeitpunkte der angenommenen
Bitten. Ein fester Minutenzähler ließe zwanzig Bitten in zwei Sekunden durch,
wenn sie um die Grenze herum liegen. Eine abgelehnte Bitte belegt keinen Platz,
sonst sperrte sich ein Agent, der einmal zu schnell war, dauerhaft selbst aus —
und **die Reihenfolge der Prüfungen in `/ask` gehört zu dieser Zusage**: erst
die Länge des Rumpfes, dann Säuberung und Leerprüfung, und erst danach der
Platz im Fenster. Andersherum verbrauchten zehn leere Rümpfe das Fenster, und
die Grenze wäre eine Waffe gegen den, den sie schützt. Die Fenster werden bei
jedem Zugriff gegen `now` beschnitten und danach die leeren weggeworfen, in
dieser Reihenfolge; sonst bliebe das Fenster einer Sitzung, die nicht mehr
fragt, für immer nicht-leer und die Tabelle wüchse über die Laufzeit des
Daemons (`MetaEndpoint::tracked_sessions` macht das prüfbar). Die Zeit kommt
aus `MetaClock`, nie aus der Wanduhr; die Antwort `429` trägt `Retry-After`.

**Was `/` zeigt und was nicht.** Eine Zeile je Regel mit Aktion, Methoden,
Host (mit Port, wenn die Regel einen verlangt), Pfad und Ablauf, dazu die
Vermerke `(llm passthrough)` und `(bundled)`; die letzte Zeile ist der Ausgang
ohne Treffer (`ask`, Spalte `default`). Nie Notiz, `created_from`, Regel-Id
oder Position. Abgeschaltete, abgelaufene und fremde Sitzungsregeln stehen
nicht in der Liste: Sie entscheiden nichts. Die Reihenfolge ist die von
`RuleSet::evaluate` — erst die Regeln dieser Sitzung, dann alle übrigen. Jedes
Feld läuft durch `sanitize_note`, weil ein Pfadmuster aus `rules.yaml` sonst
eine zweite Zeile in die Ausgabe schreiben könnte.

**`/why/<id>` antwortet nur für Flows derselben Sitzung.** Ein Flow einer
anderen Sitzung wird behandelt, als gäbe es ihn nicht (`404`), ebenso eine
unlesbare Id. `note=` trägt die Notiz **der Entscheidung**
(`Decision::Block { note }`, HUM-072), nie die Notiz einer Regel; sie steht als
letztes Feld der Zeile, weil sie das einzige mit Leerzeichen ist. Ein noch
nicht entschiedener Flow antwortet `decision=pending reason=<zustand>`.

**Der Vorschlag für das Regel-Blatt entsteht im Daemon, und er trägt den
Pfad.** `suggested_target` sucht die **erste** Fundstelle von `http://` oder
`https://` im gesäuberten Text, wirft Benutzerangaben vor einem `@` und den
Port weg und lässt das Ergebnis durch `HostName::parse`. Vorgeschlagen wird nur
ein DNS-Name mit mindestens einem Punkt; eine Adresse, ein einzelnes Label
(`https://ein Satz` liest sich als Host `ein`) und `humanitl.internal` selbst
ergeben keinen Vorschlag. Ist die erste Fundstelle kaputt, wird nicht
weitergesucht: Sonst lenkte ein Text den Blick des Menschen auf die eine und
den Vorschlag auf die andere URL.

Der Pfad derselben URL reist als `suggested_path` mit (Proto-Feld 4 in
`FlowEvent.AgentAsk`) und ist der Grund, warum es das Feld gibt: **Ohne ihn
wäre eine Regel aus einer Bitte eine Freigabe für jeden Pfad und jede Methode
des Hosts, während der Agent nach genau einer Adresse gefragt hat.** Query und
Fragment fallen weg (ein Glob vergleicht Pfad *und* Query), ein `*` aus dem
Text des Agenten macht den Vorschlag ungültig statt weit, alles außerhalb der
sichtbaren ASCII-Zeichen fällt aus, und `/` allein zählt als „kein Pfad". Genau
dieser Fall ist der, den das Blatt ausdrücklich benennen muss.

**Die Karte steht über der Warteschlange, nicht in ihr.** `AgentAskStrip` hängt
im `QueuePane` zwischen Kopfzeile und `AnimatedList`. Eine Bitte ist keine
angehaltene Anfrage; als Zeile in der Liste könnte sie mit einer verwechselt
werden, und sie hätte Anteil an Auswahl, Gruppierung und Einfrieren, die alle
eine `FlowId` voraussetzen. Der Text wird als `Text` gezeichnet, nie als
`Text.rich` und nie als Verweis, in der Schreibmaschinenschrift und unter dem
Abzeichen „Agent" im Violett der Durchreiche: Er ist Zitat, keine Meldung des
Programms.

**Das Regel-Blatt der Intercept-Seite ist klein und eigen.** Ein „HUM-028-Sheet"
gibt es nicht: HUM-028 ist der `RememberGrid` in der Aktionsleiste, und der
Editor der Regel-Seite ist von `features/intercept` aus unerreichbar, weil eine
Feature-Schicht keine andere importiert (`tools/check-deps.sh`). Die Karte
öffnet deshalb ein eigenes `HSheet` mit Host, Pfad, Aktion und Dauer. Es legt
nichts an, bis der Mensch den Knopf drückt, und beide Felder bleiben änderbar —
die Vorschläge stammen aus Text, den der Agent geschrieben hat.

**Keine Vorauswahl bei der Aktion.** Das eine Feld, das entscheidet, ob Verkehr
fließt, wird von Hand gewählt; der Knopf bleibt aus, bis das geschehen ist. Ein
vorgewähltes `allow` machte aus einem bestätigenden Klick einen Netzzugang, der
aus der Bitte eines Programms entstand, dem wir nicht trauen. Ein vorgewähltes
`ask` wäre die andere Falle: Es schriebe eine Regel, die nichts ändert — `ask`
gilt ohnehin ohne Regel —, und der Mensch ginge in dem Glauben weg, er habe
etwas eingerichtet (4.13). Ist das Pfad-Feld leer, steht im Blatt der Satz, dass
die Regel jede Methode und jeden Pfad des Hosts abdeckt.

**Nichts wird rechts abgeschnitten.** Der vorgeschlagene Host stand mit
`TextOverflow.ellipsis` in der Karte; aus `pypi.org.attacker.com` wurde
`pypi.org…`. Das ist Domain-Täuschung durch die eigene Oberfläche, genau in dem
Augenblick, in dem ein Mensch entscheidet. Der Name bricht jetzt um. Die
registrierbare Domäne wird auch nicht hervorgehoben: `app/lib/features/intercept/psl.dart`
rät sie aus einer kurzen Tabelle, und ein falsch geratener Apex wäre dieselbe
Täuschung mit umgekehrtem Vorzeichen.

**Gestapelte kombinierende Zeichen werden begrenzt, und die Karte clippt
trotzdem.** `sanitize_note` lässt höchstens `MAX_COMBINING_MARKS` (zwei)
kombinierende Zeichen je Basiszeichen stehen; sechzig Akzente auf einem `a`
laufen sonst über alles, was daneben steht. Die Prüfung `is_combining` deckt die
reinen Kombinationsblöcke ab und ist **keine** vollständige Prüfung der
Eigenschaft `Mn` — eine vollständige bräuchte eine Bibliothek und damit einen
Eintrag in `daemon/Cargo.toml`. Weil sie unvollständig ist, deckelt die Karte
zusätzlich ihre Zeilen (`agentAskMaxLines`) und beschneidet ihren Rahmen; das
gilt für jedes Zeichen, gleich aus welchem Block.

**Ein Fehlschlag beim Anlegen bleibt sichtbar.** Scheitert `AddRule`, steht der
Satz des Daemons im Blatt und der Entwurf bleibt stehen. Ein still gescheitertes
Anlegen ist schlimmer als eines, das nie versucht wurde: Der Mensch geht in dem
Glauben weg, die Regel gebe es. Der Host wird vorher mit `hostPatternProblem`
geprüft, damit ein untauglicher Vorschlag gar nicht erst hinausgeht.

### 4.17 Aus der Umsetzung des Sandbox-Bildschirms (HUM-040, 2026-09-04)

Abweichungen von `backlog/sprint-3.md`, die dauerhaft gelten. Wo die
Spezifikation anderes sagt, gilt dieser Abschnitt.

**Die Einhängetabelle wird aus der Kommandozeile gelesen, nicht daneben
geführt.** `daemon/crates/ipc/src/sandbox.rs` baut den Argumentvektor mit
`SandboxProfile::to_bwrap_args` und liest die Tabelle anschließend aus genau
diesem Vektor (`--ro-bind`, `--bind`, `--ro-bind-data`, `--tmpfs`, `--proc`,
`--dev`, `--symlink`, bis zum ersten `--`). Eine zweite Buchführung aus den
Abschnitten des Profils wäre die naheliegende Umsetzung und hätte den Fehler,
den dieser Bildschirm nicht haben darf: Sie könnte von der Zeile abweichen, die
wirklich startet. So ist ein Mount, der in der Zeile steht, zwangsläufig auch in
der Tabelle. Die Spalte „Woher" ist die einzige Ableitung daneben; sie ist eine
Zuordnung des Ziels und keine zweite Quelle.

**Werte der Umgebung gehen nach einer Erlaubnisliste, nicht nach einer Liste
verdächtiger Namen.** Die Spezifikation sieht einen Reveal-Toggle vor, der den
maskierten Wert zeigt, und nennt als Regel die Endungen `_TOKEN`, `_KEY`,
`_SECRET`, `PASSWORD`. Beides gilt nicht mehr.

Die Endungsregel ist fail-open, und ihre Lücken sind die gefährlichen:
`AWS_ACCESS_KEY_ID` endet auf `_ID`, `DATABASE_URL` trägt das Passwort in der
URL, `GH_PAT`, `AUTHORIZATION` und `APIKEY` heißen nach gar nichts. Sie wäre
auch nicht zu schließen: Zwei der drei Quellen der Umgebung sind offen und vom
Menschen geschrieben — das `[env]` eines eigenen Profils und `sandbox.env` aus
`config.toml`, und genau dort landet später die Zugangsdaten-Injektion. Über
einem offenen Namensraum kann keine Namensregel vollständig sein.

`humanitl_ipc::sandbox::VISIBLE_ENV` dreht die Richtung um: Gezeigt wird nur,
was der Bildschirm als Beleg braucht — wohin der Agent darf (Proxy), wem er
glaubt (Zertifikate), wo er steht (Pfade, Sprache, XDG) und was ihn steuert (die
fünf Variablen des Shims, `HUMANITL_SESSION`, die zehn des Adapters). Jeder
andere Wert bleibt zurück, auch ein harmloser. Damit ist eine neue Variable
stumm statt versehentlich sichtbar, und die Vorgabe steht auf der sicheren Seite
(4.13). Die Namen des Shims und des Adapters stehen als Konstanten und nicht als
Text; wer eine umbenennt, bricht den Bau, statt sie still von der Liste zu
nehmen. Kein Präfix: `OPENCODE_*` als Muster hieße, dass ein künftiges
`OPENCODE_API_KEY` von selbst sichtbar wäre. Das Feld heißt deshalb
`EnvVar.withheld` und nicht `secret`: Der Daemon behauptet nicht, den Wert
beurteilt zu haben; er hat ihn nur nicht gebraucht.

Aufgedeckt wird nichts. Die Oberfläche hat den Wert gar nicht, und ein Control,
das es verspräche, wäre eine Lüge auf dem einen Bildschirm, der geglaubt werden
muss. Die Hälfte der Zusage, die bleibt, ist die wichtigere: Ein
zurückgehaltener Wert sieht nie aus wie ein leerer. Er steht als Punkte plus das
Wort dafür in der Fehlerfarbe, eine wirklich leere Variable sagt „leer" im
Klartext, und ein Satz über der Tabelle sagt, wonach gezeigt wird. Wer den Wert
prüfen will, prüft ihn dort, wo er herkommt.

**Die angezeigte Kommandozeile schwärzt dieselben Werte wie die Tabelle.** Die
Zeile enthält `--setenv <KEY> <VALUE>` für jede Variable der Sandbox; ein
Geheimnis, das die Umgebungstabelle zurückhält und die Zeile daneben
ausschreibt, wäre zurückgehalten nur dem Namen nach — und die Zeile ist genau
das Stück dieses Bildschirms, das ein Mensch in die Zwischenablage legt.
`redact_hidden_values` setzt deshalb `<withheld>` an die Stelle jedes Werts, den
auch die Tabelle zurückhält — dieselbe Regel und keine zweite daneben, sonst
zeigte die Tabelle Punkte und die Zeile den Wert. Das ist zulässig, weil die
Zeile Anzeige und
nie Ausführung ist: Gestartet wird die Liste, die der Launcher beim Start selbst
baut (`SandboxProfile::to_bwrap_args`). Die Einhängetabelle bleibt unberührt,
weil `--setenv` hinter `--clearenv` steht und damit hinter allem, was sie liest.
Gefunden hat das der Test `a_value_whose_name_ends_in_token_never_leaves_the_daemon`.

**Ein erlaubter Name sagt nichts über den Wert darunter; die Herkunft
entscheidet mit.** Die Erlaubnisliste allein löst nur die eine Hälfte: neue
Namen sind stumm. Die andere Hälfte ist, dass `HTTP_PROXY` auf der Liste steht
und in `sandbox.env` trotzdem `http://nutzer:passwort@host` lauten kann. Der
Dienst führt deshalb die Herkunft durch die Zusammenführung mit — mitgeliefertes
Profil, Adapter, Sitzung, und `User` für alles, was ein Mensch geschrieben hat
(`[env]` eines eigenen Profils, `sandbox.env`) — in genau der Reihenfolge, in
der `effective_env` zusammenführt, letzter Schreiber gewinnt. Gezeigt wird ein
Wert nur, wenn **beides** gilt: Name auf der Liste **und** Herkunft nicht
`User`. Eine Herkunft, die sich nicht zuordnen ließ, zählt als `User`; eine
unbekannte Quelle ist der Fall, in dem die Vorgabe zurückhalten muss. Folge, die
man kennen muss: Wer ein eigenes Sandbox-Profil schreibt, sieht dessen Werte
nicht mehr im Bildschirm — Namen und Herkunft stehen weiter da. Das ist gewollt;
sein Profil ist die Quelle, die er selbst lesen kann.

**Was über den Socket hereinkommt, wird geprüft, bevor es irgendwo landet.**
`Plan` und `Start` tragen Profilname und Projektverzeichnis. Beides bestimmt,
was die Sandbox einhängt; der Socket ist die Vertrauensgrenze, nicht die
Oberfläche.

- **Der Profilname ist ein Name, kein Pfad.** Ohne Prüfung machte
  `format!("{name}.toml")` aus `/tmp/evil` den Pfad `/tmp/evil.toml`, denn
  `Path::join` ersetzt die Basis, sobald das Angehängte absolut ist, und `..`
  liefe aus dem Suchpfad heraus. Geprüft wird mit
  `humanitl_config::profile::check_name`, derselben Regel wie für die Profile
  der Sitzung, und an der Stelle, die aus dem Namen einen Pfad macht.
- **Das Projektverzeichnis kommt aus dem Heimatverzeichnis.** Drei Stufen: es
  ist absolut, ohne `..`, vorhanden und nach dem Auflösen der Symlinks ein
  Verzeichnis; es besteht `MountPolicy::check_work_dir`; und es liegt unter
  `$HOME` oder ist genau das Verzeichnis aus `sandbox.work_dir`. Die dritte
  Stufe ist nötig, weil die Denylist `/etc`, `/usr` und `/var/lib` nicht kennt —
  ein Bildschirm, der „der Agent sieht nur dein Projekt" verspricht und `/etc`
  zeigt, hat die Zusage gebrochen. Wer ein Projekt außerhalb des
  Heimatverzeichnisses hat, schreibt es in `config.toml`; dort steht die
  Erklärung eines Menschen und nicht der Wunsch eines Clients.
- **Gemerkt wird nur ein Wunsch, der ganz durchkommt.** Ein halb übernommener
  hinterließe einen Zustand, den niemand gewählt hat, und die Momentaufnahme
  nach einem abgelehnten Wunsch zeigt den alten Stand, nie den abgelehnten.

**Der Zustand kommt vom gehaltenen Handle, `agent_running` von der Lebendigkeit
des Kindes.** Eine Sitzung, deren Agent sich beendet hat, läuft weiter: Sie hat
eine Kennung, eine Startzeit und einen Stopp, der noch etwas tut. Beides zu
vermengen meldete `stopped` mit einer Startzeit daneben.

**Das Ziel eines Verweises ist kein Wirtspfad.** `Mount.src` heißt in der
Oberfläche „auf diesem Rechner"; bei `--symlink` stand dort das Sprungziel, also
ein Pfad **in** der Sandbox. `Mount.link_target` (Feld 5) trägt es jetzt, `src`
bleibt leer, und die Tabelle schreibt „Verweis auf {target}" in die Spalte, die
die Art nennt.

**Der gewählte Projektordner gilt für die Sitzung, nicht für `config.toml`.**
Die Spezifikation schreibt ihn über `configProvider.set(...)` fest. `SetConfig`
antwortet bis HUM-069 mit `UNIMPLEMENTED`, und ein zweiter Schreibweg neben dem
Einstellungs-Bildschirm wäre teurer als die Abweichung. Der Wunsch reist
stattdessen in `SandboxRequest.Plan` und `SandboxRequest.Start` mit; der Dienst
merkt ihn sich für die laufende Sitzung, und die Rangfolge ist: was läuft, vor
dem, was gewählt wurde, vor dem, was konfiguriert ist. Mit HUM-069 wandert die
Wahl in die Konfiguration, und diese Merkstelle entfällt.

**`Sandbox(Plan)` ist die neue Operation, mit der die Oberfläche fragt, bevor
sie handelt.** Sie liefert dieselbe Momentaufnahme wie `Status` — Einhängungen,
Umgebung, Kommandozeile —, aber für ein Projektverzeichnis, das noch nicht gilt.
Ohne sie müsste die Oberfläche den Satz „Der Agent sieht nur `/work` = …" selbst
zusammensetzen, also genau die Fachlogik führen, die ADR-018 im Daemon haben
will.

**Der Isolations-Reiter und das Terminal sind erklärte Platzhalter.** Beide
sagen, was dort stehen wird und was bis dahin an ihrer Stelle den Beleg trägt
(die Kommandozeile). Ein leerer Kasten liest sich als Fehler, und ein halbes
Terminal wäre schlimmer als keines (HUM-041, HUM-042).

**Der geteilte Aufteiler zwischen Terminal und Reitern entfällt vorerst.**
`HResizablePanes` teilt nur waagerecht; ein senkrechter Aufteiler gehört in
`app/packages/ui` und nicht in dieses Feature. Die Aufteilung steht deshalb fest
bei 60 zu 40 — derselben Aufteilung, die die Spezifikation als Startwert nennt,
damit die Panes nicht springen, sobald HUM-042 das Terminal füllt.

**Was `app/packages/ui` für diesen Bildschirm fehlt.** Ein Reiter-Streifen, eine
Tabelle, ein Zustandspunkt, ein senkrechter Aufteiler und markierbarer Text ohne
Material. Alle fünf stehen vorerst in `app/lib/features/sandbox`, gebaut aus den
Teilen des Pakets (`HButton`, `HAnimatedFill`, `HHairline`, die Token) und ohne
einen einzigen Import von `package:shadcn_flutter`.

### 4.25 Aus der Umsetzung des Leser-Registers (HUM-101, 2026-09-05)

**`limits.idle_timeout_secs` ist entfernt; die Leerlaufgrenze der Verbindung
zum Agenten heißt `limits.header_timeout_secs`.** Das ist die erste
Entscheidung von HUM-101, und sie fiel gegen den Einbau, aus vier Gründen:

1. Auf einer Keep-Alive-Verbindung beschreiben beide Schlüssel dieselbe Spanne.
   `header_read_timeout` in `hyper` deckt beides ab, das Eintreffen der
   Kopfzeilen und die Lücke bis zur nächsten Anfrage; die Uhr ist genau so
   lange gespannt, wie die Verbindung auf einen Anfragekopf wartet.
2. Auf **dieser** Spanne war der zweite Schlüssel unerreichbar: 90 Sekunden
   Leerlauf gegen 30 Sekunden Kopf-Frist, die kürzere Uhr läuft immer zuerst
   ab. Das gilt nur für Kopf und Keep-Alive-Lücke; auf den Spannen unten wäre
   er die einzige Uhr gewesen.
3. Auf zwei Spannen hätte er genau das getroffen, was er verschonen soll. Der
   Hold sitzt in der Service-Future innerhalb `serve_connection`; solange er
   läuft, fließen auf der Verbindung des Agenten null Bytes. Dasselbe gilt für
   den streamenden Antwort-Rumpf: Vom Client kommt nichts, während das
   Sprachmodell antwortet. Eine Uhr über der ganzen Verbindung kann diese
   Stille nicht von der eines hängenden Clients unterscheiden — sie hätte mit
   den Vorgaben jeden gehaltenen Fluss nach 90 Sekunden getötet, obwohl die
   Haltefrist 300 Sekunden beträgt, und dazu jede schweigende Durchreiche.
   Nachgemessen: Eine naive Leerlaufuhr vor `serve_connection` macht
   `a_held_request_survives_the_header_timeout` und
   `a_streaming_passthrough_survives_the_header_timeout` rot und
   `an_idle_connection_does_not_survive_the_header_timeout` grün
   (`daemon/crates/proxy/tests/timeouts.rs`).
4. Weil kein zweiter Schlüssel bleibt, braucht es auch keinen Wrapper vor
   `serve_connection` und keinen Zähler gehaltener Flüsse im
   `ConnectionContext`: Es gibt keine Uhr, die anzuhalten wäre. Was stattdessen
   nötig ist, sind Grenzen **je Spanne** mit eigenen Namen. Eine Uhr über der
   ganzen Verbindung ist der falsche Zuschnitt, nicht der falsche Wert.

**Drei Spannen bleiben damit unbewacht, und HUM-120 trägt sie.** Sie stehen
hier, damit die Entfernung nicht als Schließen einer Lücke gelesen wird, die
sie offenlässt:

- Der **Anfrage-Rumpf** (`handler.rs:597`, `body::buffer` ohne Frist). Hypers
  Kopf-Uhr ist gelöscht, sobald der Kopf geparst ist; ein Client, der
  `Content-Length` ankündigt, zehn Bytes schickt und schweigt, hält die
  Verbindung unbegrenzt. Es greift allein `limits.hold_body_cap_bytes`, eine
  Byte-Grenze.
- Der **gestreamte Antwort-Rumpf** (`body.rs`, `TeeBody`). Bis zu den
  Antwort-Kopfzeilen deckt der `handshake_timeout` des Upstreams alles ab,
  danach nichts.
- Der **TLS-Handschlag nach `CONNECT`** (`handler.rs:337`, `tls::accept` ohne
  Frist). Wer den Tunnel öffnet und nie ein `ClientHello` schickt, hält den
  Task für immer; die Kopf-Frist des inneren `serve_connection` beginnt erst
  danach.

`limits.body_timeout_secs` speist in HUM-120 die beiden Rumpf-Spannen,
`limits.header_timeout_secs` den Handschlag. Beide Rumpf-Grenzen begrenzen die
Stille **zwischen zwei Stücken** und nicht die Gesamtdauer; das ist eine
Umdeutung des heutigen Doku-Kommentars und gehört samt `docs/CONFIG.md` und
Abschnitt 4.4 in denselben Commit.

Ein `reason: idle_timeout` ist nie entstanden und entsteht auch nicht: Im
Leerlauf ist keine Anfrage in Flug, es gibt also keine Antwort, in die ein
Banner passen würde; der Leerlauf schließt still und meldet sich im Protokoll.

**Ein entfernter Schlüssel warnt, er scheitert nicht.** Das gilt für jeden
Schlüssel, den wir selbst streichen, also auch für
`experimental.upstream_port_map` in HUM-088. Die Regel:

- Der Pfad kommt in `alias::RETIRED` (`daemon/crates/config/src/alias.rs`), mit
  dem Issue und einem Satz Grund. Kein Alias — es gibt kein Ziel, auf das er
  zeigen könnte.
- Das Laden übergeht ihn und legt `CONFIG_005` mit `Severity::Warning` dazu:
  Schlüssel, Ebene, Issue, Grund, und die Aufforderung, die Zeile zu löschen.
  Der Wert erreicht die Konfiguration nicht, der Daemon startet.
- Ein unbekannter Schlüssel, der **nicht** in `RETIRED` steht, bleibt der harte
  `CONFIG_002` von vorher. Die Milde gilt genau den Pfaden der Liste.

Der Grund für die Ausnahme: Ein `CONFIG_002` sagt „du hast dich vertippt". Wer
`limits.idle_timeout_secs` in seiner Datei stehen hat, hat sich nicht vertippt
— die Datei war gestern gültig, und die Entscheidung, den Schlüssel zu
entfernen, haben wir getroffen. Ein Update, das den Daemon nicht mehr starten
lässt, verlangt vom Nutzer die Reparatur einer Änderung, die er nicht
veranlasst hat. Stilles Übergehen wäre die andere Hälfte desselben Fehlers und
genau das, was dieses Issue behebt: ein Schlüssel, der dasteht und nichts tut.
`docs/CONFIG.md` führt die entfallenen Schlüssel in einer eigenen Tabelle, damit
der Text der Warnung nicht der einzige Ort ist, an dem sie noch vorkommen.

Eine `FixAction` trägt der Befund nicht: Es gibt keinen Nachfolger, auf den ein
Knopf zeigen könnte, und `ChangeSetting` auf einen anderen Schlüssel wäre eine
Empfehlung, die niemand geprüft hat. Was zu tun ist, steht im Satz.

**Das Register der Leser steht in `daemon/crates/config/tests/config_readers.rs`.**
Eine Zeile je Blattpfad des Schemas, genau eine Einstufung: `effective` oder
`pending(HUM-xxx)`. Sein Test liest `schema::leaf_paths()` und wird rot, sobald
das Schema einen Pfad kennt, den das Register nicht kennt, sobald das Register
einen Pfad nennt, den es nicht mehr gibt, und sobald eine Einstufung von der
Angabe am Feld abweicht. Die Angabe am Feld ist `x-pending-issue` in
`src/model.rs`, gebaut wie `x-tier` und `x-project-scope`; aus ihr schreibt der
Generator die Spalte „Wirkung" in `docs/CONFIG.md`. Ein Eintrag `pending` muss
ein Issue nennen, das `BACKLOG.md` als Zeile führt — ein Verweis ins Leere ist
schlechter als keiner.

**Wo das Register aufhört, sagt sein Kopf.** Eine Zeile deckt einen Blattpfad,
und Blätter findet der Durchlauf über `properties`. Die Schlüssel *in* einer
freien Tabelle (`sandbox.env`, `resolver.overrides`,
`experimental.upstream_port_map`) und die Elemente einer Liste sind deshalb
nicht einzeln erfasst; der Behälter trägt die Zeile, und seine Einstufung gilt
für alles darin. Das reicht, solange darin Skalare stehen, und genau das prüft
`the_schema_hides_no_leaf_from_the_walk`: eine Tabelle oder Liste von
Strukturen macht ihn rot, ebenso `allOf`, `anyOf` und `$ref`.
`#[serde(flatten)]` ist dabei kein Loch — nachgemessen: `inline_subschemas`
schmilzt die Felder in die `properties` des Elternknotens, sie erscheinen als
gewöhnliche Blätter, und der Vollständigkeitstest nennt sie beim Namen. Der
Riegel gegen `allOf` bleibt für den Fall, dass jemand `inline_subschemas`
abschaltet. Aliase stehen neben dem Schema; `every_alias_leads_to_a_registered_key`
hält fest, dass jeder auf ein registriertes Blatt zeigt und selbst keines ist.

Ein Gate über Feldnamen aus der Shell wäre untauglich gewesen und ist deshalb
nicht gebaut: `env` trifft im Repository 244-mal, `profile` 464-mal,
`timeout_secs` ist Teilzeichenkette von vier Schlüsseln, `enabled` ist zweimal
vergeben, ein Doku-Kommentar zählt als Treffer, und `humanitl config get` liest
über serde ohnehin jeden Schlüssel. `effective` ist deshalb die Behauptung
eines Menschen und keine Messung. Das ist Absicht: Das Register soll den
**vergessenen** Schlüssel finden; wer beim Anlegen „wirkt" schreibt, ohne zu
verdrahten, hat nicht übersehen, sondern gelogen.

Die Zählung hat drei Fälle mehr gefunden, als HUM-101 nannte:
`resolver.nameserver` (der Daemon warnt beim Start und fragt trotzdem
`/etc/resolv.conf`; HUM-115 baut den Hickory-Adapter dahinter), `ui.theme`
(dieselbe fehlende Naht wie bei `ui.notifications`: dem Client fehlt
`GetConfig`, deshalb beide HUM-069) und `resolver.test_ca`, das HUM-087 bereits
als eigenes Issue führt. Genau dafür ist das Register da. Neu angelegt wurden
nur zwei Issues: HUM-120 für die Rumpfgrenze und HUM-121 für `ui.sound` und
`experimental.ws_hold`.

**Der IPC-Stapel bleibt von den Zeitgrenzen ausgenommen.** `limits.*` speist
über `ipc/src/server.rs` denselben Verbindungsstapel; eine symmetrische
Verdrahtung einer Leerlaufgrenze dorthin schnitte den Ereignisstrom der
Oberfläche ab, der minutenlang stumm sein darf.

**Warum die Grenzen nicht in diesem Commit stehen.** Alle drei Spannen werden
an Stellen bewacht, die in `daemon/crates/proxy/src/handler.rs` liegen — an
dieser Datei wurde parallel gearbeitet. `limits.body_timeout_secs` trägt
deshalb bis auf Weiteres `pending(HUM-120)` statt einer Zusage, und
`docs/SECURITY.md` nennt ihn ausdrücklich als heute wirkungslos, statt ihn in
einer Zeile mit den drei wirksamen Grenzen zu führen.
### 4.26 Aus der Umsetzung von `humanitl run` (HUM-067, 2026-09-05)

Abweichungen von `backlog/sprint-3.md`, die dauerhaft gelten. Wo die
Spezifikation anderes sagt, gilt dieser Abschnitt. Der Zuschnitt dieses
Issues — was gebaut wurde und was nicht — steht als Stand-Abschnitt unter der
Spezifikation.

**Die Sitzungskonfiguration ist der Rumpf, und sie steht an zwei Stellen.**
`humanitl_ipc::session::SessionResolver` löst je Start neu auf;
`humanitl_proxy::session::SessionSettings` hält, was der laufende Proxy davon
liest. `SandboxService::new` nimmt deshalb den Resolver statt einer
eingefrorenen `Config`, und `Inner::config` steht hinter einem `RwLock`: Was
der Sandbox-Bildschirm zeigt, ist die Sitzung, die läuft, und nicht die
Auflösung des Daemon-Starts.

Die Trennung ist eine Aussage. Die Regeln haben mit dem `RulesStore` schon
einen Ort, der sich ändern lässt und Zuhörer benachrichtigt; sie bekommen
deshalb keine zweite Stelle daneben, sondern `RulesStore::set_bundled`. Alles
andere aus der Konfiguration ändert sich innerhalb eines Daemons nicht: Wer
Grenzen, Detektoren oder Resolver anders will, startet ihn neu.

**Ein Client darf zwei Konfigurationspfade setzen, und die Liste ist eine
Erlaubnisliste.** `humanitl_ipc::session::SESSION_OVERRIDE_KEYS` nennt
`llm.endpoint` und `hold.timeout_secs`; jeder andere Pfad ist `CONFIG_003`.
Die Richtung ist dieselbe wie bei `VISIBLE_ENV` (4.17) und aus demselben Grund:
Über einem offenen Namensraum kann keine Sperrliste vollständig sein, und die
Lücken wären die gefährlichen. `sandbox.profile` bestimmt die Einhängefläche,
`agent.command` den Prozess darin, `sandbox.env` seine Umgebung,
`resolver.overrides` und `experimental.*`, wohin der Verkehr wirklich geht,
`findings.enabled`, ob vor einer Entscheidung überhaupt noch gesucht wird, und
`recorder.retention_days`, wie lange die Aufzeichnung sie belegt. Keiner davon
gehört einem Prozess am Socket.

Die beiden erlaubten stehen aus verschiedenen Gründen darauf.
`hold.timeout_secs` vergrößert nichts: Eine kürzere Frist blockt früher, eine
längere lässt nur den Menschen länger warten, und null ist nicht einstellbar.
`llm.endpoint` **vergrößert sehr wohl etwas** — `AgentAdapter::llm_passthrough`
baut daraus eine Regel in Rang 1 mit `allow_private`, die nicht gehalten wird
und die Block-Regeln des Nutzers überholt. Er steht trotzdem darauf, weil 3.8
ihn als Flag von `humanitl run` führt und weil seine Wirkung an drei Stellen
sichtbar ist: als eigene Regel in der Liste, unter `http://humanitl.internal/`
und in der Aufzeichnung. Dazu meldet der Start `LLM_006`, wenn der Endpunkt
nach seinem Namen nicht im eigenen Netz liegt
(`humanitl_proxy::not_private_by_name`, ohne Auflösung — ein Name verlässt den
Rechner erst nach einer Freigabe, ADR-006).

**Eine Profilregel kann sich den Rang der Durchreiche nicht selbst ausstellen.**
`set_bundled` stempelt auf jede Regel der mitgelieferten Gruppe `bundled`, und
Rang 1 gilt für `bundled && passthrough_llm`. `humanitl_rules::parse_rules`
verwirft `bundled` aus einer Datei, `passthrough_llm` aber nicht — seit
Profilregeln in dieser Gruppe stehen, genügte deshalb ein globales Profil mit
`[rules].inline` und `passthrough_llm = true`, um für einen beliebigen Host
einen ungehaltenen Weg zu öffnen, der die Block-Regeln des Nutzers überholt.
`humanitl_ipc::session::BundledRules` trennt die Durchreiche deshalb im Typ:
Ihr Konstruktor nimmt jeder anderen Regel den Vermerk und meldet den Entzug.
Nur was aus `llm_passthrough_rule` kommt, trägt ihn (4.5, HUM-104).

Profil, Projektverzeichnis, Arbeitsmodus, Frage-Modus und Befehl reisen
**nicht** in dieser Liste, sondern in eigenen Feldern von `Start` mit je
eigener Prüfung. Zwei Wege zu einem Feld wären zwei Regeln, welcher gewinnt —
dieselbe Begründung, mit der `cli.rs` `--ro` neben `--work-mode` ablehnt.

**Das Projektverzeichnis reist geprüft weiter, nie geschrieben.** `Start`
trägt es als Text, `SandboxService::remember` prüft es nach den drei Stufen aus
4.17 und legt den aufgelösten Pfad ab; die Sitzungsauflösung nimmt genau
diesen, auch für die Suche nach dem Projekt-Profil. Ein Pfad, der die Prüfung
nicht besteht, erreicht die Auflösung nie.

**`--ask none` bleibt `504`.** Die Spezifikation ließ offen, ob eine Anfrage
ohne Regel bei `ask_mode = none` weiter in die Zeitüberschreitung läuft (`504`,
`reason: timeout`, `DECISION_KIND_TIMED_OUT`) oder einen eigenen Blockgrund
bekommt (`403`, `BlockReason::AskModeNone`). Es bleibt bei `504`: Der Wert ist
wahr — niemand hat entschieden, und genau das heißt `504` —, das Briefing des
Agenten sagt es seit HUM-071 wörtlich und hält es mit einem Test fest, und
`403` hieße „ein Mensch hat entschieden", was in diesem Modus niemand getan
hat. Der Kommentar in `profiles/llm-only.toml` behauptete das Gegenteil und ist
korrigiert. `BlockReason::AskModeNone` gibt es damit nicht.

**Die mitgelieferte Regelgruppe entsteht an einer Stelle.**
`humanitl_ipc::session::bundled_rules` setzt sie zusammen: Durchreiche, dann
die Regeln der beteiligten Profile (`[rules].inline` und `[rules].files`, Rang
4), dann `rules/default.yaml`. Dieselbe Funktion ruft `humanitld` beim Start
und der Sandbox-Dienst bei jeder Sitzung; `BUNDLED_RULES` ist aus
`humanitld/src/main.rs` dorthin gewandert. Zwei Stellen, die dieselbe Gruppe
bauen, liefen auseinander, und die Reihenfolge darin ist genau das, woran
HUM-104 gearbeitet hat.

**`SandboxService` bekommt seine Anschlüsse als Wert, nicht über `with_`.**
`SandboxPorts` trägt Regelspeicher und Sitzungszustand, beide wahlfrei. Ein
`with_rules(mut self)` hätte `Arc::get_mut` und damit ein `expect` gebraucht,
das die Codebasis außerhalb von Tests und `main` nicht kennt.

**Der Strom von `Sandbox(Start)` endet erst mit dem Agenten.** Er trägt jetzt
auch dessen Ausgabe (`SandboxEvent.output`) und seinen Exit-Code
(`SandboxEvent.exit`). Wer nur den Zustand wissen will, liest bis zum ersten
`running`, `failed` oder `stopped` und hört auf; die Tests tun genau das.

**Die Ausgabe des Agenten wird gefiltert, bevor sie den Daemon verlässt, und
der Filter ist eine Erlaubnisliste.** `humanitl_core::TerminalFilter` lässt von
allen Steuerfolgen genau eine hinaus: `ESC [ … m`, Farbe und Attribute. Alles
andere wird verworfen — jede OSC-Folge, jede Zeichenkettenfolge (DCS, SOS, PM,
APC), jede andere CSI-Folge und jede Ein-Zeichen-Escape-Folge, jeweils auch in
ihrer einbytigen C1-Form (`0x9b` ist CSI, `0x9d` ist OSC).

Die Richtung ist dieselbe wie bei `VISIBLE_ENV` (4.17) und
`SESSION_OVERRIDE_KEYS` und aus demselben Grund. Die erste Fassung sperrte
OSC 52 und OSC 8; der Review von HUM-067 fand vier wirksame Wege daran vorbei
(`OSC 052`, `OSC 0`, `\x9d52;…`, `ESC P tmux;…`). Über einem offenen
Namensraum kann keine Sperrliste vollständig sein.

Cursorbewegung, Löschen, Scrollen und das Zurücksetzen des Terminals gehen
damit ebenfalls nicht hinaus. Das ist gewollt: `\x1b[1A\x1b[2K` überschreibt
eine Zeile, die schon steht, und die drei Zeilen der Isolationsprüfung von
`humanitl run` stehen genau dort. Ohne PTY braucht kein Agent mehr als Farbe;
mit PTY entscheidet HUM-042 neu.

Erkannt wird jede Folge in drei Schreibweisen: mit `ESC`, als einzelnes
C1-Byte und als dessen wohlgeformte UTF-8-Kodierung (`C2 9B` ist `U+009B`,
also CSI). Die dritte fehlte in der zweiten Fassung und war voll wirksam:
VTE-basierte Terminals dekodieren UTF-8 vor dem Parser. Entschieden wird
deshalb am Codepunkt und nicht am Byte — `0xC2` ist das einzige Anfangsbyte,
aus dem ein C1-Steuerzeichen werden kann, und wird zurückgehalten, bis das
Folgebyte es entscheidet.

Der Filter hat Zustand, weil eine Folge über die Grenze zweier Lesevorgänge
laufen darf; ein zustandsloser Filter je Stück sähe die Hälfte und ließe sie
durch. Die Restlücke — ein Terminal, das nicht in UTF-8 arbeitet — steht in
`docs/SECURITY.md` 3.3. Er sitzt im Daemon und nicht im Client: Es gibt mehr
als einen Client, und die Zusage darf nicht an dem hängen, der gerade liest.
HUM-042 erweitert diesen Filter für den PTY-Pfad, statt einen zweiten daneben
zu bauen.

**Ein Start beansprucht die Sitzung, bevor er etwas merkt.** `SandboxService`
setzt `self.running` erst, wenn `bwrap` steht; dazwischen liegen die Auflösung
der Sitzung und der Start selbst. Zwei gleichzeitige `Start` kämen an
`is_running()` beide vorbei, beide starteten, und der zweite verdrängte den
ersten aus `running` — der erste Prozess liefe dann weiter, ohne dass ihn noch
jemand beenden könnte. `Pending::claimed` wird deshalb unter demselben Schloss
geprüft und gesetzt, und ein `StartClaim` gibt ihn beim Fallenlassen frei,
damit kein Fehlerpfad ihn vergisst. Aus demselben Grund gibt `remember` das
geprüfte Projektverzeichnis zurück, statt dass der Aufrufer es später ein
zweites Mal aus `pending` liest: Dazwischen könnte ein anderer Aufruf ein
anderes hineingelegt haben.

**Der Zuhörer der Ausgabe hängt am Backend, das startet, nicht an dem, das
gehalten wird.** `BwrapBackend::with_output_sink` setzt ihn vor dem Start;
`Running.backend` trägt ihn nicht. Ein gehaltener Sender schlösse seinen Kanal
nie, und wer auf dessen Ende wartet — der Weiterleiter der Ausgabe — wartete
für immer. Losgelassen wird er in `supervise`, nachdem die Leser eingesammelt
sind: Die Ausgabe ist zu Ende, wenn die Pipes zu sind, und nicht, wenn jemand
ein Handle fallen lässt.

**`CLI_005` ist der zweite Start.** Der Daemon führt genau eine Sitzung; ein
zweiter `Sandbox(Start)`, während eine läuft, bekommt den Befund vor der
Momentaufnahme. Das weicht von HUM-040 ab, wo ein Start auf eine laufende
Sandbox stillschweigend die Momentaufnahme lieferte — die Oberfläche schaltet
ihre Schaltfläche ohnehin ab, `humanitl run` dagegen bekäme einen laufenden
Zustand ohne Ausgabe und ohne Exit-Code und wüsste nicht, warum. Der Text nennt
keinen Anhänge-Befehl, weil es keinen gibt.

**Die Vertrags-Minor steht auf 4.** Neu sind `Start.session_profile`,
`Start.ask_mode`, `Start.cli_overrides`, `SandboxEvent.output` und
`SandboxEvent.exit`. Die Spiegelung in `app/lib/core/ipc/proto_version.dart`
bleibt bei 3 und darf nachziehen: Eine abweichende Minor ist verabredetermaßen
kein Grund, die Verbindung abzulehnen, und die Oberfläche liest die neuen
Felder nicht.

### 4.27 Aus der Umsetzung der Meta-Flüsse in der Historie (HUM-103, 2026-09-05)

Abweichungen von `backlog/sprint-3.md`, die dauerhaft gelten. Wo die
Spezifikation anderes sagt, gilt dieser Abschnitt. Der Zuschnitt des Issues —
was gebaut wurde und was nicht — steht als Stand-Abschnitt unter der
Spezifikation.

**Es gibt kein `decision=meta`, und es soll keines geben.** `BACKLOG.md` und das
vierte Akzeptanzkriterium von HUM-073 nennen eine neue Variante in `Decision`
und in `DecisionKind`. Gebaut ist die Spalte `flows.meta` **neben** der
Entscheidung, und `decision` bleibt an einem Meta-Fluss leer. `decision` sagt
aus, wie ein Mensch oder eine Regel über eine Anfrage entschieden hat; über eine
Anfrage an `humanitl.internal` entscheidet niemand. Der Gewinn ist nicht nur
Ehrlichkeit: Jede Auswertung über Entscheidungen — `decision:allow`,
`decision:block`, die Zahlen des Demolaufs — lässt einen Meta-Fluss dadurch von
selbst aus und musste ihn nicht ausnehmen. **Für jedes künftige Feld dieser Art
gilt dieselbe Frage: Ist es eine Art von Entscheidung oder etwas daneben? Wenn
es niemand entschieden hat, ist es etwas daneben.**

**Der einzige Weg nach `Recorded` ohne Entscheidung hängt am Fluss, nicht an
Disziplin und nicht an einem Wert.** `TransitionInput::Answer(MetaAnswer)` führt
aus `Received` unmittelbar in den Endzustand (ADR-0004, Nachtrag). Geprüft wird
er in `Flow::apply`, an `Flow::is_meta`, im Augenblick des Abschließens; die
einzige Tür dorthin ist `Flow::answer`, die den Grund nennt (`PROXY_009` für den
falschen Host) und den Nachweis nicht herausgibt. `MetaAnswer` ist
`#[non_exhaustive]`, außerhalb der Crate nicht baubar, und trägt keine Angaben.

**Die Lehre, teuer bezahlt:** Der erste Entwurf hängte den Weg an einen
Nachweis, den `MetaAnswer::for_request(&HttpRequest)` ausstellte. Das war zu
wenig. Ein solcher Nachweis belegt, dass *eine* Anfrage an den reservierten
Namen ging, nicht dass **diese** es tat — ein Aufrufer konnte sich einen für
`humanitl.internal` holen und ihn über `Flow::apply` auf einen gewöhnlichen
Fluss anwenden, der noch in `Received` stand; jede Anfrage steht dort nach der
Ankunft, und der Fluss landete in `Recorded`, ohne dass ein Mensch ihn je
gesehen hätte. Codex hat das am 2026-09-05 gefunden. **Wer künftig einen
Übergang hinzufügt, der eine Zusage dieses Produkts umgeht, bindet ihn an den
Gegenstand, um den es geht — nicht an seine Gattung —, prüft ihn dort, wo dieser
Gegenstand bekannt ist, und schreibt den Test dazu, dass ein echtes Zeugnis am
falschen Gegenstand abgelehnt wird.**

**Der reservierte Name wohnt im Kern.** `humanitl_core::META_HOST` und
`HostName::is_meta`; `humanitl_proxy::meta` reicht beide weiter. Zwei Kopien
derselben Zeichenkette liefen auseinander, und an der einen hinge ein Endpunkt,
an der anderen ein Weg am Menschen vorbei.

**Ein Meta-Fluss geht nicht durch `HoldQueue::publish`.** Der Handler ruft
`Recorder::apply` direkt. Der Fluss ist fertig, bevor ein Zuhörer etwas mit ihm
anfangen könnte; er gehört nicht in die `FlowRegistry`, die die Flows dieser
Sitzung führt, über die noch entschieden werden kann, und deren Datensätze
`/why/<id>` beantwortet. Folge, ausgesprochen: Die Historie zeigt einen
Meta-Fluss erst beim nächsten Laden, nicht als Ankunft in der Pille, und
`/why/<meta-id>` antwortet `404` — über einen Meta-Fluss gibt es nichts zu
erklären.

**Der Filterterm `meta:` steht an zwei der drei Auslegungen.** In
`daemon/crates/recorder/src/filter.rs` und im Dart-Fake, als Wahrheitswert wie
`edited:` und `passthrough:`. `humanitl_ipc::convert::matches_filter` bleibt bei
seiner bewusst kleineren Lesart (`host:`, `state:`, `session:`), die auch
`decision:` nicht kennt; HUM-099 führt die drei zusammen. Bis dahin hält ein
Dart-Test die Naht sichtbar: Er liest `KEYS` aus `filter.rs` und vergleicht die
Liste mit `fakeFilterKeys`.

**Ein Meta-Fluss wird nie als gehalten gezeichnet.** `historyVisualState` fängt
ihn vor allem anderen ab; die gemeinsame Ableitung `FlowVisualState` bildet
`decision == null` auf `held` ab, und das behauptete, ein Mensch sei gerade
dabei zu entscheiden. Er trägt bis auf Weiteres das Violett der Durchreiche
(`HFlowState.passthroughLlm`), die Farbe, die dieses Produkt schon für den
eigenen Kanal des Agenten benutzt (4.24, die `AgentAsk`-Karte). Ein eigener
`HFlowState.meta` in `app/packages/ui` bleibt offen.

**Was aufgezeichnet wird und was nicht.** Kopfzeilen der Anfrage, Pfad, Methode
und der Statuscode, den der Proxy selbst geschrieben hat; bei `/ask` zusätzlich
der **gesäuberte** Text der Bitte, nie der rohe Rumpf des Agenten. Der Rumpf
einer Meta-Antwort wird an keiner Stelle aufgezeichnet.
### 4.28 Aus der Umsetzung des Terminals (HUM-042, 2026-09-05)

Abweichungen von `backlog/sprint-3.md`, die dauerhaft gelten. Wo die
Spezifikation anderes sagt, gilt dieser Abschnitt.

**Jede Sitzung läuft an einem Pseudoterminal.** `SandboxService::launch`
startet mit `StdioMode::Pty { cols, rows }`, `80x24` bis ein Client seine
eigene Geometrie nennt. Ein zweiter Modus daneben hätte einen Schalter
gebraucht, den der Vertrag nicht hat: `Start` trägt kein Feld dafür, und jede
Ableitung aus einem anderen Feld wäre eine zweite Bedeutung für dieses Feld.
Drei Dinge ändern sich damit für **jeden** Leser dieser Ausgabe, auch für
`humanitl run`: Ein Terminal hat einen Strom, also kommt alles als
`OutputStream::Stdout`; die Zeilendisziplin macht aus `\n` ein `\r\n`; und die
Eingabe endet nicht mehr von selbst, ein Agent, der von stdin liest, wartet
statt sofort zu enden. Das Letzte ist der Zweck der Übung — ohne Terminal
beendet sich ein Vollbild-TUI sofort.

**Ein Strom, zwei Zusagen.** Die rohen Stücke werden **einmal** gelesen
(`Inner::stream_output`) und bedienen zwei Wege mit zwei Politiken:
`TerminalPolicy::ColourOnly` für `SandboxEvent.output`, das ein Mensch
unverändert in sein eigenes Terminal schreibt, und `TerminalPolicy::FullScreen`
für die `Terminal`-RPC. Der Leser hört nicht auf, wenn der Ereignisstrom
seinen Client verliert: Das Terminal hängt daran, und es gehört nicht dem, der
als erster gestartet hat.

**Der Filter bleibt einer, mit zwei Politiken.** 4.26 verlangt, dass HUM-042
den vorhandenen Filter erweitert statt einen zweiten daneben zu bauen; das gilt
und ist umgesetzt. Der neue Zustandsautomat liegt weiter in
`humanitl_core::terminal` und nicht in `humanitl-sandbox`, wohin die
Spezifikation ihn legen wollte: Er ist eine reine Funktion über Bytes und
gehört damit in den Kern (ARCHITECTURE 1), und der einzige Aufrufer sitzt
ohnehin in `humanitl-ipc`. `daemon/crates/sandbox/src/osc_filter.rs` gibt es
nicht.

**Die erlaubten OSC-Nummern sind eine Erlaubnisliste, keine Sperrliste.** Die
Spezifikation nannte `OscPolicy { deny: [0, 1, 2, 7, 8, 9, 52, 777, 1337] }`.
Diese Liste hat Löcher, und sie sind nachgemessen: OSC 99 ist die
Benachrichtigung in kitty, OSC 12 die Cursorfarbe, OSC 50 die Schriftart, und
jede Nummer, die ein Terminal morgen belegt, wäre offen. Es gilt
`terminal::OSC_ALLOWED` = `[4, 10, 11, 104, 110, 111, 133]`: Farbe, ihre
Rücknahmen, Prompt-Marken. Dieselbe Richtung wie `VISIBLE_ENV` (4.17) und
`SESSION_OVERRIDE_KEYS` (4.26), aus demselben Grund.

Zwei Folgen fallen zusätzlich weg, die die Spezifikation nicht nannte:
`CSI … t` (XTWINOPS setzt und liest den Fenstertitel an OSC 0 vorbei und ändert
die Fenstergröße) und die Nutzlast einer erlaubten OSC-Folge, sobald darin ein
Byte außerhalb von `0x20..=0x7e` steht — ein `ESC` in der Nutzlast ist eine
zweite Folge im Bauch der ersten, und Terminals brechen die äußere daran ab.

**Der Hinweis im Strom ist eine Bequemlichkeit, der Streifen ist die Zusage.**
Beide entstehen aus demselben Ereignis (`HoldQueue::subscribe`), aber nur der
Streifen über dem Terminal ist das Akzeptanzkriterium: Ein Vollbild-Agent
zeichnet die Zeile im Strom mit dem nächsten Bild weg, und er kann dieselbe
Zeile selbst schreiben — ein Absender in einem Bytestrom ist keine
Beglaubigung. Die Zeile läuft trotzdem als Ganzes durch `sanitize_note`, der
Pfad wird auf 48 Zeichen gekürzt, und **die eckige Klammer gehört dem
Absender**: `[` wird in allem, was aus dem Agenten stammt, zu `(`, damit
`[humanitl]` in der Zeile genau einmal vorkommt. Eingefügt wird nur an einer
Grenze (`TerminalFilter::at_boundary`), also weder in einer halben Folge noch
in einem halben Zeichen. `ui.terminal_notices` (Vorgabe `true`, Stufe
`advanced`) schaltet die Zeile ab; der Streifen bleibt.

**Ein Schreiber, beliebig viele Leser, und die Grenze steht im Daemon.**
`TerminalHub` hält den Platz des Schreibers, und ein `WriterSlot` gibt ihn beim
Fallenlassen frei — ein Platz, der nach einem abgebrochenen Strom belegt
bliebe, verweigerte jeden weiteren Schreiber bis zum Ende der Sitzung. `data`
und `Resize` eines Lesers werden im Handler verworfen. Der Ringpuffer (64 KiB)
hält **gefilterte** Bytes; Ring lesen und Rundfunk abonnieren geschehen unter
demselben Schloss, unter dem gefüttert wird, sonst läge zwischen beidem eine
Lücke oder eine Wiederholung.

**`TERM_002` ist neu:** Das Terminal der Sandbox nimmt weder Eingabe noch
Geometrie an — die Sitzung läuft ohne Pseudoterminal, oder der Agent ist weg
und der Kernel meldet `EIO`. Ohne laufende Sitzung antwortet `Terminal` mit
`IPC_006`, wie `Sandbox`; ein leeres `Open.sandbox_id` heißt „die Sitzung, die
läuft".

**Die Startdiagnostik überlebt das Pseudoterminal.** Ein PTY hat keine
getrennte Fehlerausgabe, und `Shared::verdict` liest genau die. Die ersten
`PTY_MIRROR_BYTES` (2 KiB) der Terminalausgabe werden deshalb in den
Fehlerpuffer gespiegelt — nicht über `append_stderr`, sonst bekäme ein
Zuhörer jedes Byte des Anfangs zweimal. Ohne diese Spiegelung verlöre
`is_userns_failure` seine Quelle, und aus `SANDBOX_003` mit
Behebungsvorschlag würde ein `SANDBOX_012` ohne Grund.

**`resize` ist zwei Schritte.** `tcsetwinsize` trägt die Größe ein, und
`kill_process_group(child_pid, SIGWINCH)` sagt es dem Agenten. Der Kernel tut
das Zweite nicht: Er schickt `SIGWINCH` an die Vordergrundgruppe des
*steuernden* Terminals, und die Sandbox hat mit `--new-session` keines. Aus
demselben Grund erreicht `Ctrl+C` den Agenten als Byte `0x03` und nie als
Signal. Schreiber-Resizes sind auf eine je 50 ms gedrosselt, die letzte
gewinnt.

**`humanitl sandbox attach [--read-only]`** ist die CLI-Hälfte der RPC
(ADR-018). Rohmodus über `rustix::termios`, `SIGWINCH` hinauf, `close` beim
Ende der eigenen Eingabe, und beim Verlassen `ESC [ ? 1049 l ESC [ ? 25 h` in
das eigene Terminal: Ein Vollbild-Agent lässt sonst den Alternativschirm an
und den Cursor versteckt.

**Nur wohlgeformtes UTF-8 geht hinaus, und eine C1-eingeleitete Folge nie.**
Beides kam aus dem Review. Ein Filter, der nur Folgebytes zählt, lässt die
überlangen Formen durch: `E0 82 9B` ist `U+009B` (CSI), `C0 9B` sogar `U+001B`
(`ESC`), und in vier Bytes geht dasselbe. RFC 3629 verbietet sie, und ein
konformer Dekoder macht `U+FFFD` daraus — aber **ein Filter, der zusichert,
dass keine Steuerfolge hinausgeht, darf sich nicht darauf verlassen, dass der
Empfänger korrekt dekodiert**; der Empfänger ist irgendein Terminal des
Nutzers. Ein Mehrbytezeichen wird deshalb zusammengehalten, bis es vollständig
und die kürzeste Kodierung seines Codepunktes ist; was die Prüfung nicht
besteht, fällt weg, auch das schon gesehene Anfangsbyte. Und eine Folge, die
mit einem C1-Byte beginnt, geht nie hinaus, auch wenn dieselbe Folge mit `ESC`
erlaubt wäre: Ohne diese Regel machte der Filter aus `\x9b 2 J` die erlaubte
Sieben-Bit-Form und reichte sie weiter.

**Zwischen `spawn` und der Warteschleife gibt es keinen Rückweg.** `bwrap`
läuft ab dem `spawn`; wer danach zurückkehrt, sammelt es nie ein, und der
Prozess bleibt als Zombie stehen. Der Leser-Deskriptor des Pseudoterminals
wird deshalb schon in `open_pty` verdoppelt und nicht erst nach dem Start.
Einen Fehlerpfad zu entfernen ist besser, als ihn zu behandeln.

**Der Zuhörer der Warteschlange endet mit seiner Sitzung.** `HeldNotices::run`
hängt an einem Kanal, der dem Daemon gehört und länger lebt als die Sitzung.
`accompany` hält deshalb sein `JoinHandle` und bricht es nach `stream_output`
ab. Ohne das bliebe je beendeter Sitzung eine Aufgabe stehen, die einen
`TerminalHub` hält — und mit ihm den `SandboxHandle` und die Herrscherseite
des Pseudoterminals —, und sie könnte noch in den Ring einer Sitzung
schreiben, die es nicht mehr gibt.

**Ein aufgeschobener Hinweis geht mit dem Ende der Sitzung noch hinaus.**
`TerminalHub::finish` gibt aus, was auf eine Grenze gewartet hat, bevor es
`Exit` sendet: Nach dem `flush` steht der Filter wieder auf einer Grenze, und
der Agent schreibt nichts mehr, in das der Hinweis fallen könnte. Ihn dort
wegzuwerfen hieße, genau den Fluss zu verschweigen, der beim Ende noch offen
war.

**Die Oberfläche filtert nicht ein zweites Mal.** `TerminalPane` schreibt, was
kommt, und registriert keinen OSC-Handler; die 16 Farben stehen als
`HTokens.terminal` (`HTerminalPalette`) und leiten sich aus den Zustandsfarben
ab — was ein Mensch in der Warteschlange lernt, gilt im Terminal. Neu in
`packages/ui` ist außerdem `HContextMenu` samt `HContextMenuController`, weil
der Emulator seinen Rechtsklick selbst braucht; zweiter Nutzer ist HUM-030.
Und der Emulator erfährt, wenn er nur zusehen darf (`TerminalView.readOnly`
aus `TerminalSessionState.readOnly` und der Phase): Der Daemon verwirft die
Tastendrücke eines Lesers, und ein Cursor, der auf Eingabe zu warten scheint,
verspricht etwas, das niemand hält.
