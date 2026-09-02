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

Flutter 3.47.x gepinnt in `app/.fvmrc` und `pubspec.yaml`. Pakete: `shadcn_flutter` (exakte Version gepinnt, z. B. `0.0.54`), `flutter_riverpod` 3.x + `riverpod_annotation` + `riverpod_generator`, `freezed` 4 + `json_serializable`, `grpc` 5.x + `protobuf`, `two_dimensional_scrollables`, `re_editor`, `xterm2`, `diff_match_patch`, `window_manager`, `tray_manager`, `flutter_local_notifications`, `file_picker`, `flutter_localizations` + `intl`, `alchemist` (dev).

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
app/packages/ui/                   Wrapper um shadcn_flutter: HButton, HPill, HBadge, HPanel, HRow, HModal, HSheet, HTokens
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
Datei `daemon/crates/core-types/src/diagnostics/codes.rs` hält alle Codes als Konstanten mit Doc-Kommentar. Reservierte Bereiche: `DAEMON_001..019`, `SANDBOX_001..029` (001–006 Launcher/Profil, 007 Bridge-Richtung, 010–012 Start-Fehler), `TLS_001..009`, `LLM_001..009`, `RULES_001..009`, `TERM_001..009`, `RECORDER_001..009`, `LIMIT_001..009`, `AUDIT_001..009`, `CONFIG_001..009`. Ein Code wird nie wiederverwendet; entfernte Codes bleiben als `#[deprecated]` stehen. CI-Test: jeder im Code verwendete Code ist im Register.

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
