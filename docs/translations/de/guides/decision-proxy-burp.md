---
source: guides/decision-proxy-burp.md
source-sha256: 7e1d8bf7dbc810d6c0b1c08d7e80f88506179ff8cff1bb957ac86c747ed652c6
lang: de
---
# Burp Suite hinter Humanitl

Applies to: Humanitl 0.1

> **Von der KI auf einem virtuellen Bildschirm ausprobiert, noch nicht von einem
> Menschen.** Am 2026-11-18 lief Burp Suite Community Edition 2026.11.2 (das JAR
> von portswigger.net, Prüfsumme gegen die von PortSwigger veröffentlichte
> geprüft) unter Linux in Xvfb, und der Egress-Code von Humanitl war auf seinen
> Listener gerichtet. Wo diese Anleitung „im Probelauf beobachtet" sagt, hat genau
> dieser Build das getan; jede andere Aussage über Burp verweist auf die
> Dokumentation von PortSwigger. Was der Probelauf nicht zeigen konnte, ist als
> „im Probelauf der KI nicht beobachtet" markiert und offen für den Durchgang mit
> dem Eigentümer (HUM-553). Wie der Probelauf ablief, steht unter
> [So lief der Probelauf](#how-the-trial-was-run).

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

Burp braucht drei Einstellungen abweichend von seinen Vorgaben, bevor es ein
sicherer Entscheidungs-Proxy ist, und Burp Community vergisst sie bei jedem Start.
Schritt 1 lädt sie aus einer Datei; die Gründe stehen in den Schritten 1, 6 und 7:

- „Verify upstream TLS" ist von Haus aus aus, also lässt Burp ein Zertifikat des
  Ziels durch, dem niemand vertraut.
- Eine verworfene Anfrage kommt beim Agenten als `200 OK` an, außer „Suppress Burp
  error messages in browser" ist an.
- Die vorgegebene Abfangregel lässt Anfragen nach Bildern, Stylesheets, Skripten
  und Schriften ohne Frage durch.

## Wie die Teile zusammenpassen

```text
Agent ──▶ Humanitl ──▶ Burp ──▶ Ziel
     TLS mit der        TLS mit der      TLS zum Ziel, geprüft nur
     Humanitl-CA        Burp-CA          mit „Verify upstream TLS"
```

Es gibt zwei CAs, und jede Seite vertraut genau einer davon:

| Verbindung | Wer ein Zertifikat zeigt | Ausgestellt von | Wer ihm vertraut |
|---|---|---|---|
| Agent zu Humanitl | Humanitl, für den Namen, den der Agent wollte | der Humanitl-CA | der Agent, in der Sandbox |
| Humanitl zu Burp | Burp, ein Zertifikat, das es je Host erzeugt | der Burp-CA dieser Installation | Humanitl, über `egress.proxy_ca`, nur auf diesem Weg |
| Burp zum Ziel | der echte Server | einer öffentlichen CA | Burp, nur mit „Verify upstream TLS"; von Haus aus aus |

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

Burp prüft das Zertifikat des Ziels nur, wenn du es ihm sagst. Seine
TLS-Einstellungen sagen:
[„To enable upstream TLS verification, click Verify upstream TLS"](https://portswigger.net/burp/documentation/desktop/settings/network/tls).
Im Probelauf beobachtet: Die Option ist in einem frischen Burp aus, und ein Ziel,
dessen selbstsigniertem Zertifikat niemand vertraute, kam mit seiner Antwort beim
Agenten an. Mit der Option lehnte Burp dieses Ziel ab (das Ziel sah den TLS-Alarm
„certificate unknown"), und der Agent bekam sofort `502`. Ohne sie prüft niemand
die Verbindung von Burp zum Ziel, und Humanitl kann das nicht ausgleichen: Es sieht
nur das Zertifikat von Burp, nie das des Ziels. Weitere CAs für diese Prüfung nimmt
Burp aus seiner Liste der
[„additional certificate authorities that Burp trusts when validating upstream TLS connections"](https://portswigger.net/burp/documentation/desktop/settings/network/tls).
Dass ein Ziel mit einem Zertifikat einer öffentlichen CA mit eingeschalteter Option
durchgeht, ist im Probelauf der KI nicht beobachtet (offen für den Durchgang mit
dem Eigentümer, HUM-553).

## 1. Burp mit den Einstellungen für Humanitl starten

Burp Community hat nur ein vorübergehendes Projekt im Speicher; der Startdialog
sagt „Project files are Burp Suite Professional only" (im Probelauf beobachtet).
[Projekteinstellungen](https://portswigger.net/burp/documentation/desktop/settings)
gelten nur im laufenden Projekt („only apply to the current project"), was du an
ihnen änderst, ist also beim nächsten Start weg. Halte sie deshalb in einer Datei. Speichere dies als
`~/burp/humanitl.json`:

```json
{
  "project_options": {
    "ssl": { "negotiation": { "enforce_upstream_trust": true } }
  },
  "proxy": {
    "intercept_client_requests": {
      "do_intercept": true,
      "rules": [
        {
          "boolean_operator": "and",
          "enabled": false,
          "match_condition": "(^gif$|^jpg$|^png$|^css$|^js$|^ico$|^svg$|^eot$|^woff$|^woff2$|^ttf$)",
          "match_relationship": "does_not_match",
          "match_type": "file_extension"
        }
      ]
    },
    "miscellaneous": { "suppress_burp_error_messages_in_browser": true }
  }
}
```

Sie schaltet „Verify upstream TLS" ein, die vorgegebene Abfangregel ab, die
statische Dateien überspringt (Schritt 6), und „Suppress Burp error messages in
browser" ein (Schritt 7). Die Namen der Schlüssel stammen aus einer Datei, die Burp
2026.11.2 mit „Save project settings" selbst gespeichert hat (im Probelauf
beobachtet); die
[Seite zu den Einstellungen](https://portswigger.net/burp/documentation/desktop/settings)
beschreibt Speichern und Laden als JSON.

Starte Burp mit der Datei. Das JAR nimmt sie als `--config-file`, das
[„Load[s] the specified project configuration file"](https://portswigger.net/burp/documentation/desktop/troubleshooting/launch-from-command-line):

```sh
java -jar burpsuite.jar --config-file="$HOME/burp/humanitl.json"
```

Wähle „Temporary project in memory" und „Start Burp". Im Probelauf beobachtet:
Nach einem solchen Start galten alle drei Einstellungen. Das JAR braucht dein
eigenes Java; PortSwigger nennt
[„Java 21"](https://portswigger.net/burp/documentation/desktop/troubleshooting/launch-from-command-line)
als Mindestversion, die nativen Installer bringen ihr eigenes mit. Startest du Burp
aus dem Installer, lade die Datei nach dem Start: Settings, „Manage global
settings", „Project settings", „Load project settings" (diese Menünamen im
Probelauf beobachtet). Ob der installierte Starter `--config-file` nimmt, ist im
Probelauf der KI nicht beobachtet (offen für den Durchgang mit dem Eigentümer,
HUM-553).

Prüfe dann den Listener unter Proxy settings, „Proxy listeners". Von Haus aus legt
Burp einen einzigen Listener auf Port 8080 der Loopback-Schnittstelle an
([„creates a single listener on port 8080 of the loopback interface"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy)),
also dort, wo Humanitl ihn erreicht; im Probelauf beobachtet: `127.0.0.1:8080`,
Zertifikat „Per-host". An alle Schnittstellen gebunden, können ihn andere Rechner
in deinem Netz nutzen.

Lass die Einstellungen „Request handling" des Listeners („Redirect to host",
„Redirect to port", „Force use of TLS", unsichtbares Proxying) aus. Humanitl ist
ein Client, der den Proxy kennt: Es schickt `CONNECT` vor dem TLS, und das ist der
Fall, den Burp ohne
[invisible proxying](https://portswigger.net/burp/documentation/desktop/tools/proxy/invisible)
behandelt.

Ob Burp das Ziel kontaktiert, bevor du entscheidest: Im Probelauf beobachtet hat
Burp eine angehaltene Anfrage vor „Forward" nicht an das Ziel geschickt, mit beiden
Werten von `egress.proxy_resolve`. Von 16 angehaltenen Anfragen, bei denen das
Verbindungsprotokoll des Ziels gelesen wurde, während die Anfrage wartete (6 bis 12
Sekunden), hinterließen 15 vor der Entscheidung keine Verbindung am Ziel. Einmal sah
das Ziel einen TLS-Handschlag ohne Anfrage in dem Moment, in dem eine angehaltene
Anfrage bei Burp ankam; die 10 angehaltenen Anfragen danach wiederholten das nicht.
Nimm es als möglich: Das Ziel kann erfahren, dass jemand verbindet (deine Adresse,
der Name in SNI), bevor du entscheidest. mitmproxy tut das mit seinen Vorgaben
jedes Mal, siehe
[Schritt 1 der mitmproxy-Anleitung](decision-proxy-mitmproxy.md#1-install-and-start-mitmproxy).

## 2. Die CA von Burp exportieren

Humanitl braucht das Zertifikat der Burp-CA im PEM-Format. Burp exportiert es aus
den Listener-Einstellungen, wie es
[Managing CA certificates](https://portswigger.net/burp/documentation/desktop/tools/proxy/manage-certificates)
beschreibt:

1. Im Reiter Proxy die Proxy settings öffnen.
2. Unter „Proxy listeners" auf „Import / export CA certificate" klicken.
3. Unter „Export" „Certificate in DER format" wählen, „Next" klicken, einen
   Dateinamen wie `~/burp/burp-ca.der` eingeben und wieder „Next" klicken. Nicht
   „Private key in DER format" oder „Certificate and private key in PKCS#12
   keystore" wählen. (Die Namen der Wahlen im Probelauf beobachtet.)

Mit OpenSSL nach PEM umwandeln:

```sh
openssl x509 -inform der -in ~/burp/burp-ca.der -out ~/burp/burp-ca.pem
```

Im Probelauf beobachtet: Das Zertifikat heißt „PortSwigger CA". Gib Humanitl nie
eine Datei, die den privaten Schlüssel enthält. Humanitl liest aus der Datei nur
Zertifikate und braucht nichts weiter.

## 3. Humanitl auf Burp richten

Auf der Kommandozeile jeden Schlüssel mit `humanitl config set`. `egress.proxy_ca`
muss ein absoluter Pfad sein, also lass die Shell `$HOME` einsetzen:

```sh
humanitl config set egress.proxy http://127.0.0.1:8080
humanitl config set egress.proxy_ca "$HOME/burp/burp-ca.pem"
humanitl config set egress.proxy_resolve proxy     # die Vorgabe; mit Burp beibehalten
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
leer: Der vorgegebene Listener von Burp verlangt keine Anmeldung. Bleib bei
„Proxy": Mit Burp schützt „Humanitl" dein Netz nicht, siehe
[Wer Namen auflöst](#who-resolves-names).

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
hat dieselbe Prüfung unter „Entscheidungs-Proxy", „Verbindung prüfen".

Im Probelauf beobachtet: Die Prüfung bestand gegen Burp mit `egress.proxy_resolve`
auf `proxy` und auf `humanitl`, mit `proxy` auch bei eingeschaltetem „Verify
upstream TLS". Der Probelauf rief den Code der Prüfung aus dem Proxy-Crate von
Humanitl auf, nicht den Befehl `humanitl` gegen einen laufenden Daemon; Befehl und
Knopf in der App sind im Probelauf der KI nicht beobachtet (offen für den Durchgang
mit dem Eigentümer, HUM-553). Die Prüfung zeigt, dass Humanitl Burp erreicht und
seiner CA vertraut. Sie zeigt nicht, dass Burp das Ziel erreicht: Burp schließt den
Handschlag mit Humanitl ab, ohne das Ziel zu kontaktieren (im Probelauf an
angehaltenen Anfragen beobachtet).

## 6. In Burp abfangen und entscheiden

Unter Proxy, Intercept schaltet der Knopf „Intercept on/off" das Abfangen:
[„If the button shows Intercept on, messages are intercepted"](https://portswigger.net/burp/documentation/desktop/tools/proxy/intercept-messages);
mit „Intercept off" leitet Burp alles von selbst weiter. Im Probelauf beobachtet:
Ein frisches Burp startete mit „Intercept off", auch mit der Datei aus Schritt 1,
schalte es also nach jedem Start ein. Ob das Abfangen beim Start von Burp an ist,
ist eine Nutzereinstellung,
[„Default Proxy interception state"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy).

An einer angehaltenen Anfrage, wie
[Intercepting messages](https://portswigger.net/burp/documentation/desktop/tools/proxy/intercept-messages)
es beschreibt:

- „Forward" schickt sie an das Ziel, und der Agent bekommt dessen Antwort.
- „Drop" verwirft sie, „so that they never reach the target server".

Im Probelauf beobachtet, mit beiden Werten von `egress.proxy_resolve`: Eine
weitergeleitete Anfrage erreichte das Ziel erst nach „Forward", und der Agent bekam
die Antwort des Ziels; eine verworfene Anfrage erreichte das Ziel nie. Der
Probelauf klickte die Knöpfe mit einem Programm auf einem virtuellen Bildschirm;
ein Mensch an einem echten Bildschirm ist im Probelauf der KI nicht beobachtet
(offen für den Durchgang mit dem Eigentümer, HUM-553).

Abfangregeln in den Proxy settings
[„control which messages are stalled"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy)
für dich. Im Probelauf beobachtet: Ein frisches Burp hat eine Regel eingeschaltet,
„File extension Does not match `(^gif$|^jpg$|^png$|^css$|^js$|^ico$|^svg$|^eot$|^woff$|^woff2$|^ttf$)`".
Mit ihr wurde eine Anfrage des Agenten nach `/lib/app.js` nicht angehalten,
obwohl das Abfangen an war: Burp kontaktierte das Ziel binnen 76 Millisekunden (das
Ziel protokollierte die Verbindung; es lehnte sie ab, weil in diesem Lauf „Verify
upstream TLS" an war). Die Datei aus Schritt 1 schaltet die Regel ab; dann stand
dieselbe Anfrage in der Intercept-Liste von Burp, und das Ziel sah 6 Sekunden lang
bis „Drop" keine Verbindung (im Probelauf beobachtet). Startest du Burp
ohne die Datei, schalte die Regel von Hand ab.

Burp weiß nicht, welche Anfragen eine Regel in Humanitl freigegeben hat: Sein
Abfangen gilt für alles, was ankommt, auch für die Durchreiche zum Sprachmodell und
für Regeln `allow`. Soll der Verkehr zum Modellserver nicht auf dich warten, lege
eine Abfangregel an, die seinen Host auslässt. Wie sich die Durchreiche einer
echten Session durch Burp verhält, ist im Probelauf der KI nicht beobachtet (offen
für den Durchgang mit dem Eigentümer, HUM-553).

## 7. Was der Agent sieht

| In Burp | Der Agent bekommt |
|---|---|
| weitergeleitet | die Antwort des Ziels, unverändert |
| verworfen, mit „Suppress Burp error messages in browser" | `502`, sofort, hinter Humanitl geschlossen (`upstream_proxy_closed`) |
| verworfen, mit der Vorgabe von Burp | `200 OK` von Burp mit einer HTML-Seite „Request was dropped by user.", sofort |
| Burp kann den Namen des Ziels nicht auflösen, Vorgabe von Burp | `200 OK` von Burp mit einer HTML-Seite „Unknown host: …", sofort |
| Burp vertraut dem Ziel nicht („Verify upstream TLS" an), mit „Suppress Burp error messages in browser" | `502`, sofort, hinter Humanitl geschlossen (`upstream_proxy_closed`) |
| niemand entscheidet | `502` von Humanitl, nach `egress.proxy_response_timeout_secs` |

Alle Zeilen im Probelauf beobachtet. Nicht beobachtet: ein unbekannter Name mit
eingeschalteter Einstellung und ein nicht vertrautes Ziel mit der Vorgabe von Burp;
nach den beiden Zeilen zu „verworfen" ist wohl dasselbe `502` und `200 OK` zu
erwarten, gezeigt hat der Probelauf das nicht. Die Vorgabe bei „Drop" ist die
gefährliche: Der Agent sieht `200 OK` und kann die Fehlerseite von Burp für die
Antwort des Ziels halten, und Humanitl zeichnet die Anfrage mit Status `200` auf.
Dasselbe geschah, als Burp den Namen des Ziels nicht auflösen konnte („Unknown
host"). Mit
[„Suppress Burp error messages in browser"](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy),
das die Meldungen unterdrückt, „that are usually sent to the browser when errors
occur", schließt Burp stattdessen die Verbindung, und Humanitl antwortet sofort mit
`502`. Die Datei aus Schritt 1 schaltet das ein.

Im Probelauf lautete dieses `502` noch `Blocked by Humanitl.` mit dem Grund
`upstream_connect`, als hätte Humanitl entschieden oder wäre das Ziel nicht
erreichbar. Seit HUM-555 nennt seine erste Zeile statt Humanitl den Proxy
dahinter oder das Ziel dahinter, und die Historie verbucht den Fluss als
gescheitert mit diesem Grund (`FlowSummary.error`), nicht als Ziel, das niemand
erreichte. Das HMAC-Audit-Log hat für einen gescheiterten Fluss noch keinen
Eintrag; es zeigt nur die Weiterleitung.

```text
Closed without an answer by the decision proxy or the target behind it.
reason: upstream_proxy_closed
flow: <flow id>
host: <host>
```

Das folgt aus dem Code und dem Test mit dem Stellvertreter-Proxy; mit Burp wurde
es nicht noch einmal ausprobiert. Ein „Drop" kann Humanitl bei eingeschalteter
Einstellung nicht von einem eigenen Fehler von Burp unterscheiden: Beide schließen
ohne Antwort, `upstream_proxy_closed` sagt also, wo geschlossen wurde, nicht
warum. `upstream_connect` bleibt für ein Burp, das nicht erreichbar ist, `CONNECT`
ablehnt oder `407` sagt.

**Ein „Drop" mit der Fehlerseite von Burp bleibt in den Aufzeichnungen von
Humanitl ein `200`.** Humanitl kann eine Antwort, die Burp selbst schreibt, nicht
von einer unterscheiden, die Burp vom Ziel weiterreicht: Im Tunnel kommen beide
über die TLS-Verbindung von Burp mit dem Zertifikat von Burp, und nichts in der
Antwort kennzeichnet sie auf eine Weise, die alle Proxys teilen. „Request was
dropped by user." oder „Unknown host" zu erkennen hieße, die Seitentexte von Burp
zu raten; die können sich mit jeder Fassung ändern, und ein Ziel kann sie
nachahmen. Humanitl tut das nicht. Mit der Vorgabe von Burp erscheint eine
verworfene Anfrage im Audit und in der Historie als weitergeleitet mit Status
`200`. „Suppress Burp error messages in browser" eingeschaltet lassen.

Die letzte Zeile ist das Verhalten von Humanitl selbst und gilt für jeden Proxy
dahinter; im Probelauf kam sie nach der Frist, mit dem Grund `upstream_timeout` im
Rumpf. `egress.proxy_response_timeout_secs` (Vorgabe 300 Sekunden) ist, wie lange
Humanitl auf den Kopf der Antwort von Burp wartet, weil dort ein Mensch entscheiden
kann. Das Briefing des Agenten sagt ihm dasselbe: warten, dann `502`; ein `403` ist
endgültig.

Humanitl zeichnet eine solche Anfrage mit der Entscheidungsquelle `upstream` auf
(im Audit `decision: forward`, `decided_by: upstream`) und mit dem Status der
Antwort von Burp. Es behauptet keine Freigabe und keine Sperre, die es nicht
gesehen hat.

## Wer Namen auflöst

`egress.proxy_resolve` entscheidet, wer den Namen des Ziels in eine Adresse auflöst.
Mit Burp nimm `proxy`.

| | `proxy` (Vorgabe) | `humanitl` |
|---|---|---|
| Was Humanitl schickt | `CONNECT example.com:443` | `CONNECT 93.184.215.14:443`, den Namen in SNI |
| DNS-Anfrage aus Humanitl | keine | eine, nach den eigenen Regeln von Humanitl, vor deiner Entscheidung in Burp |
| Wohin Burp verbindet | zum Namen, von Burp aufgelöst; die Anfrage erst nach deiner Entscheidung, ein TLS-Handschlag ohne Anfrage wurde aber einmal davor gesehen (Schritt 1) | wieder zum Namen, von Burp aufgelöst; die Adresse im `CONNECT` wird nicht benutzt |
| Private Ziele | Humanitl lehnt private IP-Literale und lokale Namen ab; ein öffentlicher Name, der in dein Netz zeigt, ist Sache von Burp | wie bei `proxy`: Die Prüfung der aufgelösten Adresse durch Humanitl bindet Burp nicht |
| Geht mit Burp | ja, im Probelauf beobachtet | die Anfrage geht durch, im Probelauf beobachtet, aber siehe die Zeile darüber |

Das `CONNECT` mit dem Namen ist der
[übliche Weg](https://portswigger.net/burp/documentation/desktop/tools/proxy/invisible),
wie ein Client mit Burp spricht: „clients send a CONNECT request that identifies
the destination host and then perform TLS negotiation". Im Probelauf beobachtet,
mit `humanitl`: Burp nahm `CONNECT 127.0.0.1:18443` an und gab Humanitl ein
Zertifikat für den Namen aus SNI, das Humanitl annahm. Für die Anfrage im Tunnel
löste Burp aber den Namen aus `Host` selbst auf und überging die Adresse: Mit einem
Namen, den Burp nicht auflösen konnte, scheiterte die Anfrage mit „Unknown host",
obwohl das `CONNECT` eine erreichbare Adresse nannte.

Mit Burp kostet `humanitl` also die DNS-Anfrage vor deiner Entscheidung und bringt
nichts: Humanitl prüft eine Adresse, die Burp nicht benutzt, und die eigene
Auflösung von Burp entscheidet, wohin die Anfrage geht. Burp verband ohne Einwand
zu einem Ziel auf `127.0.0.1` (im Probelauf beobachtet), ein öffentlicher Name, der
auf deinen Router oder einen Dienst auf deinem Rechner zeigt, erreicht ihn also,
wenn du die Anfrage weiterleitest. Sieh dir den Host an, bevor du „Forward"
klickst.

## Grenzen

Sie stehen in [SECURITY.md](../SECURITY.md#the-proxy-behind-decides-holdask_mode--upstream-hum-512)
und im [Bedrohungsmodell](../THREAT-MODEL.md); kurz:

- Burp sieht den entschlüsselten Verkehr, auch Geheimnisse, die Humanitl nicht
  gesperrt hat, und die Durchreiche zum Sprachmodell. Sein HTTP-Verlauf enthält
  sie, solange Burp läuft; wer den Speicher von Burp lesen kann, kann sie lesen.
- Burp prüft das Zertifikat des Ziels nur mit „Verify upstream TLS" (Schritt 1).
  Ohne die Option ist die Verbindung von Burp zum Ziel nicht geprüft, und Humanitl
  kann das nicht ausgleichen.
- `egress.proxy_resolve = "humanitl"` hält mit Burp private Ziele nicht fern, weil
  Burp den Namen erneut auflöst (siehe [Wer Namen auflöst](#who-resolves-names)).
- Die Vorgabe von Burp bei „Drop" und bei eigenen Fehlern ist `200 OK` mit einer
  Fehlerseite (Schritt 7). Humanitl kann diese Seite nicht von der Antwort des
  Ziels unterscheiden: Ein „Drop" mit Fehlerseite erscheint im Audit und in der
  Historie als `200`.
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

## So lief der Probelauf

Am 2026-11-18 ließ die KI Burp Suite Community Edition 2026.11.2 aus dem JAR
laufen (SHA-256
`ddd11e506bbbee7491c120901722a8bbd79d64d1298afceee1d334d27981516b`, wie von
PortSwigger veröffentlicht), mit OpenJDK 25 aus Debian, das Burp als „not been
fully tested" meldet, auf einem virtuellen X-Bildschirm (Xvfb) und mit eigenem
Home-Verzeichnis. Sie nahm die Lizenz an, wählte Community Edition und ein
vorübergehendes Projekt, exportierte die CA wie oben und klickte Intercept, Forward
und Drop mit `java.awt.Robot`, den Bildschirm las sie aus Screenshots.

Statt eines laufenden Daemons nutzte der Probelauf den Proxy von Humanitl aus der
Testunterstützung des Proxy-Crates: denselben Egress-Code mit `hold.ask_mode =
"upstream"`, `egress.proxy = http://127.0.0.1:8080`, der exportierten CA als
`egress.proxy_ca` und beiden Werten von `egress.proxy_resolve`. Der Agent war ein
Client, der `CONNECT` und TLS mit diesem Proxy sprach. Das Ziel war ein kleiner
TLS-Server auf `127.0.0.1` mit einem selbstsignierten Zertifikat für
`upstream.test`, dem nichts vertraute; er schrieb jede Verbindung mit Zeitstempel
mit, und der Probelauf las dieses Protokoll, während eine Anfrage angehalten war,
und wieder nach der Entscheidung; so zählte er Kontakte vor der Entscheidung. Die
Seite des Agenten protokollierte Status, Rumpf und Zeit jeder Antwort. Diese
Protokolle und die Screenshots bleiben bei den Notizen zum Probelauf von HUM-524
außerhalb des Repositorys. Burp fand ihn über seine „Hostname
resolution overrides"
([DNS settings](https://portswigger.net/burp/documentation/desktop/settings/network/dns)).
Die Prüfung aus Schritt 5 lief gegen das echte `example.com`.

Im Probelauf der KI nicht beobachtet, offen für den Durchgang mit dem Eigentümer
(HUM-553):

- `humanitl egress check` und „Verbindung prüfen" in der App gegen Burp, mit einem
  echten Daemon nach `humanitl daemon restart`.
- Eine echte Agenten-Session durch Humanitl und Burp, samt Durchreiche zum
  Sprachmodell.
- Ein Ziel mit einem Zertifikat einer öffentlichen CA, mit eingeschaltetem „Verify
  upstream TLS".
- Burp aus dem nativen Installer, und ob dessen Starter `--config-file` nimmt.
- Intercept, Forward und Drop, von einem Menschen an einem echten Bildschirm
  geklickt, und der Abfangzustand beim Start des eigenen Burp des Eigentümers.
- Ob der einzelne frühe TLS-Handschlag zum Ziel (Schritt 1) wieder auftaucht.

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
