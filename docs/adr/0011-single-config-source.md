# ADR-0011 · Eine Konfigurationsquelle, drei Sichtbarkeitsstufen
Status: Accepted
Datum: 2026-09-02

## Kontext

Humanitl ist konfigurierbar bis in die Sandbox hinein: LLM-Endpunkt,
Agent-Adapter, Sandbox-Profil, Mounts, Umgebungsvariablen, Timeouts, Caps,
Detektoren, Katalogquellen, Regeln. Gleichzeitig soll der Standardweg drei
Entscheidungen brauchen: LLM-Server, Projektordner, Start (Prinzip 8, „wenig tun
müssen, viel tun können").

Diese beiden Anforderungen kollidieren auf zwei Arten. Erstens droht die
klassische Doppelpflege: Ein Wert existiert in einer TOML-Datei, als CLI-Flag und
als Häkchen im Einstellungsdialog, mit drei verschiedenen Namen, drei
Beschreibungen und einem Default, der an zwei Stellen abweicht. Zweitens droht
die Überforderung: Ein Einstellungsdialog mit achtzig Feldern hilft niemandem,
auch wenn jedes einzelne Feld nötig ist.

Dazu kommt eine dritte Anforderung aus Prinzip 10: Das UI darf nie der einzige
Weg sein. Was im UI einstellbar ist, muss auch in einer Datei und auf der
Kommandozeile einstellbar sein — sonst ist der Headless-Betrieb (ADR-0013) eine
Teilmenge statt einer Alternative.

## Entscheidung

**Es gibt genau eine Quelle für jede Einstellung: einen Rust-Typ.** Alle
Einstellungen sind `serde`-Typen mit `schemars`-Ableitung in der Crate
`humanitl-config`, mit `Config` als Wurzel. Daraus entstehen — generiert, nicht
parallel gepflegt:

- das TOML-Schema und dessen Validierung,
- die CLI-Flags (`clap`, mit denselben Namen),
- die Einstellungsoberfläche im UI (aus dem JSON-Schema, mit Beschreibung,
  Default und Reset),
- die Dokumentation.

**Kein Setting existiert nur im UI.**

**Drei Sichtbarkeitsstufen.** Jedes Feld trägt ein Attribut
`#[humanitl(tier = "basic" | "advanced" | "expert")]`:

| Stufe | Ort | Verhalten |
|---|---|---|
| `basic` | Setup-Checkliste | die drei Entscheidungen des Standardwegs |
| `advanced` | Einstellungsbildschirm | standardmäßig sichtbar |
| `expert` | Einstellungsbildschirm | eingeklappt, mit Warnhinweis bei Sicherheitsrelevanz |

Die Suche geht immer über alle Stufen; nichts ist unauffindbar, nur
standardmäßig eingeklappt.

**Präzedenz, von niedrig nach hoch:** eingebaute Defaults → globale
`config.toml` → globales Profil → Projekt-Profil → Umgebungsvariablen
`HUMANITL_*` (Pfadtrennung mit `__`, etwa `HUMANITL_HOLD__TIMEOUT_SECS`) →
CLI-Flag. Jede Auflösung merkt sich pro Feld die **Herkunft** (`Origin`), damit
das UI „dieser Wert kommt aus dem Projekt-Profil" anzeigen kann.

**Profile bündeln** Sandbox-Profil, Regelsatz, Agent-Adapter, LLM-Endpunkt,
Timeout, Mounts und Umgebung zu einer Datei. Sie liegen global unter
`$XDG_CONFIG_HOME/humanitl/profiles/` oder pro Projekt unter
`.humanitl/profile.toml`; das Projekt gewinnt. `humanitl run --profile llm-only`
und das UI benutzen dieselben Profildateien.

**Regeln haben zwei Ablageorte** (ADR-0007): gespeichert in `rules.yaml`
(überlebt den Neustart) und temporär mit `expires: session` (nur im Speicher des
Daemons). Das UI zeigt beide in getrennten Tabs; temporäre Regeln haben eine
Restlaufzeit und eine Aktion „dauerhaft machen".

## Begründung

Aus einem Typ zu generieren statt drei Repräsentationen zu pflegen, ist der
einzige Weg, der auf Dauer nicht auseinanderläuft. Ein neues Feld erscheint
automatisch im Schema, in der CLI, im UI und in der Dokumentation — und wenn die
Beschreibung fehlt, fällt das beim Erzeugen auf, nicht beim Nutzer. Die Definition
of Done verlangt für jedes neue Feld Stufe, Beschreibung und Default; damit ist
die Vollständigkeit prüfbar statt erhofft.

Die drei Stufen lösen die Überforderung, ohne etwas zu verstecken. Progressive
Disclosure heißt hier: Der Standardweg zeigt drei Felder, der
Einstellungsbildschirm zeigt die nützlichen, und die gefährlichen sind einen
Klick entfernt und mit einem Warnhinweis versehen. Weil die Suche über alle
Stufen geht, muss niemand raten, ob eine Einstellung existiert.

Die Herkunft pro Feld ist die Antwort auf die häufigste Verwirrung bei
geschichteter Konfiguration: „Ich habe das doch eingestellt, warum gilt es
nicht?" Wenn das UI sagt, dass der Wert aus dem Projekt-Profil kommt und den
globalen überschreibt, erledigt sich die Frage selbst.

Profile sind die Einheit, in der Menschen tatsächlich denken: nicht „Timeout 300
Sekunden und Mount `/work` als `ro`", sondern „mein Setup für Kundenprojekte".
Dass ein Profil eine Datei ist, macht es teilbar, versionierbar und im
Fehlerfall lesbar.

Die Trennung von gespeicherten und temporären Regeln bildet die zwei realen
Vertrauensarten ab: „das soll immer gelten" und „das gilt jetzt gerade, weil ich
hier arbeite". Ohne diese Trennung sammelt eine `rules.yaml` innerhalb von zwei
Wochen dreißig Einträge, die niemand mehr überblickt — und jede davon ist eine
dauerhaft offene Tür.

## Verworfene Alternativen

- **Konfiguration nur im UI, Datei als Implementierungsdetail.** Der bequemste
  Weg für die UI-Entwicklung und ein Bruch mit Prinzip 10: Headless-Betrieb,
  Skripting und Versionierung der Konfiguration wären ausgeschlossen.
- **Konfiguration nur in Dateien, kein Einstellungsbildschirm.** Für die
  Zielgruppe (keine Security-Experten, kein Terminal-Zwang) unzumutbar.
- **Handgeschriebene CLI-Flags neben handgeschriebenem UI.** Der Normalfall in
  vielen Projekten und der Grund, warum Defaults dort auseinanderlaufen. Ein
  Generator kostet einmal Aufwand und danach nichts.
- **JSON oder YAML statt TOML für die Hauptkonfiguration.** JSON kennt keine
  Kommentare, was für eine handbearbeitete Datei disqualifizierend ist. YAML hat
  bekannte Fußangeln bei impliziten Typen. TOML ist für flache bis mittlere
  Verschachtelung die klarste Wahl. Regeln bleiben YAML, weil sie eine geordnete
  Liste mit Verschachtelung sind (ADR-0007).
- **Nur zwei Stufen (einfach/erweitert).** Hätte die sicherheitsrelevanten
  Einstellungen mit den bloß selten benutzten vermischt. Die dritte Stufe
  existiert, damit ein Warnhinweis eine Bedeutung hat.
- **Eine flache Einstellungsliste ohne Profile.** Hätte den häufigsten
  Anwendungsfall („anderes Projekt, andere Regeln") auf manuelles Umstellen von
  sechs Werten reduziert.
- **Regeln nur persistent.** Wäre einfacher, aber jede Ad-hoc-Freigabe wäre
  dauerhaft. Das ist genau der schleichende Verlust von Kontrolle, den das
  Werkzeug verhindern soll.

## Konsequenzen

- `humanitl-config` gehört zum Kern: `serde` und `schemars`, kein IO im Typ
  selbst, tabellengetriebene Tests für die Präzedenz.
- Jedes Feld braucht `#[schemars(description = "…")]` und ein Tier-Attribut. Ohne
  beides ist ein Feld unvollständig und wird im Review beanstandet.
- `humanitl config schema` gibt das JSON-Schema aus. Der Einstellungsbildschirm
  wird daraus erzeugt; er enthält keine Feldliste im Dart-Code.
- Die Gruppe `limits` ist die Heimat aller Caps und Timeouts
  (`backlog/CONVENTIONS.md` 4.4); ältere Schlüssel wie `hold.body_cap_bytes`
  bleiben als Alias erhalten, damit vorhandene Dateien weiter funktionieren.
- `humanitl config set` validiert gegen das Schema und liefert bei einem Fehler
  ein `Diagnostic` statt einer Fehlermeldung (ADR-0012).
- Live-Reload der Konfigurationsdatei ist möglich, weil die Auflösung eine reine
  Funktion über die Schichten ist.
- Die generierte Oberfläche ist gut genug für achtzig Felder, aber nicht für
  Sonderfälle. Wo ein Feld eine eigene Interaktion braucht (LLM-Server suchen,
  Projektordner wählen), gibt es ein handgebautes Widget für dieses Feld — nicht
  für den ganzen Bildschirm.

## Betroffene Issues

`HUM-062` (config-Crate mit Schema, Stufen und Präzedenz-Tests), `HUM-066`
(Profile, global und pro Projekt), `HUM-069` (Einstellungsbildschirm mit
Progressive Disclosure), `HUM-070` (`humanitl config get|set|schema|edit`),
`HUM-033` (Rules-Screen mit Tabs für gespeicherte und temporäre Regeln).
