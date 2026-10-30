---
source: adr/0006-dns-after-allow.md
source-sha256: 397ed101872a8a0db6944ee9101e266a540718fc7f72173ff98c403477a0bc75
lang: de
---
# ADR-0006 · DNS-Auflösung erst nach dem Erlauben

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Ein Proxy, der eine Anfrage annimmt, löst den Zielhost normalerweise sofort auf,
schon weil die meisten HTTP-Client-Bibliotheken Auflösung und Verbindungsaufbau
in einem Schritt erledigen. In Humanitl sitzt zwischen Annahme und Verbindung
aber ein Mensch, und diese Lücke ist ein Kanal.

Eine DNS-Anfrage verlässt den Rechner. Sie geht an einen Resolver, der nicht unter
unserer Kontrolle steht, und die autoritative Zone kann durchaus der Angreifer
kontrollieren. Ein Label darf 63 Bytes tragen, ein Name 253. Ein Agent, der
`aGVsbG8td29ybGQtc2VjcmV0.attacker.com` auflösen lässt, hat Daten ausgeleitet,
auch wenn der Mensch die Anfrage danach blockiert und nie eine TCP-Verbindung
zustande kommt. In diesem Fall wäre das Blockieren Theater.

Das zweite Problem ist DNS-Rebinding. Löst der Proxy zum Zeitpunkt der Prüfung auf
und verbindet sich später erneut, kann derselbe Name beim zweiten Mal auf eine
andere Adresse zeigen, etwa auf den Router, auf `169.254.169.254`
(Cloud-Metadaten) oder auf den LLM-Server im LAN. Der Mensch hätte dann
`example.com` erlaubt, und der Agent spräche mit dem Router.

## Entscheidung

**Nichts wird aufgelöst, bevor die Anfrage erlaubt ist.** Die Warteschlange der
angehaltenen Anfragen und die Regelauswertung arbeiten ausschließlich mit dem
Hostnamen als normalisiertem String. Erst nach `allow` wird er aufgelöst, genau
einmal, über den Port `Resolver` (Adapter: `hickory`) im Crate `humanitl-proxy`,
nie über den System-Resolver eines HTTP-Connectors, nie über `GaiResolver`, nie
über das DNS eines `HttpConnector`. Die so gewonnene IP-Adresse wird an
`Egress::connect(authority, Some(ip))` übergeben und damit für diese Verbindung
festgenagelt (ADR-0017).

**Private Zieladressen werden nach der Auflösung abgewiesen.** Löst ein Name zu
einer Adresse in `10/8`, `172.16/12`, `192.168/16` (RFC 1918), `127/8`,
`169.254/16`, `100.64/10` (CGNAT), `fc00::/7` oder `::1` auf, wird die Verbindung
abgewiesen: `FlowState::Failed { UpstreamError::PrivateAddress(ip) }` und `502`
an den Client, dazu die Diagnostic `PROXY_008` mit einem Regelvorschlag.
Ausnahme: Die passende Regel trägt `allow_private: true`. Die Regel für die
LLM-Durchleitung setzt dieses Flag automatisch, damit `localhost` oder
`192.168.x.y` als LLM-Host funktionieren.

Der Regelvorschlag ist `action: ask` mit `allow_private: true`, zugeschnitten auf
Host, Port, Schema, Methode, Protokoll-Upgrade und Pfad-Präfix der gescheiterten
Anfrage: Das Recht öffnet ein Ziel, es entscheidet nichts, und deshalb kommt die
Anfrage weiterhin jedes Mal vor einen Menschen. Ein vorgeschlagenes `allow` würde
mehr öffnen als das Erlauben, das gerade gescheitert ist. Damit dieser Vorschlag
wirkt, gilt `allow_private` einer passenden Regel für jede Aktion und nicht nur
für `allow` (HUM-102).

Ein Vorschlag muss `parse_rules` bestehen und danach auch auf die Anfrage passen;
wo eine Anfrage keins von beidem zulässt, gibt es keinen Vorschlag, und die
Diagnostic sagt, warum. Das betrifft Port `0`, den keine Regel tragen kann, und
eine Methode, die die Regel-Engine nicht kennt und auf die deshalb überhaupt keine
Regel passt. Ergibt der Pfad kein Präfix, das auf die Anfrage passt, entfällt das
Feld; die Regel gilt dann für jeden Pfad dieses Hosts, und die Diagnostic spricht
diese Breite aus. Die Einzelheiten stehen in `backlog/CONVENTIONS.md` 4.10.

Offen bleibt, wo die angenommene Regel landet. Eine `FixAction::AddRule` trägt
keine Position, und der Weg über die Leitung hängt sie ans Ende der
Nutzerregeln; passt schon eine ältere Regel auf denselben Host, entscheidet
weiter diese Regel. Solange das so ist, nennt die Diagnostic die Bedingung in
ihrem `why` und in der Notiz der vorgeschlagenen Regel, statt einen Knopf
anzubieten, der stillschweigend nichts tut. Die Abhilfe berührt die Form auf der
Leitung und gehört in ein eigenes Issue.

Die Adresse selbst geht in die Diagnostic und in `resolved_ip` des
`Failed`-Ereignisses, also an die Oberfläche. Sie steht nicht im Body der
`502`-Antwort und in keinem Header: Die Sandbox hat keinen Resolver, der Agent
kennt also nur den Namen, und die Zuordnung dieses Namens zu einer privaten
Adresse wäre für ihn eine neue Information über das lokale Netz.

**Die Domain-Vorschau in der Oberfläche holt nie automatisch etwas aus dem
Netz.** Die Karte im Domain-Bereich stammt aus einem mitgelieferten Katalog
(`catalog/domains.yaml`). Ein Live-Abruf (Favicon, `og:title`) geschieht nur auf
ausdrücklichen Klick, auf der Seite des Rechners und nur für die eTLD+1.

## Begründung

Der Hostname ist die einzige Information, die ein Angreifer kontrollieren und
ohne Erlauben nach draußen tragen kann. Wer ihn vor der Entscheidung auflöst, hat
einen Ausleitungskanal mit 63 Bytes pro Label offen gelassen, leise, ohne jedes
Zutun des Nutzers und in einem Protokoll, das die meisten nicht als Datenkanal
wahrnehmen. Die Auflösung nach hinten zu verschieben kostet fast nichts und
schließt den Kanal vollständig.

Die Prüfung auf private Adressen muss *nach* der Auflösung stattfinden, nicht
vorher am Namen, denn genau das ist der Rebinding-Trick: Ein völlig öffentlich
aussehender Name zeigt auf `169.254.169.254`. Und weil die geprüfte IP dann für
die Verbindung festgenagelt ist, kann zwischen Prüfung und Verbindung kein
zweiter Auflösungsversuch dazwischenkommen.

`allow_private` als Regel-Flag statt als globale Einstellung hält die Ausnahme
sichtbar und lokal. Der LLM-Server ist der einzige Fall, in dem eine private
Adresse normal ist, und er hat ohnehin seine eigene, eng geschnittene Regel
(Host + Port + Pfad-Präfix + Methode). Ein Nutzer, der eine weitere private
Adresse braucht, sieht seine Ausnahme als eine Zeile in `rules.yaml`.

Die automatische Domain-Vorschau wäre derselbe Fehler in anderer Farbe: Die
Oberfläche würde eine Verbindung genau zu der Domain öffnen, über die der Mensch
gerade entscheidet, und damit dem Angreifer bestätigen, dass jemand hinschaut,
mit der IP-Adresse des Rechners, nicht der der Sandbox.

## Verworfene Alternativen

- **Auflösen und die IP in der Oberfläche zeigen.** Wäre für den Nutzer
  aufschlussreich („diese Domain zeigt auf einen Server in …"), öffnet aber genau
  den Kanal, den dieses ADR schließt. Der Gewinn an Information wiegt das nicht
  auf; den nützlichen Teil davon liefert der Katalog offline.
- **Über DNS over HTTPS bei einem festen Resolver auflösen.** Verschlüsselt den
  Kanal, schließt ihn aber nicht: Der Angreifer kontrolliert die autoritative
  Zone und sieht die Anfrage dort, egal auf welchem Weg sie ihn erreicht hat.
- **Nur die eTLD+1 auflösen.** Verkleinert den Kanal, statt ihn zu schließen.
  Eine halbe Sache, die sich schwer erklären lässt.
- **Private Adressen am Namen prüfen (`localhost`, `*.local`, IP-Literale).**
  Erwischt den naiven Fall und verfehlt den interessanten: einen öffentlichen
  Namen mit privater Antwort. Bleibt als zusätzliche Prüfung sinnvoll, nicht als
  einzige.
- **Private Adressen global erlauben, mit einer Warnung.** Verworfen:
  Cloud-Metadaten unter `169.254.169.254` sind ein Standardziel von
  Ausleitungs-Payloads. Standardmäßig erlauben und ein Warnbanner zeigen ist kein
  Fail-closed-Verhalten.
- **Die DNS-Auflösung dem vorgeschalteten Proxy überlassen.** Genau das geschieht
  im späteren Modus `Socks5h` (ADR-0017), in dem lokale Auflösung sogar verboten
  ist. Für den MVP ohne vorgeschalteten Proxy ist die lokale Auflösung nach dem
  Erlauben der einfachere Weg.

## Folgen

- Die Warteschlange der angehaltenen Anfragen und die Regel-Engine arbeiten nur
  mit Strings. Beide bleiben dadurch frei von IO und tabellengetrieben testbar
  (ADR-0015).
- Ein Auflösungsfehler tritt erst *nach* dem Erlauben auf und ist deshalb ein
  eigener Zustand: `FlowState::Failed{ UpstreamError::Dns }`, `502` an den
  Client (ADR-0004). Der Nutzer sieht „du hast es erlaubt, aber der Name lässt
  sich nicht auflösen" statt eines stillen Blockierens.
- Der Test für diese Entscheidung misst, statt zu behaupten: Escape-Test 3 prüft,
  dass der Host-Resolver **null** Anfragen zeigt, solange die Anfrage in der
  Warteschlange steht (HUM-024, Resolver-Statistik).
- `Egress::connect` bekommt die IP als Parameter. Die Stelle, an der Auflösung und
  Verbindung auseinandergehen, ist dadurch im Typsystem sichtbar.
- Homographen und Punycode werden bei der Normalisierung behandelt, nicht bei der
  Auflösung: Der angezeigte und der geprüfte Host sind derselbe A-Label-String
  (ADR-0007).
- Die Karte aus dem Katalog muss ohne Netz genug liefern, damit der Nutzer
  entscheiden kann. Daher rund 200 mitgelieferte Dev-Dienste mit Icon, Kategorie
  und Beschreibung sowie ein Tranco-Rang aus einer mitgelieferten Liste
  (HUM-031).

## Betroffene Issues

`HUM-024` (DNS erst nach dem Erlauben, mit Resolver-Statistik im Test),
`HUM-015` (Proxy-Kern, Verbindungsaufbau über `Egress`), `HUM-031` (Domain-Bereich
ohne automatischen Abruf), `HUM-039` (LLM-Durchleitung setzt `allow_private`),
`HUM-022` (Regel-Flag `allow_private`), `HUM-102` (die Diagnostic `PROXY_008` mit
ihrem Regelvorschlag; `allow_private` gilt seitdem für jede Aktion).
