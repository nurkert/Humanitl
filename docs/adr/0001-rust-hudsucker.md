# ADR-0001 · Daemon in Rust auf hudsucker, nicht mitmproxy
Status: Accepted
Datum: 2026-09-02

## Kontext

Humanitl setzt einen abfangenden HTTPS-Proxy zwischen einen sandboxed
LLM-Agenten und das Internet. Dieser Proxy hält jeden Request an, bis ein Mensch
entschieden hat. Er hält dabei die sensibelsten Dinge der Installation in der
Hand: den privaten Schlüssel der MITM-CA, die vollständigen Aufzeichnungen aller
Anfragen und Antworten inklusive Bodies, und die Steuerung der Sandbox, in der
der Agent läuft. Für dieses Bauteil ist Sicherheit nicht ein Qualitätsmerkmal
neben anderen, sondern das Produkt selbst.

Es gab zwei realistische Wege. Erstens: einen fertigen, ausgereiften
MITM-Proxy nehmen (mitmproxy, Python) und die Fachlogik als Addon schreiben.
Zweitens: einen eigenen Daemon in einer speichersicheren Systemsprache bauen und
für die Protokollarbeit eine Bibliothek nutzen. Der Unterschied zwischen den
Wegen ist nicht die Menge an Fachlogik — die ist in beiden Fällen dieselbe —
sondern die Größe der Trusted Computing Base und die Frage, ob das Ergebnis als
ein Paket auslieferbar ist.

Zielplattform ist Linux-Desktop, Auslieferung als `.deb` und AppImage, Betrieb
als systemd user service. Die Zielgruppe sind Professionelle ohne
Security-Hintergrund, die ein Paket installieren und danach nichts konfigurieren
wollen.

## Entscheidung

Der Daemon `humanitld` ist ein Rust-Binary. Die MITM-Schicht ist
[hudsucker](https://github.com/omjadas/hudsucker) 0.25 mit den Features
`rcgen-ca`, `rustls-client`, `http2`. Alles darüber (Hold-Queue, Regel-Engine,
Findings, Recorder, Audit-Kette, gRPC-Server, Sandbox-Steuerung) ist eigener
Code in den Crates unter `daemon/crates/`.

Verbindliches Protokoll-Ziel je Milestone, damit kein Sprint-Ziel den Status
„experimentell" trägt:

| Milestone | Client-seitig | Upstream | Sonstiges |
|---|---|---|---|
| M1 | HTTP/1.1 (ALPN bietet dem Client nur `http/1.1`) | HTTP/1.1 erzwungen | CONNECT, WebSocket-Passthrough, SSE |
| M1 | gRPC | — | dokumentierter Fehlschlag in der Konformitäts-Matrix (`PROXY_007 h2 not available`) |
| M6 | HTTP/2 | HTTP/2 | hebt gRPC aus `[experimental]` |

Der Listener ist genau ein Unix-Socket, kein Loopback-TCP-Port auf dem Host.
hudsucker lauscht in seiner Standardform auf TCP; HUM-015 Schritt 0 klärt in
einem Spike, ob hudsucker 0.25 einen generischen `Accept`-Stream annimmt. Falls
ja, lauscht der Proxy direkt auf dem Unix-Socket. Falls nein, wird hudsuckers
Accept-Schleife (etwa 100 Zeilen) als Fork im Repository gehalten. Die Tür in
die Sandbox bleibt in beiden Fällen das, was sie sein soll: eine einzelne
Socket-Datei, die kein anderer Prozess auf dem Host erreicht.

## Begründung

Ein statisch gelinktes, speichersicheres Binary ist eine kleinere und leichter
zu argumentierende Trusted Computing Base als ein Python-Bundle mit rund vierzig
transitiven Paketen. Für ein Werkzeug, dessen ganze Aussage „hier kommt nichts
raus" lautet, ist die Größe der Angriffsfläche das zentrale Argument. Die
Sicherheitsargumentation muss in drei Sätzen erklärbar und per Klick prüfbar
sein (Prinzip 2 in `BACKLOG.md` 1.3); jede Zeile fremden Codes im
Vertrauensbereich verlängert diese drei Sätze.

hudsucker liefert genau die Protokollarbeit, die man nicht selbst schreiben
will: CONNECT-Handling, Zertifikats-Cache über `rcgen`, HTTP/1 und HTTP/2 auf
hyper 1, WebSocket-Handler, streamende Bodies mit Trailern. Entscheidend ist die
Form des Hooks: `handle_request` ist `async`. Ein `await` auf einen
`oneshot`-Channel ist damit der natürliche Haltepunkt für die Hold-Queue,
und ein gehaltener Request kostet keinen Thread.

Die eigentliche Arbeit — Hold-Queue, Regeln, Findings, Recorder, Audit, gRPC,
Sandbox — wäre in jeder Sprache neu zu schreiben. mitmproxy hätte davon nichts
abgenommen. Es hätte Protokoll-Randfälle abgenommen, dafür aber ein
Packaging-Problem eingekauft.

Der Preis der Entscheidung sind die MITM-Randfälle, die jetzt uns gehören.
Mitigation: eine Konformitäts-Matrix ab Sprint 1 (HUM-017), die `curl`,
`websocat` und `grpcurl` gegen einen in-process axum-Fake-Upstream fährt —
chunked, SSE, CONNECT, große Bodies, WebSocket-Upgrade, Trailer. Der
Testkorpus und die Regel-Semantik von mitmproxy bleiben als Referenz nutzbar,
auch ohne mitmproxy im Produkt.

## Verworfene Alternativen

- **mitmproxy als Addon-Host (Python).** Packaging ist der Killer: Python-Runtime
  plus C-Extensions ergeben 80 bis 150 MB pro Artefakt, und ein `.deb`, das eine
  eigene Python-Umgebung mitbringt, ist auf Dauer eine Wartungslast. Dazu kommt
  die Größe der TCB und die fehlende Speichersicherheit der C-Extensions. Der
  Vorteil (ausgereifte Protokollimplementierung) wiegt das nicht auf, weil die
  Fachlogik ohnehin neu entsteht.
- **Go mit goproxy.** goproxy ist veraltet und HTTP/2-MITM ist in `net/http`
  mühsam bis unvollständig. Ein zweites Ökosystem im Repository (neben Dart für
  die UI) ohne klaren Gewinn.
- **Eigene MITM-Schicht direkt auf hyper, ohne hudsucker.** Wäre möglich, aber
  Zertifikats-Cache, CONNECT-Tunnel und WebSocket-Upgrade sind genau die Stellen,
  an denen subtile Fehler entstehen. hudsucker ist dünn genug, um es notfalls zu
  forken (siehe Listener-Frage), also ist die Abhängigkeit reversibel.
- **Node.js.** Speichersicher im Sinne von „keine Buffer-Overflows", aber
  npm-Abhängigkeitsbaum und Runtime-Größe reproduzieren das Python-Problem.
- **Kein eigener Proxy, stattdessen nur Firewall-Regeln.** Löst das Problem
  nicht: Eine Firewall sieht Hostnamen, aber keine Request-Bodies. Genau im Body
  wird exfiltriert (siehe ADR-0005).

## Konsequenzen

- Rust wird die Sprache des gesamten Daemons, inklusive Shim und CLI. Ein
  Beitragender braucht Rust-Kenntnisse; dafür erzwingt der Compiler die
  Modulgrenzen aus ADR-0015.
- MITM-Konformität ist unsere Verantwortung. Sie wird nicht behauptet, sondern
  in HUM-017 gemessen; die Matrix ist Teil der Dokumentation und listet auch die
  bewussten Lücken (gRPC bis M6).
- Die Bindung an hudsucker 0.25 ist eine Versionsbindung mit Bruchrisiko. Sie ist
  auf die Crate `humanitl-proxy` beschränkt (ADR-0015: keine Abstraktion über
  Fremdbibliotheken, aber Fremdbibliothek nur in einer Crate), damit ein Wechsel
  lokal bleibt.
- Die Listener-Frage ist der einzige offene Punkt dieser Entscheidung. Ihr
  Ausgang ändert nicht die Entscheidung, nur die Menge des geforkten Codes.
- `experimental.h2_upstream` bleibt bis M6 der einzige Schalter, hinter dem h2
  liegt. Ungenutzte `experimental`-Flags fallen nach zwei Sprints (ADR-0015).

## Betroffene Issues

`HUM-015` (MITM-Proxy-Kern, Schritt 0 Listener-Spike), `HUM-013`
(Proxy-Socket-Bind), `HUM-017` (Konformitäts-Matrix), `HUM-021` (Demo-Skript
M1), `HUM-056` (Fuzzing der Parser und Decoder), `HUM-061` (Puffer für
MITM-Randfälle).
