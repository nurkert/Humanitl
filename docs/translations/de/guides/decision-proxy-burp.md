---
source: guides/decision-proxy-burp.md
source-sha256: 817f82013da8d8f6fac67993af0fb81dbfbfc54800563fbb6beedba054ea93db
lang: de
---
# Burp Suite hinter Humanitl

Applies to: Humanitl 0.1

> **Noch nicht mit einem aktuellen Build ausprobiert.** Diese Anleitung beruht
> allein auf der Dokumentation von PortSwigger. Noch niemand hat diese Schritte mit
> einer aktuellen Burp Suite durchgespielt, deshalb gilt Burp nicht als unterstützt
> ([ADR-0022](../adr/0022-decision-proxy-behind-humanitl.md)). Jede Aussage über
> Burp verweist auf die Dokumentation von PortSwigger oder ist als „im Probelauf zu
> bestätigen" markiert; die offenen Punkte stehen gesammelt unter
> [Offen bis zum Probelauf](#open-until-the-trial). Datum, Burp-Version und das
> Beobachtete kommen in die Anleitung, sobald der Probelauf gelaufen ist.

Diese Anleitung stellt [Burp Suite](https://portswigger.net/burp/documentation/desktop/tools/proxy)
hinter Humanitl, damit du die Anfragen des Agenten in Burp entscheidest statt in
der App von Humanitl. Humanitl tut weiter, was nur es kann: Es isoliert den
Agenten, wendet deine Regeln an und sperrt, was eine Regel `block` trifft. Jede
Anfrage, die keine Regel entscheidet, geht weiter an Burp, und dort leitest du sie
weiter („Forward") oder verwirfst sie („Drop"). Der Entwurf steht in
[ADR-0022](../adr/0022-decision-proxy-behind-humanitl.md); was das an der
Sicherheitsaussage ändert, steht in
[SECURITY.md](../SECURITY.md#the-proxy-behind-decides-holdask_mode--upstream-hum-512).

Die Seite von Humanitl ist dieselbe wie mit mitmproxy und steht ausführlicher in
[mitmproxy hinter Humanitl](decision-proxy-mitmproxy.md); diese Anleitung
wiederholt nur, was du für Burp brauchst. Nichts davon ist von Haus aus
eingeschaltet. Ohne `egress.proxy` ist Humanitl der einzige Proxy, und jede Anfrage
ohne Regel wartet in Humanitl auf dich.

## Wie die Teile zusammenpassen

```text
Agent ──▶ Humanitl ──▶ Burp ──▶ Ziel
     TLS mit der        TLS mit der      TLS zum Ziel;
     Humanitl-CA        Burp-CA          geprüft? siehe unten
```

Es gibt zwei CAs, und jede Seite vertraut genau einer davon:

| Verbindung | Wer ein Zertifikat zeigt | Ausgestellt von | Wer ihm vertraut |
|---|---|---|---|
| Agent zu Humanitl | Humanitl, für den Namen, den der Agent wollte | der Humanitl-CA | der Agent, in der Sandbox |
| Humanitl zu Burp | Burp, ein Zertifikat, das es je Host erzeugt | der Burp-CA dieser Installation | Humanitl, über `egress.proxy_ca`, nur auf diesem Weg |
| Burp zum Ziel | der echte Server | einer öffentlichen CA | Burp, nur wenn es das TLS zum Ziel prüft (im Probelauf zu bestätigen) |

Der Proxy-Listener von Burp erzeugt von Haus aus
[„CA-signed per-host certificates"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy),
ausgestellt von der CA, die
[„each installation of Burp generates"](https://portswigger.net/burp/documentation/desktop/tools/proxy/manage-certificates).
Humanitl öffnet das TLS des Agenten, wendet Regeln und Geheimniserkennung auf die
Anfrage im Klartext an und baut dann eine neue TLS-Verbindung durch Burp auf, mit
dem Namen des Hosts in SNI. Burp öffnet diese wieder, zeigt dir die Anfrage und
schickt sie, wenn du sie weiterleitest, an das Ziel. Der Agent vertraut der
Burp-CA nie und erreicht Burp nie selbst; er hat weiter genau einen Weg hinaus,
Humanitl. Humanitl vertraut der Burp-CA nur auf Verbindungen über `egress.proxy`,
nie auf einer direkten.

Ob Burp das Zertifikat des Ziels prüft, ist offen. Seine TLS-Einstellungen haben
eine Option „Verify upstream TLS" und eine Liste
[„additional certificate authorities that Burp trusts when validating upstream TLS connections"](https://portswigger.net/burp/documentation/desktop/settings/network/tls);
ob die Prüfung von Haus aus an ist, sagt die Dokumentation nicht (im Probelauf zu
bestätigen). Prüft Burp nicht, prüft niemand die Verbindung von Burp zum Ziel, und
Humanitl kann das nicht ausgleichen: Es sieht nur das Zertifikat von Burp, nie das
des Ziels.

## 1. Burp starten und den Listener prüfen

Starte Burp Suite als dein eigener Nutzer. Von Haus aus legt Burp einen einzigen
Listener auf Port 8080 der Loopback-Schnittstelle an
([„creates a single listener on port 8080 of the loopback interface"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy)),
also dort, wo Humanitl ihn erreicht. Prüfe das unter Proxy settings, „Proxy
listeners": Der Listener sollte nur an die Loopback-Schnittstelle gebunden sein.
An alle Schnittstellen gebunden, können ihn andere Rechner in deinem Netz nutzen.

Lass die Einstellungen „Request handling" des Listeners („Redirect to host",
„Redirect to port", „Force use of TLS", unsichtbares Proxying) aus. Humanitl ist
ein Client, der den Proxy kennt: Es schickt `CONNECT` vor dem TLS, und das ist der
Fall, den Burp ohne
[invisible proxying](https://portswigger.net/burp/documentation/desktop/tools/proxy/invisible)
behandelt.

Ob Burp das Ziel kontaktiert, bevor du entscheidest, ist offen: Die Dokumentation
sagt nicht, wann Burp für eine angehaltene Anfrage die Verbindung zum Server
öffnet (im Probelauf zu bestätigen). mitmproxy tut es mit seinen Vorgaben, siehe
[Schritt 1 der mitmproxy-Anleitung](decision-proxy-mitmproxy.md#1-install-and-start-mitmproxy).

## 2. Die CA von Burp exportieren

Humanitl braucht das Zertifikat der Burp-CA im PEM-Format. Burp exportiert es aus
den Listener-Einstellungen, wie es
[Managing CA certificates](https://portswigger.net/burp/documentation/desktop/tools/proxy/manage-certificates)
beschreibt:

1. Im Reiter Proxy die Proxy settings öffnen.
2. Unter „Proxy listeners" auf „Import / export CA certificate" klicken.
3. Unter „Export" das Zertifikat im DER-Format wählen, nicht das mit dem privaten
   Schlüssel, und speichern, etwa als `~/burp/burp-ca.der`. (Die Namen der
   Exportwahlen stehen nicht auf jener Seite; im Probelauf zu bestätigen.)

Mit OpenSSL nach PEM umwandeln:

```sh
openssl x509 -inform der -in ~/burp/burp-ca.der -out ~/burp/burp-ca.pem
```

Gib Humanitl nie eine Datei, die den privaten Schlüssel enthält. Humanitl liest aus
der Datei nur Zertifikate und braucht nichts weiter.

## 3. Humanitl auf Burp richten

Auf der Kommandozeile jeden Schlüssel mit `humanitl config set`. `egress.proxy_ca`
muss ein absoluter Pfad sein, also lass die Shell `$HOME` einsetzen:

```sh
humanitl config set egress.proxy http://127.0.0.1:8080
humanitl config set egress.proxy_ca "$HOME/burp/burp-ca.pem"
humanitl config set egress.proxy_resolve proxy     # die Vorgabe; siehe „Wer Namen auflöst"
```

Oder in `config.toml`:

```toml
[egress]
proxy = "http://127.0.0.1:8080"
proxy_ca = "/home/du/burp/burp-ca.pem"
proxy_resolve = "proxy"
proxy_response_timeout_secs = 300
```

In der App: Einstellungen (`Ctrl+,`), Abschnitt „Entscheidungs-Proxy". „Adresse"
nimmt `http://127.0.0.1:8080`; „CA-Zertifikat" hat eine Dateiauswahl für die
PEM-Datei; „Wer Namen auflöst" ist „Proxy" oder „Humanitl". „Zugangsdaten" bleibt
leer: Der vorgegebene Listener von Burp verlangt keine Anmeldung.

Jeder Schlüssel steht in [CONFIG.md](../CONFIG.md#egress). Die Schlüssel sind
`x-project-scope = denied`: Ein Projektprofil in einem geklonten Repository kann
sie weder setzen noch ändern.

## 4. Burp über Anfragen ohne Regel entscheiden lassen

Setze den Fragemodus `upstream`, damit Anfragen ohne Regel an Burp gehen, statt in
Humanitl zu warten:

```sh
humanitl config set hold.ask_mode upstream
```

In der App: Einstellungen, Abschnitt „Anfragen", „Fragen", die vierte Wahl „Proxy
dahinter". Ohne `egress.proxy` lehnt Humanitl `upstream` ab (`CONFIG_003`). Was die
Regeln in diesem Modus weiter tun, ist dasselbe wie mit mitmproxy, siehe
[Schritt 4 der mitmproxy-Anleitung](decision-proxy-mitmproxy.md#4-let-mitmproxy-decide-requests-without-a-rule):
`block` antwortet in Humanitl mit `403`, und Burp sieht die Anfrage nie; `allow`
geht sofort über Burp hinaus; `ask` oder keine Regel geht ohne Halt an Burp;
`redact` hält weiter in Humanitl.

## 5. Den Daemon neu starten und prüfen

Die Schlüssel `egress.*` wirken, wenn der Daemon startet. Starte ihn neu, solange
keine Session läuft:

```sh
humanitl daemon restart
humanitl egress status
humanitl egress check
```

`humanitl egress check` baut durch Burp einen Tunnel zu `example.com:443` und darin
den TLS-Handschlag auf; es schickt keine Anfrage, in Burp wartet also nichts auf
dich. Ein Fehler nennt seine Ursache mit einer Abhilfe, etwa einen Proxy, der nicht
erreichbar ist, oder ein Zertifikat, das nicht zu `egress.proxy_ca` passt. Die App
hat dieselbe Prüfung unter „Entscheidungs-Proxy", „Verbindung prüfen". Dass die
Prüfung gegen Burp besteht, ist im Probelauf zu bestätigen.

## 6. In Burp abfangen und entscheiden

Unter Proxy, Intercept schaltet der Knopf „Intercept on/off" das Abfangen:
[„If the button shows Intercept on, messages are intercepted"](https://portswigger.net/burp/documentation/desktop/tools/proxy/intercept-messages);
mit „Intercept off" leitet Burp alles von selbst weiter. Ob das Abfangen beim Start
von Burp an ist, ist eine Einstellung,
[„Default Proxy interception state"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy).

An einer angehaltenen Anfrage, wie
[Intercepting messages](https://portswigger.net/burp/documentation/desktop/tools/proxy/intercept-messages)
es beschreibt:

- „Forward" schickt sie an das Ziel, und der Agent bekommt dessen Antwort.
- „Drop" verwirft sie, „so that they never reach the target server".

Abfangregeln in den Proxy settings
[„control which messages are stalled"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy)
für dich. Welche Regeln ein frisches Burp mitbringt und welche Anfragen es also
ohne Frage durchlässt, ist im Probelauf zu bestätigen.

Burp weiß nicht, welche Anfragen eine Regel in Humanitl freigegeben hat: Sein
Abfangen gilt für alles, was ankommt, auch für die Durchreiche zum Sprachmodell und
für Regeln `allow`. Soll der Verkehr zum Modellserver nicht auf dich warten, lege
eine Abfangregel an, die seinen Host auslässt.

## 7. Was der Agent sieht

| In Burp | Der Agent bekommt |
|---|---|
| weitergeleitet | die Antwort des Ziels, unverändert |
| verworfen | im Probelauf zu bestätigen: eine Antwort von Burp, eine geschlossene Verbindung (`502` von Humanitl, sofort) oder `502` von Humanitl nach `egress.proxy_response_timeout_secs` |
| niemand entscheidet | `502` von Humanitl, nach `egress.proxy_response_timeout_secs` |

Die letzte Zeile ist das Verhalten von Humanitl selbst und gilt für jeden Proxy
dahinter. `egress.proxy_response_timeout_secs` (Vorgabe 300 Sekunden) ist, wie
lange Humanitl auf den Kopf der Antwort von Burp wartet, weil dort ein Mensch
entscheiden kann. Das Briefing des Agenten sagt ihm dasselbe: warten, dann `502`;
ein `403` ist endgültig.

Humanitl zeichnet eine solche Anfrage mit der Entscheidungsquelle `upstream` auf
(im Audit `decision: forward`, `decided_by: upstream`) und mit dem Status der
Antwort von Burp. Es behauptet keine Freigabe und keine Sperre, die es nicht
gesehen hat.

## Wer Namen auflöst

`egress.proxy_resolve` entscheidet, wer den Namen des Ziels in eine Adresse auflöst.

| | `proxy` (Vorgabe) | `humanitl` |
|---|---|---|
| Was Humanitl schickt | `CONNECT example.com:443` | `CONNECT 93.184.215.14:443`, den Namen in SNI |
| DNS-Anfrage aus Humanitl | keine; wann Burp auflöst, ist im Probelauf zu bestätigen | eine, nach den eigenen Regeln von Humanitl, vor deiner Entscheidung in Burp |
| Private Ziele | Humanitl lehnt private IP-Literale und lokale Namen ab; ein öffentlicher Name, der in dein Netz zeigt, ist Sache von Burp | Humanitl prüft auch die aufgelöste Adresse, wie ohne Burp |
| Geht mit Burp | im Probelauf zu bestätigen | im Probelauf zu bestätigen |

Das `CONNECT` mit dem Namen ist der
[übliche Weg](https://portswigger.net/burp/documentation/desktop/tools/proxy/invisible),
wie ein Client mit Burp spricht: „clients send a CONNECT request that identifies
the destination host and then perform TLS negotiation". Mit `humanitl` nennt das
`CONNECT` nur eine Adresse, und das Zertifikat von Burp muss trotzdem den Namen
tragen, weil Humanitl es gegen den Namen prüft. Für Clients ohne `CONNECT` nimmt
Burp den Namen aus SNI, „to generate a certificate for that host"
([Invisible proxying](https://portswigger.net/burp/documentation/desktop/tools/proxy/invisible));
ob es nach einem `CONNECT` zu einer Adresse dasselbe tut, ist im Probelauf zu
bestätigen. Tut es das nicht, scheitert `humanitl egress check` mit einem
Zertifikat, das nicht passt.

Nimm `proxy`, wenn keine DNS-Anfrage Humanitl verlassen soll, bevor du entschieden
hast; ob Burp dann mit dem Auflösen auf deine Entscheidung wartet, ist offen, siehe
Schritt 1. Nimm `humanitl`, wenn dir dein Netz wichtiger ist als die Anfrage beim
Namensdienst: Dann ist nichts Privates erreichbar, und der Name geht als
DNS-Anfrage hinaus, bevor du die Anfrage siehst.

## Grenzen

Sie stehen in [SECURITY.md](../SECURITY.md#the-proxy-behind-decides-holdask_mode--upstream-hum-512)
und im [Bedrohungsmodell](../THREAT-MODEL.md); kurz:

- Burp sieht den entschlüsselten Verkehr, auch Geheimnisse, die Humanitl nicht
  gesperrt hat, und die Durchreiche zum Sprachmodell. Seine Projektdatei und sein
  HTTP-Verlauf enthalten sie; wer diese lesen kann, kann sie lesen.
- Ob Burp das Zertifikat des Ziels prüft, ist offen (siehe
  [Wie die Teile zusammenpassen](#how-the-pieces-fit)). Prüft es nicht, ist die
  Verbindung von Burp zum Ziel nicht geprüft, und Humanitl kann das nicht
  ausgleichen.
- Die Isolierung wandert nicht: Der Agent erreicht nur Humanitl, die Verbindung zu
  Burp beginnt im Daemon.
- Die für eine Session erklärte Durchreiche zum Sprachmodell ist ein `allow`, das
  vor deinen eigenen Sperrregeln steht; sie geht über Burp hinaus und wird als
  `passthrough` aufgezeichnet.
- Anfragen, die der Daemon selbst stellt (Suche nach Modellservern,
  Update-Prüfung), bleiben direkt.
- Humanitl spricht mit Burp nur `http://`; TLS zum Proxy selbst und SOCKS gibt es
  nicht.
- Läuft Burp nicht, scheitern Anfragen mit einem Befund; sie gehen nie
  stattdessen direkt hinaus.
- Pausiert jemand das Abfangen in Humanitl, geht auch eine Anfrage mit `redact` an
  Burp, so wie alles Wartende hinausgeht.

## Offen bis zum Probelauf

Der Probelauf auf einem aktuellen Burp-Build beantwortet diese Fragen; die
Antworten ersetzen die Markierungen oben:

- Besteht `humanitl egress check`, mit `egress.proxy_resolve` auf `proxy` und auf
  `humanitl`?
- Erreicht eine weitergeleitete Anfrage den Agenten, und bleibt eine verworfene vom
  Ziel fern, mit beiden Werten?
- Prüft Burp von Haus aus das Zertifikat des Ziels („Verify upstream TLS")?
- Kontaktiert Burp das Ziel, bevor du eine angehaltene Anfrage weiterleitest?
- Was bekommt der Agent, wenn du eine Anfrage verwirfst, und nach welcher Zeit?
- Mit welchen Abfangregeln und welchem Abfangzustand startet ein frisches Burp, und
  wie heißen die Wahlen beim Export der CA?

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
