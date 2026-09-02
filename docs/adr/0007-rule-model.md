# ADR-0007 · Regel-Modell: geordnete Liste, first match wins, Default `ask`
Status: Accepted
Datum: 2026-09-02

## Kontext

Der Mensch soll entscheiden, aber nicht dreihundertmal dieselbe Entscheidung
treffen. Nach dem dritten `allow` für `registry.npmjs.org` muss es eine Regel
geben, sonst benutzt niemand das Werkzeug länger als eine Stunde. Zugleich ist
jede Regel eine dauerhaft geöffnete Tür: Sie muss so präzise formulierbar sein,
dass sie nicht mehr aufmacht als gemeint, und so lesbar bleiben, dass ein Nutzer
Wochen später versteht, was er da erlaubt hat.

Die Fallen sind bekannt und alle schon einmal in echten Produkten aufgetreten:
Substring-Matching auf Hostnamen (`github.com` matcht `evil-github.com`),
Groß-/Kleinschreibung, trailing dot, internationalisierte Domains und ihre
Punycode-Form, IP-Literale, die eine Host-Regel unterlaufen, und Domain Fronting
— derselbe TLS-Tunnel, aber ein anderer `Host`-Header innen als außen.

## Entscheidung

Regeln sind eine **geordnete Liste**, es gilt **first match wins**, und wenn
nichts matcht, ist die Aktion **`ask`**. Format ist YAML in `rules.yaml`, das
Schema bewusst so gehalten, dass es später nach [Cedar](https://www.cedarpolicy.com/)
übersetzbar bleibt.

**Aktionen:** `allow`, `block`, `ask`, `redact` (Pseudonymisierer laufen lassen,
danach `allow` oder `ask`).

**Matcher-Schlüssel:** `host`, `method`, `path`, `scheme`, `port`, `upgrade`.

- `host` ist ein Glob **auf Labels, nie auf Substrings**: `*` steht für genau
  ein Label, `**` für ein oder mehr Label, ein Host ohne Stern matcht exakt.
  `**.example.com` matcht zusätzlich `example.com` selbst.
- `path` ist ein Glob oder, mit führendem `~`, ein regulärer Ausdruck
  (`~^/v[0-9]+/`).
- `upgrade: websocket` matcht nur einen WebSocket-Upgrade-Request; ein solcher
  Request matcht keine Regel ohne diesen Schlüssel und läuft sonst in `ask`.

**Normalisierung vor jedem Vergleich:** Hostnamen werden auf A-Label
(Punycode, `idna::domain_to_ascii`) gebracht, kleingeschrieben und um einen
trailing dot gekürzt. `API.GITHUB.COM.` und `api.github.com` sind derselbe Host.

**IP-Literale matchen nie eine Host-Regel.** Für sie braucht es eine eigene
Schreibweise: `host: "ip:192.168.1.50"` oder `host: "cidr:192.168.0.0/16"`.
Damit ist ausgeschlossen, dass `**` versehentlich `169.254.169.254` einschließt.

**`expires`:** `never` | `session` | Zeitstempel — genau die drei Varianten
`Expiry::Never | Session | At` aus `backlog/CONVENTIONS.md` 3.3. `session` ist
an die Sandbox-Instanz gebunden, nicht an eine Uhrzeit — wer den Agenten neu
startet, startet mit sauberer Weste. „Einmal erlauben" ist keine Regel und
darum kein `expires`-Wert: Es entscheidet nur diesen einen Flow und legt nichts
an.

**Session-Regeln werden vor persistenten Regeln ausgewertet.** Innerhalb jeder
der beiden Gruppen gilt die Listenreihenfolge. Grund: Was der Nutzer soeben
entschieden hat, soll sofort gelten, auch wenn eine ältere, breitere persistente
Regel ebenfalls matchen würde.

**`allow_private: true`** erlaubt private Zieladressen nach der Auflösung; ohne
das Flag werden sie geblockt (ADR-0006).

**Authority-Konsistenz:** Nach der TLS-Terminierung wird pro Request geprüft,
dass `Host` beziehungsweise `:authority` mit dem CONNECT-Ziel und der SNI
übereinstimmt. Ein Mismatch wird ohne Nachfrage geblockt
(`BlockReason::AuthorityMismatch`, `403`). Das ist Domain Fronting, und es gibt
keinen legitimen Grund, dazu einen Menschen zu fragen.

**WebSocket:** Der Upgrade ist eine eigene, mit `ask` vorbelegte Entscheidung.
Danach werden Frames aufgezeichnet, aber nicht einzeln angehalten. Das UI sagt
diese Einschränkung ausdrücklich, statt sie zu verschweigen.

## Begründung

First match wins mit Default `ask` ist die einzige Ordnung, die fail-closed ist
und sich trotzdem in einem Satz erklären lässt: „Die erste passende Zeile
gewinnt; passt keine, wirst du gefragt." Ein Nutzer kann eine solche Liste von
oben nach unten lesen und vorhersagen, was passiert. Bei prioritäts- oder
spezifitätsbasierter Auflösung kann er das nicht.

Label-Globs statt Substrings sind nicht Bequemlichkeit, sondern die Abwehr einer
ganzen Angriffsklasse. `github.com` als Substring matcht `evil-github.com`,
`github.com.evil.io` und `mygithub.community`. Als Label-Ausdruck matcht
`*.github.com` genau `api.github.com` und `raw.github.com`, aber weder
`github.com` selbst noch `a.b.github.com` noch irgendetwas mit fremdem Apex. Die
Regel-Tabelle in Escape-Test 4 zählt diese Fälle einzeln auf und ist damit die
ausführbare Fassung dieses Absatzes.

Die Normalisierung auf A-Label schließt die Homograph-Lücke: `рaypal.com` mit
kyrillischem `р` wird zu `xn--aypal-uye.com` und ist damit sichtbar nicht
`paypal.com` — sowohl für die Regel als auch für das Auge im UI.

Dass IP-Literale nie eine Host-Regel matchen, ist eine bewusste Reibung. Wer
`169.254.169.254` erreichen will, schreibt das hin. Ein Nutzer, der
`**.example.com` freigibt, hat damit garantiert keine Adresse freigegeben.

`session` an die Sandbox-Instanz zu binden statt an eine Zeitspanne, macht die
Lebensdauer verständlich: Die Regel gilt für „diesen Agentenlauf". Eine
Zeitspanne wäre in dem Moment falsch, in dem der Nutzer den Agenten neu startet
und die alte Regel weiterlebt.

Die Vorrangstellung der Session-Regeln löst einen konkreten Ärger: Der Nutzer
klickt „diese Session erlauben", und nichts passiert, weil eine drei Wochen alte
`block`-Regel weiter oben steht. Die Reihenfolge macht die soeben getroffene
Entscheidung wirksam.

YAML statt einer eigenen Sprache, weil Nutzer die Datei von Hand bearbeiten
sollen und weil das Schema mit `serde_yaml` typisiert einlesbar ist. Die Nähe
zu Cedar ist eine offengehaltene Tür, keine Abhängigkeit.

## Verworfene Alternativen

- **OPA/Rego als Policy-Engine.** Mächtig und etabliert, aber: eine eigene
  Sprache, die die Zielgruppe nicht lesen kann, eine schwer vorhersagbare
  Auswertung, ein zusätzliches Laufzeitartefakt im Paket und eine deutlich
  größere TCB. Für eine geordnete Liste mit sechs Schlüsseln ist das
  unverhältnismäßig.
- **Cedar sofort.** Gute Semantik, aber die Rust-Integration hätte die
  Regel-Engine im MVP komplizierter gemacht, ohne dass jemand die
  Ausdrucksstärke braucht. Deshalb: Schema so schneiden, dass eine spätere
  Übersetzung möglich bleibt, aber nicht heute integrieren.
- **Spezifischste Regel gewinnt.** Klingt hilfreich, ist aber nicht vorhersagbar:
  Der Nutzer müsste eine Spezifitätsmetrik im Kopf haben, um seine eigene Liste
  zu verstehen. First match wins braucht nur Lesen von oben nach unten.
- **Default `allow` mit Blocklisten.** Der Standardweg fast aller Firewalls und
  hier grundfalsch: Humanitl existiert, weil man dem Agenten nicht traut. Eine
  Blockliste schützt nur gegen bekannte Ziele.
- **Reguläre Ausdrücke für Hostnamen.** Zu leicht falsch zu schreiben
  (`.` unescaped, fehlende Anker), zu leicht zu weit. Regex bleibt für `path`
  erlaubt, wo die Folgen eines Fehlers kleiner sind, und ist dort durch das
  `~`-Präfix als Ausnahme markiert.
- **Domain Fronting nachfragen statt blocken.** Verworfen: Es gibt keinen
  legitimen Fall, in dem der innere `Host` vom CONNECT-Ziel abweicht, und eine
  Nachfrage würde den Nutzer trainieren, eine Angriffssignatur wegzuklicken.
- **WebSocket-Frames einzeln halten.** Wäre konsequent, macht aber jede
  interaktive Verbindung unbrauchbar. Der Aufsatzpunkt ist der Upgrade; der Rest
  wird aufgezeichnet und die Lücke wird im UI benannt. `experimental.ws_hold`
  hält die Tür offen.

## Konsequenzen

- Die Regel-Engine (`humanitl-rules`) ist rein: Parsen, Normalisieren, Matchen,
  ohne IO und ohne async. Die Werttypen (`Rule`, `Matcher`, `Action`, `Expiry`,
  `HostPattern`, `Upgrade`) liegen in `humanitl-core::rule`
  (`backlog/CONVENTIONS.md` 4.1).
- Regeln haben zwei Ablageorte: persistent in `rules.yaml` und temporär im
  Speicher des Daemons (`expires: session`). Das UI zeigt beide in getrennten
  Tabs, temporäre mit Restlaufzeit und „dauerhaft machen" (ADR-0011).
- `RuleSet::evaluate` liefert `Verdict::Matched { rule, action }` oder
  `Verdict::Default`, das `ask` bedeutet. Unbekannte HTTP-Methoden führen zu
  `ask`, nicht zu einem Fehler.
- Jede Regel kann ihre Herkunft nennen (`created_from: <FlowId>`), damit das
  Rules-Screen „erstellt vor 2 min aus Request #41" anzeigen kann.
- Escape-Test 4 ist die verbindliche Tabelle: `*.github.com` gegen
  `api.github.com` ✓, `github.com` ✗, `evil-github.com` ✗, `github.com.evil.io` ✗,
  `a.b.github.com` ✗, `API.GITHUB.COM.` ✓, dazu die `**`-Varianten, IP-Literale
  und der WebSocket-Fall.
- Mitgelieferte Regeln (`rules/default.yaml`) tragen ein `bundled: true` und
  werden im UI als solche gekennzeichnet, damit ein Nutzer erkennt, was er selbst
  angelegt hat.

## Betroffene Issues

`HUM-022` (Regel-Engine, Escape-Test 4), `HUM-023` (Host/SNI/Authority-Konsistenz),
`HUM-027` (Rules-RPCs inklusive `dry_run`), `HUM-033` (Rules-Screen mit Tabs
„Gespeichert" und „Temporär"), `HUM-038` (Default-Regeln), `HUM-039`
(LLM-Passthrough-Regel), `HUM-065` (`humanitl rules test`).
