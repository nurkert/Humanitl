# Konfiguration

Applies to: Humanitl 0.1

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
| 6 | die Projektwahl, die der Daemon je Projekt auf dem Host hält (HUM-532); nur die Schlüssel des Erststart-Bildschirms, Herkunft `project-choice` |
| 7 | Umgebungsvariablen `HUMANITL_*` |
| 8 | Argumente der Kommandozeile |
| 9 | Sperren einer Firmenvorgabe (HUM-534), nur lesbar, Herkunft `policy` |

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
| `agent.pi.web_access` | auto \| on \| off | `"auto"` | advanced | denied | ja | Lädt die pi-Erweiterung `pi-web-access` (Websuche und Abruf von Seiten), wenn sie global installiert ist (`sudo npm install -g --ignore-scripts pi-web-access`). `auto` lädt sie, wenn die Sandbox sie findet, `on` ebenso und meldet `AGENT_007`, wenn sie fehlt, `off` lädt sie nie. Jede Anfrage der Erweiterung geht wie jede andere durch den Proxy. Im Projekt-Profil gesperrt: fremden Code in den Agenten zu laden, entscheidet der Nutzer. |

### `agents`

Woher die Agenten kommen: vom Host oder aus dem Depot des Daemons (ADR-0023). Nicht zu verwechseln mit `agent`, das den Adapter einer Sitzung wählt.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `agents.agy.credential_writeback` | boolean | `true` | expert | denied | ja | Ob eine Sitzung die geprüfte Zugangsdatei des Agenten ins Depot zurückschreiben darf, damit eine erneuerte Anmeldung die Sitzung überdauert. Der Wächter lässt nur diese eine Datei durch, im Format des Adapters, nie älter und nie von einem anderen Konto (HUM-520); mit `false` bleibt nichts einer Sitzung im Depot. Im Projekt-Profil gesperrt. |
| `agents.agy.source` | host \| depot \| null | `-` | advanced | denied | ja | Woher dieser Agent kommt, `host` oder `depot`; ungesetzt gilt `agents.source`. Zurück auf `host` behält das Depot, bis `humanitl agents remove` es entfernt. Im Projekt-Profil gesperrt. |
| `agents.claude.credential_writeback` | boolean | `true` | expert | denied | ja | Ob eine Sitzung die geprüfte Zugangsdatei des Agenten ins Depot zurückschreiben darf, damit eine erneuerte Anmeldung die Sitzung überdauert. Der Wächter lässt nur diese eine Datei durch, im Format des Adapters, nie älter und nie von einem anderen Konto (HUM-520); mit `false` bleibt nichts einer Sitzung im Depot. Im Projekt-Profil gesperrt. |
| `agents.claude.source` | host \| depot \| null | `-` | advanced | denied | ja | Woher dieser Agent kommt, `host` oder `depot`; ungesetzt gilt `agents.source`. Zurück auf `host` behält das Depot, bis `humanitl agents remove` es entfernt. Im Projekt-Profil gesperrt. |
| `agents.codex.credential_writeback` | boolean | `true` | expert | denied | ja | Ob eine Sitzung die geprüfte Zugangsdatei des Agenten ins Depot zurückschreiben darf, damit eine erneuerte Anmeldung die Sitzung überdauert. Der Wächter lässt nur diese eine Datei durch, im Format des Adapters, nie älter und nie von einem anderen Konto (HUM-520); mit `false` bleibt nichts einer Sitzung im Depot. Im Projekt-Profil gesperrt. |
| `agents.codex.source` | host \| depot \| null | `-` | advanced | denied | ja | Woher dieser Agent kommt, `host` oder `depot`; ungesetzt gilt `agents.source`. Zurück auf `host` behält das Depot, bis `humanitl agents remove` es entfernt. Im Projekt-Profil gesperrt. |
| `agents.encrypt_credentials` | boolean | `false` | advanced | denied | ja | Ob die Zugangsdateien im Depot verschlüsselt liegen (ChaCha20-Poly1305, HUM-523). Der Schlüssel liegt nur im Schlüsselbund des Systems, nie in einer Datei neben den Daten; ohne erreichbaren Schlüsselbund lehnt der Daemon `true` ab. Umschalten über `humanitl config set agents.encrypt_credentials` oder den Schalter im Abschnitt Agenten: Der Daemon verschlüsselt oder entschlüsselt dabei die vorhandenen Dateien. Der Daemon entschlüsselt für jede Sitzung in ihre Arbeitskopie. Im Projekt-Profil gesperrt. |
| `agents.opencode.credential_writeback` | boolean | `true` | expert | denied | ja | Ob eine Sitzung die geprüfte Zugangsdatei des Agenten ins Depot zurückschreiben darf, damit eine erneuerte Anmeldung die Sitzung überdauert. Der Wächter lässt nur diese eine Datei durch, im Format des Adapters, nie älter und nie von einem anderen Konto (HUM-520); mit `false` bleibt nichts einer Sitzung im Depot. Im Projekt-Profil gesperrt. |
| `agents.opencode.source` | host \| depot \| null | `-` | advanced | denied | ja | Woher dieser Agent kommt, `host` oder `depot`; ungesetzt gilt `agents.source`. Zurück auf `host` behält das Depot, bis `humanitl agents remove` es entfernt. Im Projekt-Profil gesperrt. |
| `agents.pi.credential_writeback` | boolean | `true` | expert | denied | ja | Ob eine Sitzung die geprüfte Zugangsdatei des Agenten ins Depot zurückschreiben darf, damit eine erneuerte Anmeldung die Sitzung überdauert. Der Wächter lässt nur diese eine Datei durch, im Format des Adapters, nie älter und nie von einem anderen Konto (HUM-520); mit `false` bleibt nichts einer Sitzung im Depot. Im Projekt-Profil gesperrt. |
| `agents.pi.source` | host \| depot \| null | `-` | advanced | denied | ja | Woher dieser Agent kommt, `host` oder `depot`; ungesetzt gilt `agents.source`. Zurück auf `host` behält das Depot, bis `humanitl agents remove` es entfernt. Im Projekt-Profil gesperrt. |
| `agents.source` | host \| depot | `"host"` | advanced | denied | ja | Woher die Agenten kommen, für jeden, der unter `agents.<id>.source` nichts sagt: `host` (Vorgabe) sucht das Programm auf dem Rechner, `depot` nimmt Programm, Konfiguration und Anmeldung aus dem Depot unter dem Datenordner des Daemons (`humanitl agents status`). Im Projekt-Profil gesperrt. |

### `audit`

Die Audit-Kette: Anker, Synchronisation, Aufbewahrung.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `audit.anchor_every` | integer | `100` | advanced | denied | ja | Jeder wievielte Record ein Anker ist, in der Datei und zugleich in der Datenbank. Records hinter dem letzten Anker kann jemand mit Zugriff auf die Datei am Ende abschneiden, ohne dass die Prüfung es merkt; ein kleinerer Wert macht dieses Fenster kleiner. Beim Beenden des Daemons wird immer geankert. |
| `audit.fsync_every` | integer | `50` | expert | denied | ja | Nach wie vielen Records der Daemon das Audit-Log auf die Platte zwingt (fsync). Spätestens nach einer Sekunde und vor jedem Anker geschieht es ohnehin. |
| `audit.max_bytes` | integer | `0` | expert | denied | ja | Obergrenze für die Größe des Audit-Logs in Bytes; 0 heißt keine Grenze, sonst 2097152 (2 MiB) bis 1099511627776 (1 TiB). Derselbe Lauf wie bei `audit.retention_days`, beim Start und danach täglich, löscht ganze Records am Anfang der Kette, das Älteste zuerst, bis die Datei samt dem Record `audit.pruned` und seinem Anker unter der Grenze liegt; zwischen zwei Läufen kann sie darüber wachsen. Der neueste Record bleibt immer stehen; ist er länger als 1 MiB, kann das Log mit ihm über der Grenze bleiben, und der Daemon meldet das im Protokoll. Eine Kette, die nicht hält, wird nicht gekürzt. Der Lauf braucht kurz Platz für eine Kopie des Rests neben der Datei; fehlt er, bleibt das Log, wie es war, und der nächste Lauf versucht es erneut. |
| `audit.retention_days` | integer | `0` | expert | denied | ja | Tage, die ein Record im Audit-Log aufgehoben wird; 0 heißt für immer. Ein täglicher Lauf löscht die Records am Anfang der Kette, die älter sind, und dokumentiert den Schnitt mit einem Record `audit.pruned` samt Anker; die Prüfung erkennt den so dokumentierten Anfang an, einen Anfang ohne ihn meldet sie weiter als Bruch. Eine Kette, die nicht hält, wird nicht gekürzt. Was in den gelöschten Records stand, beweist die Kette danach nicht mehr; die Anker in `audit_anchors` bleiben stehen. |

### `egress`

Ein Entscheidungs-Proxy des Nutzers hinter Humanitl (ADR-0022, HUM-511).

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `egress.proxy` | string, optional | `-` | advanced | denied | ja | Adresse eines Entscheidungs-Proxys des Nutzers als `http://HOST:PORT` (etwa mitmproxy). Gesetzt verlässt jede erlaubte Anfrage des Agenten den Daemon über diesen Proxy; `https` geht als `CONNECT`-Tunnel, in dem Humanitl TLS neu aufbaut, `http` als Anfrage in absoluter Form. Anfragen des Daemons selbst (Suche nach Modellservern, Update-Prüfung) bleiben direkt. Die Prüfung auf private Adressen gilt dem Ziel, nie diesem Proxy; er darf auf `127.0.0.1` laufen. Leer heißt: kein zweiter Proxy, nichts ändert sich. Wirkt nach dem Neustart des Daemons. |
| `egress.proxy_auth_ref` | string, optional | `-` | advanced | denied | ja | Verweis auf die Zugangsdaten des Entscheidungs-Proxys als `NUTZER:PASSWORT` (geteilt am ersten Doppelpunkt; das Passwort darf Leerzeichen tragen) im Schlüsselbund oder, wenn er nicht antwortet, in einer Datei mit `0600` (`CONFIG_019`), nie die Daten selbst: `secret-service:humanitl/egress-proxy`, unter macOS `keychain:humanitl/egress-proxy`, oder `file:egress-proxy`. Gesetzt wird er mit `humanitl egress auth set` oder in den Einstellungen; der Daemon schickt sie als `Proxy-Authorization: Basic` nur an diesen Proxy. Leer heißt: ohne Anmeldung. |
| `egress.proxy_ca` | string, optional | `-` | advanced | denied | ja | Absoluter Pfad der PEM-Datei mit der CA des Entscheidungs-Proxys, bei mitmproxy `~/.mitmproxy/mitmproxy-ca-cert.pem` (ausgeschrieben, ohne `~`). Ihr wird nur auf dem Weg über `egress.proxy` vertraut, zusätzlich zu den Ankern aus `tls.trust_system_store`, nie auf einer direkten Verbindung. Wirkt nach dem Neustart des Daemons. |
| `egress.proxy_resolve` | proxy \| humanitl | `"proxy"` | advanced | denied | ja | Wer den Namen des Ziels auflöst, wenn `egress.proxy` gesetzt ist. `proxy` (Vorgabe): Humanitl löst nichts auf und schickt `CONNECT host:port`, der Entscheidungs-Proxy löst nach seiner Entscheidung auf; Humanitl lehnt vorher nur private IP-Literale und lokale Namen ab (`localhost`, `*.localhost`, `*.local`, `*.internal`, `*.home.arpa` und jeden Namen ohne Punkt, etwa `metadata`), und das bei beiden Werten, bevor irgendetwas aufgelöst wird. Ein öffentlicher Name, der auf eine private Adresse zeigt, ist dann Sache des Entscheidungs-Proxys. `humanitl`: Humanitl löst nach seiner Entscheidung auf, prüft die Adresse wie ohne zweiten Proxy und schickt `CONNECT IP:PORT`; die DNS-Anfrage verlässt den Rechner dann, bevor der Entscheidungs-Proxy entschieden hat. |
| `egress.proxy_response_timeout_secs` | integer | `300` | advanced | denied | ja | Sekunden, die der Daemon über `egress.proxy` auf den Kopf der Antwort wartet, auch nach einem gestreamten Upload, weil dort ein Mensch entscheiden kann. `CONNECT` und der TLS-Handschlag behalten `limits.connect_timeout_secs` und `limits.header_timeout_secs`. Läuft die Frist ab, bekommt der Agent `502`. |

### `experimental`

Schalter für unfertige Wege. Alles hier darf ohne Ankündigung wegfallen.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `experimental.h2_upstream` | boolean | `false` | expert | denied | ja | Bietet dem Ziel HTTP/2 an. In M1 spricht der Proxy nach oben nur HTTP/1.1. |

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
| `hold.ask_mode` | ui \| terminal \| none \| upstream | `"ui"` | advanced | denied | ja | Wo gefragt wird: in der Oberfläche, im Terminal, gar nicht, oder `upstream`: Eine Anfrage ohne Regel oder mit `ask` wird nicht gehalten, sondern geht an den Entscheidungs-Proxy aus `egress.proxy`, der dort entscheidet (nur mit `egress.proxy`). `block` bleibt endgültig, `redact` hält weiter hier. |
| `hold.hard_block_checksum_secrets` | boolean | `false` | advanced | denied | ja | Lässt Anfragen mit prüfsummen-sicheren Geheimnissen nur bearbeitet hinaus; keine Regel und keine Freigabe sendet sie unverändert. |
| `hold.timeout_secs` | integer | `300` | basic | allowed | ja | Sekunden, die eine angehaltene Anfrage auf eine Entscheidung wartet, bevor sie als Zeitüberschreitung endet. |

### `limits`

Alle Caps und Zeitgrenzen an einer Stelle.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `limits.body_timeout_secs` | integer | `300` | expert | allowed | ja | Sekunden Stille zwischen zwei Stücken eines Bodys, nicht seine Gesamtdauer. Gilt in beide Richtungen: für den Anfrage-Rumpf des Clients (danach 408) und für den gestreamten Antwort-Rumpf des Ziels (danach wird der Strom abgebrochen und die Aufzeichnung als gekürzt vermerkt). Ein großer Upload und ein langer Modell-Strom dürfen deshalb beliebig lange dauern, solange sie nicht verstummen. |
| `limits.connect_timeout_secs` | integer | `10` | advanced | allowed | ja | Sekunden bis zum Aufbau der Verbindung zum Ziel. |
| `limits.event_buffer` | integer | `1024` | expert | allowed | ja | Länge der Ereignis-Warteschlange je Client. Läuft sie über, meldet der Daemon `Lagged`. |
| `limits.header_timeout_secs` | integer | `30` | expert | allowed | ja | Sekunden, in denen der Client seine Anfrage-Kopfzeilen gesendet haben muss. Auf einer Keep-Alive-Verbindung ist das zugleich die Frist bis zur nächsten Anfrage, also die einzige Leerlaufgrenze der Verbindung zum Agenten; während eine Anfrage gehalten wird, läuft sie nicht. |
| `limits.hold_body_cap_bytes` | integer | `33554432` | advanced | allowed | ja | Größte Anfrage, deren Body für die Entscheidung im Speicher gehalten wird. Darüber antwortet der Proxy mit 413, außer eine Regel `allow` oder `ask` mit `stream: true` trifft die Anfrage und `hold.hard_block_checksum_secrets` ist aus: Dann geht der Body gestreamt und ungeprüft hinaus. |
| `limits.hold_max_bytes` | integer | `268435456` | advanced | allowed | ja | Größte Summe der Bodies aller angehaltenen Flows. Darüber antwortet der Proxy mit 503. |
| `limits.hold_max_flows` | integer | `200` | advanced | allowed | ja | Größte Zahl gleichzeitig angehaltener Flows. Darüber antwortet der Proxy mit 503. |
| `limits.max_client_connections` | integer | `256` | expert | allowed | ja | Größte Zahl gleichzeitiger Verbindungen aus der Sandbox je Sitzung. Darüber antwortet der Proxy mit 503 und schließt; eine Uhr je Spanne allein hindert einen Prozess nicht daran, dieselben Ressourcen über viele Verbindungen zu binden. |
| `limits.max_decompress_ratio` | integer | `100` | expert | allowed | ja | Höchstes erlaubtes Verhältnis von entpackten zu gepackten Bytes einer Vorschau. |
| `limits.preview_cap_bytes` | integer | `8388608` | expert | allowed | ja | Größte Menge Body, die die Oberfläche als Vorschau bekommt. |
| `limits.recorder_max_body_bytes` | integer | `33554432` | expert | denied | ja | Größter Body, den die Aufzeichnung als Blob ablegt. Alles darüber wird nur mit Prüfsumme vermerkt. |
| `limits.registry_max_finished_flows` | integer | `10000` | expert | denied | ja | Größte Zahl abgeschlossener Flows, die der Daemon im Speicher behält. Darüber fällt der älteste abgeschlossene heraus; die Aufzeichnung hat ihn da längst. Gehaltene und laufende Flows zählen nicht und fallen nie heraus. Ohne Aufzeichnung zeigt die Liste nur diese letzten Flows. |

### `llm`

Der lokale LLM-Endpunkt und was als Passthrough gilt.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `llm.active` | string, optional | `-` | advanced | denied | ja | Der Name des aktiven Eintrags aus `llm.servers`. Nur sein Endpunkt bekommt den hinterlegten Zugangsschlüssel und gilt als LLM-Passthrough. Leer, solange es keine Einträge gibt. |
| `llm.api_key_ref` | string, optional | `-` | advanced | denied | ja | Verweis auf den Zugangsschlüssel des Endpunkts, nie der Schlüssel selbst: `secret-service:humanitl/llm` für den Schlüsselbund des Systems, unter macOS `keychain:humanitl/llm`, oder `file:llm` für eine Datei mit `0600` unter `$XDG_DATA_HOME/humanitl/keys`. Gesetzt wird er mit `humanitl llm key set` oder in den Einstellungen; der Agent bekommt den Schlüssel als Datei in der Sandbox, nie über die Kommandozeile. Leer heißt: kein Schlüssel. |
| `llm.endpoint` | string, optional | `-` | basic | denied | ja | OpenAI-kompatibler Endpunkt im LAN. Verkehr dorthin wird nicht angehalten, aber protokolliert. |
| `llm.models` | list of string | `[]` | basic | denied | ja | Modelle, die der Endpunkt anbietet. Leer heißt: der Agent bekommt ein Platzhalter-Modell und eine Warnung. |
| `llm.passthrough_paths` | list of string | `["/v1/","/api/"]` | advanced | denied | ja | Pfadpräfixe, die als LLM-Passthrough gelten. Ein Präfix soll einen Endpunkt benennen, keine ganze API-Fläche: Der Agent-Adapter ersetzt `/v1/` und `/api/` deshalb durch die Endpunkte, die Inferenz machen, damit `POST /api/pull` und `POST /v1/files` nicht ungefragt hinausgehen. Ein Pfad, der mehr nennt, bleibt stehen, wie er hier steht. |
| `llm.server` | string, optional | `-` | advanced | allowed | ja | Der Name eines Eintrags aus `llm.servers`, der für die Sitzung statt `llm.active` gilt: gesetzt im Projekt-Profil, in einem Profil oder für einen Lauf (`humanitl run --llm NAME`). Er wählt nur unter den gespeicherten Servern und legt nie selbst einen Endpunkt, ein Modell oder einen Schlüssel fest. Rangfolge: Lauf vor Projekt vor Profil vor `llm.active`. Ein Name, den es nicht gibt, lässt die Sitzung nicht starten. |
| `llm.servers` | table of table | `{}` | advanced | denied | ja | Gespeicherte Modellserver, je Name ein Eintrag mit `endpoint`, `models` und `api_key_ref` (`secret-service:humanitl/NAME`, unter macOS `keychain:humanitl/NAME`). Der Eintrag, den `llm.active` nennt, liefert `llm.endpoint`, `llm.models` und `llm.api_key_ref`; die flachen Schlüssel sind seine Ansicht. Ein Name hat 1 bis 32 Zeichen `a-z`, `0-9`, `-` und `_`. Gepflegt wird die Tabelle mit `humanitl llm server` oder in den Einstellungen; ein Wechsel gilt ab der nächsten Sitzung. Ein Eintrag mit `local = true` ist ein Server im eigenen Netz, dessen Name öffentlich aussieht: Sitzungen mit ihm zeigen keinen Hinweis auf Cloud-Inferenz (HUM-516). |

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
| `recorder.retention_days` | integer | `180` | advanced | denied | ja | Tage, die eine Aufzeichnung (Anfragen, Antworten, Bodies) aufgehoben wird; 0 heißt nie löschen. Ein täglicher Lauf löscht, was älter ist, und vermerkt jede Löschung im Audit-Log (`recorder.retention_applied`). Die Frist setzt die Speicherbegrenzung aus DSGVO Art. 5 Abs. 1 lit. e um: Personenbezogenes in Anfragen bleibt nicht länger als nötig liegen. Die Audit-Kette löscht dieser Lauf nie. |

### `resolver`

Namensauflösung nach der Entscheidung.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `resolver.cache_ttl_secs` | integer | `300` | expert | denied | ja | Sekunden, die eine Antwort im Zwischenspeicher bleibt. |
| `resolver.nameserver` | string, optional | `-` | expert | denied | ja | Nameserver als `IP:Port`, ohne Port gilt 53. Gesetzt fragt der Daemon nach einer Freigabe nur noch diesen Server, über UDP mit TCP-Rückfall, und weder `/etc/resolv.conf` noch `/etc/hosts`. Leer bedeutet: der Namensdienst des Systems. |
| `resolver.overrides` | table of string | `{}` | expert | denied | ja | Feste Zuordnungen von Hostname zu Adresse, vor jeder Abfrage. Nicht leer meldet der Start als `CONFIG_016`, damit der Testhebel nicht unbemerkt in einem Alltagslauf steht. |
| `resolver.prefer` | ipv4 \| ipv6 | `"ipv4"` | expert | denied | ja | Welche Adressfamilie bevorzugt wird, wenn beide vorliegen. |
| `resolver.test_ca` | string, optional | `-` | expert | denied | ja | Zusätzliche CA für Tests, als PEM und mit absolutem Pfad. Gilt nur, wenn der Daemon mit `--allow-test-ca` gestartet wurde; ohne das Flag wird der Schlüssel nicht angenommen. Ein nicht absoluter Pfad wird schon beim Laden abgelehnt, mit und ohne Flag, statt gegen das Arbeitsverzeichnis aufgelöst zu werden. Nur in Testläufen setzen, nie im Alltag. |

### `sandbox`

Welches Sandbox-Profil mit welchem Arbeitsverzeichnis startet.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `sandbox.backend` | auto \| bwrap \| podman \| seatbelt \| qemu | `"auto"` | basic | denied | ja | Welches Sandbox-Backend startet. `auto`, die Vorgabe, ist bubblewrap unter Linux und Seatbelt unter macOS. Ein gewähltes Backend fällt nie auf ein anderes zurück: Gibt es es auf diesem System nicht, lehnt der Start mit `SANDBOX_034` ab; liegt es unter `sandbox.min_isolation`, mit `SANDBOX_035`; verlangt das Sandbox-Profil ein anderes, mit `SANDBOX_036`. `qemu`, die Kernel-Stufe unter Linux auf x86-64, ist seit HUM-449 zugelassen: Ein Start damit läuft in einer Micro-VM mit eigenem Gastkern, und fehlt eine ihrer Voraussetzungen, lehnt er mit `SANDBOX_045` ab; unter macOS gibt es sie nicht. Im Projekt-Profil gesperrt; ein Client darf den Schlüssel für eine Sitzung setzen (`cli_overrides`), die Untergrenze gilt auch dann. |
| `sandbox.env` | table of string | `{}` | advanced | denied | ja | Zusätzliche Umgebungsvariablen für die Sandbox; sie überschreiben gleichnamige Einträge aus dem `[env]` des Profils. Der Schlüssel lässt sich aus der Umgebung des Prozesses setzen und ist damit nur so vertrauenswürdig wie die Shell, aus der Humanitl startet; die Variablen des dynamischen Linkers (`LD_PRELOAD`, `LD_AUDIT`, `LD_LIBRARY_PATH`) werden deshalb abgelehnt, sie liefen vor dem seccomp-Filter des Shims. |
| `sandbox.min_isolation` | policy \| namespaces \| kernel | `"policy"` | advanced | allowed | ja | Die schwächste Isolationsstufe, mit der eine Sitzung starten darf: `policy` (eine Richtlinie des Kerns, Seatbelt), `namespaces` (eigene Namensräume über dem Kern des Hosts, bubblewrap und Podman) oder `kernel` (ein eigener Gastkern in einer Micro-VM, das Backend `qemu`). Liegt das gewählte Backend darunter, lehnt der Start mit `SANDBOX_035` ab, statt ein anderes zu nehmen. Über alle Ebenen gilt der strengste Wert, nicht der oberste: Ein Projekt-Profil darf die Untergrenze heben, und weder ein Projekt noch die Umgebung noch die Kommandozeile senkt sie unter das, was eine andere Ebene verlangt. |
| `sandbox.mode` | auto \| direct \| disposable | `"auto"` | basic | denied | ja | Wohin die Änderungen des Agenten gehen. `direct` schreibt in das Projektverzeichnis; `disposable` legt es nur lesbar unter eine Schicht (unter Linux im Arbeitsspeicher), und nach dem Lauf entscheidet der Mensch, welche Änderungen er übernimmt (`humanitl sandbox changes`, `apply`, `discard`). `auto`, die Vorgabe, ist `disposable`, wo das System es kann (Linux, bubblewrap 0.11 oder neuer, Laufzeitverzeichnis auf tmpfs, Projekt schreibbar eingehängt), sonst `direct`, und der Start sagt dann mit `SANDBOX_033`, warum (HUM-357). Ein ausdrücklich gesetztes `disposable` fällt nie zurück: Wo es nicht geht, lehnt der Start mit `SANDBOX_030` ab. Unter macOS ist die Schicht ein APFS-Klon des Projekts statt eines Overlays; geht der Klon nicht, gilt dieselbe Regel (HUM-374). |
| `sandbox.podman.cpus` | integer, optional | `-` | advanced | denied | ja | Zahl der CPUs der Sandbox, mindestens 1 und höchstens die Zahl der CPUs, die der Daemon nutzen darf. Die Obergrenze prüft der Start dieses Backends (`CONFIG_003`), nicht das Laden. Leer heißt: der Start leitet sie vom Host ab (die Hälfte seiner CPUs, mindestens 1, höchstens 4). |
| `sandbox.podman.image` | string, optional | `-` | advanced | denied | ja | Das Abbild für `rootfs = "image"`, immer mit Digest (`name@sha256:` und 64 Hex-Ziffern); bei `rootfs = "image"` Pflicht. Vor jedem Start prüft der Daemon, dass genau dieser Digest im lokalen Speicher liegt (sonst `SANDBOX_043`); ein Start zieht nie, das tut nur `humanitl sandbox image pull`, und das Ziehen nutzt das Netz direkt, nicht durch den Proxy. |
| `sandbox.podman.memory_mib` | integer, optional | `-` | advanced | denied | ja | Arbeitsspeicher der Sandbox in MiB, mindestens 256 und höchstens der Arbeitsspeicher des Hosts. Die Obergrenze prüft der Start dieses Backends (`CONFIG_003`), nicht das Laden: Ein Wert über diesem Rechner hält kein anderes Backend auf. Leer heißt: der Start leitet ihn vom Host ab (die Hälfte seines Arbeitsspeichers, höchstens 4096), damit die Vorgabe auf jedem Rechner gilt. |
| `sandbox.podman.pids_limit` | integer | `4096` | expert | denied | ja | Größte Zahl von Prozessen und Threads in der Sandbox, mindestens 64. |
| `sandbox.podman.program` | string, optional | `-` | expert | denied | ja | Das Programm `podman`, als absoluter Pfad. Leer heißt: die Suche im `PATH` des Daemons. |
| `sandbox.podman.rootfs` | host \| image | `"host"` | advanced | denied | ja | Die Wurzel der Sandbox: `host` ist ein leeres, nur lesbares Verzeichnis mit den Einhängungen des Sandbox-Profils, `image` ein lokales OCI-Abbild aus `sandbox.podman.image`, das ein Start nie zieht. |
| `sandbox.podman.runtime` | crun | `"crun"` | advanced | denied | ja | Die OCI-Laufzeit: `crun`, die den Kern des Hosts teilt (Stufe `namespaces`). Einen eigenen Gastkern startet nicht Podman, sondern das Backend `qemu`; die frühere Laufzeit dafür lehnt das Laden mit `CONFIG_023` ab. |
| `sandbox.podman.start_timeout_secs` | integer | `30` | expert | denied | ja | Sekunden, die ein Start auf die laufende Sandbox wartet, bevor er abbricht, 1 bis 600. |
| `sandbox.profile` | string | `"default"` | advanced | denied | ja | Name des Profils unter `profiles/sandbox/`, ohne Endung. |
| `sandbox.qemu.cpus` | integer, optional | `-` | advanced | denied | ja | Zahl der virtuellen CPUs der Micro-VM, mindestens 1 und höchstens die Zahl der CPUs, die der Daemon nutzen darf. Die Obergrenze prüft der Start dieses Backends (`CONFIG_003`), nicht das Laden. Leer heißt: der Start leitet sie vom Host ab (die Hälfte seiner CPUs, mindestens 1, höchstens 4). |
| `sandbox.qemu.kernel` | string, optional | `-` | expert | denied | ja | Das Kernel-Abbild des Gasts, als absoluter Pfad einer lesbaren Datei. Leer heißt: der Kernel des Hosts, `/boot/vmlinuz-` mit der Fassung aus `uname -r`. |
| `sandbox.qemu.memory_mib` | integer, optional | `-` | advanced | denied | ja | Arbeitsspeicher der Micro-VM in MiB, mindestens 256 und höchstens der Arbeitsspeicher des Hosts. Die Obergrenze prüft der Start dieses Backends (`CONFIG_003`), nicht das Laden: Ein Wert über diesem Rechner hält kein anderes Backend auf. Leer heißt: der Start leitet ihn vom Host ab (die Hälfte seines Arbeitsspeichers, höchstens 4096). |
| `sandbox.qemu.modules` | string, optional | `-` | expert | denied | ja | Das Verzeichnis der Module dieses Kernels, als absoluter Pfad; der Gast lädt daraus die Module für vsock und sperrt danach das Laden weiterer. Leer heißt: `/usr/lib/modules/` mit der Fassung des Kernels. |
| `sandbox.qemu.qemu` | string, optional | `-` | expert | denied | ja | Der Emulator der Maschine (`qemu-system-x86_64`), als absoluter Pfad. Leer heißt: die Suche im `PATH` des Daemons. |
| `sandbox.qemu.start_timeout_secs` | integer | `30` | expert | denied | ja | Sekunden, die ein Start auf den Gast wartet, bevor er abbricht, 1 bis 600. |
| `sandbox.qemu.vhost_device_vsock` | string, optional | `-` | expert | denied | ja | Das vsock-Gerät der Tür (`vhost-device-vsock`), als absoluter Pfad. Leer heißt: die Suche im `PATH` des Daemons. |
| `sandbox.qemu.virtiofsd` | string, optional | `-` | expert | denied | ja | Der Dienst für die Dateisysteme des Gasts (`virtiofsd`), als absoluter Pfad. Leer heißt: die Suche im `PATH` des Daemons, danach `/usr/libexec/virtiofsd`. |
| `sandbox.work_dir` | string, optional | `-` | basic | denied | ja | Projektverzeichnis, das als `/work` eingehängt wird. Leer bedeutet: das aktuelle Verzeichnis. |
| `sandbox.work_mode` | ro \| rw | `"rw"` | basic | denied | ja | Ob der Agent im Projektverzeichnis schreiben darf. |

### `terminal`

Das Terminal der Kommandozeile, wenn sie an einer Sitzung hängt.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `terminal.bell_on_hold` | boolean | `true` | advanced | denied | ja | Eine neu gehaltene Anfrage klingelt im Terminal der Sitzung (`humanitl sandbox attach`, `humanitl run -- CMD` an einem Terminal, `humanitl go`): ein `BEL`-Byte zwischen zwei ganzen Stücken der Ausgabe des Agenten, nie mitten in einer Steuerfolge, höchstens eines je Sekunde. Text schreibt die Kommandozeile dafür nie in das Terminal des Agenten. Solange eine Ansicht über dem Agenten steht (Einstellungen, Interceptor, Hilfe), klingelt nichts; der Interceptor zeigt die neue Anfrage selbst. `false` schaltet die Glocke ab; die Oberfläche und die Benachrichtigung des Systems melden die Anfrage weiter. |
| `terminal.direct_keys` | boolean | `true` | advanced | denied | ja | `Ctrl+,` öffnet die Einstellungen ohne Präfix, aber nur, solange der Agent im Terminal das kitty-Tastaturprotokoll eingeschaltet hat (`CSI > Flags u`, abgeschaltet mit `CSI < u`): Nur dann schickt das Terminal für diese Taste eine eigene Folge (`CSI 44 ; 5 u`). Die Kommandozeile schaltet das Protokoll nie selbst ein. Mit `false` oder ohne Präfix (`terminal.prefix = "none"`) geht auch diese Folge an den Agenten. |
| `terminal.prefix` | string | `"ctrl-b"` | advanced | denied | ja | Die Präfixtaste im Terminal einer Sitzung (`humanitl sandbox attach`, `humanitl run -- CMD` an einem Terminal, `humanitl go`): `ctrl-a` bis `ctrl-z`, `ctrl-\`, `ctrl-]`, `ctrl-^`, `ctrl-_`, oder `none`, dann gibt es keine und jedes Byte geht an den Agenten. Danach gilt eine feste Tabelle: `d` hängt ab und lässt die Sitzung laufen, `,` Einstellungen über dem Agenten, `Esc` kehrt zu ihm zurück, `i` Interceptor über dem Agenten, gehaltene Anfragen entscheiden, `Esc` kehrt zurück, `?` zeigt diese Tabelle. Das Präfix zweimal oder sein Buchstabe schickt es einmal an den Agenten, so erreicht `Ctrl+B` ein tmux in der Sandbox und eine Shell, für die es ein Zeichen zurück heißt; jede andere Taste geht mit dem Präfix unverändert weiter. `ctrl-[` ist ausgenommen, weil jede Pfeiltaste mit diesem Byte beginnt, `ctrl-h`, `ctrl-i`, `ctrl-j` und `ctrl-m`, weil Rücktaste, Tab und Enter dieselben Bytes schicken. Der alte Name `terminal.detach_prefix` gilt weiter. `--prefix` (oder `--detach-keys`) überschreibt den Wert für einen Aufruf. |
| `terminal.splash` | boolean | `true` | advanced | denied | ja | Die Ladeanimation von `humanitl go`: eine sich drehende Faust aus dem Logo, solange die Wegwerf-Sitzung startet, mindestens eine Sekunde lang und nie länger als der Start. Sie erscheint nur, wenn Ausgabe und Fehlerausgabe ein Terminal sind und `--json` fehlt, mit `NO_COLOR` ohne Farbe. `false` schaltet sie ab. |

### `tls`

Wem der Daemon auf dem Weg zum Ziel vertraut.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `tls.trust_system_store` | boolean | `true` | advanced | denied | ja | Vertraut beim Ziel den Wurzeln aus der Zertifikatsablage des Systems (`/etc/ssl/certs`), wie `curl` und der Browser: Eine Firmen-CA oder eine mit `update-ca-certificates` installierte CA wird angenommen. Aus gelten nur die öffentlichen Wurzeln aus `webpki-roots`. Wirkt nach dem Neustart des Daemons; das Projekt-Profil kann den Schlüssel nicht setzen. |

### `ui`

Sprache, Erscheinungsbild und Meldungen der Oberfläche.

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `ui.language` | en \| de | `"en"` | basic | allowed | ja | Sprache der Oberfläche. |
| `ui.notifications` | boolean | `true` | advanced | allowed | ja | Meldung des Systems, wenn eine Anfrage wartet und das Fenster nicht vorn ist. |
| `ui.terminal_notices` | boolean | `true` | advanced | allowed | ja | Hinweiszeile des Daemons, wenn eine Anfrage des Agenten auf eine Entscheidung wartet oder entschieden ist. Sie geht als eigener Rahmen an jeden Client, nie in den Bytestrom des Agenten: Wer am Terminal sitzt, sieht sie in der Fußzeile der Einstellungen über dem Agenten; in das Terminal des Agenten schreibt die Kommandozeile sie nicht (GitHub #12), dort klingelt statt dessen `terminal.bell_on_hold`; nur wenn ihre Ausgabe in eine Datei oder Pipe geht, steht sie dort zwischen den Bytes. Die Oberfläche zeigt sie im Streifen über dem Terminal. Der Streifen bleibt in jedem Fall, weil er aus dem Ereignisstrom kommt; dieser Schalter betrifft nur den Rahmen. |
| `ui.theme` | dark \| light \| system | `"dark"` | advanced | allowed | ja | Erscheinungsbild der Oberfläche. |

### `update`

Updates von Humanitl selbst (HUM-382).

| Schlüssel | Typ | Vorgabe | Stufe | Projekt | Wirkung | Beschreibung |
|---|---|---|---|---|---|---|
| `update.check` | boolean | `true` | basic | denied | ja | Fragt höchstens einmal am Tag bei GitHub (`api.github.com`, `github.com`), ob es ein neueres Release gibt, und prüft dessen signiertes Manifest. Gesendet wird nur, was jeder Download sendet; installiert wird nie ohne Klick. Aus fragt der Daemon nur, wenn jemand ausdrücklich prüft (`humanitl update check`, „Jetzt prüfen"). Das Projekt-Profil kann den Schlüssel nicht setzen. |

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
| `terminal.detach_prefix` | `terminal.prefix` | GitHub #10 |
| `upstream.connect_timeout_secs` | `limits.connect_timeout_secs` | HUM-057 |

## Entfallene Schlüssel

Diese Schlüssel gab es einmal und gibt es nicht mehr. Sie haben keinen Nachfolger. Wer
sie noch in einer Datei stehen hat, bekommt beim Laden eine Warnung (`CONFIG_005`) mit
dem Issue und dem Grund; der Wert wird übergangen, und der Daemon startet trotzdem. Ein
harter Fehler wäre hier die Strafe für eine Entscheidung, die nicht der Nutzer getroffen
hat (`backlog/CONVENTIONS.md` 4.25).

Die Spalte „Form" sagt, was mit dem geschieht, was **unter** dem Schlüssel steht. Bei
einem Wert hat es dort nie eine Ebene gegeben: `[schlüssel.irgendwas]` ist kein alter
Eintrag, sondern ein unbekannter, und scheitert weiterhin mit `CONFIG_002`. Bei einer
Tabelle gehörten ihre Schlüssel dem Nutzer; sie verschwindet als Ganzes, in einer
einzigen Warnung.

| Schlüssel | Entfallen mit | Form | Grund (Text des Befunds) |
|---|---|---|---|
| `experimental.upstream_port_map` | HUM-088 | Tabelle | the proxy never redirected a port, and no shipped test needs one: they bind the real port inside their own network namespace or address the ephemeral port directly |
| `experimental.ws_hold` | HUM-121 | Wert | no upgrade was ever held by it: a WebSocket upgrade is decided by the rule alone, and the proxy never read the switch |
| `limits.idle_timeout_secs` | HUM-101 | Wert | it described the same span as limits.header_timeout_secs, the one idle clock of the connection to the agent |
| `ui.sound` | HUM-121 | Wert | there is no sound the switch could have turned off: nothing in the user interface ever played one |

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
