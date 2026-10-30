---
source: adr/0007-rule-model.md
source-sha256: 172f3704cf5014deab3553fc1d0ce4be29786f78642dc819d65183313aaf408f
lang: de
---
# ADR-0007 · Regelmodell: geordnete Liste, die erste passende Regel gilt, Vorgabe `ask`

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Der Mensch soll entscheiden, aber nicht dreihundertmal dieselbe Entscheidung
treffen. Nach dem dritten `allow` für `registry.npmjs.org` muss es eine Regel
geben, sonst nutzt niemand das Werkzeug länger als eine Stunde. Zugleich ist
jede Regel eine dauerhaft geöffnete Tür: Sie muss sich so genau formulieren
lassen, dass sie nicht mehr öffnet als beabsichtigt, und sie muss lesbar genug
bleiben, dass ein Nutzer noch Wochen später versteht, was er dort erlaubt hat.

Die Fallen sind bekannt und alle schon in echten Produkten vorgekommen:
Teilstring-Vergleich auf Hostnamen (`github.com` passt auf `evil-github.com`),
Groß- und Kleinschreibung, ein abschließender Punkt, internationalisierte
Domains und ihre Punycode-Form, IP-Literale, die eine Host-Regel unterlaufen,
und Domain Fronting: derselbe TLS-Tunnel, aber innen ein anderer
`Host`-Header als außen.

## Entscheidung

Regeln sind eine **geordnete Liste**, es gilt **die erste passende Regel**, und
passt keine, ist die Aktion **`ask`**. Das Format ist YAML in `rules.yaml`, das
Schema bewusst so gehalten, dass es sich später nach
[Cedar](https://www.cedarpolicy.com/) übersetzen lässt.

**Aktionen:** `allow`, `block`, `ask`, `redact` (die Pseudonymisierer
ausführen, dann `allow` oder `ask`).

**Matcher-Schlüssel:** `host`, `method`, `path`, `scheme`, `port`, `upgrade`.

- `host` ist ein Glob **auf Labels, nie auf Teilstrings**: `*` steht für genau
  ein Label, `**` für ein oder mehrere Labels, ein Host ohne Stern passt
  genau. `**.example.com` passt zusätzlich auf `example.com` selbst.
- `path` ist ein Glob oder, mit vorangestelltem `~`, ein regulärer Ausdruck
  (`~^/v[0-9]+/`).
- `upgrade: websocket` passt nur auf eine WebSocket-Upgrade-Anfrage; eine
  solche Anfrage passt auf keine Regel ohne diesen Schlüssel und landet sonst
  bei `ask`.

**Normalisierung vor jedem Vergleich:** Hostnamen werden in die A-Label-Form
gebracht (Punycode, `idna::domain_to_ascii`), kleingeschrieben und von einem
abschließenden Punkt befreit. `API.GITHUB.COM.` und `api.github.com` sind
derselbe Host.

**IP-Literale passen nie auf eine Host-Regel.** Sie brauchen eine eigene
Schreibweise: `host: "ip:192.168.1.50"` oder `host: "cidr:192.168.0.0/16"`.
Das schließt aus, dass `**` versehentlich `169.254.169.254` einschließt.

**`expires`:** `never` | `session` | Zeitstempel, genau die drei Varianten
`Expiry::Never | Session | At` aus `backlog/CONVENTIONS.md` 3.3. `session` ist
an die Sandbox-Instanz gebunden, nicht an eine Uhrzeit: Wer den Agenten neu
startet, fängt mit einem leeren Blatt an. „Einmal erlauben" ist keine Regel und
deshalb kein `expires`-Wert: Es entscheidet nur diesen einen Flow und legt
nichts an.

**Session-Regeln werden vor dauerhaften Regeln ausgewertet.** Innerhalb jeder
der beiden Gruppen gilt die Reihenfolge der Liste. Grund: Was der Nutzer gerade
entschieden hat, soll sofort gelten, auch wenn eine ältere, breitere dauerhafte
Regel ebenfalls passen würde.

**`allow_private: true`** erlaubt private Zieladressen nach der Auflösung; ohne
das Flag werden sie blockiert (ADR-0006).

**Konsistenz der Authority:** Nach der TLS-Terminierung wird je Anfrage
geprüft, dass `Host` beziehungsweise `:authority` zum CONNECT-Ziel und zur SNI
passt. Eine Abweichung wird ohne Nachfrage blockiert
(`BlockReason::AuthorityMismatch`, `403`). Das ist Domain Fronting, und es gibt
keinen legitimen Grund, einen Menschen danach zu fragen.

**WebSocket:** Das Upgrade ist eine eigene Entscheidung, voreingestellt auf
`ask`. Danach werden Frames aufgezeichnet, aber nicht einzeln angehalten. Die
Oberfläche nennt diese Einschränkung ausdrücklich, statt sie zu verschweigen.

## Begründung

Die erste passende Regel gilt, mit Vorgabe `ask`: Das ist die einzige
Reihenfolge, die im Zweifel schließt und sich trotzdem in einem Satz erklären
lässt: „Die erste passende Zeile gilt; passt keine, wirst du gefragt." Ein
Nutzer kann eine solche Liste von oben nach unten lesen und vorhersagen, was
passiert. Bei einer Auflösung nach Priorität oder Spezifität kann er das nicht.

Label-Globs statt Teilstrings sind keine Bequemlichkeit, sondern die Abwehr
gegen eine ganze Klasse von Angriffen. `github.com` als Teilstring passt auf
`evil-github.com`, `github.com.evil.io` und `mygithub.community`. Als
Label-Ausdruck passt `*.github.com` genau auf `api.github.com` und
`raw.github.com`, aber weder auf `github.com` selbst noch auf `a.b.github.com`
noch auf irgendetwas mit fremder Apex-Domain. Die Regeltabelle in Escape-Test 4
zählt diese Fälle einzeln auf und ist damit die ausführbare Fassung dieses
Absatzes.

Die Normalisierung auf die A-Label-Form schließt die Homograph-Lücke:
`рaypal.com` mit kyrillischem `р` wird zu `xn--aypal-uye.com` und ist damit
sichtbar nicht `paypal.com`, für die Regel ebenso wie für das Auge in der
Oberfläche.

Dass IP-Literale nie auf eine Host-Regel passen, ist gewollte Reibung. Wer
`169.254.169.254` erreichen will, schreibt das hin. Ein Nutzer, der
`**.example.com` erlaubt, hat damit ganz sicher keine Adresse erlaubt.

`session` an die Sandbox-Instanz statt an eine Zeitspanne zu binden, macht die
Lebensdauer verständlich: Die Regel gilt für „diesen Lauf des Agenten". Eine
Zeitspanne wäre in dem Moment falsch, in dem der Nutzer den Agenten neu startet
und die alte Regel weiterlebt.

Der Vorrang der Session-Regeln löst ein konkretes Ärgernis: Der Nutzer klickt
„für diese Session erlauben", und nichts passiert, weil weiter oben eine drei
Wochen alte `block`-Regel steht. Die Reihenfolge macht die eben getroffene
Entscheidung wirksam.

YAML statt einer eigenen Sprache, weil Nutzer die Datei von Hand bearbeiten
sollen und weil sich das Schema mit `serde_yaml` typisiert einlesen lässt. Die
Nähe zu Cedar ist eine offen gehaltene Tür, keine Abhängigkeit.

## Verworfene Alternativen

- **OPA/Rego als Policy-Engine.** Mächtig und etabliert, aber: eine eigene
  Sprache, die das Zielpublikum nicht lesen kann, eine schwer vorhersagbare
  Auswertung, ein zusätzliches Laufzeit-Artefakt im Paket und eine erheblich
  größere TCB. Für eine geordnete Liste mit sechs Schlüsseln ist das
  unverhältnismäßig.
- **Sofort Cedar.** Gute Semantik, aber die Rust-Anbindung hätte die
  Regel-Engine im MVP komplizierter gemacht, ohne dass jemand die
  Ausdrucksstärke braucht. Deshalb: das Schema so schneiden, dass eine spätere
  Übersetzung möglich bleibt, aber heute nicht einbinden.
- **Die spezifischste Regel gilt.** Klingt hilfreich, ist aber nicht
  vorhersagbar: Der Nutzer müsste ein Spezifitätsmaß im Kopf behalten, um seine
  eigene Liste zu verstehen. „Die erste passende Regel gilt" verlangt nur, von
  oben nach unten zu lesen.
- **Vorgabe `allow` mit Sperrlisten.** Der Standardweg fast aller Firewalls und
  hier grundsätzlich falsch: Humanitl gibt es, weil dem Agenten nicht vertraut
  wird. Eine Sperrliste schützt nur vor bekannten Zielen.
- **Reguläre Ausdrücke für Hostnamen.** Zu leicht falsch zu schreiben (`.`
  nicht maskiert, fehlende Anker), zu leicht zu breit zu fassen. Für `path`
  bleiben reguläre Ausdrücke erlaubt, weil die Folgen eines Fehlers dort
  kleiner sind, und sind dort durch das Präfix `~` als Ausnahme gekennzeichnet.
- **Bei Domain Fronting nachfragen statt blockieren.** Verworfen: Es gibt
  keinen legitimen Fall, in dem der innere `Host` vom CONNECT-Ziel abweicht,
  und eine Nachfrage würde den Nutzer darauf trainieren, eine Angriffssignatur
  wegzuklicken.
- **WebSocket-Frames einzeln anhalten.** Wäre konsequent, macht aber jede
  interaktive Verbindung unbrauchbar. Der Ansatzpunkt ist das Upgrade; der Rest
  wird aufgezeichnet, und die Lücke wird in der Oberfläche benannt. Ein Schalter
  `experimental.ws_hold` sollte die Tür offen halten; er ist mit HUM-121
  entfallen, weil er nie einen Leser bekam und der Upgrade-Pfad selbst noch
  aussteht (HUM-110). Die Tür bleibt offen, sie hängt nur nicht mehr an einem
  wirkungslosen Schlüssel: Wer die Frames anhalten will, bringt den Schalter im
  selben Commit zusammen mit seinem Leser zurück (`backlog/CONVENTIONS.md`
  4.25).

## Folgen

- Die Regel-Engine (`humanitl-rules`) ist rein: Parsen, Normalisieren,
  Vergleichen, ohne IO und ohne async. Die Werttypen (`Rule`, `Matcher`,
  `Action`, `Expiry`, `HostPattern`, `Upgrade`) liegen in `humanitl-core::rule`
  (`backlog/CONVENTIONS.md` 4.1).
- Regeln haben zwei Speicherorte: dauerhaft in `rules.yaml` und vorübergehend
  im Speicher des Daemons (`expires: session`). Die Oberfläche zeigt beide in
  getrennten Tabs, die vorübergehenden mit verbleibender Lebensdauer und
  „Dauerhaft machen" (ADR-0011).
- `RuleSet::evaluate` liefert `Verdict::Matched { rule, action }` oder
  `Verdict::Default`, was `ask` bedeutet. Unbekannte HTTP-Methoden führen zu
  `ask`, nicht zu einem Fehler.
- Jede Regel kann ihre Herkunft nennen (`created_from: <FlowId>`), damit der
  Regel-Bildschirm „vor 2 Min. aus Anfrage #41 angelegt" zeigen kann.
- Escape-Test 4 ist die verbindliche Tabelle: `*.github.com` gegen
  `api.github.com` ✓, `github.com` ✗, `evil-github.com` ✗, `github.com.evil.io` ✗,
  `a.b.github.com` ✗, `API.GITHUB.COM.` ✓, dazu die `**`-Varianten, IP-Literale
  und der WebSocket-Fall.
- Mitgelieferte Regeln (`rules/default.yaml`) tragen ein `bundled: true` und
  sind in der Oberfläche als solche gekennzeichnet, damit ein Nutzer erkennt,
  was er selbst angelegt hat.

## Betroffene Issues

`HUM-022` (Regel-Engine, Escape-Test 4), `HUM-023` (Konsistenz von Host, SNI und
Authority), `HUM-027` (Regel-RPCs einschließlich `dry_run`), `HUM-033`
(Regel-Bildschirm mit den Tabs „Gespeichert" und „Temporär"), `HUM-038`
(Standardregeln), `HUM-039` (Regel für die LLM-Durchleitung), `HUM-065`
(`humanitl rules test`).
