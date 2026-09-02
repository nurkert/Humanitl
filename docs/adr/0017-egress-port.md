# ADR-0017 · Ein Egress-Port für Direktverbindung, Upstream-Proxy und Tor
Status: Accepted
Datum: 2026-09-02

## Kontext

Wenn der Mensch einen Request freigegeben hat, muss der Proxy ihn nach draußen
bringen. Im einfachsten Fall ist das eine TCP-Verbindung zur aufgelösten
IP-Adresse. Es gibt aber zwei absehbare Varianten: In vielen Firmennetzen führt
der einzige Weg nach außen über einen HTTP-Proxy mit `CONNECT`, und manche
Nutzer wollen Agentenverkehr über Tor leiten.

Die eigentliche Gefahr liegt woanders. Der Verbindungsaufbau ist in einem
HTTP-Client eine unscheinbare Zeile, und sie kann an vielen Stellen entstehen:
im Upstream-Client, in einem Health-Check, in einem Wiederholungsversuch, in
einer Bibliothek, die intern selbst verbindet. Jede dieser Stellen umgeht
möglicherweise die Regeln aus ADR-0006 (Auflösung erst nach Freigabe,
IP-Pinning, Prüfung auf private Adressen). Ein einziger vergessener
`TcpStream::connect` hebelt eine Kernzusage aus, ohne dass ein Test darüber
stolpert.

## Entscheidung

**Jede Upstream-Verbindung geht durch genau einen Port.**

```rust
pub trait Egress: Send + Sync {
    async fn connect(
        &self,
        authority: &Authority,
        resolved: Option<IpAddr>,
    ) -> Result<Box<dyn AsyncStream>, Diagnostic>;
}

pub struct Direct;   // MVP
```

Im MVP existiert genau eine Implementierung: `Direct`. Sie verbindet zu der IP,
die nach der Freigabe über den `Resolver`-Port ermittelt wurde, und pinnt sie
(ADR-0006).

Später, ohne Änderung am Kern:

- `HttpProxy(Url)` — `CONNECT` an einen Firmen-Proxy.
- `Socks5h(Url)` — Tor unter `socks5h://127.0.0.1:9050`.

Konfiguriert wird global über `egress.via` oder pro Regel über `via:`.

**Bei `Socks5h` wird der Hostname unaufgelöst an den SOCKS-Proxy übergeben, und
lokale DNS-Auflösung ist in diesem Modus verboten.** Andernfalls entstünde ein
DNS-Leak, der den ganzen Zweck der Tor-Nutzung aufhebt. Der Isolation-Check
bekommt in diesem Fall eine vierte, optionale Zeile „Egress: Tor" mit einer
Prüfung gegen `check.torproject.org`.

**Maschinell erzwungen:** `tools/check-deps.sh` schlägt fehl, sobald
`TcpStream::connect` außerhalb von `daemon/crates/proxy/src/egress/` im
Produktionscode auftaucht (ADR-0015).

Dieser ADR ist im MVP **nur** die Portdefinition plus die Durchsetzung. Die
Adapter `HttpProxy` und `Socks5h` sind Post-MVP.

## Begründung

Der Wert dieses Ports liegt nicht in der Erweiterbarkeit, sondern in der
Kontrolle. Ein einziger Ort, an dem eine Verbindung nach außen entsteht, ist ein
einziger Ort, an dem die Prüfung auf private Adressen stattfindet, an dem die IP
gepinnt wird, an dem ein Timeout gilt und an dem ein Fehler zu
`FlowState::Failed` mit einem `UpstreamError` wird. Wären es fünf Orte, müsste
jeder davon dieselben fünf Dinge richtig machen — und der sechste, der später
dazukommt, würde es nicht.

Deshalb ist der Grep in CI Teil der Entscheidung und nicht eine begleitende
Maßnahme. Er ist grob, kennt keine Aliasse und keine Bibliothek, die intern
verbindet, aber er fängt zuverlässig den häufigsten Fall: jemand schreibt schnell
einen direkten Verbindungsaufbau, weil es an dieser Stelle gerade praktisch ist.

Dass `Egress::connect` sowohl die `Authority` als auch optional die aufgelöste IP
nimmt, macht die Aufteilung im Typsystem sichtbar. `Direct` bekommt die IP und
verbindet dorthin. `Socks5h` bekommt sie **nicht** und darf sie auch nicht
ermitteln — der Aufrufer gibt `None`, und der Adapter reicht den Namen an den
SOCKS-Proxy weiter. Der DNS-Leak wird damit zu einem Fall, den die Signatur
ausschließt, statt zu einer Regel, an die man sich erinnern muss.

Der Rückgabewert `Result<_, Diagnostic>` statt eines `io::Error` sorgt dafür,
dass ein fehlgeschlagener Verbindungsaufbau beim Nutzer als „warum" und „was
jetzt" ankommt (ADR-0012): keine Route zum Firmen-Proxy, Tor läuft nicht, Ziel
ist eine private Adresse.

`via:` pro Regel statt nur global, weil der realistische Fall gemischt ist: Der
LLM-Server im LAN wird direkt erreicht, der Rest über den Firmen-Proxy. Eine rein
globale Einstellung würde diesen Fall nicht abbilden.

## Verworfene Alternativen

- **Kein Port, direkte Verbindungen im Proxy-Code.** Am wenigsten Code und die
  Ursache genau der Streuung, die dieser ADR verhindert. Auch mit „wir passen
  auf" nicht haltbar.
- **Konfiguration über die Umgebungsvariablen `HTTP_PROXY`/`ALL_PROXY` des
  Daemons.** Hätte den Weg nach außen von einer Umgebung abhängig gemacht, die
  ein Systemd-Unit-Detail ist — unsichtbar im UI, nicht pro Regel steuerbar und
  in einer Sicherheitskomponente eine unangenehme Fernwirkung.
- **`HttpProxy` und `Socks5h` schon im MVP mitbauen.** Verstößt gegen „kein
  Trait ohne zweiten Nutzer in Sicht" in der praktischen Lesart: Der Port
  entsteht jetzt (er hat einen konkreten zweiten Adapter in Aussicht), die
  Adapter entstehen, wenn jemand sie braucht.
- **Tor als eingebetteter `arti`-Client statt über einen lokalen SOCKS-Proxy.**
  Deutlich größere Abhängigkeit und eine eigene Aktualisierungspflicht für eine
  Nice-to-have-Funktion. Der lokale Tor-Dienst ist der etablierte Weg.
- **DNS-Auflösung auch im SOCKS-Modus lokal.** Wäre einfacher und macht die
  Tor-Nutzung wertlos, weil der DNS-Verkehr die Ziele preisgibt.
- **Prüfung der Egress-Regel per Clippy-Lint statt per Grep.** Wäre sauberer,
  setzt aber ein eigenes Lint-Werkzeug voraus. Der Grep ist heute verfügbar und
  kostet nichts.

## Konsequenzen

- Im MVP gibt es genau einen Adapter, aber der Port existiert und wird
  durchgesetzt. Der spätere Tor- oder Firmen-Proxy-Modus berührt weder Kern noch
  Anwendung (ADR-0015).
- Timeouts für den Verbindungsaufbau gehören zum Port und stehen in der Gruppe
  `limits` (`limits.connect_timeout_secs`, Alias `upstream.connect_timeout_secs`).
- Ein fehlgeschlagener Verbindungsaufbau wird zu
  `FlowState::Failed { UpstreamError::Connect | Tls | PrivateAddress(_) |
  Timeout }` und damit zu `502` beim Client (ADR-0004, ADR-0005).
- Der Grep in `tools/check-deps.sh` schließt Testcode aus; Tests dürfen direkt
  verbinden, Produktionscode nicht.
- Kommen `HttpProxy` oder `Socks5h` dazu, braucht der Isolation-Check eine
  zusätzliche Zeile, damit der Nutzer sieht, welchen Weg sein Verkehr nimmt. Ohne
  diese Anzeige wäre der Egress-Modus eine unsichtbare Einstellung mit großer
  Wirkung.
- Der Port hat eine `async`-Signatur und liegt damit in der Adapterschicht, nicht
  im IO-freien Kern.

## Betroffene Issues

`HUM-015` (MITM-Proxy-Kern: alle Upstream-Verbindungen über `Egress`),
`HUM-024` (DNS erst nach Allow, IP an `Egress::connect` gepinnt), `HUM-074`
(Grep-Prüfung auf `TcpStream::connect` in `tools/check-deps.sh`), `HUM-057`
(Timeouts in der Gruppe `limits`).
