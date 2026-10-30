---
source: reference/limits.md
source-sha256: fe83011e48c4f2942396941b99b0287e30c60b78e3385ff8fb7c92f4e6ee23cb
lang: de
---
# Ressourcengrenzen

Applies to: Humanitl 0.1

Jede harte Grenze des Daemons lebt in der Gruppe `limits` von `config.toml`
(`backlog/CONVENTIONS.md` 4.4, HUM-057). Jede hat eine Vorgabe, die zu einem
Coding-Agenten auf einem Laptop passt, und jede Überschreitung endet in einem
festen Zustand mit einer Diagnose, nie in einem stillen Abbruch.

Namen, Vorgaben und Stufen legt die Ausgabe von `humanitl config schema` fest
(und die daraus erzeugte [`CONFIG.md`](../CONFIG.md)); diese Seite erklärt, was
jede Grenze tut. Werte setzt du mit `humanitl config set limits.<key> <value>`
oder im Einstellungsdialog (Stufen `advanced` und `expert`).

## Die elf Grenzen

| Schlüssel | Vorgabe | Stufe | Bei Überschreitung | Diagnose |
|---|---|---|---|---|
| `limits.hold_body_cap_bytes` (Alias `hold.body_cap_bytes`) | 32 MiB | advanced | Die Anfrage wird mit `413` blockiert (`BlockReason::BodyCap`), auch wenn eine Regel den Host erlaubt. Ausnahme: Eine `allow`- oder `ask`-Regel mit `stream: true` lässt sie gestreamt und ungeprüft hinaus, aber nur solange `hold.hard_block_checksum_secrets` aus ist (siehe unten). | `LIMIT_001` (`Warning` beim Blockieren mit einem Regelvorschlag, `Info` beim Streamen) |
| `limits.preview_cap_bytes` (Alias `preview.cap_bytes`) | 8 MiB | expert | Der Secret-Scan und die Vorschau sehen nur den Anfang; die Entscheidung bleibt möglich, die Funde gelten als unvollständig. | `FINDINGS_002` |
| `limits.event_buffer` (Alias `ipc.event_buffer`) | 1024 | expert | Ein Abonnent des Ereignisstroms, der mehr Ereignisse verpasst, bekommt `Lagged` und lädt über `ListFlows` neu. Der Proxy wartet nie auf ihn. | `LIMIT_006` (nur im Log, `Info`) |
| `limits.max_decompress_ratio` (Alias `preview.max_decompress_ratio`) | 100 | expert | Das Dekodieren eines Bodys für Scan und Vorschau bricht ab, sobald mehr herauskäme als das Verhältnis mal die gepackte Größe; geprüft wird beim Dekodieren, nicht danach. | `LIMIT_003` (keine Abhilfe), `FINDINGS_002`, wenn statt des Verhältnisses die Vorschau-Grenze griff |
| `limits.hold_max_flows` | 200 | advanced | Eine neue Anfrage, die angehalten werden müsste, wird mit `503` blockiert (`BlockReason::HoldMaxFlows`); keine angehaltene Anfrage wird verdrängt. Ab 80 % zeigt die Statusleiste die Auslastung bernsteinfarben, bei 100 % rot. | `LIMIT_002` |
| `limits.hold_max_bytes` | 256 MiB | advanced | Summe aller Bodys in Bearbeitung: reserviert beim Puffern, freigegeben nach Weiterleiten, Blockieren, Zeitüberschreitung oder Abbruch. Eine Anfrage, die nicht mehr hineinpasst, bekommt `503` (`BlockReason::HoldMemory`), auch mitten im Puffern. Ein angehaltener Kopf unter `stream: true` zählt nur das schon gelesene Präfix. | `LIMIT_002` |
| `limits.connect_timeout_secs` (Alias `upstream.connect_timeout_secs`) | 10 s | advanced | Der Verbindungsaufbau zum Ziel bricht ab; der Flow wird `Failed`, der Client bekommt `502`. | `PROXY_003` |
| `limits.header_timeout_secs` | 30 s | expert | Ein Client, der seinen Anfragekopf nicht in dieser Zeit sendet, wird getrennt; es entsteht kein Flow. Deckt auch die Pause vor der nächsten Anfrage auf einer Keep-alive-Verbindung und den TLS-Handshake nach `CONNECT` ab, nie während eine Anfrage angehalten ist. Bei einem gestreamten Upload auch die Zeit vom Ende des Bodys bis zu den Antwort-Headern des Ziels. | `LIMIT_005` (nur im Log), `LIMIT_004` (Upload ohne Antwort) |
| `limits.body_timeout_secs` | 300 s | expert | Stille **zwischen zwei Chunks** eines Bodys, nie die Gesamtdauer. Anfrage-Body: `408` mit `Connection: close`. Gestreamter Antwort-Body: Der Strom wird abgeschnitten und als gekürzt aufgezeichnet. Gestreamter Upload, den das Ziel nicht mehr liest: Er scheitert. | `PROXY_011` (Anfrage), `LIMIT_004` (Antwort, Upload) |
| `limits.recorder_max_body_bytes` (Alias `recorder.max_body_bytes`) | 32 MiB | expert | Was darüber hinausgeht, speichert die Aufzeichnung nicht; die Zeile trägt `truncated` und die volle Größe. | keine, Markierung `truncated` am Body |
| `limits.max_client_connections` | 256 | expert | Die Accept-Schleife antwortet mit `503` und `Connection: close`, statt die Verbindung anzunehmen und liegen zu lassen. | `PROXY_010` (einmal pro Minute zusammengefasst) |

Die Spezifikation von HUM-057 nannte drei Schlüssel anders
(`limits.hold_memory_cap_bytes`, `limits.upstream_connect_timeout_secs`,
`limits.client_header_timeout_secs`) und zwei weitere
(`limits.client_body_timeout_secs`, `limits.response_idle_timeout_secs`), die
HUM-120 zu `limits.body_timeout_secs` zusammengelegt hat. Es gelten die Namen in
dieser Tabelle (`backlog/CONVENTIONS.md` 4.4).

## `stream: true`

Der Anfrage-Body wird vor jeder Entscheidung vollständig gepuffert, weil ein
Mensch sonst nicht genau den Teil sieht, in dem Daten hinausgehen. Eine Anfrage
über `limits.hold_body_cap_bytes` passt nicht in diesen Puffer und wird
blockiert. Eine Regel mit `stream: true` ist die einzige Ausnahme, und der Mensch
schreibt sie selbst:

```yaml
- action: ask          # or allow
  match:
    host: uploads.example.com
    method: [POST]
    path_prefixes: ["/v1/files"]
  stream: true
```

Mit `ask` hält der Proxy die Anfrage mit ihrem Kopf an, und erst eine Zustimmung
lässt den Body fließen. Mit `allow` geht sie sofort. In beiden Fällen gibt es
keinen Secret-Scan und keine Vorschau; der Body wird bis
`limits.recorder_max_body_bytes` aufgezeichnet. `LIMIT_001` beim Blockieren trägt
den Vorschlag einer solchen Regel, immer mit `ask`. Ist
`hold.hard_block_checksum_secrets` an, greift `stream: true` nie: Ein ungeprüfter
Strom kann das harte Blockieren nicht einhalten. Die Anfrage wird dann blockiert
wie ohne Regel, und `LIMIT_001` schlägt statt einer Regel die Grenze vor. Die
Sicherheitsaussage steht in [`SECURITY.md`](../SECURITY.md#6-the-proxy), das
Regelfeld in [`rules.md`](rules.md).

## Belegt durch

- `daemon/crates/proxy/tests/limits.rs`: Blockieren an der Grenze mit `LIMIT_001`,
  Streamen unter `stream: true`, eine stumme Antwort mit `LIMIT_004`, die Frist
  für den Kopf mit `LIMIT_005`.
- `daemon/crates/proxy/tests/hold.rs`: `hold_rejects_over_memory`,
  `hold_rejects_over_count`.
- `daemon/crates/proxy/tests/timeouts.rs`: die Zeitspannen von HUM-120.
- `daemon/crates/findings/tests/regressions/main.rs`: Dekompressionsbomben,
  `LIMIT_003` gegenüber `FINDINGS_002`.
- `daemon/crates/ipc/src/server.rs`: `subscriber_lag_gets_lagged_event`.
- `daemon/crates/proxy/tests/backpressure.rs`: 1.000 Flows in 10 s gegen einen
  langsamen Abonnenten, in der CI mit Messwerten im Job-Log.
