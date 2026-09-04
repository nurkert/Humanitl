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
humanitl rules list|add|remove|test URL [--json]
humanitl flows list [FILTER] | show ID [--json]
humanitl audit verify|export [--format jsonl|csv] [--out FILE]
humanitl config get KEY | set KEY VALUE | schema | edit
humanitl daemon install|status|logs
```

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
- Gruppe `limits` (HUM-057) ist die Heimat aller Caps und Timeouts: `limits.hold_body_cap_bytes` (Alias `hold.body_cap_bytes`), `limits.preview_cap_bytes` (Alias `preview.cap_bytes`), `limits.event_buffer` (Alias `ipc.event_buffer`), `limits.max_decompress_ratio`, `limits.hold_max_flows`, `limits.hold_max_bytes`, `limits.connect_timeout_secs`, `limits.header_timeout_secs`, `limits.body_timeout_secs`, `limits.idle_timeout_secs`, `limits.recorder_max_body_bytes` (Alias `recorder.max_body_bytes`).
- `resolver.nameserver`, `resolver.overrides`, `resolver.cache_ttl_secs`, `resolver.prefer` (`ipv4|ipv6`), `resolver.test_ca` (nur Tests).
- `upstream.connect_timeout_secs` ist Alias von `limits.connect_timeout_secs`.
- `findings.enabled`, `findings.user_terms`, `findings.email_allow_domains`, `findings.ignored_hashes`.
- `agent.briefing.enabled` (HUM-071).
- `experimental.upstream_port_map` (nur Tests).

### 4.5 Regeln
- Session-Regeln (In-Memory, `expires: session`) werden vor persistenten Regeln ausgewertet. Innerhalb jeder Gruppe gilt die Reihenfolge der Liste. Grund: Was der Nutzer gerade entschieden hat, soll sofort gelten, auch wenn eine ältere persistente Regel breiter matcht.
- `match.upgrade: websocket` ist Teil des Schemas.

### 4.6 Diagnostic-Register
Datei `daemon/crates/core-types/src/diagnostics/codes.rs` hält alle Codes als Konstanten mit Doc-Kommentar. Reservierte Bereiche: `DAEMON_001..019`, `SANDBOX_001..029` (001–006 Launcher/Profil, 007 Bridge-Richtung, 010–012 Start-Fehler), `TLS_001..009`, `LLM_001..009`, `RULES_001..019` (001–008 Datei und Muster, 009–011 Regelspeicher aus HUM-027), `TERM_001..009`, `RECORDER_001..009`, `LIMIT_001..009`, `AUDIT_001..009`, `CONFIG_001..009`. Ein Code wird nie wiederverwendet; entfernte Codes bleiben als `#[deprecated]` stehen. CI-Test: jeder im Code verwendete Code ist im Register.

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
- Weitere Defaults: `limits.header_timeout_secs` 30, `limits.body_timeout_secs` 300, `limits.idle_timeout_secs` 90, `limits.recorder_max_body_bytes` 32 MiB, `resolver.cache_ttl_secs` 300, `resolver.prefer` ipv4, `pseudonyms.max_response_bytes` 1 MiB, `pseudonyms.translate_responses` true, `findings.enabled` true, `agent.briefing.enabled` true, `hold.hard_block_checksum_secrets` false.
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
- `socketpair()` bleibt vom seccomp-Filter unberührt: es kennt nur `AF_UNIX`, verbindet zwei Deskriptoren desselben Prozessbaums und ist kein Egress (Node/Bun-Kindprozess-IPC). Der Halbsatz in 3.1 („`socket`/`socketpair` nur für `allow_families`") wird zu „`socket()` nur für `allow_families` × `allow_types`". ESC-1 und HUM-041 Check 3 erwarten `socketpair` = ok.
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
- `escape-launch` läuft fail-closed: Ist eine der drei Garantien rot oder fehlt der Bericht, beendet es die Sandbox und endet mit dem Befund (Exit 3), statt den Befehl trotzdem laufen zu lassen. Ein Escape-Test in einer Sandbox ohne belegte Isolation misst nichts.
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
Nutzerdatei gelesen wird, bevor `RuleSet::prepend_bundled` die mitgelieferten
Regeln davorstellt; eine Id darin darf eine Regel benennen, die es in dieser
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
