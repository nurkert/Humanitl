---
source: adr/0001-rust-hudsucker.md
source-sha256: 8e4067f67f3fab19f93ec2af3ac9e8e9057a74bced0b33fd75431ab2a684f0ac
lang: de
---
# ADR-0001 · Daemon in Rust auf hudsucker, nicht mitmproxy

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Humanitl stellt einen HTTPS-Proxy, der Anfragen anhält, zwischen einen LLM-Agenten in der
Sandbox und das Internet. Dieser Proxy hält jede Anfrage an, bis ein Mensch entschieden hat.
Dabei hat er das Empfindlichste der ganzen Installation in der Hand: den privaten Schlüssel
der MITM-CA, die vollständigen Aufzeichnungen aller Anfragen und Antworten samt Bodys und die
Steuerung der Sandbox, in der der Agent läuft. Für diese Komponente ist Sicherheit kein
Qualitätsmerkmal unter vielen; sie ist das Produkt selbst.

Es gab zwei realistische Wege. Erstens: einen fertigen, ausgereiften MITM-Proxy nehmen
(mitmproxy, Python) und die Fachlogik als Addon schreiben. Zweitens: einen eigenen Daemon in
einer speichersicheren Systemsprache bauen und für die Protokollarbeit eine Bibliothek
nutzen. Der Unterschied zwischen den Wegen liegt nicht im Umfang der Fachlogik, der ist in
beiden Fällen gleich, sondern in der Größe der Trusted Computing Base und in der Frage, ob
sich das Ergebnis als ein Paket ausliefern lässt.

Zielplattform ist der Linux-Desktop, ausgeliefert als `.deb` und AppImage, betrieben als
systemd-User-Dienst. Zielgruppe sind Fachleute ohne Sicherheitshintergrund, die ein Paket
installieren und danach nichts einrichten wollen.

## Entscheidung

Der Daemon `humanitld` ist ein Rust-Binary. Die MITM-Schicht ist
[hudsucker](https://github.com/omjadas/hudsucker) 0.25 mit den Features `rcgen-ca`,
`rustls-client`, `http2`. Alles darüber (Warteschlange der angehaltenen Anfragen,
Regel-Engine, Funde, Recorder, Audit-Kette, gRPC-Server, Sandbox-Steuerung) ist eigener Code
in den Crates unter `daemon/crates/`.

Verbindliches Protokollziel je Meilenstein, damit kein Sprintziel den Status
„experimentell" trägt:

| Meilenstein | Client-Seite | Upstream | Sonstiges |
|---|---|---|---|
| M1 | HTTP/1.1 (ALPN bietet dem Client nur `http/1.1` an) | HTTP/1.1 erzwungen | CONNECT, WebSocket-Durchleitung, SSE |
| M1 | gRPC | — | dokumentiertes Scheitern in der Konformitätsmatrix (`PROXY_007 h2 not available`) |
| M6 | HTTP/2 | HTTP/2 | holt gRPC aus `[experimental]` heraus |

Der Listener ist genau ein Unix-Socket, kein Loopback-TCP-Port auf dem Host. In seiner
Standardform lauscht hudsucker auf TCP; HUM-015 Schritt 0 klärt in einem Spike, ob hudsucker
0.25 einen generischen `Accept`-Stream annimmt. Wenn ja, lauscht der Proxy direkt auf dem
Unix-Socket. Wenn nein, wird die Accept-Schleife von hudsucker (etwa 100 Zeilen) als Fork im
Repository gehalten. In beiden Fällen bleibt die Tür in die Sandbox, was sie sein soll: eine
einzige Socket-Datei, die kein anderer Prozess auf dem Host erreichen kann.

## Begründung

Ein statisch gelinktes, speichersicheres Binary ist eine kleinere Trusted Computing Base als
ein Python-Bündel mit rund vierzig transitiven Paketen, und über seine Sicherheit lässt sich
leichter urteilen. Für ein Werkzeug, dessen ganzer Anspruch „hier kommt nichts raus" lautet,
ist die Größe der Angriffsfläche das zentrale Argument. Das Sicherheitsargument muss sich in
drei Sätzen erklären und mit einem Klick prüfen lassen (Grundsatz 2 in `BACKLOG.md` 1.3);
jede Zeile Fremdcode im vertrauenswürdigen Bereich macht diese drei Sätze länger.

hudsucker liefert genau die Protokollarbeit, die niemand selbst schreiben will:
CONNECT-Behandlung, Zertifikat-Cache über `rcgen`, HTTP/1 und HTTP/2 auf hyper 1,
WebSocket-Handler, gestreamte Bodys mit Trailern. Entscheidend ist die Form des Hooks:
`handle_request` ist `async`. Ein `await` auf einen `oneshot`-Kanal ist damit der natürliche
Haltepunkt für die Warteschlange, und eine angehaltene Anfrage kostet keinen Thread.

Die eigentliche Arbeit, also Warteschlange, Regeln, Funde, Recorder, Audit, gRPC, Sandbox,
müsste in jeder Sprache neu geschrieben werden. mitmproxy hätte uns davon nichts abgenommen.
Es hätte uns Grenzfälle des Protokolls abgenommen, dafür aber ein Paketierungsproblem
eingehandelt.

Der Preis der Entscheidung sind die MITM-Grenzfälle, die jetzt uns gehören. Gegenmaßnahme:
eine Konformitätsmatrix ab Sprint 1 (HUM-017), die `curl`, `websocat` und `grpcurl` gegen
einen axum-Fake-Upstream im selben Prozess laufen lässt: chunked, SSE, CONNECT, große Bodys,
WebSocket-Upgrade, Trailer. Testkorpus und Regelsemantik von mitmproxy bleiben als Referenz
nutzbar, auch ohne mitmproxy im Produkt.

## Verworfene Alternativen

- **mitmproxy als Addon-Host (Python).** Die Paketierung ist das K.-o.-Kriterium:
  Python-Laufzeit plus C-Erweiterungen kommen auf 80 bis 150 MB je Artefakt, und ein `.deb`,
  das seine eigene Python-Umgebung mitbringt, ist auf Dauer eine Wartungslast. Dazu kommen
  die Größe der TCB und die fehlende Speichersicherheit der C-Erweiterungen. Der Vorteil
  (ausgereifte Protokollimplementierung) wiegt das nicht auf, weil die Fachlogik ohnehin neu
  geschrieben wird.
- **Go mit goproxy.** goproxy ist veraltet, und HTTP/2-MITM in `net/http` ist mühsam bis
  unvollständig. Ein zweites Ökosystem im Repository (neben Dart für die Oberfläche) ohne
  klaren Gewinn.
- **Eigene MITM-Schicht direkt auf hyper, ohne hudsucker.** Wäre möglich, aber
  Zertifikat-Cache, CONNECT-Tunnel und WebSocket-Upgrade sind genau die Stellen, an denen
  subtile Fehler entstehen. hudsucker ist dünn genug, um es bei Bedarf zu forken (siehe die
  Listener-Frage), die Abhängigkeit ist also umkehrbar.
- **Node.js.** Speichersicher im Sinne von „keine Pufferüberläufe", aber der
  npm-Abhängigkeitsbaum und die Größe der Laufzeit wiederholen das Python-Problem.
- **Kein eigener Proxy, nur Firewall-Regeln.** Löst das Problem nicht: Eine Firewall sieht
  Hostnamen, aber keine Bodys von Anfragen. Gerade im Body findet die Ausleitung statt
  (siehe ADR-0005).

## Konsequenzen

- Rust wird die Sprache des ganzen Daemons, einschließlich Shim und Kommandozeile. Wer
  beiträgt, braucht Rust-Kenntnisse; dafür erzwingt der Compiler die Modulgrenzen aus
  ADR-0015.
- Die MITM-Konformität liegt in unserer Verantwortung. Sie wird nicht behauptet, sondern in
  HUM-017 gemessen; die Matrix ist Teil der Dokumentation und nennt auch die bewussten Lücken
  (gRPC bis M6).
- Die Bindung an hudsucker 0.25 ist eine Versionsbindung mit Bruchrisiko. Sie ist auf das
  Crate `humanitl-proxy` beschränkt (ADR-0015: keine Abstraktion über Fremdbibliotheken, aber
  eine Fremdbibliothek nur in einem Crate), damit ein Wechsel lokal bleibt.
- Die Listener-Frage ist der einzige offene Punkt dieser Entscheidung. Ihr Ausgang ändert
  nicht die Entscheidung, nur die Menge an geforktem Code.
- `experimental.h2_upstream` bleibt bis M6 der einzige Schalter, hinter dem h2 steht.
  Ungenutzte `experimental`-Schalter entfallen nach zwei Sprints (ADR-0015).

## Betroffene Issues

`HUM-015` (MITM-Proxy-Kern, Schritt 0 Listener-Spike), `HUM-013` (Bind des Proxy-Sockets),
`HUM-017` (Konformitätsmatrix), `HUM-021` (Demo-Skript M1), `HUM-056` (Fuzzing der Parser
und Decoder), `HUM-061` (Puffer für MITM-Grenzfälle).
