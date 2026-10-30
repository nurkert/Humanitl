---
source: adr/0016-browser-cdp.md
source-sha256: f026fce2f81fd9ec8484b7f49c81682e3789381388b566d098ef281945d2bb4a
lang: de
---
# ADR-0016 · Browser für den Agenten über CDP, Zusehen und Eingreifen in der Oberfläche

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Agenten brauchen zunehmend einen Browser: Dokumentation lesen, die Inhalte mit
JavaScript nachlädt, Formulare ausfüllen, Anmeldeabläufe durchgehen, eine
Weboberfläche bedienen. Ein Agent ohne Browser fällt auf `curl` zurück und scheitert an
jeder Seite, die auf dem Client gerendert wird.

Ein Browser in der Sandbox wirft drei Fragen auf. Erstens: Läuft er überhaupt? Chromium
bringt seine eigene Sandbox mit, die User-Namespaces belegt, dieselbe Ressource, die
bwrap schon nutzt. Zweitens: Hält die Garantie noch? Ein Browser öffnet Verbindungen,
spricht QUIC, hat seinen eigenen DNS-Cache und nutzt Unix-Socketpaare für seine interne
Kommunikation. Drittens: Was sieht der Mensch? Ein Agent, der unbeobachtet in einer
Weboberfläche herumklickt, ist genau das Szenario des Kontrollverlusts, gegen das
Humanitl gebaut ist.

Diese Entscheidung ist **nach dem MVP (M7)**. Sie fällt jetzt, weil im MVP zwei
Vorarbeiten entstehen, die sonst falsch bemessen würden: die Verwaltung der Bridges im
Shim und die Konfigurierbarkeit der erlaubten Socket-Familien von seccomp je Profil.

## Entscheidung

**Sandbox-Profil `browser`.** Es nutzt das Chromium des Hosts über den schreibgeschützten
Mount von `/usr` und bringt `nodriver` im Python-Paket mit. Startflags:

```
--proxy-server=http://127.0.0.1:3128
--disable-quic
--remote-debugging-port=9222
--headless=new
--no-sandbox
--ignore-certificate-errors-spki-list=<SPKI hash of the Humanitl CA>
```

Ein mitgeliefertes `humanitl_browser.py` liefert die fertige Konfiguration für nodriver.
Alle Seitenaufrufe gehen wie jede andere Anfrage durch den Proxy und unterliegen
denselben Regeln und derselben Aufzeichnung.

**Zusehen.** Vor dem seccomp-Filter startet der Shim eine zweite, vom Host ausgehende
Bridge: einen Unix-Socket in der Sandbox, der an `127.0.0.1:9222` weiterleitet
(Richtung `out`). Darüber spricht der Daemon CDP (`chromiumoxide`);
`Page.startScreencast` liefert Einzelbilder, und ein gRPC-Stream `Browser` bringt sie in
die Oberfläche.

**Eingreifen.** Maus- und Tastatureingaben gehen als CDP-Eingabe zurück. Der Modus der
Übernahme ist in der Oberfläche sichtbar (Rahmen). Agent und Mensch können gleichzeitig
verbunden sein, weil Chromium mehrere CDP-Clients zulässt.

**Folgen für die Sicherheit, ausgesprochen.** Der Shim unterstützt eine **Liste** von
Bridges aus dem Profil, jede mit der Richtung `in` oder `out`. Der seccomp-Filter erlaubt
immer `AF_INET`/`AF_INET6` (im Namespace gibt es nur `lo`); das Profil `browser`
**erlaubt zusätzlich `AF_UNIX`** (und `SOCK_DGRAM`), weil Chromium für die IPC seiner
Prozesse eigene Unix-Sockets anlegt; `socketpair()` bleibt davon unabhängig in allen
Profilen unbeschränkt (CONVENTIONS 4.11). In diesem Profil tragen der
Netzwerk-Namespace und die Allowlist für Mounts die Garantie; seccomp bleibt eine zweite
Grundsperre, ist aber nicht mehr die engste Stelle. Chromium läuft ohne eigene Sandbox
(`--no-sandbox`), weil bwrap die User-Namespaces belegt: bwrap **ist** die äußere
Sandbox.

**Im MVP nur die Vorarbeit:** die Liste der Bridges im Shim (HUM-012) und die
seccomp-Familien je Profil (HUM-010).

## Begründung

Das Chromium des Hosts statt eines mitgelieferten Browsers, weil ein ausgeliefertes
Chromium das Paket um mehr als hundert Megabyte aufbläht und eine eigene Pflicht schafft,
Sicherheitsupdates auszuliefern. Der schreibgeschützte Mount von `/usr` ist ohnehin da.

`--disable-quic` ist nicht optional. QUIC läuft über UDP und würde den HTTP-Proxy
umgehen; im leeren Netzwerk-Namespace scheitert es ohnehin, aber ein stilles Scheitern
mit langen Zeitüberschreitungen ist schlimmer als ein klares Abschalten.

Der SPKI-Hash der Humanitl-CA statt `--ignore-certificate-errors` insgesamt: Der Browser
vertraut genau einem Schlüssel, nicht irgendeinem Zertifikat. Der Unterschied ist die
Frage, ob ein MITM-Angriff innerhalb der Sandbox möglich wäre.

CDP statt einer eingebetteten WebView in der Oberfläche: Der Browser läuft dort, wo er
hingehört, in der Sandbox, und die Oberfläche sieht nur Einzelbilder. Damit bleibt
ADR-0009 („keine WebView unter Linux“) intakt, und der Render-Prozess der begutachteten
Seite läuft nie im selben Prozess wie die Oberfläche.

Dass Agent und Mensch gleichzeitig verbunden sein können, ist die eigentliche
Produktidee dahinter: Zusehen ist der Normalfall, Eingreifen die Ausnahme, und der
Übergang braucht keinen Moduswechsel, nur einen sichtbaren Rahmen.

Der Preis wird genannt statt versteckt: Im Profil `browser` ist der seccomp-Filter
schwächer als im Vorgabeprofil. Deshalb ist es ein **eigenes Profil**. Wer keinen
Browser braucht, bekommt die engere Einstellung; wer einen braucht, sieht die Ausnahme
im Profil und im Bereich „Isolation“.

## Verworfene Alternativen

- **Browser auf dem Host, gesteuert vom Daemon.** Umgeht die Sandbox vollständig: Der
  Browser hätte das Netz, das Dateisystem und die Cookies des Nutzers. Der Angriffsweg
  „Agent besucht Seite, Seite leitet über den Browser des Hosts aus“ wäre offen.
- **Playwright oder Selenium statt CDP direkt.** Bringt eine große Abhängigkeit von Node
  oder Python und eine eigene Browserverwaltung mit, die Browser herunterlädt, genau
  das, was in einer Sandbox ohne Netz nicht funktioniert. `chromiumoxide` spricht CDP
  direkt.
- **Firefox über das Remote-Protokoll.** Marionette und das Protokoll des Remote Agent
  sind für Screencast und Eingabe weniger vollständig als CDP.
- **Eine eingebettete WebView in der Oberfläche.** Widerspricht ADR-0009 und würde die
  besuchte Seite im Prozess der Oberfläche rendern.
- **Screencast über Videokodierung statt Einzelbilder.** Bessere Bildrate, aber eine
  Abhängigkeit von einem Encoder und mehr Latenz beim Eingreifen. JPEG-Einzelbilder,
  gedrosselt auf zehn Bilder pro Sekunde, reichen zum Zusehen.
- **`AF_UNIX` global erlauben, um kein zweites Profil zu brauchen.** Ausdrücklich
  verworfen: Das hätte Garantie 3 für alle Nutzer geschwächt, auch für die ohne Browser.
  Die Ausnahme bleibt im Profil, wo sie sichtbar ist.
- **Chromium mit eigener Sandbox innerhalb von bwrap.** Technisch nicht möglich, weil
  beide dieselben User-Namespaces beanspruchen. `--no-sandbox` ist hier keine Lockerung,
  sondern die Aussage darüber, wer die äußere Sandbox ist.

## Konsequenzen

- Der Shim (`humanitl-shim`) verwaltet eine **Liste** von Bridges mit Richtung, nicht
  eine einzelne. Diese Verallgemeinerung entsteht schon im MVP (HUM-012), obwohl der
  zweite Eintrag erst in M7 genutzt wird.
- Das Format der Sandbox-Profile hat `[network].bridges` als Liste und
  `[seccomp].allow_families` und `[seccomp].allow_types` als vom Profil abhängige Werte
  (HUM-010).
- Im Profil `browser` muss der Bereich „Isolation“ eine zusätzliche, ehrliche Zeile
  zeigen: Der seccomp-Filter ist hier weiter. Kein grüner Haken für etwas, das nicht
  zutrifft.
- `chromiumoxide`, ein `Browser`-RPC und ein Browser-Reiter in der Oberfläche kommen in
  M7 dazu; keiner dieser Teile berührt Kern oder Anwendung (ADR-0015).
- Bekannte Fallstricke für M7, damit sie nicht neu entdeckt werden müssen:
  Screencast-Bilder drosseln (höchstens 10 fps, JPEG-Qualität 60); Tastaturbelegungen in
  `dispatchKeyEvent` sind fehleranfällig; nodriver-Versionen festnageln.
- Die Domain-Vorschau im Domain-Bereich kann später denselben Browser nutzen (HUM-085)
  statt eines eigenen Abrufwegs und hält damit die Regel aus ADR-0006 ein, dass nichts
  automatisch abgerufen wird.

## Betroffene Issues

Im MVP: `HUM-012` (Liste der Bridges im Shim), `HUM-010` (seccomp-Familien je Profil).
In M7: `HUM-080` (Sandbox-Profil `browser`), `HUM-081` (umgekehrte Bridge und
`cdp.sock`), `HUM-082` (CDP-Client im Daemon), `HUM-083` (bidirektionaler gRPC-Stream
`Browser`), `HUM-084` (Browser-Reiter in der Oberfläche), `HUM-085`
(Bildschirmfoto-Vorschau für den Domain-Bereich).
