---
source: reference/rules.md
source-sha256: 7082af93eacd27e33ad38a668a2ad59eca6b0ea20733d62e55f3d24260216d3c
lang: de
---
# Regel-Referenz

Applies to: Humanitl 0.1

Eine Regel entscheidet eine Anfrage, ohne einen Menschen zu fragen. Ohne passende
Regel wird jede Anfrage angehalten, und ein Mensch entscheidet (`ask`). Diese Seite
beschreibt das Dateiformat von `rules.yaml`, wie eine Regel passt, in welcher
Reihenfolge Regeln ausgewertet werden und welche Regeln Humanitl mitbringt.

- [Wo Regeln liegen](#where-rules-live)
- [Wie die Beispiele auf dieser Seite geprüft werden](#how-the-examples-on-this-page-are-checked)
- [Aufbau der Datei](#file-layout)
- [Felder einer Regel](#rule-fields)
- [Semantik des Vergleichs](#matching-semantics)
- [Reihenfolge der Auswertung](#evaluation-order)
- [Eine Regel ausprobieren](#trying-a-rule)
- [Rezepte](#recipes)
- [Mitgelieferte Regeln](#bundled-rules)
- [Diagnosen](#diagnostics)

## Wo Regeln liegen

| Quelle | Ort | Geschrieben von | Entfernbar |
|---|---|---|---|
| Deine dauerhaften Regeln | `$XDG_CONFIG_HOME/humanitl/rules.yaml`, Modus `0600` | dem Daemon, im Auftrag der Oberfläche und von `humanitl rules add/update/remove` | ja |
| Deine Session-Regeln | nur im Speicher | dem Daemon, wenn eine Regel mit `expires: session` angelegt wird | ja, und sie enden mit der Session |
| Profilregeln | `[rules].inline` und `[rules].files` eines Profils | dir, im Profil | nein, sie gehören zum Profil |
| Mitgelieferte Regeln | [`rules/default.yaml`](../../rules/default.yaml) und der Agenten-Adapter | dem Build | nein; abschalten mit `disabled_bundled` |

Der Daemon liest `rules.yaml` beim Start. Hast du die Datei von Hand bearbeitet,
führe `humanitl rules reload` aus; der Daemon beobachtet die Datei nicht.

Das Einlesen ist streng. Ein unbekannter Schlüssel, eine unbekannte Methode, ein
Muster, das sich nicht kompilieren lässt, oder eine doppelte ID ist ein Fehler, und
**ein einziger Fehler weist die ganze Datei zurück**. Der Daemon läuft dann mit dem
letzten gültigen Regelsatz weiter und meldet die Diagnosen. Ein halb geladener
Regelsatz wäre schlimmer, weil niemand sagen könnte, welche Hälfte fehlt. Warnungen
weisen die Datei nicht zurück; sie werden neben den geladenen Regeln gemeldet.

## Wie die Beispiele auf dieser Seite geprüft werden

Jeder umzäunte `yaml`-Block auf dieser Seite ist eine vollständige `rules.yaml` und
wird vom Test `rules_reference_examples`
(`daemon/crates/rules/tests/reference_examples.rs`) mit dem Parser des Daemons
eingelesen. Ein Block, der sich nicht einlesen lässt, lässt den Build scheitern.
Ausschnitte, die zurückgewiesen werden sollen, stehen als `text`, nie als `yaml`.

Innerhalb eines Blocks ist ein Kommentarpaar wie dieses eine Probe:

```text
# humanitl rules test https://api.github.com/repos --method POST
# verdict: block
```

Der Test wertet die Probe allein gegen diesen Block aus, mit der echten
Regelauswertung, und vergleicht das Urteil. Der Befehl ist der, den du selbst
ausführen kannst (siehe [Eine Regel ausprobieren](#trying-a-rule)); seine erste
Ausgabezeile ist das Urteil. Gegen einen laufenden Daemon kann die Antwort abweichen,
wenn eine mitgelieferte Regel oder eine deiner Regeln zuerst passt; die Proben auf
dieser Seite beschreiben deshalb den Block für sich. Der mitgelieferte Block in
[Mitgelieferte Regeln](#bundled-rules) wird zusätzlich Regel für Regel mit
`rules/default.yaml` verglichen.

## Aufbau der Datei

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "*.github.com"
      method: [GET, HEAD]
    note: "read the GitHub API without asking"
disabled_bundled: []
# humanitl rules test https://api.github.com/repos/demo-project
# verdict: allow
# humanitl rules test https://api.github.com/repos/demo-project --method POST
# verdict: ask
```

| Schlüssel | Typ | Pflicht | Vorgabe | Bedeutung |
|---|---|---|---|---|
| `version` | Ganzzahl | ja | keine | Formatversion. Es gibt nur `1`; alles andere oder ein fehlender Schlüssel ist `RULES_006`. |
| `rules` | Liste von Regeln | nein | `[]` | Die Regeln, in Reihenfolge. Die Reihenfolge zählt nur innerhalb eines Rangs, siehe [Reihenfolge der Auswertung](#evaluation-order). |
| `disabled_bundled` | Liste von Regel-IDs | nein | `[]` | IDs mitgelieferter Regeln, die du abgeschaltet hast. Siehe [`disabled_bundled`](#disabled_bundled). |

Jeder andere Schlüssel auf der obersten Ebene ist ein Fehler (`RULES_001`). Eine
leere Datei mit nur `version: 1` ist gültig und bedeutet „keine eigenen Regeln".

## Felder einer Regel

Eine Regel hat eine `action`, einen `match`-Block und ein paar Flags. Alle Schlüssel
sind kleingeschrieben. Unbekannte Schlüssel in einer Regel oder in `match` sind ein
Fehler (`RULES_001`).

| Feld | Typ | Pflicht | Vorgabe |
|---|---|---|---|
| [`id`](#id) | UUID als Zeichenkette | nein | eine neue ID wird erzeugt |
| [`action`](#action) | `allow`, `block`, `ask`, `redact` | ja | keine |
| [`match.host`](#matchhost) | Host-Muster als Zeichenkette | ja | keine |
| [`match.method`](#matchmethod) | Liste von Methodennamen | nein | jede bekannte Methode |
| [`match.path`](#matchpath) | Glob oder `~` gefolgt von einem regulären Ausdruck | nein | jeder Pfad |
| [`match.path_prefixes`](#matchpath_prefixes) | Liste von Zeichenketten | nein | `[]`, jeder Pfad |
| [`match.scheme`](#matchscheme) | `http`, `https`, `ws`, `wss` | nein | jedes Schema |
| [`match.port`](#matchport) | Ganzzahl von 1 bis 65535 | nein | jeder Port |
| [`match.upgrade`](#matchupgrade) | `websocket` | nein | keine, die Regel passt nur auf Anfragen ohne Upgrade |
| [`expires`](#expires) | `never`, `session`, Zeitstempel nach RFC 3339 | nein | `never` |
| [`stream`](#stream) | Wahrheitswert | nein | `false` |
| [`allow_private`](#allow_private) | Wahrheitswert | nein | `false` |
| [`created_from`](#created_from) | Flow-ID (UUID als Zeichenkette) | nein | keine |
| [`bundled`](#bundled) | Wahrheitswert | nein | `false`, und gesetzt wird es übergangen |
| [`passthrough_llm`](#passthrough_llm) | Wahrheitswert | nein | `false` |
| [`note`](#note) | Zeichenkette | nein | keine |

Jede Bedingung in `match` engt die Regel ein. Eine Anfrage passt, wenn alle
vorhandenen Bedingungen zutreffen.

### `id`

Eine UUID in der üblichen Form mit Bindestrichen, zum Beispiel
`01920000-0000-7000-8000-000000000001`. Der Daemon erzeugt IDs nach UUIDv7; der
Parser nimmt jede UUID-Version an.

- Lass sie weg, wenn du eine Regel von Hand schreibst. Bei jedem Einlesen wird eine
  neue ID erzeugt, also bekommt eine Regel ohne ID nach jedem Neuladen eine andere
  ID. Der Daemon schreibt jede Regel mit ihrer ID zurück, wenn er die Datei das
  nächste Mal speichert.
- Zwei Regeln mit derselben ID weisen die Datei zurück (`RULES_007`).
- Text, der keine UUID ist, weist die Datei zurück (`RULES_001`).

### `action`

Was mit einer passenden Anfrage geschieht.

| Wert | Wirkung in 0.1 | Exit-Code von `humanitl rules test` |
|---|---|---|
| `allow` | Ohne Nachfrage weitergeleitet. Funde von Secrets können sie trotzdem stoppen (harte Sperre). | `0` |
| `block` | Mit `403` zurückgewiesen, ohne Nachfrage. | `10` |
| `ask` | Für einen Menschen angehalten, genau wie ohne Regel. Nützlich, um einen engen Fall vor einem breiteren `allow` zu halten. | `11` |
| `redact` | Vom Parser angenommen, aber wie `ask` angehalten, bis es den Pseudonymisierer gibt. | `11` |

Werte sind kleingeschrieben; `Allow` ist ein Fehler.

### `match.host`

Pflicht. Eine von vier Formen:

| Form | Beispiel | Passt auf |
|---|---|---|
| Genauer Name | `github.com` | genau diesen Namen, keine Subdomain |
| Label-Glob | `*.github.com`, `**.github.com` | Namen, Label für Label |
| `ip:` | `ip:192.168.1.50`, `ip:::1` | genau diese Adresse |
| `cidr:` | `cidr:192.168.0.0/16`, `cidr:2606:4700::/32` | jede Adresse in diesem Netz |

Die vollständigen Vergleichsregeln mit einer Wahrheitstabelle stehen in
[Semantik des Vergleichs](#matching-semantics). Grenzfälle:

- Ein Platzhalter muss ein ganzes Label sein. `*foo.com` und `foo*.com` sind Fehler
  (`RULES_003`), ebenso ein leeres Label (`foo..com`) und ein leeres Muster.
- Ein Port gehört nicht in den Host. Nimm `match.port`.
- Eine nackte IP-Adresse, als Name geschrieben (`host: "140.82.112.3"`), wird als
  `ip:140.82.112.3` gelesen und als Warnung gemeldet (`RULES_002`). Schreib `ip:`,
  um die Absicht deutlich zu machen.
- Ein selbst geschriebenes Punycode-Label (`xn--mnchen-3ya.de`) ist gültig, aber
  eine Warnung (`RULES_002`), weil niemand lesen kann, welcher Name es ist. Schreib
  stattdessen den Unicode-Namen (`münchen.de`); er wird auf dasselbe Label
  normalisiert.
- Ein IPv4-gemapptes Netz (`cidr:::ffff:192.168.0.0/112`) wird in seine IPv4-Form
  umgewandelt. Ein Präfix unter 96 auf einem solchen Netz ist ein Fehler
  (`RULES_003`).

### `match.method`

Eine Liste von HTTP-Methoden, ohne Rücksicht auf Groß- und Kleinschreibung,
gespeichert in Großbuchstaben. Bekannte Methoden sind `GET`, `HEAD`, `POST`, `PUT`,
`PATCH`, `DELETE`, `OPTIONS`, `CONNECT` und `TRACE`. Alles andere in der Liste ist
ein Fehler (`RULES_001`).

- Ohne `method` passt die Regel auf jede bekannte Methode.
- Eine Anfrage mit unbekannter Methode (zum Beispiel `PROPFIND`) passt nie auf eine
  Regel und wird immer angehalten.
- Eine leere Liste (`method: []`) ist gültiges YAML und passt auf nichts. Die Regel
  entscheidet dann nie etwas.

### `match.path`

Ein Glob über den Pfad oder ein regulärer Ausdruck, wenn der Wert mit `~` beginnt.
Der Query-String wird nie verglichen: `/search` passt auf `/search?q=x`, und ein
angehängtes `?a=b` kann eine Regel weder umgehen noch erweitern.

- Glob: `*` passt innerhalb eines Segments und überschreitet nie `/`; `**`
  überschreitet Segmente; `?`, `[abc]` und `{a,b}` funktionieren wie üblich.
  `/repos/*` passt auf `/repos/demo-project`, aber nicht auf
  `/repos/demo-project/issues`.
- Regulärer Ausdruck: nicht verankert, passt also überall im Pfad. Schreib `^` und
  `$`, wenn du Anfang und Ende meinst, zum Beispiel `~^/v[0-9]+/models$`. Die Engine
  ist das Rust-Crate `regex` (ohne Backtracking), mit einer Größengrenze von 1 MiB
  je kompiliertem Ausdruck.
- Ein ungültiger Glob oder regulärer Ausdruck weist die Datei zurück (`RULES_005`).
- Eine Regel mit Pfadbedingung, die eine Anfrage durchlässt (`allow`, `redact`),
  passt nie auf einen Pfad mit einem `..`-Segment, in keiner Schreibweise (`%2e%2e`,
  `..;x`, Backslashes). Eine Regel, die blockiert oder nachfragt, passt auf den Pfad,
  wie er geschrieben ist, **oder** auf den aufgelösten, also erfasst ein `block` für
  `/admin/**` auch `/x/../admin/users`. Weitergeleitet wird die Anfrage trotzdem mit
  dem Pfad, wie er geschrieben ist.

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "api.example.com"
      path: "/repos/*"
  - action: allow
    match:
      host: "api.example.com"
      path: "~^/v[0-9]+/models$"
  - action: allow
    match:
      host: "code.example.com"
      path: "/repos/me/**"
  - action: block
    match:
      host: "admin.example.com"
      path: "/admin/**"
# humanitl rules test https://api.example.com/repos/demo-project
# verdict: allow
# humanitl rules test https://api.example.com/repos/demo-project?page=2
# verdict: allow
# humanitl rules test https://api.example.com/repos/demo-project/issues
# verdict: ask
# humanitl rules test https://code.example.com/repos/me/demo-project
# verdict: allow
# humanitl rules test https://code.example.com/repos/me/../../user/keys
# verdict: ask
# humanitl rules test https://api.example.com/v1/models
# verdict: allow
# humanitl rules test https://api.example.com/v1/models/delete
# verdict: ask
# humanitl rules test https://admin.example.com/x/../admin/users
# verdict: block
```

### `match.path_prefixes`

Eine Liste von Pfadpräfixen. Die Bedingung trifft zu, wenn der Pfad (ohne Query)
mit einem von ihnen beginnt. Ein Eintrag, der auf `?` endet, meint genau diesen Pfad
statt eines Präfixes: `/v1/models?` passt auf `/v1/models` und
`/v1/models?limit=5`, aber nicht auf `/v1/models/delete`.

- Jeder Eintrag muss mit `/` beginnen und mindestens zwei Zeichen lang sein. `""`
  und `/` würden auf jeden Pfad passen und weisen die Datei zurück (`RULES_005`).
- Ein Eintrag, der auf `?` endet, ist eine Warnung (`RULES_005`), damit niemand
  einen genauen Pfad schreibt, während er ein Präfix meint.
- Ein Präfix vergleicht Zeichen, nicht Segmente: `/api` passt auch auf `/apifoo`.
  Schreib `/api/`, wenn du das Verzeichnis meinst.
- Stehen sowohl `path` als auch `path_prefixes` da, müssen beide zutreffen.
- Es gilt dieselbe Behandlung von `..` wie bei `path`.

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "llm.example.com"
      method: [POST, GET]
      path_prefixes: ["/v1/chat/completions?", "/v1/models?"]
# humanitl rules test https://llm.example.com/v1/chat/completions --method POST
# verdict: allow
# humanitl rules test https://llm.example.com/v1/models?limit=5
# verdict: allow
# humanitl rules test https://llm.example.com/v1/models/delete --method POST
# verdict: ask
```

### `match.scheme`

`http`, `https`, `ws` oder `wss`, ohne Rücksicht auf Groß- und Kleinschreibung.
Alles andere ist ein Fehler (`RULES_001`).

Der Proxy zeichnet eine Anfrage unter dem Schema der Verbindung auf, die sie trägt:
`https` in einem `CONNECT`-Tunnel, sonst `http`. Ein WebSocket-Handshake ist eine
solche Anfrage mit einem Upgrade, also lassen sich `ws` und `wss` einlesen, passen
in 0.1 aber auf nichts, was der Proxy sieht. Nimm für WebSockets
[`match.upgrade`](#matchupgrade).

### `match.port`

Der Zielport, 1 bis 65535. `0` ist ein Fehler (`RULES_001`), ebenso eine Zahl über
65535. Eine Anfrage trägt immer einen Port; nennt die URL keinen, gilt die Vorgabe
des Schemas (443 für `https`, 80 für `http`).

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "api.example.com"
      port: 8443
# humanitl rules test https://api.example.com:8443/status
# verdict: allow
# humanitl rules test https://api.example.com/status
# verdict: ask
```

### `match.upgrade`

Der einzige Wert ist `websocket` (ohne Rücksicht auf Groß- und Kleinschreibung). Die
Upgrade-Dimension wirkt in beide Richtungen:

- Eine Regel **ohne** `upgrade` passt nie auf eine Anfrage, die ein Upgrade
  verlangt. Einen Host zu erlauben, erlaubt keine WebSockets zu ihm.
- Eine Regel **mit** `upgrade: websocket` passt nie auf eine gewöhnliche Anfrage.

Jeder andere Wert ist ein Fehler (`RULES_001`). Siehe das
[WebSocket-Rezept](#allowing-a-websocket).

### `expires`

Wie lange die Regel gilt.

| Wert | Bedeutung |
|---|---|
| `never` | Bis du sie entfernst. Die Vorgabe, wenn der Schlüssel fehlt. |
| `session` | Nur für die laufende Session; im Speicher gehalten, nie in `rules.yaml` geschrieben. |
| Zeitstempel nach RFC 3339, zum Beispiel `"2099-01-01T00:00:00Z"` | Bis zu diesem Zeitpunkt. Jeder Versatz wird angenommen und in UTC umgerechnet. Ab diesem Zeitpunkt wird die Regel übersprungen. |

- `never` und `session` sind kleingeschrieben. Alles andere, was kein Zeitstempel
  nach RFC 3339 ist, ist ein Fehler (`RULES_001`).
- Setz Zeitstempel in Anführungszeichen, damit YAML sie als Text behält.
- Abgelaufene Regeln werden bei der Auswertung übersprungen und entfernt, wenn der
  Daemon den Regelsatz bereinigt.
- Session-Regeln sind das, was die Oberfläche mit der Gültigkeit „diese Session"
  anlegt und was `humanitl rules add --expires session` anlegt. Wird eine Regel mit
  `expires: session` aus einer Datei gelesen, ist sie an die Session gebunden, in der
  der Daemon läuft, wenn er die Datei liest.

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "old.example.com"
    expires: "2025-01-01T00:00:00Z"
  - action: allow
    match:
      host: "new.example.com"
    expires: "2099-01-01T00:00:00+02:00"
  - action: allow
    match:
      host: "now.example.com"
    expires: session
# humanitl rules test https://old.example.com/
# verdict: ask
# humanitl rules test https://new.example.com/
# verdict: allow
# humanitl rules test https://now.example.com/
# verdict: allow
```

### `stream`

Nur von Belang für Anfrage-Bodies über `limits.hold_body_cap_bytes` (standardmäßig
32 MiB). Eine solche Anfrage passt nicht in den Puffer, den ein Mensch prüft, und
wird mit `413` blockiert, auch wenn eine Regel den Host erlaubt. Eine Regel mit
`action: allow` oder `action: ask` und `stream: true` lässt sie als Strom hinaus,
ungeprüft und ohne Vorschau. Mit `ask` wird der Kopf angehalten, und der Body fließt
erst, nachdem ein Mensch zugestimmt hat. Solange
`hold.hard_block_checksum_secrets` an ist, greift `stream: true` nie.

Bei `block` hat das Flag keine Wirkung. Einzelheiten stehen in
[limits.md](limits.md); siehe auch das
[Rezept für gestreamte Uploads](#allowing-a-streaming-upload).

### `allow_private`

Ohne dieses Flag weist der Proxy ein Ziel zurück, das auf eine private Adresse
aufgelöst wird: RFC 1918, Loopback, Link-Local oder CGNAT. Mit `allow_private: true`
darf das Ziel geöffnet werden.

- Das Flag öffnet das Ziel; es entscheidet nichts. Es gilt für jede Aktion, also
  hält `action: ask` mit `allow_private: true` jede Anfrage an und kann einen
  privaten Host trotzdem erreichen, nachdem ein Mensch zugestimmt hat. Das ist die
  Regel, die der Daemon vorschlägt, wenn er eine private Adresse zurückweist.
- Die mitgelieferte LLM-Durchleitung setzt es, weil ein lokales Modell meist im
  eigenen Netz steht.
- Bei `block` hat es keine Wirkung.

### `created_from`

Die ID des Flows, aus dem die Regel angelegt wurde, zum Beispiel wenn du in der
Oberfläche „Merken" wählst. Sie dient nur der Information; am Vergleich ändert sie
nichts. Text, der keine UUID ist, ist ein Fehler (`RULES_001`).

### `bundled`

Sagt, woher eine Regel kommt. Eine Datei kann das nicht entscheiden: Der Daemon
setzt die Markierung nur für Regeln, die er aus `rules/default.yaml` und vom
Agenten-Adapter lädt. In `rules.yaml` oder einem Profil wird `bundled: true`
übergangen und als Warnung gemeldet (`RULES_010`); die Regel wirkt an ihrem Platz in
der Liste weiter.

Der Grund: Ein Rang hängt von der Markierung ab (siehe
[Reihenfolge der Auswertung](#evaluation-order)). Eine Datei, die sie setzen könnte,
könnte sich selbst befördern.

### `passthrough_llm`

Kennzeichnet die erklärte Durchleitung zum Sprachmodell: Eine passende Anfrage wird
weitergeleitet, ohne angehalten zu werden, vollständig aufgezeichnet, und Funde darin
erzeugen eine Warnung (`LLM_005`), statt sie zu stoppen. Das ist der eine erklärte
Seitenkanal (siehe [SECURITY.md](../SECURITY.md)).

- Nur mit `action: allow` erlaubt; mit jeder anderen Aktion wird die Datei
  zurückgewiesen (`RULES_001`).
- Die Durchleitungsregel von 0.1 baut der Agenten-Adapter aus `llm.endpoint` und
  `llm.passthrough_paths`. Du musst keine schreiben.
- Nur diese mitgelieferte Regel wird zuerst ausgewertet. Eine Durchleitung in deiner
  eigenen `rules.yaml` bleibt an ihrem Platz in der Liste. In einem Profil wird das
  Flag entfernt und gemeldet.
- Eine Durchleitung, die nicht einen einzelnen Host, einen Port, ein Schema und eine
  Pfadbedingung nennt, ist eine Warnung (`RULES_008`): Alles, was sie abdeckt, geht
  hinaus, ohne angehalten zu werden.

### `note`

Freier Text: warum es die Regel gibt. Er wird im Bildschirm „Regeln" angezeigt und
in `rules.yaml` aufbewahrt. Schreib ihn; eine Regel ohne Grund ist später schwer zu
prüfen.

### `disabled_bundled`

Eine Liste auf der obersten Ebene mit den IDs mitgelieferter Regeln, die du
abgeschaltet hast. Eine abgeschaltete Regel bleibt im Bildschirm „Regeln" sichtbar
und entscheidet nichts. `humanitl rules disable <ID>` und
`humanitl rules enable <ID>` bearbeiten diese Liste.

- Eine ID, die keine mitgelieferte Regel nennt, bleibt stehen, damit das Abschalten
  eine Version übersteht, in der es die Regel noch nicht oder nicht mehr gibt.
- Eine ID, die eine deiner eigenen Regeln nennt, schaltet auch diese ab, mit einer
  Warnung (`RULES_010`): Eigene Regeln werden entfernt, nicht abgeschaltet.
- Text, der keine UUID ist, ist ein Fehler (`RULES_001`).

```yaml
version: 1
rules: []
disabled_bundled:
  - 01920000-0000-7000-8000-000000000008
```

## Semantik des Vergleichs

### Hostnamen werden zuerst normalisiert

Vor dem Vergleich werden sowohl der Host der Anfrage als auch die festen Labels eines
Musters normalisiert: IDNA nach ASCII (`münchen.de` wird zu `xn--mnchen-3ya.de`),
Kleinschreibung, abschließender Punkt entfernt. Ein Host, der kein gültiger Name und
keine gültige Adresse ist, etwa `0x8c527003` oder `0177.0.0.1`, wird zurückgewiesen,
bevor eine Regel ihn sieht.

### Label-Globs

Hosts werden Label für Label verglichen, nie als Zeichenketten. `*.github.com` kann
nicht auf `evil-github.com` oder `github.com.evil.io` passen, was eine Prüfung auf
Suffix oder Teilstring täte.

- `*` passt auf genau ein Label.
- `**` passt auf ein oder mehrere Labels.
- Ein Muster, das mit `**.` beginnt, passt zusätzlich auf den Namen ohne diesen
  Teil: `**.example.com` passt auf `example.com` selbst (Apex-Ausnahme). Ein `**` in
  der Mitte braucht weiterhin mindestens ein Label.
- Ein Muster ohne `*` ist genau.
- Eine IP-Adresse passt nie auf einen Glob oder einen genauen Namen, nicht einmal auf
  `**`. Nur `ip:` und `cidr:` passen auf Adressen, und sie passen nie auf Namen. Eine
  Regel löst nie einen Namen auf, um ihn gegen eine Adresse zu prüfen.

### Wahrheitstabelle

Die Tabelle ist Test 4 der Escape-Tests (BACKLOG.md 4.5), erweitert um die
Host-Vergleichstabelle der Regel-Engine. Jede Zeile ist ein Test in
`daemon/crates/rules/tests/host_table.rs` (benannt `host_NN`) oder
`daemon/crates/rules/tests/escape_table.rs`.

| Muster | Host der Anfrage | Passt | Warum |
|---|---|---|---|
| `*.github.com` | `api.github.com` | ja | ein Label |
| `*.github.com` | `github.com` | nein | `*` braucht genau ein Label |
| `*.github.com` | `evil-github.com` | nein | Labels, kein Suffix |
| `*.github.com` | `github.com.evil.io` | nein | Labels, kein Teilstring |
| `*.github.com` | `a.b.github.com` | nein | zwei Labels |
| `*.github.com` | `API.GITHUB.COM.` | ja | Schreibweise und abschließender Punkt werden normalisiert |
| `**.github.com` | `github.com` | ja | Apex-Ausnahme |
| `**.github.com` | `api.github.com` | ja | ein Label |
| `**.github.com` | `a.b.c.github.com` | ja | mehrere Labels |
| `**.github.com` | `github.com.evil.io` | nein | Labels, kein Teilstring |
| `**.github.com` | `notgithub.com` | nein | Labels, kein Suffix |
| `github.com` | `github.com` | ja | genau |
| `github.com` | `www.github.com` | nein | genau heißt genau |
| `github.com` | `GitHub.Com` | ja | Schreibweise |
| `*.*.example.com` | `a.b.example.com` | ja | zwei einzelne Labels |
| `*.*.example.com` | `a.example.com` | nein | ein Label zu wenig |
| `api.*.com` | `api.github.com` | ja | Platzhalter in der Mitte |
| `api.*.com` | `api.co.uk` | nein | das letzte Label entscheidet |
| `**` | `anything.example` | ja | jeder Name |
| `**` | `140.82.112.3` | nein | eine Adresse passt nie auf einen Glob |
| `*.github.com` | `140.82.112.3` | nein | eine Adresse passt nie auf einen Glob |
| `münchen.de` | `xn--mnchen-3ya.de` | ja | IDNA-Normalisierung |
| `ip:140.82.112.3` | `140.82.112.3` | ja | genaue Adresse |
| `ip:140.82.112.3` | `140.82.112.4` | nein | eine andere Adresse |
| `ip:140.82.112.3` | `[::ffff:140.82.112.3]` | ja | IPv4-gemapptes IPv6 ist dieselbe Adresse |
| `cidr:192.168.0.0/16` | `192.168.1.50` | ja | innerhalb des Netzes |
| `cidr:192.168.0.0/16` | `10.0.0.1` | nein | außerhalb des Netzes |
| `cidr:192.168.0.0/16` | `192.168.1.50.nip.io` | nein | ein Name, keine Adresse |
| `ip:127.0.0.1` | `localhost` | nein | keine Regel löst einen Namen auf |
| `*.github.com`, einfache Regel | `api.github.com` mit WebSocket-Upgrade | nein, also `ask` | ein Upgrade braucht `upgrade: websocket` |

Den letzten Fall von Test 4, ein `POST` mit 40 MB, entscheidet keine Regel: Zuerst
greift die Body-Obergrenze, und die Anfrage wird gepuffert und angehalten oder
oberhalb der Grenze blockiert, nie gestreamt, außer eine Regel sagt `stream: true`
(siehe [`stream`](#stream)).

Dieselbe Tabelle als Proben:

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "*.github.com"
# humanitl rules test https://api.github.com/
# verdict: allow
# humanitl rules test https://github.com/
# verdict: ask
# humanitl rules test https://evil-github.com/
# verdict: ask
# humanitl rules test https://github.com.evil.io/
# verdict: ask
# humanitl rules test https://a.b.github.com/
# verdict: ask
# humanitl rules test https://API.GITHUB.COM./
# verdict: allow
# humanitl rules test https://140.82.112.3/
# verdict: ask
# humanitl rules test https://api.github.com/ --upgrade websocket
# verdict: ask
```

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "**.github.com"
  - action: allow
    match:
      host: "a.**.example.com"
# humanitl rules test https://github.com/
# verdict: allow
# humanitl rules test https://a.b.c.github.com/
# verdict: allow
# humanitl rules test https://notgithub.com/
# verdict: ask
# humanitl rules test https://a.example.com/
# verdict: ask
# humanitl rules test https://a.x.y.example.com/
# verdict: allow
```

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "**"
  - action: block
    match:
      host: "ip:140.82.112.3"
  - action: allow
    match:
      host: "cidr:192.168.0.0/16"
    allow_private: true
# humanitl rules test https://anything.example/
# verdict: allow
# humanitl rules test https://140.82.112.3/
# verdict: block
# humanitl rules test http://192.168.1.50:8080/
# verdict: allow
# humanitl rules test http://10.0.0.1/
# verdict: ask
```

Die erste Regel des letzten Blocks erlaubt jeden Namen ohne jede Bedingung; der
Parser nimmt sie mit einer Warnung an (`RULES_008`).

### Pfad, Methode, Schema, Port und Upgrade

Diese sind bei ihren Feldern beschrieben: [`match.path`](#matchpath),
[`match.path_prefixes`](#matchpath_prefixes), [`match.method`](#matchmethod),
[`match.scheme`](#matchscheme), [`match.port`](#matchport),
[`match.upgrade`](#matchupgrade). Eine Regel vergleicht nur das Ziel: Host, Methode,
Pfad, Schema, Port und Upgrade. Header und Body sieht sie sich nie an; die sind für
die Secret-Detektoren und für den Menschen da.

## Reihenfolge der Auswertung

`RuleSet::evaluate` in `daemon/crates/rules/src/eval.rs` tut genau das:

1. Eine Anfrage mit unbekannter Methode bekommt `ask`, ohne dass eine Regel
   angesehen wird.
2. Die Regeln werden in vier Rängen geprüft, in dieser Reihenfolge:
   1. die **mitgelieferte** LLM-Durchleitung (`passthrough_llm` und `bundled`),
   2. deine **Session**-Regeln (`expires: session`, nicht mitgeliefert),
   3. deine **dauerhaften** Regeln (`rules.yaml`),
   4. alles andere, was **mitgeliefert** ist: Profilregeln, der Agenten-Adapter,
      `rules/default.yaml`.
3. Innerhalb eines Rangs gilt die Reihenfolge der Liste, und **die erste passende
   Regel gewinnt**.
4. Abgelaufene und abgeschaltete Regeln werden übersprungen, als gäbe es sie nicht.
5. Passt keine Regel, lautet das Urteil `ask`: Die Anfrage wird für einen Menschen
   angehalten.

Folgen:

- Der Rang gehört zur Regel, nicht zu ihrem Platz in der Liste. Eine Regel zu
  verschieben, ändert nur ihren Platz innerhalb ihres eigenen Rangs; deshalb
  verschiebt `humanitl rules reorder` eine Regel innerhalb ihrer Gruppe.
- Was du eben entschieden hast (eine Session-Regel), gilt sofort, auch wenn eine
  ältere, breitere dauerhafte Regel passen würde.
- Deine eigenen Regeln gehen mitgelieferten vor. Um eine mitgelieferte Regel zu
  überstimmen, schreib eine Regel mit demselben Muster oder schalte sie mit
  `disabled_bundled` ab.
- Die Durchleitung zu deinem Modell bleibt als solche erkennbar: Keine deiner Regeln,
  nicht einmal `block` für `**`, kann sie verdecken. So blockiert das Profil
  `llm-only` alles außer dem Modell.
- Eine mitgelieferte Regel mit `expires: session` bleibt in Rang 4.
- Eine Session-Regel ist an ihre Session gebunden. Eine Regel einer anderen Session
  gilt als abgelaufen.
- Es gibt keinen Weg, auf dem ein Fehler im Regelsatz zu einem stillen Erlauben
  führt: Eine Datei, die sich nicht einlesen lässt, lässt den letzten gültigen Satz
  stehen, und eine Regel, die auf nichts passt, führt zu `ask`.

Die erste passende Regel innerhalb eines Rangs:

```yaml
version: 1
rules:
  - action: block
    match:
      host: "**.github.com"
      method: [POST]
    note: "first matching rule wins"
  - action: allow
    match:
      host: "*.github.com"
# humanitl rules test https://api.github.com/repos --method POST
# verdict: block
# humanitl rules test https://api.github.com/repos
# verdict: allow
# humanitl rules test https://api.github.com/repos --method PATCH
# verdict: allow
```

## Eine Regel ausprobieren

`humanitl rules test <URL>` fragt den laufenden Daemon, wie er eine Anfrage an diese
URL entscheiden würde, mit dem vollständigen Regelsatz einschließlich der
mitgelieferten Regeln:

```text
humanitl rules test https://api.github.com/repos/demo-project
humanitl rules test https://registry.npmjs.org/left-pad --method HEAD
humanitl rules test https://realtime.example.com/socket --upgrade websocket
humanitl rules test https://api.github.com/repos --json
```

| Flag | Bedeutung |
|---|---|
| `URL` | Eine vollständige URL mit Schema, ohne `#fragment`. Die Normalisierung (Punycode, abschließender Punkt, Schreibweise) übernimmt der Daemon. |
| `--method M` | Die Methode der gedachten Anfrage, eine der neun bekannten Methoden, ohne Rücksicht auf Groß- und Kleinschreibung. Vorgabe `GET`. |
| `--upgrade websocket` | Die gedachte Anfrage verlangt ein WebSocket-Upgrade. |
| `--json` | Ein JSON-Objekt mit `verdict`, `matched`, `rule_id`, `origin`, `position` und `passthrough`. |

Die erste Ausgabezeile ist `verdict: allow`, `verdict: block` oder `verdict: ask`,
die zweite nennt die Regel, die entschieden hat, oder sagt, dass keine es tat. Der
Exit-Code ist `0` für allow, `10` für block und `11` für ask oder redact, also
taugt der Befehl für Skripte. Die erzeugte Referenz steht in
[cli.md](cli.md#humanitl-rules-test).

Um zu sehen, auf welche der zuletzt aufgezeichneten Anfragen eine neue Regel gepasst
hätte, bevor du sie hinzufügst, nimm `humanitl rules dry-run` mit denselben Flags
wie `humanitl rules add` (`--action`, `--host`, `--method`, `--path`, `--scheme`,
`--port`, `--upgrade`); siehe [cli.md](cli.md#humanitl-rules-dry-run).

## Rezepte

### Nur das Sprachmodell

Das Profil `llm-only` lässt eine Session ohne Menschen laufen: Nur der eingestellte
LLM-Endpunkt ist erreichbar, alles andere wird ohne Nachfrage blockiert. Starte es
mit `humanitl run --profile llm-only`. Das Profil trägt eine Regel:

```yaml
version: 1
rules:
  - action: block
    match:
      host: "**"
    expires: never
    note: "llm-only profile: block everything that is not the LLM passthrough"
# humanitl rules test https://example.com/
# verdict: block
# humanitl rules test https://registry.npmjs.org/left-pad
# verdict: block
```

Die mitgelieferte Durchleitung zum Modell wird in Rang 1 ausgewertet, vor dieser
Regel, also bleibt das Modell erreichbar. Das Profil setzt außerdem
`hold.ask_mode = "none"`: Eine Anfrage, die keine Regel entscheidet, wird nicht für
einen Menschen angehalten, sondern läuft sofort in die Zeitüberschreitung, und der
Agent bekommt `504`. Die Durchleitung selbst wird aus `llm.endpoint` und
`llm.passthrough_paths` gebaut (siehe
[Die LLM-Durchleitung](#the-llm-passthrough)); um weitere Modellpfade
durchzuleiten, füge sie zu `llm.passthrough_paths` hinzu, statt eine Regel zu
schreiben.

### npm, PyPI und GitHub, nur lesend

Paket-Downloads und Klonen ohne Nachfrage; Veröffentlichen und Pushen werden
weiterhin angehalten:

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "registry.npmjs.org"
      method: [GET, HEAD]
    note: "npm package metadata and tarballs"
  - action: allow
    match:
      host: "pypi.org"
      method: [GET, HEAD]
      path_prefixes: ["/simple/", "/pypi/"]
    note: "PyPI index and JSON API"
  - action: allow
    match:
      host: "files.pythonhosted.org"
      method: [GET, HEAD]
    note: "PyPI package files"
  - action: allow
    match:
      host: "github.com"
      method: [GET, HEAD]
    note: "GitHub pages and git ref discovery"
  - action: allow
    match:
      host: "github.com"
      method: [POST]
      path: "~^/[^/]+/[^/]+/git-upload-pack$"
    note: "git fetch and clone over HTTPS; git-receive-pack (push) stays held"
  - action: allow
    match:
      host: "codeload.github.com"
      method: [GET, HEAD]
    note: "release and branch archives"
# humanitl rules test https://registry.npmjs.org/left-pad
# verdict: allow
# humanitl rules test https://registry.npmjs.org/-/user/org.couchdb.user:demo --method PUT
# verdict: ask
# humanitl rules test https://pypi.org/simple/requests/
# verdict: allow
# humanitl rules test https://pypi.org/account/login/
# verdict: ask
# humanitl rules test https://github.com/demo-org/demo-project.git/info/refs?service=git-upload-pack
# verdict: allow
# humanitl rules test https://github.com/demo-org/demo-project.git/git-upload-pack --method POST
# verdict: allow
# humanitl rules test https://github.com/demo-org/demo-project.git/git-receive-pack --method POST
# verdict: ask
```

Diese Regeln stehen in deinem Rang 3, gehen also auch der mitgelieferten
`ask`-Regel für `registry.npmjs.org` vor.

### Eine Firmen-Registry

Eine Registry im eigenen Netz wird auf eine private Adresse aufgelöst, die der Proxy
zurückweist, außer die Regel sagt `allow_private: true`. Lesen ist erlaubt;
Veröffentlichen bleibt bei einem Menschen, und die zweite Regel öffnet die private
Adresse für die Anfrage, sobald sie freigegeben ist:

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "npm.corp.example"
      method: [GET, HEAD]
      scheme: https
      port: 443
    allow_private: true
    note: "company npm registry, read-only"
  - action: ask
    match:
      host: "npm.corp.example"
      scheme: https
      port: 443
    allow_private: true
    note: "publishing to the company registry needs a human"
# humanitl rules test https://npm.corp.example/@corp%2fui
# verdict: allow
# humanitl rules test https://npm.corp.example/@corp%2fui --method PUT
# verdict: ask
# humanitl rules test http://npm.corp.example/@corp%2fui
# verdict: ask
```

Richte den Paketmanager in der Sandbox wie gewohnt auf die Registry aus (für npm
`registry=https://npm.corp.example/` in der `.npmrc` des Projekts).

### Einen WebSocket erlauben

Ein WebSocket ist eine eigene Entscheidung. Eine Regel nur für den Host deckt ihn
nicht ab; die Regel muss `upgrade: websocket` sagen:

```yaml
version: 1
rules:
  - action: allow
    match:
      host: "realtime.example.com"
      path: "/socket"
      upgrade: websocket
    note: "live updates for the demo-project dashboard"
  - action: allow
    match:
      host: "api.example.com"
# humanitl rules test https://realtime.example.com/socket --upgrade websocket
# verdict: allow
# humanitl rules test https://realtime.example.com/socket
# verdict: ask
# humanitl rules test https://api.example.com/stream --upgrade websocket
# verdict: ask
```

Setz nicht `scheme: wss`; der Handshake reist als `https`-Anfrage (siehe
[`match.scheme`](#matchscheme)).

### Einen gestreamten Upload erlauben

Ein Anfrage-Body über `limits.hold_body_cap_bytes` wird mit `413` blockiert
(`LIMIT_001`), auch wenn eine Regel den Host erlaubt. Um einen solchen Upload
hinauszulassen, braucht eine Regel `stream: true`. Mit `ask` wird der Kopf
angehalten, und der Body fließt erst, nachdem du zugestimmt hast; mit `allow` geht er
sofort. In beiden Fällen wird der Body nicht nach Secrets durchsucht und hat keine
Vorschau.

```yaml
version: 1
rules:
  - action: ask
    match:
      host: "uploads.example.com"
      method: [POST]
      path_prefixes: ["/v1/files"]
    stream: true
    note: "large dataset uploads for demo-project; held, then streamed"
# humanitl rules test https://uploads.example.com/v1/files --method POST
# verdict: ask
# humanitl rules test https://uploads.example.com/v1/files --method PUT
# verdict: ask
```

Die Proben sehen mit und ohne `stream: true` gleich aus; das Flag ändert nur, was mit
einem Body über der Grenze geschieht. Halte die Regel eng (Host, Methode, Pfad) und
zieh `ask` dem `allow` vor. Solange `hold.hard_block_checksum_secrets` an ist, greift
`stream: true` nie, und der Upload wird blockiert. Die beteiligten Grenzen stehen in
[limits.md](limits.md).

## Mitgelieferte Regeln

Humanitl bringt mit dem OpenCode-Adapter zwei Arten von Regeln mit. Beide stehen,
außer der Durchleitung, in Rang 4, lassen sich nicht löschen und tragen im Bildschirm
„Regeln" das Abzeichen „mitgeliefert".

### Die LLM-Durchleitung

Vom Agenten-Adapter aus `llm.endpoint` gebaut, mit der festen ID
`01920000-0000-7000-8000-0000000000ff`. Sie hat keine eigene Datei. Für einen
Endpunkt wie `http://192.168.1.20:11434/v1` entspricht sie:

```yaml
version: 1
rules:
  - id: 01920000-0000-7000-8000-0000000000ff
    action: allow
    match:
      host: "192.168.1.20"
      method: [POST, GET]
      path_prefixes:
        - "/v1/chat/completions?"
        - "/v1/completions?"
        - "/v1/responses?"
        - "/v1/embeddings?"
        - "/v1/models?"
        - "/api/chat?"
        - "/api/generate?"
        - "/api/embed?"
        - "/api/embeddings?"
        - "/api/tags?"
        - "/api/show?"
        - "/api/ps?"
        - "/api/version?"
      scheme: http
      port: 11434
    allow_private: true
    passthrough_llm: true
    note: "LLM passthrough for http://192.168.1.20:11434/v1. Logged, never held."
# humanitl rules test http://192.168.1.20:11434/v1/chat/completions --method POST
# verdict: allow
# humanitl rules test http://192.168.1.20:11434/api/pull --method POST
# verdict: ask
# humanitl rules test http://192.168.1.20:11434/v1/files --method POST
# verdict: ask
```

Gründe: genau ein Host, ein Schema und ein Port, weil das die eine Art von Anfrage
ist, die nicht angehalten wird; nur die Inferenz-Endpunkte als genaue Pfade, damit
Endpunkte, die den Server verändern (Datei-Uploads, Fine-Tuning, Modelle laden und
löschen), weiterhin durch die Warteschlange gehen; `allow_private`, weil ein lokales
Modell in deinem Netz steht. Die Pfade kommen aus `llm.passthrough_paths`, Vorgabe
`["/v1/", "/api/"]`: `/v1/` wird zu den OpenAI-kompatiblen Inferenz-Endpunkten unter
dem Basispfad des Endpunkts erweitert, `/api/` zu den nativen Inferenz-Endpunkten von
Ollama (nur für einen Endpunkt an der Wurzel des Servers), und jeder andere Eintrag
bleibt als Präfix stehen. In einer
von Hand geschriebenen `rules.yaml` liest der Parser `192.168.1.20` als
`ip:192.168.1.20` mit einer Warnung (`RULES_002`); der Adapter baut die Regel
direkt.

### `rules/default.yaml`

Der Zweck dieser Regeln: Beim ersten Start soll niemand eine Flut von Anfragen sehen,
die er nicht ausgelöst hat. Die erste Anfrage, die ein Mensch sieht, soll eine sein,
die er versteht. Die IDs sind fest, damit `disabled_bundled` auf sie zeigen kann.
`bundled: true` steht mit Absicht nicht in der Datei; der Lader setzt es.

<!-- bundled-rules: rules/default.yaml -->
```yaml
version: 1
rules:
  - id: 01920000-0000-7000-8000-000000000001
    action: block
    match: { host: "models.dev" }
    expires: never
    note: "models.dev is the model catalog that the OpenCode documentation names. The installed 1.18.25 uses models.opencode.ai instead (rule ...0009); this rule covers the documented host. Humanitl ships the catalog as a file, so neither call is needed."

  - id: 01920000-0000-7000-8000-000000000002
    action: block
    match: { host: "api.github.com", path: "/repos/anomalyco/opencode/releases/**" }
    expires: never
    note: "Update check. Disabled via OPENCODE_DISABLE_AUTOUPDATE; this rule is the second lock."

  - id: 01920000-0000-7000-8000-000000000003
    action: block
    match: { host: "**.posthog.com" }
    expires: never
    note: "Product analytics endpoint used by several agent CLIs. The string posthog does not appear in the OpenCode 1.18.25 binary at all; this rule is a lock for other agents and for a later version, not a claim about what OpenCode does today."

  - id: 01920000-0000-7000-8000-000000000004
    action: block
    match: { host: "**.sentry.io" }
    expires: never
    note: "Crash reporting would carry stack traces including file paths. OpenCode 1.18.25 bundles a Sentry shim but calls no sentry.io address; this rule is a lock for other agents and for a later version, not a claim about what OpenCode does today."

  - id: 01920000-0000-7000-8000-000000000005
    action: block
    match: { host: "opncd.ai" }
    expires: never
    note: "Session sharing uploads the whole conversation. OpenCode 1.18.25 posts it to opncd.ai (POST /api/share, then /api/share/<id>/sync), and that is the only place the host appears in the binary, so the rule carries no path. What else the host may serve is unknown. Sharing is off by config as well; this rule is the second lock."

  - id: 01920000-0000-7000-8000-000000000006
    action: block
    match: { host: "**.exa.ai" }
    expires: never
    note: "Hosted web search backend for OpenCode's websearch tool."

  - id: 01920000-0000-7000-8000-000000000007
    action: block
    match: { host: "**.parallel.ai" }
    expires: never
    note: "Hosted web search backend (Parallel)."

  - id: 01920000-0000-7000-8000-000000000008
    action: ask
    match: { host: "registry.npmjs.org", method: [GET, HEAD] }
    expires: never
    note: "OpenCode installs provider packages on first use. Shown as a normal held request so the user learns what it is."

  - id: 01920000-0000-7000-8000-000000000009
    action: block
    match: { host: "models.opencode.ai" }
    expires: never
    note: "OpenCode 1.18.25 fetches its model catalog from models.opencode.ai; the documentation and the rule above name models.dev. Both are blocked because Humanitl ships the catalog as a file."

  - id: 01920000-0000-7000-8000-00000000000a
    action: block
    match: { host: "opencode.ai", path: "/share/**" }
    expires: never
    note: "The share links of OpenCode are read under this path. The upload itself goes to opncd.ai in 1.18.25; this rule covers the case that it moves back to the main host."

  # pi (HUM-274). Measured on pi 1.3.0 with a logging proxy; the adapter sets
  # PI_OFFLINE=1, PI_SKIP_VERSION_CHECK=1 and PI_TELEMETRY=0, so these rules
  # are the second lock. Nothing is allowed in addition.
  - id: 01920000-0000-7000-8000-000000000101
    action: block
    match: { host: "pi.dev", path: "/api/**" }
    expires: never
    note: "pi asks pi.dev for its latest version (/api/latest-version) and for a fresh model catalogue (/api/models/providers/<id>) at every interactive start; `pi update` fetches its installer from /api/installer/releases. Switched off by PI_OFFLINE and PI_SKIP_VERSION_CHECK; this rule is the second lock. The documentation pages of pi.dev stay unmatched."

  - id: 01920000-0000-7000-8000-000000000102
    action: block
    match: { host: "radius.pi.dev" }
    expires: never
    note: "Radius, the service of the pi authors: bug report uploads (/bug) and session sharing with sessionShareDestination radius upload the whole conversation. pi 1.3.0 contacts it only on such a command; this rule keeps a session from leaving that way."

  - id: 01920000-0000-7000-8000-000000000103
    action: block
    match: { host: "github.com", path: "/sharkdp/fd/releases/**" }
    expires: never
    note: "pi downloads fd from its GitHub releases at start when no fd or fdfind is installed. Switched off by PI_OFFLINE, which makes pi print a warning and work without fd; this rule is the second lock."

  - id: 01920000-0000-7000-8000-000000000104
    action: block
    match: { host: "github.com", path: "/BurntSushi/ripgrep/releases/**" }
    expires: never
    note: "pi downloads ripgrep from its GitHub releases at start when no rg is installed. Switched off by PI_OFFLINE; this rule is the second lock."
# humanitl rules test https://models.dev/api.json
# verdict: block
# humanitl rules test https://eu.posthog.com/capture
# verdict: block
# humanitl rules test https://posthog.com/
# verdict: block
# humanitl rules test https://opncd.ai/api/share --method POST
# verdict: block
# humanitl rules test https://registry.npmjs.org/@ai-sdk%2fopenai
# verdict: ask
# humanitl rules test https://opencode.ai/docs/
# verdict: ask
# humanitl rules test https://pi.dev/api/latest-version
# verdict: block
# humanitl rules test https://pi.dev/docs/latest
# verdict: ask
# humanitl rules test https://radius.pi.dev/v1/bug --method POST
# verdict: block
# humanitl rules test https://github.com/sharkdp/fd/releases/latest
# verdict: block
# humanitl rules test https://github.com/earendil-works/pi
# verdict: ask
```

| ID (Endung) | Host | Aktion | Grund |
|---|---|---|---|
| `…0001` | `models.dev` | block | Modellkatalog, den die OpenCode-Dokumentation nennt. Humanitl liefert den Katalog als Datei mit. |
| `…0002` | `api.github.com`, Pfad `/repos/anomalyco/opencode/releases/**` | block | Update-Prüfung von OpenCode. Abgeschaltet durch `OPENCODE_DISABLE_AUTOUPDATE`; das hier ist das zweite Schloss. |
| `…0003` | `**.posthog.com` | block | Produktanalyse, die mehrere Agenten-CLIs nutzen; ein Schloss für andere Agenten und spätere Versionen. |
| `…0004` | `**.sentry.io` | block | Absturzberichte würden Stacktraces mit Dateipfaden tragen; ein Schloss für andere Agenten und spätere Versionen. |
| `…0005` | `opncd.ai` | block | Das Teilen einer Session lädt das ganze Gespräch hoch. Teilen ist auch per Konfiguration aus; das hier ist das zweite Schloss. |
| `…0006` | `**.exa.ai` | block | Gehostetes Websuche-Backend des Werkzeugs websearch von OpenCode. |
| `…0007` | `**.parallel.ai` | block | Gehostetes Websuche-Backend (Parallel). |
| `…0008` | `registry.npmjs.org`, `GET`/`HEAD` | ask | OpenCode installiert Provider-Pakete bei der ersten Nutzung; mit Absicht angehalten, damit du erfährst, was das ist. |
| `…0009` | `models.opencode.ai` | block | Der Host des Modellkatalogs, den das installierte OpenCode tatsächlich nutzt. |
| `…000a` | `opencode.ai`, Pfad `/share/**` | block | Links zum Teilen, für den Fall, dass der Upload zurück auf den Haupthost wandert. |
| `…0101` | `pi.dev`, Pfad `/api/**` | block | Versionsprüfung, Modellkatalog und Installer von pi. Abgeschaltet durch `PI_OFFLINE`; das hier ist das zweite Schloss. |
| `…0102` | `radius.pi.dev` | block | Uploads von Fehlerberichten und das Teilen von Sessions über Radius bei pi; sie laden das ganze Gespräch hoch. |
| `…0103` | `github.com`, Pfad `/sharkdp/fd/releases/**` | block | pi lädt `fd` herunter, wenn es fehlt. Abgeschaltet durch `PI_OFFLINE`; das hier ist das zweite Schloss. |
| `…0104` | `github.com`, Pfad `/BurntSushi/ripgrep/releases/**` | block | pi lädt `rg` herunter, wenn es fehlt. Abgeschaltet durch `PI_OFFLINE`; das hier ist das zweite Schloss. |

Die `ask`-Regel `…0008` verhält sich wie gar keine Regel; es gibt sie, damit der
Bildschirm „Regeln" die Anfrage erklären kann. Jede deiner Regeln für
`registry.npmjs.org` geht ihr vor.

## Diagnosen

| Code | Schwere | Wann |
|---|---|---|
| `RULES_001` | Fehler | Die Datei ist kein gültiges YAML, hat einen unbekannten Schlüssel, einen falschen Typ, einen ungültigen Wert für ID, Methode, Schema, Port, Upgrade, Ablauf oder Flow-ID, oder `passthrough_llm` ohne `allow`. |
| `RULES_002` | Warnung | Ein Host-Muster enthält ein Punycode-Label oder ist eine nackte IP-Adresse. |
| `RULES_003` | Fehler | Ein Host-Muster ist ungültig. |
| `RULES_005` | Fehler oder Warnung | Ein Pfad-Glob, regulärer Ausdruck oder Präfix ist ungültig (Fehler); ein Präfix endet auf `?` (Warnung). |
| `RULES_006` | Fehler | `version` fehlt oder ist nicht `1`. |
| `RULES_007` | Fehler | Zwei Regeln teilen sich eine ID. |
| `RULES_008` | Warnung | Eine Regel erlaubt jeden Host ohne jede Bedingung, oder eine Durchleitung ist nicht begrenzt. |
| `RULES_010` | Warnung | `bundled` in einer Datei, oder `disabled_bundled` nennt eine deiner eigenen Regeln. |

Jeder Fehler weist die ganze Datei zurück; die Meldung nennt den Index der Regel, das
Feld und, wo bekannt, die Zeile, zum Beispiel
`rules[3].match.host (line 12): host pattern "*foo.com" is invalid: a wildcard must be a whole label`.
