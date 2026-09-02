# ADR-0005 · Request-Body vollständig puffern, bevor der Mensch entscheidet
Status: Accepted
Datum: 2026-09-02

## Kontext

Der Mensch entscheidet über einen Request. Damit diese Entscheidung etwas wert
ist, muss er sehen, worüber er entscheidet. Bei einem Exfiltrationsversuch steht
das Interessante fast nie in den Headern: Der Host ist `api.github.com`, die
Methode ist `POST`, der Pfad ist unauffällig — und im Body steht der Inhalt von
`.env`, base64-kodiert.

Ein Proxy, der nur die Header prüft und den Body durchstreamt, während der
Mensch noch liest, gibt also genau den Teil frei, um dessentwillen es das
Werkzeug gibt. Die Freigabe muss vor dem ersten Byte Body liegen, das den Host
verlässt.

Dem steht Speicher entgegen: Ein gepufferter 40-MB-Upload liegt im Daemon, und
zweihundert gleichzeitig gehaltene Requests können den Rechner an die Wand
fahren. Außerdem gibt es `Expect: 100-continue`: Ein Client, der so fragt,
schickt den Body erst nach einer Zwischenantwort — wer diese Zwischenantwort
zurückhält, bekommt den Body nie zu sehen und kann ihn folglich auch nicht
anzeigen.

## Entscheidung

Der Request-Body wird vollständig gepuffert, bevor der Flow in `Held` geht. Erst
danach wird der Request im UI angezeigt. Nichts erreicht den Upstream vor der
Entscheidung.

- **Body-Cap** `limits.hold_body_cap_bytes` (Alias `hold.body_cap_bytes`),
  Default 32 MiB. Über dem Cap wird mit `413` geblockt
  (`BlockReason::BodyCap`), außer eine matchende Regel setzt ausdrücklich
  `stream: true`.
- **`Expect: 100-continue`** beantwortet der Proxy sofort mit `100 Continue`.
  Der Body fließt dann in den Hold-Puffer des Proxys, nicht zum Upstream. Erst
  die Entscheidung öffnet den Weg nach draußen.
- **Hold-Speicherbudget** ist global: `limits.hold_max_bytes` (Default 256 MiB)
  und `limits.hold_max_flows` (Default 200), als atomare Zähler in der
  `HoldQueue`. Wird ein Budget überschritten, wird der *neue* Request mit `503`
  abgewiesen (`BlockReason::HoldMemory` beziehungsweise
  `BlockReason::HoldMaxFlows`). Ein bereits gehaltener Request wird nie
  verworfen.
- **Responses** werden immer gestreamt (LLM-Streaming, SSE funktionieren) und
  parallel in den Recorder gespiegelt.

Statuscodes je Blockgrund, verbindlich:

| Grund | Status |
|---|---|
| `User`, `Rule`, `AuthorityMismatch`, `PrivateAddress` | `403` |
| `BodyCap` | `413` |
| `Timeout` (Hold-Timeout) | `504` |
| `HoldMemory`, `HoldMaxFlows` | `503` |
| Upstream-Fehler (`FlowState::Failed`) | `502` |
| `NoRoute` | `502` |
| `ClientTimeout` | `408` |

Der Antwort-Body ist in allen Fällen gleich aufgebaut, `text/plain`:

```
Blocked by Humanitl.
reason: <BlockReason als snake_case>
flow: <FlowId>
host: <host>
```

## Begründung

Wer nur Header freigibt und den Body streamt, sieht genau den Teil nicht, in dem
exfiltriert wird. Das ist der ganze Grund. Alles andere in dieser Entscheidung
sind Folgekosten, die man bezahlt, damit dieser Satz stimmt.

Der Cap begrenzt die Kosten. 32 MiB decken normale API-Aufrufe, Datei-Uploads
mittlerer Größe und LLM-Prompts mit Kontext ab. Darüber ist ein Block mit `413`
die ehrliche Antwort: „Ich kann dir das nicht zeigen, also lasse ich es nicht
durch." Der Ausweg `stream: true` ist eine bewusste, benannte Regel — der Nutzer
gibt eine Route explizit frei, statt dass das System still eine Ausnahme macht.

Das globale Budget schützt gegen die zweite Angriffsform: nicht ein großer
Request, sondern tausend mittlere. Zähler statt Verdrängung, weil das Verwerfen
eines gehaltenen Requests eine Entscheidung wäre, die der Nutzer nicht getroffen
hat.

Der einheitliche Body-Aufbau ist Agent-Ergonomie. Ein Agent, der lernt, dass
`Blocked by Humanitl.` endgültig ist, muss das genau einmal lernen und nicht
einmal pro Fehlerklasse. Er kann die Zeile `reason:` maschinell auswerten und
seinem Nutzer sagen, was los ist. ADR-0014 baut darauf auf und erweitert den
Body um `note:`.

Die Behandlung von `Expect: 100-continue` ist eine **Korrektur** einer früheren
Fassung dieses ADR. Dort stand „`100 Continue` erst nach der Entscheidung". Das
wäre in sich widersprüchlich gewesen: Ohne `100 Continue` schickt der Client den
Body nicht, ohne Body gibt es nichts anzuzeigen, ohne Anzeige keine
Entscheidung. Das sofortige `100 Continue` ist sicher, weil es nur bedeutet
„schick mir den Body", nicht „ich leite ihn weiter". Die Formulierung in
`backlog/CONVENTIONS.md` 4.10 ist maßgeblich.

## Verworfene Alternativen

- **Nur Header halten, Body streamen.** Der bequeme Weg, und der einzige, der
  ohne Speicherbudget auskommt. Verliert, weil er das Kernversprechen bricht:
  Der Mensch entschiede über eine Zusammenfassung, während der Inhalt bereits
  unterwegs ist.
- **Body streamen und nachträglich analysieren.** Ein Alarm nach dem Abfluss ist
  kein Schutz, sondern ein Protokoll. Für die DSGVO-Situation der Zielgruppe ist
  „wir haben gemerkt, dass Kundendaten abgeflossen sind" der Schadensfall, nicht
  seine Vermeidung.
- **Nur die ersten N Kilobyte puffern und anzeigen.** Ein Angreifer legt seine
  Nutzlast hinter Byte N+1. Für die *Anzeige* großer Bodies gibt es eine
  gestaffelte Darstellung (Größe, erste 64 KB, Scan-Status, HUM-030), aber die
  *Freigabe* bezieht sich immer auf den vollständig vorliegenden Body.
- **Auf Platte puffern statt im Speicher.** Hebt den Cap, kostet aber IO im
  heißen Pfad, schreibt potenziell sensible Bodies unverschlüsselt auf die Platte
  und verschiebt das Problem nur. Bei Bedarf später, hinter demselben Budget.
- **Gehaltene Requests bei Speichernot verdrängen.** Verworfen: Der Nutzer würde
  Requests aus seiner Warteschlange verschwinden sehen, ohne sie entschieden zu
  haben. `503` für den Neuankömmling ist die verständlichere Regel.

## Konsequenzen

- Der Speicherbedarf des Daemons ist durch `limits.hold_max_bytes` nach oben
  begrenzt und damit vorhersagbar. Die Zähler entstehen bereits in HUM-016, nicht
  erst in der Härtungsphase; HUM-057 tunt sie nur noch.
- Sehr große Uploads funktionieren nur mit einer ausdrücklichen `stream`-Regel.
  Das ist eine bewusste Reibung, die im UI erklärt wird.
- `Expect: 100-continue` ist ein eigener Testfall der Konformitäts-Matrix
  (HUM-017): `100 Continue` geht sofort raus, aber der Upstream sieht bis zur
  Entscheidung nichts.
- Der Block-Body ist Teil des öffentlichen Verhaltens und damit
  regressionsgetestet. Änderungen daran sind Vertragsänderungen.
- Dekompression von Bodies für die Vorschau braucht ein eigenes Ratio-Limit
  (`limits.max_decompress_ratio`, Default 100) und einen eigenen Vorschau-Cap
  (`limits.preview_cap_bytes`, Default 8 MiB), sonst wäre der Cap über eine
  Zip-Bombe umgehbar.

## Betroffene Issues

`HUM-015` (MITM-Proxy-Kern mit Pufferung und `Expect`-Behandlung), `HUM-016`
(Hold-Queue mit Budget-Zählern), `HUM-017` (Konformitäts-Matrix, große Bodies
und `Expect`), `HUM-030` (Body-Ansichten für große Bodies), `HUM-057`
(Ressourcen-Limits und Dekompressions-Ratio).
