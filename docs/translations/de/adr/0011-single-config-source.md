---
source: adr/0011-single-config-source.md
source-sha256: 88ae20f1e72d090ad47c390fb6ef2ec3dc1905f90b3519c63c15c04efcb2e9a4
lang: de
---
# ADR-0011 · Eine Konfigurationsquelle, drei Sichtbarkeitsstufen

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Humanitl lässt sich bis in die Sandbox hinein konfigurieren: LLM-Endpunkt,
Agenten-Adapter, Sandbox-Profil, Mounts, Umgebungsvariablen, Zeitlimits,
Obergrenzen, Detektoren, Katalogquellen, Regeln. Zugleich soll der Standardweg
mit drei Entscheidungen auskommen: LLM-Server, Projektordner, Start (Prinzip 8,
„wenig tun müssen, viel tun können").

Diese beiden Anforderungen stoßen auf zwei Arten zusammen. Erstens droht die
klassische Doppelpflege: Ein Wert steht in einer TOML-Datei, als CLI-Flag und
als Kontrollkästchen im Einstellungsdialog, mit drei verschiedenen Namen, drei
Beschreibungen und einem Vorgabewert, der an zwei Stellen abweicht. Zweitens
droht Überladung: Ein Einstellungsdialog mit achtzig Feldern hilft niemandem,
auch wenn jedes einzelne Feld nötig ist.

Dazu kommt eine dritte Anforderung aus Prinzip 10: Die Oberfläche darf nie der
einzige Weg sein. Was sich in der Oberfläche einstellen lässt, muss sich auch in
einer Datei und auf der Kommandozeile einstellen lassen, sonst ist der Betrieb
ohne Oberfläche (ADR-0013) eine Teilmenge statt einer Alternative.

## Entscheidung

**Für jede Einstellung gibt es genau eine Quelle: einen Rust-Typ.** Alle
Einstellungen sind `serde`-Typen mit einer `schemars`-Ableitung im Crate
`humanitl-config`, mit `Config` als Wurzel. Daraus entstehen, erzeugt und nicht
parallel gepflegt:

- das TOML-Schema und seine Validierung,
- die CLI-Flags (`clap`, mit denselben Namen),
- die Einstellungsoberfläche in der App (aus dem JSON-Schema, mit
  Beschreibung, Vorgabewert und Zurücksetzen),
- die Dokumentation.

**Keine Einstellung gibt es nur in der Oberfläche.**

**Drei Sichtbarkeitsstufen.** Jedes Feld trägt ein Attribut
`#[humanitl(tier = "basic" | "advanced" | "expert")]`:

| Stufe | Ort | Verhalten |
|---|---|---|
| `basic` | Einrichtungs-Checkliste | die drei Entscheidungen des Standardwegs |
| `advanced` | Einstellungsbildschirm | standardmäßig sichtbar |
| `expert` | Einstellungsbildschirm | eingeklappt, mit Warnung, wo sicherheitsrelevant |

Die Suche erfasst immer alle Stufen; nichts ist unauffindbar, nur standardmäßig
eingeklappt.

**Vorrang, von niedrig nach hoch:** eingebaute Vorgabewerte → globale
`config.toml` → globales Profil → Projekt-Profil → Umgebungsvariablen
`HUMANITL_*` (Pfadtrenner `__`, zum Beispiel `HUMANITL_HOLD__TIMEOUT_SECS`) →
CLI-Flag. Jede Auflösung hält je Feld die **Herkunft** (`Origin`) fest, damit die
Oberfläche „dieser Wert kommt aus dem Projekt-Profil" anzeigen kann.

**Profile bündeln** Sandbox-Profil, Regelsatz, Agenten-Adapter, LLM-Endpunkt,
Zeitlimit, Mounts und Umgebung in einer Datei. Sie liegen global unter
`$XDG_CONFIG_HOME/humanitl/profiles/` oder je Projekt unter
`.humanitl/profile.toml`; das Projekt gewinnt. `humanitl run --profile llm-only`
und die Oberfläche nutzen dieselben Profildateien.

**Regeln haben zwei Speicherorte** (ADR-0007): gespeichert in `rules.yaml`
(übersteht den Neustart) und temporär mit `expires: session` (nur im Speicher
des Daemons). Die Oberfläche zeigt beide in getrennten Tabs; temporäre Regeln
haben eine verbleibende Lebensdauer und eine Aktion „Dauerhaft machen".

## Begründung

Aus einem Typ zu erzeugen, statt drei Darstellungen zu pflegen, ist der einzige
Weg, der mit der Zeit nicht auseinanderläuft. Ein neues Feld erscheint
automatisch im Schema, in der CLI, in der Oberfläche und in der Dokumentation,
und fehlt die Beschreibung, fällt das beim Erzeugen auf, nicht beim Nutzer. Die
Definition of Done verlangt für jedes neue Feld Stufe, Beschreibung und
Vorgabewert; das macht Vollständigkeit prüfbar, statt sie zu erhoffen.

Die drei Stufen lösen die Überladung, ohne etwas zu verstecken. Schrittweise
Offenlegung heißt hier: Der Standardweg zeigt drei Felder, der
Einstellungsbildschirm zeigt die nützlichen, und die gefährlichen sind einen
Klick entfernt und tragen eine Warnung. Weil die Suche alle Stufen erfasst, muss
niemand raten, ob es eine Einstellung gibt.

Die Herkunft je Feld ist die Antwort auf die häufigste Verwirrung bei
geschichteter Konfiguration: „Aber das habe ich doch eingestellt, warum gilt es
nicht?" Wenn die Oberfläche sagt, dass der Wert aus dem Projekt-Profil kommt und
den globalen überschreibt, beantwortet sich die Frage von selbst.

Profile sind die Einheit, in der Menschen tatsächlich denken: nicht „Zeitlimit
300 Sekunden und `/work` als `ro` einhängen", sondern „mein Setup für
Kundenprojekte". Dass ein Profil eine Datei ist, macht es teilbar,
versionierbar und im Fehlerfall lesbar.

Die Trennung von gespeicherten und temporären Regeln bildet die beiden echten
Arten von Vertrauen ab: „das soll immer gelten" und „das gilt gerade, weil ich
hier arbeite". Ohne diese Trennung sammelt eine `rules.yaml` binnen zwei Wochen
dreißig Einträge, die niemand mehr überblickt, und jeder davon ist eine
dauerhaft offene Tür.

## Verworfene Alternativen

- **Konfiguration nur in der Oberfläche, Datei als Implementierungsdetail.** Der
  bequemste Weg für die Entwicklung der Oberfläche und ein Bruch mit Prinzip 10:
  Betrieb ohne Oberfläche, Skripte und Versionierung der Konfiguration wären
  ausgeschlossen.
- **Konfiguration nur in Dateien, kein Einstellungsbildschirm.** Für das
  Zielpublikum unzumutbar (keine Sicherheitsexperten, kein Terminalzwang).
- **Handgeschriebene CLI-Flags neben einer handgeschriebenen Oberfläche.** Der
  Normalfall in vielen Projekten und der Grund, warum Vorgabewerte dort
  auseinanderlaufen. Ein Generator kostet einmal Aufwand und danach nichts.
- **JSON oder YAML statt TOML für die Hauptkonfiguration.** JSON kennt keine
  Kommentare, was für eine von Hand bearbeitete Datei ein Ausschlussgrund ist.
  YAML hat bekannte Fallstricke mit impliziten Typen. TOML ist für flache bis
  mittlere Verschachtelung die klarste Wahl. Regeln bleiben YAML, weil sie eine
  geordnete Liste mit Verschachtelung sind (ADR-0007).
- **Nur zwei Stufen (einfach/erweitert).** Hätte die sicherheitsrelevanten
  Einstellungen mit den bloß selten genutzten vermischt. Die dritte Stufe gibt
  es, damit eine Warnung eine Bedeutung hat.
- **Eine flache Einstellungsliste ohne Profile.** Hätte den häufigsten
  Anwendungsfall („anderes Projekt, andere Regeln") auf das Ändern von sechs
  Werten von Hand reduziert.
- **Nur dauerhafte Regeln.** Wäre einfacher, aber jede spontane Freigabe wäre
  dauerhaft. Genau das ist der schleichende Kontrollverlust, den das Werkzeug
  verhindern soll.

## Folgen

- `humanitl-config` gehört zum Kern: `serde` und `schemars`, kein IO im Typ
  selbst, tabellengetriebene Tests für den Vorrang.
- Jedes Feld braucht `#[schemars(description = "…")]` und ein Stufen-Attribut.
  Ohne beides ist ein Feld unvollständig und wird im Review beanstandet.
- `humanitl config schema` gibt das JSON-Schema aus. Der Einstellungsbildschirm
  wird daraus erzeugt; er enthält keine Feldliste im Dart-Code.
- Die Gruppe `limits` ist die Heimat aller Obergrenzen und Zeitlimits
  (`backlog/CONVENTIONS.md` 4.4); ältere Schlüssel wie `hold.body_cap_bytes`
  bleiben als Aliase erhalten, damit bestehende Dateien weiter funktionieren.
- `humanitl config set` validiert gegen das Schema und liefert bei einem Fehler
  ein `Diagnostic` statt einer Fehlermeldung (ADR-0012).
- Live-Nachladen der Konfigurationsdatei ist möglich, weil die Auflösung eine
  reine Funktion über die Schichten ist.
- Die erzeugte Oberfläche ist für achtzig Felder gut genug, aber nicht für
  Sonderfälle. Wo ein Feld eine eigene Interaktion braucht (LLM-Server suchen,
  Projektordner wählen), gibt es ein handgebautes Widget für dieses Feld, nicht
  für den ganzen Bildschirm.

## Betroffene Issues

`HUM-062` (Config-Crate mit Schema, Stufen und Tests für den Vorrang), `HUM-066`
(Profile, global und je Projekt), `HUM-069` (Einstellungsbildschirm mit
schrittweiser Offenlegung), `HUM-070` (`humanitl config get|set|schema|edit`),
`HUM-033` (Regel-Bildschirm mit Tabs für gespeicherte und temporäre Regeln).
