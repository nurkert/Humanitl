# Ressourcen-Limits

Alle harten Grenzen des Daemons stehen in der Gruppe `limits` von `config.toml`
(`backlog/CONVENTIONS.md` 4.4, HUM-057). Jede hat eine Vorgabe, die für einen
Coding-Agenten auf einem Laptop passt, und jede Überschreitung endet in einem
festen Zustand mit einem Befund, nie in einem stillen Abbruch.

Verbindlich für Namen, Vorgaben und Stufen ist die Ausgabe von
`humanitl config schema`; diese Seite erklärt, was die Grenze bewirkt. Werte
setzt `humanitl config set limits.<schlüssel> <wert>` oder der
Einstellungsdialog (ab Stufe `advanced` beziehungsweise `expert`).

## Die elf Grenzen

| Schlüssel | Vorgabe | Stufe | Wirkung beim Überschreiten | Befund |
|---|---|---|---|---|
| `limits.hold_body_cap_bytes` (Alias `hold.body_cap_bytes`) | 32 MiB | advanced | Die Anfrage wird mit `413` geblockt (`BlockReason::BodyCap`), auch wenn eine Regel den Host erlaubt. Ausnahme: Eine Regel `allow` oder `ask` mit `stream: true` lässt sie gestreamt hinaus, ungeprüft, aber nur bei ausgeschaltetem `hold.hard_block_checksum_secrets` (siehe unten). | `LIMIT_001` (`Warning` beim Block mit Regelvorschlag, `Info` beim Streamen) |
| `limits.preview_cap_bytes` (Alias `preview.cap_bytes`) | 8 MiB | expert | Suche nach Geheimnissen und Vorschau sehen nur den Anfang; die Entscheidung bleibt möglich, die Funde gelten als unvollständig. | `FINDINGS_002` |
| `limits.event_buffer` (Alias `ipc.event_buffer`) | 1024 | expert | Ein Abonnent des Ereignisstroms, der mehr Ereignisse verpasst, bekommt `Lagged` und lädt über `ListFlows` nach. Der Proxy wartet nie auf ihn. | `LIMIT_006` (nur Protokoll, `Info`) |
| `limits.max_decompress_ratio` (Alias `preview.max_decompress_ratio`) | 100 | expert | Das Entpacken eines Bodys für Suche und Vorschau bricht ab, sobald mehr entstünde als Verhältnis mal gepackte Größe; geprüft während des Entpackens, nicht danach. | `LIMIT_003` (ohne Fix), `FINDINGS_002`, wenn statt des Verhältnisses das Vorschau-Cap zog |
| `limits.hold_max_flows` | 200 | advanced | Eine neue Anfrage, die gehalten werden müsste, wird mit `503` geblockt (`BlockReason::HoldMaxFlows`); keine gehaltene wird verdrängt. Ab 80 % zeigt die Statusleiste die Auslastung amber, bei 100 % rot. | `LIMIT_002` |
| `limits.hold_max_bytes` | 256 MiB | advanced | Summe aller Bodies in Arbeit: reserviert schon beim Puffern, frei nach Weiterleitung, Block, Zeitüberschreitung oder Abbruch. Passt eine Anfrage nicht mehr hinein, `503` (`BlockReason::HoldMemory`), auch mitten im Puffern. Ein gehaltener Kopf unter `stream: true` zählt nur sein schon gelesenes Präfix. | `LIMIT_002` |
| `limits.connect_timeout_secs` (Alias `upstream.connect_timeout_secs`) | 10 s | advanced | Der Verbindungsaufbau zum Ziel bricht ab; der Fluss wird `Failed`, der Client bekommt `502`. | `PROXY_003` |
| `limits.header_timeout_secs` | 30 s | expert | Ein Client, der seinen Anfrage-Kopf nicht in dieser Zeit schickt, wird getrennt; ein Fluss entsteht nicht. Gilt auch für die Lücke bis zur nächsten Anfrage einer Keep-Alive-Verbindung und für den TLS-Handschlag nach `CONNECT`, nie während eine Anfrage gehalten wird. Bei einem gestreamten Upload zusätzlich die Frist vom Ende des Bodys bis zu den Antwort-Kopfzeilen des Ziels. | `LIMIT_005` (nur Protokoll), `LIMIT_004` (Upload ohne Antwort) |
| `limits.body_timeout_secs` | 300 s | expert | Stille **zwischen zwei Stücken** eines Bodys, nie die Gesamtdauer. Anfrage-Body: `408` mit `Connection: close`. Gestreamter Antwort-Body: Der Strom wird abgeschnitten und als gekürzt aufgezeichnet. Gestreamter Upload, den das Ziel nicht mehr liest: Er scheitert. | `PROXY_011` (Anfrage), `LIMIT_004` (Antwort, Upload) |
| `limits.recorder_max_body_bytes` (Alias `recorder.max_body_bytes`) | 32 MiB | expert | Was darüber liegt, legt die Aufzeichnung nicht ab; die Zeile trägt `truncated` und die volle Größe. | keiner, Vermerk `truncated` am Body |
| `limits.max_client_connections` | 256 | expert | Der Accept-Loop antwortet `503` mit `Connection: close`, statt anzunehmen und liegen zu lassen. | `PROXY_010` (zusammengefasst je Minute) |

Die Spezifikation von HUM-057 nannte drei Schlüssel anders
(`limits.hold_memory_cap_bytes`, `limits.upstream_connect_timeout_secs`,
`limits.client_header_timeout_secs`) und zwei weitere
(`limits.client_body_timeout_secs`, `limits.response_idle_timeout_secs`), die
HUM-120 zu `limits.body_timeout_secs` zusammengelegt hat. Es gelten die Namen
dieser Tabelle (`backlog/CONVENTIONS.md` 4.4).

## `stream: true`

Der Anfrage-Body wird vor jeder Entscheidung vollständig gepuffert, weil ein
Mensch sonst genau den Teil nicht sieht, in dem exfiltriert wird. Eine Anfrage
über `limits.hold_body_cap_bytes` passt nicht in diesen Puffer und wird
geblockt. Eine Regel mit `stream: true` ist die einzige Ausnahme, und der
Mensch schreibt sie selbst:

```yaml
- action: ask          # oder allow
  match:
    host: uploads.example.com
    method: [POST]
    path_prefixes: ["/v1/files"]
  stream: true
```

Mit `ask` hält der Proxy die Anfrage mit ihrem Kopf, und erst eine Freigabe
lässt den Body fließen. Mit `allow` geht er sofort. In beiden Fällen gibt es
keine Suche nach Geheimnissen und keine Vorschau; aufgezeichnet wird der Body
bis `limits.recorder_max_body_bytes`. Den Vorschlag einer solchen Regel trägt
`LIMIT_001` am Block, immer mit `ask`. Steht `hold.hard_block_checksum_secrets`
an, greift `stream: true` nie: Ein ungeprüfter Strom kann die harte Sperre nicht
einhalten. Die Anfrage wird dann geblockt wie ohne Regel, und `LIMIT_001`
schlägt das Cap vor statt einer Regel. Die Sicherheitsaussage dazu steht in
`docs/SECURITY.md` (Grenzen und Zeitlimits).

## Belegt durch

- `daemon/crates/proxy/tests/limits.rs`: Block am Cap mit `LIMIT_001`,
  Streamen unter `stream: true`, verstummte Antwort mit `LIMIT_004`, Kopf-Frist
  mit `LIMIT_005`.
- `daemon/crates/proxy/tests/hold.rs`: `hold_rejects_over_memory`,
  `hold_rejects_over_count`.
- `daemon/crates/proxy/tests/timeouts.rs`: die Spannen aus HUM-120.
- `daemon/crates/findings/tests/regressions/main.rs`: Entpack-Bomben,
  `LIMIT_003` gegen `FINDINGS_002`.
- `daemon/crates/ipc/src/server.rs`: `subscriber_lag_gets_lagged_event`.
- `daemon/crates/proxy/tests/backpressure.rs`: 1000 Flüsse in 10 s gegen einen
  langsamen Abonnenten, in der CI mit Messwerten im Job-Log.
