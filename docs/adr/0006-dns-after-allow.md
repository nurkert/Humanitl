# ADR-0006 · DNS-Auflösung erst nach der Freigabe
Status: Accepted
Datum: 2026-09-02

## Kontext

Ein Proxy, der einen Request annimmt, löst normalerweise sofort den Zielhost auf
— schon deshalb, weil die meisten HTTP-Clientbibliotheken Auflösung und Verbindung
in einem Schritt erledigen. In Humanitl liegt zwischen Annahme und Verbindung
aber ein Mensch, und diese Lücke ist ein Kanal.

Eine DNS-Anfrage verlässt den Rechner. Sie geht an einen Resolver, der nicht
unter unserer Kontrolle steht, und der Angreifer kontrolliert im Zweifel die
autoritative Zone. Ein Label darf 63 Bytes tragen, ein Name 253. Ein Agent, der
`aGVsbG8td29ybGQtc2VjcmV0.attacker.com` auflösen lässt, hat Daten exfiltriert —
auch dann, wenn der Mensch den Request danach blockt und nie eine TCP-Verbindung
zustande kommt. Der Block wäre in diesem Fall Theater.

Das zweite Problem ist DNS-Rebinding. Löst der Proxy zur Prüfzeit auf und
verbindet später erneut, kann derselbe Name beim zweiten Mal auf eine andere
Adresse zeigen — etwa auf den Router, auf `169.254.169.254` (Cloud-Metadaten)
oder auf den LLM-Server im LAN. Der Mensch hätte dann `example.com` freigegeben
und der Agent spräche mit dem Router.

## Entscheidung

**Es wird nichts aufgelöst, bevor der Request freigegeben ist.** Die Hold-Queue
und die Regelauswertung arbeiten ausschließlich auf dem Hostnamen als
normalisiertem String. Erst nach `allow` wird genau einmal aufgelöst, und zwar
über den Port `Resolver` (Adapter: `hickory`) in der Crate `humanitl-proxy` —
nie über den System-Resolver eines HTTP-Connectors, nie über `GaiResolver`, nie
über das DNS eines `HttpConnector`. Die so gewonnene IP-Adresse wird an
`Egress::connect(authority, Some(ip))` übergeben und damit für diese Verbindung
gepinnt (ADR-0017).

**Private Zieladressen werden nach der Auflösung verweigert.** Löst ein Name auf
eine Adresse in `10/8`, `172.16/12`, `192.168/16` (RFC 1918), `127/8`,
`169.254/16`, `100.64/10` (CGNAT), `fc00::/7` oder `::1` auf, wird die
Verbindung abgelehnt: `BlockReason::PrivateAddress`, Diagnostic `PROXY_005` mit
einem Regelvorschlag. Ausnahme: Die matchende Regel trägt `allow_private: true`.
Die LLM-Passthrough-Regel setzt dieses Flag automatisch, damit `localhost` oder
`192.168.x.y` als LLM-Host funktionieren.

**Die Domain-Vorschau im UI holt nie automatisch etwas aus dem Netz.** Die
Karte im Domain-Panel kommt aus einem gebündelten Katalog
(`catalog/domains.yaml`). Ein Live-Abruf (Favicon, `og:title`) geschieht nur auf
ausdrücklichen Klick, host-seitig, und nur für die eTLD+1.

## Begründung

Der Hostname ist die einzige Information, die ein Angreifer ohne Freigabe
kontrollieren und nach außen bringen kann. Wer ihn vor der Entscheidung auflöst,
hat einen Exfiltrationskanal mit 63 Bytes pro Label offengelassen — leise, ohne
Zutun des Nutzers, und in einem Protokoll, das die meisten Menschen nicht als
Datenkanal wahrnehmen. Die Auflösung nach hinten zu verschieben kostet fast
nichts und schließt den Kanal vollständig.

Die Prüfung auf private Adressen muss *nach* der Auflösung stattfinden, nicht
vorher am Namen, denn genau das ist der Rebinding-Trick: Ein völlig öffentlich
aussehender Name zeigt auf `169.254.169.254`. Und weil die geprüfte IP dann für
die Verbindung gepinnt wird, kann zwischen Prüfung und Verbindung kein zweiter
Auflösungsversuch dazwischenkommen.

`allow_private` als Regel-Flag statt als globale Einstellung hält die Ausnahme
sichtbar und lokal. Der LLM-Server ist der einzige Fall, in dem eine private
Adresse normal ist, und er hat ohnehin eine eigene, eng geschnittene Regel
(Host + Port + Pfadpräfix + Methode). Ein Nutzer, der eine weitere private
Adresse braucht, sieht seine Ausnahme als eine Zeile in `rules.yaml`.

Die automatische Domain-Vorschau wäre der gleiche Fehler in Grün: Das UI würde
eine Verbindung zu genau der Domain aufbauen, über die der Mensch gerade
entscheidet, und damit dem Angreifer bestätigen, dass jemand hinschaut — mit der
IP-Adresse des Hosts, nicht der der Sandbox.

## Verworfene Alternativen

- **Auflösen und die IP im UI anzeigen.** Wäre für den Nutzer informativ („diese
  Domain zeigt auf einen Server in …"), öffnet aber genau den Kanal, den dieser
  ADR schließt. Der Informationsgewinn wiegt das nicht auf; der Katalog liefert
  die nützliche Hälfte davon offline.
- **Auflösen über DNS-over-HTTPS zu einem festen Resolver.** Verschlüsselt den
  Kanal, schließt ihn aber nicht: Der Angreifer kontrolliert die autoritative
  Zone und sieht die Anfrage dort, egal wie sie zu ihm gekommen ist.
- **Nur die eTLD+1 auflösen.** Verkleinert den Kanal, statt ihn zu schließen.
  Eine Halbmaßnahme, die schwer zu erklären ist.
- **Private Adressen am Namen prüfen (`localhost`, `*.local`, IP-Literale).**
  Fängt den naiven Fall und verfehlt den interessanten: Ein öffentlicher Name mit
  privater Antwort. Bleibt als zusätzliche, nicht als alleinige Prüfung sinnvoll.
- **Private Adressen global erlauben, mit Warnung.** Verworfen: Cloud-Metadaten
  unter `169.254.169.254` sind ein Standardziel von Exfiltrations-Payloads. Ein
  Standard-Erlauben mit Warnbanner ist kein fail-closed-Verhalten.
- **DNS-Auflösung dem Upstream-Proxy überlassen.** Genau das passiert im späteren
  `Socks5h`-Modus (ADR-0017), wo lokale Auflösung sogar verboten ist. Für den
  MVP ohne Upstream-Proxy ist die lokale Auflösung nach Freigabe der einfachere
  Weg.

## Konsequenzen

- Die Hold-Queue und die Regel-Engine arbeiten nur auf Strings. Beide bleiben
  damit IO-frei und tabellengetrieben testbar (ADR-0015).
- Ein Auflösungsfehler tritt erst *nach* der Freigabe auf und ist damit ein
  eigener Zustand: `FlowState::Failed{ UpstreamError::Dns }`, `502` an den
  Client (ADR-0004). Der Nutzer sieht „du hast freigegeben, aber der Name löst
  nicht auf" statt eines stillen Blocks.
- Der Test für diese Entscheidung misst, nicht behauptet: Escape-Test 3 prüft,
  dass der Host-Resolver **null** Lookups zeigt, solange der Request in der
  Warteschlange steht (HUM-024, Resolver-Statistik).
- `Egress::connect` bekommt die IP als Parameter. Damit ist der Punkt, an dem
  Auflösung und Verbindung auseinanderfallen, im Typsystem sichtbar.
- Homographe und Punycode werden bei der Normalisierung behandelt, nicht bei der
  Auflösung: Der angezeigte und der geprüfte Host sind derselbe A-Label-String
  (ADR-0007).
- Die Katalog-Karte muss ohne Netz genug hergeben, damit der Nutzer entscheiden
  kann. Daher rund 200 gebündelte Dev-Services mit Icon, Kategorie und
  Beschreibung sowie ein Tranco-Rang aus einer gebündelten Liste (HUM-031).

## Betroffene Issues

`HUM-024` (DNS erst nach Allow, mit Resolver-Statistik im Test), `HUM-015`
(Proxy-Kern, Verbindungsaufbau über `Egress`), `HUM-031` (Domain-Panel ohne
automatischen Fetch), `HUM-039` (LLM-Passthrough setzt `allow_private`),
`HUM-022` (Regel-Flag `allow_private`).
