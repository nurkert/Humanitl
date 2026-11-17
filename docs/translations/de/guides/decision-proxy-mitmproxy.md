---
source: guides/decision-proxy-mitmproxy.md
source-sha256: 6bc84769cfe4de6b800096fd23b438d8b9011de6551c580ca0bd691f1be5b811
lang: de
---
# mitmproxy hinter Humanitl

Applies to: Humanitl 0.1

Diese Anleitung stellt [mitmproxy](https://mitmproxy.org/) hinter Humanitl, damit du
die Anfragen des Agenten in mitmproxy entscheidest statt in der App von Humanitl.
Humanitl tut weiter, was nur es kann: Es isoliert den Agenten, wendet deine Regeln
an und sperrt, was eine Regel `block` trifft. Jede Anfrage, die keine Regel
entscheidet, geht weiter an mitmproxy, und dort lässt du sie weiter oder lehnst sie
ab. Der Entwurf steht in [ADR-0022](../adr/0022-decision-proxy-behind-humanitl.md);
was das an der Sicherheitsaussage ändert, steht in
[SECURITY.md](../SECURITY.md#the-proxy-behind-decides-holdask_mode--upstream-hum-512).

Nichts davon ist von Haus aus eingeschaltet. Ohne `egress.proxy` ist Humanitl der
einzige Proxy, und jede Anfrage ohne Regel wartet in Humanitl auf dich.

Die Schritte mit mitmproxy sind mit mitmproxy 12.3.1 unter Linux ausprobiert; wo
diese Anleitung „beobachtet" sagt, hat genau diese Version das getan. Jede andere
Aussage über mitmproxy verweist auf dessen Dokumentation.

## Wie die Teile zusammenpassen

```text
Agent ──▶ Humanitl ──▶ mitmproxy ──▶ Ziel
     TLS mit der        TLS mit der        TLS, geprüft
     Humanitl-CA        mitmproxy-CA       von mitmproxy
```

Es gibt zwei CAs, und jede Seite vertraut genau einer davon:

| Verbindung | Wer ein Zertifikat zeigt | Ausgestellt von | Wer ihm vertraut |
|---|---|---|---|
| Agent zu Humanitl | Humanitl, für den Namen, den der Agent wollte | der Humanitl-CA | der Agent, in der Sandbox |
| Humanitl zu mitmproxy | mitmproxy, für denselben Namen | der mitmproxy-CA | Humanitl, über `egress.proxy_ca`, nur auf diesem Weg |
| mitmproxy zum Ziel | der echte Server | einer öffentlichen CA | mitmproxy, das es prüft, solange seine Option `ssl_insecure` nicht gesetzt ist |

Humanitl öffnet das TLS des Agenten, wendet Regeln und Geheimniserkennung auf die
Anfrage im Klartext an und baut dann eine neue TLS-Verbindung durch mitmproxy auf,
mit dem Namen des Hosts in SNI. mitmproxy öffnet diese wieder, zeigt dir die Anfrage
und schickt sie, wenn du sie weiterlässt, an das Ziel. Der Agent vertraut der
mitmproxy-CA nie und erreicht mitmproxy nie selbst; er hat weiter genau einen Weg
hinaus, Humanitl. Humanitl vertraut der mitmproxy-CA nur auf Verbindungen über
`egress.proxy`, nie auf einer direkten.

## 1. mitmproxy installieren und starten

Installiere mitmproxy, wie es die
[Installationsanleitung](https://docs.mitmproxy.org/stable/overview/installation/)
beschreibt; unter Linux die eigenständigen Programme von mitmproxy.org oder
`uv tool install mitmproxy`. Beides bringt drei Werkzeuge mit: `mitmproxy`
(Konsole), `mitmweb` (Oberfläche im Browser) und `mitmdump` (ohne Oberfläche, kann
also keine Anfrage für dich anhalten).

Starte es als dein eigener Nutzer im regulären Proxy-Modus, der Vorgabe, die
[auf Port 8080 lauscht](https://docs.mitmproxy.org/stable/concepts/modes/):

```sh
mitmproxy --listen-host 127.0.0.1 --listen-port 8080 \
  --set connection_strategy=lazy --set upstream_cert=false
```

`--listen-host` und `--listen-port` setzen die
[Optionen](https://docs.mitmproxy.org/stable/concepts/options/) `listen_host` und
`listen_port`. Beobachtet: Ohne `--listen-host` lauscht mitmproxy auf jeder
Schnittstelle (`*:8080`), jeder in deinem Netz könnte ihn also nutzen; `127.0.0.1`
hält ihn auf diesem Rechner, wo Humanitl ihn erreicht. Mit `mitmweb` geht dasselbe;
seine Oberfläche liegt von Haus aus auf Port 8081 (Option `web_port`).

Die beiden `--set`-Optionen halten mitmproxy davon ab, das Ziel vor deiner
Entscheidung zu kontaktieren. Von Haus aus tut er das: Seine
[Option](https://docs.mitmproxy.org/stable/concepts/options/) `connection_strategy`
steht auf `eager`, die Verbindung zum Server entsteht so früh wie möglich, und
`upstream_cert` ist an, mitmproxy verbindet sich also noch während des
TLS-Handschlags mit Humanitl zum Ziel, um dessen Zertifikat nachzusehen
([certificate sniffing](https://docs.mitmproxy.org/stable/concepts/how-mitmproxy-works/)).
Beides geschieht, bevor es die Anfrage im Tunnel gibt, also bevor ein Abfangfilter
sie anhalten kann: Der Name wird aufgelöst und das Ziel kontaktiert, auch für eine
Anfrage, die du danach beendest. `connection_strategy=lazy` schiebt die Verbindung
zum Server „so lange wie möglich" auf.

Beobachtet mit dem Abfangfilter `~q` und einem lokalen TLS-Ziel, das seine
angenommenen Verbindungen zählte, für ein `CONNECT` zum Namen und eines zur Adresse:

| Optionen von mitmproxy | Verbindungen am Ziel vor der Entscheidung | Nach dem Weiterlassen |
|---|---|---|
| Vorgaben | 1 | 1 |
| nur `upstream_cert=false` | 1 | 1 |
| nur `connection_strategy=lazy` | 0 | 1 |
| beide | 0 | 1 |

In 12.3.1 genügte `connection_strategy=lazy` allein; diese Anleitung setzt
`upstream_cert=false` trotzdem, weil diese Option nichts anderes ist als eine
Verbindung zum Ziel. Was es kostet: mitmproxy baut sein Zertifikat für Humanitl nur
aus dem Namen in SNI, statt die Namen aus dem Zertifikat des Ziels zu übernehmen.
Humanitl schickt in SNI immer den Namen des Hosts und prüft das Zertifikat gegen
genau diesen Namen, die Verbindung lässt sich also weiter prüfen; verloren gehen nur
weitere Namen und, mit `lazy`, das Spiegeln der ALPN-Wahl des Ziels (beides in den
oben verlinkten Texten der Optionen). Das Zertifikat des Ziels selbst prüft
mitmproxy weiterhin, wenn er nach deiner Entscheidung verbindet.

## 2. Die CA von mitmproxy finden

Beim [ersten Start](https://docs.mitmproxy.org/stable/concepts/certificates/) legt
mitmproxy seine CA in seinem Konfigurationsverzeichnis an, von Haus aus
`~/.mitmproxy` (Option `confdir`). Humanitl braucht das Zertifikat im PEM-Format:

```sh
ls ~/.mitmproxy/mitmproxy-ca-cert.pem
```

Gib Humanitl nie `mitmproxy-ca.pem`: Diese Datei enthält auch den privaten
Schlüssel. Humanitl liest aus der Datei nur Zertifikate und braucht nichts weiter.

## 3. Humanitl auf mitmproxy richten

Auf der Kommandozeile jeden Schlüssel mit `humanitl config set`. `egress.proxy_ca`
muss ein absoluter Pfad sein, also lass die Shell `$HOME` einsetzen:

```sh
humanitl config set egress.proxy http://127.0.0.1:8080
humanitl config set egress.proxy_ca "$HOME/.mitmproxy/mitmproxy-ca-cert.pem"
humanitl config set egress.proxy_resolve proxy     # die Vorgabe; siehe „Wer Namen auflöst"
```

Oder in `config.toml`:

```toml
[egress]
proxy = "http://127.0.0.1:8080"
proxy_ca = "/home/du/.mitmproxy/mitmproxy-ca-cert.pem"
proxy_resolve = "proxy"
proxy_response_timeout_secs = 300
```

In der App: Einstellungen (`Ctrl+,`), Abschnitt „Entscheidungs-Proxy". „Adresse"
nimmt `http://127.0.0.1:8080`; „CA-Zertifikat" hat eine Dateiauswahl für
`~/.mitmproxy/mitmproxy-ca-cert.pem`; „Wer Namen auflöst" ist „Proxy" oder
„Humanitl". „Zugangsdaten" braucht nur ein Proxy, der eine Anmeldung verlangt
(mitmproxy tut das nur mit seiner Option `proxyauth`).

Jeder Schlüssel steht in [CONFIG.md](../CONFIG.md#egress). Die Schlüssel sind
`x-project-scope = denied`: Ein Projektprofil in einem geklonten Repository kann
sie weder setzen noch ändern.

## 4. mitmproxy über Anfragen ohne Regel entscheiden lassen

Bis hier verlässt jede Anfrage, die eine Regel oder du freigegeben hast, den Rechner
über mitmproxy, aber Anfragen ohne Regel warten noch in Humanitl. Um sie an
mitmproxy zu geben, setze den Fragemodus `upstream`:

```sh
humanitl config set hold.ask_mode upstream
```

In der App: Einstellungen, Abschnitt „Anfragen", „Fragen", die vierte Wahl „Proxy
dahinter". Sie ist erst wählbar, wenn unter „Entscheidungs-Proxy" eine Adresse
steht. Ohne `egress.proxy` lehnt Humanitl `upstream` ab (`CONFIG_003`).

Was die Regeln in diesem Modus weiter tun:

| Regel | Was geschieht |
|---|---|
| `block` | Humanitl antwortet `403`; mitmproxy sieht die Anfrage nie. Ein gesperrter Name wird nicht einmal aufgelöst. |
| `allow` | Geht sofort über mitmproxy hinaus, ohne Halt. |
| `ask` oder keine Regel | Geht ohne Halt in Humanitl an mitmproxy. Du entscheidest dort. |
| `redact` | Hält weiter in Humanitl, weil du die Anfrage nur dort bearbeiten kannst. |

Mit `hold.hard_block_checksum_secrets` sperrt das System eine Anfrage mit einem
prüfsummen-sicheren Geheimnis (`403`); mitmproxy sieht kein Byte davon.

## 5. Den Daemon neu starten und prüfen

Die Schlüssel `egress.*` wirken, wenn der Daemon startet. Starte ihn neu, solange
keine Session läuft:

```sh
humanitl daemon restart
humanitl egress status
humanitl egress check
```

`humanitl egress status` zeigt, was in `config.toml` steht und welchen Weg hinaus
der laufende Daemon nimmt. `humanitl egress check` baut durch mitmproxy einen
Tunnel zu `example.com:443` und darin den TLS-Handschlag auf; es schickt keine
Anfrage, in mitmproxy wartet also nichts auf dich. Ein Fehler nennt seine Ursache
mit einer Abhilfe, etwa einen Proxy, der nicht erreichbar ist, oder ein Zertifikat,
das nicht zu `egress.proxy_ca` passt. Die App hat dieselbe Prüfung unter
„Entscheidungs-Proxy", „Verbindung prüfen".

Die Moduszeile jeder Session sagt danach, wer entscheidet: `humanitl sessions` und
`humanitl sandbox plan` schreiben `without a rule: decided by 127.0.0.1:8080`, und
der Isolationsreiter der App nennt den Proxy, wer Namen auflöst und was daraus folgt.

## 6. In mitmproxy abfangen und entscheiden

mitmproxy leitet alles weiter, bis du einen Abfangfilter setzt. Drücke in der
Konsole `i` und trage zwischen die Anführungszeichen von `set intercept ''` einen
[Filterausdruck](https://docs.mitmproxy.org/stable/concepts/filters/) ein, wie es
das [Tutorial zum Abfangen](https://docs.mitmproxy.org/stable/mitmproxytutorial-interceptrequests/)
zeigt. Oder setze ihn beim Start (Option `intercept`):

```sh
mitmproxy --listen-host 127.0.0.1 --listen-port 8080 \
  --set connection_strategy=lazy --set upstream_cert=false \
  --set intercept='~q'
```

`~q` trifft jede Anfrage, die noch keine Antwort hat, also wartet jede Anfrage, die
Humanitl weiterreicht, auf dich. `~q & ~d example\.com` hält nur Anfragen an diese
Domain an (`~d` ist die Domain, `&` ist und). Beobachtet: `~d` trifft den Namen auch
dann, wenn Humanitl zur Adresse verbindet (`egress.proxy_resolve = "humanitl"`),
weil die Anfrage im Tunnel den Namen in `Host` trägt.

mitmproxy weiß nicht, welche Anfragen eine Regel in Humanitl freigegeben hat: Sein
Filter gilt für alles, was ankommt, auch für die Durchreiche zum Sprachmodell und
für Regeln `allow`. Soll der Verkehr zum Modellserver nicht auf dich warten, lass
ihn aus dem Filter heraus, etwa `~q & !~d api\.example-llm\.com` (`!` ist nicht).

An einer angehaltenen Anfrage, wie im Tutorial beschrieben:

- `a` lässt sie unverändert weiter; mitmproxy schickt sie an das Ziel, und der
  Agent bekommt dessen Antwort.
- `X` beendet sie („kill"), sie wird nicht weitergeleitet.

`mitmweb` nimmt dieselbe Option `intercept` und lässt eine angehaltene Anfrage im
Browser weiter oder beendet sie.

Um mit einer klaren Antwort abzulehnen statt zu beenden, nimm die Option
[`block_list`](https://docs.mitmproxy.org/stable/overview/features/) mit einem
Filter und einem Statuscode. mitmproxy antwortet dann selbst, und das Ziel sieht
die Anfrage nie:

```sh
mitmproxy --listen-host 127.0.0.1 --listen-port 8080 \
  --set connection_strategy=lazy --set upstream_cert=false \
  --set intercept='~q' \
  --set 'block_list=:~d tracker\.example$:403'
```

Der Statuscode `444` lässt mitmproxy ohne Antwort auflegen.

## 7. Was der Agent sieht

| In mitmproxy | Der Agent bekommt |
|---|---|
| weitergelassen | die Antwort des Ziels, unverändert |
| abgelehnt mit `block_list` und `403` | das `403` von mitmproxy, unverändert, sofort |
| `block_list` mit `444` | `502` von Humanitl, sofort |
| beendet, während sie angehalten war (`X`) | `502` von Humanitl, nach `egress.proxy_response_timeout_secs` |
| niemand entscheidet | `502` von Humanitl, nach `egress.proxy_response_timeout_secs` |

Beobachtet: Eine Anfrage, die beendet wurde, während sie angehalten war, ging nicht
weiter, aber ihre Verbindung blieb offen, also wartet Humanitl bis zur Frist. In
mitmproxy 12.3.1 markiert
[`Flow.kill()`](https://github.com/mitmproxy/mitmproxy/blob/v12.3.1/mitmproxy/flow.py)
einen angehaltenen Fluss als nicht mehr abgefangen, ohne ihn fortzusetzen, und ein
Kommentar dort nennt die Art, wie das Beenden signalisiert wird, „not ideal"
([mitmproxy#4711](https://github.com/mitmproxy/mitmproxy/issues/4711)). Soll der
Agent eine Ablehnung sofort erfahren, lehne mit `block_list` ab.

`egress.proxy_response_timeout_secs` (Vorgabe 300 Sekunden) ist, wie lange Humanitl
auf den Kopf der Antwort von mitmproxy wartet, weil dort ein Mensch entscheiden kann.
Das Briefing des Agenten sagt ihm dasselbe: warten, dann `502`; ein `403` ist
endgültig.

Humanitl zeichnet eine solche Anfrage mit der Entscheidungsquelle `upstream` auf (im
Audit `decision: forward`, `decided_by: upstream`) und mit dem Status der Antwort
von mitmproxy. Es behauptet keine Freigabe und keine Sperre, die es nicht gesehen
hat; der Verlauf in der App zeigt „Weitergereicht, Proxy antwortete 403".

## Wer Namen auflöst

`egress.proxy_resolve` entscheidet, wer den Namen des Ziels in eine Adresse auflöst.

| | `proxy` (Vorgabe) | `humanitl` |
|---|---|---|
| Was Humanitl schickt | `CONNECT example.com:443` | `CONNECT 93.184.215.14:443`, den Namen in SNI |
| DNS-Anfrage aus Humanitl | keine; mitmproxy löst erst nach deiner Entscheidung auf, nur mit den Optionen aus Schritt 1 | eine, nach den eigenen Regeln von Humanitl, vor deiner Entscheidung in mitmproxy |
| Private Ziele | Humanitl lehnt private IP-Literale und lokale Namen ab (`localhost`, `*.local`, `*.internal`, Namen ohne Punkt); ein öffentlicher Name, der in dein Netz zeigt, ist Sache von mitmproxy | Humanitl prüft auch die aufgelöste Adresse, wie ohne mitmproxy |
| Was mitmproxy als Host zeigt | den Namen | die Adresse (beobachtet); der Name steht in `Host` |

mitmproxy kommt mit beiden Formen zurecht: Ein `CONNECT` zu einer Adresse ist für
ihn [legitim](https://docs.mitmproxy.org/stable/concepts/how-mitmproxy-works/), und
er nimmt den Namen aus SNI, um das Ziel zu verbinden. Beobachtet: Bei einem
`CONNECT` zu `127.0.0.1` mit SNI `localhost` prüfte mitmproxy das Zertifikat des
Ziels gegen `localhost`.

Nimm `proxy`, wenn keine DNS-Anfrage hinausgehen soll, bevor du entschieden hast.
Das gilt nur mit mitmproxy, gestartet wie in Schritt 1, mit
`connection_strategy=lazy` und `upstream_cert=false`: Mit seinen Vorgaben löst
mitmproxy den Namen auf und kontaktiert das Ziel schon während des
TLS-Handschlags, vor der Entscheidung. Mit `humanitl` hat Humanitl den Namen
schon aufgelöst, aber mit seinen Vorgaben kontaktiert mitmproxy das Ziel auch
dann vor der Entscheidung.
mitmproxy hat selbst keinen Schutz vor privaten Zielen (seine
[Option](https://docs.mitmproxy.org/stable/concepts/options/) `block_private` gilt
Clients, die sich mit ihm verbinden, nicht Zielen), ein öffentlicher Name, der auf
`127.0.0.1` oder deinen Router zeigt, erreicht ihn also, wenn du die Anfrage
weiterlässt. Nimm `humanitl`, wenn dir dein Netz wichtiger ist als die Anfrage beim
Namensdienst: Dann ist nichts Privates erreichbar, und der Name geht als DNS-Anfrage
hinaus, bevor du die Anfrage siehst.

## Grenzen

Sie stehen in [SECURITY.md](../SECURITY.md#the-proxy-behind-decides-holdask_mode--upstream-hum-512)
und im [Bedrohungsmodell](../THREAT-MODEL.md); kurz:

- mitmproxy sieht den entschlüsselten Verkehr, auch Geheimnisse, die Humanitl nicht
  gesperrt hat, und die Durchreiche zum Sprachmodell. Wer die Flüsse von mitmproxy
  lesen kann, kann sie lesen.
- Die Isolierung wandert nicht: Der Agent erreicht nur Humanitl, die Verbindung zu
  mitmproxy beginnt im Daemon.
- Die für eine Session erklärte Durchreiche zum Sprachmodell ist ein `allow`, das
  vor deinen eigenen Sperrregeln steht; sie geht über mitmproxy hinaus und wird als
  `passthrough` aufgezeichnet.
- Anfragen, die der Daemon selbst stellt (Suche nach Modellservern,
  Update-Prüfung), bleiben direkt.
- Humanitl spricht mit mitmproxy nur `http://`; TLS zum Proxy selbst und SOCKS gibt
  es nicht.
- Läuft mitmproxy nicht, scheitern Anfragen mit einem Befund; sie gehen nie
  stattdessen direkt hinaus.
- Pausiert jemand das Abfangen in Humanitl, geht auch eine Anfrage mit `redact` an
  mitmproxy, so wie alles Wartende hinausgeht.

## Zurück

Entferne die Schlüssel und starte den Daemon neu; Humanitl ist wieder der einzige
Proxy:

```sh
humanitl config set hold.ask_mode ui
humanitl config set egress.proxy_ca -
humanitl config set egress.proxy -
humanitl daemon restart
```

`-` (oder `null`) entfernt einen Schlüssel aus `config.toml`.
