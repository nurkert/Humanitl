---
source: adr/0017-egress-port.md
source-sha256: 822ade41b8a4b480ec06b4a2edf72205d2775798d18030b44b88987bf0b1fe4b
lang: de
---
# ADR-0017 · Ein Egress-Port für Direktverbindung, Upstream-Proxy und Tor

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Hat der Mensch eine Anfrage freigegeben, muss der Proxy sie nach draußen
bringen. Im einfachsten Fall ist das eine TCP-Verbindung zur aufgelösten
IP-Adresse. Es gibt aber zwei absehbare Varianten: In vielen Firmennetzen führt
der einzige Weg nach draußen über einen HTTP-Proxy mit `CONNECT`, und manche
Nutzer wollen den Verkehr des Agenten über Tor leiten.

Die eigentliche Gefahr liegt woanders. In einem HTTP-Client ist ein
Verbindungsaufbau eine unauffällige Zeile, und sie kann an vielen Stellen
entstehen: im Upstream-Client, in einem Health-Check, in einem erneuten
Versuch, in einer Bibliothek, die sich intern selbst verbindet. Jede dieser
Stellen umgeht womöglich die Regeln aus ADR-0006 (Auflösung erst nach der
Freigabe, IP-Pinning, Prüfung auf private Adressen). Ein einziges vergessenes
`TcpStream::connect` untergräbt ein Kernversprechen, ohne dass ein Test darüber
stolpert.

## Entscheidung

**Jede Upstream-Verbindung läuft über genau einen Port.**

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

Im MVP gibt es genau eine Implementierung: `Direct`. Sie verbindet sich mit der
IP, die nach der Freigabe über den Port `Resolver` ermittelt wurde, und pinnt
sie (ADR-0006).

Später, ohne Änderung am Kern:

- `HttpProxy(Url)`: `CONNECT` zu einem Firmen-Proxy.
- `Socks5h(Url)`: Tor unter `socks5h://127.0.0.1:9050`.

Eingestellt wird das global über `egress.via` oder je Regel über `via:`.

**Mit `Socks5h` wird der Hostname unaufgelöst an den SOCKS-Proxy übergeben, und
lokale DNS-Auflösung ist in diesem Modus verboten.** Sonst entstünde ein
DNS-Leck, das den ganzen Zweck von Tor zunichtemacht. In diesem Fall bekommt die
Isolationsprüfung eine vierte, optionale Zeile „Egress: Tor" mit einer Prüfung
gegen `check.torproject.org`.

**Maschinell erzwungen:** `tools/check-deps.sh` schlägt fehl, sobald
`TcpStream::connect` im Produktivcode außerhalb von
`daemon/crates/proxy/src/egress/` steht (ADR-0015).

Im MVP ist dieses ADR **nur** die Port-Definition plus die Durchsetzung. Die
Adapter `HttpProxy` und `Socks5h` kommen nach dem MVP.

## Begründung

Der Wert dieses Ports liegt nicht in der Erweiterbarkeit, sondern in der
Kontrolle. Eine einzige Stelle, an der eine ausgehende Verbindung entsteht, ist
eine einzige Stelle, an der die Prüfung auf private Adressen stattfindet, an der
die IP gepinnt wird, an der ein Zeitlimit gilt und an der ein Fehler zu
`FlowState::Failed` mit einem `UpstreamError` wird. Gäbe es fünf Stellen, müsste
jede davon dieselben fünf Dinge richtig machen, und die sechste, später
hinzugekommene, würde es nicht.

Deshalb ist der grep in der CI Teil der Entscheidung und keine Begleitmaßnahme.
Er ist grob, kennt keine Aliase und keine Bibliothek, die sich intern verbindet,
aber er fängt zuverlässig den häufigsten Fall: Jemand schreibt schnell einen
direkten Verbindungsaufbau, weil es an dieser Stelle gerade bequem ist.

Dass `Egress::connect` sowohl die `Authority` als auch, optional, die
aufgelöste IP nimmt, macht die Aufteilung im Typsystem sichtbar. `Direct`
bekommt die IP und verbindet sich dorthin. `Socks5h` bekommt sie **nicht** und
darf sie auch nicht ermitteln: Der Aufrufer übergibt `None`, und der Adapter
reicht den Namen an den SOCKS-Proxy weiter. Das DNS-Leck wird damit zu einem
Fall, den die Signatur ausschließt, statt zu einer Regel, an die man denken
muss.

Der Rückgabewert `Result<_, Diagnostic>` statt eines `io::Error` sorgt dafür,
dass ein gescheiterter Verbindungsaufbau den Nutzer als „Grund" und „was jetzt"
erreicht (ADR-0012): keine Route zum Firmen-Proxy, Tor läuft nicht, das Ziel ist
eine private Adresse.

`via:` je Regel statt nur global, weil der realistische Fall gemischt ist: Der
LLM-Server im LAN wird direkt erreicht, der Rest über den Firmen-Proxy. Eine
rein globale Einstellung würde diesen Fall nicht abdecken.

## Verworfene Alternativen

- **Kein Port, direkte Verbindungen im Proxy-Code.** Der wenigste Code und die
  Ursache genau der Zerstreuung, die dieses ADR verhindert. Auch mit „wir passen
  auf" nicht haltbar.
- **Konfiguration über die Umgebungsvariablen des Daemons
  `HTTP_PROXY`/`ALL_PROXY`.** Hätte den Weg nach draußen von einer Umgebung
  abhängig gemacht, die ein Detail der systemd-Unit ist: unsichtbar in der
  Oberfläche, nicht je Regel steuerbar und in einer Sicherheitskomponente eine
  unangenehme Fernwirkung.
- **`HttpProxy` und `Socks5h` schon im MVP bauen.** Verstößt gegen „kein Trait
  ohne zweiten Nutzer in Sicht" in seiner praktischen Lesart: Der Port entsteht
  jetzt (er hat einen konkreten zweiten Adapter in Aussicht), die Adapter
  entstehen, wenn jemand sie braucht.
- **Tor als eingebetteter `arti`-Client statt über einen lokalen SOCKS-Proxy.**
  Eine erheblich größere Abhängigkeit und eine eigene Pflicht zu Updates für
  eine Funktion, die schön, aber nicht nötig ist. Der lokale Tor-Dienst ist der
  etablierte Weg.
- **Lokale DNS-Auflösung auch im SOCKS-Modus.** Wäre einfacher und macht Tor
  wertlos, weil der DNS-Verkehr die Ziele verrät.
- **Die Egress-Regel mit einem Clippy-Lint statt mit einem grep prüfen.** Wäre
  sauberer, verlangt aber ein eigenes Lint-Werkzeug. Den grep gibt es heute,
  und er kostet nichts.

## Folgen

- Im MVP gibt es genau einen Adapter, aber der Port existiert und wird
  durchgesetzt. Der spätere Tor- oder Firmen-Proxy-Modus berührt weder Kern
  noch Anwendung (ADR-0015).
- Zeitlimits für den Verbindungsaufbau gehören zum Port und stehen in der
  Gruppe `limits` (`limits.connect_timeout_secs`, Alias
  `upstream.connect_timeout_secs`).
- Ein gescheiterter Verbindungsaufbau wird zu
  `FlowState::Failed { UpstreamError::Connect | Tls | PrivateAddress(_) |
  Timeout }` und damit beim Client zu `502` (ADR-0004, ADR-0005).
- Der grep in `tools/check-deps.sh` nimmt Testcode aus; Tests dürfen sich
  direkt verbinden, Produktivcode nicht.
- Kommen `HttpProxy` oder `Socks5h` hinzu, braucht die Isolationsprüfung eine
  zusätzliche Zeile, damit der Nutzer sieht, welchen Weg sein Verkehr nimmt.
  Ohne diese Anzeige wäre der Egress-Modus eine unsichtbare Einstellung mit
  großer Wirkung.
- Der Port hat eine `async`-Signatur und lebt deshalb in der Adapterschicht,
  nicht im Kern ohne IO.

## Betroffene Issues

`HUM-015` (Kern des MITM-Proxys: alle Upstream-Verbindungen über `Egress`),
`HUM-024` (DNS erst nach dem Erlauben, IP gepinnt bei `Egress::connect`),
`HUM-074` (grep-Prüfung auf `TcpStream::connect` in `tools/check-deps.sh`),
`HUM-057` (Zeitlimits in der Gruppe `limits`).
