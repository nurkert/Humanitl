# Aufgezeichnete Sitzungen

Eine Sitzungsdatei ist die Eingabe des Fake-Daemons (HUM-005). Sie beschreibt,
was ein Agent hinter dem Proxy getan hätte, damit die Oberfläche gebaut und
geprüft werden kann, bevor es Proxy, Sandbox und Sprachmodell gibt.

```
humanitld --fake fixtures/sessions/mixed.jsonl
humanitld --fake fixtures/sessions/npm-install.jsonl --speed 10 --loop
```

Der Fake bedient denselben Unix-Socket und dieselbe gRPC-Schnittstelle wie der
echte Daemon. Ein Client merkt den Unterschied nur an `Info.capabilities`, das
`fake` enthält.

## Format

JSON Lines: eine Zeile je Ereignis, ein JSON-Objekt pro Zeile. Leere Zeilen und
Zeilen, die mit `#` beginnen, werden übersprungen. Jede Zeile hat

- `t_ms` — Zeitpunkt relativ zum Start der Sitzung, in Millisekunden. Der
  Abspieler sortiert die Datei danach; die Reihenfolge im Text ist frei.
- `type` — eine der acht Ereignisarten unten.

Bodies stehen als `body_b64` (Base64, die verbindliche Form) oder als `body`
(reiner Text, bequem für handgeschriebene Dateien). Ist beides gesetzt,
gewinnt `body_b64`. Ein `Content-Type` wird aus den Kopfzeilen übernommen.

Alle Ids sind UUIDv7 in Textform. Ein Test prüft das für die mitgelieferten
Dateien.

### `session`

Kopfzeile der Sitzung. Höchstens einmal, üblicherweise bei `t_ms: 0`.

```json
{"t_ms":0,"type":"session","session_id":"018f0000-0000-7000-8000-000000000001","llm_endpoint":"http://192.168.1.50:11434","work_dir":"/home/nik/projects/shop-frontend"}
```

### `request`

Eine Anfrage trifft ein. Erzeugt `Received` und, wenn keine `findings`-Zeile
für denselben Flow existiert, sofort `Analyzed` mit leerer Fundliste.

| Feld | Pflicht | Bedeutung |
|---|---|---|
| `flow_id` | ja | UUIDv7 |
| `method` | ja | `GET`, `POST`, … |
| `host` | ja | Hostname oder IP-Literal |
| `path` | ja | Pfad samt Query |
| `scheme` | nein | `http`, `https`, `ws`, `wss`; Vorgabe `https` |
| `port` | nein | Vorgabe: Standard-Port des Schemas |
| `headers` | nein | Liste von Paaren `["name","wert"]` |
| `body_b64`, `body` | nein | der Anfrage-Body |
| `origin_tool` | nein | zum Beispiel `npm`, `curl`, `opencode` |
| `upgrade` | nein | `websocket` ergänzt `Connection`/`Upgrade` |

### `findings`

Was die Detektoren gefunden hätten. Erzeugt `Analyzed`.

```json
{"t_ms":430,"type":"findings","flow_id":"018f…","findings":[
  {"kind":"api_key.github","location":"header:authorization","tier":"checksum","value":"ghp_…","span":[7,47]}]}
```

`kind` ist der Wire-Name (`api_key.<anbieter>`, `jwt`, `email`, `iban`,
`credit_card`, `phone`, `ipv4`, `user_term.<begriff>`, `custom.<name>`),
`location` ist `body`, `query` oder `header:<name>`, `tier` ist `checksum`,
`regex` oder `user_term`. `value` ist der gefundene Text; er wird gehasht und
auf acht Zeichen gekürzt, nie gespeichert. `span` ist optional.

### `hold`

Die Anfrage wartet auf eine Entscheidung. Erzeugt `Held` und startet den
Zeitgeber. Ohne `timeout_ms` gilt `hold.timeout_secs` aus der Konfiguration.

```json
{"t_ms":450,"type":"hold","flow_id":"018f…","timeout_ms":300000}
```

Gehaltene Flows werden nie von der Datei entschieden. Die Entscheidung kommt
vom Client (`Decide`) oder vom Ablauf der Frist.

### `auto`

Eine Regel oder die Durchreiche entscheidet ohne den Menschen. `kind` ist
`allow` oder `block`, `source` ist `rule` (Vorgabe) oder `passthrough`.

```json
{"t_ms":1260,"type":"auto","flow_id":"018f…","source":"rule","rule_id":"018f…a1","kind":"block","note":"…"}
```

### `response`

Die Antwort des Ziels. Sie wird bereitgelegt, sobald die Anfrage eintrifft, und
gespielt, sobald der Flow weitergeleitet ist — nach einem `auto allow` zum
Zeitpunkt dieser Zeile, nach einer Entscheidung des Nutzers sofort. Ohne
`response`-Zeile entsteht eine leere `200`.

```json
{"t_ms":1500,"type":"response","flow_id":"018f…","status":201,"headers":[["content-type","application/json"]],"body":"{…}","streaming":false}
```

### `passthrough`

Ein vollständiger Durchreiche-Flow zum Sprachmodell in einer Zeile: Anfrage,
Entscheidung mit Herkunft `passthrough`, Antwort, Aufzeichnung.

```json
{"t_ms":2600,"type":"passthrough","flow_id":"018f…","method":"POST","host":"192.168.1.50","port":11434,"path":"/api/chat","body":"{…}","response_status":200,"response_body":"{…}"}
```

### `diagnostic`

Ein sitzungsweiter Befund, ohne Flow. `code` muss im Register stehen
(`daemon/crates/core-types/src/diagnostics/codes.rs`), sonst lehnt der Fake die
Datei ab. `severity` ist `info`, `warning` (Vorgabe), `error` oder `blocking`.
`fix` kennt `set_env`, `change_setting`, `copy_command` und `open_url`.

```json
{"t_ms":8000,"type":"diagnostic","code":"TLS_001","severity":"warning","why":"…","fix":{"set_env":{"key":"CURL_CA_BUNDLE","value":"/etc/humanitl/ca.crt"}}}
```

## Zeit

`--speed N` teilt alle `t_ms` durch `N`. Wartezeiten aus `hold` werden dabei
**nicht** gerafft — eine Frist ist Wanduhrzeit, unabhängig vom Abspieltempo.
Wer auch sie raffen will, gibt zusätzlich `--scale-timeouts` an.

`--loop` startet die Datei nach dem Ende neu. Die Flow-Ids behalten dabei ihren
zufälligen Teil, bekommen aber den Zeitstempel des neuen Durchlaufs: sie
bleiben wiedererkennbar und sortieren weiterhin nach Zeit.

## Alle Flaggen

| Flagge | Vorgabe | Wofür |
|---|---|---|
| `--fake <FILE>` | — | die Sitzungsdatei; ohne sie tut `humanitld` nichts |
| `--speed <N>` | `1` | teilt alle `t_ms` durch `N`; eine endliche Zahl über null |
| `--loop` | aus | Datei nach dem Ende neu starten |
| `--scale-timeouts` | aus | auch die Wartezeiten mit `--speed` raffen |
| `--hold-timeout-secs <N>` | `300` | Wartezeit für `hold`-Zeilen ohne eigenen Wert |
| `--event-buffer <N>` | `1024` | Kapazität des Rundfunks, mindestens 1; darüber gibt es `Lagged` |
| `--socket <PATH>` | XDG | abweichender Socket; die Token-Datei liegt daneben |

Ohne `--socket` liegen Socket und Token dort, wo sie der echte Daemon anlegt:
`$XDG_RUNTIME_DIR/humanitl/daemon.sock` und `…/token`, das Verzeichnis `0700`,
beide Dateien `0600`. Mit `--socket` gehört das Verzeichnis dem Nutzer: gibt es
das schon, behält es seine Rechte; fehlt es, wird es mit `0700` angelegt. Socket
und Token sind auch dort `0600`. Ist der Socket bereits belegt oder der Pfad
länger als `sun_path` erlaubt (107 Bytes), endet der Start mit einem Befund,
bevor irgendetwas angelegt wird. `SIGTERM` und `SIGINT` beenden den Fake und
räumen Socket und Token wieder weg.

## Die mitgelieferten Sitzungen

### `npm-install.jsonl`

Fünfzehn `GET` an `registry.npmjs.org` zwischen 100 ms und 19,4 s: elf
Metadaten-Abfragen und vier Tarballs. Alle werden gehalten, keine Funde. Das
ist der Fall, für den die Oberfläche gruppieren und im Stapel entscheiden
können muss.

### `mixed.jsonl`

Sieben Vorgänge, jeder für einen anderen Bildschirm:

1. `POST api.github.com/graphql` mit einem GitHub-Token im `Authorization`-Header
   und einer E-Mail-Adresse im Body — zwei Funde, wird gehalten.
2. `GET models.dev/api.json` — von der mitgelieferten Regel
   `018f0000-0000-7000-8000-0000000000a1` geblockt, mit Notiz an den Agenten.
3. `POST 192.168.1.50:11434/api/chat` — Durchreiche zum Sprachmodell.
4. `GET example.org/` mit einer Frist von fünf Sekunden — läuft in den Timeout,
   wenn niemand entscheidet.
5. `POST httpbin.org/post` mit einem JWT im Body — gedacht zum Bearbeiten der
   Anfrage („Allow edited").
6. Ein `TLS_001`-Befund samt Vorschlag, `CURL_CA_BUNDLE` zu setzen.
7. Ein WebSocket-Upgrade auf `wss://ws.example.org/agent`, ebenfalls gehalten.

Die Schlüssel und Adressen in beiden Dateien sind erfunden. Sie sehen echt
genug aus, damit die Detektoren aus HUM-021 später an ihnen geprüft werden
können.
