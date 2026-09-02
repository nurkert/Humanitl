# ADR-0016 · Browser für den Agenten über CDP, Zuschauen und Eingreifen im UI
Status: Accepted
Datum: 2026-09-02

## Kontext

Agenten brauchen zunehmend einen Browser: Dokumentation lesen, die
JavaScript nachlädt, Formulare ausfüllen, Anmeldeflüsse durchlaufen, eine
Weboberfläche bedienen. Ein Agent ohne Browser fällt auf `curl` zurück und
scheitert an jeder Seite, die clientseitig rendert.

Ein Browser in der Sandbox wirft drei Fragen auf. Erstens: Läuft er überhaupt?
Chromium bringt eine eigene Sandbox mit, die User-Namespaces belegt — dieselbe
Ressource, die bwrap bereits verwendet. Zweitens: Bleibt die Garantie bestehen?
Ein Browser öffnet Verbindungen, spricht QUIC, hat einen eigenen DNS-Cache und
benutzt Unix-Socketpairs für seine interne Kommunikation. Drittens: Was sieht
der Mensch? Ein Agent, der unbeobachtet in einer Weboberfläche klickt, ist genau
das Kontrollverlustszenario, gegen das Humanitl gebaut ist.

Diese Entscheidung ist **Post-MVP (M7)**. Sie wird jetzt getroffen, weil zwei
Vorarbeiten im MVP anfallen und sonst falsch dimensioniert würden: die
Bridge-Verwaltung im Shim und die Konfigurierbarkeit der erlaubten
seccomp-Socket-Familien pro Profil.

## Entscheidung

**Sandbox-Profil `browser`.** Es benutzt das Chromium des Hosts über den
Nur-Lese-Mount von `/usr` und bringt `nodriver` im Python-Paket mit.
Start-Flags:

```
--proxy-server=http://127.0.0.1:3128
--disable-quic
--remote-debugging-port=9222
--headless=new
--no-sandbox
--ignore-certificate-errors-spki-list=<SPKI-Hash der Humanitl-CA>
```

Ein gebündeltes `humanitl_browser.py` liefert die fertige nodriver-Konfiguration.
Alle Seitenaufrufe laufen durch den Proxy wie jeder andere Request und
unterliegen denselben Regeln und derselben Aufzeichnung.

**Zuschauen.** Der Shim startet vor dem seccomp-Filter eine zweite,
host-initiierte Bridge: ein Unix-Socket in der Sandbox, der auf
`127.0.0.1:9222` weiterleitet (Richtung `out`). Der Daemon spricht darüber CDP
(`chromiumoxide`); `Page.startScreencast` liefert Einzelbilder, und ein
gRPC-Stream `Browser` bringt sie ins UI.

**Eingreifen.** Maus- und Tastatureingaben gehen als CDP-Input zurück. Der
Übernahme-Modus ist im UI sichtbar (Rahmen). Agent und Mensch können gleichzeitig
verbunden sein, weil Chromium mehrere CDP-Clients erlaubt.

**Sicherheitsfolgen, ausgesprochen.** Der Shim unterstützt eine **Liste** von
Bridges aus dem Profil, jede mit Richtung `in` oder `out`. Der seccomp-Filter
erlaubt `AF_INET`/`AF_INET6` immer (im Namespace existiert nur `lo`); das Profil
`browser` erlaubt **zusätzlich `AF_UNIX`** (und `SOCK_DGRAM`), weil Chromium eigene
Unix-Sockets für seine Prozess-IPC anlegt; `socketpair()` bleibt davon unabhängig in allen
Profilen unbeschränkt (CONVENTIONS 4.11). Die Garantie trägt in diesem
Profil das Netzwerk-Namespace plus die Mount-Allowlist; seccomp bleibt doppelter
Boden, ist aber nicht mehr die engste Stelle. Chromium läuft ohne eigene Sandbox
(`--no-sandbox`), weil bwrap die User-Namespaces belegt — bwrap **ist** die
äußere Sandbox.

**Im MVP nur die Vorarbeiten:** die Bridge-Liste im Shim (HUM-012) und die
seccomp-Familien pro Profil (HUM-010).

## Begründung

Chromium des Hosts statt eines gebündelten Browsers, weil ein mitgeliefertes
Chromium das Paket um mehr als hundert Megabyte aufbläht und eine eigene
Sicherheitsaktualisierungspflicht erzeugt. Der Nur-Lese-Mount von `/usr` liegt
ohnehin vor.

`--disable-quic` ist nicht optional. QUIC läuft über UDP und würde den
HTTP-Proxy umgehen; im leeren Netzwerk-Namespace scheitert es zwar ohnehin, aber
ein stiller Fehlschlag mit langen Timeouts ist schlechter als eine klare
Abschaltung.

Der SPKI-Hash der Humanitl-CA statt `--ignore-certificate-errors` insgesamt: Der
Browser vertraut genau einem Schlüssel, nicht jedem beliebigen Zertifikat. Der
Unterschied ist die Frage, ob eine MITM-Attacke innerhalb der Sandbox möglich
wäre.

CDP statt eines eingebetteten WebViews im UI: Der Browser läuft dort, wo er
hingehört — in der Sandbox — und das UI sieht nur Einzelbilder. Damit bleibt
ADR-0009 („kein WebView auf Linux") intakt, und der Rendering-Prozess der zu
prüfenden Seite läuft nie im selben Prozess wie die Oberfläche.

Dass Agent und Mensch gleichzeitig verbunden sein können, ist die eigentliche
Produktidee dahinter: Zuschauen ist der Normalfall, Eingreifen die Ausnahme, und
der Übergang braucht keinen Moduswechsel — nur einen sichtbaren Rahmen.

Der Preis wird benannt statt versteckt: Im Profil `browser` ist der
seccomp-Filter schwächer als im Standardprofil. Deshalb ist es ein **eigenes
Profil**. Wer keinen Browser braucht, bekommt die engere Einstellung; wer einen
braucht, sieht die Ausnahme im Profil und im Isolation-Panel.

## Verworfene Alternativen

- **Browser auf dem Host, gesteuert aus dem Daemon.** Umgeht die Sandbox
  vollständig: Der Browser hätte das Netzwerk, das Dateisystem und die Cookies
  des Nutzers. Der Angriffsweg „Agent besucht Seite, Seite exfiltriert über den
  Host-Browser" wäre offen.
- **Playwright oder Selenium statt CDP direkt.** Bringt eine große
  Node- oder Python-Abhängigkeit und eine eigene Browser-Verwaltung mit, die
  Browser herunterlädt — genau das, was in einer netzwerklosen Sandbox nicht
  funktioniert. `chromiumoxide` spricht CDP direkt.
- **Firefox über das Remote-Protokoll.** Marionette und das Remote-Agent-Protokoll
  sind weniger vollständig für Screencast und Input als CDP.
- **Ein eingebetteter WebView im UI.** Widerspricht ADR-0009 und würde die
  besuchte Seite im Prozess der Oberfläche rendern.
- **Screencast über Video-Encoding statt Einzelbildern.** Bessere Bildrate, aber
  eine Encoder-Abhängigkeit und mehr Latenz beim Eingreifen. JPEG-Einzelbilder,
  auf zehn Bilder pro Sekunde gedrosselt, reichen zum Zuschauen.
- **`AF_UNIX` global erlauben, um kein zweites Profil zu brauchen.**
  Ausdrücklich verworfen: Das hätte die Garantie 3 für alle Nutzer geschwächt,
  auch für die ohne Browser. Die Ausnahme bleibt im Profil, wo sie sichtbar ist.
- **Chromium mit eigener Sandbox in bwrap.** Technisch nicht möglich, weil beide
  dieselben User-Namespaces beanspruchen. `--no-sandbox` ist hier kein
  Nachlassen, sondern die Feststellung, wer die äußere Sandbox ist.

## Konsequenzen

- Der Shim (`humanitl-shim`) verwaltet eine **Liste** von Bridges mit Richtung,
  nicht eine einzelne. Diese Verallgemeinerung fällt bereits im MVP an
  (HUM-012), obwohl der zweite Eintrag erst in M7 benutzt wird.
- Das Sandbox-Profil-Format hat `[network].bridges` als Liste und
  `[seccomp].allow_families` und `[seccomp].allow_types` als profilabhängige
  Werte (HUM-010).
- Das Isolation-Panel muss im Profil `browser` eine zusätzliche, ehrliche Zeile
  zeigen: Der seccomp-Filter ist hier weiter gefasst. Kein grüner Haken für
  etwas, das nicht zutrifft.
- `chromiumoxide`, ein `Browser`-RPC und ein Browser-Tab im UI kommen in M7 dazu;
  keiner dieser Teile berührt Kern oder Anwendung (ADR-0015).
- Bekannte Fallstricke für M7, damit sie nicht neu entdeckt werden müssen:
  Screencast-Bilder drosseln (maximal 10 fps, JPEG-Qualität 60);
  Tastaturbelegungen in `dispatchKeyEvent` sind fehleranfällig;
  nodriver-Versionen pinnen.
- Die Domain-Vorschau im Domain-Panel kann später denselben Browser benutzen
  (HUM-085), statt eines eigenen Abrufwegs — und behält damit die Regel aus
  ADR-0006, dass nichts automatisch geholt wird.

## Betroffene Issues

Im MVP: `HUM-012` (Bridge-Liste im Shim), `HUM-010` (seccomp-Familien pro
Profil). In M7: `HUM-080` (Sandbox-Profil `browser`), `HUM-081` (Reverse-Bridge
und `cdp.sock`), `HUM-082` (CDP-Client im Daemon), `HUM-083` (gRPC
`Browser`-Bidi-Stream), `HUM-084` (Browser-Tab im UI), `HUM-085`
(Screenshot-Vorschau für das Domain-Panel).
