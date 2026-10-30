---
source: adr/0005-buffer-request-body.md
source-sha256: 177676fe3c6e748ad4aa343570b1c6125248a447c31442058892c0f8ac34c582
lang: de
---
# ADR-0005 · Den Body der Anfrage vollständig puffern, bevor der Mensch entscheidet

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Der Mensch entscheidet über eine Anfrage. Damit diese Entscheidung etwas wert ist, muss er
sehen, worüber er entscheidet. Bei einem Ausleitungsversuch steht der interessante Teil
fast nie in den Headern: Der Host ist `api.github.com`, die Methode ist `POST`, der Pfad
ist unauffällig, und der Body enthält den Inhalt von `.env`, base64-kodiert.

Ein Proxy, der nur die Header prüft und den Body durchreicht, während der Mensch noch
liest, gibt also genau den Teil frei, um dessentwillen das Werkzeug existiert. Die
Freigabe muss vor dem ersten Byte Body kommen, das den Rechner verlässt.

Dagegen steht der Speicher: Ein gepufferter Upload von 40 MB liegt im Daemon, und
zweihundert gleichzeitig angehaltene Anfragen können den Rechner an die Wand fahren. Dazu
kommt `Expect: 100-continue`: Ein Client, der so fragt, schickt den Body erst nach einer
Zwischenantwort. Wer diese Zwischenantwort zurückhält, bekommt den Body nie zu sehen und
kann ihn folglich auch nicht anzeigen.

## Entscheidung

Der Body der Anfrage wird vollständig gepuffert, bevor der Flow nach `Held` geht. Erst dann
wird die Anfrage in der Oberfläche gezeigt. Vor der Entscheidung erreicht nichts den
Upstream.

- **Body-Obergrenze** `limits.hold_body_cap_bytes` (Alias `hold.body_cap_bytes`), Vorgabe
  32 MiB. Oberhalb der Grenze wird die Anfrage mit `413` blockiert
  (`BlockReason::BodyCap`), es sei denn, eine passende Regel setzt ausdrücklich
  `stream: true`.
- **`Expect: 100-continue`** beantwortet der Proxy sofort mit `100 Continue`. Der Body
  fließt dann in den Haltepuffer des Proxys, nicht zum Upstream. Erst die Entscheidung
  öffnet den Weg nach draußen.
- **Speicherbudget für angehaltene Anfragen** ist global: `limits.hold_max_bytes` (Vorgabe
  256 MiB) und `limits.hold_max_flows` (Vorgabe 200), als atomare Zähler in der
  `HoldQueue`. Wird ein Budget überschritten, wird die *neue* Anfrage mit `503`
  abgewiesen (`BlockReason::HoldMemory` beziehungsweise `BlockReason::HoldMaxFlows`). Eine
  bereits angehaltene Anfrage wird nie verworfen.
- **Antworten** werden immer durchgereicht (LLM-Streaming und SSE funktionieren) und
  parallel an den Recorder gespiegelt.

Statuscodes je Grund der Blockade, verbindlich:

| Grund | Status |
|---|---|
| `User`, `Rule`, `AuthorityMismatch`, `PrivateAddress` | `403` |
| `BodyCap` | `413` |
| `Timeout` (Zeitüberschreitung beim Anhalten) | `504` |
| `HoldMemory`, `HoldMaxFlows` | `503` |
| Fehler am Upstream (`FlowState::Failed`) | `502` |
| `NoRoute` | `502` |
| `ClientTimeout` | `408` |

Der Body der Antwort hat in allen Fällen dieselbe Struktur, `text/plain`:

```
Blocked by Humanitl.
reason: <BlockReason as snake_case>
flow: <FlowId>
host: <host>
```

## Begründung

Wer nur Header freigibt und den Body durchreicht, sieht genau den Teil nicht, in dem die
Ausleitung geschieht. Das ist der ganze Grund. Alles andere in dieser Entscheidung sind
Folgekosten, bezahlt, damit dieser Satz gilt.

Die Obergrenze begrenzt die Kosten. 32 MiB decken normale API-Aufrufe, Datei-Uploads
mittlerer Größe und LLM-Prompts mit Kontext ab. Darüber ist eine Blockade mit `413` die
ehrliche Antwort: „Ich kann dir das nicht zeigen, also lasse ich es nicht durch." Der
Ausweg, `stream: true`, ist eine bewusste, benannte Regel: Der Nutzer gibt eine Route
ausdrücklich frei, statt dass das System still eine Ausnahme macht.

Das globale Budget schützt vor der zweiten Form des Angriffs: nicht eine große Anfrage,
sondern tausend mittlere. Zähler statt Verdrängung, weil das Verwerfen einer angehaltenen
Anfrage eine Entscheidung wäre, die der Nutzer nicht getroffen hat.

Die einheitliche Struktur des Body dient der Ergonomie für den Agenten. Ein Agent, der
lernt, dass `Blocked by Humanitl.` endgültig ist, muss das genau einmal lernen, nicht
einmal je Fehlerklasse. Er kann die Zeile `reason:` maschinell auswerten und seinem Nutzer
sagen, was los ist. ADR-0014 baut darauf auf und erweitert den Body um `note:`.

Der Umgang mit `Expect: 100-continue` ist eine **Korrektur** einer früheren Fassung dieser
ADR. Dort stand „`100 Continue` erst nach der Entscheidung". Das hätte sich selbst
widersprochen: Ohne `100 Continue` schickt der Client den Body nicht, ohne Body gibt es
nichts anzuzeigen, ohne Anzeige keine Entscheidung. Das sofortige `100 Continue` ist
sicher, weil es nur „schick mir den Body" bedeutet, nicht „ich leite ihn weiter".
Maßgeblich ist die Formulierung in `backlog/CONVENTIONS.md` 4.10.

## Verworfene Alternativen

- **Nur die Header anhalten, den Body durchreichen.** Der bequeme Weg und der einzige, der
  ohne Speicherbudget auskommt. Verliert, weil er das Kernversprechen bricht: Der Mensch
  würde über eine Zusammenfassung entscheiden, während der Inhalt schon unterwegs ist.
- **Den Body durchreichen und hinterher analysieren.** Ein Alarm nach dem Abfluss ist kein
  Schutz, sondern ein Protokoll. Für die DSGVO-Lage der Zielgruppe ist „wir haben bemerkt,
  dass Kundendaten abgeflossen sind" der Schadensfall, nicht seine Verhinderung.
- **Puffern und nur die ersten N Kilobyte anzeigen.** Ein Angreifer legt seine Nutzlast
  hinter Byte N+1. Für die *Anzeige* großer Bodies gibt es eine gestaffelte Darstellung
  (Größe, die ersten 64 KB, Stand des Scans, HUM-030), aber die *Freigabe* bezieht sich
  immer auf den Body, wie er vollständig vorliegt.
- **Auf der Platte statt im Speicher puffern.** Hebt die Obergrenze auf, kostet aber IO im
  heißen Pfad, schreibt womöglich sensible Bodies unverschlüsselt auf die Platte und
  verschiebt das Problem nur. Später bei Bedarf, hinter demselben Budget.
- **Angehaltene Anfragen bei Speicherdruck verdrängen.** Verworfen: Der Nutzer sähe
  Anfragen aus seiner Warteschlange verschwinden, ohne über sie entschieden zu haben. `503`
  für den Neuankömmling ist die verständlichere Regel.

## Folgen

- Der Speicherbedarf des Daemons ist nach oben durch `limits.hold_max_bytes` begrenzt und
  damit vorhersehbar. Die Zähler entstehen schon in HUM-016, nicht erst in der
  Härtungsphase; HUM-057 stimmt sie nur ab.
- Sehr große Uploads funktionieren nur mit einer ausdrücklichen `stream`-Regel. Das ist
  bewusste Reibung, die in der Oberfläche erklärt wird.
- `Expect: 100-continue` ist ein eigener Testfall in der Konformitätsmatrix (HUM-017):
  `100 Continue` geht sofort hinaus, aber der Upstream sieht bis zur Entscheidung nichts.
- Der Block-Body gehört zum öffentlichen Verhalten und wird deshalb mit Regressionstests
  abgesichert. Änderungen daran sind Vertragsänderungen.
- Das Dekomprimieren von Bodies für die Vorschau braucht eine eigene Grenze für das
  Verhältnis (`limits.max_decompress_ratio`, Vorgabe 100) und eine eigene Obergrenze für
  die Vorschau (`limits.preview_cap_bytes`, Vorgabe 8 MiB), sonst ließe sich die
  Obergrenze mit einer Zip-Bombe umgehen.

## Betroffene Issues

`HUM-015` (Kern des MITM-Proxys mit Puffern und Behandlung von `Expect`), `HUM-016`
(Warteschlange der angehaltenen Anfragen mit Budgetzählern), `HUM-017`
(Konformitätsmatrix, große Bodies und `Expect`), `HUM-030` (Body-Ansichten für große
Bodies), `HUM-057` (Ressourcengrenzen und Dekompressionsverhältnis).
