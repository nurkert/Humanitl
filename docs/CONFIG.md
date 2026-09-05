# Konfiguration

<!-- Erzeugt aus daemon/crates/config/src/model.rs.
     Nicht von Hand ändern: `UPDATE_CONFIG_DOCS=1 cargo test -p humanitl-config --test config_docs` schreibt die Datei neu. -->

Humanitl hat eine Konfigurationsquelle: die Rust-Typen in `humanitl-config`. Aus ihnen
entstehen das JSON-Schema, die Prüfung beim Laden, der Einstellungs-Bildschirm und diese
Seite. Wer ein Feld ändern will, ändert den Typ.

## Sichtbarkeitsstufen

| Stufe | Wo sie erscheint |
|---|---|
| `basic` | Immer sichtbar, im ersten Bild der Einstellungen. |
| `advanced` | Hinter „Mehr anzeigen". |
| `expert` | Nur in `config.toml` und hier. |

## Projekt-Profil

`<projekt>/.humanitl/profile.toml` liegt im geklonten Repository und ist damit
Angreifer-beeinflusst: Wer ein Repository klont, führt dessen Profil aus. Die Spalte
„Projekt" sagt für jedes Feld, ob das Projekt-Profil es setzen darf (im JSON-Schema
`x-project-scope`).

| Projekt | Bedeutung |
|---|---|
| `allowed` | Das Projekt-Profil darf den Wert setzen. |
| `denied` | Nur Vorgabewerte, `config.toml`, globales Profil, Umgebung oder Kommandozeile dürfen den Wert setzen. Im Projekt-Profil ist der Schlüssel `CONFIG_003`, auch unter einem alten Namen. |

Zwei Werte werden beim Laden über ihren Typ hinaus geprüft: `llm.endpoint` nimmt nur
`http` oder `https`; `sandbox.work_dir` muss absolut sein, ohne `..`, und ein
existierendes Verzeichnis, das sich kanonisieren lässt. Beides sonst `CONFIG_003`.

## Wirkung

Diese Seite entsteht aus dem Schema und kann deshalb keinen Schlüssel vergessen. Ob ein
Schlüssel etwas bewirkt, steht dem Schema nicht an: Ein Wert, der beschrieben und geprüft
wird und den niemand liest, sähe hier aus wie jeder andere. Die Spalte „Wirkung" sagt es
deshalb ausdrücklich (im JSON-Schema `x-pending-issue`, HUM-101).

| Wirkung | Bedeutung |
|---|---|
| `ja` | Der Schlüssel wird gelesen und wirkt. |
| `offen (HUM-xxx)` | Der Schlüssel hat heute keinen Leser. Das genannte Issue entscheidet ihn, durch Einbau oder durch Streichung; bis dahin ändert sein Wert nichts. Das Register `daemon/crates/config/tests/config_readers.rs` hält für jeden Schlüssel fest, welcher der beiden Fälle gilt. |

## Reihenfolge der Quellen

Von unten nach oben; die obere Ebene gewinnt. Jedes Feld merkt sich, aus welcher Ebene
sein Wert stammt, und die Oberfläche zeigt es an.

| Ebene | Quelle |
|---|---|
| 1 | eingebaute Vorgabewerte |
| 2 | `$XDG_CONFIG_HOME/humanitl/config.toml` |
| 3 | Profil `default`: `$XDG_CONFIG_HOME/humanitl/profiles/default.toml`, sonst die eingebettete Fassung |
| 4 | das gewählte Profil, falls es nicht `default` ist; Datei, sonst eingebettet |
| 5 | `<projekt>/.humanitl/profile.toml`, Block `[config]`; nur Felder mit Projekt `allowed` |
| 6 | Umgebungsvariablen `HUMANITL_*` |
| 7 | Argumente der Kommandozeile |

Ein Profil hat neben `[config]` nur `name`, `description` und `[rules]` (HUM-066,
`docs/profiles.md`). Jeder andere Block auf der obersten Ebene ist `CONFIG_002`; eine
Gruppe wie `[hold]` gehört im Profil unter `[config.hold]`. Das mitgelieferte Profil
`default` setzt mit Absicht keinen Wert: es liegt über `config.toml` und machte sie
sonst für jeden Schlüssel wirkungslos, den es nennt.

`<projekt>` ist `sandbox.work_dir`, sonst das aktuelle Verzeichnis — nicht umgekehrt:
Wer mit `--work` aus einem fremden Verzeichnis heraus arbeitet, bekommt das Profil des
Projekts, an dem er arbeitet. Der Schlüssel ist auf der Projekt-Ebene gesperrt, deshalb
kann das Projekt-Profil nicht bestimmen, wo nach ihm gesucht wird. Sein `name` wählt nur
unter den mitgelieferten Profilen; jeder andere Wunsch wird übergangen und mit
`CONFIG_009` gemeldet (`docs/profiles.md`).

Eine Umgebungsvariable heißt wie ihr Pfad in Großbuchstaben, mit `__` zwischen den
Ebenen: `hold.timeout_secs` wird zu `HUMANITL_HOLD__TIMEOUT_SECS`. Der Wert wird nach dem
Typ des Feldes gelesen: für ein Textfeld bleibt er Text, sonst wird er als Wahrheitswert,
dann als Zahl, sonst als Zeichenkette gelesen; eine Liste steht in eckigen Klammern.
Ein unbekannter Schlüssel in einer Datei oder auf der Kommandozeile ist ein Fehler
(`CONFIG_002`), in der Umgebung eine Warnung, die das Laden nicht abbricht. Variablen
ohne `__` im Namen (`HUMANITL_GALLERY`, `HUMANITL_ESCAPE_MARKER`) gehören anderen
Werkzeugen und werden übergangen.

## Felder

### `agent`

Welcher Agent in der Sandbox läuft.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `agent.adapter` | string | `"opencode"` | advanced | denied | ja | Kennung des Adapters, zum Beispiel `opencode`. |
| `agent.briefing.enabled` | boolean | `true` | advanced | allowed | ja | Legt die Instruktionsdatei des Agenten in der Sandbox an. |
| `agent.command` | list of string, optional | `-` | expert | denied | ja | Ersetzt die Kommandozeile des Adapters vollständig. Leer bedeutet: die des Adapters. |

### `experimental`

Schalter für unfertige Wege. Alles hier darf ohne Ankündigung wegfallen.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `experimental.h2_upstream` | boolean | `false` | expert | denied | ja | Bietet dem Ziel HTTP/2 an. In M1 spricht der Proxy nach oben nur HTTP/1.1. |
| `experimental.upstream_port_map` | table of integer | `{}` | expert | denied | offen (HUM-088) | Lenkt einen Zielport auf einen anderen um, Schlüssel und Wert als Portnummer. Nur für Tests. |
| `experimental.ws_hold` | boolean | `false` | expert | denied | offen (HUM-121) | Hält auch WebSocket-Upgrades an, statt sie über eine Regel zu entscheiden. |

### `findings`

Erkennung von Geheimnissen und persönlichen Daten.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `findings.email_allow_domains` | list of string | `[]` | advanced | denied | ja | Domains, deren Mailadressen kein Fund sind, zum Beispiel die eigene Firma. |
| `findings.enabled` | boolean | `true` | advanced | denied | ja | Schaltet die Erkennung ganz ab. Aus bedeutet: keine Markierungen, keine Pseudonyme. |
| `findings.ignored_hashes` | list of string | `[]` | expert | denied | ja | Prüfsummen (SHA-256, hex) einzelner Werte, die nie wieder als Fund erscheinen. |
| `findings.user_terms` | list of string | `[]` | basic | allowed | ja | Eigene Begriffe, die als Fund gelten, zum Beispiel ein Projektname oder ein Kundenname. |

### `hold`

Wie lange und auf welchem Weg gefragt wird, bevor eine Anfrage weiterläuft.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `hold.ask_mode` | ui \| terminal \| none | `"ui"` | advanced | denied | ja | Wo gefragt wird: in der Oberfläche, im Terminal oder gar nicht. |
| `hold.hard_block_checksum_secrets` | boolean | `false` | advanced | allowed | ja | Blockt Anfragen mit prüfsummen-sicheren Geheimnissen sofort, ohne zu fragen. |
| `hold.timeout_secs` | integer | `300` | basic | allowed | ja | Sekunden, die eine angehaltene Anfrage auf eine Entscheidung wartet, bevor sie als Zeitüberschreitung endet. |

### `limits`

Alle Caps und Zeitgrenzen an einer Stelle.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `limits.body_timeout_secs` | integer | `300` | expert | allowed | ja | Sekunden Stille zwischen zwei Stücken eines Bodys, nicht seine Gesamtdauer. Gilt in beide Richtungen: für den Anfrage-Rumpf des Clients (danach 408) und für den gestreamten Antwort-Rumpf des Ziels (danach wird der Strom abgebrochen und die Aufzeichnung als gekürzt vermerkt). Ein großer Upload und ein langer Modell-Strom dürfen deshalb beliebig lange dauern, solange sie nicht verstummen. |
| `limits.connect_timeout_secs` | integer | `10` | advanced | allowed | ja | Sekunden bis zum Aufbau der Verbindung zum Ziel. |
| `limits.event_buffer` | integer | `1024` | expert | allowed | ja | Länge der Ereignis-Warteschlange je Client. Läuft sie über, meldet der Daemon `Lagged`. |
| `limits.header_timeout_secs` | integer | `30` | expert | allowed | ja | Sekunden, in denen der Client seine Anfrage-Kopfzeilen gesendet haben muss. Auf einer Keep-Alive-Verbindung ist das zugleich die Frist bis zur nächsten Anfrage, also die einzige Leerlaufgrenze der Verbindung zum Agenten; während eine Anfrage gehalten wird, läuft sie nicht. |
| `limits.hold_body_cap_bytes` | integer | `33554432` | advanced | allowed | ja | Größte Anfrage, deren Body für die Entscheidung im Speicher gehalten wird. Darüber antwortet der Proxy mit 413. |
| `limits.hold_max_bytes` | integer | `268435456` | advanced | allowed | ja | Größte Summe der Bodies aller angehaltenen Flows. Darüber antwortet der Proxy mit 503. |
| `limits.hold_max_flows` | integer | `200` | advanced | allowed | ja | Größte Zahl gleichzeitig angehaltener Flows. Darüber antwortet der Proxy mit 503. |
| `limits.max_client_connections` | integer | `256` | expert | allowed | ja | Größte Zahl gleichzeitiger Verbindungen aus der Sandbox je Sitzung. Darüber antwortet der Proxy mit 503 und schließt; eine Uhr je Spanne allein hindert einen Prozess nicht daran, dieselben Ressourcen über viele Verbindungen zu binden. |
| `limits.max_decompress_ratio` | integer | `100` | expert | allowed | ja | Höchstes erlaubtes Verhältnis von entpackten zu gepackten Bytes einer Vorschau. |
| `limits.preview_cap_bytes` | integer | `8388608` | expert | allowed | ja | Größte Menge Body, die die Oberfläche als Vorschau bekommt. |
| `limits.recorder_max_body_bytes` | integer | `33554432` | expert | allowed | ja | Größter Body, den die Aufzeichnung als Blob ablegt. Alles darüber wird nur mit Prüfsumme vermerkt. |

### `llm`

Der lokale LLM-Endpunkt und was als Passthrough gilt.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `llm.endpoint` | string, optional | `-` | basic | denied | ja | OpenAI-kompatibler Endpunkt im LAN. Verkehr dorthin wird nicht angehalten, aber protokolliert. |
| `llm.models` | list of string | `[]` | basic | denied | ja | Modelle, die der Endpunkt anbietet. Leer heißt: der Agent bekommt ein Platzhalter-Modell und eine Warnung. |
| `llm.passthrough_paths` | list of string | `["/v1/","/api/"]` | advanced | denied | ja | Pfadpräfixe, die als LLM-Passthrough gelten. Ein Präfix soll einen Endpunkt benennen, keine ganze API-Fläche: Der Agent-Adapter ersetzt `/v1/` und `/api/` deshalb durch die Endpunkte, die Inferenz machen, damit `POST /api/pull` und `POST /v1/files` nicht ungefragt hinausgehen. Ein Pfad, der mehr nennt, bleibt stehen, wie er hier steht. |

### `pseudonyms`

Rücktausch von Pseudonymen in Antworten.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `pseudonyms.max_response_bytes` | integer | `8388608` | expert | denied | offen (HUM-079) | Größte Antwort, die für den Rücktausch gepuffert wird. Alles darüber läuft unverändert durch. |
| `pseudonyms.translate_responses` | boolean | `true` | advanced | denied | offen (HUM-079) | Ersetzt Pseudonyme in Text-Antworten wieder durch den Originalwert. |

### `recorder`

Aufzeichnung der Flows.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `recorder.inline_max_bytes` | integer | `262144` | expert | allowed | ja | Bodies bis zu dieser Größe stehen in der Datenbank, größere als Datei im Blob-Speicher. |
| `recorder.retention_days` | integer | `90` | advanced | denied | ja | Tage, die eine Aufzeichnung aufgehoben wird. |

### `resolver`

Namensauflösung nach der Entscheidung.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `resolver.cache_ttl_secs` | integer | `300` | expert | denied | ja | Sekunden, die eine Antwort im Zwischenspeicher bleibt. |
| `resolver.nameserver` | string, optional | `-` | expert | denied | offen (HUM-115) | Nameserver als `IP:Port`. Leer bedeutet: die Einstellung des Systems. |
| `resolver.overrides` | table of string | `{}` | expert | denied | ja | Feste Zuordnungen von Hostname zu Adresse, vor jeder Abfrage. |
| `resolver.prefer` | ipv4 \| ipv6 | `"ipv4"` | expert | denied | ja | Welche Adressfamilie bevorzugt wird, wenn beide vorliegen. |
| `resolver.test_ca` | string, optional | `-` | expert | denied | offen (HUM-087) | Zusätzliche CA für Tests. Nur in Testläufen setzen, nie im Alltag. |

### `sandbox`

Welches Sandbox-Profil mit welchem Arbeitsverzeichnis startet.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `sandbox.env` | table of string | `{}` | advanced | denied | ja | Zusätzliche Umgebungsvariablen für die Sandbox; sie überschreiben gleichnamige Einträge aus dem `[env]` des Profils. Der Schlüssel lässt sich aus der Umgebung des Prozesses setzen und ist damit nur so vertrauenswürdig wie die Shell, aus der Humanitl startet; die Variablen des dynamischen Linkers (`LD_PRELOAD`, `LD_AUDIT`, `LD_LIBRARY_PATH`) werden deshalb abgelehnt, sie liefen vor dem seccomp-Filter des Shims. |
| `sandbox.profile` | string | `"default"` | advanced | denied | ja | Name des Profils unter `profiles/sandbox/`, ohne Endung. |
| `sandbox.work_dir` | string, optional | `-` | basic | denied | ja | Projektverzeichnis, das als `/work` eingehängt wird. Leer bedeutet: das aktuelle Verzeichnis. |
| `sandbox.work_mode` | ro \| rw | `"rw"` | basic | denied | ja | Ob der Agent im Projektverzeichnis schreiben darf. |

### `ui`

Sprache, Erscheinungsbild und Meldungen der Oberfläche.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `ui.language` | en \| de | `"en"` | basic | allowed | ja | Sprache der Oberfläche. |
| `ui.notifications` | boolean | `true` | advanced | allowed | offen (HUM-069) | Meldung des Systems, wenn eine Anfrage wartet und das Fenster nicht vorn ist. |
| `ui.sound` | boolean | `false` | advanced | allowed | offen (HUM-121) | Ton zur Meldung. |
| `ui.theme` | dark \| light \| system | `"dark"` | advanced | allowed | offen (HUM-069) | Erscheinungsbild der Oberfläche. |

## Alte Namen

Diese Schlüssel funktionieren weiter. Steht der alte neben dem heutigen Namen, gewinnt der
heutige, und das Laden legt einen Befund dazu.

| Alt | Heute | Seit |
|---|---|---|
| `hold.body_cap_bytes` | `limits.hold_body_cap_bytes` | HUM-057 |
| `ipc.event_buffer` | `limits.event_buffer` | HUM-057 |
| `preview.cap_bytes` | `limits.preview_cap_bytes` | HUM-057 |
| `preview.max_decompress_ratio` | `limits.max_decompress_ratio` | HUM-057 |
| `recorder.max_body_bytes` | `limits.recorder_max_body_bytes` | HUM-057 |
| `upstream.connect_timeout_secs` | `limits.connect_timeout_secs` | HUM-057 |

## Entfallene Schlüssel

Diese Schlüssel gab es einmal und gibt es nicht mehr. Sie haben keinen Nachfolger. Wer
sie noch in einer Datei stehen hat, bekommt beim Laden eine Warnung (`CONFIG_005`) mit
dem Issue und dem Grund; der Wert wird übergangen, und der Daemon startet trotzdem. Ein
harter Fehler wäre hier die Strafe für eine Entscheidung, die nicht der Nutzer getroffen
hat (`backlog/CONVENTIONS.md` 4.25).

| Schlüssel | Entfallen mit | Grund (Text des Befunds) |
|---|---|---|
| `limits.idle_timeout_secs` | HUM-101 | it described the same span as limits.header_timeout_secs, the one idle clock of the connection to the agent |

## Pfade

| Was | Wo |
|---|---|
| Konfiguration | `$XDG_CONFIG_HOME/humanitl/config.toml` |
| Regeln | `$XDG_CONFIG_HOME/humanitl/rules.yaml` |
| Profile | `$XDG_CONFIG_HOME/humanitl/profiles/<name>.toml` |
| Projekt-Profil | `<projekt>/.humanitl/profile.toml` |
| Datenbank | `$XDG_DATA_HOME/humanitl/humanitl.db` |
| Blobs | `$XDG_DATA_HOME/humanitl/blobs/<hex[0..2]>/<sha256-hex>` |
| Audit | `$XDG_DATA_HOME/humanitl/audit/audit.jsonl` |
| CA | `$XDG_DATA_HOME/humanitl/ca/ca.crt`, `ca.key` (0600) |
| Daemon-Socket | `$XDG_RUNTIME_DIR/humanitl/daemon.sock` (0600, Verzeichnis 0700) |
| Proxy-Socket | `$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock` |
| Token | `$XDG_RUNTIME_DIR/humanitl/token` (0600) |

Fehlt `XDG_RUNTIME_DIR`, wird `/run/user/<uid>` benutzt; fehlt auch das, weicht Humanitl
auf `$TMPDIR/humanitl-<uid>` aus und meldet `CONFIG_004` als Hinweis.
