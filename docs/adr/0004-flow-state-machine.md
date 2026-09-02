# ADR-0004 · Request-Lebenszyklus als Zustandsautomat, Events werden abgeleitet
Status: Accepted
Datum: 2026-09-02

## Kontext

Ein Request durchläuft in Humanitl mehr Stationen als in einem gewöhnlichen
Proxy: Er kommt an, wird auf Findings untersucht, gegen Regeln bewertet,
möglicherweise minutenlang angehalten, entschieden, weitergeleitet,
beantwortet und aufgezeichnet. Fünf Verbraucher wollen zu jedem Zeitpunkt
wissen, wo er steht: der Proxy-Handler selbst, die Hold-Queue, das UI, der
Recorder und das Audit-Log.

Ohne eine explizite Modellierung entsteht genau die Sorte Code, die nach zwei
Jahren nicht mehr korrigierbar ist: verstreute Boolean-Flags (`is_held`,
`was_decided`, `response_started`), deren erlaubte Kombinationen niemand
aufschreibt, und Zustände, die im UI existieren, aber im Daemon nicht. Dazu
kommt die Frage, ob Fehler nach der Freigabe (DNS schlägt fehl, TCP-Connect
schlägt fehl, TLS-Handshake schlägt fehl) als „Antwort" gelten. Wenn ja, sieht
ein Audit-Leser einen `502` und kann nicht unterscheiden, ob der Server das
gesagt hat oder wir.

## Entscheidung

Der Lebenszyklus ist ein `enum FlowState` in `humanitl-core` mit genau einer
Methode, die ihn ändert. `Flow` hält Zustand und Historie und ruft sie auf:

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

`Transition` ist ein Umschlag: `TransitionInput` sagt, was geschehen soll;
`flow_id` und `at` kommen vom Aufrufer, weil der reine Automat weder IDs
erfinden noch eine Uhr lesen darf, das erzeugte Ereignis aber beides trägt.
`TransitionInput` hat die Varianten `Analyze` (mit den Findings der
Detektoren), `Hold { deadline, queue_bytes, queue_count }`,
`Decide { decision, source }`, `Forward`, `Respond { status }`, `Record`,
`Timeout` und `Fail { error }`; dazu Konstruktoren `Transition::analyze(..)`
usw. `Flow::apply` kennt die eigene ID, ruft `on` und ersetzt Zustand und
Historie; bei einem ungültigen Übergang bleiben beide unverändert
(`backlog/CONVENTIONS.md` 4.11). `FlowEvent` ist die abgeleitete Ausgabe.
Zustände:

```
Received → Analyzed{findings} → Held{deadline} → Decided(Allow | AllowEdited | Block | TimedOut)
         → Forwarded → Responded{status} → Recorded
```

Zusätzlich `Failed{error: UpstreamError}` mit
`UpstreamError = Dns | Connect | Tls | PrivateAddress(IpAddr) | Timeout`,
erreichbar aus `Decided(Allow | AllowEdited)` und aus `Forwarded`, also aus
allen Zuständen nach der Freigabe. `Failed` geht nach `Recorded`.

Erlaubt sind genau: `Received→Analyzed→Held→Decided→Forwarded→Responded→Recorded`,
`Held→Decided(TimedOut)→Recorded`, `Decided(Block)→Recorded`,
`Analyzed→Decided` (Regel-Auto-Entscheidung, überspringt `Held`),
`Decided(Allow|AllowEdited)→Failed`, `Forwarded→Failed`, `Failed→Recorded`.
Alles andere liefert `InvalidTransition { from, input }`.

Wer entscheiden darf, hängt von der `DecisionSource` ab. `DecisionSource::System`
(der Daemon selbst, etwa bei erschöpftem Budget) darf aus `Analyzed` und `Held`
nur ablehnen (`Block`, `TimedOut`), nie erlauben. Nur so sind `HoldMemory`,
`HoldMaxFlows`, `BodyCap` und `ClientTimeout` ausdrückbar
(`backlog/CONVENTIONS.md` 4.11).

Ein Upstream-Fehler wird nie als `Responded` verbucht. Der Client bekommt in
diesem Fall `502` mit dem einheitlichen `Blocked by Humanitl.`-Body und
`reason: upstream_*`. Die Statuscodes je Blockgrund gehören zu dieser
Modellierung und sind mit ADR-0005 identisch:

| Grund | Status |
|---|---|
| `User`, `Rule`, `AuthorityMismatch`, `PrivateAddress` | `403` |
| `BodyCap` | `413` |
| `Timeout` (Hold-Timeout) | `504` |
| `HoldMemory`, `HoldMaxFlows` | `503` |
| `NoRoute` | `502` |
| `ClientTimeout` | `408` |
| Upstream-Fehler (`FlowState::Failed`) | `502` |

Aus jedem Übergang entsteht ein `FlowEvent`, der zugleich den gRPC-Stream und
das Audit-Log speist. Das ist bewusst *kein* Event Sourcing: Der In-Memory-Zustand
wird nicht aus Events rekonstruiert. Wahrheit ist der Automat plus SQLite; Events
sind Ausgabe.

Gehaltene Requests blockieren nie auf das UI. Die Hold-Queue ist eine
`DashMap<FlowId, oneshot::Sender<Decision>>` mit einem Deadline-Timer; der
Proxy-Handler `await`et auf den Empfänger.

## Begründung

Ein `enum` mit einer einzigen Änderungsmethode macht ungültige Zustände nicht
nur unwahrscheinlich, sondern unkonstruierbar. Die Liste der erlaubten Übergänge
ist endlich und klein genug, um sie in einem tabellengetriebenen Test vollständig
aufzuzählen — inklusive aller verbotenen Kombinationen. Ein Bug in diesem Bereich
ist damit ein fehlender Testfall, kein Rätsel.

Dass das Event die *Ausgabe* des Übergangs ist und nicht seine Eingabe, ist der
Punkt, an dem sich die Sache entscheidet. Wäre das Event die Eingabe, könnte ein
Aufrufer ein Ereignis erfinden, das nie stattgefunden hat, und der Zustand würde
folgen. So kann er nur einen Übergang *versuchen*; das Ereignis entsteht
nachweislich aus einem stattgefundenen Übergang. Für ein Audit-Log, dessen
ganzer Zweck die Nachvollziehbarkeit ist, ist diese Richtung die einzig
vertretbare.

Der Handler *treibt* den Automaten, er *besitzt* ihn nicht: Er ruft
`Flow::apply`, veröffentlicht das Ereignis und wartet gegebenenfalls auf die
Entscheidung. Damit bleibt der Handler dünn und die Logik ohne Netzwerk
testbar.

`Failed` als eigener Zustand statt als `Responded{502}` hält die Aufzeichnung
ehrlich. Im UI und im Export ist unterscheidbar, ob ein Server geantwortet hat
oder ob wir gar nicht hingekommen sind — und wenn ja, woran es lag (DNS,
Connect, TLS, private Zieladresse, Timeout). Für die Diagnose beim Nutzer ist das
der Unterschied zwischen „der Dienst ist kaputt" und „dein DNS ist kaputt".

Timeouts sind ein Übergang wie jeder andere (`TransitionInput::Timeout` →
`Decided(TimedOut)`), kein Sonderpfad. Damit gilt für sie automatisch dieselbe
Aufzeichnung, dieselbe Audit-Zeile und dieselbe UI-Darstellung.

## Verworfene Alternativen

- **Boolean-Flags und `Option`-Felder auf einer `Flow`-Struktur.** Der
  naheliegende Weg. Verliert, weil die Menge der gültigen Kombinationen nirgends
  steht und mit jedem neuen Flag exponentiell wächst.
- **Vollständiges Event Sourcing.** Der In-Memory-Zustand wäre eine Faltung über
  den Ereignisstrom. Klingt sauber, kostet aber Replay-Logik, Snapshot-Verwaltung
  und Schema-Migrationen für Ereignisse — für einen Prozess, der pro Sitzung
  einige hundert Flows hält und beim Neustart ohnehin keinen In-Memory-Zustand
  wiederherstellen soll. Der Ereignisstrom bleibt trotzdem vollständig, nur eben
  nach außen und nicht nach innen.
- **Zustand nur in SQLite, jeder Schritt ein `UPDATE`.** Jeder Übergang würde
  IO kosten, der Kern wäre nicht mehr IO-frei (Verstoß gegen ADR-0015), und ein
  gehaltener Request hinge an einer Datenbanktransaktion.
- **Upstream-Fehler als `Responded{502}`.** Wäre weniger Code, macht die
  Aufzeichnung aber mehrdeutig. Ausdrücklich verworfen im Review vom
  2026-09-02 (`backlog/CONVENTIONS.md` 4.10).
- **Übergangsmethode nimmt das Event entgegen (`on(self, &FlowEvent)`).** Diese
  Form stand in einer frühen Fassung von `docs/ARCHITECTURE.md` 3 und wurde
  korrigiert: Das Event ist Ausgabe, nicht Eingabe
  (`backlog/CONVENTIONS.md` 4.2).

## Konsequenzen

- Der Automat lebt in `humanitl-core`, ohne IO, ohne async, ohne Protobuf. Er ist
  in einem Unit-Test vollständig durchlaufbar.
- Der Test für verbotene Übergänge ist Pflichtbestandteil von HUM-004: Jede
  Kombination aus Zustand und `TransitionInput`, die nicht in der Liste steht,
  muss `InvalidTransition` liefern.
- `FlowEvent` ist der einzige Weg, an dem UI, Recorder und Audit hängen. Ein
  neuer Zustand ohne neues Ereignis ist unmöglich.
- Der `Held`-Event trägt zusätzlich `queue_bytes` und `queue_count`, damit das
  Hold-Budget im UI sichtbar ist (`backlog/CONVENTIONS.md` 4.2, HUM-057).
- Die Statuscodes für die einzelnen Blockgründe stehen in der Tabelle oben und
  noch einmal in ADR-0005; beide Tabellen müssen gleich bleiben.

## Betroffene Issues

`HUM-004` (core-types mit Automat und Übergangstests), `HUM-016` (Hold-Queue,
durchläuft den Automaten), `HUM-024` (DNS-/Connect-Fehler als `Failed`),
`HUM-026` (Recorder als Ereignis-Verbraucher), `HUM-018` (`Subscribe`-Stream),
`HUM-057` (`queue_bytes`/`queue_count` im `Held`-Event).
