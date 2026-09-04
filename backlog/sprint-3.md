# Sprint 3 · Agent Inside (M3)

Ziel des Sprints: OpenCode läuft in der bwrap-Sandbox gegen einen LLM-Server im LAN. Das UI zeigt Terminal, Sandbox-Status und Isolation-Check. `humanitl run --profile llm-only` liefert im aktuellen Verzeichnis eine reine Inferenz-Instanz ohne UI. Jeder Fehlerpfad dieses Sprints liefert ein `Diagnostic` mit `why` und `fix`.

Voraussetzungen aus Sprint 0 bis 2: `humanitl-core` (HUM-004, HUM-063), `humanitl-config` (HUM-062), Sandbox-Launcher und Shim (HUM-011, HUM-012), Proxy-Kern und Hold-Queue (HUM-015, HUM-016), Regel-Engine (HUM-022), Findings (HUM-025), Recorder (HUM-026), gRPC-Server (HUM-018), CLI-Grundgerüst (HUM-064), Flutter-Shell und Intercept-Screen (HUM-019, HUM-020, HUM-028).

| Reihenfolge | ID | Titel | Größe |
|---|---|---|---|
| 1 | HUM-037 | AgentAdapter-Trait und OpenCode-Profil | M |
| 2 | HUM-071 | Agent-Briefing | S | HUM-037 |
| HUM-038 | Default-Regeln | S |
| 3 | HUM-039 | LLM-Passthrough-Regel und Endpoint-Test | M |
| 4 | HUM-073 | Meta-Endpoint `humanitl.internal` | M | HUM-039, HUM-072 |
| HUM-066 | Profile | M |
| 5 | HUM-040 | Sandbox-Screen | M |
| 6 | HUM-041 | Isolation-Check-Panel und Ring | M |
| 7 | HUM-042 | Terminal | L |
| 8 | HUM-043 | `/work`-Härtung | M |
| 9 | HUM-044 | Setup-Flow | M |
| 10 | HUM-045 | TLS-Fehler-Erkennung | S |
| 11 | HUM-075 | `humanitl doctor` | M | HUM-064, HUM-063, HUM-041 |
| HUM-076 | LLM-Server finden | M | HUM-039, HUM-044 |
| HUM-068 | Geführte Diagnostics im Sandbox-Screen | M |
| 12 | HUM-067 | `humanitl run` | L |
| 13 | HUM-046 | Demo-Skript M3 | S |

Demo-Ziel am Sprint-Ende (HUM-046): CI startet einen Ollama-Mock, `humanitl run --profile default` startet OpenCode, der erste Prompt geht per Passthrough ans Mock-LLM, der models.dev-Aufruf wird per Default-Regel geblockt, ein `webfetch` wird gehalten, per gRPC erlaubt, und die Antwort erscheint im Terminal-Stream.

---

> **Review-Korrekturen 2026-09-02** (gelten vor dem Text): HUM-042: ein schreibender Client, beliebig viele lesende (`read_only`), Geometrie des Schreibers, Leser letterboxed; `TERM_001` nur bei zweitem Schreiber. HUM-067: `--ask terminal` verweigert Vollbild-TUI-Agenten (`AgentAdapter::is_fullscreen_tui()` ist für OpenCode `true`) mit Diagnostic `CLI_002` und schlägt `--ask ui` oder `--ask none` vor; `--ask terminal` bleibt für `humanitl sandbox run -- <zeilenorientiertes Kommando>`.
>
> **Abgleich 2026-09-02**: Escape-Tests heißen `esc-N-<name>.sh`. Neue Issues HUM-071 (Agent-Briefing, nach HUM-037) und HUM-073 (Meta-Endpoint, nach HUM-039) sind unten angehängt; HUM-046 prüft zusätzlich Block mit Notiz (HUM-072). Bridges und seccomp-Familien kommen aus dem Profil (siehe Sprint 1 Abgleich).

## HUM-037 · AgentAdapter-Trait und OpenCode-Profil
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-004, HUM-011, HUM-014, HUM-022, HUM-062 · Blockiert: HUM-038, HUM-039, HUM-067

### Kontext
Humanitl startet nicht irgendein Programm, sondern einen bekannten Agenten mit bekannten Eigenheiten. OpenCode braucht eine Provider-Konfiguration für einen OpenAI-kompatiblen Endpoint, holt beim Start ungefragt `https://models.dev/api.json`, prüft auf Updates über GitHub, lädt Provider-Pakete per npm nach und hat einen gehosteten `websearch`. All das muss vor dem ersten Start deterministisch gesetzt sein, sonst sieht der Nutzer als erste gehaltene Anfrage etwas, das er nicht ausgelöst hat (Usability-Review, BACKLOG.md Abschnitt 5). Das Trait `AgentAdapter` (CONVENTIONS.md 3.10, BACKLOG.md Abschnitt 6) ist der Erweiterungspunkt für weitere Agenten und wird hier zum ersten Mal implementiert.

### Ziel
Die Crate `humanitl-sandbox` enthält das Trait `AgentAdapter` und die Implementierung `OpenCodeAdapter`. Für eine `SessionContext` liefert der Adapter das Startkommando, die Umgebungsvariablen, die in der Sandbox anzulegenden Dateien (`opencode.json`, gebündelte `models.json`), seine Default-Regeln und die Passthrough-Regel. Der Daemon fügt beim Start einer Session diese Beiträge in den `LaunchPlan` ein. Ein Test startet die Sandbox mit dem Adapter gegen einen Fake-LLM und beobachtet, dass OpenCode ohne Netzwerkzugriff außer dem Passthrough bis zum Prompt kommt.

### Nicht-Ziel
Keine anderen Adapter (Aider, Codex, Claude Code kommen nach dem MVP, BACKLOG.md Abschnitt 9). Keine Permission-Bridge über `opencode serve` (Post-MVP). Keine Installation von OpenCode selbst: der Adapter setzt voraus, dass `opencode` im Host-`$PATH` liegt oder `agent.command` gesetzt ist; die Installationsanleitung liefert HUM-044 als Diagnostic.

### Betroffene Pfade
- `daemon/crates/sandbox/src/agent/mod.rs` (neu): Trait, `SessionContext`, Registry
- `daemon/crates/sandbox/src/agent/opencode.rs` (neu): `OpenCodeAdapter`
- `daemon/crates/sandbox/src/agent/opencode_models.rs` (neu): Einbettung der gebündelten `models.json` via `include_bytes!`
- `agents/opencode/opencode.json.tmpl` (neu): Template
- `agents/opencode/models.json` (neu): gebündelter Ersatz für `https://models.dev/api.json`
- `agents/opencode/README.md` (neu): Wie `models.json` erneuert wird
- `agents/opencode/update-models.sh` (neu)
- `daemon/crates/sandbox/tests/opencode_adapter.rs` (neu)
- `daemon/bin/humanitld/src/session.rs`: Adapter-Beiträge in `LaunchPlan` einfügen

### Spezifikation

Trait und Kontext (in `agent/mod.rs`):

```rust
/// Everything an adapter needs to know about the session it is asked to prepare.
pub struct SessionContext {
    pub session: SessionId,
    pub work_dir_host: PathBuf,          // host path of the project
    pub work_dir_sandbox: PathBuf,       // always "/work"
    pub llm: LlmConfig,                  // from humanitl-config
    pub agent_command_override: Option<Vec<OsString>>,   // config key agent.command
    pub proxy_port: u16,                 // 3128
    pub ca_path_sandbox: PathBuf,        // "/etc/humanitl/ca.crt"
    pub language: Language,              // en | de, for agent-visible banners
}

pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn command(&self, ctx: &SessionContext) -> Vec<OsString>;
    fn env(&self, ctx: &SessionContext) -> Vec<(String, String)>;
    fn files(&self, ctx: &SessionContext) -> Vec<SandboxFile>;
    fn default_rules(&self) -> Vec<Rule>;
    fn llm_passthrough(&self, llm: &LlmConfig) -> Rule;
    /// Host-side preflight. Must not touch the network. Returns diagnostics, empty = ok.
    fn preflight(&self, ctx: &SessionContext) -> Vec<Diagnostic>;
}

/// A file the daemon writes into the sandbox before exec (tmpfs-backed, never into /work).
pub struct SandboxFile { pub dst: PathBuf, pub content: Vec<u8>, pub mode: u32 }

pub struct AdapterRegistry { adapters: Vec<Box<dyn AgentAdapter>> }
impl AdapterRegistry {
    pub fn builtin() -> Self;                         // contains OpenCodeAdapter
    pub fn get(&self, id: &str) -> Option<&dyn AgentAdapter>;
    pub fn ids(&self) -> Vec<&'static str>;
}
```

Konkrete Werte für `OpenCodeAdapter`:

- `id()` = `"opencode"`.
- `command()`: bei `agent_command_override` genau dieses; sonst `["opencode"]`. Kein `--port`, kein `serve`. Das TUI läuft im PTY aus HUM-042.
- `env()` (zusätzlich zum Env-Kit aus HUM-014, das der Launcher setzt):

| Variable | Wert | Grund |
|---|---|---|
| `OPENCODE_DISABLE_AUTOUPDATE` | `true` | kein GitHub-Release-Check |
| `OPENCODE_MODELS_URL` | `file:///etc/humanitl/opencode/models.json` | ersetzt `https://models.dev/api.json`; falls OpenCode das Schema `file://` in der laufenden Version nicht akzeptiert, greift Fallstrick 1 |
| `OPENCODE_CONFIG` | `/etc/humanitl/opencode/opencode.json` | Konfig außerhalb von `/work`, damit der Agent sie nicht umschreibt |
| `OPENCODE_AUTO_SHARE` | `false` | kein Session-Sharing |
| `OPENCODE_ENABLE_EXA` | `false` | gehosteter Websearch aus |
| `OPENCODE_ENABLE_PARALLEL` | `false` | ebenso |
| `HOME` | `/home/agent` | tmpfs-Home, damit `~/.config/opencode` und `~/.local/share/opencode` nicht auf dem Host landen |
| `XDG_CONFIG_HOME` | `/home/agent/.config` | |
| `XDG_DATA_HOME` | `/home/agent/.local/share` | |
| `XDG_CACHE_HOME` | `/home/agent/.cache` | |
| `NODE_EXTRA_CA_CERTS` | `/etc/humanitl/ca.crt` | Bun/Node-Fetch vertraut der Proxy-CA |
| `TERM` | `xterm-256color` | |
| `COLORTERM` | `truecolor` | |
| `LANG` | `C.UTF-8` | |

- `files()`:
  1. `/etc/humanitl/opencode/opencode.json` (mode 0444) aus dem Template unten, mit `{{LLM_BASE_URL}}` = `llm.endpoint` plus `/v1` (falls der Endpoint nicht bereits auf `/v1` endet) und `{{MODELS}}` = Liste aus `llm.models` (Config, Default leer). Ist die Liste leer, wird ein Platzhalter-Modell `"default"` eingetragen und ein `Diagnostic` `LLM_004` (Warning) aus `preflight()` geliefert.
  2. `/etc/humanitl/opencode/models.json` (mode 0444) aus `include_bytes!("../../../../agents/opencode/models.json")`.
  3. `/home/agent/.config/opencode/.keep` (leer), damit das Verzeichnis existiert.

Template `agents/opencode/opencode.json.tmpl`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "humanitl-local": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "Humanitl local LLM",
      "options": {
        "baseURL": "{{LLM_BASE_URL}}"
      },
      "models": {{MODELS}}
    }
  },
  "model": "humanitl-local/{{DEFAULT_MODEL}}",
  "autoupdate": false,
  "share": "disabled",
  "permission": {
    "webfetch": "ask",
    "websearch": "deny",
    "external_directory": "deny"
  }
}
```

`{{MODELS}}` wird zu einem JSON-Objekt der Form `{"<name>": {"name": "<name>"}}` pro Modell. `{{DEFAULT_MODEL}}` ist das erste Modell der Liste.

- `default_rules()`: liefert die Regeln aus HUM-038 mit `bundled: true`. Der Adapter ist die einzige Quelle für diese Regeln; `rules/default.yaml` ist die Datei, die der Adapter per `include_str!` lädt.
- `llm_passthrough()`: siehe HUM-039, dort spezifiziert.
- `preflight()`:
  - `AGENT_001` (Blocking): `opencode` nicht im `$PATH` und kein Override. `fix: CopyCommand("curl -fsSL https://opencode.ai/install | bash")`, `docs: https://opencode.ai/docs/`.
  - `AGENT_002` (Warning): Override gesetzt, Binary nicht ausführbar.
  - `LLM_004` (Warning): keine Modelle konfiguriert, Platzhalter aktiv. `fix: ChangeSetting { key: "llm.models", value: "<aus Test-Button>" }`.

Gebündelte `models.json`: eine reduzierte Kopie des `models.dev`-Formats, die genau einen Provider `humanitl-local` mit den konfigurierten Modellen beschreibt. Da OpenCode diese Datei beim Start zwingend lädt, muss sie syntaktisch dem echten Schema entsprechen. `update-models.sh` lädt `https://models.dev/api.json` einmalig auf dem Host des Entwicklers (nie in der Sandbox), extrahiert das Schema-Skelett eines Providers und schreibt es als Vorlage; die Modell-Einträge werden zur Laufzeit vom Adapter eingesetzt (Template-Mechanismus identisch zu `opencode.json`). Das Skript ist Entwickler-Werkzeug, kein Laufzeitpfad.

Integration in `session.rs`:

```rust
let adapter = registry.get(&config.agent.adapter).ok_or(Diagnostic::agent_unknown(&config.agent.adapter))?;
let diags = adapter.preflight(&ctx);
if diags.iter().any(|d| d.severity == Severity::Blocking) { return Err(diags); }
plan.argv_agent = adapter.command(&ctx);
plan.env.extend(adapter.env(&ctx));
plan.files.extend(adapter.files(&ctx));
ruleset.prepend_bundled(adapter.default_rules());
ruleset.prepend_bundled(vec![adapter.llm_passthrough(&config.llm)]);
```

`LaunchPlan` bekommt das Feld `files: Vec<SandboxFile>`; der bwrap-Launcher (HUM-011) schreibt jede Datei per `--file FD DST` bzw. `--ro-bind-data FD DST` (bwrap unterstützt `--file` mit einem FD, der vor dem Aufruf befüllt wird).

### Schritte
1. `agent/mod.rs` mit Trait, `SessionContext`, `SandboxFile`, `AdapterRegistry` anlegen. `cargo build` grün.
2. `LaunchPlan.files` einführen, im bwrap-Launcher `--file`-Argumente erzeugen (FD über `memfd_create`, Inhalt schreiben, `lseek(0)`). Test: Datei erscheint mit Modus 0444 in der Sandbox.
3. Template und `models.json` unter `agents/opencode/` anlegen. `update-models.sh` schreiben und einmal ausführen, Ergebnis committen.
4. `opencode.rs` implementieren: `command`, `env`, `files` (Template-Rendering ohne externe Crate: `str::replace` für die drei Platzhalter reicht, JSON-Validität per `serde_json::from_str` im Test), `preflight`.
5. `default_rules()` über `include_str!` an `rules/default.yaml` binden (Datei kommt aus HUM-038; bis dahin leere Liste, Test wird in HUM-038 ergänzt).
6. `session.rs` verdrahten. Config-Schlüssel `agent.adapter` (Default `opencode`, Tier `advanced`), `agent.command` (Tier `expert`), `llm.models` (Tier `basic`, Liste von Strings) im Schema ergänzen.
7. Integrationstest `opencode_adapter.rs` (siehe Tests).

### Tests
- `opencode_env_contains_required_keys`: `env()` enthält alle Variablen der Tabelle mit exakt den Werten.
- `opencode_config_is_valid_json_and_points_to_llm`: `files()` liefert `opencode.json`, `serde_json` parst, `provider["humanitl-local"]["options"]["baseURL"] == "http://192.168.1.50:11434/v1"` bei `llm.endpoint = "http://192.168.1.50:11434"`.
- `opencode_config_appends_v1_only_once`: Endpoint `http://x:1/v1` ergibt `http://x:1/v1`, nicht `/v1/v1`.
- `opencode_models_placeholder_when_empty`: leere `llm.models` ⇒ Modell `default`, `preflight` enthält `LLM_004`.
- `opencode_preflight_missing_binary`: `$PATH` ohne `opencode`, kein Override ⇒ `AGENT_001` mit `Severity::Blocking`.
- `opencode_default_rules_are_bundled`: alle Regeln aus `default_rules()` haben `bundled == true`.
- Integration (benötigt bwrap, markiert `#[ignore]` ohne Feature `escape`): Sandbox mit Adapter starten, Kommando durch `sh -c 'cat /etc/humanitl/opencode/opencode.json; env | grep OPENCODE_'` ersetzen, Ausgabe enthält `humanitl-local` und `OPENCODE_DISABLE_AUTOUPDATE=true`.
- Integration mit echtem OpenCode (nur lokal, `#[ignore]`): Start gegen Fake-LLM, innerhalb von 20 s erscheint der TUI-Prompt im PTY, kein Flow außer Passthrough im Recorder.

### Akzeptanzkriterien
- [ ] `cargo test -p humanitl-sandbox` grün, alle oben genannten Tests vorhanden.
- [ ] `humanitl sandbox argv --profile default` zeigt die `--file`-Einträge für `opencode.json` und `models.json`.
- [ ] `humanitl config schema | jq '.properties.agent'` zeigt `adapter` und `command` mit Tier und Beschreibung.
- [ ] Manuell: OpenCode startet in der Sandbox, `opencode` zeigt den Provider `Humanitl local LLM` in der Modellauswahl.
- [ ] Manuell: Kein gehaltener Flow zu `models.dev` oder `api.github.com` beim Start (wird in HUM-038 automatisiert).

### Fallstricke
1. **`OPENCODE_MODELS_URL` und `file://`.** Ob OpenCode `file://` akzeptiert, hängt von der Version ab. Falls nicht: der Daemon startet einen zweiten Unix-Socket-Listener, der nur `GET /api.json` mit der gebündelten Datei beantwortet, und der Adapter setzt `OPENCODE_MODELS_URL=http://127.0.0.1:3129/api.json`; der Shim öffnet die zweite socat-Bridge auf Port 3129. Das ist im Sandbox-Modell erlaubt, weil es derselbe Mechanismus wie der Proxy-Socket ist. Test: `curl http://127.0.0.1:3129/api.json` in der Sandbox liefert die Datei. Entscheidung wird beim ersten echten Start getroffen und als Kommentar im Adapter dokumentiert.
2. **npm-Nachladen des Provider-Pakets.** OpenCode installiert `@ai-sdk/openai-compatible` beim ersten Gebrauch nach `~/.cache/opencode`. Das erzeugt gehaltene Flows zu `registry.npmjs.org`. Zwei Optionen: (a) Default-Regel `ask` für `registry.npmjs.org` (HUM-038), der Nutzer sieht einen erklärbaren Flow; (b) Paket vorinstallieren in ein Cache-Volume. MVP macht (a) und zeigt in der Katalog-Karte „npm registry · typisch für Provider-Installation". Das Home ist tmpfs, also wird bei jedem Start neu geladen; ein persistentes Cache-Volume unter `$XDG_DATA_HOME/humanitl/cache/<projekt-hash>/opencode` wird als `rw`-Mount nach `/home/agent/.cache/opencode` eingehängt (Profil-Schlüssel `mounts.cache = true`, Default `true`).
3. **Bun und CA.** Bun ab 1.1.22 liest `NODE_EXTRA_CA_CERTS`, ältere Versionen nicht. OpenCode-Binaries bündeln Bun. Wenn TLS gegen den Proxy fehlschlägt, greift HUM-045 mit `TLS_002`.
4. **Config-Pfad.** Wenn `OPENCODE_CONFIG` nicht respektiert wird, fällt OpenCode auf `~/.config/opencode/opencode.json` zurück. Der Adapter legt die Datei deshalb zusätzlich dort ab (Schritt 4, zwei Ziele, gleicher Inhalt).
5. **Template-Injection.** `llm.endpoint` und Modellnamen kommen aus der Config und landen in JSON. Modellnamen mit `"` müssen per `serde_json::to_string` escaped werden; das Template wird deshalb nicht per String-Replace, sondern durch Parsen des Templates als `serde_json::Value` und Setzen der Felder gerendert. Die Platzhalter im Template sind nur Dokumentation für Menschen.
6. **`--file` in bwrap** nimmt den FD-Inhalt, nicht einen Pfad. Der FD muss vor `exec` von bwrap offen bleiben; `CLOEXEC` darf nicht gesetzt sein.

### Referenzen
BACKLOG.md 3.2, 6, 9 (Punkt 6); ADR-002; CONVENTIONS.md 3.4, 3.10. OpenCode Providers (https://opencode.ai/docs/providers/), Permissions (https://opencode.ai/docs/permissions/), Network (https://opencode.ai/docs/network/), Ollama-Integration (https://docs.ollama.com/integrations/opencode), Issue zu Offline-Betrieb (https://github.com/anomalyco/opencode/issues/16117), bwrap Manpage `--file` (https://manpages.debian.org/trixie/bubblewrap/bwrap.1.en.html).

---

## HUM-038 · Default-Regeln
Sprint: 3 · Größe: S · Abhängigkeiten: HUM-022, HUM-037 · Blockiert: HUM-046

### Kontext
Beim ersten Start darf der Nutzer keine Flut ungefragter Anfragen sehen. Die Recherche (BACKLOG.md Abschnitt 10, Risiko „OpenCode telefoniert nach Hause") nennt die Hosts. Default-Regeln werden als `bundled` markiert, erscheinen im Rules-Screen mit Badge und sind vom Nutzer deaktivierbar, aber nicht löschbar.

### Ziel
`rules/default.yaml` existiert, wird vom `OpenCodeAdapter` geladen und beim Session-Start vor die Nutzerregeln gestellt. Ein Test misst: Anzahl gehaltener Flows zwischen Sandbox-Start und erstem Nutzer-Prompt ist höchstens 1.

### Nicht-Ziel
Keine Regeln für andere Agenten. Keine Regel-UI-Änderungen (HUM-033 hat das Bundled-Badge bereits vorgesehen).

### Betroffene Pfade
- `rules/default.yaml` (neu)
- `daemon/crates/sandbox/src/agent/opencode.rs`: `default_rules()` lädt die Datei
- `daemon/crates/rules/src/lib.rs`: `RuleSet::prepend_bundled`, Flag `disabled` pro Regel
- `daemon/crates/sandbox/tests/startup_noise.rs` (neu)

### Spezifikation

`rules/default.yaml`, vollständig:

```yaml
# Bundled default rules for the OpenCode adapter.
# Loaded before user rules. First match wins. Users may disable but not delete these.
version: 1
rules:
  - id: 01920000-0000-7000-8000-000000000001
    action: block
    match: { host: "models.dev" }
    expires: never
    bundled: true
    note: "OpenCode fetches its model catalog from models.dev on every start. Humanitl ships a bundled copy; the network call is unnecessary."

  - id: 01920000-0000-7000-8000-000000000002
    action: block
    match: { host: "api.github.com", path: "/repos/anomalyco/opencode/releases/**" }
    expires: never
    bundled: true
    note: "Update check. Disabled via OPENCODE_DISABLE_AUTOUPDATE; this rule is the second lock."

  - id: 01920000-0000-7000-8000-000000000003
    action: block
    match: { host: "**.posthog.com" }
    expires: never
    bundled: true
    note: "Product analytics endpoint used by several agent CLIs."

  - id: 01920000-0000-7000-8000-000000000004
    action: block
    match: { host: "**.sentry.io" }
    expires: never
    bundled: true
    note: "Crash reporting. Would carry stack traces including file paths."

  - id: 01920000-0000-7000-8000-000000000005
    action: block
    match: { host: "opencode.ai", path: "/share/**" }
    expires: never
    bundled: true
    note: "Session sharing uploads the whole conversation. Disabled by config; rule is the second lock."

  - id: 01920000-0000-7000-8000-000000000006
    action: block
    match: { host: "**.exa.ai" }
    expires: never
    bundled: true
    note: "Hosted web search backend for OpenCode's websearch tool."

  - id: 01920000-0000-7000-8000-000000000007
    action: block
    match: { host: "**.parallel.ai" }
    expires: never
    bundled: true
    note: "Hosted web search backend (Parallel)."

  - id: 01920000-0000-7000-8000-000000000008
    action: ask
    match: { host: "registry.npmjs.org", method: [GET, HEAD] }
    expires: never
    bundled: true
    note: "OpenCode installs provider packages on first use. Shown as a normal held request so the user learns what it is."
```

Regel-Engine-Erweiterung: `Rule.disabled: bool` (Default `false`, wird in `rules.yaml` des Nutzers als Override `disabled_bundled: [<id>, ...]` persistiert, nicht in `default.yaml`). `RuleSet::evaluate` überspringt `disabled` Regeln. `RuleSet::prepend_bundled(rules)` fügt vorne ein und setzt `bundled = true` erzwungen.

Metrik-Test: Sandbox-Start mit OpenCode-Adapter gegen Fake-LLM (HUM-046 liefert den Mock, für diesen Test reicht ein axum-Server, der `/v1/models` und `/api/tags` mit einem Modell beantwortet). Der Test wartet 15 s oder bis das PTY den Prompt-Marker zeigt, zählt dann `flows` mit `state == Held` oder `Decided(Block { reason: Rule(_) })`. Erwartung: `held <= 1` (der npm-Ask), `blocked_by_rule` beliebig.

### Schritte
1. `rules/default.yaml` anlegen. `humanitl rules test https://models.dev/api.json` (HUM-065) liefert `block · bundled`.
2. `disabled`-Flag und `disabled_bundled`-Override in `humanitl-rules` ergänzen, Tests.
3. `OpenCodeAdapter::default_rules()` via `include_str!` und `serde_yaml`. Ladefehler ist ein Panic beim Daemon-Start (die Datei ist Teil des Builds, nicht Nutzereingabe): Test stellt sicher, dass sie parst.
4. Metrik-Test schreiben, `#[ignore]` ohne Feature `agent-e2e`.

### Tests
- `default_rules_parse`: Datei parst, 8 Regeln, alle `bundled`.
- `default_rules_match_table`: `models.dev` ⇒ block; `api.github.com /repos/anomalyco/opencode/releases/latest` ⇒ block; `api.github.com /repos/foo/bar` ⇒ Default (ask); `eu.posthog.com` ⇒ block; `registry.npmjs.org GET` ⇒ ask, `POST` ⇒ Default (ask, aber ohne Regel-Treffer).
- `disabled_bundled_is_skipped`: Override deaktiviert Regel 1 ⇒ `models.dev` ⇒ Default.
- `startup_noise_budget` (ignore-Feature): `held <= 1`.

### Akzeptanzkriterien
- [ ] `humanitl rules list` zeigt die 8 Regeln mit `bundled` und Position vor allen Nutzerregeln. Gemessen: `humanitl rules list --all` zeigt zehn mitgelieferte Regeln mit `ORIGIN bundled`; ohne `--all` nennt die Fußzeile sie jetzt. Die Reihenfolge ist offen und wird in HUM-104 entschieden, nicht hier.
- [ ] Rules-Screen zeigt Badge „Bundled" und „Deaktivieren" statt „Löschen" für diese Regeln. Abzeichen, Schloss, eigener Block und der fehlende Papierkorb stehen; der Schalter „Deaktivieren" fehlt, weil die Dart-`Rule` das Feld `disabled` nicht kennt.
- [ ] `startup_noise_budget` grün im `agent-e2e`-Job. Es gibt weder den Test noch das Feature noch den Job; der Metriktest wartet auf den Modell-Mock aus HUM-046.

### Stand (2026-09-04): Regelsatz und Kommandozeile stehen, Reihenfolge offen (HUM-104), Oberfläche halb

Umgesetzt und gemessen ist alles, was ohne die Dart-Seite prüfbar ist:

- `rules/default.yaml` steht und wird über `include_str!` in `humanitld`
  gebunden. Beleg ist die Ausgabe, nicht die Zahl im Protokoll:
  `humanitl rules list --all` zeigt die zehn Regeln, jede mit
  `ORIGIN bundled` und ihrer festen Id. Die Zeile `rule store loaded` taugt
  dafür nicht — `main.rs:648` zählt `bundled.len()`, und darin steckt die
  Durchreichregel aus `main.rs:633`: ohne `llm.endpoint` steht dort
  `bundled=10`, mit gesetztem Endpunkt `bundled=11` (beides gemessen).
- Regel-Engine: `Rule.disabled` in `humanitl-core`,
  `RuleSet::set_disabled_bundled` und der Dateischlüssel `disabled_bundled`
  in `humanitl-rules`; `RuleSet::evaluate` überspringt eine abgeschaltete
  Regel, als gäbe es sie nicht. Ein `disabled_bundled`, das eine Regel
  derselben Datei benennt, warnt seit HUM-038 mit `RULES_010` und schaltet sie
  trotzdem ab; wer „aus" schreibt, bekommt nie das Gegenteil.
- Kommandozeile: `humanitl rules list --all` zeigt die zehn Regeln mit
  `ORIGIN bundled`, eine abgeschaltete als `bundled (off)`;
  `humanitl rules disable|enable ID` schaltet sie über
  `RulesRequest.set_disabled` ab und wieder an; `--json` trägt `bundled`,
  `disabled` und `origin`. Ohne `--all` nennt eine Fußzeile, was fehlt:
  `10 bundled rules and 1 passthrough rule also apply; humanitl rules list
  --all shows them`. Vorher stand der Hinweis nur im Zweig der leeren Liste,
  und sobald der Nutzer eine eigene Regel hatte, schwieg die Kommandozeile
  über elf Regeln, die entscheiden — während `cli.rs:287` verspricht, sie
  zeige die Regeln „in the order in which they are evaluated"
  (CONVENTIONS 4.13).

**Was die Fußzeile zählt, und warum getrennt.** Eine abgeschaltete oder
abgelaufene Regel steht nicht unter „gilt auch", sondern wird eigens
genannt: `8 bundled rules and 1 passthrough rule also apply, 2 rules are
switched off`. Die Alternative — eine Summe mit Nebensatz („10 bundled
rules also apply, 2 of them switched off") — nennt zuerst eine Zahl, die
nicht stimmt, und nimmt sie danach halb zurück; wer nur den ersten Halbsatz
liest, hat die falsche Zahl im Kopf. Es wäre außerdem derselbe Fehler, den
diese Bestandsaufnahme dem Regel-Bildschirm vorwirft: eine abgeschaltete
Regel wie eine wirksame zu zeichnen (CONVENTIONS 4.13). Vier Töpfe,
geprüft in dieser Reihenfolge: abgelaufen, abgeschaltet, Durchreiche,
mitgeliefert. Abgelaufen steht vorn, weil eine abgelaufene Regel auch dann
nichts entscheidet, wenn jemand sie wieder anschaltet. Jede verborgene Regel
fällt in genau einen Topf; die Zusage „Zeilen der Tabelle plus Summe der
Fußzeile ist die Zeilenzahl von `--all`" steht als Test
(`every_hidden_rule_is_counted_exactly_once`) und ist an einem laufenden
Daemon nachgerechnet: 1 + (8 + 1 + 2) = 12.

**`RuleSet::prepend_bundled` gehört nicht in diese Liste.** Die Funktion
(`daemon/crates/rules/src/eval.rs:394`) hatte außerhalb von Tests keinen
Aufrufer; alle vier Fundstellen waren Tests, darunter der grüne
`prepend_bundled_puts_the_bundled_rules_first`. Der Daemon baute seinen
Regelsatz über `RulesStore::load`, nicht über sie. Ihre Tests sagten deshalb
nichts über das laufende Produkt aus, und dieses Häkchen hätte auf sie nie
gestützt werden dürfen. HUM-104 hat das aufgelöst: Die Funktion heißt jetzt
`RuleSet::add_bundled`, hängt hinten an und wird von `RulesStore::snapshot_of`
gerufen.
- Tests grün: `default_rules_parse`, `default_rules_match_table`,
  `disabled_bundled_is_skipped`, `a_user_rule_overrides_a_bundled_rule`
  (`daemon/crates/sandbox/tests/default_rules.rs`), dazu in
  `daemon/crates/rules` die Datei-Tests von `disabled_bundled`
  (`tests/parse.rs`) und die Engine-Tests dazu (`tests/eval.rs`): eine
  abgeschaltete Regel entscheidet nichts, die nächste kommt dran, und ohne
  weitere Regel bleibt es bei `ask`.

**Offen und ausdrücklich nicht gedeckt:**

- **Der Schalter „Deaktivieren" im Rules-Screen.** Der Daemon kann es, die
  Oberfläche nicht: `app/lib/core/domain/rule.dart` kennt kein `disabled`,
  `app/lib/core/ipc/convert.dart` liest es nicht aus der Proto, und
  `DaemonClient` hat keine Operation dafür. Solange das fehlt, zeichnet der
  Bildschirm eine abgeschaltete mitgelieferte Regel wie eine wirksame und
  behauptet damit etwas, das nicht stimmt (CONVENTIONS 4.13). Es fehlen: ein
  Feld in `Rule`, eine Zeile in `RuleToDomain`, eine Methode
  `setRuleDisabled` in `DaemonClient`, `GrpcDaemonClient` und
  `FakeDaemonClient`, dann der Schalter in `rule_row.dart` samt seinen
  ARB-Schlüsseln.
- **`startup_noise_budget`.** Der Metriktest braucht einen Endpunkt, der
  `/v1/models` beantwortet; den liefert HUM-046. Bis dahin gibt es weder
  `daemon/crates/sandbox/tests/startup_noise.rs` noch das Feature `agent-e2e`
  noch einen CI-Job dieses Namens.

**Die Reihenfolge ist keine falsche Anforderung, sondern eine offene
Entscheidung. Sie wird in HUM-104 getroffen, und bis dahin bleibt das
Kästchen leer.** Das Repository widerspricht sich über genau diese Frage:

- **Für „mitgeliefert zuletzt":** HUM-027 (`backlog/sprint-2.md`) legt
  Sitzung, Nutzer, mitgeliefert fest, damit eine eigene Regel eine
  mitgelieferte überstimmen kann, ohne sie abschalten zu müssen.
  `rules_store.rs:20-24` schreibt es so hin, und `State::all`
  (`rules_store.rs:163-170`) setzt es um.
- **Für „Durchreiche und mitgeliefert zuerst":** `docs/profiles.md:171-175`
  aus HUM-066 sagt das Gegenteil — „die Durchreichregel des Agent-Adapters,
  dann seine mitgelieferten Regeln, dann die Dateien und Regeln der Profile,
  zuletzt die `rules.yaml` des Nutzers". Der Kommentar in `main.rs:629-634`
  meint dasselbe („Die Durchreiche steht vor allem anderen") und erreicht
  es nicht: er legt die Durchreiche in dieselbe Liste wie die mitgelieferten
  Regeln, und die hängt `State::all` hinten an.
- Beide Seiten berufen sich auf `backlog/CONVENTIONS.md` 4.5. Dort steht nur
  Sitzung vor dauerhaft; über die mitgelieferten Regeln schweigt der
  Abschnitt. Die Nummer trägt die Entscheidung also nicht, die sie belegen
  soll.

**Was daran heute schon kaputt ist**, und was HUM-104 zu reparieren hat:

- `profiles/llm-only.toml:35` bringt `block host "**"` mit, und sein
  Kommentar (Zeilen 8 bis 10) erklärt, die Durchreiche träfe davor. Sie trifft
  danach. Sobald `[rules].inline` des Profils verdrahtet ist, erreicht
  `humanitl run --profile llm-only` sein eigenes Sprachmodell nicht mehr.
- `pipeline.rs:350` erkennt die Durchreiche an der Regel, die entschieden hat.
  Trifft vorher eine `allow`-Regel des Nutzers, gibt es kein
  `DecisionSource::Passthrough` und kein `LLM_005`: der erklärte Seitenkanal
  (BACKLOG.md 4.2) wird still zu einem gewöhnlichen Allow.

**Ein Teil des Kriteriums bleibt trotzdem abgelehnt:**
„die 8 Regeln" und `humanitl rules list` ohne `--all`. Es sind zehn, und
die beiden zusätzlichen sind am installierten Binary 1.18.25 gemessen
(CONVENTIONS 4.17). `--all` ist der Schalter, unter dem HUM-065 mitgelieferte
und abgelaufene Regeln zeigt; ohne ihn zeigt die Liste, was dem Nutzer
gehört. Der Zweck des Kriteriums — niemand soll übersehen, dass mitgelieferte
Regeln mitentscheiden — ist seit der Fußzeile oben erfüllt.

### Fallstricke
- Hostnamen ändern sich. Jede Regel hat eine `note`, die erklärt, warum sie existiert, damit ein späterer Leser sie prüfen kann.
- `**.posthog.com` matcht auch `posthog.com` selbst (CONVENTIONS.md 3.3). Gewollt.
- Die npm-Regel darf nicht `allow` sein: Pakete können Postinstall-Skripte tragen. `ask` ist bewusst.
- Regel-IDs sind feste UUIDv7-Literale, damit `disabled_bundled` stabil referenzieren kann.

### Referenzen
BACKLOG.md 10 (Risiko 3), Abschnitt 5 (Usability: „Der erste Flow, den der Nutzer sieht, muss einer sein, den er ausgelöst hat"); CONVENTIONS.md 3.3. OpenCode CLI-Doku zu Env-Variablen (https://opencode.ai/docs/cli/).

---

## HUM-039 · LLM-Passthrough-Regel und Endpoint-Test
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-015, HUM-022, HUM-025, HUM-026, HUM-037 · Blockiert: HUM-041, HUM-044, HUM-067

### Kontext
Der Agent muss legitim Code ans LLM schicken, ohne dass jede Inferenz gehalten wird. ADR-006 und BACKLOG.md 4.2 verlangen: kein direktes Loch, sondern eine Passthrough-Regel im Proxy, eng gefasst (Host, Port, Pfadpräfix, POST), vollständig geloggt, Findings-Scan mit Warnung ohne Halt. Der Passthrough ist die deklarierte Vertrauensgrenze und wird im Isolation-Panel (HUM-041) amber angezeigt.

### Ziel
Aus `llm.endpoint` erzeugt der Adapter eine Regel mit `action: allow`, Flag `passthrough_llm: true`. Der Proxy behandelt Flows mit dieser Regel als `passthroughLlm`: kein Hold, Request-Body wird gepuffert (für Findings und Recorder), Response gestreamt und gespiegelt, Findings erzeugen ein `Diagnostic` `LLM_005` (Warning) pro Flow, das im Feed als amber Zeile erscheint. Der Setup-Screen hat ein Endpoint-Feld mit „Test", das host-seitig `GET /api/tags` (Ollama) oder `GET /v1/models` (OpenAI-kompatibel) aufruft und die Modellliste anbietet.

### Nicht-Ziel
Kein Halten von Passthrough-Flows (Post-MVP: findings-basiertes Halten). Keine TLS-Passthrough-Regel ohne MITM: ist `llm.endpoint` `https`, wird ganz normal terminiert. Kein Rate-Limit.

### Betroffene Pfade
- `daemon/crates/rules/src/lib.rs`: `Rule.passthrough_llm: bool`, `Matcher.path_prefixes: Vec<String>`
- `daemon/crates/sandbox/src/agent/opencode.rs`: `llm_passthrough()`
- `daemon/crates/proxy/src/flow.rs`: Passthrough-Pfad
- `daemon/crates/proxy/src/llm_probe.rs` (neu): Endpoint-Test
- `daemon/crates/ipc/src/service.rs`: RPC `ProbeLlm`
- `proto/humanitl/v1/humanitl.proto`: `ProbeLlmRequest { string endpoint }`, `ProbeLlmResponse { repeated string models; string flavor; Diagnostic diagnostic }`
- `app/lib/features/setup/widgets/llm_endpoint_field.dart` (neu)
- `app/lib/features/setup/providers/llm_probe_provider.dart` (neu)

### Spezifikation

Regel-Erzeugung:

```rust
fn llm_passthrough(&self, llm: &LlmConfig) -> Rule {
    let url = &llm.endpoint;                       // validated Url in config
    let host = HostName::parse(url.host_str().unwrap_or_default());   // Ip or Dns, normalized
    Rule {
        id: RuleId::from_u128(0x0192_0000_0000_7000_8000_0000_0000_00ff),
        action: Action::Allow,
        matcher: Matcher {
            host: HostPattern::Exact(host),
            method: Some(vec![Method::POST, Method::GET]),
            path_prefixes: llm.passthrough_paths.clone(),   // default ["/v1/", "/api/"]
            scheme: Some(url.scheme().parse()?),
            port: Some(url.port_or_known_default().unwrap_or(80)),
            upgrade: None,
        },
        expires: Expiry::Never,
        stream: false,
        created_from: None,
        bundled: true,
        passthrough_llm: true,
        note: Some(format!("LLM passthrough for {url}. Logged, never held.")),
    }
}
```

`GET` ist enthalten, weil OpenCode `GET /v1/models` zur Modellliste aufruft. `path_prefixes` ist ein neues Matcher-Feld: Treffer, wenn der Request-Pfad mit einem der Präfixe beginnt. Für IP-Literale wird `HostPattern::Exact(HostName::Ip(..))` verwendet; das ist die einzige Stelle, an der eine IP-Regel automatisch entsteht (CONVENTIONS.md 3.3: IPs matchen sonst nie).

Proxy-Pfad in `flow.rs`, nach `Analyzed`:

```rust
match ruleset.evaluate(&key, now, session) {
    Verdict::Matched { rule, action: Action::Allow } if ruleset.is_passthrough_llm(rule) => {
        state = state.on(&FlowEvent::Decided { id, decision: Decision::Allow, rule: Some(rule), passthrough: true })?;
        if !findings.is_empty() {
            diagnostics.emit(Diagnostic {
                code: DiagnosticCode("LLM_005"), severity: Severity::Warning,
                title: t!("llm.findings_in_prompt.title"),
                why: t!("llm.findings_in_prompt.why", count = findings.len(), host = key.host),
                fix: None, docs: Some(DOCS_LLM_TRUST_BOUNDARY),
            }.for_flow(id));
        }
        forward_streaming(flow).await   // request body already buffered; response streamed + mirrored
    }
    ...
}
```

`FlowEvent::Decided` bekommt das Feld `passthrough: bool`; die UI leitet daraus den Zustand `passthroughLlm` ab (Farbe #B48AF0, Icon `chevrons-right`). Der Recorder speichert Passthrough-Flows wie alle anderen, `flows.decision = "allow"`, `flows.rule_id = <passthrough id>`.

Endpoint-Probe (`llm_probe.rs`), host-seitig, nie aus der Sandbox:

```rust
pub enum LlmFlavor { Ollama, OpenAiCompatible, Unknown }
pub struct ProbeResult { pub flavor: LlmFlavor, pub models: Vec<String>, pub latency_ms: u32 }
pub async fn probe(endpoint: &Url, timeout: Duration) -> Result<ProbeResult, Diagnostic>;
```

Ablauf: (1) `GET {endpoint}/api/tags`, bei 200 und JSON mit `models[].name` ⇒ `Ollama`. (2) Sonst `GET {endpoint}/v1/models`, bei 200 und `data[].id` ⇒ `OpenAiCompatible`. (3) Beides 404 ⇒ `Unknown` mit leerer Liste und `LLM_003`. Timeout 3 s. Keine Redirect-Verfolgung. Keine Cookies. Nur `http` und `https`; bei `https` wird der System-Trust-Store des Hosts genutzt, nicht die Humanitl-CA.

Diagnostics:

| Code | Severity | Auslöser | why (en) | fix |
|---|---|---|---|---|
| `LLM_001` | Blocking | TCP-Connect fehlgeschlagen oder Timeout | "Humanitl could not reach {host}:{port} from this machine. The agent will not be able to talk to the model." | `ChangeSetting { key: "llm.endpoint" }`, plus `CopyCommand("curl -sS {endpoint}/api/tags")` |
| `LLM_002` | Error | Verbindung ok, HTTP-Status 401/403 | "The LLM server answered {status}. It requires authentication that Humanitl does not send in the MVP." | `OpenUrl(docs)` |
| `LLM_003` | Warning | Verbindung ok, weder Ollama- noch OpenAI-Pfad gefunden | "Connected, but neither /api/tags nor /v1/models answered. Check that the URL points at the API root, not a chat UI." | `ChangeSetting { key: "llm.endpoint" }` |
| `LLM_004` | Warning | keine Modelle konfiguriert | siehe HUM-037 | `ChangeSetting { key: "llm.models" }` |
| `LLM_005` | Warning | Findings im Passthrough-Request | "This request to your LLM contains {count} potential secret(s) or personal data. It was sent because the LLM endpoint is a declared trust boundary." | keine, `docs` |
| `LLM_006` | Info | Endpoint ist keine private Adresse (nicht RFC1918, nicht Loopback, nicht `.local`/`.lan`/`.home.arpa`) | "This endpoint is not on a private network. Only put a machine you control here." | `ChangeSetting { key: "llm.endpoint" }` |

Setup-Widget `LlmEndpointField`: Textfeld (Placeholder `http://192.168.1.50:11434`), Button „Test" (HButton secondary), unter dem Feld genau ein Satz (ARB `setup_llm_passthrough_note`: en "Traffic to this address bypasses the queue. Only put a machine you control here." / de "Verkehr zu dieser Adresse umgeht die Warteschlange. Trage hier nur eine Maschine ein, die du kontrollierst."). Nach erfolgreichem Test: Chip mit Flavor und Latenz, Multi-Select der Modelle, „Übernehmen" schreibt `llm.models`. Bei Diagnostic: Inline-Karte unter dem Feld mit `title`, `why`, Fix-Button.

### Schritte
1. `Matcher.path_prefixes`, `Rule.passthrough_llm`, `RuleSet::is_passthrough_llm` in `humanitl-rules` mit Tests.
2. `llm_passthrough()` im Adapter.
3. Proxy-Pfad: Passthrough-Zweig, `Decided.passthrough`, `LLM_005`-Emission. Integrationstest gegen axum-Fake mit SSE-Antwort.
4. `llm_probe.rs` + `ProbeLlm`-RPC + Diagnostics `LLM_001..003, 006`.
5. Config: `llm.endpoint` (Tier basic), `llm.passthrough_paths` (advanced), `llm.models` (basic), `llm.probe_timeout_ms` (expert, Default 3000).
6. Flutter: `LlmEndpointField`, Provider, ARB-Strings, Widget-Test.

### Tests
- `passthrough_rule_matches_only_endpoint`: Endpoint `http://192.168.1.50:11434`; `POST 192.168.1.50:11434 /v1/chat/completions` ⇒ allow passthrough; `POST 192.168.1.50:11434 /admin` ⇒ Default; `POST 192.168.1.50:8080 /v1/x` ⇒ Default; `POST 192.168.1.51:11434 /v1/x` ⇒ Default; `DELETE 192.168.1.50:11434 /v1/x` ⇒ Default.
- `passthrough_streams_sse`: Fake-Upstream sendet 50 SSE-Chunks mit 20 ms Abstand; Client erhält den ersten Chunk unter 100 ms nach Request-Ende (nicht gepuffert); Recorder enthält den vollständigen Body.
- `passthrough_findings_emit_llm_005`: Body enthält `ghp_` + 36 Zeichen ⇒ genau ein `LLM_005`, Flow ist trotzdem `Forwarded`.
- `probe_detects_ollama`, `probe_detects_openai`, `probe_unknown_paths_llm_003`, `probe_timeout_llm_001`, `probe_public_ip_llm_006`.
- Widget-Test `LlmEndpointField`: Test-Klick ruft Provider, Erfolg zeigt Modell-Chips, Diagnostic zeigt Karte mit Fix-Button.

### Akzeptanzkriterien
- [ ] `humanitl rules test http://192.168.1.50:11434/v1/chat/completions --method POST` ⇒ `allow · passthrough_llm · bundled`.
- [ ] History-Screen zeigt Passthrough-Flows in Violett mit Regel-Chip, standardmäßig eingeklappt (Filter-Chip „LLM anzeigen").
- [ ] Setup: Test gegen laufendes Ollama liefert Modellliste in unter 3 s.
- [ ] Setup: Test gegen `http://10.255.255.1:1` liefert `LLM_001` mit kopierbarem curl-Befehl.
- [ ] `LLM_005` erscheint im Feed als amber Zeile, öffnet die Flow-Details.

### Fallstricke
- Der Passthrough darf nie `stream: true` für den Request-Body bekommen. ADR-005 gilt auch hier: Request wird gepuffert, nur die Response streamt.
- `path_prefixes` mit leerem Präfix `""` würde alles matchen. Validierung: jeder Präfix beginnt mit `/` und ist mindestens 2 Zeichen.
- `llm.endpoint` mit Pfadanteil (`http://host:1/v1`) muss den Pfadanteil im Matcher ignorieren, aber in `opencode.json` korrekt weiterreichen (HUM-037 Test `appends_v1_only_once`).
- Die Probe läuft auf dem Host mit Host-DNS. Ein Hostname wie `ollama.lan` funktioniert in der Probe, aber der Proxy löst denselben Namen erst nach Regel-Treffer auf (ADR-006). Beides muss dasselbe Resolver-Verhalten haben; Test mit `/etc/hosts`-Eintrag im CI.
- Ollama antwortet auf `/v1/models` ebenfalls; Reihenfolge der Probe ist deshalb Ollama zuerst, damit der Flavor stimmt.

### Referenzen
BACKLOG.md 4.2, 4.3, ADR-005, ADR-006, Abschnitt 5 (Usability §1 LLM-Feld); CONVENTIONS.md 3.3, 3.5. Ollama API (`/api/tags`), OpenAI-kompatible Modelle-Liste (`/v1/models`).

---

## HUM-066 · Profile

> **Abgleich 2026-09-02**: Das Profil-Format ist durch HUM-062 festgelegt: `name`, `description`, `[config.<gruppe>]` für Konfigurationswerte, daneben `[rules]` und `[agent]`. Flache Blöcke wie `[hold]` auf der obersten Ebene, wie sie unten skizziert sind, lehnt der Loader mit `CONFIG_002` ab; die Beispiele in diesem Issue sind entsprechend zu lesen (`[config.hold]`).

Sprint: 3 · Größe: M · Abhängigkeiten: HUM-062, HUM-010, HUM-037, HUM-039 · Blockiert: HUM-040, HUM-067

### Kontext
ADR-011: Wenig tun müssen, viel tun können. Ein Profil bündelt alles, was eine Session ausmacht, und ist eine Datei, die global oder pro Projekt liegt. `humanitl run --profile llm-only` und das UI nutzen dieselben Profile. Profile sind auch der spätere Erweiterungspunkt für Team-Profile (BACKLOG.md Abschnitt 6).

### Ziel
`humanitl-config` kennt den Typ `Profile`, lädt Profile aus drei Orten mit fester Präzedenz, liefert die aufgelöste `EffectiveConfig` mit `Origin` pro Feld, und die zwei mitgelieferten Profile `default` und `llm-only` existieren als Dateien und als eingebettete Fallbacks. Das Setup (HUM-044) und die CLI (HUM-067) wählen Profile über denselben Loader.

### Nicht-Ziel
Kein Profil-Editor im UI (HUM-069 Settings-Screen zeigt die aufgelöste Config, Profil-Bearbeitung ist Datei-basiert im MVP). Kein Import aus URLs.

### Betroffene Pfade
- `daemon/crates/config/src/profile.rs` (neu)
- `daemon/crates/config/src/resolve.rs`: Präzedenz erweitern
- `profiles/default.toml` (neu), `profiles/llm-only.toml` (neu)
- `daemon/crates/config/tests/profiles.rs` (neu)
- `daemon/bin/humanitl/src/cmd/run.rs`: `--profile`
- `docs/profiles.md` (neu)

### Spezifikation

Profil-Format (alle Felder optional, fehlende Felder erben aus der nächstniedrigeren Ebene):

```toml
# profiles/default.toml
name = "default"
description = "Ask for everything not covered by a rule. Full moderation with UI."

[llm]
# endpoint is intentionally absent: it comes from config.toml (basic setup)
passthrough_paths = ["/v1/", "/api/"]

[hold]
timeout_secs = 300
body_cap_bytes = 33554432
ask_mode = "ui"            # ui | terminal | none

[sandbox]
profile = "default"        # references profiles/sandbox/default.toml
work_mode = "rw"
[sandbox.mounts]
cache = true               # persistent per-project cache volume for the agent
extra_ro = []              # additional host paths mounted read-only, absolute
extra_rw = []
[sandbox.env]
# additional environment variables for the agent, merged after the adapter's env

[agent]
adapter = "opencode"
# command = ["opencode"]  # override

[rules]
files = ["rules.yaml"]     # relative to the profile's directory or absolute; merged in order after bundled rules
inline = []                # rules written directly in the profile, same schema as rules.yaml

[recorder]
inline_max_bytes = 262144
retention_days = 90
```

```toml
# profiles/llm-only.toml
name = "llm-only"
description = "Pure inference. Only the configured LLM endpoint is reachable; everything else is blocked without asking. No UI needed."

[hold]
ask_mode = "none"
timeout_secs = 1

[sandbox]
profile = "default"
work_mode = "rw"

[agent]
adapter = "opencode"

[rules]
inline = [
  { action = "block", match = { host = "**" }, expires = "never", note = "llm-only profile: block everything that is not the LLM passthrough" },
]
```

Die Passthrough-Regel wird vom Adapter vorangestellt und trifft vor der `**`-Blockregel. `ask_mode = "none"` bedeutet: `Verdict::Default` wird sofort zu `Decision::Block { reason: BlockReason::Rule(NONE_MODE_RULE_ID) }`, ohne Hold; `timeout_secs` ist dann irrelevant, wird aber auf 1 gesetzt, damit ein Fehlkonfiguration nicht hängt.

Präzedenz (niedrig nach hoch), erweitert aus CONVENTIONS.md 3.7:

1. Eingebaute Defaults (`Config::default()`)
2. `$XDG_CONFIG_HOME/humanitl/config.toml`
3. Profil `default` (eingebettet via `include_str!`, überschrieben durch `$XDG_CONFIG_HOME/humanitl/profiles/default.toml`, falls vorhanden)
4. Gewähltes Profil, falls nicht `default`: `$XDG_CONFIG_HOME/humanitl/profiles/<name>.toml`, sonst eingebettetes `<name>` (nur `llm-only` ist eingebettet)
5. Projekt-Profil `<work_dir>/.humanitl/profile.toml` (falls vorhanden; darf `name` setzen, dann wird zuerst dieses benannte Profil aus Ebene 4 geladen und danach die Projekt-Datei darüber gelegt)
6. Umgebungsvariablen `HUMANITL_*`
7. CLI-Flags

```rust
pub struct Profile { pub name: String, pub description: Option<String>, pub overlay: ConfigOverlay }  // ConfigOverlay = Config mit allen Feldern Option<T>
pub enum Origin { Default, GlobalConfig, ProfileBuiltin(String), ProfileGlobal(PathBuf), ProfileProject(PathBuf), Env(String), Cli(String) }
pub struct EffectiveConfig { pub config: Config, pub origins: BTreeMap<String /* dotted key */, Origin>, pub profile_chain: Vec<Origin> }
pub fn resolve(sel: &ProfileSelection, work_dir: Option<&Path>, env: &[(String,String)], cli: &CliOverrides) -> Result<EffectiveConfig, Diagnostic>;
pub struct ProfileSelection { pub name: Option<String> }   // None = "default" or project's name
```

Sicherheitsregel: Ein Projekt-Profil darf `sandbox.mounts.extra_ro`/`extra_rw` nicht setzen (sonst könnte ein Repository beim Öffnen Host-Pfade in die Sandbox holen). Enthält `.humanitl/profile.toml` diese Schlüssel, liefert `resolve` `CONFIG_003` (Blocking): "The project profile tries to mount host paths. Only global profiles may do that." Ebenso verboten im Projekt-Profil: `agent.command`, `sandbox.profile` (der bwrap-Profilname), `hold.ask_mode = "none"` ist erlaubt.

Diagnostics: `CONFIG_001` (Blocking) Profil nicht gefunden, `fix: CopyCommand("humanitl config schema --profiles")`; `CONFIG_002` (Blocking) TOML-Parse-Fehler mit Zeile/Spalte; `CONFIG_003` wie oben; `CONFIG_004` (Warning) Projekt-Profil vorhanden, aber Verzeichnis nicht dem Nutzer gehörend (Datei eines Fremden).

### Schritte
1. `ConfigOverlay` per Makro oder manuell als Option-Spiegel von `Config`; Merge-Funktion `Config::apply(&mut self, &ConfigOverlay, origin)` mit Origin-Tracking. Tests.
2. `Profile`-Loader: Datei lesen, `name` validieren (`^[a-z0-9-]{1,32}$`), eingebettete Profile registrieren.
3. `resolve()` mit der Präzedenz-Kette, Projekt-Profil-Verbote.
4. Die zwei Profil-Dateien anlegen, `include_str!` einbinden.
5. CLI: `--profile NAME` in `run` und `sandbox run|argv`; `humanitl config get hold.ask_mode --profile llm-only` zeigt Wert und Origin.
6. `docs/profiles.md` mit Format, Präzedenz, Verboten.

### Tests
- `builtin_profiles_parse`: beide Dateien parsen, Namen stimmen.
- `precedence_global_profile_project_env_cli`: Setze `hold.timeout_secs` auf jeder Ebene mit verschiedenen Werten; Ergebnis ist der CLI-Wert, `origins["hold.timeout_secs"] == Origin::Cli`.
- `project_profile_inherits_named`: `.humanitl/profile.toml` mit `name = "llm-only"` und `[hold] timeout_secs = 7` ⇒ `ask_mode == none`, `timeout_secs == 7`.
- `project_profile_cannot_mount`: `extra_rw` im Projekt-Profil ⇒ `CONFIG_003`.
- `unknown_profile_config_001`.
- `llm_only_blocks_everything_but_passthrough`: RuleSet aus Profil + Adapter; `POST llm-host /v1/chat` ⇒ allow; `GET github.com /` ⇒ block; kein Held.

### Akzeptanzkriterien
- [ ] `humanitl config get --profile llm-only hold.ask_mode` ⇒ `none (origin: profile builtin llm-only)`.
- [ ] `humanitl config schema --profiles` listet `default`, `llm-only` und alle Dateien unter `profiles/` mit Beschreibung.
- [ ] Ein Projekt mit `.humanitl/profile.toml`, das `extra_rw` setzt, verweigert den Start mit `CONFIG_003` in CLI und UI.
- [ ] `docs/profiles.md` existiert und enthält die Präzedenztabelle.

### Fallstricke
- Overlay-Merge muss feldweise sein, nicht tabellenweise: `[hold]` im Projekt-Profil mit nur `timeout_secs` darf `ask_mode` aus dem benannten Profil nicht auf Default zurücksetzen.
- Listen (`rules.files`, `extra_ro`) werden ersetzt, nicht konkateniert. Dokumentieren.
- Pfade in `rules.files` relativ zur Profil-Datei auflösen, nicht zum cwd.
- `name` im Projekt-Profil darf nicht `default` mit anderem Inhalt „überschreiben"; der Name bezeichnet nur die Basis.

### Referenzen
BACKLOG.md ADR-011, Abschnitt 6 (Profile), 3.5; CONVENTIONS.md 3.7, 3.8.

---

## HUM-040 · Sandbox-Screen
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-019, HUM-008, HUM-018, HUM-066 · Blockiert: HUM-041, HUM-042, HUM-068

### Kontext
Der Sandbox-Screen ist der Ort, an dem der Nutzer sieht, was der Agent bekommt: Projektordner, Modus, Mounts, Env, Status. Usability-Review: der Nutzer fürchtet „bekommt der Agent meine ganze Platte?", also steht dort wörtlich „Der Agent sieht nur `/work` = ~/clients/acme. Nichts sonst." Start/Stop lebt hier; Stop bei laufendem Agent ist eine der wenigen Modal-Bestätigungen (BACKLOG.md Abschnitt 5).

### Ziel
Screen `SandboxScreen` unter `features/sandbox` mit Header (Status, Start/Stop, Profil-Chip), Terminal-Bereich oben (Platzhalter bis HUM-042), Tabs unten: Mounts, Env, Isolation (Platzhalter bis HUM-041), Log. Alle Daten kommen über `sandboxStatusProvider` und `configProvider` vom Daemon.

### Nicht-Ziel
Terminal-Inhalt (HUM-042), Isolation-Panel (HUM-041), Diagnostics-Inhalte (HUM-068). Keine Profil-Bearbeitung.

### Betroffene Pfade
- `app/lib/features/sandbox/sandbox_screen.dart` (neu)
- `app/lib/features/sandbox/providers/sandbox_status_provider.dart` (neu)
- `app/lib/features/sandbox/widgets/{sandbox_header.dart, mounts_tab.dart, env_tab.dart, log_tab.dart, stop_dialog.dart, work_dir_picker.dart}` (neu)
- `app/lib/core/domain/sandbox.dart` (neu): `SandboxStatus`, `MountEntry`, `EnvEntry`
- `app/l10n/app_en.arb`, `app_de.arb`: Schlüssel `sandbox_*`
- `proto/humanitl/v1/humanitl.proto`: `SandboxRequest { oneof { Start, Stop, Status } }`, `SandboxEvent { oneof { Status, LogLine, IsolationResult, Diagnostic } }`, `SandboxStatus { state; session_id; started_at; profile; work_dir_host; work_mode; repeated Mount mounts; repeated EnvVar env; string argv_preview }`
- `app/test/features/sandbox/*_test.dart`

### Spezifikation

Domain:

```dart
@freezed
class SandboxStatus with _$SandboxStatus {
  const factory SandboxStatus({
    required SandboxState state,            // stopped | starting | running | stopping | failed
    SessionId? sessionId,
    DateTime? startedAt,
    required String profile,
    String? workDirHost,
    required WorkMode workMode,             // ro | rw
    required List<MountEntry> mounts,       // dst, src?, mode (ro|rw|tmpfs|masked), origin (profile|adapter|session)
    required List<EnvEntry> env,            // key, value, origin, secret(bool) -> value rendered as ••••
    required String argvPreview,            // full bwrap argv, one string
    required List<Diagnostic> diagnostics,
  }) = _SandboxStatus;
}
```

Widget-Baum:

```
SandboxScreen
└─ Column
   ├─ SandboxHeader (40px)
   │  ├─ HStatusDot(state) + Text(sandbox_state_<state>)
   │  ├─ HChip(profile)               // "default" · click -> Settings
   │  ├─ Spacer
   │  ├─ WorkDirPicker (compact)       // "~/clients/acme · rw"  click -> file_picker + ro/rw toggle; disabled while running
   │  └─ HButton.primary(Start) | HButton.secondary(Stop)
   ├─ HResizable(vertical, initial 0.6)
   │  ├─ TerminalPane (placeholder: HPanel with sandbox_terminal_placeholder)
   │  └─ HTabs [Mounts, Env, Isolation, Log]
   │     ├─ MountsTab: sentence + TableView(dst, src, mode, origin)
   │     ├─ EnvTab: TableView(key, value|••••, origin), search field, "reveal" per row (toggle, no clipboard)
   │     ├─ IsolationTab: placeholder (HUM-041)
   │     └─ LogTab: ListView.builder of LogLine (mono 12), auto-scroll with pause on user scroll
   └─ HStatusBar: session id short, uptime, "argv anzeigen" link -> Sheet with argvPreview (mono, selectable, copy)
```

Satz in `MountsTab` (ARB `sandbox_mounts_sentence`): en "The agent sees only `/work` = {hostPath} ({mode}). Nothing else from this machine." de "Der Agent sieht nur `/work` = {hostPath} ({mode}). Sonst nichts von diesem Rechner." Wenn `extra_ro`/`extra_rw` aus einem globalen Profil aktiv sind, wird der Satz ergänzt: en "Plus {n} additional path(s) from your profile:" gefolgt von der Liste.

Start-Ablauf: Klick auf Start ⇒ `Sandbox(Start { profile, work_dir, work_mode })` ⇒ Daemon streamt `Status(starting)`, dann `IsolationResult` × 3 (HUM-041), dann `Status(running)` oder `Diagnostic` + `Status(failed)`. Der Start-Button ist deaktiviert, wenn `diagnosticsProvider` ein `Blocking` enthält, und zeigt Tooltip mit dessen `title`.

Stop-Dialog (einzige Modal in diesem Screen): Titel `sandbox_stop_title` ("Stop the agent?"), Body `sandbox_stop_body` ("The agent is running. Stopping ends its session; unsaved work inside the agent's terminal is lost. Files in /work stay."), Buttons „Abbrechen" (default focus), „Stoppen" (destructive). Ist `state == running` und kein Agent-Prozess mehr aktiv (Exit erkannt), wird ohne Dialog gestoppt.

`WorkDirPicker`: `file_picker.getDirectoryPath()`, danach Toggle `ro | rw` (Default aus Profil). Schreibt `sandbox.work_dir`, `sandbox.work_mode` über `configProvider.set(...)` (Origin `Cli`-äquivalent „UI", persistiert in `config.toml`). Deaktiviert bei `running`.

### Schritte
1. Proto-Messages ergänzen, Codegen. Daemon: `Sandbox`-RPC-Handler liefert `Status` aus dem laufenden `SessionManager`.
2. Domain-Typen in Dart (freezed), Mapping aus Proto.
3. `sandboxStatusProvider` als `StreamNotifier`, der bei `Subscribe`-Verbindung `Sandbox(Status)` anfragt und Events einarbeitet.
4. Header, Picker, Tabs, Stop-Dialog. Alle Strings in ARB.
5. Widget-Tests mit `FakeDaemonClient`.
6. Golden: Header in `stopped`, `running`, `failed`.

### Tests
- `start_button_disabled_when_blocking_diagnostic`.
- `stop_shows_dialog_when_running`, `stop_without_dialog_when_agent_exited`.
- `mounts_sentence_renders_host_path_and_mode`.
- `env_tab_masks_secret_values`: Env-Eintrag mit `secret: true` rendert `••••`, Reveal-Toggle zeigt Wert, kein Copy-Button.
- `argv_sheet_shows_full_command`: Sheet enthält `--unshare-all`.
- `workdir_picker_disabled_while_running`.

### Akzeptanzkriterien
- [ ] `flutter test test/features/sandbox` grün.
- [ ] Manuell mit echtem Daemon: Start zeigt innerhalb 2 s `running`, Mounts-Tab listet `/work`, `/run/humanitl/proxy.sock`, `/etc/humanitl/ca.crt`, Env-Tab listet `HTTP_PROXY`.
- [ ] Goldens für drei Header-Zustände abgelegt.
- [ ] Stop bei laufendem Agent verlangt Dialog, `Esc` bricht ab.

### Fallstricke
- Env-Werte können Secrets enthalten (spätere Credential-Injection). `secret: true` wird vom Daemon gesetzt für Schlüssel, die auf `_TOKEN`, `_KEY`, `_SECRET`, `PASSWORD` enden. Nie in Logs.
- `argvPreview` kann 2 KB lang sein; Sheet mit `SelectableText` und horizontalem Scroll, kein Umbruch mitten in Pfaden.
- Der Picker liefert unter Wayland-Portalen manchmal `null` bei Abbruch; `null` ist kein Fehler.
- Tabs behalten Scrollposition (`AutomaticKeepAliveClientMixin` oder `PageStorageKey`).

### Referenzen
BACKLOG.md Abschnitt 5 (IA, Modal-Regel, Usability §1 Projektordner); CONVENTIONS.md 3.9. file_picker (https://pub.dev/packages/file_picker).

---

## HUM-041 · Isolation-Check-Panel und Ring
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-011, HUM-012, HUM-013, HUM-039, HUM-040 · Blockiert: HUM-044, HUM-046

### Kontext
Die drei Garantien (BACKLOG.md 4.1) sind nur dann ein Argument, wenn der Nutzer sie live sehen kann. Usability: der Check ist der Reassurance-Moment bei jedem Start, drei Zeilen animieren auf grün, eine vierte amber Zeile zeigt die LLM-Ausnahme. Der Ring im Header (Signature-Element 2) ist das Produktversprechen, immer sichtbar. Fehlschlag deaktiviert den Start, nie „trotzdem starten".

### Ziel
Der Daemon führt beim Session-Start drei Prüfungen aus, deren Beweise aus der laufenden Sandbox stammen (nicht aus dem Host), streamt sie als `IsolationResult`, und das UI zeigt Panel und Ring. Der Ring hat drei Segmente, jedes Segment entspricht einer Prüfung. Der Agent wird erst `exec`t, wenn alle drei bestanden sind.

### Nicht-Ziel
Keine Prüfungen der Regel-Engine oder des Proxys (ESC-3/4 sind CI-Tests, nicht Laufzeit). Kein periodisches Re-Checking im MVP (Post-MVP: alle 60 s).

### Betroffene Pfade
- `daemon/bin/humanitl-shim/src/main.rs`: Check-Phase vor `exec`
- `daemon/crates/sandbox/src/isolation.rs` (neu): Auswertung der Shim-Meldungen
- `daemon/crates/sandbox/src/bwrap.rs`: Report-Pipe
- `app/lib/features/sandbox/widgets/isolation_panel.dart` (neu)
- `app/lib/features/sandbox/widgets/isolation_ring.dart` (neu)
- `app/lib/app.dart`: Ring im Header
- `app/lib/features/sandbox/providers/isolation_check_provider.dart` (neu)
- `tests/escape/esc-1-sockets.sh`, `esc-2-mounts.sh` (bestehend aus HUM-006): nutzen dieselben Beweise
- ARB: `isolation_*`

### Spezifikation

Ablauf im Shim (`humanitl-shim`, kein tokio, nur `libc`/`nix`):

1. Shim wird von bwrap als PID 1 des Sandbox-PID-Namespace gestartet mit Argumenten `--report-fd 3 --proxy-sock /run/humanitl/proxy.sock --proxy-port 3128 --extra-bridge 3129:/run/humanitl/models.sock -- <agent argv>`.
2. Startet socat-Bridge(n) als Kindprozesse. Wartet per `connect()` auf `127.0.0.1:3128`, bis Listen bereit ist (max 2 s, 20 ms Schritte).
3. **Check 1 `NoNetworkInterface`:** Liest `/sys/class/net/` (Verzeichnisliste) und `/proc/net/dev`. Bestanden, wenn beide genau `lo` enthalten. Evidence-String: `"interfaces: lo"`. Fallback, wenn `/sys` nicht gemountet ist: nur `/proc/net/dev`.
4. **Check 2 `SingleSocket`:** Liest `/proc/net/unix`, zählt Zeilen mit Pfad (Spalte 8, nicht leer, nicht mit `@` beginnend). Erlaubt: genau die Pfade aus `--proxy-sock` und `--extra-bridge`. Zusätzlich `walk /` mit `nftw`, zählt `S_IFSOCK`-Einträge (max. Tiefe 8, `/proc` und `/sys` ausgelassen). Bestanden, wenn beide Mengen ⊆ erlaubte Pfade. Evidence: `"sockets: /run/humanitl/proxy.sock"`. Prüft außerdem, dass `/run/humanitl/daemon.sock` und `$XDG_RUNTIME_DIR` nicht existieren.
5. Schreibt Ergebnis von Check 1 und 2 als eine Zeile JSON pro Check auf `--report-fd`: `{"check":"NoNetworkInterface","passed":true,"evidence":"interfaces: lo"}`.
6. Setzt seccomp-Filter (HUM-012). Danach:
7. **Check 3 `SeccompActive`:** Liest `/proc/self/status`, Zeile `Seccomp:`, erwartet `2`. Ruft `socket(AF_INET, SOCK_STREAM, 0)` auf und erwartet Erfolg (Loopback zum Proxy ist erlaubt); `socket(AF_UNIX, SOCK_STREAM, 0)` und `socket(AF_INET, SOCK_DGRAM, 0)` erwarten `-1` mit `errno == EPERM`; `socketpair(AF_UNIX)` bleibt erlaubt (kein Egress, CONVENTIONS 4.11). Gelesen wird `/proc/self/status` eines gefilterten Kindes, nie `/proc/1/status` (PID 1 ist bwraps Init). Evidence: `"seccomp: 2, socket(AF_INET,SOCK_STREAM)=ok, socket(AF_UNIX)=EPERM, socket(AF_INET,SOCK_DGRAM)=EPERM, socketpair=ok"`. Fehlschlag ⇒ `SANDBOX_016`; kein Bericht ⇒ `SANDBOX_013`; Check 1 ⇒ `SANDBOX_014`; Check 2 ⇒ `SANDBOX_015`. Schreibt Zeile auf Report-FD.
8. Schließt Report-FD (EOF ist das Signal „Checks fertig"). Wenn ein Check fehlgeschlagen ist: `exit(3)` ohne `exec`. Sonst `execvp(agent argv)`.

Daemon-Seite (`isolation.rs`): liest die Report-Pipe bis EOF, parst die drei Zeilen, streamt je ein `SandboxEvent::IsolationResult`. Fehlt eine Zeile (Shim abgestürzt) ⇒ alle fehlenden Checks `passed: false`, Evidence `"no report from shim"`, Diagnostic `SANDBOX_010`. Danach:

| Ergebnis | Aktion |
|---|---|
| alle bestanden | `Status(running)`, Agent läuft |
| Check 1 fehlgeschlagen | `SANDBOX_011` (Blocking): "The sandbox has a network interface other than loopback ({evidence}). This should be impossible with --unshare-net. Refusing to start." `fix: CopyCommand("bwrap --version")`, docs Link |
| Check 2 fehlgeschlagen | `SANDBOX_012` (Blocking): "Unexpected socket(s) inside the sandbox: {evidence}. A mount in your profile exposes a host socket." `fix: ChangeSetting { key: "sandbox.mounts" }` |
| Check 3 fehlgeschlagen | `SANDBOX_013` (Blocking): "The seccomp filter is not active ({evidence}). Kernel or bubblewrap too old, or seccomp disabled." `fix: CopyCommand("uname -r && bwrap --version")` |

`CheckResult` (CONVENTIONS.md 3.4) trägt `evidence` und optional `diagnostic`.

Vierte Zeile (Ausnahme): kein Check, sondern aus `configProvider`: `llm.endpoint` plus Passthrough-Regel. Wenn kein Endpoint gesetzt ist, zeigt die Zeile „Keine LLM-Ausnahme konfiguriert" in grau.

Panel-Layout (`IsolationTab`):

```
┌ Isolation ────────────────────────────────────────────────┐
│ ● No network interface. There is nowhere for traffic to go. │  evidence: interfaces: lo
│ ● Exactly one door: a socket that leads to Humanitl.        │  evidence: sockets: /run/humanitl/proxy.sock
│ ● The kernel refuses to open any new door (seccomp).        │  evidence: seccomp: 2, socket()=EPERM
│ ◐ Exception: LLM at 192.168.1.50:11434 — passthrough,       │  [change]
│   logged, never held.                                       │
│                                                             │
│ ▸ Show the exact sandbox command                            │
└─────────────────────────────────────────────────────────────┘
```

Strings (ARB `isolation_check_1..3`, `isolation_exception`, `isolation_exception_none`, `isolation_show_argv`):
- en 1: "No network interface. There is nowhere for traffic to go." / de: "Kein Netzwerk-Interface. Es gibt keinen Weg nach draußen."
- en 2: "Exactly one door: a socket that leads to Humanitl." / de: "Genau eine Tür: ein Socket, der zu Humanitl führt."
- en 3: "The kernel refuses to open any new door (seccomp)." / de: "Der Kernel verweigert jede neue Tür (seccomp)."
- en Ausnahme: "Exception: LLM at {endpoint} — passthrough, logged, never held." / de: "Ausnahme: LLM unter {endpoint}, Passthrough, geloggt, nie angehalten."

Animation: Zeilen erscheinen nacheinander mit 120 ms Versatz, Punkt wechselt von `fg-2` auf `allowed`-Grün mit 200 ms Fade, wenn das Ergebnis eintrifft. Fehlschlag: Punkt `blocked`-Rot, Zeile bekommt darunter die Diagnostic-Karte mit Fix-Button. Evidence in `fg-2`, Mono 11, rechts.

Ring (`IsolationRing`, 20 px, im Header rechts neben dem Sandbox-Glyph): `CustomPainter`, drei Bogen-Segmente à 110° mit 10° Lücke, Strichstärke 2 px. Segment-Farbe: `fg-2` (unbekannt/gestoppt), `allowed` (bestanden), `blocked` (fehlgeschlagen), `held`-Amber pulsierend (läuft). Klick ⇒ `NavIntent(4)` und Tab `Isolation`. Tooltip: „3/3 Isolation checks passed" oder „Isolation check failed: {title}".

### Schritte
1. Shim: Report-FD-Argument, Check 1 und 2 vor seccomp, Check 3 danach, Exit-Code 3 bei Fehlschlag. Unit-Tests für die Parser (`/proc/net/dev`, `/proc/net/unix`) mit eingebetteten Beispieldateien.
2. bwrap-Launcher: Pipe anlegen, FD 3 vererben (`--` Argumente, kein `CLOEXEC`), Leser-Task im Daemon.
3. `isolation.rs`: Parser, Diagnostics `SANDBOX_010..013`, Event-Emission, Start-Abbruch.
4. Flutter: Provider, Panel, Ring, Header-Einbindung, Strings.
5. Escape-Tests `esc-1-sockets.sh`, `esc-2-mounts.sh` verwenden `humanitl sandbox check --json`, das dieselben Ergebnisse ausgibt.

### Tests
- Shim-Unit: `parse_proc_net_dev_only_lo`, `parse_proc_net_unix_filters_abstract`, `socket_walk_ignores_proc_sys`.
- Sandbox-Integration (bwrap, Feature `escape`): Normalstart ⇒ drei `passed: true`; Profil mit absichtlichem Mount von `/tmp/.X11-unix` ⇒ Check 2 `passed: false`, `SANDBOX_012`, Agent wurde nicht gestartet (Marker-Datei fehlt).
- Daemon-Unit: fehlende Report-Zeilen ⇒ `SANDBOX_010`.
- Widget: Panel zeigt drei Zeilen grün nach drei Events; Fehlschlag zeigt Diagnostic-Karte; Ring-Painter-Golden für 0/3, 3/3, 1 rot.

### Akzeptanzkriterien
- [ ] `humanitl sandbox check --json` liefert drei Objekte mit `passed: true` auf dem Entwicklungsrechner.
- [ ] Mount von `/tmp/.X11-unix` im Profil führt zu `SANDBOX_012` in CLI (Exit 3) und UI (Start deaktiviert, Diagnostic sichtbar).
- [ ] Ring im Header ist bei laufender Sandbox komplett grün, bei gestoppter grau.
- [ ] Vierte Zeile zeigt den konfigurierten Endpoint amber.
- [ ] ESC-1 und ESC-2 grün in CI.

### Fallstricke
- Check 3 muss **nach** dem seccomp-Aufruf laufen, Check 2 **davor** (die `nftw`-Suche braucht keine Sockets, aber `connect()` zum Warten auf socat braucht `socket()`, und das ist nach dem Filter verboten). Reihenfolge ist Sicherheitsrelevant: Warten auf socat, dann Checks 1–2, dann seccomp, dann Check 3, dann exec.
- `/proc/net/unix` zeigt im neuen Netz-Namespace nur Sockets dieses Namespaces; filesystem-Sockets aus Bind-Mounts erscheinen dort erst nach `connect()`. Deshalb zusätzlich der Dateisystem-Walk.
- `nftw` über `/work` kann bei großen Projekten langsam sein: Tiefe 8, und `/work` wird nur bis Tiefe 3 durchsucht (Sockets im Projekt sind unüblich, aber ein `.sock` in `/work` wäre ein Fund).
- Report-FD darf nicht der Terminal-PTY sein. FD 3 wird explizit übergeben; der Shim schließt alle FDs > 3 vor `exec` außer den socat-Kindern.
- Wenn bwrap ohne User-Namespaces läuft (setuid-Variante), stimmt alles trotzdem; wenn `bwrap` fehlt, greift `SANDBOX_001` aus HUM-011.

### Referenzen
BACKLOG.md 4.1, 4.5 (ESC-1, ESC-2), Abschnitt 5 (Signature-Element Isolation Ring, Usability §4); CONVENTIONS.md 3.4, 3.11. `seccomp(2)`, `proc(5)` Abschnitt `/proc/net/unix`.

---

## HUM-042 · Terminal
Sprint: 3 · Größe: L · Abhängigkeiten: HUM-011, HUM-012, HUM-018, HUM-040 · Blockiert: HUM-067, HUM-046

### Kontext
Der Nutzer arbeitet mit dem Agenten im Terminal. Das PTY muss im Daemon leben, weil dort bwrap läuft und weil die UI im Flatpak später keinen Zugriff auf den Host hat (ADR-003). Sicherheitsreview: Terminal-Ausgabe ist ein Seitenkanal (OSC 52 schreibt ins Host-Clipboard, OSC 8 baut anklickbare Links, Titel-Sequenzen fälschen Fenster). Der Daemon filtert, nicht die UI.

### Ziel
Der Daemon öffnet für jede Session ein PTY, startet bwrap darin, und exponiert einen gRPC-Bidi-Stream `Terminal`. Die Flutter-App rendert den Stream mit `xterm2`, sendet Tastatureingaben und Resize. Der Daemon filtert die Ausgabe byteweise gegen eine Liste verbotener Escape-Sequenzen. Wenn ein Flow gehalten wird, schreibt der Daemon eine Zeile ins Terminal, damit der Nutzer es auch vom Agenten aus sieht.

### Nicht-Ziel
Kein Terminal-Multiplexing (nur ein Client pro Session). Kein lokales PTY in Flutter (`flutter_pty` wird nicht verwendet). Keine Scrollback-Persistenz über Neustart hinaus.

### Betroffene Pfade
- `daemon/crates/sandbox/src/pty.rs` (neu): PTY-Erzeugung mit `nix::pty::openpty`, Größe, Kindprozess-Anbindung
- `daemon/crates/sandbox/src/osc_filter.rs` (neu): Byte-Filter
- `daemon/crates/ipc/src/terminal.rs` (neu): Bidi-Handler
- `proto/humanitl/v1/humanitl.proto`: `TerminalInput { oneof { bytes data; Resize resize; bool detach } }`, `Resize { uint32 cols; uint32 rows }`, `TerminalOutput { oneof { bytes data; Exit exit; Notice notice } }`, `Exit { int32 code; string signal }`, `Notice { string text }`
- `app/lib/features/sandbox/widgets/terminal_pane.dart` (neu)
- `app/lib/features/sandbox/providers/terminal_provider.dart` (neu)
- `app/pubspec.yaml`: `xterm2`
- `daemon/crates/sandbox/tests/pty.rs`, `daemon/crates/sandbox/tests/osc_filter.rs`

### Spezifikation

PTY:

```rust
pub struct PtySession { master: OwnedFd, child: Pid, size: Mutex<(u16,u16)> }
impl PtySession {
    pub fn spawn(plan: &LaunchPlan, initial: (u16,u16)) -> Result<Self, Diagnostic>;   // openpty, fork, setsid, TIOCSCTTY, dup2 slave -> 0/1/2, exec bwrap argv
    pub fn resize(&self, cols: u16, rows: u16) -> nix::Result<()>;                        // TIOCSWINSZ, then SIGWINCH to child
    pub fn writer(&self) -> impl AsyncWrite;     // tokio AsyncFd over master
    pub fn reader(&self) -> impl AsyncRead;
    pub async fn wait(&self) -> ExitStatus;
}
```

Environment für das PTY-Kind: `TERM=xterm-256color`, `COLUMNS`/`LINES` nicht setzen (TIOCSWINSZ reicht). Master-FD nicht-blockierend, `tokio::io::unix::AsyncFd`.

Bidi-Handler: Genau ein aktiver Client pro Session. Ein zweiter `Terminal`-Aufruf mit derselben `session_id` erhält `Status::already_exists` mit Message `TERM_001`. Beim Verbinden sendet der Daemon zuerst die letzten 64 KiB Scrollback (Ringpuffer im Daemon), dann live. `detach` beendet den Stream ohne das PTY zu schließen. Session-Ende ⇒ `Exit { code, signal }` und Stream-Ende.

Resize-Ordnung: Client sendet `Resize` vor dem ersten `data`. Der Daemon wendet Resize an, bevor er die Scrollback-Bytes sendet. Resize-Events werden im Daemon gedrosselt: maximal eines pro 50 ms, das letzte gewinnt.

OSC-Filter (`osc_filter.rs`), zustandsbehafteter Byte-Filter, der über beliebige Chunk-Grenzen funktioniert:

```rust
pub struct OscFilter { state: State, buf: Vec<u8> }
impl OscFilter {
    pub fn new(policy: OscPolicy) -> Self;
    /// Feeds bytes, returns the bytes to forward. Never reorders. Blocks only complete forbidden sequences.
    pub fn feed(&mut self, input: &[u8]) -> Vec<u8>;
}
pub struct OscPolicy { pub deny: Vec<u16> }   // OSC numbers; default [0, 1, 2, 7, 8, 9, 52, 777, 1337]
```

Grammatik: `ESC ] <num> ; <payload> (BEL | ESC \)`. Der Filter erkennt `ESC ]`, sammelt bis zum Terminator (max. 64 KiB, danach verwerfen), prüft `<num>` gegen `deny`. Verbotene Sequenz wird komplett entfernt; erlaubte (z. B. OSC 4/10/11 Farben, OSC 133 Prompt-Marker) werden unverändert durchgereicht. Zusätzlich entfernt: `ESC c` (RIS, Terminal-Reset) und DCS-Sequenzen (`ESC P ... ESC \`), da xterm2 sie nicht braucht und sie Sixel-Uploads tragen könnten. CSI/SGR bleiben unangetastet. Der Filter ist die einzige Stelle, die Terminalbytes verändert; er läuft im Daemon vor dem Recorder-Mirror (Terminal-Ausgabe wird nicht aufgezeichnet im MVP, nur der Ringpuffer).

Notice bei gehaltenem Flow: Der Proxy sendet an den Terminal-Handler `Notice { text }`; der Handler schreibt `\r\n\x1b[2m[humanitl] request held: {method} {host}{path_truncated} · waiting for you\x1b[0m\r\n` in den Ausgabestream (nicht ins PTY, damit der Agent es nicht als Eingabe sieht). Bei Entscheidung: `[humanitl] allowed` / `blocked` / `timed out`. Konfigurierbar über `ui.terminal_notices` (Default `true`, Tier `advanced`).

Flutter `TerminalPane`:

```dart
class TerminalPane extends ConsumerStatefulWidget { ... }
// build: TerminalView(terminal, controller: ..., autoResize: true, theme: HTokens.terminalTheme, textStyle: JetBrains Mono 13)
// terminal.onOutput -> ref.read(terminalProvider(sessionId).notifier).sendInput(bytes)
// terminal.onResize -> sendResize(cols, rows)
// stream.listen: data -> terminal.write(utf8.decode(data, allowMalformed: true)); notice -> terminal.write(...) ; exit -> banner
```

Banner über dem Terminal (HRow, 24 px, `bg-2`, `fg-1`): ARB `terminal_untrusted_banner` en "Agent output is untrusted. Clipboard and link sequences are filtered." / de "Agent-Ausgabe ist nicht vertrauenswürdig. Zwischenablage- und Link-Sequenzen werden gefiltert." xterm2-Terminal-Optionen: `Terminal(maxLines: 10000)`, OSC-Handler nicht registrieren (kein `onTitleChange`-Effekt), Rechtsklick-Menü mit Copy/Paste (Copy aus Selektion ist erlaubt, das ist eine Nutzeraktion).

### Schritte
1. `pty.rs` mit `openpty`, fork/exec, Resize, AsyncFd. Test: `sh -c 'stty size; echo hi'` liefert `24 80` und `hi`.
2. `osc_filter.rs` mit Tabellen-Tests.
3. Bidi-Handler mit Ringpuffer, Ein-Client-Regel, Resize-Drossel, Notice-Kanal (`mpsc` vom Proxy).
4. Proto-Erweiterung, Codegen.
5. Flutter: Provider (öffnet Stream bei `sessionId` ≠ null), Pane, Banner, Theme aus Tokens, Fokus-Handling (Terminal bekommt Fokus beim Betreten des Screens, gibt ihn bei `Ctrl+1..5` ab).
6. Integrationstest mit echtem Daemon: Eingabe `echo $TERM\n`, Ausgabe enthält `xterm-256color`.

### Tests
- `osc52_removed_across_chunks`: Sequenz `ESC ] 52 ; c ; base64 BEL` in zwei Chunks geteilt ⇒ Ausgabe enthält sie nicht, umliegender Text unverändert.
- `osc8_removed`, `osc0_title_removed`, `osc133_passes`, `sgr_passes`, `ris_removed`, `dcs_removed`.
- `unterminated_osc_dropped_after_cap`: 70 KiB ohne Terminator ⇒ verworfen, Filter erholt sich.
- `pty_resize_reaches_child`: `resize(120, 40)` ⇒ `stty size` im Kind liefert `40 120`.
- `second_client_rejected`: zweiter Stream ⇒ `TERM_001`.
- `scrollback_replayed_on_attach`: 1000 Zeilen Ausgabe, Attach ⇒ Client erhält die letzten 64 KiB.
- Widget: Banner sichtbar; Eingabe `a` sendet `[0x61]`.

### Akzeptanzkriterien
- [ ] OpenCode-TUI ist im Flutter-Terminal bedienbar (Pfeiltasten, Enter, Ctrl+C), Farben stimmen.
- [ ] `printf '\e]52;c;SGVsbG8=\a'` in der Sandbox ändert das Host-Clipboard nicht (ESC-5 Terminalteil grün).
- [ ] Gehaltener Flow erzeugt eine `[humanitl] request held` Zeile im Terminal.
- [ ] Fenster-Resize im UI ändert die Spaltenzahl im Agenten ohne Zeilensalat (Test mit `tput cols`).
- [ ] Detach und Re-Attach zeigen den Scrollback.

### Fallstricke
- **Resize-Race:** Wenn Resize nach dem ersten Datenblock kommt, rendert der Agent einmal mit 80×24. Deshalb Resize vor Scrollback und im Client vor dem Öffnen des Streams die aktuelle Größe ermitteln.
- Der OSC-Filter darf UTF-8-Mehrbytezeichen nicht zerschneiden: er arbeitet auf Bytes und gibt nur ganze Sequenzen oder Rohbytes weiter; UTF-8-Dekodierung passiert erst in Flutter mit `allowMalformed`.
- `ESC \` (ST) besteht aus zwei Bytes, die über eine Chunk-Grenze fallen können. Der Zustandsautomat merkt sich das `ESC`.
- PTY-Master `read` liefert `EIO`, wenn das Kind beendet ist; das ist EOF, kein Fehler.
- Zombie-Vermeidung: `waitpid` im Daemon-Task, `SIGCHLD` nicht global ignorieren.
- xterm2 unter Impeller: Text-Rendering testen, bei Problemen `--no-enable-impeller` dokumentieren (BACKLOG.md 10).

### Referenzen
BACKLOG.md 4.2 (Terminal-Ausgabe), 4.5 ESC-5, Abschnitt 5; ADR-003; CONVENTIONS.md 3.6, 3.9. xterm2 (https://pub.dev/packages/xterm2), `pty(7)`, XTerm Control Sequences (OSC 52, OSC 8).

---

## HUM-043 · `/work`-Härtung
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-011, HUM-025, HUM-026, HUM-040 · Blockiert: HUM-046

### Kontext
Sicherheitsreview, Kanal 1: `/work` mit Schreibrecht ist der größte Seitenkanal. Der Agent kann Secrets in `.git/hooks`, `.envrc`, `.vscode/settings.json` oder Workflow-Dateien schreiben, die der Nutzer später ausführt oder pusht. Symlinks aus `/work` nach außen werden host-seitig aufgelöst. Der Kanal wird nicht geschlossen, sondern deklariert und beobachtet: Maskierung, Diff-Zusammenfassung am Session-Ende, Secret-Scan über den Diff, Symlink-Erkennung.

### Ziel
Das Sandbox-Profil maskiert gefährliche Pfade in `/work`. Der Daemon nimmt beim Start einen Dateibaum-Snapshot (Pfad, Größe, mtime, Blake3 für Dateien ≤ 4 MiB) und beim Session-Ende einen zweiten, berechnet den Diff, scannt neue und geänderte Textdateien mit den Findings-Detektoren, erkennt Symlinks mit Ziel außerhalb `/work`, und liefert eine `SessionSummary`, die im UI als Sheet und in der CLI als Tabelle erscheint. Host-seitige Dateizugriffe des Daemons in `/work` verwenden `openat2` mit `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`.

### Nicht-Ziel
Kein Git-Integration (kein `git diff`, keine Commits). Kein Blockieren von Schreibvorgängen zur Laufzeit (kein FUSE, kein inotify-basiertes Eingreifen). Keine Wiederherstellung.

### Betroffene Pfade
- `profiles/sandbox/default.toml`: `tmpfs`, `masked_files` (bereits definiert in CONVENTIONS.md 3.4, hier vollständig)
- `daemon/crates/sandbox/src/worktree.rs` (neu): Snapshot, Diff, Symlink-Check, Safe-Open
- `daemon/crates/sandbox/src/summary.rs` (neu): `SessionSummary`
- `daemon/crates/recorder/src/migrations/V3__session_summary.sql` (neu)
- `proto/humanitl/v1/humanitl.proto`: `SessionSummary { repeated FileChange changes; repeated Finding findings; repeated SymlinkEscape symlinks; uint64 scanned_bytes; bool truncated }`
- `app/lib/features/sandbox/widgets/session_summary_sheet.dart` (neu)
- `daemon/bin/humanitl/src/cmd/flows.rs`: `humanitl sessions summary <id>` (neues Subkommando `sessions`, Ergänzung zu CONVENTIONS.md 3.8)
- `tests/escape/esc-5-filesystem.sh`: Dateisystem-Teil

### Spezifikation

Maskierungsliste (Profil `default.toml`, vollständig):

```toml
[mounts]
tmpfs = [
  "/tmp", "/dev/shm", "/home/agent",
  "/work/.git/hooks",
  "/work/.vscode", "/work/.idea", "/work/.fleet",
  "/work/.github/workflows",
  "/work/.gitlab-ci.yml.d",
]
masked_files = [
  "/work/.envrc", "/work/.env", "/work/.env.local",
  "/work/.git/config",
  "/work/.npmrc", "/work/.yarnrc", "/work/.yarnrc.yml", "/work/.pypirc",
  "/work/.gitlab-ci.yml", "/work/Jenkinsfile", "/work/.pre-commit-config.yaml",
  "/work/.direnv",
]
```

`masked_files` werden als leere Datei (`--ro-bind /dev/null DST`) über den Originalpfad gelegt, nur wenn der Pfad existiert oder das Elternverzeichnis existiert (bwrap legt Zielpfad als Datei an). `tmpfs`-Pfade werden nur gemountet, wenn das Elternverzeichnis existiert; sonst übersprungen. Das Profil ist die einzige Quelle; der Nutzer kann global (nicht per Projekt, siehe HUM-066) `masked_files` erweitern oder mit `unmask = ["/work/.env"]` einzelne Pfade freigeben (Tier `expert`, Diagnostic `SANDBOX_020` Warning beim Start: "You unmasked {path}. The agent can read and write it.").

Snapshot:

```rust
pub struct TreeSnapshot { entries: BTreeMap<RelPath, Entry>, truncated: bool }
pub struct Entry { kind: Kind /* File|Dir|Symlink{target}|Other */, size: u64, mtime_ns: i128, hash: Option<[u8;32]> }
pub fn snapshot(root: &Path, limits: &SnapshotLimits) -> Result<TreeSnapshot, Diagnostic>;
pub struct SnapshotLimits { pub max_entries: usize /* 200_000 */, pub hash_max_bytes: u64 /* 4 MiB */, pub skip_dirs: Vec<&'static str> /* node_modules, .git/objects, target, .venv, __pycache__, .cache */ }
pub fn diff(before: &TreeSnapshot, after: &TreeSnapshot) -> Vec<FileChange>;
pub enum FileChange { Added(RelPath), Modified(RelPath), Removed(RelPath), SymlinkAdded { path: RelPath, target: PathBuf, escapes: bool }, ModeChanged(RelPath) }
```

Der Walk verwendet `openat2(dirfd, name, OpenHow { flags: O_PATH|O_NOFOLLOW|O_CLOEXEC, resolve: RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS|RESOLVE_NO_MAGICLINKS })` relativ zum Root-FD von `/work` (Host-Pfad). Symlinks werden per `readlinkat` gelesen, nie gefolgt. `escapes = true`, wenn `target` absolut ist oder nach lexikalischer Normalisierung mit `..` aus `/work` hinausführt. Kernel < 5.6 (kein `openat2`): Fallback `openat` mit `O_NOFOLLOW` pro Komponente, Diagnostic `SANDBOX_021` (Info): "Kernel without openat2; using slower path-by-path resolution."

`.git/objects` wird übersprungen (groß, opak), `.git/HEAD`, `.git/refs/**`, `.git/config` (maskiert, also unverändert), `.git/hooks` (tmpfs, also unverändert) werden erfasst.

Scan: Für `Added`/`Modified` Dateien ≤ 4 MiB, deren erste 8 KiB kein NUL-Byte enthalten (Text-Heuristik), laufen die Findings-Detektoren (HUM-025) mit `FindingLocation::File(path)` (neue Variante). Zusätzlicher Detektor `WorkflowDetector`: Pfad matcht `.github/workflows/**`, `.gitlab-ci.yml`, `Makefile`, `package.json` (nur Schlüssel `scripts.postinstall|preinstall|prepare`), `setup.py`, `pyproject.toml` (`[tool.*.scripts]`), `Cargo.toml` (`build`) ⇒ `FindingKind::Custom("executable-on-host")`, Tier `Regex`. Diese Findings werden nicht geblockt, sondern gelistet.

`SessionSummary` wird im Recorder in Tabelle `session_summaries(session_id PK, created, json BLOB)` gespeichert und per `Sandbox(Status)`-Event `SessionEnded { summary }` an die UI gestreamt. UI: Sheet von rechts, Titel „Session summary", drei Abschnitte: Changed files (Tabelle Pfad, Art, Größe), Findings (Chips nach Typ, Klick zeigt Datei und Zeile), Symlinks (rot, wenn `escapes`). Buttons: „Open folder" (`xdg-open` host-seitig auf `/work`-Hostpfad), „Copy list". CLI: `humanitl sessions summary <id> [--json]`.

Diagnostics: `SANDBOX_022` (Warning) pro Symlink mit `escapes`: "The agent created a symlink {path} pointing outside the project ({target}). Do not follow it." `fix: CopyCommand("rm '{host_path}'")`. `SANDBOX_023` (Warning) bei Findings in geänderten Dateien: "{n} potential secret(s) were written into the project during this session." `SANDBOX_024` (Info) bei `truncated`.

### Schritte
1. Profil-Liste vervollständigen, Launcher: `masked_files` als `/dev/null`-Bind, Existenzprüfung, `unmask`.
2. `worktree.rs`: Safe-Open-Helper mit `openat2` (Crate `rustix` oder `nix` ≥ 0.29), Fallback, Snapshot, Diff. Tests mit `tempfile`-Bäumen.
3. `FindingLocation::File`, `WorkflowDetector`.
4. `summary.rs`, Recorder-Migration, Event, CLI-Kommando.
5. Flutter-Sheet.
6. `esc-5-filesystem.sh` Dateisystem-Teil: Symlink `/work/x -> /home`, Datei in `.git/hooks/pre-commit`, `.envrc` schreiben ⇒ Summary listet Symlink mit `escapes`, Hook-Datei existiert auf dem Host nicht, `.envrc` auf dem Host unverändert.

### Tests
- `snapshot_skips_node_modules_and_git_objects`.
- `diff_detects_added_modified_removed`.
- `symlink_escape_absolute`, `symlink_escape_dotdot`, `symlink_inside_ok`.
- `openat2_refuses_symlink_traversal`: Baum mit `a -> /etc`; Öffnen von `a/passwd` schlägt mit `EXDEV`/`ELOOP` fehl.
- `masked_envrc_is_empty_in_sandbox`, `hooks_dir_is_tmpfs`: Integration mit bwrap.
- `findings_in_added_file`: neue Datei mit `AKIA...`-Muster ⇒ `SANDBOX_023`.
- `workflow_detector_flags_github_workflow`.

### Akzeptanzkriterien
- [ ] ESC-5 Dateisystem-Teil grün.
- [ ] Nach einer Session, in der der Agent `echo x > .env` ausführt, ist `.env` auf dem Host unverändert und die Summary zeigt keinen Eintrag für `.env` (weil maskiert), aber `SANDBOX_020` erscheint, falls `unmask` gesetzt war.
- [ ] Summary-Sheet erscheint automatisch beim Session-Ende, Findings-Chips sind klickbar.
- [ ] `humanitl sessions summary <id> --json` liefert `changes`, `findings`, `symlinks`.
- [ ] Snapshot eines Projekts mit 50 000 Dateien dauert unter 5 s (Benchmark-Test, `#[ignore]`).

### Fallstricke
- `masked_files` über `/dev/null` bedeutet: der Agent kann die Datei nicht lesen **und** Schreibversuche scheitern mit `EACCES`/`EPERM` (read-only bind). Tools, die `.env` erwarten, sehen eine leere Datei; das ist gewollt und wird in `docs/SECURITY.md` beschrieben.
- Hash-Vergleich statt nur mtime, weil Agents `touch` verwenden und manche Tools mtime erhalten.
- `.git/index` ändert sich bei jedem `git status` des Agenten; als `Modified` listen, aber im UI unter „Git-Metadaten" zusammenfassen, nicht als Finding.
- Kein Scan von Binärdateien; NUL-Heuristik dokumentieren.
- Der Snapshot läuft host-seitig als Daemon-User über den Host-Pfad, nicht in der Sandbox. Deshalb `RESOLVE_BENEATH` zwingend: ein während der Session angelegter Symlink darf den Snapshot nicht aus `/work` hinausführen.

### Referenzen
BACKLOG.md 4.2 (Kanal `/work`), 4.5 ESC-5, ADR-002; CONVENTIONS.md 3.4. `openat2(2)`, bwrap `--ro-bind`, Blake3 (Crate `blake3`).

---

## HUM-044 · Setup-Flow
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-019, HUM-039, HUM-041, HUM-062, HUM-063, HUM-066 · Blockiert: HUM-046

### Kontext
Usability-Review §1: Der erste Start darf nicht in eine leere Queue führen, sondern in eine Checkliste mit vier Punkten. Die drei Grundentscheidungen (LLM, Projekt, Start) sind die `basic`-Stufe aus ADR-011. Fehlender Daemon ist kein Modal, sondern der Setup-Screen mit einem Ein-Zeilen-Befehl und Live-Indikator. Der erste gehaltene Request bekommt einen Coach-Mark.

### Ziel
`SetupScreen` unter `features/setup` zeigt vier Checks: Daemon, LLM, Projekt, Sandbox. Jeder Check hat Status-Punkt, eine Aktion und bei Fehlschlag eine Diagnostic-Karte. Alle vier grün ⇒ Button „Start agent" aktiv ⇒ Navigation zum Intercept-Screen mit Sandbox-Screen als zweitem Tab. Die App startet in den Setup-Screen, wenn einer der vier Checks nicht grün ist, sonst direkt in Intercept. Ein Coach-Mark erscheint genau einmal am ersten gehaltenen Request.

### Nicht-Ziel
Kein Onboarding-Video, keine Tour durch alle Screens. Keine Installation von OpenCode oder bwrap durch die App selbst (nur Befehle zum Kopieren). Kein Settings-Screen (HUM-069).

### Betroffene Pfade
- `app/lib/features/setup/setup_screen.dart` (neu)
- `app/lib/features/setup/providers/setup_provider.dart` (neu)
- `app/lib/features/setup/widgets/{setup_check_row.dart, daemon_check.dart, llm_check.dart, project_check.dart, sandbox_check.dart, coach_mark.dart}` (neu)
- `app/lib/app.dart`: Start-Routing
- `daemon/bin/humanitl/src/cmd/daemon.rs`: `daemon install|status`
- `packaging/systemd/humanitld.service`, `humanitld.socket` (neu)
- `daemon/crates/sandbox/src/preflight.rs` (neu): Host-Voraussetzungen (bwrap vorhanden, Version, user namespaces)
- ARB: `setup_*`

### Spezifikation

Zustandsmodell:

```dart
enum CheckState { unknown, checking, ok, failed }
@freezed class SetupCheck { CheckKind kind; CheckState state; Diagnostic? diagnostic; String? detail; }
enum CheckKind { daemon, llm, project, sandbox }
```

Ablauf (Schritt-Diagramm):

```
App start
  └─ daemon: connect UDS ──ok──> GetInfo ──version ok──> [daemon ok]
        │ fail                      │ major mismatch
        ▼                           ▼
     DAEMON_001                  DAEMON_002
  └─ llm:  config.llm.endpoint set? ──no──> LLM_000 (Info: "Not configured") [action: field + Test (HUM-039)]
        └─ yes ──> ProbeLlm ──ok──> [llm ok, models chip]   / fail ──> LLM_001..003
  └─ project: config.sandbox.work_dir set & exists & readable? ──no──> PROJECT_001 [action: picker]
        └─ .humanitl/profile.toml present? ──> resolve() ──> CONFIG_003/004 if any
  └─ sandbox: SandboxPreflight RPC ──> bwrap found (SANDBOX_001), version ≥ 0.8 (SANDBOX_002),
        user namespaces enabled (SANDBOX_003: /proc/sys/kernel/unprivileged_userns_clone or apparmor restriction),
        seccomp available (SANDBOX_004), $XDG_RUNTIME_DIR writable (SANDBOX_005), agent preflight (AGENT_001..002 from HUM-037)
All ok ──> "Start agent" enabled ──> Sandbox(Start) ──> Isolation checks (HUM-041) ──> Intercept screen
```

Diagnostics dieses Issues:

| Code | Severity | why (en) | fix |
|---|---|---|---|
| `DAEMON_001` | Blocking | "Humanitl's background service is not running. The app cannot see any traffic without it." | `InstallService` (führt `humanitl daemon install` aus, das die Unit nach `~/.config/systemd/user/` schreibt und `systemctl --user enable --now humanitld.socket` aufruft) plus `CopyCommand("systemctl --user start humanitld")` |
| `DAEMON_002` | Blocking | "The service speaks protocol v{x}, this app expects v{y}. Update both from the same release." | `OpenUrl(releases)` |
| `PROJECT_001` | Blocking | "No project folder chosen. The agent needs exactly one folder to work in." | `ChangeSetting { key: "sandbox.work_dir" }` |
| `PROJECT_002` | Blocking | "The folder {path} is not readable by your user." | `CopyCommand("ls -ld '{path}'")` |
| `SANDBOX_001` | Blocking | "bubblewrap (bwrap) is not installed. It is the sandbox Humanitl runs the agent in." | `CopyCommand("sudo apt install bubblewrap")` (Distribution erkannt über `/etc/os-release`: apt / dnf / pacman / zypper) |
| `SANDBOX_002` | Blocking | "bwrap {found} is too old; 0.8.0 or newer is required for --file and seccomp." | wie oben |
| `SANDBOX_003` | Blocking | "Unprivileged user namespaces are disabled on this system. Rootless sandboxes need them." | `CopyCommand("sudo sysctl -w kernel.unprivileged_userns_clone=1")` bzw. AppArmor-Hinweis auf Ubuntu ≥ 23.10: `CopyCommand("sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0")` mit `docs` |
| `SANDBOX_004` | Blocking | "The kernel has no seccomp filter support." | `docs` |
| `SANDBOX_005` | Blocking | "$XDG_RUNTIME_DIR is not set or not writable; Humanitl keeps its sockets there." | `CopyCommand("loginctl enable-linger $USER")` |

`daemon install` schreibt:

```ini
# ~/.config/systemd/user/humanitld.service
[Unit]
Description=Humanitl moderation daemon
[Service]
ExecStart=%h/.local/bin/humanitld
Restart=on-failure
NoNewPrivileges=yes
ProtectHome=read-only
ReadWritePaths=%h/.local/share/humanitl %h/.config/humanitl %t/humanitl
PrivateTmp=yes
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
SystemCallFilter=@system-service
[Install]
WantedBy=default.target
```

```ini
# ~/.config/systemd/user/humanitld.socket
[Socket]
ListenStream=%t/humanitl/daemon.sock
SocketMode=0600
DirectoryMode=0700
[Install]
WantedBy=sockets.target
```

Der Daemon unterstützt Socket-Aktivierung (`LISTEN_FDS`, Crate `listenfd`). Der Binary-Pfad in `ExecStart` wird beim Install aus `std::env::current_exe()` der CLI abgeleitet (gleiches Verzeichnis, Name `humanitld`).

Screen-Layout: zentrierte Spalte, max. 640 px, Titel „Set up Humanitl", vier `SetupCheckRow` (Punkt, Titel, Detail/Diagnostic, Aktion rechts), darunter `HButton.primary("Start agent")`. Zeilen: Daemon („Background service"), LLM („Your model server" mit `LlmEndpointField` aus HUM-039), Project („Project folder" mit `WorkDirPicker` aus HUM-040 und Profil-Auswahl als Dropdown der Profile aus HUM-066), Sandbox („Sandbox check" mit Preflight-Ergebnis). Der Daemon-Check pollt alle 2 s, solange er rot ist (Live-Indikator).

Coach-Mark: Beim ersten `Held`-Flow einer Installation (Flag `ui.coach_marks_seen: ["first_hold"]` in `config.toml`) erscheint über der Aktionsleiste ein Popover mit `setup_coach_first_hold` en "Held because no rule matches (default: ask). Allow sends it unchanged; the response is recorded, not held. Use the arrow to remember a rule." de "Angehalten, weil keine Regel passt (Standard: ask). Senden schickt die Anfrage unverändert; die Antwort wird aufgezeichnet, nicht angehalten. Über den Pfeil kannst du eine Regel merken." Schließen per Klick oder `Esc`, danach nie wieder.

### Schritte
1. `preflight.rs` mit den fünf Sandbox-Prüfungen, RPC `SandboxPreflight` (Ergänzung in `Sandbox`-oneof), Diagnostics.
2. `daemon install|status` in der CLI, Unit-Dateien, `listenfd` im Daemon.
3. `setupProvider`: vier Checks parallel, Daemon-Poll.
4. Screen und Widgets, Routing in `app.dart`.
5. Coach-Mark mit Config-Flag.
6. Widget-Tests, Golden des Setup-Screens (alle rot, alle grün).

### Tests
- `setup_shows_daemon_001_with_install_action` (FakeDaemonClient wirft Connect-Fehler).
- `start_button_enabled_only_when_all_ok`.
- `llm_row_uses_probe` (Provider-Aufruf).
- `coach_mark_shown_once`: erster Held ⇒ Popover; Flag gesetzt; zweiter Held ⇒ kein Popover.
- Daemon-Unit: `preflight_detects_missing_bwrap` (PATH leer), `preflight_parses_bwrap_version`.
- CLI: `daemon install` schreibt beide Units in ein temporäres `XDG_CONFIG_HOME`, Inhalt entspricht der Vorlage.

### Akzeptanzkriterien
- [ ] Frische Installation ohne laufenden Daemon: App zeigt Setup mit `DAEMON_001`, Klick auf Fix installiert und startet die Unit, Zeile wird binnen 4 s grün.
- [ ] Ohne bwrap: `SANDBOX_001` mit distributionsspezifischem Befehl.
- [ ] Alle grün ⇒ Start ⇒ Intercept-Screen, Ring im Header grün.
- [ ] Coach-Mark erscheint genau einmal.
- [ ] Goldens abgelegt.

### Fallstricke
- `systemctl --user` funktioniert nicht in Umgebungen ohne User-Session-Bus (SSH ohne Linger). `SANDBOX_005`/`DAEMON_001` decken das ab; Fix-Text nennt `loginctl enable-linger`.
- Ubuntu ≥ 23.10 blockiert unprivilegierte User-Namespaces per AppArmor; bwrap aus dem Paket hat ein AppArmor-Profil, das es erlaubt. Preflight prüft erst, ob `bwrap --unshare-user true` funktioniert, bevor es sysctl-Hinweise gibt.
- Der Daemon darf nie mit `sudo` installiert werden; die Fix-Befehle enthalten kein `sudo` für Humanitl selbst, nur für Paketinstallation.
- Setup-Screen darf gehaltene Flows nicht verstecken: Wenn die App wegen eines nicht-blockierenden Checks im Setup ist, aber Flows gehalten werden, zeigt der Header-Badge den Zähler und `Ctrl+1` wechselt trotzdem.

### Referenzen
BACKLOG.md Abschnitt 5 (Usability §1, §6), 4.4 (systemd-Härtung), ADR-010, ADR-012; CONVENTIONS.md 3.4, 3.8. `systemd.exec(5)`, `systemd.socket(5)`, Flathub-Diskussion zu bwrap (https://discourse.flathub.org/t/help-with-running-bubblewrap-in-a-flatpak/3572).

---

## HUM-045 · TLS-Fehler-Erkennung
Sprint: 3 · Größe: S · Abhängigkeiten: HUM-014, HUM-015, HUM-063 · Blockiert: HUM-068

### Kontext
Nicht jedes Tool in der Sandbox vertraut der Humanitl-CA (Java ignoriert PEM-Env, alte Bun-Versionen, Go mit eigenem Pool). Das Symptom ist ein TLS-Fehler im Agenten-Terminal, der Nutzer starrt allein darauf. Usability §6: der Proxy sieht den Handshake-Abbruch und muss ihn als Karte mit Fix zeigen.

### Ziel
Der Proxy erkennt Client-seitige TLS-Abbrüche nach dem CONNECT (Alert `unknown_ca`/`bad_certificate` oder Verbindungsabbruch direkt nach dem `ServerHello`), ordnet sie einem Flow zu, und liefert ein `Diagnostic` `TLS_001..003` mit `FixAction::SetEnv`, dessen Vorschlag vom `User-Agent`-Hinweis abhängt, falls vorhanden.

### Nicht-Ziel
Keine automatische Injektion weiterer Env-Variablen zur Laufzeit. Keine Unterstützung von Certificate Pinning.

### Betroffene Pfade
- `daemon/crates/proxy/src/tls_observe.rs` (neu)
- `daemon/crates/proxy/src/flow.rs`: Verknüpfung CONNECT-Flow ↔ Handshake
- `app/lib/features/intercept/widgets/diagnostic_card.dart` (Nutzung, bereits aus HUM-063/HUM-020)

### Spezifikation

Erkennung: Für jeden CONNECT-Flow existiert nach `200 Connection established` die MITM-Phase. hudsucker übergibt den Handshake an rustls; Fehler kommen als `rustls::Error::AlertReceived(AlertDescription)` oder als `io::Error` (`UnexpectedEof`, `ConnectionReset`) vor Abschluss des Handshakes. `tls_observe.rs`:

```rust
pub enum TlsFailure { AlertUnknownCa, AlertBadCertificate, AlertOther(String), EofBeforeFinished, ResetBeforeFinished }
pub fn classify(err: &dyn std::error::Error) -> Option<TlsFailure>;
pub fn diagnostic_for(failure: &TlsFailure, host: &HostName, hint: ToolHint) -> Diagnostic;
pub enum ToolHint { Curl, Node, Bun, Python, Go, Java, Git, Cargo, Unknown }
pub fn tool_hint(connect_headers: &HeaderMap) -> ToolHint;   // from User-Agent of the CONNECT request, if present
```

Diagnostics:

| Code | Severity | Auslöser | why (en) | fix |
|---|---|---|---|---|
| `TLS_001` | Warning | `AlertUnknownCa` oder `AlertBadCertificate` | "{tool} inside the sandbox rejected Humanitl's certificate for {host}. The request never left the sandbox. Most tools read a CA file from an environment variable; this one did not." | `SetEnv` je Hint: Curl ⇒ `CURL_CA_BUNDLE=/etc/humanitl/ca.crt`; Node ⇒ `NODE_EXTRA_CA_CERTS=/etc/humanitl/ca.crt`; Bun ⇒ dasselbe plus Hinweis „Bun ≥ 1.1.22"; Python ⇒ `SSL_CERT_FILE` und `REQUESTS_CA_BUNDLE`; Go ⇒ `SSL_CERT_FILE`; Java ⇒ `JAVA_TOOL_OPTIONS=-Djavax.net.ssl.trustStore=/etc/humanitl/cacerts.jks` (Datei aus HUM-014); Git ⇒ `GIT_SSL_CAINFO`; Cargo ⇒ `CARGO_HTTP_CAINFO`; Unknown ⇒ `SSL_CERT_FILE` |
| `TLS_002` | Warning | `EofBeforeFinished`/`ResetBeforeFinished` dreimal in 10 s für denselben Host | "A client in the sandbox keeps dropping the TLS handshake to {host}. This usually means certificate pinning or a tool that ignores CA variables." | `docs`, `AddRule` (Vorschlag: `block` für den Host, damit der Agent schnell scheitert statt zu hängen) |
| `TLS_003` | Info | Handshake mit `ClientHello` ohne SNI | "A client connected without SNI; Humanitl cannot issue a certificate without a hostname and closed the connection." | keine |

Die `SetEnv`-Fix-Aktion wird in der UI als „Für nächste Session setzen" gerendert: schreibt in das Profil (global) unter `[sandbox.env]`. Zusätzlich „Befehl kopieren" mit `export KEY=VALUE` für die laufende Session (der Nutzer kann es im Terminal eingeben). Der Flow selbst erhält `Decided(Block { reason: BlockReason::NoRoute })`? Nein: der Flow bleibt beim CONNECT-Status; er wird als `Responded { status: 0 }` mit `error = "tls_handshake_failed"` aufgezeichnet, damit die History ihn zeigt (Zustandsfarbe `error`).

### Schritte
1. `classify`, `tool_hint`, `diagnostic_for` mit Tabelle.
2. hudsucker-Hook: Fehlerpfad des MITM-Handshakes abfangen (Handler `handle_error` oder Wrapper um den `TlsAcceptor`), Flow-ID aus dem CONNECT-Kontext.
3. Recorder: Flow-Abschluss mit `error`-Feld (Migration `V4__flow_error.sql`: `ALTER TABLE flows ADD COLUMN error TEXT`).
4. Zähler für `TLS_002` (pro Host, gleitendes Fenster 10 s).
5. UI: Diagnostic-Karte im Feed (kommt über `diagnosticsProvider`), Fix-Buttons.

### Tests
- `classify_unknown_ca`, `classify_eof`.
- `tool_hint_from_user_agent`: `curl/8.5` ⇒ Curl, `node` ⇒ Node, `Bun/1.1` ⇒ Bun, `python-requests` ⇒ Python, `Go-http-client` ⇒ Go, `Java/21` ⇒ Java, `git/2.4` ⇒ Git, leer ⇒ Unknown.
- Integration: Client mit leerem Trust-Store (rustls-Client ohne Root) macht CONNECT + Handshake ⇒ genau ein `TLS_001`, History zeigt Flow mit `error`.
- `tls_002_after_three_resets`.

### Akzeptanzkriterien
- [ ] `curl --cacert /dev/null https://example.com` in der Sandbox erzeugt `TLS_001` mit `CURL_CA_BUNDLE`-Fix im UI und in `humanitl flows list --json` (Feld `error`). Die Daemon-Hälfte steht samt Integrationstest; die Karte im UI fehlt ganz.
- [ ] Fix „Für nächste Session setzen" schreibt `[sandbox.env]` ins globale Profil, sichtbar in `humanitl config get sandbox.env`. Lesen geht: `humanitl config get sandbox.env` antwortet `{}`, lokal aufgelöst ohne Daemon (`cmd/config.rs:115-119`). Geschrieben wird nichts — es gibt weder den Knopf noch einen Schreibweg.

### Stand (2026-09-04): nur die Daemon-Hälfte

Erkennung, Zuordnung und Aufzeichnung sind fertig:

- `daemon/crates/proxy/src/tls_observe.rs` mit `TlsFailure`, `ToolHint`,
  `classify`, `tool_hint` und `diagnostic_for` samt der Tabelle der Hinweise;
  `HandshakeWatch` zählt Abbrüche im 10-Sekunden-Fenster für `TLS_002`,
  entstört `TLS_001` je Host und Hinweis für 60 Sekunden und führt den
  Zähler der unterdrückten Versuche in der nächsten Karte mit.
- `handler.rs` hängt den gescheiterten Handschlag an den Flow des `CONNECT`,
  der Recorder schreibt `error = tls_handshake_failed` (Migration
  `V4__flow_error.sql`), und `humanitl flows list --json` zeigt das Feld
  `error`.
- Tests grün: `classify_unknown_ca`, `classify_eof`,
  `tool_hint_from_user_agent`, `tls_002_after_three_resets` und die sechs
  Integrationstests in `daemon/crates/proxy/tests/tls_observe.rs`, darunter
  `a_client_without_a_trust_store_gets_one_tls_001_and_a_flow_with_an_error`.

**Offen und ausdrücklich nicht gedeckt:**

- **Die Oberfläche.** Es gibt keinen `diagnosticsProvider` und keine Karte im
  Feed. `FlowEvent.diagnostic` kommt in der App an und wird verworfen
  (`app/lib/features/intercept/providers/flows.dart`,
  `app/lib/features/history/providers/history_page.dart`). Bis dahin sieht ein
  Mensch den TLS-Fehler nur in `humanitl flows list --json` und im Protokoll
  des Agenten. Der Platz dafür ist
  `app/lib/features/intercept/widgets/diagnostic_card.dart`.
- **Der Knopf „Für nächste Session setzen".** Er bräuchte einen
  Schreibweg in die Konfiguration, und den gibt es nicht: der RPC `SetConfig`
  antwortet `unimplemented` und wartet auf den Einstellungen-Bildschirm
  (HUM-069), `humanitl config set` gibt es noch nicht (`CLI_004:
  unrecognized subcommand 'set'`, gemessen). Das betrifft nur das Schreiben:
  `humanitl config get` löst lokal auf und braucht den Daemon nicht, nur der
  RPC `GetConfig` ist ebenfalls unimplementiert.
- Der Befund selbst trägt den Vorschlag bereits — `FixAction::SetEnv` je
  Hinweis, dazu den Kopierbefehl für die laufende Sitzung. Es fehlt die
  Hand, die ihn ausführt.

**Abweichung mit Folge: „globales Profil" heißt seit HUM-066 etwas anderes
als `config.toml`.** `docs/profiles.md:91-99` macht `config.toml` zur Ebene 2
und die Profile zu den Ebenen 3 und 4; ein Profil setzt Konfigurationswerte im
Block `[config.sandbox]`, also auch `env`. Ein globales Profil, das
`sandbox.env` mitbringt, überstimmt damit genau die Stelle, in die der Fix
schreiben würde. Nach `config.toml` zu schreiben bleibt trotzdem richtig, weil
`profiles/default.toml` mit Absicht leer ist und der Wert so über jedes
gewählte Profil hinweg gilt — aber der Knopf verspricht dann mehr, als er
halten kann.

Wer ihn baut, schuldet deshalb zweierlei: den `why` des Befunds um den Satz
zu ergänzen, dass ein globales Profil mit eigenem `sandbox.env` den Fix
überstimmt (heute nennt `tls_observe.rs:437-446` nur den Fall, dass das
Sandbox-Profil die Variable schon in seinem `[env]` trägt), und die Beschriftung
so zu wählen, dass sie kein Profil verspricht, das sie nicht schreibt.

### Fallstricke
- Nicht jeder Client sendet einen Alert; viele schließen einfach. Deshalb `EofBeforeFinished` als eigener Fall mit Schwellwert, sonst False Positives bei normalen Abbrüchen.
- `User-Agent` im CONNECT-Request ist optional; curl sendet ihn, Node nicht immer. Hint `Unknown` ist der häufige Fall.
- `TLS_001` darf nicht pro Retry spammen: dedupe pro (Host, Hint) für 60 s, Zähler in der Karte.

### Referenzen
BACKLOG.md Abschnitt 4 (TLS), Abschnitt 5 (Usability §6); CONVENTIONS.md 3.2 (Diagnostic). rustls `AlertDescription`, HTTP-Toolkit-Liste der CA-Env-Variablen (https://httptoolkit.com/blog/announcing-terminal-interception/).

---

## HUM-068 · Geführte Diagnostics im Sandbox-Screen
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-040, HUM-041, HUM-044, HUM-045, HUM-063 · Blockiert: HUM-046

### Kontext
ADR-012: Jeder nicht-grüne Zustand trägt Grund und Fix. Dieses Issue schließt die Lücken im Sandbox-Bereich, definiert den Katalog aller Codes dieses Sprints an einer Stelle und stellt sicher, dass jede Diagnostic im UI am Ort des Problems erscheint und in der CLI als Block.

### Ziel
`docs/diagnostics.md` listet alle Codes mit Auslöser, `why`, `fix`. Der Daemon hat einen Registry-Test, der prüft, dass jeder im Code verwendete Code in der Doku steht und `why` nicht leer ist. Der Sandbox-Screen rendert Diagnostics kontextbezogen: Start-Fehler im Header, Mount-Fehler im Mounts-Tab, Isolation-Fehler im Isolation-Tab, TLS-Fehler als Feed-Karte. Die CLI rendert denselben Block überall gleich.

### Nicht-Ziel
Keine neuen Prüfungen. Keine Diagnostics für Regeln oder Editor (Sprint 4).

### Betroffene Pfade
- `docs/diagnostics.md` (neu)
- `daemon/crates/core-types/src/diagnostic.rs`: `DiagnosticCode::ALL` (const-Liste), `Diagnostic::for_flow`, `Diagnostic::for_session`, `scope: DiagnosticScope`
- `daemon/crates/core-types/tests/diagnostic_registry.rs` (neu)
- `daemon/bin/humanitl/src/render.rs` (neu): CLI-Renderer
- `app/lib/core/ui/diagnostic_card.dart`: Placement-Varianten (inline row, card, banner)
- `app/lib/features/sandbox/**`: Einbindung

### Spezifikation

`DiagnosticScope`: `Global`, `Session(SessionId)`, `Flow(FlowId)`, `Check(IsolationCheck)`, `Setting(String)`. Die UI wählt den Ort aus dem Scope: `Global` ⇒ Banner oben im aktuellen Screen; `Session` ⇒ Sandbox-Header; `Flow` ⇒ Feed-Karte und Flow-Detail; `Check` ⇒ Isolation-Zeile; `Setting` ⇒ neben dem Feld (Setup/Settings).

Katalog Sprint 3 (vollständige Liste, Details in den jeweiligen Issues):

| Code | Scope | Severity | Auslöser | fix |
|---|---|---|---|---|
| `DAEMON_001` | Global | Blocking | Daemon nicht erreichbar | InstallService, CopyCommand |
| `DAEMON_002` | Global | Blocking | Proto-Major-Mismatch | OpenUrl |
| `PROJECT_001` | Setting(sandbox.work_dir) | Blocking | kein Ordner | ChangeSetting |
| `PROJECT_002` | Setting(sandbox.work_dir) | Blocking | nicht lesbar | CopyCommand |
| `CONFIG_001` | Setting(profile) | Blocking | Profil unbekannt | CopyCommand |
| `CONFIG_002` | Setting(profile) | Blocking | TOML-Fehler | OpenUrl(file) |
| `CONFIG_003` | Session | Blocking | Projekt-Profil mountet Host-Pfade | ChangeSetting |
| `CONFIG_004` | Session | Warning | Projekt-Profil fremder Besitzer | keine |
| `SANDBOX_001..005` | Global | Blocking | bwrap fehlt / alt / userns / seccomp / XDG_RUNTIME_DIR | CopyCommand |
| `SANDBOX_010` | Session | Blocking | kein Shim-Report | CopyCommand("humanitl daemon logs") |
| `SANDBOX_011..013` | Check(*) | Blocking | Check 1/2/3 fehlgeschlagen | siehe HUM-041 |
| `SANDBOX_020` | Session | Warning | `unmask` aktiv | ChangeSetting |
| `SANDBOX_021` | Session | Info | kein openat2 | keine |
| `SANDBOX_022` | Session | Warning | Symlink nach außen | CopyCommand |
| `SANDBOX_023` | Session | Warning | Findings in geänderten Dateien | keine |
| `SANDBOX_024` | Session | Info | Snapshot gekürzt | keine |
| `SANDBOX_030` | Session | Blocking | Agent-Prozess sofort beendet (Exit < 2 s) | CopyCommand("humanitl sessions logs <id>") |
| `SANDBOX_031` | Session | Warning | Cache-Volume nicht anlegbar | ChangeSetting(sandbox.mounts.cache=false) |
| `AGENT_001` | Global | Blocking | opencode fehlt | CopyCommand |
| `AGENT_002` | Setting(agent.command) | Warning | Override nicht ausführbar | ChangeSetting |
| `LLM_000` | Setting(llm.endpoint) | Info | nicht konfiguriert | ChangeSetting |
| `LLM_001..006` | Setting(llm.endpoint) / Flow | siehe HUM-039 | | |
| `TLS_001..003` | Flow | Warning/Info | siehe HUM-045 | SetEnv, AddRule |
| `TERM_001` | Session | Error | zweiter Terminal-Client | keine |

CLI-Renderer (`render.rs`), Format für jede Diagnostic:

```
✖ SANDBOX_012  Unexpected socket inside the sandbox
  why:  A mount in your profile exposes a host socket: /tmp/.X11-unix/X0
  fix:  Remove the mount from sandbox.mounts.extra_ro in ~/.config/humanitl/profiles/default.toml
  docs: https://humanitl.dev/docs/diagnostics#SANDBOX_012
```

Symbol nach Severity: `ℹ` Info, `⚠` Warning, `✖` Error, `⛔` Blocking. Bei `--json` das Diagnostic-Objekt. Farben nur bei TTY.

UI-Renderer `DiagnosticCard` mit drei Varianten: `inline` (eine Zeile mit Icon, Titel, Fix-Button, aufklappbar für `why`), `card` (Titel, `why`, Fix-Buttons, Docs-Link), `banner` (volle Breite, `bg-2`, Icon, Titel, Fix rechts). Farben: Info `fg-1`, Warning `held`-Amber, Error `error`-Orange, Blocking `blocked`-Rot. Fix-Button-Label je `FixAction`: SetEnv „Für nächste Session setzen", AddRule „Regel anlegen", InstallService „Dienst installieren", ChangeSetting „Einstellung öffnen", CopyCommand „Befehl kopieren", OpenUrl „Mehr erfahren", RemountReadOnly „Nur lesend einbinden".

Registry-Test: `grep`-freier Ansatz. `DiagnosticCode::ALL: &[(&str, &str /* doc anchor */)]` wird per Test gegen `docs/diagnostics.md` geprüft (jeder Code hat eine Überschrift `### CODE`), und ein zweiter Test scannt mit `syn` alle `DiagnosticCode("…")`-Literale im Workspace (Build-Skript oder `cargo test` mit `walkdir` über `daemon/`) und prüft Mitgliedschaft in `ALL`.

### Schritte
1. `DiagnosticScope`, `ALL`, Builder-Methoden.
2. `docs/diagnostics.md` mit allen Codes dieses Sprints (Tabelle oben plus Absätze aus den Issues).
3. Registry-Tests.
4. CLI-Renderer, in allen CLI-Fehlerpfaden verwenden.
5. `DiagnosticCard`-Varianten, Placement im Sandbox-Screen, Snapshot-Tests pro Code (Golden pro Variante, nicht pro Code).
6. `SANDBOX_030`, `SANDBOX_031` implementieren (Agent-Exit-Watchdog, Cache-Volume-Anlage).

### Tests
- `all_codes_documented`, `all_used_codes_registered`.
- `render_cli_format` (Snapshot mit `insta`).
- Widget: `diagnostic_card_inline_expands_why`, `fix_button_label_per_action`, Golden für `card` in vier Severities.
- `sandbox_030_on_immediate_exit`: Agent-Kommando `false` ⇒ Diagnostic innerhalb 3 s.

### Akzeptanzkriterien
- [ ] `docs/diagnostics.md` enthält jeden Code aus der Tabelle mit Auslöser, why, fix.
- [ ] `cargo test -p humanitl-core diagnostic_registry` grün; ein neuer, undokumentierter Code lässt den Test rot werden.
- [ ] `humanitl sandbox check` auf einem System ohne bwrap zeigt den Block im Format oben, Exit 3.
- [ ] Im Sandbox-Screen erscheinen Diagnostics am jeweils definierten Ort, nie als Modal.

### Fallstricke
- `why` muss die konkreten Werte enthalten (Pfad, Host, Version), nicht nur Platzhalter. Diagnostics werden im Daemon mit Werten formatiert; die UI übersetzt nur Titel und Fix-Label über den Code, `why` kommt zweisprachig vom Daemon (`why_en`, `why_de`, Daemon kennt `ui.language`).
- Keine Diagnostic ohne Code. Keine zwei Codes für denselben Auslöser.
- CLI-Farben nur bei `isatty(stdout)`, sonst nackter Text.

### Referenzen
BACKLOG.md ADR-012, Abschnitt 1.3 Prinzip 7; CONVENTIONS.md 3.2, 3.12.

---

## HUM-067 · `humanitl run`
Sprint: 3 · Größe: L · Abhängigkeiten: HUM-064, HUM-066, HUM-037, HUM-039, HUM-042, HUM-018 · Blockiert: HUM-046

### Kontext
ADR-013: Die CLI ist erstklassig. `humanitl run` im Projektverzeichnis startet den Agenten isoliert, ohne dass das UI läuft. `--profile llm-only` liefert die reine Inferenz-Instanz: nur der LLM-Server ist erreichbar, alles andere wird ohne Nachfrage geblockt. Mit `--ask terminal` moderiert der Nutzer im selben Terminal (Muster pipelock). Ein später gestartetes UI hängt sich an dieselbe Session.

### Ziel
`humanitl run` verbindet sich mit dem Daemon (startet ihn per Socket-Aktivierung), löst das Profil auf, startet eine Session mit `work_dir = cwd`, reicht das PTY des Nutzers durch (Raw-Mode, Resize-Weiterleitung), zeigt gehaltene Requests je nach `--ask` als Terminal-Prompt oder blockt sie, und beendet sich mit dem Exit-Code des Agenten. `Ctrl+C` geht an den Agenten; `Ctrl+]` (wie telnet) öffnet ein kleines Humanitl-Menü (Detach, Stop, Status).

### Nicht-Ziel
Kein eigener Proxy in der CLI (alles läuft im Daemon). Kein Daemon-loser Modus. Kein Multi-Session-Management (nur eine Session pro cwd gleichzeitig).

### Betroffene Pfade
- `daemon/bin/humanitl/src/cmd/run.rs` (neu)
- `daemon/bin/humanitl/src/tty.rs` (neu): Raw-Mode, Resize, Restore
- `daemon/bin/humanitl/src/ask_terminal.rs` (neu): Prompt-Renderer
- `daemon/crates/ipc/src/service.rs`: `Sandbox(Start)` mit `ask_mode`, `Subscribe` mit Filter `held_only`
- `daemon/crates/proxy/src/hold.rs`: `ask_mode == none` Pfad
- `docs/cli.md` (neu)

### Spezifikation

Syntax (CONVENTIONS.md 3.8):

```
humanitl run [--profile NAME] [--work DIR] [--ask ui|terminal|none] [--llm URL] [--ro] [--detach] [-- CMD...]
```

Semantik:
- `--work` Default cwd. Muss existieren und lesbar sein (`PROJECT_002`).
- `--ask` Default aus Profil (`hold.ask_mode`). `ui`: Requests bleiben in der Queue, die CLI zeigt nur `[humanitl] request held: …` als Zeile, das UI entscheidet (oder Timeout). `terminal`: Prompt im Terminal (unten). `none`: Default-Verdict ⇒ sofort Block, Zeile `[humanitl] blocked (no rule): GET example.com/…`.
- `--llm` überschreibt `llm.endpoint` (Origin `Cli`).
- `--ro` setzt `sandbox.work_mode = ro`.
- `-- CMD...` überschreibt `agent.command` für diese Session (z. B. `-- bash`, um in der Sandbox zu arbeiten).
- `--detach` startet die Session und beendet die CLI sofort; Ausgabe: `session <id> started; attach with: humanitl attach <id>` (`attach` ist ein neues Subkommando, gleiche Implementierung wie `run` ohne Start).

Ablauf:

1. Daemon verbinden (UDS). Fehlschlag ⇒ versuchen, `systemctl --user start humanitld.socket` auszuführen (nur wenn Unit existiert), 2 s warten, erneut. Dann `DAEMON_001`, Exit 2.
2. `GetInfo`, Versionscheck (`DAEMON_002`, Exit 1).
3. Profil auflösen (`resolve()` in der CLI, für Fehlermeldungen; der Daemon löst erneut auf, die CLI schickt nur `ProfileSelection` + CLI-Overrides).
4. Terminalgröße lesen (`ioctl TIOCGWINSZ`), Raw-Mode setzen (`termios`: `cfmakeraw`, `ISIG` bleibt aus, damit Ctrl+C als Byte an den Agenten geht), Restore bei jedem Exit-Pfad (inklusive Panic-Hook und `SIGTERM`).
5. `Sandbox(Start { profile, work_dir, work_mode, ask_mode, cli_overrides })`. Events streamen: `IsolationResult` × 3 werden als drei Zeilen gedruckt (`✔ no network interface`, …), Fehlschlag ⇒ Diagnostic-Block, Exit 3.
6. `Terminal`-Bidi-Stream öffnen, zuerst `Resize`. stdin ⇒ `data`, `data` ⇒ stdout. `SIGWINCH` ⇒ `Resize`.
7. Parallel `Subscribe { held_only: true }` für `--ask terminal|ui`.
8. Bei `Exit { code }`: Terminal restore, Summary-Kurzform drucken (HUM-043: „3 files changed, 1 finding, 0 symlinks. Details: humanitl sessions summary <id>"), Exit mit `code` (Signal ⇒ 128+n).

`Ctrl+]` (0x1D) wird von der CLI abgefangen (nicht ans PTY geschickt) und zeigt eine Zeile `[humanitl] (d)etach (s)top (q)ueue (Esc) back`. `q` listet gehaltene Flows mit Nummer, `a<n>`/`b<n>` entscheiden. Das ist der Notausgang, wenn `--ask ui` läuft und kein UI da ist.

Prompt-Format `--ask terminal` (wird auf stderr geschrieben, während stdout weiter den Agenten zeigt; zur Vermeidung von Zeilensalat wird die Agent-Ausgabe für die Dauer des Prompts gepuffert, max. 256 KiB, dann durchgeleitet):

```
┌─ humanitl · request held (1 of 2) ─────────────────────────────── 04:52 left ─┐
│ POST https://api.github.com/graphql                                            │
│ from: opencode · webfetch                                                      │
│ size: 2.1 KB · json                                                            │
│ findings: 1 · GITHUB_TOKEN in header Authorization                             │
│ catalog: GitHub API · source hosting · Rang #37                              │
│                                                                                │
│ [a] allow once   [s] allow this session   [b] block   [r] rule…   [e] edit…   │
│ [v] view body    [n] next                                                      │
└────────────────────────────────────────────────────────────────────────────────┘
```

Tasten: `a` Allow einmal; `s` Allow + Regel `expires: session` mit Ziel Host; `b` Block; `r` öffnet Zwei-Schritt-Auswahl (Ziel: `1` exact URL, `2` host, `3` apex `**.`, `4` host+method; Dauer: `1` once, `2` session, `3` forever) und zeigt den Regelsatz vor Bestätigung mit `Enter`; `e` schreibt Request als Datei nach `$XDG_RUNTIME_DIR/humanitl/edit-<id>.http` (Roh-HTTP), öffnet `$EDITOR`, nach Schließen wird die Datei geparst und als `AllowEdited` gesendet (Parse-Fehler ⇒ zurück zum Prompt mit Fehlerzeile); `v` zeigt die ersten 4 KiB des Bodys mit `less`-artigem Pager (eingebaut, `q` zurück); `n` nächster gehaltener Request ohne Entscheidung. Countdown oben rechts aktualisiert sekündlich. Timeout ⇒ Zeile `[humanitl] timed out → blocked` und Prompt für den nächsten. `Ctrl+C` im Prompt ⇒ Prompt schließen (Request bleibt gehalten), zurück zum Agenten.

`--ask none` im Daemon: `HoldQueue` wird umgangen; `Verdict::Default` ⇒ `Decision::Block { reason: BlockReason::AskModeNone }` (neue Variante), Client erhält `403` mit `reason: ask_mode_none`. Der Flow wird als `Decided` aufgezeichnet, mit `rule_id = NULL`.

Exit-Codes: Agent-Exit-Code durchgereicht; 2 Daemon nicht erreichbar; 3 Isolation-Check fehlgeschlagen; 1 Nutzerfehler (Profil, Pfad); 4 nur für Tests.

Signal-Handling: `SIGINT` im Raw-Mode kommt als Byte 0x03 und geht an den Agenten; `SIGTERM` an die CLI ⇒ `Sandbox(Stop)`, Restore, Exit 143; `SIGHUP` (Terminal geschlossen) ⇒ Session läuft weiter (Detach-Verhalten), Meldung ins Daemon-Log.

Attach durch UI: Das UI zeigt in der Session-Liste (Sandbox-Screen Header-Dropdown, neu) alle laufenden Sessions mit Herkunft `cli · ~/clients/acme`. Auswahl ⇒ `Terminal`-Stream (die CLI wird nicht getrennt; die Ein-Client-Regel aus HUM-042 wird für den Fall CLI+UI gelockert: ein **schreibender** Client (CLI) und beliebige **lesende** Clients (`TerminalInput.detach` = read-only-Modus bei Verbindungsaufbau, neues Feld `read_only: bool`). Hold-Entscheidungen kann jeder Client treffen; die erste gewinnt, die zweite erhält `NotHeld`.

### Schritte
1. `tty.rs`: Raw-Mode mit garantiertem Restore (Guard-Struct, `Drop`, Panic-Hook, Signal-Handler via `signal-hook`).
2. `run.rs`: Verbindung, Profil, Start, Isolation-Ausgabe, Terminal-Bridge, Exit-Code.
3. `--ask none` im Daemon, `BlockReason::AskModeNone`.
4. `ask_terminal.rs`: Prompt-Rendering, Tastenbelegung, Regel-Dialog, Editor-Roundtrip, Pager, Ausgabe-Pufferung während Prompt.
5. `Ctrl+]`-Menü, `--detach`, `attach`.
6. Terminal-Handler: `read_only`-Clients.
7. `docs/cli.md`.
8. Integrationstests mit `expectrl` (PTY-Testtreiber) gegen echten Daemon und `-- sh`.

### Tests
- `run_sh_echo_exit_code`: `humanitl run --profile llm-only -- sh -c 'echo hi; exit 7'` ⇒ stdout enthält `hi`, Exit 7, Isolation-Zeilen erscheinen zuerst.
- `run_llm_only_blocks_curl`: `-- sh -c 'curl -sS -o /dev/null -w "%{http_code}" https://example.com'` ⇒ `403`, Flow `Decided(Block{AskModeNone})`, kein Held.
- `run_llm_only_allows_llm`: Fake-LLM unter `--llm` ⇒ `curl … /v1/models` ⇒ `200`.
- `ask_terminal_prompt_allow`: `--ask terminal -- sh -c 'curl …'` mit `expectrl`: Prompt erscheint mit `request held`, Taste `a` ⇒ curl liefert 200.
- `ask_terminal_rule_session`: Taste `s` ⇒ zweiter curl zum selben Host ohne Prompt.
- `ask_terminal_timeout_blocks`: `hold.timeout_secs=2`, keine Taste ⇒ `timed out → blocked`, curl 403.
- `raw_mode_restored_on_panic` (Unit mit simuliertem Panic).
- `sigterm_stops_session_exit_143`.
- `attach_read_only_sees_output`.

### Akzeptanzkriterien
- [ ] `cd ~/projekt && humanitl run --profile llm-only` startet OpenCode, der Prompt erscheint, `webfetch` liefert dem Agenten eine 403-Meldung, Inferenz funktioniert.
- [ ] `humanitl run --ask terminal` zeigt den Prompt exakt im Format oben, `a`/`b`/`s`/`r`/`e`/`v`/`n` funktionieren.
- [ ] Terminal ist nach jedem Exit-Pfad (normal, Ctrl+], SIGTERM, Panic) wieder im Normalmodus (`stty -a` zeigt `icanon echo`).
- [ ] UI zeigt die CLI-Session und kann sie read-only beobachten und Hold-Entscheidungen treffen.
- [ ] `docs/cli.md` beschreibt alle Flags, Tasten und Exit-Codes.

### Fallstricke
- **Prompt und Agent-Ausgabe im selben Terminal.** Ohne Pufferung überschreibt der Agent den Prompt. Pufferung mit Cap; wenn der Cap erreicht ist, wird der Prompt neu gezeichnet, nachdem die Ausgabe durchgeleitet wurde (einfacher als Cursor-Save/Restore, robust gegen TUI-Vollbild). Alternative nur für Vollbild-TUIs wie OpenCode: Prompt in die Statuszeile ist nicht möglich; deshalb Empfehlung in `docs/cli.md`: `--ask terminal` für Shell-Sessions, `--ask ui` oder `none` für TUI-Agenten.
- Raw-Mode ohne Restore macht das Terminal unbrauchbar. Restore in `Drop`, Panic-Hook und Signal-Handler, dreifach.
- `$EDITOR` kann ein GUI-Editor sein, der sofort zurückkehrt (`code` ohne `--wait`). Hinweis in der Prompt-Zeile: „waiting for editor to close".
- Ein zweites `humanitl run` im selben cwd ⇒ `SESSION_001` (Blocking): "A session for this folder is already running (id …). Attach with `humanitl attach <id>`."
- `--llm` mit öffentlicher IP ⇒ `LLM_006` wird vor dem Start gedruckt, Start läuft trotzdem (Info).

### Referenzen
BACKLOG.md ADR-013, Abschnitt 1.3 Prinzip 9; CONVENTIONS.md 3.8; HUM-042 (Terminal), HUM-066 (Profile). pipelock `action: ask` (https://github.com/luckyPipewrench/pipelock), `termios(3)`, `expectrl` (Crate).

---

## HUM-046 · Demo-Skript M3
Sprint: 3 · Größe: S · Abhängigkeiten: alle Issues dieses Sprints · Blockiert: Sprint 4

### Kontext
Jeder Sprint endet mit einem grünen Demo-Skript in CI (BACKLOG.md Abschnitt 8). M3 beweist: Agent in der Sandbox, LLM-Passthrough, Default-Regeln greifen, ein Hold wird per gRPC entschieden, das Ergebnis erscheint im Terminal-Stream. Da echtes OpenCode und ein echtes LLM in CI nicht verfügbar sind, laufen zwei Varianten: `agent-e2e` mit Mock-LLM und Mock-Agent (immer), und `agent-real` mit OpenCode gegen den Mock-LLM (nur wenn `opencode` im Runner-Image ist, sonst übersprungen).

### Ziel
`tests/e2e/m3_agent_inside.rs` startet den Daemon, den Ollama-Mock, eine Session mit Profil `default` und einem Skript-Agenten (`sh`), und prüft die fünf Demo-Schritte automatisch. Ein zweiter Test nutzt `humanitl run --ask none`. CI-Job `e2e-agent` führt beide aus.

### Nicht-Ziel
Keine UI-Automation (das ist `e2e-xvfb` in Sprint 4). Keine Performance-Messung.

### Betroffene Pfade
- `tests/e2e/mock_llm/` (neu): axum-Server
- `tests/e2e/m3_agent_inside.rs` (neu)
- `tests/e2e/fixtures/agent_script.sh` (neu)
- `.github/workflows/ci.yml`: Job `e2e-agent`
- `tests/e2e/README.md`

### Spezifikation

Ollama-Mock (`mock_llm`, axum, Port aus `--port`, Default 0 = zufällig, Ausgabe des Ports auf stdout):

- `GET /api/tags` ⇒ `{"models":[{"name":"mock:latest","modified_at":"2026-09-01T00:00:00Z","size":1}]}`
- `GET /v1/models` ⇒ `{"object":"list","data":[{"id":"mock","object":"model"}]}`
- `POST /v1/chat/completions` mit `stream: true` ⇒ SSE mit 10 Chunks à `{"choices":[{"delta":{"content":"tok{i} "}}]}` im Abstand 30 ms, dann `data: [DONE]`; ohne `stream` ⇒ eine JSON-Antwort mit `content: "mock reply"`. Der Mock speichert den letzten Request-Body unter `GET /_debug/last` (nur für Tests).
- `POST /api/chat` analog im Ollama-Format (`{"message":{"content":"tok"},"done":false}` NDJSON).

Agent-Skript (`agent_script.sh`, läuft in der Sandbox als `-- sh /tests/agent_script.sh`; die Datei wird per `SandboxFile` eingespielt):

```sh
#!/bin/sh
set -u
echo "STEP1 llm"
curl -sS -X POST "$LLM/v1/chat/completions" -H 'content-type: application/json' \
  -d '{"model":"mock","stream":true,"messages":[{"role":"user","content":"hello"}]}' | head -c 200
echo; echo "STEP2 modelsdev"
curl -sS -o /dev/null -w '%{http_code}\n' https://models.dev/api.json
echo "STEP3 webfetch"
curl -sS -o /dev/null -w '%{http_code}\n' https://example.com/docs
echo "STEP4 done"
```

`$LLM` wird über `sandbox.env` gesetzt (Test setzt `--llm http://127.0.0.1:<port>`; der Mock läuft auf dem Host, die Sandbox erreicht ihn ausschließlich über den Proxy-Passthrough, weil sie kein Interface hat).

Testablauf `m3_agent_inside`:

1. Daemon im Testmodus starten (`humanitld --config <tmp> --socket <tmp>/daemon.sock`), Mock starten, Port lesen.
2. gRPC-Client: `Sandbox(Start { profile: "default", work_dir: <tmp-projekt>, cli_overrides: { llm.endpoint, agent.command: ["sh","/tests/agent_script.sh"] } })`.
3. Erwartung A: drei `IsolationResult` mit `passed: true`, dann `Status(running)`.
4. `Terminal`-Stream öffnen (read-only), Ausgabe sammeln.
5. Erwartung B (STEP1): innerhalb 5 s ein Flow `Decided { passthrough: true }` zu `127.0.0.1:<port>` `/v1/chat/completions`; Terminal-Ausgabe enthält `tok0`; Mock `/_debug/last` enthält `"hello"`.
6. Erwartung C (STEP2): Flow zu `models.dev` mit `Decided(Block { reason: Rule(<id …0001>) })`, kein `Held`; Terminal zeigt `403`.
7. Erwartung D (STEP3): Flow zu `example.com` mit `Held`; Terminal-Stream enthält `[humanitl] request held: GET example.com/docs`. Der Test sendet `Decide { Allow }`. Da CI keinen Internetzugang garantiert, läuft für diesen Schritt ein zweiter lokaler Mock als Upstream: der Test setzt im Daemon-Testmodus `experimental.upstream_override = "127.0.0.1:<port2>"` (Testmodus-Flag, im Produktions-Build nicht vorhanden, `#[cfg(feature = "test-hooks")]`), der Mock antwortet `200`. Terminal zeigt `200`, Flow `Recorded`, Terminal zeigt `[humanitl] allowed`.
8. Erwartung E (STEP4): `Exit { code: 0 }`, `SessionEnded { summary }` mit `changes.len() == 0`.
9. History-Prüfung via `ListFlows`: genau 3 Flows in der Reihenfolge Passthrough, Block, Allow; Audit-Kette (falls HUM-050 bereits vorhanden, sonst übersprungen) verifiziert.

Zweiter Test `m3_cli_llm_only`: `humanitl run --profile llm-only --llm http://127.0.0.1:<port> --work <tmp> -- sh /tests/agent_script.sh` über `expectrl`; Erwartung: STEP1 zeigt `tok0`, STEP2 und STEP3 zeigen `403`, kein `Held` in `ListFlows`, Exit 0.

Dritter Test `m3_real_opencode` (`#[ignore]`, in CI nur wenn `which opencode` erfolgreich): Profil `default`, Adapter OpenCode, `--ask none`; Erwartung: innerhalb 30 s Terminal-Ausgabe enthält den OpenCode-Prompt-Marker (String aus `agents/opencode/README.md`, z. B. das TUI-Banner), `ListFlows` enthält höchstens einen `Held` (npm-Ask) und keinen Flow zu `models.dev` mit anderem Zustand als Block; Passthrough-Flow zu `/v1/models` vorhanden.

CI-Job `e2e-agent`: Ubuntu-Runner, installiert `bubblewrap`, `curl`, `socat`; führt `cargo test -p humanitl-e2e --features escape,test-hooks -- m3_` aus; Artefakte: Daemon-Log, Terminal-Transkript, `humanitl flows list --json`.

### Schritte
1. `mock_llm` schreiben (axum, ~120 Zeilen), eigener Cargo-Bin unter `tests/e2e`.
2. `test-hooks`-Feature mit `upstream_override` im Proxy (nur Testbuild).
3. `agent_script.sh`, Einspielung als `SandboxFile`.
4. Drei Tests, Hilfsfunktionen für Daemon-Start und Event-Warten mit Timeouts.
5. CI-Job, Artefakt-Upload.
6. README mit lokalem Aufruf.

### Tests
Die Tests sind das Deliverable. Zusätzlich: `mock_llm_streams_sse` (Mock isoliert), `agent_script_is_posix` (`shellcheck -s sh`).

### Akzeptanzkriterien
- [ ] `e2e-agent` grün in CI, Laufzeit unter 3 min.
- [ ] Lokal mit installiertem OpenCode: `cargo test -- --ignored m3_real_opencode` grün.
- [ ] Artefakte enthalten das Terminal-Transkript mit `[humanitl] request held` und `[humanitl] allowed`.

### Fallstricke
- Der Passthrough zu `127.0.0.1:<port>` funktioniert nur, weil der Proxy auf dem Host läuft und dort Loopback erreicht. In der Sandbox gibt es kein Loopback zum Host. Das ist die gewollte Architektur; der Test dokumentiert das.
- Regel-Engine: `HostName::Ip(127.0.0.1)` matcht nur die Passthrough-Regel (exakt); ein zweiter Mock-Port braucht `upstream_override`, nicht eine zweite IP-Regel.
- Timing: Isolation-Checks brauchen auf CI-Runnern bis zu 2 s (nftw). Warte-Timeouts großzügig (10 s), aber Gesamtlaufzeit begrenzen.
- `expectrl` mit Raw-Mode: der Test setzt `TERM=dumb` und prüft nur Substrings, keine Escape-Sequenzen.

### Referenzen
BACKLOG.md Abschnitt 8 (Demo-Skript-Regel), Sprint-3-Tabelle; HUM-021, HUM-036 (Vorläufer). axum, `expectrl`.


## HUM-071 · Agent-Briefing
Sprint: 3 · Größe: S · Abhängigkeiten: HUM-037 · Blockiert: HUM-046

### Kontext
ADR-014: Der Agent soll mit etwa 150 Token wissen, wo er ist, dass Warten normal ist, dass ein 403 von Humanitl endgültig ist, und wo er Status abfragen kann. Das Briefing ist Teil des `AgentAdapter`, liegt in der Sandbox-Home, nie in `/work`.

### Ziel
`OpenCodeAdapter::files()` liefert zusätzlich `~/.config/opencode/AGENTS.md` mit dem Inhalt aus `agents/opencode/briefing.<lang>.md`, `lang` aus `ui.language`. Der Text ist gebündelt (`include_str!`), enthält Platzhalter `{llm_host}`, `{timeout}`, `{ask_mode}`, die beim Start ersetzt werden.

### Nicht-Ziel
Kein Briefing-Editor im UI (Post-MVP, Settings `expert`). Keine Adapter außer OpenCode.

### Betroffene Pfade
- `agents/opencode/briefing.en.md`, `briefing.de.md` (neu)
- `daemon/crates/core-types/src/agent/opencode.rs` (`files()` erweitern)
- `daemon/crates/config/src/schema.rs`: `agent.briefing` (`enabled: bool`, Default true, Tier `advanced`)

### Spezifikation
`briefing.en.md` (Wortlaut verbindlich, ≤ 160 Token):

```
# Environment: Humanitl sandbox
You run inside an isolated sandbox with no direct internet access. Every HTTP(S) request you make goes through a proxy where a human reviews it and allows or blocks it. Waiting up to {timeout}s on a request is normal; do not abort or retry while waiting.
- Local LLM at {llm_host} is always allowed.
- A response `403` starting with `Blocked by Humanitl.` is final. Do not retry the same request. Read the `note:` line if present, tell the user what was blocked and why, and propose an alternative.
- Status and current rules: `GET http://humanitl.internal/`. To ask the user for access, `POST http://humanitl.internal/ask` with a one-line reason.
- Never try to bypass the proxy. Tools that ignore the proxy simply fail.
```

`briefing.de.md` ist die Übersetzung mit identischer Struktur. `ask_mode == none` ersetzt Zeile 3 durch „Requests without a rule are blocked automatically; ask the user to add a rule."

### Schritte
1. Vorlagen schreiben, Token zählen (`tiktoken`-Skript in `tools/`), ≤ 160.
2. `files()` erweitern, Platzhalter ersetzen, Test.
3. Prüfen, dass OpenCode die Datei liest: Sandbox starten, im Terminal `opencode` fragen „Wo läufst du?", Antwort enthält „Humanitl".

### Tests
- `opencode::tests::briefing_written_outside_work`: `files()` enthält Pfad unter `$HOME/.config/opencode/`, keiner unter `/work`.
- `opencode::tests::briefing_placeholders_replaced`: kein `{` im Ergebnis.
- e2e in HUM-046: Frage „Wo läufst du?" ⇒ Antwort enthält „Humanitl" oder „sandbox".

### Akzeptanzkriterien
- [ ] Datei existiert in der laufenden Sandbox unter `~/.config/opencode/AGENTS.md`.
- [ ] `/work` ist nach dem Start byte-identisch (Hash-Vergleich aus HUM-043).
- [ ] Beide Sprachen ≤ 160 Token.
- [ ] `agent.briefing.enabled = false` unterdrückt die Datei.

### Fallstricke
- OpenCode liest projektlokale `AGENTS.md` aus `/work` zusätzlich; das Briefing darf nicht dort landen, sonst wird es committet.
- Wortlaut ist Teil der Sicherheitskommunikation: keine Formulierung, die dem Agenten „Umgehen" als Option nahelegt, auch nicht verneint in Details.

### Referenzen
BACKLOG.md ADR-014; `docs/ARCHITECTURE.md` 8.1; OpenCode Rules-Doku (https://opencode.ai/docs/rules/).

## HUM-073 · Meta-Endpoint `humanitl.internal`
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-039, HUM-072, HUM-022 · Blockiert: HUM-046

### Kontext
ADR-014: Statusabfrage und Bitte an den Nutzer über den einzigen vorhandenen Kanal. Der Proxy beantwortet Requests an den Host `humanitl.internal` selbst, ohne DNS, ohne Upstream, ohne Regelauswertung.

### Ziel
`GET http://humanitl.internal/` liefert `text/plain` mit Session-ID, Ask-Modus, Timeout, LLM-Host und den aktuell gültigen Regeln (eine Zeile pro Regel: `allow  GET,HEAD  *.npmjs.org  session`). `GET /why/<flow-id>` liefert Entscheidung, Grund, Notiz. `POST /ask` mit Freitext-Body (≤ 2 KB) erzeugt `FlowEvent::AgentAsk { text, ts }` und im UI eine Karte „Der Agent bittet um …" mit Button „Regel anlegen" (öffnet HUM-028-Sheet mit vorausgefülltem Host, falls im Text eine URL steht). Alles andere ⇒ `404`, andere Methoden ⇒ `405`.

### Nicht-Ziel
Keine Aktion durch den Agenten: `/ask` legt nie eine Regel an. Kein JSON-API (Post-MVP, wenn Plugins es brauchen). Keine Authentifizierung nötig, da nur aus der Sandbox erreichbar.

### Betroffene Pfade
- `daemon/crates/proxy/src/meta.rs` (neu)
- `daemon/crates/proxy/src/handler.rs`: Weiche vor Regelauswertung, `authority.host == "humanitl.internal"`
- `daemon/crates/core-types/src/flow.rs`: `FlowEvent::AgentAsk`
- `proto/humanitl/v1/humanitl.proto`: `AgentAsk`-Variante
- `app/lib/features/intercept/widgets/agent_ask_card.dart` (neu)

### Spezifikation
- Nur Scheme `http`; `https://humanitl.internal` wird per CONNECT terminiert und dann identisch behandelt (CA ist vertraut).
- `/`-Antwortformat:
```
humanitl session=<id> ask=<ui|terminal|none> timeout=<s> llm=<host:port>
rules (first match wins):
  allow   POST      192.168.1.50:11434  /v1/**   never   (llm passthrough)
  block   *         models.dev          never   (bundled)
  ask     *         *                   default
```
- `/why/<id>`: `decision=<allow|allow_edited|block|timed_out> reason=<…> note=<…>`; unbekannte ID ⇒ `404`.
- `/ask`: Body wird wie die Block-Notiz sanitized (HUM-072), Rate-Limit 10 pro Minute pro Session (danach `429`), Antwort `202 queued`.
- Meta-Requests werden im Recorder als Flow mit `state=Recorded`, `decision=meta` gespeichert (sichtbar in History, Filter `meta:true`), nicht gehalten, nicht auditiert außer `/ask`.

### Schritte
1. `meta.rs` mit Router (drei Pfade), Tests gegen `HoldQueue`-freien Handler.
2. Weiche im Handler vor `RuleSet::evaluate`.
3. `AgentAsk`-Event, Proto, Dart-Spiegel, Karte im Intercept (oben in der Queue, eigener Zustand `agentAsk`, violett wie Passthrough, Icon `message-square`).
4. Briefing (HUM-071) verweist bereits auf die Pfade; Demo in HUM-046 erweitern.

### Tests
- `meta::tests::status_lists_effective_rules` (Session-Regel vor persistenter).
- `meta::tests::why_unknown_404`, `meta::tests::ask_rate_limited`, `meta::tests::ask_creates_event`, `meta::tests::post_root_405`.
- Escape-Test-Ergänzung `esc-3-egress.sh`: `curl http://humanitl.internal/` aus der Sandbox ⇒ 200, kein DNS-Lookup auf dem Host.
- Widget-Test `agent_ask_card_test.dart`: Karte erscheint, „Regel anlegen" öffnet Sheet mit Host.

### Akzeptanzkriterien
- [ ] Die drei Pfade antworten wie spezifiziert, andere ⇒ 404/405.
- [ ] Kein Resolver-Aufruf für `humanitl.internal` (Resolver-Mock zählt 0).
- [ ] `/ask` erzeugt genau eine Karte pro Request, Rate-Limit greift.
- [ ] History zeigt Meta-Flows mit Filter `meta:true`. **Offen, verschoben nach HUM-103.** Der Zustandsautomat kennt keinen Weg von einer Nicht-Sperre nach `Recorded`, und `decision=meta` verlangt eine neue Variante in `Decision`, in `DecisionKind` der Proto, im Schema und im Filter des Recorders sowie die Historie in der Oberfläche. Meta-Anfragen erzeugen bis dahin gar keinen Flow; sichtbar ist allein `/ask` als `FlowEvent::AgentAsk` und als Karte (`backlog/CONVENTIONS.md` 4.24).

### Fallstricke
- Die Weiche muss vor DNS und vor Regelauswertung liegen, sonst landet `humanitl.internal` als `ask` in der Queue oder löst einen Lookup aus.
- Regeln in `/` nie mit Notizen oder `created_from` ausgeben; nur Aktion, Methode, Host, Pfad, Ablauf.
- `/ask`-Text ist Agent-Eingabe: im UI als Klartext rendern, nie als Markdown oder Link.

### Referenzen
BACKLOG.md ADR-014; `docs/ARCHITECTURE.md` 8.3; HUM-072.


## HUM-075 · `humanitl doctor`
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-064, HUM-063, HUM-041 · Blockiert: HUM-044, HUM-077

### Kontext
Prinzip 9 („It just works") gegen Linux-Varianz: bwrap-Versionen, User-Namespace-Einschränkungen (Ubuntu 24.04+ AppArmor), fehlende systemd-User-Session, GNOME ohne AppIndicator, Impeller-Probleme. Vorbild `flutter doctor`: jede Zeile ein Befund mit Fix.

### Ziel
`humanitl doctor [--json]` prüft die Maschine und gibt eine Tabelle `ok|warn|fail` aus, jede Nicht-ok-Zeile mit `Diagnostic` (Code `DOCTOR_001..019`) und `FixAction` (meist `CopyCommand`). Exit 0 bei nur ok/warn, 3 bei fail. Der Setup-Screen (HUM-044) zeigt dieselben Zeilen über RPC `Doctor()` und blockiert den Sandbox-Start bei `fail`.

### Nicht-Ziel
Keine automatische Reparatur mit Root (nur Befehle anbieten). Keine Distro-spezifischen Paketmanager-Aufrufe außer als Text.

### Betroffene Pfade
- `daemon/crates/sandbox/src/doctor.rs` (neu), `daemon/bin/humanitl/src/cmd/doctor.rs` (neu)
- `proto/humanitl/v1/humanitl.proto`: `rpc Doctor(Empty) returns (DoctorReport)`
- `daemon/crates/core-types/src/diagnostics/codes.rs`: Bereich `DOCTOR_`
- `app/lib/features/setup/widgets/doctor_list.dart` (neu)

### Spezifikation
Prüfungen (Reihenfolge = Ausgabe):
| # | Prüfung | ok | fail/warn | Fix |
|---|---|---|---|---|
| 1 | `bwrap --version` ≥ 0.8 | vorhanden | fehlt ⇒ fail `DOCTOR_001` | `sudo apt install bubblewrap` |
| 2 | Userns: `unshare -Ur true` gelingt | ja | nein ⇒ fail `DOCTOR_002`; Ubuntu 24.04 AppArmor-Hinweis wenn `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` = 1 | Text mit `sysctl` bzw. Hinweis, dass System-bwrap profiliert ist und genutzt wird |
| 3 | seccomp: `/proc/self/status` hat `Seccomp` und Kernel ≥ 5.4 | ja | nein ⇒ fail `DOCTOR_003` | Kernel-Update |
| 4 | `$XDG_RUNTIME_DIR` gesetzt, 0700 | ja | nein ⇒ fail `DOCTOR_004` | `loginctl enable-linger`, Hinweis |
| 5 | systemd user session (`systemctl --user is-system-running`) | running | nein ⇒ warn `DOCTOR_005` (Daemon manuell startbar) | Befehl |
| 6 | Daemon-Socket erreichbar, Version passt | ja | nein ⇒ warn `DOCTOR_006` | `humanitl daemon install` |
| 7 | OpenCode im PATH, Version | ja | nein ⇒ warn `DOCTOR_007` | Install-Befehl aus Adapter |
| 8 | LLM-Endpoint antwortet (`/api/tags` oder `/v1/models`) | ja | nein ⇒ warn `DOCTOR_008` | „LLM finden" (HUM-076) |
| 9 | Tray: `libayatana-appindicator3` vorhanden, GNOME ⇒ Extension-Hinweis | ja | warn `DOCTOR_009` | Paket/Extension |
| 10 | GPU/Renderer: `FLUTTER_ENGINE`-Hinweis, NVIDIA + Wayland ⇒ warn `DOCTOR_010` | | | `--no-enable-impeller` |
| 11 | Freier Platz in `$XDG_DATA_HOME` ≥ 1 GB | ja | warn `DOCTOR_011` | Retention |

Ausgabe (Text): `[ok] bubblewrap 0.11.0`, `[fail] user namespaces: unshare -Ur failed (EPERM) — fix: …`. JSON: `{ "checks": [{ "id", "status", "evidence", "diagnostic"? }] }`.

### Schritte
1. `doctor.rs` mit `trait Check { fn run(&self) -> CheckOutcome }` und den elf Checks, jeder in < 2 s, parallel mit `std::thread::scope`.
2. CLI-Subkommando, Text- und JSON-Ausgabe, Exit-Codes.
3. RPC `Doctor`, Setup-Screen-Liste (wiederverwendet `DiagnosticCard`).
4. Fixture-Tests: jede Prüfung gegen gemockte Umgebung (Env-Variablen, Fake-`bwrap`-Skript im PATH).

### Tests
- `doctor::tests::bwrap_missing_is_fail`, `userns_blocked_apparmor_hint`, `xdg_runtime_missing`, `json_shape_stable` (Snapshot).
- CLI-Test: Exit 3 bei einem `fail`, 0 bei nur `warn`.

### Akzeptanzkriterien
- [ ] `humanitl doctor` auf der Entwicklungsmaschine: alle Zeilen ok oder warn mit Fix.
- [ ] Jede Nicht-ok-Zeile hat `why` und `fix`.
- [ ] Setup-Screen zeigt dieselben Zeilen, Start-Button bleibt bei `fail` deaktiviert.
- [ ] `--json` ist stabil (Snapshot-Test).

### Fallstricke
- `unshare -Ur` kann auf gehärteten Systemen hängen statt scheitern; Timeout 2 s.
- Auf Ubuntu 24.04+ ist unprivilegiertes userns per AppArmor eingeschränkt, das System-`bwrap` hat aber ein Profil. Deshalb prüft der Doctor `bwrap --unshare-user true`, nicht nur `unshare`.
- Keine Prüfung darf Netzwerk außer dem konfigurierten LLM-Endpoint ansprechen.

### Referenzen
BACKLOG.md Prinzip 9; HUM-044; Ubuntu userns-Restriktion (https://ubuntu.com/blog/ubuntu-23-10-restricted-unprivileged-user-namespaces).

## HUM-076 · LLM-Server finden
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-039, HUM-044 · Blockiert: keine

### Kontext
Prinzip 9: Der Nutzer soll keine IP kennen müssen. Ein LAN-Scan ist aber ein Netzwerkvorgang vom Host aus, deshalb nur auf expliziten Klick, sichtbar, nie automatisch.

### Ziel
Button „LLM finden" im Setup (und `humanitl llm discover [--json]`) sucht im lokalen /24 (aus der Default-Route-Schnittstelle) nach OpenAI-kompatiblen Servern und Ollama, listet Host, Port, Produkt, Modelle. Ein Klick übernimmt `llm.endpoint`. Vor dem Scan ein Hinweis: „Sucht im lokalen Netz (192.168.1.0/24) nach LLM-Servern. Sendet nur Verbindungsversuche an Ports 11434, 1234, 8000, 8080."

### Nicht-Ziel
Kein mDNS (Ollama kündigt nichts an). Keine Suche außerhalb des lokalen /24. Keine Übernahme ohne Klick.

### Betroffene Pfade
- `daemon/crates/sandbox/src/llm_discover.rs` (neu; gehört fachlich zur Sandbox-Session, nutzt `Egress::Direct`)
- `daemon/bin/humanitl/src/cmd/llm.rs` (neu)
- `proto/humanitl/v1/humanitl.proto`: `rpc DiscoverLlm(DiscoverRequest) returns (stream DiscoverResult)`
- `app/lib/features/setup/widgets/llm_discover_sheet.dart` (neu)

### Spezifikation
- Kandidaten: 254 Adressen × 4 Ports, TCP-Connect mit 200 ms Timeout, Parallelität 64. Danach pro offenem Port: `GET /api/tags` (Ollama, Antwort `models[]`), sonst `GET /v1/models` (`data[].id`). Timeout 2 s, Antwort ≤ 256 KB.
- Ergebnis `DiscoverResult { host, port, product: ollama|openai_compatible|unknown, models: Vec<String>, latency_ms }`, gestreamt sobald gefunden.
- Der eigene Host (127.0.0.1 und eigene LAN-IP) wird zuerst geprüft und oben gelistet.
- Der Scan läuft im Daemon (Host-Netz), nie in der Sandbox; er ist kein Flow und erscheint nicht in der Queue, wird aber im Audit-Log als `llm_discover` mit Zeit und Subnetz vermerkt.

### Schritte
1. Subnetz aus Default-Route (`/proc/net/route`) ableiten; bei Mehrdeutigkeit alle nicht-Loopback-Interfaces anbieten.
2. Scanner mit `tokio::net::TcpStream::connect` hinter `Egress::Direct`, Semaphore 64.
3. Erkennung per Endpunkt, Modelle extrahieren.
4. RPC, CLI, Sheet mit Fortschritt (x/254) und Abbruch.

### Tests
- `llm_discover::tests::detects_ollama_mock` (axum-Mock auf `127.0.0.1:11434`-ähnlichem Port über `experimental.upstream_port_map`).
- `llm_discover::tests::subnet_from_route`.
- Widget-Test: Ergebnisklick setzt `configProvider.llm.endpoint`.

### Akzeptanzkriterien
- [ ] Scan eines /24 endet in ≤ 5 s ohne Treffer.
- [ ] Ollama-Mock wird gefunden, Modelle gelistet.
- [ ] Ohne Klick kein einziger Verbindungsversuch (Test: Scanner-Aufrufzähler 0 beim Setup-Öffnen).
- [ ] Audit-Eintrag vorhanden.

### Fallstricke
- IDS/Firewalls in Firmennetzen melden Port-Scans; deshalb der Hinweistext und die Beschränkung auf vier Ports.
- Ollama ohne `OLLAMA_HOST=0.0.0.0` lauscht nur auf localhost; Ergebnisleere ist dann normal. Hinweistext im Sheet.
- vLLM `/v1/models` kann Auth verlangen (401) ⇒ als `openai_compatible (auth required)` listen, nicht verwerfen.

### Referenzen
BACKLOG.md Prinzip 9, HUM-039; Ollama API (https://docs.ollama.com/api); OpenAI Models-Endpoint.

---

## HUM-087 · resolver.test_ca wirkt nicht
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-024, HUM-036, HUM-062 · Blockiert: keine

### Kontext
Der Schlüssel `resolver.test_ca` steht im Schema (`daemon/crates/config/src/model.rs:213`), in der Schema-Fixture (`daemon/crates/config/tests/fixtures/config.schema.json:496`), in der Schlüsselliste des Tests (`daemon/crates/config/tests/schema.rs:170`) und in der erzeugten Referenz (`docs/CONFIG.md:160`: „Zusätzliche CA für Tests. Nur in Testläufen setzen, nie im Alltag."). Er wird geladen, validiert, in der Stufe `expert` geführt und erscheint mit HUM-069 im Settings-Bildschirm. Gelesen wird er nirgends: `daemon/bin/humanitld/src/main.rs:488-489` meldet beim Start nur `resolver.test_ca is set but the daemon does not read it yet (HUM-024)`, und `main.rs:510` baut den Verbindungsstapel mit `ClientTls::new(&[], config.experimental.h2_upstream)`, also mit leerer Wurzelliste. Ein Schlüssel, der dokumentiert und validiert ist und nichts bewirkt, behauptet eine Fähigkeit, die es nicht gibt. Das ist der Bruch, der dieses Issue zu einem Fehler und nicht zu einer Wunschliste macht: `backlog/CONVENTIONS.md` 4.13 verlangt „Nie mehr behaupten als bewiesen ist", und Prinzip 8 in `BACKLOG.md` sagt zu, eine Konfigurationsquelle speise UI und CLI gleichermaßen.

Der Preis ist messbar. Weil keine fremde Wurzel gilt, fährt der M2-Demolauf über Klartext-HTTP (`tests/e2e/m2_first_decision/script.json`, Kommentarblock und die Schritt-URLs), und damit bleibt der TLS-Weg des Proxys — CONNECT, Blatt aus der eigenen CA, Handschlag zum Ziel, Fund im entschlüsselten Body — im einzigen vollständigen Lauf des Produkts unbelegt. Sechzehn der siebzehn Anfragen sind Klartext; die einzige verschlüsselte existiert, um zu scheitern. `backlog/CONVENTIONS.md` 4.22 hält das als verlorene Abdeckung fest, nicht als bloße Variante, und nennt dieses Issue als den Weg zurück.

Kein Regress und kein rotes Demoskript: Der Lauf ist grün, weil er die heutige Lage ausdrücklich als Zusicherung festhält. Schritt 7 („a test CA in the configuration is not trusted on its own") erwartet für `https://registry.npmjs.org/tls-probe` genau `502` und `error = upstream_tls`. Daneben stehen drei Vorkehrungen, die die Aussage ehrlich halten und beim Umdrehen mitwandern: ein Stolperdraht auf `humanitld --help`, der mit einer Anweisung stirbt, sobald das Flag da ist; das Auslesen der Warnzeile aus dem Daemon-Protokoll, damit die Behauptung vom Daemon stammt und nicht aus einem Ausbleiben; und eine positive Kontrolle über `/tls-control`, die mit derselben Wurzel gelingt und ohne sie scheitert. `tests/e2e/m2_first_decision/config.toml` und `backlog/CONVENTIONS.md` 4.22 nennen diese Richtung die sichere Seite. Es fehlt eine Fähigkeit, Schweregrad major. Das Flag `--allow-test-ca`, auf das Konfiguration, Zertifikatsskript und Konventionen zeigen, gibt es nirgends (`grep -rn "allow-test-ca" daemon/` liefert null Treffer); sein einziger Fundort in einer Spezifikation ist HUM-036 (`backlog/sprint-2.md:1635` und `:1653`). Die Warnung verweist auf HUM-024, dessen Spezifikation (`backlog/sprint-2.md` ab Zeile 371) weder den Schlüssel noch das Flag nennt; auch dieser Verweis geht ins Leere.

Die Fähigkeit selbst ist da: `ClientTls::new(extra_roots, h2)` nimmt zusätzliche Wurzeln entgegen (`daemon/crates/proxy/src/upstream.rs:86`) und wird in den Proxy-Tests so benutzt (`daemon/crates/proxy/tests/support/mod.rs:367`, Bauer-Methode `ProxyBuilder::trust`). Zu tun ist Verdrahtung, keine neue TLS-Logik.

### Ziel
`humanitld --allow-test-ca` liest `resolver.test_ca`, lädt die Datei als PEM und reicht die enthaltenen Zertifikate als zusätzliche Wurzeln an `ClientTls::new` weiter; ohne das Flag bleibt die Wurzelliste leer, und der gesetzte Schlüssel erzeugt einen Befund statt einer nackten Zeile im Log. Ein Flag mit unbrauchbarer Datei lässt den Daemon gar nicht erst starten. Der M2-Demolauf fährt danach über `https://` und dreht Schritt 7 um: Die Anfrage, die die Sitzungsregel erlaubt, erreicht das Ziel.

### Nicht-Ziel
Kein Eintrag in den System- oder Browser-Trust-Store und keine Änderung an der Humanitl-CA oder an dem, was in die Sandbox geht (`docs/SECURITY.md` 5). Keine Kopplung von Flag und Schlüssel im Config-Crate; es kennt die Kommandozeile nicht. Kein Flag, das mehrere Pfade oder ein Verzeichnis nimmt. Keine zweite Daemon-Instanz im Demolauf, nur um die Richtung ohne Flag noch einmal zu zeigen: Sie wird im Rust-Test gemessen, nicht im Skript.

### Betroffene Pfade
- `daemon/bin/humanitld/src/main.rs`: `--allow-test-ca` in `Cli` (65-99), Lader `test_ca_roots`, Warnung 488-489 ersetzen, Wurzeln durch `build_handler` (501-510) reichen
- `daemon/crates/ipc/src/server.rs`: `build_llm_probe` (1088-1112) und der zweite Stapel mit leerer Wurzelliste (1101)
- `daemon/crates/core-types/src/diagnostics/codes.rs`: `CONFIG_007`, `CONFIG_008` (gemeinsam genutzte Datei, nur anhängen)
- `tests/e2e/lib.sh`: `start_daemon` reicht Daemon-Argumente durch (geteilt mit M1)
- `tests/e2e/m2_first_decision/{run.sh,script.json,config.toml}`, `tests/e2e/fake-upstream/gen-test-ca.sh:17-24`
- `docs/CONFIG.md:160` und `docs/DIAGNOSTICS.md` (beide erzeugt, siehe Fallstricke), `docs/SECURITY.md` 5
- `backlog/CONVENTIONS.md` 4.22 (Abschnitt über die Testwurzel), `backlog/sprint-2.md:1635` und `:1653`

### Spezifikation

Flag (Vorgabe aus, `expert`-Charakter im Hilfetext sichtbar):

```rust
/// Nimmt die Wurzel aus `resolver.test_ca` als zusätzlichen Vertrauensanker
/// für Verbindungen zum Ziel an. Nur für Testläufe.
#[arg(long)]
allow_test_ca: bool,
```

Lader in `main.rs`, damit die vier Fälle ohne laufenden Daemon prüfbar sind:

```rust
struct TestCa {
    roots: Vec<CertificateDer<'static>>,
    note: Option<Diagnostic>,
}

fn test_ca_roots(allow: bool, resolver: &ResolverConfig) -> Result<TestCa, Diagnostic>;
```

| Flag | `resolver.test_ca` | Ergebnis |
|---|---|---|
| aus | nicht gesetzt | `roots` leer, kein Befund; der Standardweg ist unverändert |
| aus | gesetzt | `roots` leer, `note` = `CONFIG_008` (Warning), `why`: „resolver.test_ca is set but the daemon was started without --allow-test-ca; the root is ignored", `fix`: `FixAction::CopyCommand("humanitld --allow-test-ca")` |
| an | nicht gesetzt | `roots` leer, `note` = `CONFIG_008` (Warning), `why` nennt die andere Hälfte: das Flag ohne Schlüssel bewirkt nichts |
| an | gesetzt, PEM mit mindestens einem Zertifikat | `roots` = alle gelesenen Zertifikate, kein Befund; der Start meldet `roots = <n>` und den Pfad auf `info` |
| an | gesetzt, Datei fehlt, unlesbar oder ohne Zertifikat | `Err(CONFIG_007)` (Error), `why` nennt Pfad und Ursache, `fix`: `FixAction::CopyCommand("openssl x509 -in <pfad> -noout -subject")`; der Daemon startet nicht |

Gelesen wird mit `CertificateDer::pem_slice_iter`, dem Muster aus `daemon/crates/proxy/src/ca.rs:512` und `:628`. Lehnt rustls eine Wurzel ab, kommt der Fehler bereits als `PROXY_003` mit „test CA is no trust anchor" aus `ClientTls::new` (`daemon/crates/proxy/src/upstream.rs:86-102`); dieser Fehler wird durchgereicht, nicht verschluckt.

Verdrahtung: `build_handler` bekommt einen Parameter `extra_roots: &[CertificateDer<'static>]` und gibt ihn an `ClientTls::new` weiter. Der zweite Stapel, die Endpunkt-Probe in `daemon/crates/ipc/src/server.rs:1101`, bekommt dieselben Wurzeln: Zwei verschiedene Vertrauensentscheidungen in einem Prozess wären genau die Überraschung, die dieses Repository vermeidet, und die Probe spricht mit demselben Netz wie der Proxy. Weil `IpcServer::new` neun Aufrufer hat, bleibt die Signatur, und die Wurzeln kommen über eine anhängende Methode:

```rust
/// Baut die Endpunkt-Probe mit zusätzlichen Wurzeln neu (`--allow-test-ca`).
pub fn with_extra_roots(self, roots: &[CertificateDer<'static>]) -> Self;
```

Demolauf: `start_daemon` in `tests/e2e/lib.sh` nimmt die Argumente des Daemons als weiteren Parameter entgegen (leer für M1), `run.sh` startet mit `--allow-test-ca`. Alle URLs in `script.json` gehen auf `https://`, ebenso `M2_URL_BLOCKED`, `M2_URL_ALLOWED` und `M2_URL_TIMEOUT` im Kopf von `run.sh`. Schritt 7 dreht sich um: `200` statt `502`, leeres `error`-Feld, und der Stolperdraht auf `humanitld --help` wird zu seinem Gegenteil — er muss das Flag jetzt finden. Die Gegenprobe in Schritt 9 zieht mit: `served` steigt von 16 auf 17, und aus „genau eine TLS-Anfrage, und zwar die Kontrolle" werden zwei, beide mit ihrem Pfad benannt. Die positive Kontrolle über `/tls-control` bleibt stehen; sie belegt weiterhin, dass das Material gültig ist, unabhängig davon, wem der Daemon vertraut. Die Zahlen in Schritt 8 bleiben, wie sie sind — die TLS-Anfrage zählt schon heute als `allow` und als eine der vierzehn erlaubten Anfragen an die Registry —, und `M2_EXPECTED_ASSERTIONS` wird am grünen Lauf nachgezogen.

Doku: `docs/CONFIG.md:160` nennt die Flagpflicht (über den Doc-Kommentar in `model.rs`, siehe Fallstricke). `docs/SECURITY.md` 5 bekommt einen Satz: Eine fremde Wurzel gilt nur für Verbindungen zum Ziel, nur mit `--allow-test-ca`, nie für die Sandbox und nie ohne ausdrücklichen Start. `backlog/CONVENTIONS.md` 4.22 wird von „gibt es nicht" auf die neue Lage umgeschrieben; `backlog/sprint-2.md:1635` und `:1653` werden auf den tatsächlichen Startbefehl angeglichen (kein `--config`; die Konfiguration findet der Daemon über den XDG-Baum des Laufs).

### Schritte
1. `test_ca_roots` samt `TestCa` schreiben, die fünf Zeilen der Tabelle als Unit-Tests in `main.rs`.
2. `CONFIG_007` und `CONFIG_008` ans Register anhängen, `docs/DIAGNOSTICS.md` erzeugen.
3. Flag in `Cli`, Warnung 488-489 entfernen, Befund in `run_daemon` protokollieren oder mit `Err` abbrechen; `build_handler` und `IpcServer::with_extra_roots` verdrahten.
4. Paar-Test im Proxy mit `UpstreamCa` und `ProxyBuilder::trust`.
5. Daemon-Test: Start mit Flag, ohne Flag und mit unbrauchbarer Datei.
6. `tests/e2e/lib.sh` um die Daemon-Argumente erweitern, M1 unverändert grün halten.
7. M2 auf `https://` stellen, Schritt 7 umdrehen, Zahlen in Schritt 9 und `M2_EXPECTED_ASSERTIONS` nachziehen, Lauf grün.
8. `docs/CONFIG.md` neu erzeugen, `docs/SECURITY.md`, `CONVENTIONS.md` 4.22, `sprint-2.md` und die Kommentare in `config.toml`, `script.json` und `gen-test-ca.sh` nachziehen.

### Tests
- `tests::test_ca_without_the_flag_is_ignored`, `tests::the_flag_without_the_key_says_so`, `tests::a_readable_test_ca_becomes_one_root`, `tests::an_unusable_test_ca_refuses_the_start` (alle in `daemon/bin/humanitld/src/main.rs`).
- `daemon/crates/proxy/tests/upstream_roots.rs`: dasselbe Blatt aus `UpstreamCa::new()`, einmal mit `ProxyBuilder::trust(root)`, einmal ohne.
- `daemon/bin/humanitld/tests/daemon_end_to_end.rs`: `a_test_ca_is_only_trusted_with_the_flag` (Log), `a_broken_test_ca_stops_the_start` (Exit-Code und fehlende Sockets).
- `tests/e2e/m2_first_decision/run.sh` Schritt 7, 9 und der Zähler am Ende.

### Akzeptanzkriterien
- [ ] `test_ca_roots` erfüllt die fünf Zeilen der Tabelle: ohne Schlüssel leer und ohne Befund, Schlüssel ohne Flag leer mit genau einem `CONFIG_008`, Flag mit gültiger PEM genau eine Wurzel, Flag mit Datei ohne Zertifikat `Err` mit `CONFIG_007`.
- [ ] `humanitld --allow-test-ca` mit gültiger Wurzel schreibt beim Start genau eine Zeile mit `roots=1` und dem Pfad; derselbe Baum ohne Flag schreibt `CONFIG_008` und keine Zeile mit `roots=`.
- [ ] `humanitld --allow-test-ca` mit unlesbarer Datei endet mit Exit-Code ungleich 0, meldet `CONFIG_007` mit `why` und `fix`, und weder `daemon.sock` noch `proxy.sock` entstehen.
- [ ] Paar-Test im Proxy: mit `trust(root)` antwortet der Proxy `200` und `upstream.hits() == 1`; ohne `trust(root)` antwortet er `502`, der Body enthält `reason: upstream_tls`, und `upstream.hits() == 0`.
- [ ] `E2E_ONLY=m2 tests/e2e/run.sh` grün mit `https://` in `script.json`: Schritt 7 misst `200` für `https://registry.npmjs.org/tls-probe`, leeres `error`-Feld und `m2_upstream_hits '/tls-probe'` gleich 1; Schritt 9 zählt 16 bediente Anfragen.
- [ ] `E2E_ONLY=m1 tests/e2e/run.sh` bleibt grün, und der M2-Lauf meldet weder „only N of M assertions ran" noch die Notiz über einen zu niedrigen `M2_EXPECTED_ASSERTIONS`.
- [ ] `make check` ist grün, ohne dass `docs/CONFIG.md`, `docs/DIAGNOSTICS.md` oder `config.schema.json` von Hand geändert wurden; `grep -rn "allow-test-ca" backlog/ docs/ tests/` beschreibt überall dieselbe, jetzt vorhandene Fähigkeit.

### Fallstricke
- `docs/CONFIG.md` und die Schema-Fixture sind erzeugt. Der Text zu `resolver.test_ca` ändert sich über den Doc-Kommentar in `daemon/crates/config/src/model.rs:212`, danach `UPDATE_CONFIG_DOCS=1 cargo test -p humanitl-config --test config_docs` und `UPDATE_SNAPSHOTS=1 cargo test -p humanitl-config --test schema`. Für das Code-Register `UPDATE_DIAG_DOCS=1 cargo test -p humanitl-core --test diag_docs`. Handarbeit an diesen drei Dateien macht `make check` rot.
- `codes.rs` ist eine gemeinsam genutzte Datei: unmittelbar vor dem Schreiben neu einlesen, nur anhängen, Bereich `CONFIG_001..009` (frei sind 007 bis 009).
- `tests/e2e/lib.sh` fährt beide Demos. Der neue Parameter von `start_daemon` muss vorne stehen bleiben, was M1 übergibt, sonst kippt der M1-Lauf mit.
- `M2_EXPECTED_ASSERTIONS=47` in `run.sh:76` wird exakt verglichen: zu wenige Behauptungen beenden den Lauf, zu viele erzeugen eine Notiz. Die Zahl gehört zu jeder Änderung an den Schritten dazu.
- Das Ziel des Laufs liegt in TEST-NET-2 im eigenen Netz-Namensraum, nicht auf `127.0.0.1`: Der Proxy weist private Adressen ab (`ip_is_private`, ADR-006, CONVENTIONS 4.22). Der Wechsel auf `https://` ändert daran nichts, und eine Vereinfachung auf Loopback bricht den Lauf.
- Das Flag ist standardmäßig aus und darf in keiner systemd-Unit, keinem Paket-Startbefehl und keiner Beispielkonfiguration auftauchen. Eine fremde Wurzel, die ohne Flag gälte, wäre ein Loch in `docs/SECURITY.md`.
- Die alte Warnung nennt HUM-024 als Grund; dieser Verweis ist falsch und verschwindet mit ihr, statt in den neuen Text übernommen zu werden.
- `backlog/sprint-2.md:1653` nennt zusätzlich `--config config.toml`, ein Flag, das `humanitld` ebenfalls nicht hat. Beim Angleichen wird geschrieben, was der Lauf tut, nicht was dort steht.
- Über `https://` läuft jede Anfrage des Demolaufs durch die TLS-Terminierung. Gruppierung, Funde und die Notiz im 403 werden am grünen Lauf nachgeprüft, nicht angenommen.

### Referenzen
BACKLOG.md Prinzip 3 und 8, ADR-006; `backlog/CONVENTIONS.md` 4.6, 4.13, 4.22; `docs/SECURITY.md` 5; `docs/CONFIG.md` `resolver.test_ca`; HUM-024, HUM-036, HUM-045; rustls `RootCertStore` (https://docs.rs/rustls/latest/rustls/struct.RootCertStore.html).

---

## HUM-088 · experimental.upstream_port_map wirkt nicht
Sprint: 3 · Größe: S · Abhängigkeiten: HUM-062 · Blockiert: HUM-076

### Kontext
`docs/CONFIG.md:80` sagt über `experimental.upstream_port_map`: „Lenkt einen Zielport auf einen anderen um, Schlüssel und Wert als Portnummer. Nur für Tests." Die Seite entsteht aus `daemon/crates/config/src/model.rs`, ist also keine Beschreibung neben dem Code, sondern der Code in Prosa. Umgelenkt wird trotzdem nichts. `grep -rn "port_map" --include="*.rs" daemon/` (ohne `target/`) trifft ausschließlich die Config-Kiste: Feld `daemon/crates/config/src/model.rs:452`, Prüfung `src/validate.rs:349`, Schema-Test `src/schema.rs:338`, dazu `tests/schema.rs:189` und `tests/precedence.rs:333`. In `daemon/crates/proxy/` und `daemon/bin/humanitld/` kommt der Name nicht vor. Der einzige gelesene Nachbar desselben Abschnitts ist `experimental.h2_upstream` in `daemon/bin/humanitld/src/main.rs:510`.

Das ist ein Fehler und nicht bloß eine Lücke, weil die Konfiguration sonst streng ist: Ein unbekannter Schlüssel in der Datei ist ein harter `CONFIG_002` (CONVENTIONS 4.11). Wer diesen Schlüssel setzt, bekommt deshalb kein „gibt es nicht", sondern Stille — und `experimental_is_well_formed` (`src/validate.rs:348`) prüft obendrein, dass die Schlüssel Portnummern sind. Ein Wert, der geprüft wird und dann nie gelesen wird, ist von außen nicht von einem wirksamen zu unterscheiden. CONVENTIONS 4.13 verlangt „Nie mehr behaupten als bewiesen ist"; für die Konfiguration gilt das wie für die Oberfläche. Für `resolver.test_ca` zieht der Daemon die Konsequenz und warnt beim Start (`daemon/bin/humanitld/src/main.rs:488-489`); für den Portschlüssel gibt es kein Gegenstück, obwohl `backlog/sprint-2.md:1690` genau das verlangt.

Die Lücke ist bereits zweimal festgehalten worden, ohne geschlossen zu werden: `backlog/CONVENTIONS.md` 4.22 (Zeile 1284) — „liest den Schlüssel `experimental.upstream_port_map` heute niemand — er steht im Schema, wird validiert und hat im Proxy keinen Aufrufer" — und `tests/e2e/m2_first_decision/run.sh:80`, das ausdrücklich daran vorbeifährt. Beide Testaufbauten, für die der Schlüssel gedacht war (HUM-024 in `backlog/sprint-2.md:439`, HUM-036 in `:1631`), stellen ihr Ziel auf `127.0.0.1:8443`; `ip_is_private` (ADR-006, angewendet in `daemon/crates/proxy/src/upstream.rs:213`) weist jede aufgelöste Adresse aus einem privaten Bereich ab, solange die Regel nicht `allow_private: true` setzt, und `127.0.0.1` ist eine — unabhängig vom Port. Der Schlüssel hätte die beiden Aufbauten also auch dann nicht lauffähig gemacht. HUM-024 ist ohne ihn fertig geworden: `pinned_addr_used` (`daemon/crates/proxy/tests/dns_after_allow.rs:274`) setzt stattdessen den echten flüchtigen Port in die Host-Kopfzeile. Kein ausgelieferter Test braucht den Schlüssel.

`Experimental` kündigt seinen eigenen Abbau an (`src/model.rs:71`, `docs/CONFIG.md:75`: „Schalter für unfertige Wege. Alles hier darf ohne Ankündigung wegfallen."). Ein Schalter, der nie einen Weg geschaltet hat, fällt deshalb weg, statt nachträglich einen zu bekommen.

### Ziel
`experimental.upstream_port_map` verschwindet aus Modell, Prüfung, Schema, Fixture, `docs/CONFIG.md` und aus allen Stellen in `backlog/` und `tests/`, die ihn als Test-Hebel nennen. Danach ist eine `config.toml` mit dem Schlüssel ein harter `CONFIG_002` mit dem Schlüsselnamen in der Meldung, und `docs/CONFIG.md` behauptet nichts mehr, was der Proxy nicht tut. `backlog/CONVENTIONS.md` 4.22 hält die Entscheidung samt Grund fest, damit sie nicht als Versehen gelesen wird.

### Nicht-Ziel
Kein Einbau der Portumlenkung in den Proxy; der Bauplan dafür steht unten in Spezifikation B und wird nur ausgeführt, wenn seine Bedingung eintritt. Keine Änderung an `ip_is_private` oder `allow_private`. Kein `upstream_override` (`backlog/sprint-3.md:1492` und `:1504`, `#[cfg(feature = "test-hooks")]` aus HUM-046) — ein anderer Hebel, der ebenfalls noch nicht existiert. Nicht behandelt wird die zweite Hälfte des Fallstricks aus `backlog/sprint-2.md:1690`: Auch für `resolver.overrides` warnt der Daemon beim Start nicht; das bleibt offen und gehört in ein eigenes Issue. `resolver.test_ca` bleibt, wie es ist (Warnung vorhanden, Leser fehlt, HUM-024).

### Betroffene Pfade
- `daemon/crates/config/src/model.rs:452` (Feld entfernen), `src/validate.rs:348-357` und `:386` (Prüffunktion und Aufruf entfernen), `src/schema.rs:338` (Testerwartung auf `resolver.overrides` umstellen)
- `daemon/crates/config/tests/schema.rs:189`, `tests/precedence.rs:333-336`
- `daemon/crates/config/tests/fixtures/config.schema.json` (erzeugt, zwei Stellen bei Zeile 65 und 77)
- `docs/CONFIG.md:80` (erzeugt)
- `backlog/CONVENTIONS.md:334`, `:390`, Abschnitt 4.22 (Zeile 1284)
- `backlog/RECONCILE.md:34`
- `backlog/sprint-2.md:414`, `:439` (HUM-024), `:1631`, `:1690` (HUM-036)
- `backlog/sprint-3.md:1733` (HUM-076, Test `detects_ollama_mock`)
- `tests/e2e/m2_first_decision/run.sh:80` (Kommentar)

### Spezifikation

**A. Entfernung (dieses Issue).**

`Experimental` behält `h2_upstream` und `ws_hold` und verliert `upstream_port_map`. `experimental_is_well_formed` hat danach nichts mehr zu prüfen und entfällt mitsamt dem Aufruf in `validate.rs:386`; `Experimental` als Import in `validate.rs` fällt mit weg, `BTreeMap` in `model.rs` bleibt (`resolver.overrides`, `sandbox.env`). Schema, Fixture und `docs/CONFIG.md` sind erzeugte Artefakte: `free_table_paths()` (`src/schema.rs:128`) leitet sich aus dem Schema ab und braucht keine Änderung, nur seine Testerwartung; die Fixture entsteht neu über den Fixture-Test, `docs/CONFIG.md` über `UPDATE_CONFIG_DOCS=1 cargo test -p humanitl-config --test config_docs`. Von Hand editiert driften beide.

`precedence.rs` prüft mit dem Schlüssel die Regel aus CONVENTIONS 4.11 („Freiform-Tabellen sind im Merge ein Blatt: eine höhere Ebene ersetzt die ganze Tabelle"). Diese Regel behält ihren Test, der Fall wandert auf die einzige verbleibende Freiform-Tabelle `resolver.overrides`, mit `lower = { "a.test" = "10.0.0.1" }` und `higher = { "b.test" = "10.0.0.2" }`.

Prosa-Stellen, wörtlich zu korrigieren, nicht zu streichen:

- `backlog/CONVENTIONS.md` 4.22, Satz ab Zeile 1284: aus „Ausserdem liest den Schlüssel `experimental.upstream_port_map` heute niemand" wird die Feststellung, dass der Schlüssel nie einen Leser hatte und mit HUM-088 entfernt wurde, samt Grund (`ip_is_private` hätte die gedachten Aufbauten ohnehin abgewiesen).
- `backlog/CONVENTIONS.md:334` (Liste 4.4) und `:390` (Freiform-Tabellen): Eintrag streichen beziehungsweise auf `resolver.overrides` reduzieren.
- `backlog/sprint-2.md:1631`: die Zeile `[experimental] upstream_port_map = { "443" = 8443 }` aus dem `config.toml` des Demoskripts entfernen; `:1690` nennt danach nur noch `resolver.overrides` als Test-Hebel. `:414` und `:439` (HUM-024) verlieren den Schlüssel aus Schlüsselliste und Test `pinned_addr_used`; was dort wirklich läuft, steht in `daemon/crates/proxy/tests/dns_after_allow.rs:274`.
- `backlog/sprint-3.md:1733`: `detects_ollama_mock` bindet den Mock auf einem flüchtigen Port und bekommt ihn übergeben, statt den Schlüssel zu nennen.
- `tests/e2e/m2_first_decision/run.sh:80`: Der Kommentar begründet die eigenen Ports weiter mit dem Netz-Namensraum, ohne einen Schlüssel zu nennen, der es nicht mehr gibt. `M2_EXPECTED_ASSERTIONS` bleibt 47.
- `backlog/RECONCILE.md:34` ist ein Protokolleintrag von 2026-09-02 und wird nicht umgeschrieben; er bekommt den Verweis auf dieses Issue in Klammern.

**B. Einbau (nicht dieses Issue, Bedingung und Bauplan).**

Bedingung: Ein ausgelieferter Test braucht die Umlenkung und lässt sich ohne sie nicht schreiben. Tritt sie ein, gilt: `Direct::new` (`daemon/crates/proxy/src/egress/direct.rs:26`) nimmt die Tabelle entgegen und bildet den Port in Zeile 52 ab, bevor `SocketAddr::new` ihn benutzt; die beiden Aufrufstellen `daemon/bin/humanitld/src/main.rs:512` und `daemon/crates/ipc/src/server.rs:1103` haben beide `&Config` zur Hand; `ProxyBuilder` (`daemon/crates/proxy/tests/support/mod.rs:368`) reicht sie durch; neben `main.rs:488` kommt die von `backlog/sprint-2.md:1690` verlangte Startwarnung dazu. Die Host-Kopfzeile aus `daemon/crates/proxy/src/upstream.rs:348` bleibt beim Originalport — das ist Absicht, weil sie zum Namen im Zertifikat gehört, und gehört als Satz in den Doc-Kommentar der Tabelle. Abnahme dieser Variante: Ein Lauf mit `{ "443" = 8443 }` gegen ein Ziel, das auf 8443 lauscht, antwortet 200; derselbe Lauf ohne den Schlüssel endet mit 502 und `PROXY_003`.

### Schritte
1. Feld, Prüffunktion und Aufruf entfernen; `cargo test -p humanitl-config` zeigt die Testerwartungen, die nachziehen müssen.
2. `schema.rs:338` und `tests/schema.rs:189` anpassen, `precedence.rs`-Fall auf `resolver.overrides` umstellen.
3. Fixture und `docs/CONFIG.md` neu erzeugen, Diff ansehen: nur die Blöcke des Schlüssels dürfen fehlen.
4. Prosa-Stellen aus Spezifikation A korrigieren, CONVENTIONS 4.22 mit Grund fortschreiben.
5. `tests/e2e/m2_first_decision/run.sh` einmal ganz fahren.
6. `make check`, danach `tools/verify-commit.sh` gegen den Commit.

### Tests
- `humanitl-config` Schema-Test: `known_paths()` und `leaf_paths()` enthalten `experimental.upstream_port_map` nicht mehr.
- `schema::tests::free_tables_are_leaves`: prüft `resolver.overrides`, nicht mehr den entfernten Schlüssel.
- `precedence`-Fall für Freiform-Tabellen auf `resolver.overrides`: untere Ebene `{ "a.test" = "10.0.0.1" }`, obere `{ "b.test" = "10.0.0.2" }`, Ergebnis ist die obere Tabelle allein.
- Neuer Fall in den Config-Tests: eine `config.toml` mit `[experimental] upstream_port_map = { "443" = 8443 }` liefert `CONFIG_002` mit `Severity::Error` und dem Schlüsselnamen im Text.
- Fixture-Test und `config_docs`-Test grün ohne erneutes Setzen von `UPDATE_CONFIG_DOCS`.
- `tests/e2e/m2_first_decision/run.sh` unverändert grün.

### Akzeptanzkriterien
- [ ] `grep -rn "upstream_port_map" --include="*.rs" daemon/ | grep -v "/target/"` liefert keine Zeile.
- [ ] `grep -rn "upstream_port_map" docs/ tests/ app/` liefert keine Zeile; in `backlog/` bleiben nur die Zeilen von CONVENTIONS 4.22 und dieses Issue.
- [ ] Ein Start mit einer `config.toml`, die `[experimental] upstream_port_map = { "443" = 8443 }` enthält, endet mit Exit-Code ungleich 0 und einem `CONFIG_002`, dessen `why` den Schlüssel nennt.
- [ ] `cargo test -p humanitl-config` grün, inklusive Fixture- und `config_docs`-Test ohne Neuschreiben der erzeugten Dateien.
- [ ] `git diff -- daemon/crates/config/tests/fixtures/config.schema.json docs/CONFIG.md` zeigt ausschließlich Entfernungen, die den Schlüssel betreffen.
- [ ] `tests/e2e/m2_first_decision/run.sh` Exit 0, `M2_EXPECTED_ASSERTIONS` weiterhin 47.
- [ ] `backlog/CONVENTIONS.md` 4.22 nennt HUM-088, den Entfernungsgrund und die Bedingung, unter der der Schlüssel zurückkäme.
- [ ] `make check` grün und `tools/verify-commit.sh` grün gegen den Commit, nicht gegen den Arbeitsbaum.

### Fallstricke
- `precedence.rs` verliert ohne Ersatz den einzigen Test für die Regel aus CONVENTIONS 4.11; `resolver.overrides` ist die letzte Freiform-Tabelle und muss den Fall übernehmen, sonst ist die Regel unbelegt.
- Entfernen ist ein Bruch für bestehende Dateien: Der Schlüssel wird vom stillen No-Op zum harten `CONFIG_002`. Das ist zulässig, weil `Experimental` es ankündigt (`model.rs:71`, `docs/CONFIG.md:75`), gehört aber in CONVENTIONS 4.22, damit ein Fehlerbericht dazu sofort einzuordnen ist.
- `daemon/crates/config/tests/fixtures/config.schema.json` und `docs/CONFIG.md` werden erzeugt. Von Hand geändert driften sie gegen `model.rs`, und der Fixture-Test wird erst im nächsten Lauf rot.
- HUM-039 nennt den Schlüssel nicht; wer in dessen Nähe sucht, findet `upstream_override` (`backlog/sprint-3.md:1492` und `:1504`) und verwechselt zwei verschiedene, beide nicht existierende Hebel.
- Beim Einbau nach Variante B: `Direct::new` ist heute `const fn` und verliert das mit einem `BTreeMap`-Parameter; `impl Default for Direct` zieht mit.
- Beim Einbau nach Variante B: `daemon/crates/proxy/src/upstream.rs:348` baut die Host-Kopfzeile aus `request.authority.port`. Wird sie mit abgebildet, passt der Name nicht mehr zum Zertifikat, und der Test misst das Falsche.

### Referenzen
`backlog/CONVENTIONS.md` 4.4, 4.11, 4.13, 4.22; BACKLOG.md ADR-006; `docs/CONFIG.md`; HUM-024 (`backlog/sprint-2.md:371`), HUM-036 (`backlog/sprint-2.md:1605`), HUM-076 (`backlog/sprint-3.md:1702`).


## HUM-101 · Konfigurationsschlüssel ohne Leser
Sprint: 3 · Größe: L · Abhängigkeiten: HUM-062, HUM-063 · Blockiert: —

### Kontext
`docs/CONFIG.md` wird aus dem Schema erzeugt (`daemon/crates/config/tests/config_docs.rs`) und ist deshalb nie veraltet, was die **Existenz** eines Schlüssels angeht. Über seine **Wirkung** sagt der Generator nichts. Mindestens sieben Schlüssel stehen damit heute mit einer Zusage im Dokument, die der Code nicht einlöst:

- `limits.idle_timeout_secs` (Vorgabe 90) und `limits.body_timeout_secs` (Vorgabe 300). Beide werden nur in `validate.rs:211-212` auf ihren Bereich geprüft und danach von niemandem gelesen. `docs/CONFIG.md:120` und `:127` sagen zu, dass sie Verbindungen beenden.
- `experimental.upstream_port_map` — das ist HUM-088 und wird dort entfernt.
- `experimental.ws_hold` — Treffer nur unter `#[cfg(test)]` (`humanitl/src/cli.rs:644`).
- `ui.sound` und `ui.notifications` — im Rust-Teil kein Treffer, und der Leser in Dart ist festverdrahtet: `app/lib/features/tray/providers/attention.dart:70` lautet `bool notificationsEnabled(Ref ref) => true;`, der Kommentar darüber räumt die fehlende Verdrahtung ein.
- `pseudonyms.max_response_bytes` und `pseudonyms.translate_responses` gehören zu HUM-079 aus Sprint 4 und haben zu Recht noch keinen Leser — aber das Dokument sagt es nicht, sondern beschreibt sie in derselben Gegenwartsform wie jeden wirksamen Schlüssel.

Bei den beiden `limits`-Schlüsseln wiegt es schwerer als bei `resolver.test_ca` (HUM-087), weil sie Ressourcengrenzen zusagen, die es nicht gibt.

### Was der Review dieses Issues ergeben hat, und was daraus folgt
Die erste Fassung wollte beide Zeitgrenzen einbauen und dabei das Halten ausnehmen. Der Review hat gezeigt, dass das so nicht baubar ist, und die Fassung ist daran angepasst:

Der Hold sitzt **in** der Service-Future innerhalb `serve_connection` (`handler.rs:1193-1211`). Während er läuft, fließen auf der Verbindung des Agenten null Bytes. Eine Leerlaufuhr auf dieser Verbindung feuert also genau dann, wenn sie nicht darf — und mit den Vorgaben 90 Sekunden Leerlauf gegen 300 Sekunden Haltefrist würde die Standardkonfiguration **jeden** Hold nach 90 Sekunden töten. `hyper` bietet dafür keinen Aufhänger; die einzige Stelle wäre ein Wrapper um den Socket in `core.rs:134-141`, vor `serve_connection`, mit einem Zähler gehaltener Flüsse, der die Uhr anhält. Das ist kein Beiwerk, sondern der eigentliche Aufwand des Issues, und es ist der Grund für die Größe L.

Zweitens beschreiben `idle_timeout_secs` und `header_timeout_secs` auf einer Keep-Alive-Verbindung dieselbe Spanne — der Doc-Kommentar in `handler.rs:86-88` sagt selbst, die Kopf-Frist sei „zugleich die Frist bis zur nächsten Anfrage". Zwei Schlüssel für eine Uhr sind eine Zusage zu viel. **Erste zu treffende Entscheidung dieses Issues:** entweder speist `idle_timeout_secs` die vorhandene `header_read_timeout` und `header_timeout_secs` verschwindet, oder umgekehrt. Ein dritter Weg ist, `idle_timeout_secs` nach dem Muster von HUM-088 ganz zu entfernen; das ist der ehrlichste, wenn sich keine Spanne findet, die er allein beschreibt.

Drittens ist beim Ziel nur der gestreamte Antwort-Body unbegrenzt (`body.rs:102-120`); bis zu den Antwort-Kopfzeilen deckt `handshake_timeout` bereits alles ab (`upstream.rs:267,287,301`), gespeist aus `header_timeout_secs`. Die Rumpfgrenze gehört also genau dorthin und nirgends sonst — drei Uhren auf einer Verbindung ergeben einen Abbruch, dessen Grund niemand nennen kann.

Viertens: `reason: idle_timeout` im Blockbanner ist unlieferbar. Im Leerlauf ist keine Anfrage in Flug, also gibt es keine Antwort, in die ein Banner passt (`core-types/src/block.rs:162-163`). Der Leerlauf schließt still und meldet sich im Ereignisstrom und im Protokoll. Nur die Rumpfgrenze auf der Client-Seite hat eine Anfrage in Flug und kann ein Banner tragen; auf der Ziel-Seite trüge die Zeile ohnehin das Präfix `upstream_` (`flow.rs:184`).

Fünftens speist `limits.*` auch den IPC-Stapel (`ipc/src/server.rs:1107-1112`). Eine symmetrische Verdrahtung dorthin würde den Ereignisstrom der Oberfläche abschneiden, der minutenlang stumm sein darf. Der IPC-Stapel bleibt ausgenommen.

### Ziel
Die Zusagen im Dokument und das Verhalten stimmen wieder überein — durch Einbau, wo eine Grenze sinnvoll ist, und durch Streichung, wo sie es nicht ist. Jeder Schlüssel im Schema trägt entweder einen Leser oder die Angabe, ab welchem Issue er wirkt; `docs/CONFIG.md` schreibt diese Angabe mit, und ein Test verhindert, dass ein achter Fall entsteht.

### Nicht-Ziel
Keine Umsetzung von HUM-079 und keine Entfernung von `experimental.upstream_port_map` (HUM-088). Keine Verdrahtung von `ui.sound` und `ui.notifications` — sie bekommen die Angabe und ihr eigenes Issue.

### Betroffene Pfade
- `daemon/crates/config/src/model.rs`: `#[schemars(extend(...))]` mit einem neuen Schlüssel `x-pending-issue`, nach dem Vorbild von `x-tier` (`model.rs:141-174`)
- `daemon/crates/config/src/schema.rs`: die Angabe wird über `leaf_paths()` erreichbar
- `daemon/crates/config/tests/config_readers.rs` (neu): das Register und sein Test
- `daemon/crates/config/tests/config_docs.rs`: der Generator schreibt die Angabe in die Tabelle
- `daemon/crates/proxy/src/core.rs`: der Wrapper um den Socket samt Zähler
- `daemon/crates/proxy/src/connect.rs`: der Zähler im `ConnectionContext`, den `tunnel` mitklont
- `daemon/crates/proxy/src/body.rs`: die Rumpfgrenze am gestreamten Antwort-Body
- `proto/humanitl/v1/humanitl.proto`, `daemon/crates/ipc/src/convert.rs`, `app/lib/core/domain/flow_state.dart`: falls ein neuer `reason` entsteht
- `docs/CONFIG.md` aus dem Generatorlauf

### Spezifikation
**Das Register statt einer Heuristik.** Die erste Fassung wollte ein Shell-Gate, das über den Feldnamen nach einem Leser sucht. Der Review hat es durchgezählt: `env` trifft 244-mal, `profile` 464-mal, `command` 293-mal; `timeout_secs` ist Teilzeichenkette von vier verschiedenen Schlüsseln; `enabled` ist zweimal vergeben (`findings.enabled`, `recorder.enabled`) und über den Namen nicht zu trennen; ein Doc-Kommentar zählt als Treffer; und `humanitl config get` liest generisch über serde jeden Schlüssel, sodass formal keiner leserlos ist. Die Heuristik findet also weder zuverlässig noch trennt sie „wirkt" von „wird serialisiert".

Stattdessen ein Register im Repository, eine Zeile je Schema-Pfad, mit genau einer Einstufung: `effective` oder `pending(HUM-xxx)`. Ein Test in `daemon/crates/config/tests/` liest `schema::leaf_paths()` und das Register und wird rot, sobald das Schema einen Pfad kennt, den das Register nicht kennt, oder umgekehrt. Das ist dasselbe Muster wie beim Diagnostik-Register, es braucht keinen neuen Schritt in `make check`, und es muss das Schema nicht aus einer Shell heraus parsen — was ohnehin nicht ginge.

Die Einstufung `effective` ist eine Behauptung eines Menschen, keine Messung. Das ist Absicht: das Register soll den **vergessenen** Schlüssel finden, und ein Mensch, der beim Anlegen eines Schlüssels „wirkt" schreibt, ohne ihn zu verdrahten, hat bewusst gelogen statt es zu übersehen.

**Die Zeitgrenzen.** Die Rumpfgrenze begrenzt die Zeit, in der ein Rumpf vollständig angekommen sein muss, und sitzt für die Ziel-Richtung am gestreamten Antwort-Body. Die Leerlaufgrenze — sofern die erste Entscheidung sie behält — sitzt im Wrapper vor `serve_connection` und ruht, solange die Verbindung einen gehaltenen Fluss trägt. Sie ruht auch für die Durchreiche zum Sprachmodell: ein lokales Modell schweigt vor dem ersten Token regelmäßig länger als 90 Sekunden, und dieser Seitenkanal ist ausdrücklich erklärt.

### Tests
- Paar-Test der Rumpfgrenze: ein Antwort-Body, der zu langsam kommt, wird abgebrochen; mit hoher Grenze nicht. Zeit über eine einspeisbare Uhr, nie über die Wanduhr.
- Ein Test, dass eine gehaltene Anfrage die Leerlaufgrenze überlebt, auch wenn sie länger wartet — und ein zweiter, dass eine Verbindung **ohne** gehaltenen Fluss sie nicht überlebt. Ohne den zweiten prüft der erste nichts.
- Ein Test, dass eine Durchreiche zum Sprachmodell mit langer Stille nicht abgebrochen wird.
- Register-Probe: einen Schlüssel im Schema hinzufügen, ohne Zeile im Register — der Test wird rot und nennt den Pfad.
- `ui.sound`, `ui.notifications`, `experimental.ws_hold` und die beiden `pseudonyms`-Schlüssel tragen `pending`, und `docs/CONFIG.md` zeigt es.

### Akzeptanzkriterien
- [ ] Die erste Entscheidung (`idle_timeout_secs` gegen `header_timeout_secs`) ist getroffen und in `CONVENTIONS.md` begründet; am Ende beschreibt kein Schlüssel dieselbe Spanne wie ein anderer.
- [ ] Jeder Schema-Pfad steht im Register, und der Test wird rot, sobald einer fehlt oder einer zu viel ist.
- [ ] Eine gehaltene Anfrage und eine schweigende Durchreiche überleben die Leerlaufgrenze; eine leere Verbindung nicht.
- [ ] `docs/CONFIG.md` kommt aus dem Generatorlauf und nennt für jeden noch nicht wirksamen Schlüssel sein Issue.
- [ ] Der IPC-Stapel ist von den Grenzen ausgenommen, und ein Test belegt, dass ein minutenlang stummer Ereignisstrom nicht abbricht.
- [ ] `make check`, clippy mit `-D warnings` und `cargo fmt --all -- --check` grün.

### Fallstricke
- Eine Leerlaufgrenze auf der Verbindung zum Agenten schneidet ohne den Zähler genau den Hold ab, den sie verschonen soll. Das ist der Grund, warum dieser Schlüssel gefährlicher ist als sein Fehlen.
- Der Zähler muss die CONNECT-Rekursion überleben: `ConnectionContext::tunnel` klont den Kontext (`handler.rs:335-341`), und ein Zähler, der dabei verlorengeht, lässt getunnelte Holds ungeschützt.
- Ein neuer `reason` zieht Proto, `convert.rs` und die Dart-Seite nach sich, dazu `CONVENTIONS.md` 3.2 mit dem Status je Grund.

## HUM-102 · Die abgelehnte private Adresse hat keinen Diagnostic
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-015, HUM-025 · Blockiert: —

### Kontext
ADR-006, Zeile 39: „wird die Verbindung abgelehnt: `BlockReason::PrivateAddress`, Diagnostic `PROXY_005` mit einem Regelvorschlag." Den Diagnostic gibt es nicht. `PROXY_005` heißt im Code „Ungültiger Übergang im Flow" (`core-types/src/diagnostics/codes.rs:295`) und wird an ganz anderer Stelle ausgelöst (`handler.rs:1099`). Für die abgelehnte private Adresse gibt es `BlockReason::PrivateAddress` und `UpstreamError::PrivateAddress(ip)`, beides erreicht die Oberfläche und trägt die Adresse mit (`ipc/src/convert.rs:1068-1072`) — aber keinen Befund, kein `why`, keinen `fix`.

Das trifft den Normalfall dieses Produkts. Das Sprachmodell steht im LAN, andere interne Dienste auch. Die Durchreichregel trägt `allow_private` und deckt nur Inferenz und die Modellliste (`sandbox/src/agent/opencode.rs:423-453`). Jede andere Anfrage an denselben Server geht durch die Warteschlange, und wenn ein Mensch dort „Erlauben" drückt, scheitert sie trotzdem: `flow.allow_private` wird ausschließlich aus einer Regel gesetzt (`pipeline.rs:349`), nie aus einer menschlichen Entscheidung. Der Mensch sieht eine abgelehnte Verbindung mit einer IP-Adresse und erfährt nicht, dass seine Freigabe an einer Sperre gescheitert ist, die nur eine Regel öffnet.

### Der Fund, der dieses Issue umgedreht hat
Der Review hat gezeigt, dass ein bloßer Regelvorschlag das Produkt **unsicherer** machen würde, statt es zu erklären. Der Grund steht in `pipeline.rs:347-349`: `flow.allow_private` wird nur im Zweig `Action::Allow` gesetzt. Eine Regel mit `action: ask` und `allow_private: true` parst anstandslos (`rules/src/parse.rs:355`, ohne die Prüfung, die `passthrough_llm` hat) und **wirkt nicht**. Wer also dem naheliegenden, engen Vorschlag folgt, bekommt eine wirkungslose Regel und eine zweite Ablehnung ohne neue Erklärung. Wirksam bleibt nur `action: allow` — und damit wird aus „ein Mensch erlaubt eine Anfrage" dauerhaft „alle künftigen Anfragen an diesen Host laufen ungehalten hinaus". Der Vorschlag verleitet also bauartbedingt zu mehr Öffnung als die Freigabe, die gerade gescheitert ist.

Deshalb ist die Behebung von `pipeline.rs` Teil dieses Issues und nicht eines späteren: `flow.allow_private |= self.rule_allows_private(rule);` gehört vor das `match action`, damit eine Regel mit `action: ask` und `allow_private: true` das Ziel öffnet und die Anfrage trotzdem jedes Mal einem Menschen gezeigt wird. Das ist der Vorschlag, den der Befund machen soll.

### Ziel
Die Ablehnung einer privaten Adresse liefert einen Befund mit eigener Nummer, `why` und einem `fix`, den ein Klick anwendet — und der vorgeschlagene Weg ist der enge: das Ziel wird geöffnet, die Aufsicht bleibt.

### Nicht-Ziel
Keine Aufweichung von ADR-006: eine menschliche Freigabe öffnet weiterhin **nicht** von sich aus ein privates Ziel. Kein globaler Schalter. Nicht in diesem Issue: die zweite Entscheidung mit sichtbarer Adresse — die Ablehnung samt Adresse zurück in die Warteschlange zu geben, damit ein Mensch ein zweites Mal entscheidet, diesmal über `10.0.0.5`, gültig für diese eine Anfrage. Das wäre die sauberste Auflösung des Widerspruchs, dass die unbeaufsichtigte Mechanik (die Regel) mehr darf als die beaufsichtigte (der Mensch); sie ist für M3 zu groß und bekommt ihr eigenes Issue, sobald dieses steht.

### Betroffene Pfade
- `daemon/crates/core-types/src/diagnostics/codes.rs`: `PROXY_008`, ans Ende des Bereichs
- `daemon/crates/proxy/src/handler.rs`: der Befund entsteht in `record_failure` beim Match auf `UpstreamError::PrivateAddress(ip)`
- `daemon/crates/proxy/src/pipeline.rs`: `allow_private` vor das `match action`
- `daemon/crates/rules/src/parse.rs`: Rundlauf-Test für die vorgeschlagene Regel
- `docs/adr/0006-dns-after-allow.md`: die falsche Nummer wird berichtigt
- `docs/DIAGNOSTICS.md` aus dem Generatorlauf

### Spezifikation
Der Befund entsteht an **einer** Stelle: in `record_failure`, beim Match auf `UpstreamError::PrivateAddress(ip)`. Nicht dort, wo `AddressRefusal::Private` zurückkommt — das ist nur der DNS-Zweig; ein IP-Literal liefert den Fehler direkt (`upstream.rs:212-215`), und der dritte Aufrufer ist die Endpunkt-Probe (`ipc/src/server.rs:1102-1114`), wo es weder einen Fluss noch eine sinnvolle Regel gibt.

Der `fix` ist ein `FixAction::AddRule(Box<Rule>)` — dieser Typ existiert und ist bis in die Oberfläche durchgezogen (`core-types/src/diagnostics/mod.rs:85`, `ipc/src/convert.rs:110`, die Proto, `app/lib/core/ipc/convert.dart:70`) und wird dort per Klick angewandt. Ein Text zum Abtippen wäre nicht nur schwächer, er erfüllte das eigene Akzeptanzkriterium nicht, denn `fix` ist `Option<FixAction>` und kein Freitext. Und das Abtippen ist genau die Stelle, an der die Enge verlorengeht.

Die vorgeschlagene Regel trägt: `action: ask` mit `allow_private: true`; `HostPattern::Exact(host)`, bei einem IP-Literal `HostPattern::Ip(ip)`, weil ein Glob eine Adresse nie trifft (ADR-007, `rules/src/host.rs:126`); den Port; das Schema; und die Methode der gescheiterten Anfrage. Der Pfad kommt aus `flow.request.path_and_query`, aber **immer** durch `strip_query` (`rules/src/path.rs:78-96`) — roh übernommen landete sonst ein Token aus der Abfragezeichenkette in `rules.yaml`. Ergibt der Pfad kein zulässiges Präfix (die Mindestlänge ab `/` ist zwei Zeichen, `core-types/src/rule.rs:396-402`, also etwa bei `GET /`), bleibt das Feld weg statt leer.

Die Adresse und der Vorschlag stehen im `Diagnostic` und in `resolved_ip`, **nie** im Rumpf der Blockantwort und nie in einer Kopfzeile zum Agenten. Die Sandbox hat keinen Resolver — das ist die erste Garantie —, der Agent kennt also nur den Namen. Die Zuordnung dieses Namens zu einer privaten Adresse ist für ihn neue Information und taugt zur Vermessung des LAN. Heute geht sie ausschließlich an die Oberfläche (`convert.rs:772`, `:1068`), und der Banner trägt nur den Grund ohne Adresse (`block.rs:151-156`); das bleibt so.

### Tests
- Ein Fluss auf einen Namen, der auf `10.0.0.5` auflöst, ohne Regel mit `allow_private`: genau ein Befund, mit der Adresse und einer `FixAction::AddRule`, deren Regel Host, Port, Schema und Methode nennt.
- Derselbe Fall mit einem IP-Literal: der Befund entsteht ebenso, und die Regel trägt `HostPattern::Ip`.
- Rundlauf: die vorgeschlagene Regel durch `parse_rules` und `serialize_rules`, dann muss sie den gescheiterten `RequestKey` treffen. Ohne diesen Test ist „anwendbar" unbewiesen.
- Nach Anwendung des Vorschlags: die Anfrage wird gehalten statt abgelehnt, und eine zweite Anfrage an dasselbe Ziel wird wieder gehalten. Dieser Test schlägt heute fehl und belegt die Behebung in `pipeline.rs`.
- Ein Test, dass weder der Rumpf der Blockantwort noch eine Kopfzeile die Adresse trägt.
- Mutationsprobe: den `fix` aus dem Befund entfernen, dann wird der erste Test rot.

### Akzeptanzkriterien
- [ ] Jede Ablehnung wegen einer privaten Adresse erzeugt genau einen Befund mit `why` und einer anwendbaren `FixAction::AddRule`.
- [ ] Die vorgeschlagene Regel hält die Anfrage weiterhin an, statt sie dauerhaft freizugeben, und sie wirkt — belegt durch den Test, der heute fehlschlägt.
- [ ] Adresse und Vorschlag erreichen den Agenten nicht.
- [ ] ADR-006 nennt die richtige Nummer.
- [ ] `docs/DIAGNOSTICS.md` kommt aus dem Generatorlauf.
- [ ] `make check`, clippy mit `-D warnings` und `cargo fmt --all -- --check` grün.

### Fallstricke
- `PROXY_004` und `PROXY_006` sind frei. Eine Lücke nachzubelegen macht die Registry schwerer lesbar; die neue Nummer kommt ans Ende des Bereichs.
- `flow.allow_private` vor das `match action` zu ziehen ändert das Verhalten für **jede** Regel mit `allow_private`, nicht nur für die vorgeschlagene. Das ist beabsichtigt, gehört aber in `CONVENTIONS.md` und braucht einen Test, der zeigt, dass eine Regel mit `action: block` und `allow_private: true` weiterhin blockt.

## HUM-103 · Meta-Flüsse fehlen in der Historie
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-073, HUM-026, HUM-031 · Blockiert: —

### Kontext
HUM-073 verlangt in seinem vierten Akzeptanzkriterium, dass Anfragen an `humanitl.internal` in der Historie erscheinen: „Meta-Requests werden im Recorder als Flow mit `state=Recorded`, `decision=meta` gespeichert (sichtbar in History, Filter `meta:true`), nicht gehalten, nicht auditiert außer `/ask`." Umgesetzt ist es nicht, und beide Reviewer kommen unabhängig zu demselben Schluss wie der Umsetzer: innerhalb der Pfade von HUM-073 ist es nicht sauber baubar.

Was fehlt, ist mehr als eine Zeile. Der Zustandsautomat kennt keinen Weg nach `Recorded`, der nicht über eine Sperre führt (ADR-004). `decision=meta` gibt es weder in `Decision` noch in `DecisionKind` der Proto, also auch nicht im Recorder-Schema und nicht im Filter. Und `meta:` ist kein Term der Filtersprache — die drei Stellen, die diese Sprache auslegen, sind Gegenstand von HUM-099.

Ein Fluss mit erfundener Entscheidung wäre keine Lösung: `decision` sagt aus, wie über eine Anfrage entschieden wurde, und über eine Meta-Anfrage entscheidet niemand. Der Umsetzer von HUM-073 hat sie deshalb ausdrücklich nicht angelegt, statt eine Behauptung über einen Menschen zu speichern, der nichts getan hat (`backlog/CONVENTIONS.md` 4.13).

### Ziel
Eine Anfrage an den Meta-Endpunkt erscheint in der Historie als das, was sie ist: eine Auskunft, die der Agent sich geholt hat, ohne dass jemand entschieden hat. Sie ist filterbar, sie ist von einer echten Entscheidung unterscheidbar, und sie verfälscht keine Zählung, die über Entscheidungen spricht.

### Nicht-Ziel
Keine Rümpfe der Meta-Antworten in der Aufzeichnung. Keine Auditierung von `/` und `/why`; `/ask` bleibt auditiert wie bisher. Keine Änderung an der Weiche selbst.

### Betroffene Pfade
- `daemon/crates/core-types/src/flow.rs` und die Proto: die neue Variante
- `daemon/crates/recorder/`: Migration, Spalte, Query
- `daemon/crates/recorder/src/filter.rs` und der Dart-Fake: der Term `meta:`
- `app/lib/features/history/`: Anzeige und Filter
- `tests/fixtures/filter-language.json` aus HUM-099, sofern das bis dahin steht

### Spezifikation
Die neue Variante steht neben den Entscheidungen, nicht unter ihnen. Wo heute über Entscheidungen gezählt oder gefiltert wird (`decision:allow`, `decision:block`, die Zahlen im Demolauf), zählt ein Meta-Fluss nicht mit, außer der Filter nennt ihn ausdrücklich. `meta:true` liefert genau die Meta-Flüsse, `meta:false` genau die übrigen; ohne den Term erscheinen beide, damit die Historie zeigt, was wirklich passiert ist.

Die Zeile in der Historie nennt den Pfad (`/`, `/why/<id>`, `/ask`) und den Statuscode, nicht den Inhalt. Bei `/ask` steht der gesäuberte Text, denn er ist ohnehin schon als Ereignis durch die Oberfläche gegangen.

Der Weg im Zustandsautomaten ist der kleinste, der ohne Lüge auskommt: von der Annahme unmittelbar nach `Recorded`, ohne `Held` und ohne `Decided`. Der ADR-004 wird um diesen Weg ergänzt, mit der Begründung.

### Tests
- Ein Lauf mit je einer Anfrage an `/`, `/why/<id>` und `/ask`: drei Einträge in der Historie, `meta:true` liefert genau sie, `meta:false` keinen davon.
- Ein Test, dass `decision:allow` und `decision:block` keinen Meta-Fluss mitzählen.
- Ein Test, dass der Rumpf einer Meta-Antwort nirgends in der Aufzeichnung landet.
- Mutationsprobe: die neue Variante aus dem Filter entfernen, dann wird der erste Test rot.

### Akzeptanzkriterien
- [ ] Drei Meta-Anfragen erzeugen drei Einträge, unterscheidbar von Entscheidungen.
- [ ] `meta:true` und `meta:false` teilen die Historie vollständig und überschneidungsfrei.
- [ ] Keine Zählung über Entscheidungen ändert sich durch Meta-Flüsse.
- [ ] Akzeptanzkriterium 4 von HUM-073 ist hier abgehakt, und die Notiz dort verweist auf dieses Issue.
- [ ] `make check`, clippy mit `-D warnings` und `cargo fmt --all -- --check` grün.

### Fallstricke
- Der Demolauf M2 zählt bediente Anfragen und Entscheidungen. Kommen Meta-Flüsse hinzu, ohne dass die Zählung sie ausnimmt, wird er rot — und zwar zu Recht; die Zahlen gehören dann angepasst, nicht der Filter.
- `meta:` als Filterterm trifft auf drei Auslegungen derselben Sprache (HUM-099). Wer diesen Term hinzufügt, bevor HUM-099 die gemeinsame Tabelle gebaut hat, fügt ihn dreimal hinzu.

## HUM-104 · Die Durchreiche zum Sprachmodell steht hinter den Regeln des Nutzers
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-037, HUM-039, HUM-066 · Blockiert: HUM-067, HUM-046

### Kontext
`daemon/bin/humanitld/src/main.rs:629-634` schiebt die Durchreichregel in dieselbe Liste wie die mitgelieferten Regeln, mit dem Kommentar: „Die Durchreiche steht vor allem anderen … eine Blockregel des Profils `llm-only` (`host: \"**\"`) stünde sonst davor." Das trifft nicht zu. `State::all()` in `daemon/crates/proxy/src/rules_store.rs:163-170` hängt die mitgelieferten Regeln **hinter** die Sitzungsregeln und die dauerhaften Regeln des Nutzers. Die Durchreiche ist damit die **letzte** Regel des Satzes, nicht die erste.

`RuleSet::prepend_bundled` — die Funktion, die genau diese Reihenfolge herstellen würde — hat außerhalb von Tests **keinen einzigen Aufrufer**. Die vier Fundstellen liegen in `daemon/crates/rules/tests/eval.rs`, `daemon/crates/sandbox/tests/default_rules.rs` und `daemon/crates/config/tests/profiles.rs`. Die grünen Tests messen also Code, den der Daemon nie ausführt.

Zwei Folgen, beide ernst:

1. **Das Profil `llm-only` blockt das Sprachmodell.** `profiles/llm-only.toml:35` bringt `block host "**"` als Regel der Profil-Ebene. Sein eigener Kommentar (Zeilen 8 bis 10) erklärt: „Der Agent-Adapter stellt seine Durchreichregel zum Sprachmodell voran … Sie trifft vor der Blockregel unten, denn die erste passende Regel gewinnt." Nach der tatsächlichen Reihenfolge trifft die Blockregel zuerst. `humanitl run --profile llm-only` — die reine Inferenz-Instanz, die dieser Sprint verspricht — erreicht damit sein eigenes Modell nicht. Heute ist das latent, weil `Profile::rules_document()` (`daemon/crates/config/src/profile.rs:432`) außer in Tests keinen Leser hat und die Verdrahtung erst mit HUM-067 kommt; über eine eigene `rules.yaml` mit `block **` ist es schon jetzt auslösbar.

2. **Der erklärte Seitenkanal wird still zum gewöhnlichen Allow.** `pipeline.rs:350` erkennt die Durchreiche daran, dass **diese** Regel entschieden hat. Trifft vorher eine `allow`-Regel des Nutzers denselben Host, gibt es kein `DecisionSource::Passthrough` und keine Warnung `LLM_005` vor Funden. Der Kanal, den `docs/SECURITY.md` als bewusste, protokollierte Ausnahme beschreibt, verliert genau die Merkmale, die ihn erklärbar machen.

Dazu widersprechen sich drei Stellen im Repository: `daemon/crates/rules/src/eval.rs:394` sagt „die Regeln des Nutzers rücken dahinter (CONVENTIONS 4.5)", `daemon/crates/proxy/src/rules_store.rs:20-24` sagt für dieselbe Nummer „Sitzung, dauerhaft, mitgeliefert", und `docs/profiles.md:171-175` (aus HUM-066) beschreibt „die Durchreichregel des Agent-Adapters, dann seine mitgelieferten Regeln, dann die Dateien und Regeln der Profile, zuletzt die `rules.yaml` des Nutzers". `backlog/CONVENTIONS.md` 4.5 selbst sagt zu den mitgelieferten Regeln nichts.

### Ziel
Es gibt genau eine Auswertungsreihenfolge, sie steht an einer Stelle geschrieben, der Code hält sie ein, und die Durchreiche steht dort, wo beide Kommentare und das Profil sie behaupten.

### Nicht-Ziel
Keine Aufweichung von HUM-027: der Nutzer muss eine mitgelieferte Regel weiterhin überstimmen können. Keine Verdrahtung von `Profile::rules_document()` — das bleibt HUM-067.

### Betroffene Pfade
- `daemon/crates/rules/src/eval.rs`: die Auswertung
- `daemon/crates/proxy/src/rules_store.rs`: `State::all` und der Doc-Kommentar
- `daemon/bin/humanitld/src/main.rs`: die Einordnung der Durchreiche
- `backlog/CONVENTIONS.md` 4.5: die eine Reihenfolge, mit Begründung
- `docs/profiles.md`, `profiles/llm-only.toml`: die Kommentare, die heute Falsches sagen
- `daemon/crates/proxy/tests/`: der Systemtest, der heute fehlt

### Spezifikation
**Erste zu treffende Entscheidung: welche Reihenfolge gilt.** Zwei Wege stehen zur Wahl, und beide sind vertretbar:

1. **Vier Gruppen.** Durchreiche, dann Sitzung, dann Nutzer, dann mitgeliefert. Das ist der kleinste Eingriff: `evaluate` bekommt neben dem vorhandenen Durchgang über `session_scoped` einen weiteren für die Durchreiche. HUM-027 bleibt vollständig erhalten — der Nutzer überstimmt die mitgelieferten Regeln weiter —, und der eine Fall, der wirklich vorn stehen muss, steht vorn.
2. **Mitgeliefert ganz nach vorn**, wie `docs/profiles.md:171` es beschreibt. Dann muss die Ausweichregel aus `rules_store.rs:923` (`immutable_bundled`) `Expiry::Session` tragen statt des `Expiry::Never`, das `Rule::new` setzt — sonst landet sie hinter der mitgelieferten Regel, und der Fix, den `RULES_010` vorschlägt, verspricht etwas Unmögliches. Dasselbe gilt für `overrideBundled` in `app/lib/features/rules/editor.dart`.

Weg 1 ist der empfohlene. Was auch gewählt wird: `RuleSet::prepend_bundled` wird verdrahtet oder entfernt. Eine Funktion, die die richtige Reihenfolge herstellt und nie gerufen wird, ist schlimmer als ihr Fehlen, weil ihre grünen Tests Sicherheit vortäuschen.

### Tests
- Ein Systemtest über den echten Ladeweg (`load_rules` bis `RulesStore::list`), nicht über `prepend_bundled` von Hand: eine Nutzerregel `block host "**"` und eine Durchreiche zum Modell — die Anfrage an das Modell wird durchgereicht, mit `DecisionSource::Passthrough`.
- Derselbe Test mit `allow host "**"`: die Anfrage trägt weiterhin `DecisionSource::Passthrough` und die Warnung `LLM_005` bei einem Fund, statt still als gewöhnliches Allow zu gelten.
- Ein Test, dass eine Nutzerregel eine **mitgelieferte** Regel weiterhin überstimmt (HUM-027 bleibt).
- Mutationsprobe: die Durchreiche zurück ans Ende der Liste, dann werden die ersten beiden Tests rot.

### Akzeptanzkriterien
- [x] Die Entscheidung ist getroffen und in `backlog/CONVENTIONS.md` 4.5 mit Begründung festgehalten; `eval.rs`, `rules_store.rs` und `docs/profiles.md` sagen alle dasselbe.
- [x] Die Durchreiche trifft vor jeder Regel des Nutzers, belegt über den echten Ladeweg.
- [x] Eine Nutzerregel überstimmt weiterhin eine mitgelieferte.
- [x] `RuleSet::prepend_bundled` ist verdrahtet oder entfernt; kein Test misst mehr Code, der im Daemon nicht läuft.
- [x] Der Kommentar in `profiles/llm-only.toml` und der in `main.rs` stimmen mit dem Verhalten überein.
- [x] `make check`, clippy mit `-D warnings` und `cargo fmt --all -- --check` grün.

### Fallstricke
- Der Fall ist heute nur deshalb nicht sichtbar, weil das Profil noch nicht verdrahtet ist. Wer HUM-067 vor diesem Issue baut, bekommt eine Inferenz-Instanz, die ihr eigenes Modell blockt.
- `pipeline.rs:350` erkennt die Durchreiche an der entscheidenden Regel. Jede Lösung, die die Durchreiche nur „meistens" zuerst treffen lässt, lässt auch das Merkmal manchmal verschwinden — und ein Seitenkanal, der manchmal protokolliert wird, ist keiner.

### Ergebnis (2026-09-04)
Gewählt wurde Weg 1, die vier Gruppen. Alle vier Ränge macht `RuleSet::evaluate` über den neuen Typ `Tier`, je ein Durchgang: mitgelieferte Durchreiche, Sitzungsregeln des Nutzers, dauerhafte Regeln des Nutzers, alles übrige Mitgelieferte. Weg 2 wurde verworfen: Er hätte HUM-027 aufgehoben und den stillen Fall nicht gelöst, denn eine Sitzungsregel `allow host "**"` hätte die Durchreiche weiterhin überdeckt. Begründung in `backlog/CONVENTIONS.md` 4.5.

Aus den Reviews kamen zwei Hälften derselben Lücke, beide behoben. Antigravity: Der Rang hing an `passthrough_llm`, und dieses Feld liest `parse_rules` aus der `rules.yaml` des Nutzers und aus den Inline-Regeln eines Profils — eine Datei konnte sich also den Rang selbst ausstellen und ihre eigenen Block-Regeln ungehalten überholen. Codex: Dieselbe Lücke über `bundled`, und dazu ein Loch in HUM-027 — eine mitgelieferte Regel mit `expires: session` fiel in den Sitzungsdurchgang und stand damit vor den dauerhaften Regeln des Nutzers. Ergebnis: Rang 1 verlangt `passthrough_llm` **und** `bundled`; `bundled` gehört dem Lader (`RuleSet::add_bundled`) und wird von `parse_rules` verworfen und mit `RULES_010` gemeldet; `bundled` schlägt `expires`, damit Rang 4 nicht über die Gültigkeit umgangen werden kann. `rules/default.yaml` schreibt den Vermerk deshalb nicht mehr hin. Die Lehre steht als Satz in `CONVENTIONS.md` 4.5.

`RuleSet::prepend_bundled` heißt jetzt `RuleSet::add_bundled`, hängt hinten an statt vorn und wird von `snapshot_of` gerufen — also auf dem Weg, den der Daemon geht.

Tests über den echten Ladeweg: `load_rules_evaluates_the_passthrough_before_every_rule_of_the_user` und `load_rules_lets_a_user_rule_override_a_bundled_one` in `daemon/bin/humanitld/src/main.rs`, dazu `daemon/crates/proxy/tests/rules_order.rs` (fünf Fälle über `RulesStore::load` und die Proxy-Pipeline, mit `DecisionSource::Passthrough`, `LLM_005` und der Aufzeichnung). Für die Herkunft: `a_passthrough_written_into_rules_yaml_does_not_outrank_the_user` (Speicher), `a_file_cannot_declare_a_rule_bundled` und `a_passthrough_from_a_file_does_not_reach_the_first_rank` (Engine), `a_profile_cannot_declare_its_own_rule_bundled` (globales Profil), `a_session_scoped_bundled_rule_does_not_outrank_the_user` (HUM-027 gegen die Gültigkeit).
