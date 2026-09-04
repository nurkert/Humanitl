# Der mitgelieferte OpenCode-Beitrag

Dieses Verzeichnis enthält alles, was der `OpenCodeAdapter`
(`daemon/crates/sandbox/src/agent/opencode.rs`) in die Sandbox schreibt. Die
Dateien werden mit `include_str!` einkompiliert; zur Laufzeit wird nichts von
hier gelesen und nichts nachgeladen.

| Datei | Wohin in der Sandbox | Wofür |
|---|---|---|
| `opencode.json.tmpl` | `/etc/humanitl/opencode/opencode.json`, `~/.config/opencode/opencode.json` und `/etc/opencode/opencode.json` | Provider, Modell und Berechtigungen des Agenten |
| `models.json` | `/etc/humanitl/opencode/models.json` | Ersatz für den Modellkatalog, den OpenCode sonst aus dem Netz holt |
| `briefing.en.md`, `briefing.de.md` | `~/.config/opencode/AGENTS.md` | Die Einweisung des Agenten, in der Sprache aus `ui.language` |
| `update-models.sh` | — | Entwickler-Werkzeug, das das Schema-Skelett gegen die echte Quelle abgleicht |

## Warum eine gebündelte `models.json`

OpenCode lädt beim Start einen Modellkatalog. In der Fassung 1.18.25, die auf
diesem Rechner unter `~/.opencode/bin/opencode` liegt, ist die Vorgabe
`https://models.opencode.ai`, an die OpenCode `/api.json` anhängt; die
Dokumentation nennt `https://models.dev/api.json`. Andere Fassungen wurden nicht
geprüft. Beides ist ein Netzabruf, den niemand ausgelöst hat, und der erste
gehaltene Fluss, den ein Mensch zu sehen bekäme, wäre damit einer, den er nicht
versteht (BACKLOG.md Abschnitt 5).

Der Adapter setzt deshalb zwei Umgebungsvariablen:

- `OPENCODE_MODELS_PATH=/etc/humanitl/opencode/models.json` — OpenCode liest den
  Katalog aus dieser Datei statt aus dem Netz.
- `OPENCODE_DISABLE_MODELS_FETCH=true` — die Hintergrundaktualisierung, die
  sonst stündlich läuft, unterbleibt.

`OPENCODE_MODELS_URL` taugt dafür nicht: der Wert ist eine Basis-Adresse, an die
OpenCode `/api.json` anhängt und die es über seinen HTTP-Client abruft. Ein
`file://`-Schema kommt dort nicht an. Das ist der Fallstrick 1 aus
`backlog/sprint-3.md`, HUM-037; die zweite Bridge auf Port 3129, die dort als
Ausweg beschrieben ist, wird nicht gebraucht.

## Woher der Inhalt stammt und unter welcher Lizenz er steht

`models.json` enthält **keine** Daten von `models.dev` oder
`models.opencode.ai`. Die Datei beschreibt genau einen Provider,
`humanitl-local`, nämlich den LLM-Server, den der Nutzer selbst in
`llm.endpoint` einträgt. Übernommen ist allein die Form: welche Felder ein
Provider- und ein Modell-Eintrag hat und welchen Typ sie haben. Eine
Datenstruktur ist keine schützbare Leistung, und damit stellt sich die Frage
nach der Vereinbarkeit einer fremden Lizenz mit `GPL-3.0-only` gar nicht erst.
Der Inhalt dieser Datei steht unter derselben Lizenz wie das übrige Repository.

Zwei Werte darin sind Platzhalter und keine Messung:

- `release_date` steht auf `1970-01-01`. Das Erscheinungsdatum eines lokal
  betriebenen Modells kennt Humanitl nicht, und ein plausibel aussehendes Datum
  wäre eine Behauptung ohne Beleg (`backlog/CONVENTIONS.md` 4.13).
- `limit.context` und `limit.output` sind vorsichtig gewählt (32768 und 4096).
  OpenCode entscheidet daran, wann es eine Sitzung zusammenfasst. Zu große
  Werte lässt der Server auflaufen, zu kleine falten die Sitzung zu früh
  zusammen. Wer sein Modell kennt, ändert die Werte hier; ein späteres Issue
  (HUM-076) holt sie beim Endpoint-Test ab.

## Drei Orte für dieselbe Konfiguration

`OpenCode` führt seine Konfigurationsquellen in dieser Reihenfolge zusammen,
spätere gewinnen: Konfigurationsverzeichnis, `OPENCODE_CONFIG`, **dann** die
`opencode.json` des Projektbaums, `.opencode`-Verzeichnisse,
`OPENCODE_CONFIG_CONTENT`, die Konfiguration einer angemeldeten Organisation,
zuletzt das verwaltete Verzeichnis (unter Linux `/etc/opencode`). Danach wird
`OPENCODE_PERMISSION` über den Block `permission` gelegt.

Ein geklontes Repository steht damit über `OPENCODE_CONFIG`. Nur
`/etc/opencode/opencode.json` und `OPENCODE_PERMISSION` stehen darüber, und
deshalb legt der Adapter dieselbe Datei an drei Orte und setzt zusätzlich die
Variable. Gemessen an 1.18.25 mit `opencode debug config`.

Nicht durchsetzbar ist `provider`: der Zusammenführungsschritt ist additiv. Ein
Projekt kann einen eigenen Provider mit eigener Adresse hinzufügen, und
`opencode models` zeigt ihn dann. Es fällt dabei keine Garantie, weil jeder
Verkehr dorthin durch den Proxy geht und gehalten wird; welches Modell
voreingestellt ist, bestimmt weiterhin Humanitl.

## Wie der Adapter die Vorlagen füllt

Beide Dateien sind gültiges JSON und tragen ihre Platzhalter als Werte, nicht
als Textfragmente. Der Adapter liest sie mit `serde_json` als `Value` und setzt
die Felder; er ersetzt keine Zeichenketten. Anders ginge es nicht: `llm.endpoint`
und die Modellnamen kommen aus der Konfiguration, und ein Modellname mit einem
Anführungszeichen darf keine fremde Struktur in die Datei schreiben können
(HUM-037, Fallstrick 5).

- `opencode.json.tmpl`: `{{LLM_BASE_URL}}` wird zu `llm.endpoint` plus `/v1`
  (falls der Endpoint nicht schon auf `/v1` endet), `{{DEFAULT_MODEL}}` zum
  ersten Modell aus `llm.models`, und `provider["humanitl-local"].models` zu
  einem Objekt mit einem Eintrag je Modell.
- `models.json`: der Eintrag unter dem Schlüssel `{{MODEL_ID}}` ist die Vorlage
  für einen Modelleintrag. Der Adapter legt sie einmal je konfiguriertem Modell
  an und setzt `id` und `name`.

Ist kein Modell konfiguriert, trägt der Adapter das Platzhalter-Modell `default`
ein und meldet `LLM_004` als Warnung.

## Die Einweisung des Agenten

`briefing.{en,de}.md` sind der einzige Text, den Humanitl dem Agenten mitgibt
(ADR-0014, HUM-071). Er landet als `~/.config/opencode/AGENTS.md` in der
Sandbox und **nie** unter `/work`: OpenCode liest die `AGENTS.md` des Projekts
zusätzlich, und eine Datei, die Humanitl dort ablegte, stünde im Diff des
Nutzers und irgendwann in einem fremden Repository. `agent.briefing.enabled =
false` lässt die Datei ganz weg.

Gemessen an OpenCode 1.18.25 setzt `InstructionContext` seine Liste aus
`join(<Konfigurationsverzeichnis>, "AGENTS.md")` und danach den `AGENTS.md` des
Projektbaums zusammen; die globale Datei steht also vorn und wird auch bei
gesetztem `OPENCODE_DISABLE_PROJECT_CONFIG` gelesen.

Welches Verzeichnis das ist, entscheidet die Umgebung, die der Agent wirklich
sieht, nicht das Profil: `sandbox.env` wird **nach** dem Beitrag des Adapters
gesetzt und gewinnt. Setzt jemand dort `HOME` oder `XDG_CONFIG_HOME`, folgen
die Dateien dorthin (`AgentContext::config_home`). Täten sie es nicht, läge die
Einweisung an einem Ort, den niemand liest, und nichts fiele auf.

Für den Wortlaut gelten zwei Regeln, und beide sind der Grund, warum der Text
in Dateien und nicht im Quelltext steht:

1. **Jede Aussage ist am Verhalten geprüft, nicht an der Spezifikation.** Der
   Agent behandelt den Text als Wahrheit. Steht dort eine Frist, die es nicht
   gibt, bricht er zu früh ab; steht dort ein Endpunkt, der nicht antwortet,
   erzeugt er genau die Schleife, gegen die der Text geschrieben ist. Die
   Belege stehen im Modulkommentar von
   `daemon/crates/sandbox/src/agent/briefing.rs`.
2. **Der Text bleibt kurz.** Jedes Token fehlt dem Agenten für seine Arbeit.
   Gezählt wird mit `python3 tools/briefing-tokens.py`, im ungünstigsten Fall
   (längerer Ask-Block, langer Endpunkt, vierstellige Frist) und mit
   `o200k_base`. Die geltende Grenze ist `TOKEN_BUDGET` in
   `daemon/crates/sandbox/src/agent/briefing.rs`; dort steht auch, warum sie
   über den Schätzungen von ADR-0014 und HUM-071 liegt.

Was der Text sagt, steht in der Tabelle im Modulkommentar von `briefing.rs`,
Aussage für Aussage mit dem Beleg im Code — einschließlich des Meta-Endpunkts
`http://humanitl.internal/` (HUM-073) und seiner Grenze: `POST /ask` legt dem
Nutzer eine Bitte vor und legt nie eine Regel an.

Drei Platzhalter werden beim Start ersetzt: `{ask_mode}` durch den Block, der
zu `hold.ask_mode` passt, `{timeout}` durch `hold.timeout_secs` und
`{llm_host}` durch Host und Port der Durchreiche. Der Host kommt aus
`passthrough_authority()`, also aus derselben geprüften Quelle wie die
Durchreichregel; entsteht dort keine Regel, fällt die ganze Zeile weg.
HTML-Kommentare entfernt der Renderer; ein Kommentar endet dabei am ersten
`-->`, es darf deshalb keines im Text eines Kommentars stehen.

## Wie `models.json` erneuert wird

`./update-models.sh` gleicht das Skelett gegen die echte Quelle ab. Das Skript
läuft auf dem Rechner des Entwicklers, nie in der Sandbox und nie zur Laufzeit
des Daemons. Es lädt `https://models.opencode.ai/api.json` (mit `--url` auch
`https://models.dev/api.json`), nimmt sich einen beliebigen Provider-Eintrag
daraus, sammelt die Feldnamen von Provider und Modell ein und vergleicht sie mit
denen in `models.json`. Fehlt ein Pflichtfeld oder ist eines dazugekommen, sagt
das Skript, welches; die Datei wird nicht automatisch überschrieben, weil ihr
Inhalt eine Entscheidung ist und keine Kopie.

```sh
cd agents/opencode
./update-models.sh                 # gegen models.opencode.ai
./update-models.sh --url https://models.dev   # gegen models.dev
./update-models.sh --offline       # nur die eigene Datei prüfen, ohne Netz
```

Die verbindliche Liste der Pflichtfelder steht im Skript und stammt aus dem
Schema der installierten OpenCode-Fassung
(`Provider = { api?, name, env, id, npm?, models }`,
`Model = { id, name, family?, release_date, attachment, reasoning, temperature,
tool_call, cost?, limit{context, input?, output}, modalities?, status?, ... }`).
