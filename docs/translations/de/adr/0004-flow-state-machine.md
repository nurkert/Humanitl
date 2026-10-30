---
source: adr/0004-flow-state-machine.md
source-sha256: 7a366cce003bed8bf6d126f95e9248f513ff45f547aaeadc70a97691aa789867
lang: de
---
# ADR-0004 · Lebenszyklus einer Anfrage als Zustandsmaschine, Ereignisse werden abgeleitet

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

In Humanitl durchläuft eine Anfrage mehr Stationen als in einem gewöhnlichen Proxy: Sie kommt
an, wird auf Funde untersucht, gegen Regeln ausgewertet, unter Umständen minutenlang
angehalten, entschieden, weitergeleitet, beantwortet und aufgezeichnet. Fünf Abnehmer wollen
jederzeit wissen, wo sie steht: der Proxy-Handler selbst, die Warteschlange, die Oberfläche,
der Recorder und das Audit-Log.

Ohne ausdrückliche Modellierung entsteht genau die Art Code, die sich nach zwei Jahren nicht
mehr korrigieren lässt: verstreute boolesche Flags (`is_held`, `was_decided`,
`response_started`), deren erlaubte Kombinationen niemand aufschreibt, und Zustände, die es
in der Oberfläche gibt, aber nicht im Daemon. Dazu kommt die Frage, ob Fehler nach der
Freigabe (DNS scheitert, TCP-Verbindungsaufbau scheitert, TLS-Handshake scheitert) als
„Antwort" zählen. Wenn ja, sieht jemand, der das Audit liest, ein `502` und kann nicht
unterscheiden, ob der Server das gesagt hat oder wir.

## Entscheidung

Der Lebenszyklus ist ein `enum FlowState` in `humanitl-core` mit genau einer Methode, die
ihn ändert. `Flow` hält Zustand und Historie und ruft sie auf:

```rust
pub struct Transition {
    pub flow_id: FlowId,
    pub at: SystemTime,
    pub input: TransitionInput,
}

impl FlowState {
    pub fn on(self, t: Transition) -> Result<(FlowState, FlowEvent), InvalidTransition>;
}

impl Flow {
    pub fn apply(&mut self, input: TransitionInput, at: SystemTime)
        -> Result<FlowEvent, InvalidTransition>;
}
```

`Transition` ist ein Umschlag: `TransitionInput` sagt, was geschehen soll; `flow_id` und
`at` kommen vom Aufrufer, weil die reine Zustandsmaschine weder IDs erfinden noch eine Uhr
lesen darf, das erzeugte Ereignis aber beides trägt. `TransitionInput` hat die Varianten
`Analyze` (mit den Funden der Detektoren), `Hold { deadline, queue_bytes, queue_count }`,
`Decide { decision, source }`, `Forward`, `Respond { status }`, `Record`,
`Answer(MetaAnswer)`, `Timeout` und `Fail { error }`; dazu Konstruktoren
`Transition::analyze(..)` usw. `Flow::apply` kennt seine eigene ID, ruft `on` auf und
ersetzt Zustand und Historie; bei einem ungültigen Übergang bleiben beide unverändert
(`backlog/CONVENTIONS.md` 4.11). `FlowEvent` ist die abgeleitete Ausgabe. Zustände:

```
Received → Analyzed{findings} → Held{deadline} → Decided(Allow | AllowEdited | Block | TimedOut)
         → Forwarded → Responded{status} → Recorded
```

Dazu `Failed{error: UpstreamError}` mit
`UpstreamError = Dns | Connect | Tls | PrivateAddress(IpAddr) | Timeout`, erreichbar aus
`Decided(Allow | AllowEdited)` und aus `Forwarded`, also aus allen Zuständen nach der
Freigabe. `Failed` geht nach `Recorded`.

Erlaubt sind genau: `Received→Analyzed→Held→Decided→Forwarded→Responded→Recorded`,
`Held→Decided(TimedOut)→Recorded`, `Decided(Block)→Recorded`,
`Analyzed→Decided` (automatische Regelentscheidung, überspringt `Held`),
`Decided(Allow|AllowEdited)→Failed`, `Forwarded→Failed`, `Failed→Recorded`.
Alles andere liefert `InvalidTransition { from, input }`.

### Nachtrag (HUM-103, 2026-09-05): `Received→Recorded` für den Meta-Endpunkt

Es gibt genau einen weiteren erlaubten Übergang: `Received→Recorded` über
`Answer(MetaAnswer)`. Er gilt für Anfragen an den reservierten Namen `humanitl.internal`, die
der Proxy selbst beantwortet (ADR-0014). Eine solche Anfrage geht nirgendwohin, hält nichts
auf, und niemand entscheidet über sie. Sie hat deshalb keinen Weg über `Held` und keinen über
`Decided`: `decision` sagt aus, wie ein Mensch oder eine Regel über eine Anfrage entschieden
hat, und ein Eintrag mit einer erfundenen Entscheidung wäre eine Behauptung über einen
Menschen, der nichts getan hat (`backlog/CONVENTIONS.md` 4.13). Ohne diesen Übergang bliebe
die Anfrage unsichtbar, und der Verlauf zeigte weniger, als wirklich geschehen ist.

Der Übergang weicht das Versprechen nicht auf, dass jede gewöhnliche Anfrage einem Menschen
gezeigt wird. Er gilt allein für den Flow, **dessen eigene Anfrage** an den reservierten Namen
ging, und das wird an diesem Flow geprüft, im Moment des Abschlusses: `Flow::apply` weist
`Answer` zurück, sobald `Flow::is_meta` nicht gilt. Die einzige Tür dorthin ist
`Flow::answer`; sie nennt den Grund (`PROXY_009` für den falschen Host, `InvalidTransition`
für den falschen Zustand) und gibt den Zeugen nicht heraus. `MetaAnswer` selbst ist
`#[non_exhaustive]`, lässt sich also außerhalb des Crates nicht bauen, und trägt keine Daten.

Dass der Zeuge nicht wandern darf, ist der Kern: Ein Wert, der nur beweist, dass *irgendeine*
Anfrage an den reservierten Namen ging, ließe sich auf einen fremden Flow anwenden, der noch
in `Received` steht. Dort steht jede Anfrage nach ihrer Ankunft, und der Flow landete in
`Recorded`, ohne dass ein Mensch ihn je gesehen hätte. `flow_state_table.rs` zählt beide
Hälften auf (aus welchem Zustand sich der Weg öffnet und für welchen Flow); die Gegenprobe
innerhalb des Crates mit einem echten Zeugen auf einem fremden Flow steht in
`flow.rs::tests::a_witness_does_not_open_a_foreign_flow`.

Der Eintrag eines Meta-Flows trägt die Spalte `flows.meta` und **keine** `decision`. Er zählt
in keiner Auswertung, die von Entscheidungen spricht; der Filterbegriff `meta:` trennt ihn von
den anderen.

Wer entscheiden darf, hängt von der `DecisionSource` ab. `DecisionSource::System` (der Daemon
selbst, etwa wenn das Budget erschöpft ist) darf aus `Analyzed` und `Held` nur verweigern
(`Block`, `TimedOut`), nie erlauben. Nur so lassen sich `HoldMemory`, `HoldMaxFlows`,
`BodyCap` und `ClientTimeout` ausdrücken (`backlog/CONVENTIONS.md` 4.11).

Ein Fehler beim Upstream wird nie als `Responded` verbucht. In diesem Fall bekommt der Client
`502` mit dem einheitlichen Body `Blocked by Humanitl.` und `reason: upstream_*`. Die
Statuscodes je Blockiergrund gehören zu dieser Modellierung und sind identisch mit ADR-0005:

| Grund | Status |
|---|---|
| `User`, `Rule`, `AuthorityMismatch`, `PrivateAddress` | `403` |
| `BodyCap` | `413` |
| `Timeout` (Timeout beim Anhalten) | `504` |
| `HoldMemory`, `HoldMaxFlows` | `503` |
| `NoRoute` | `502` |
| `ClientTimeout` | `408` |
| Fehler beim Upstream (`FlowState::Failed`) | `502` |

Jeder Übergang erzeugt ein `FlowEvent`, das sowohl den gRPC-Stream als auch das Audit-Log
speist. Das ist bewusst *kein* Event Sourcing: Der Zustand im Speicher wird nicht aus
Ereignissen rekonstruiert. Die Wahrheit ist die Zustandsmaschine plus SQLite; Ereignisse sind
Ausgabe.

Angehaltene Anfragen blockieren nie auf die Oberfläche. Die Warteschlange ist eine
`DashMap<FlowId, oneshot::Sender<Decision>>` mit einem Frist-Timer; der Proxy-Handler
wartet mit `await` auf den Empfänger.

## Begründung

Ein `enum` mit einer einzigen ändernden Methode macht ungültige Zustände nicht bloß
unwahrscheinlich, sondern unkonstruierbar. Die Liste der erlaubten Übergänge ist endlich und
klein genug, um sie in einem tabellengetriebenen Test vollständig aufzuzählen, einschließlich
aller verbotenen Kombinationen. Ein Fehler in diesem Bereich ist deshalb ein fehlender
Testfall, kein Rätsel.

Dass das Ereignis die *Ausgabe* des Übergangs ist und nicht seine Eingabe, ist der Punkt, um
den sich alles dreht. Wäre das Ereignis die Eingabe, könnte ein Aufrufer ein Ereignis
erfinden, das nie stattgefunden hat, und der Zustand folgte ihm. So aber kann der Aufrufer
einen Übergang nur *versuchen*; das Ereignis entsteht nachweislich aus einem Übergang, der
stattgefunden hat. Für ein Audit-Log, dessen ganzer Zweck die Nachvollziehbarkeit ist, ist
diese Richtung die einzige vertretbare.

Der Handler *treibt* die Zustandsmaschine, er *besitzt* sie nicht: Er ruft `Flow::apply` auf,
veröffentlicht das Ereignis und wartet bei Bedarf auf die Entscheidung. Der Handler bleibt
dadurch dünn und die Logik ohne Netzwerk testbar.

`Failed` als eigener Zustand statt `Responded{502}` hält die Aufzeichnung ehrlich. In der
Oberfläche und im Export ist unterscheidbar, ob ein Server geantwortet hat oder ob wir nie
dort angekommen sind, und wenn nicht, woran es lag (DNS, Verbindungsaufbau, TLS, private
Zieladresse, Timeout). Für die Diagnose auf der Seite des Nutzers ist das der Unterschied
zwischen „der Dienst ist kaputt" und „dein DNS ist kaputt".

Timeouts sind ein Übergang wie jeder andere (`TransitionInput::Timeout` →
`Decided(TimedOut)`), kein Sonderweg. Für sie gelten deshalb automatisch dieselbe
Aufzeichnung, dieselbe Audit-Zeile und dieselbe Darstellung in der Oberfläche.

## Verworfene Alternativen

- **Boolesche Flags und `Option`-Felder an einem `Flow`-Struct.** Der naheliegende Weg.
  Verliert, weil die Menge der gültigen Kombinationen nirgends aufgeschrieben ist und mit
  jedem neuen Flag exponentiell wächst.
- **Vollständiges Event Sourcing.** Der Zustand im Speicher wäre eine Faltung über den
  Ereignisstrom. Klingt sauber, kostet aber Replay-Logik, Snapshot-Verwaltung und
  Schema-Migrationen für Ereignisse, und das für einen Prozess, der ein paar hundert Flows je
  Session hält und beim Neustart ohnehin keinen Zustand im Speicher wiederherstellen soll.
  Der Ereignisstrom bleibt trotzdem vollständig, nur nach außen und nicht nach innen.
- **Zustand nur in SQLite, jeder Schritt ein `UPDATE`.** Jeder Übergang kostete IO, der Kern
  wäre nicht mehr IO-frei (Verstoß gegen ADR-0015), und eine angehaltene Anfrage hinge an
  einer Datenbanktransaktion.
- **Fehler beim Upstream als `Responded{502}`.** Wäre weniger Code, macht die Aufzeichnung
  aber mehrdeutig. Im Review vom 2026-09-02 ausdrücklich verworfen
  (`backlog/CONVENTIONS.md` 4.10).
- **Die Übergangsmethode nimmt das Ereignis als Eingabe (`on(self, &FlowEvent)`).** Diese
  Form stand in einer frühen Fassung von `docs/ARCHITECTURE.md` 3 und wurde korrigiert: Das
  Ereignis ist Ausgabe, nicht Eingabe (`backlog/CONVENTIONS.md` 4.2).

## Konsequenzen

- Die Zustandsmaschine lebt in `humanitl-core`, ohne IO, ohne async, ohne Protobuf. Sie lässt
  sich in einem Unit-Test vollständig durchlaufen.
- Der Test für verbotene Übergänge ist Pflichtbestandteil von HUM-004: Jede Kombination aus
  Zustand und `TransitionInput`, die nicht in der Liste steht, muss `InvalidTransition`
  liefern.
- `FlowEvent` ist der einzige Weg, an dem Oberfläche, Recorder und Audit hängen. Ein neuer
  Zustand ohne neues Ereignis ist unmöglich.
- Das `Held`-Ereignis trägt zusätzlich `queue_bytes` und `queue_count`, damit das Budget für
  angehaltene Anfragen in der Oberfläche sichtbar ist (`backlog/CONVENTIONS.md` 4.2,
  HUM-057).
- Die Statuscodes für die einzelnen Blockiergründe stehen in der Tabelle oben und noch einmal
  in ADR-0005; beide Tabellen müssen identisch bleiben.

## Betroffene Issues

`HUM-004` (core-types mit Zustandsmaschine und Übergangstests), `HUM-016` (Warteschlange,
läuft über die Zustandsmaschine), `HUM-024` (DNS- und Verbindungsfehler als `Failed`),
`HUM-026` (Recorder als Abnehmer der Ereignisse), `HUM-018` (`Subscribe`-Stream), `HUM-057`
(`queue_bytes`/`queue_count` im `Held`-Ereignis).
