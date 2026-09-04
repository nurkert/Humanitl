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

## Reihenfolge der Quellen

Von unten nach oben; die obere Ebene gewinnt. Jedes Feld merkt sich, aus welcher Ebene
sein Wert stammt, und die Oberfläche zeigt es an.

| Ebene | Quelle |
|---|---|
| 1 | eingebaute Vorgabewerte |
| 2 | `$XDG_CONFIG_HOME/humanitl/config.toml` |
| 3 | `$XDG_CONFIG_HOME/humanitl/profiles/<name>.toml`, Block `[config]` |
| 4 | `<projekt>/.humanitl/profile.toml`, Block `[config]`; nur Felder mit Projekt `allowed` |
| 5 | Umgebungsvariablen `HUMANITL_*` |
| 6 | Argumente der Kommandozeile |

Ein Profil hat neben `[config]` nur `name`, `description`, `[rules]` und `[agent]`
(HUM-066). Jeder andere Block auf der obersten Ebene ist `CONFIG_002`; eine Gruppe wie
`[hold]` gehört im Profil unter `[config.hold]`.

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

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Beschreibung |
|---|---|---|---|---|---|
| `agent.adapter` | string | `"opencode"` | advanced | denied | Kennung des Adapters, zum Beispiel `opencode`. |
| `agent.briefing.enabled` | boolean | `true` | advanced | allowed | Legt die Instruktionsdatei des Agenten in der Sandbox an. |
| `agent.command` | list of string, optional | `-` | expert | denied | Ersetzt die Kommandozeile des Adapters vollständig. Leer bedeutet: die des Adapters. |

### `experimental`

Schalter für unfertige Wege. Alles hier darf ohne Ankündigung wegfallen.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Beschreibung |
|---|---|---|---|---|---|
| `experimental.h2_upstream` | boolean | `false` | expert | denied | Bietet dem Ziel HTTP/2 an. In M1 spricht der Proxy nach oben nur HTTP/1.1. |
| `experimental.upstream_port_map` | table of integer | `{}` | expert | denied | Lenkt einen Zielport auf einen anderen um, Schlüssel und Wert als Portnummer. Nur für Tests. |
| `experimental.ws_hold` | boolean | `false` | expert | denied | Hält auch WebSocket-Upgrades an, statt sie über eine Regel zu entscheiden. |

### `findings`

Erkennung von Geheimnissen und persönlichen Daten.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Beschreibung |
|---|---|---|---|---|---|
| `findings.email_allow_domains` | list of string | `[]` | advanced | denied | Domains, deren Mailadressen kein Fund sind, zum Beispiel die eigene Firma. |
| `findings.enabled` | boolean | `true` | advanced | denied | Schaltet die Erkennung ganz ab. Aus bedeutet: keine Markierungen, keine Pseudonyme. |
| `findings.ignored_hashes` | list of string | `[]` | expert | denied | Prüfsummen (SHA-256, hex) einzelner Werte, die nie wieder als Fund erscheinen. |
| `findings.user_terms` | list of string | `[]` | basic | allowed | Eigene Begriffe, die als Fund gelten, zum Beispiel ein Projektname oder ein Kundenname. |

### `hold`

Wie lange und auf welchem Weg gefragt wird, bevor eine Anfrage weiterläuft.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Beschreibung |
|---|---|---|---|---|---|
| `hold.ask_mode` | ui \| terminal \| none | `"ui"` | advanced | denied | Wo gefragt wird: in der Oberfläche, im Terminal oder gar nicht. |
| `hold.hard_block_checksum_secrets` | boolean | `false` | advanced | allowed | Blockt Anfragen mit prüfsummen-sicheren Geheimnissen sofort, ohne zu fragen. |
| `hold.timeout_secs` | integer | `300` | basic | allowed | Sekunden, die eine angehaltene Anfrage auf eine Entscheidung wartet, bevor sie als Zeitüberschreitung endet. |

### `limits`

Alle Caps und Zeitgrenzen an einer Stelle.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Beschreibung |
|---|---|---|---|---|---|
| `limits.body_timeout_secs` | integer | `300` | expert | allowed | Sekunden, in denen ein Body vollständig übertragen sein muss. |
| `limits.connect_timeout_secs` | integer | `10` | advanced | allowed | Sekunden bis zum Aufbau der Verbindung zum Ziel. |
| `limits.event_buffer` | integer | `1024` | expert | allowed | Länge der Ereignis-Warteschlange je Client. Läuft sie über, meldet der Daemon `Lagged`. |
| `limits.header_timeout_secs` | integer | `30` | expert | allowed | Sekunden, in denen der Client seine Anfrage-Kopfzeilen gesendet haben muss. |
| `limits.hold_body_cap_bytes` | integer | `33554432` | advanced | allowed | Größte Anfrage, deren Body für die Entscheidung im Speicher gehalten wird. Darüber antwortet der Proxy mit 413. |
| `limits.hold_max_bytes` | integer | `268435456` | advanced | allowed | Größte Summe der Bodies aller angehaltenen Flows. Darüber antwortet der Proxy mit 503. |
| `limits.hold_max_flows` | integer | `200` | advanced | allowed | Größte Zahl gleichzeitig angehaltener Flows. Darüber antwortet der Proxy mit 503. |
| `limits.idle_timeout_secs` | integer | `90` | expert | allowed | Sekunden ohne Bytes, nach denen eine offene Verbindung geschlossen wird. |
| `limits.max_decompress_ratio` | integer | `100` | expert | allowed | Höchstes erlaubtes Verhältnis von entpackten zu gepackten Bytes einer Vorschau. |
| `limits.preview_cap_bytes` | integer | `8388608` | expert | allowed | Größte Menge Body, die die Oberfläche als Vorschau bekommt. |
| `limits.recorder_max_body_bytes` | integer | `33554432` | expert | allowed | Größter Body, den die Aufzeichnung als Blob ablegt. Alles darüber wird nur mit Prüfsumme vermerkt. |

### `llm`

Der lokale LLM-Endpunkt und was als Passthrough gilt.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Beschreibung |
|---|---|---|---|---|---|
| `llm.endpoint` | string, optional | `-` | basic | denied | OpenAI-kompatibler Endpunkt im LAN. Verkehr dorthin wird nicht angehalten, aber protokolliert. |
| `llm.models` | list of string | `[]` | basic | denied | Modelle, die der Endpunkt anbietet. Leer heißt: der Agent bekommt ein Platzhalter-Modell und eine Warnung. |
| `llm.passthrough_paths` | list of string | `["/v1/","/api/"]` | advanced | denied | Pfadpräfixe, die als LLM-Passthrough gelten. |

### `pseudonyms`

Rücktausch von Pseudonymen in Antworten.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Beschreibung |
|---|---|---|---|---|---|
| `pseudonyms.max_response_bytes` | integer | `8388608` | expert | denied | Größte Antwort, die für den Rücktausch gepuffert wird. Alles darüber läuft unverändert durch. |
| `pseudonyms.translate_responses` | boolean | `true` | advanced | denied | Ersetzt Pseudonyme in Text-Antworten wieder durch den Originalwert. |

### `recorder`

Aufzeichnung der Flows.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Beschreibung |
|---|---|---|---|---|---|
| `recorder.inline_max_bytes` | integer | `262144` | expert | allowed | Bodies bis zu dieser Größe stehen in der Datenbank, größere als Datei im Blob-Speicher. |
| `recorder.retention_days` | integer | `90` | advanced | denied | Tage, die eine Aufzeichnung aufgehoben wird. |

### `resolver`

Namensauflösung nach der Entscheidung.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Beschreibung |
|---|---|---|---|---|---|
| `resolver.cache_ttl_secs` | integer | `300` | expert | denied | Sekunden, die eine Antwort im Zwischenspeicher bleibt. |
| `resolver.nameserver` | string, optional | `-` | expert | denied | Nameserver als `IP:Port`. Leer bedeutet: die Einstellung des Systems. |
| `resolver.overrides` | table of string | `{}` | expert | denied | Feste Zuordnungen von Hostname zu Adresse, vor jeder Abfrage. |
| `resolver.prefer` | ipv4 \| ipv6 | `"ipv4"` | expert | denied | Welche Adressfamilie bevorzugt wird, wenn beide vorliegen. |
| `resolver.test_ca` | string, optional | `-` | expert | denied | Zusätzliche CA für Tests. Nur in Testläufen setzen, nie im Alltag. |

### `sandbox`

Welches Sandbox-Profil mit welchem Arbeitsverzeichnis startet.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Beschreibung |
|---|---|---|---|---|---|
| `sandbox.env` | table of string | `{}` | advanced | denied | Zusätzliche Umgebungsvariablen für die Sandbox; sie überschreiben gleichnamige Einträge aus dem `[env]` des Profils. Der Schlüssel lässt sich aus der Umgebung des Prozesses setzen und ist damit nur so vertrauenswürdig wie die Shell, aus der Humanitl startet; die Variablen des dynamischen Linkers (`LD_PRELOAD`, `LD_AUDIT`, `LD_LIBRARY_PATH`) werden deshalb abgelehnt, sie liefen vor dem seccomp-Filter des Shims. |
| `sandbox.profile` | string | `"default"` | advanced | denied | Name des Profils unter `profiles/sandbox/`, ohne Endung. |
| `sandbox.work_dir` | string, optional | `-` | basic | denied | Projektverzeichnis, das als `/work` eingehängt wird. Leer bedeutet: das aktuelle Verzeichnis. |
| `sandbox.work_mode` | ro \| rw | `"rw"` | basic | denied | Ob der Agent im Projektverzeichnis schreiben darf. |

### `ui`

Sprache, Erscheinungsbild und Meldungen der Oberfläche.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Beschreibung |
|---|---|---|---|---|---|
| `ui.language` | en \| de | `"en"` | basic | allowed | Sprache der Oberfläche. |
| `ui.notifications` | boolean | `true` | advanced | allowed | Meldung des Systems, wenn eine Anfrage wartet und das Fenster nicht vorn ist. |
| `ui.sound` | boolean | `false` | advanced | allowed | Ton zur Meldung. Im MVP ohne Wirkung: der Schlüssel wird gelesen, aber kein Ton gespielt (HUM-034). |
| `ui.theme` | dark \| light \| system | `"dark"` | advanced | allowed | Erscheinungsbild der Oberfläche. |

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
