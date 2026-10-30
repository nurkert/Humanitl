---
source: adr/0009-ui-stack.md
source-sha256: cb8843ac3e64d1c6e7c0c8b380a08c85af1c000e77d65f69b616cd24b96ac234
lang: de
---
# ADR-0009 · UI-Stack: Flutter mit eigener Wrapper-Schicht `packages/ui`

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02, revised twice on 2026-09-04 (HUM-035, then the project owner)

## Kontext

Die Oberfläche von Humanitl ist ein Leitstand, kein Formular. Sie zeigt eine
Warteschlange angehaltener Anfragen mit laufenden Countdowns, einen
virtualisierten Verlauf über Zehntausende Zeilen, einen Diff-Editor für die
Pseudonymisierung, ein eingebettetes Terminal, JSON-Bäume, Hex-Ansichten und
einen Bereich mit laufenden Isolationsprüfungen. Das ist deutlich mehr, als eine
Widget-Bibliothek für Geschäftsanwendungen abdeckt.

Zugleich soll das Ergebnis ruhig aussehen und nicht wie ein Hackerwerkzeug
(Grundsatz 4): Farbe bedeutet Zustand, nie Dekoration; Entscheidungen fallen
direkt an Ort und Stelle und nie in einem Modal. Zielplattform ist der
Linux-Desktop unter X11 und Wayland, ausgeliefert als `.deb` und AppImage.

Zwei Risiken sind von Anfang an sichtbar: Eine junge Widget-Bibliothek kann
brechen oder ungepflegt liegen bleiben, und eine falsche Wahl in Sprint 0 wäre
teuer, wenn sie sich erst im dritten Sprint zeigt.

## Entscheidung

> **Am 2026-09-04 zweimal überarbeitet.** HUM-035 hatte diesen Abschnitt ersetzt
> und `shadcn_flutter` verworfen; der Projektinhaber hat das am selben Tag
> rückgängig gemacht. Es gilt der Abschnitt „Am 2026-09-04 vom Projektinhaber
> überarbeitet" ganz unten: `shadcn_flutter` wird übernommen, festgelegt auf
> 0.0.54 und nur in `app/packages/ui`. Alles andere in diesem Abschnitt gilt
> unverändert. Der ursprüngliche Wortlaut bleibt stehen, damit sich die Kette
> nachvollziehen lässt, auch der letzte Absatz, der die Überprüfung am Ende von
> Sprint 2 ankündigt: Sie hat stattgefunden, zweimal.

Die Oberfläche ist Flutter (Desktop/GTK). Für den Rahmen, also veränderbare
Bereiche, Befehlspalette, Sheet, Toast, ContextMenu und Menüleiste, wird
[shadcn_flutter](https://pub.dev/packages/shadcn_flutter) verwendet, mit einer
**exakt festgelegten Version** und **vollständig gekapselt hinter dem lokalen
Paket `app/packages/ui`**. Kein Feature importiert `shadcn_flutter` direkt; alle
Bildschirme nutzen die Wrapper `HButton`, `HPill`, `HBadge`, `HPanel`, `HRow`,
`HModal`, `HSheet` und die Design-Tokens `HTokens`. Ein Upgrade der Bibliothek
ist ein eigenes, geplantes Vorhaben: ein Upgrade-Tag je Sprint.

Die datenlastigen Widgets kommen ausdrücklich **nicht** aus shadcn, sondern aus
spezialisierten Paketen:

| Aufgabe | Paket |
|---|---|
| Virtualisierte Tabelle, Baum | `two_dimensional_scrollables` |
| Code- und Body-Editor | `re_editor` |
| Terminal | `xterm2` |
| Diff für den Pseudonymisierungs-Editor | `diff_match_patch` |

Zustand: `flutter_riverpod` 3 mit Generator (`riverpod_annotation`,
`riverpod_generator`), Modelle mit `freezed` 4, `FlowEvent` als Sealed Class.
Daten fließen in eine Richtung: Ereignisstrom, Provider, Widget. Widgets rufen
den `DaemonClient` nur über Provider-Methoden auf, nie direkt.

**Kein WebView unter Linux.** Eine Domain-Vorschau ist entweder eine Karte aus
dem mitgelieferten Katalog oder ein Bild, das der Daemon liefert.

**Ein Fenster mit angedockten Bereichen**, kein Mehrfenster-Betrieb. Mehrere
Fenster sind in Flutter noch experimentell.

Die Wahl von shadcn_flutter gegenüber [forui](https://pub.dev/packages/forui)
wird **am Ende von Sprint 2 überprüft** (HUM-035), sobald der erste echte
Anhalten-Bildschirm steht: bestätigen oder wechseln.

## Begründung

> **Am 2026-09-04 zweimal überarbeitet.** Die Absätze zu `shadcn_flutter` und zur
> Überprüfung am Ende von Sprint 2 sind beantwortet. Die Begründung für den
> heutigen Stand steht im Abschnitt „Am 2026-09-04 vom Projektinhaber
> überarbeitet"; der Abschnitt „Entscheidung nach Sprint 2" ist Geschichte.
> Alles andere gilt unverändert.

Für den Linux-Desktop liefert Flutter ein vollständiges Rendering ohne einen Zoo
von GTK-Widgets, eine sehr gute Animationsschicht (Ankunfts- und
Abgangsanimationen der Karten in der Warteschlange gehören zum Design) und ein
Werkzeug für Golden-Tests, mit dem sich ein visuelles Konzept gegen Regressionen
absichern lässt. Ein Fenster mit angedockten Bereichen lässt sich ohne Kampf
bauen.

shadcn_flutter deckt genau die Rahmenelemente ab, die man sonst selbst und
schlechter bauen würde: Befehlspalette, Resizable, Sheet, Toast, Menüleiste. Der
Wert liegt in der Einheitlichkeit dieser Elemente untereinander, nicht in
einzelnen Widgets.

Die Kapselung hinter `packages/ui` ist der eigentliche Kern dieser Entscheidung.
Sie kostet einen Arbeitstag und macht die Wahl der Bibliothek umkehrbar: Ein
Wechsel zu forui, zu einer anderen Bibliothek oder zu eigenen Widgets berührt
dann ein Paket, nicht acht Features. Genau deshalb ist die Überprüfung am Ende
von Sprint 2 (HUM-035) ein billiges Versprechen und keine leere Geste.

Die datenlastigen Widgets aus spezialisierten Paketen zu nehmen, folgt derselben
Logik wie ADR-0001: nichts Eigenes, wo es Bewährtes gibt, aber auch keine
Verrenkungen, um alles aus einer Quelle zu bekommen. Eine virtualisierte Tabelle
über Zehntausende Zeilen ist ein Problem für sich und wird von einem Paket
gelöst, das nur das tut.

Kein WebView, weil es unter Linux die größte Abhängigkeit und die größte
Angriffsfläche des ganzen Programms wäre, und weil die Oberfläche genau die
Seite rendern würde, über die der Nutzer gerade entscheidet. Das ist dieselbe
Überlegung wie in ADR-0006 zur automatischen Domain-Vorschau.

riverpod mit Generator statt handgeschriebener Provider, weil die Namen der
Provider verbindlich festgelegt sind (`backlog/CONVENTIONS.md` 3.9) und der
Generator Tippfehler und vergessene Invalidierungen zu Compilerfehlern macht.
freezed für Modelle, weil die Dart-Seite ein Spiegel der Rust-Kerntypen ist und
Wertsemantik plus erschöpfendes Pattern Matching braucht.

## Verworfene Alternativen

> **Am 2026-09-04 zweimal überarbeitet.** Der Punkt „forui statt
> shadcn_flutter" ist entschieden: `shadcn_flutter`. Die Liste bleibt als
> Geschichte stehen; maßgeblich ist der Abschnitt „Am 2026-09-04 vom
> Projektinhaber überarbeitet".

- **GTK4 direkt (Rust `gtk4-rs` oder Vala).** Nativ, leichtgewichtig, gute
  Systemintegration. Verliert bei der Zahl spezialisierter Widgets:
  virtualisierte Tabelle, Diff-Editor, Terminal und Animationen wären großteils
  Eigenbau, und Golden-Tests fehlen.
- **Tauri oder Electron.** Bringt eine Browser-Engine als Abhängigkeit mit,
  genau die Angriffsfläche, die der Verzicht auf das WebView vermeidet. Für ein
  Sicherheitswerkzeug die falsche Richtung, unabhängig von der Paketgröße.
- **Qt/QML.** Technisch machbar, aber Lizenz- und Paketierungsfragen und ein
  drittes Ökosystem neben Rust und Dart.
- **forui statt shadcn_flutter.** Ernsthaft erwogen und nicht ausgeschlossen:
  Die Entscheidung wird am Ende von Sprint 2 mit einem echten Bildschirm als
  Beleg überprüft. Die Wrapper-Schicht macht den Wechsel lokal.
- **Alle Widgets selbst bauen.** Maximale Kontrolle, aber der Aufwand fließt in
  den Rahmen statt in die Fachlichkeit. Widerspricht Grundsatz 2.
- **Mehrere Fenster (Warteschlange und Verlauf getrennt).** Wäre angenehm für
  den Arbeitsablauf, ist in Flutter aber noch experimentell. Verschoben, nicht
  verworfen.
- **Terminal als externes Fenster.** Hätte `xterm2` erspart, aber die
  Statuszeile „eine Anfrage wird angehalten" im Terminal des Agenten (ADR-0014)
  und das Mitlesen in der Oberfläche wären dann nicht möglich.

## Folgen

> **Am 2026-09-04 zweimal überarbeitet.** Die vier Punkte, die `shadcn_flutter`
> betreffen (Festlegung, Upgrade-Tag, Importverbot, Restrisiko), gelten wieder,
> in der Fassung des Abschnitts „Am 2026-09-04 vom Projektinhaber
> überarbeitet": festgelegt auf 0.0.54, Import nur in `app/packages/ui`,
> erzwungen von `tools/check-deps.sh`. Ein Nachfolge-ADR für forui entfällt,
> weil forui nicht gewählt wurde. Goldens, die Anbindung an `DaemonClient` und
> `HTokens` gelten unverändert.

- `app/packages/ui` ist verbindlich: Ein direkter Import von `shadcn_flutter` in
  einem Feature ist ein Architekturverstoß und wird im Review beanstandet.
- Die Version von shadcn_flutter ist exakt festgelegt (zum Beispiel `0.0.54`);
  ein Upgrade ist ein eigener, getesteter Schritt je Sprint, keine Nebenwirkung
  eines `pub upgrade`.
- Golden-Tests (`alchemist`) sichern die visuellen Zustände: Zeile der
  Warteschlange in drei Zuständen, Anfragekarte, Aktionsleiste, Domain-Bereich
  bekannt und unbekannt, Isolationsbereich, und zwar je Sprache, weil DE und EN
  unterschiedlich lange Texte haben.
- Jeder Bildschirm wird gegen die Schnittstelle `DaemonClient` gebaut, nicht
  gegen gRPC. Widget-Tests laufen gegen `FakeDaemonClient`, ohne Daemon.
- Die Design-Tokens (`HTokens`) sind Dart-Konstanten aus `BACKLOG.md` 5;
  Zustandsfarben kommen ausschließlich über `FlowStateColor.of(state)`, damit
  Farbe nie dekorativ eingesetzt wird.
- HUM-035 ist ein echtes Entscheidungs-Issue mit zwei möglichen Ausgängen. Fällt
  es zugunsten von forui aus, wird dieses ADR nicht überschrieben, sondern durch
  ein Nachfolge-ADR ersetzt (Status dann `Superseded by`).
- Ein Restrisiko bleibt: shadcn_flutter ist jung. Der Puffer in HUM-061 ist
  ausdrücklich auch für Brüche in shadcn reserviert.

## Entscheidung nach Sprint 2 (2026-09-04)

> **Dieser Abschnitt ist Geschichte.** Er hält fest, wie HUM-035 entschieden hat
> und woran es gemessen hat. Sein Ergebnis, keine Bibliothek, gilt nicht mehr;
> der Projektinhaber hat es am selben Tag rückgängig gemacht. Maßgeblich ist der
> Abschnitt „Am 2026-09-04 vom Projektinhaber überarbeitet". Die Messungen hier
> bleiben gültig und lesenswert, weil sie beschreiben, was die Bibliothek
> kostet.

HUM-035 sollte die Wahl zwischen `shadcn_flutter` und forui bestätigen oder
revidieren. Beim Nachsehen war die Lage eine andere als die, für die das Issue
geschrieben war: `shadcn_flutter` stand nie in `app/pubspec.yaml`.
`app/packages/ui` steht seit Sprint 0 auf schlichtem
`package:flutter/widgets.dart` und ist in Sprint 1 und 2 zu einer eigenen
Schicht gewachsen. Drei Bildschirme laufen darauf: Anhalten mit Aktionsleiste,
Regeln, Verlauf. Die Frage lautet also nicht mehr „shadcn oder forui", sondern
„bleiben, `shadcn_flutter` übernehmen oder forui übernehmen".

**Entscheidung: Wir bleiben bei unserer eigenen Wrapper-Schicht `app/packages/ui`
auf `package:flutter/widgets.dart` und übernehmen weder `shadcn_flutter` noch
forui.** Die Kapselungsregel aus dem Abschnitt „Entscheidung" gilt unverändert
weiter; nur der Name der Bibliothek hinter dem Wrapper fällt weg.

### Begründung

Ein Wechsel kostet 8,5 bis 11,5 Personentage und spart 3,5 Tage Eigenbau. Beide
Bibliotheken verlangen an ihrer Spitze Flutter 3.47.0, unsere Festlegung steht
auf 3.44.0; eine Übernahme begänne also mit einem Upgrade, nach dem alle 49
Goldens neu freigegeben werden müssen. In den letzten sechs Releases gibt es 18
Breaking-Einträge für `shadcn_flutter` und 27 für forui, die meisten davon auf
unserer Oberfläche. Unsere eigene Schicht hat null Workarounds, null Overrides in
`HTokens`, rund 30 Kontrast-Zusicherungen und rund 200 Tastenverweise; sie
erreicht 88,3 % der gewichteten Punkte, `shadcn_flutter` 48,3 % und forui
51,7 %. Was fehlt, also ContextMenu, Datums- und Zeitauswahl und vertikales
Resizable, sind drei Widgets, keine Bibliothek. Die Kapselung bleibt, damit die
Entscheidung umkehrbar bleibt.

### Ohne Prototyp entschieden

Das Issue sah einen Branch `spike/forui` vor, Zeitrahmen ein Tag, und ein
Akzeptanzkriterium dafür. Der Branch wurde nicht gebaut; das ist eine bewusste
Abweichung, festgehalten in `backlog/CONVENTIONS.md` 4.20 und in
`backlog/sprint-2.md` unter HUM-035 angeglichen. Der Grund ist die eine
Bedingung, die ohne Prototyp feststeht und die ein Prototyp auch nicht geändert
hätte: Beide Bibliotheken verlangen an ihrer Spitze Flutter 3.47.0, die
Festlegung steht auf 3.44.0. Der Zeitrahmen wäre vom Upgrade aufgebraucht
worden, bevor die erste Zeile der Portierung geschrieben war.

**Was das kostet.** Vier Kriterien, nämlich 2 (Fehler), 4 (Theming), 5
(Tastatur und Fokus) und 6 (Performance), zusammen 11 der 20 Gewichtspunkte,
bleiben für beide Bibliotheken eine Schätzung statt einer Messung. Der Abschnitt
„Wie belastbar das Ergebnis ist" rechnet aus, wie weit sie das Ergebnis tragen
könnten.

**Woran sich eine Fehleinschätzung zeigen würde.** An drei Beobachtungen, jede
ohne neuen Aufwand: Der Eigenbau braucht in einem Sprint mehr als drei neue
Rahmenelemente auf einmal; die Pflege der Schicht kostet, über einen Sprint
gemessen, mehr als ein Fünftel der UI-Zeit; oder eine der beiden Bibliotheken
erreicht 1.0 mit einem Stabilitätsversprechen und hebt damit Kriterium 3 auf.
Tritt eines davon ein, ist der Spike doch zu machen, und dann misst er genau die
vier Kriterien oben.

### Woher die Zahlen kommen

Alle Zahlen aus dem Repository wurden am 2026-09-04 im Arbeitsbaum gemessen, auf
dem Stand von Commit `2c26ad9` plus der noch nicht committeten Arbeit an
Sprint 2 (die drei Bildschirme und `packages/ui` sind zu diesem Zeitpunkt
Arbeitsbaum, kein Commit; ein reiner Commit-Anker hätte sie nicht erfasst).
An diesem Baum wird parallel gebaut, und die Zählungen haben sich innerhalb eines
Tages um einige Prozent bewegt. Wo sich eine Größe bewegt, steht sie deshalb als
gerundete Größenordnung da; exakt ist nur, was ein abzählbarer Bestand ist.
Neben jeder Größe steht, wie sie gezählt wird:

- **Zeilen:** `wc -l` über `*.dart`, ohne `.dart_tool`.
- **Widgets:** Klassen in `app/packages/ui/lib/src/widgets/`, die in Spalte 0 mit
  `class H` beginnen, ohne die reine Datenklasse `HSegmentOption`.
- **Nutzung je Bildschirm:** Vorkommen des Bezeichners mit Wortgrenze in
  `app/lib/features/<screen>/`, verschiedene Namen gezählt, nicht Aufrufe.
- **Goldens:** PNG unter `app/test/goldens/goldens/ci`. Das Verzeichnis
  `app/test/goldens/failures/` steht in `app/.gitignore` und enthält Artefakte
  fehlgeschlagener Läufe; es zählt nicht.
- **Testfälle:** Vorkommen von `test(` und `testWidgets(` in `app/test` und
  `app/packages/ui/test`, getrennt davon die Aufrufe von `goldenTest(`.
- **Tastenverweise:** Vorkommen von `LogicalKeyboardKey` in denselben beiden
  Verzeichnissen.
- **Token-Felder und Lesestellen:** Fundstellen des Musters
  `tokens\.[a-zA-Z]*\.[a-zA-Z0-9]*` in
  `app/packages/ui/lib/src/widgets/*.dart`; verschiedene Fundstellen sind die
  Felder, alle Fundstellen die Lesestellen.
- **Kontrast-Zusicherungen:** Zeilen in `app/packages/ui/test/tokens_test.dart`
  mit `greaterThanOrEqualTo(3.0)`, `greaterThanOrEqualTo(4.5)`,
  `lessThan(3.0)`, `lessThan(4.5)` oder `closeTo(`.
- **Breaking-Einträge:** für `shadcn_flutter` die Aufzählungspunkte der obersten
  Ebene unter einer Überschrift `### Breaking` oder `### Breaking Changes`, für
  forui die Zeilen mit der Markierung `**Breaking**`. Wo ein Eintrag endet und der
  nächste beginnt, ist eine Ermessensfrage: Zählt man verschachtelte Unterpunkte
  mit, kommt `shadcn_flutter` auf 26 statt 18 (allein 0.0.50 auf 16 statt 8) und
  forui 0.24.0 auf etwa 23 statt 13. An der Rangfolge ändert das nichts.
- **Externe Zahlen** (Sterne, Issues, Commits, Versionen, Lizenzen, Downloads)
  stammen aus den APIs von GitHub und pub.dev, abgerufen am 2026-09-04.

Der Stand am Messtag: `app/packages/ui` hat rund 9.900 Zeilen Dart, davon rund
6.400 in `lib/` (18 Widget-Dateien mit rund 3.300 Zeilen, rund 1.900 Zeilen
Tokens, 1.044 Zeilen Galerie, 78 Zeilen Theme) und rund 3.500 in `test/`.

### Was die drei Bildschirme wirklich brauchten

Die Schicht bietet 21 Widgets in `app/packages/ui` (dazu `HTheme` als
Theme-Träger und `HGalleryPage`) und neun weitere in `app/lib/core/ui`, die zur
Übergabe an das Paket vorgemerkt sind.

| Bildschirm | verschiedene Widgets der Schicht | die häufigsten |
|---|---|---|
| Anhalten | 15 | `HButton`, `HBadge`, `HGlyphIcon`, `HHairline`, `HRow`, `FocusRing` |
| Regeln | 14 | `HButton`, `HGlyphIcon`, `HTextField`, `HHairline`, `HSegmented` |
| Verlauf | 17 | `HButton`, `HRow`, `HHairline`, `HDiagnosticCard`, `HWait` |

Zusammen sind das 24 verschiedene Widgets aus beiden Verzeichnissen. Genau zwei
haben keinen Aufrufer außerhalb der Galerie: `HPill` und `HPanel`. Beide sind
Wrapper, die dieses ADR ursprünglich als Kern der Schicht genannt hat; die
Aktionsleiste hat die geteilte Pille (`release_valve.dart`) stattdessen selbst
gebaut, weil sie Halten mit einer Füllung braucht. `HAnimatedFill` und
`HSegment` tauchen in der Tabelle ebenfalls nicht auf, sind aber keine toten
Einträge: Sie sind Bausteine von `HButton`, `HSegmented` und `HRow`.

### Was noch fehlt

Aus `docs/UX.md` 9 und den Übergabenotizen in `app/lib/core/ui`, gemessen an den
sieben Elementen, die dieses ADR von einer Bibliothek erwartet hat. Die
Tageszahlen sind Größenordnungen aus dem Umfang der jeweiligen Aufgabe, keine
Messwerte.

| Element | Stand am 2026-09-04 | Aufwand im Eigenbau |
|---|---|---|
| Resizable | `HResizablePanes` (`app/lib/core/ui`), horizontal, ein Aufrufer | vertikale Achse 0,5 T |
| Befehlspalette | `app/lib/features/shell/widgets/command_palette.dart`, 243 Zeilen | erledigt |
| Sheet | `HSheet`, drei Aufrufe | erledigt |
| Segmented | `HSegmented` (sechs Aufrufe), `HChoiceChips` (einer) | erledigt |
| ContextMenu | fehlt; gebraucht für den JSON-Baum („Wert kopieren", „Pfad kopieren", HUM-030) und das Terminal (Rechtsklick Kopieren/Einfügen, HUM-042) | 1 T |
| Toast | wird nicht gebaut: `docs/UX.md` 2.11, 4.6 und 4.8 verbieten ihn, BACKLOG.md 5 führt „Toast-Spam" als Anti-Pattern | entfällt |
| Menüleiste | wird nicht gebraucht: Icon-Leiste und Befehlspalette sind die beiden Wege, kein Dokument verlangt eine Menüleiste | entfällt |

Damit bleiben **fünf benötigte** der sieben Elemente des ADR: Resizable,
Befehlspalette, Sheet, Segmented, ContextMenu. Toast und Menüleiste fallen für
alle Kandidaten gleichermaßen weg, nicht nur für den Eigenbau; sie zählen
deshalb in Kriterium 1 in keiner Spalte. Eine Datentabelle steht gar nicht in der
Liste des ADR und zählt ebenfalls nirgends: `history_table.dart` löst sie mit rund
890 Zeilen, `ListView.builder` mit bekanntem `itemExtent`, einem memoisierten
Build je Flow und einem `Semantics`-Knoten je Zeile statt elf.

Bleiben 3,5 Personentage Eigenbau: ContextMenu 1 T, Datums- und Zeitauswahl 2 T
(Zeitraumfilter des Audit-Bildschirms, Sprint 4), vertikale Achse des Resizable
0,5 T. Diese Zahl ist der Bezugswert für alles Weitere.

### Die Punkteskala

Das Issue schreibt 0 bis 3 Punkte je Kriterium vor, aber keine Bedeutung dafür.
Ohne Schwellen wäre jede einzelne Bewertung ein Prosa-Urteil, deshalb stehen sie
hier. Sie gelten für alle drei Spalten gleich.

| Kriterium | 3 Punkte | 2 | 1 | 0 |
|---|---|---|---|---|
| 1 Komponenten (von den fünf benötigten selbst geliefert) | fünf | vier | zwei oder drei | höchstens eine |
| 2 Getroffene Fehler | keine, und durch eigene Tests belegt | keine festgehalten, aber unbelegt | mindestens ein offener Defekt in einer Komponente, auf die wir uns stützen | mehr als drei, oder einer ohne Workaround |
| 3 Breaking-Einträge in sechs Releases | null | 1 bis 5 | 6 bis 20 | mehr als 20 |
| 4 Passung des Themings | null Overrides, `HTokens` die einzige Farbquelle, durch Tests gebunden | einzelne Overrides, eine Quelle | eine zweite Farbquelle nötig, oder ohne Prototyp nicht entscheidbar | die Bibliothek erzwingt ihre eigene Farbskala |
| 5 Tastatur und Fokus | im Repository gemessen, Overlays durch Tests abgedeckt | dokumentiert, nicht gemessen | Lücken dokumentiert | keine Fokusbehandlung |
| 6 Performance des Verlaufs | mit DevTools über 10.000 Zeilen gemessen, unter 16 ms | Methode belegt, nicht gemessen | wäre ein Rückschritt gegenüber heute | messbar über 16 ms |
| 7 Community und Pflege | über 50 Commits in 90 Tagen von mindestens drei Konten | mindestens 20 Commits von mindestens zwei Konten | weniger, oder alles auf einem Konto | kein Commit in 90 Tagen |
| 8 Lizenz | OSI-anerkannt und kompatibel mit GPL-3.0-only | kompatibel unter Bedingungen | unklar | inkompatibel |
| 9 Wechselaufwand | null Tage | bis 3 Tage | bis 8 Tage | über 8 Tage |

### Die Matrix

Kriterien und Gewichte aus `backlog/sprint-2.md`, HUM-035. Höchstwert 60.
Versionen mit Stand 2026-09-04: `shadcn_flutter` 0.0.54 (erschienen 2026-08-27),
forui 0.26.0 (erschienen 2026-08-24), unsere eigene Schicht wie oben
beschrieben.

| Kriterium | Gew. | Eigenbau | shadcn 0.0.54 | forui 0.26.0 |
|---|---:|---:|---:|---:|
| 1 Komponenten (fünf benötigt) | 3 | 2 | 3 | 2 |
| 2 Getroffene Fehler in Sprint 1–2 | 3 | 3 | 1 | 2 |
| 3 Breaking Changes in den letzten sechs Releases | 2 | 3 | 1 | 0 |
| 4 Passung des Themings zu BACKLOG.md 5 | 3 | 3 | 1 | 1 |
| 5 Tastatur und Fokus | 3 | 3 | 2 | 2 |
| 6 Performance des Verlaufs | 2 | 2 | 1 | 2 |
| 7 Community und Pflege | 1 | 1 | 1 | 3 |
| 8 Lizenz | 1 | 3 | 3 | 3 |
| 9 Wechselaufwand | 2 | 3 | 0 | 0 |
| **Gewichtete Summe** | **20** | **53** | **29** | **31** |
| **Anteil** | | **88,3 %** | **48,3 %** | **51,7 %** |

Die Belege, Kriterium für Kriterium:

**1 Komponenten (Gewicht 3).** Gezählt werden die fünf benötigten Elemente, für
jede Spalte nach derselben Regel: Was die Spalte selbst liefert, zählt; was sie
nicht liefert, bliebe Eigenbau. `shadcn_flutter` liefert alle fünf
(`control/command.dart`, `layout/resizable.dart` (zehn Vorkommen von
`Axis.vertical`), `overlay/drawer.dart`, `form/multiple_choice.dart`,
`menu/context_menu.dart`), 5 von 5, 3 Punkte. forui liefert Resizable, Sheet,
ContextMenu und mit `tabs`/`select_group` einen Ersatz für Segmented, aber keine
Befehlspalette: 4 von 5, 2 Punkte. Der Eigenbau hat Befehlspalette, Sheet,
Segmented und das horizontale Resizable, das ContextMenu fehlt: 4 von 5,
2 Punkte. Eine großzügigere Lesart, die forui das fehlende Command nachsieht,
brächte dort 3 Punkte und 34 von 60; die Rangfolge bleibt.

**2 Getroffene Fehler (Gewicht 3).** `grep -rn "WORKAROUND\|TODO\|FIXME\|HACK"
app/packages/ui` findet null Stellen. Belegt wird das durch über 500 Aufrufe von
`test(` und `testWidgets(`, dazu 26 `goldenTest(` in sechs Dateien mit 49
gespeicherten Golden-Bildern: 3 Punkte. Für beide Bibliotheken ist die Zahl der
getroffenen Fehler null, weil keine je eingebunden war; das ist kein Beleg für
Qualität, und die Wertung ist hier eine Schätzung, keine Messung. Der einzige
festgehaltene Beleg steht in `backlog/sprint-1.md`, HUM-020: `ResizablePane` in
`shadcn_flutter` 0.0.54 hat die offenen Defekte #427 und #428, mit
`multi_split_view` als Workaround, und das trifft genau die Komponente, die das
Layout des Anhalten-Bildschirms trägt. Daher 1 Punkt für `shadcn_flutter` und 2
für forui, gegen das nichts Vergleichbares vermerkt ist.

**3 Breaking Changes (Gewicht 2).** Aus den Changelogs, Stand 2026-09-04, nach
der oben genannten Zählregel. `shadcn_flutter` 0.0.49 bis 0.0.54: 18
Breaking-Einträge, acht davon in 0.0.50 (das Auswahlmodell von `NavigationBar`
wechselt von Index auf Wert, mehrere Parameter fallen weg) und zehn in 0.0.54
(Material und Cupertino aus dem Paket entfernt, `ShadcnApp` setzt keine
Material-Vorfahren mehr, Untergrenze Flutter 3.47.0). Elf davon träfen uns: Die
Navigation ist unsere Icon-Leiste, und die Entfernung von Material trifft jede
App: 1 Punkt nach der Schwelle „6 bis 20". forui 0.24.0 bis 0.26.0, mit den
drei Patch-Versionen genau sechs Releases: 27 Breaking-Einträge, 13 davon in
0.24.0 (`FThemes` entfernt, alle vordefinierten Farbschemata außer
`FColors.neutralLight` und `FColors.neutralDark` entfernt,
`FBadgeContentStyle`, `FCardContentStyle`, `FDialogContentStyle` entfernt,
`FCard.raw` und `FDialog.raw` umbenannt), zehn in 0.25.0 und vier in 0.26.0
(Wechsel auf `material_ui` und `cupertino_ui`, `forui_assets` heißt jetzt
`forui_lucide`, Untergrenze Flutter 3.47.0). Das trifft Badge, Karte, Dialog,
Theme und Textfeld, genau unsere Oberfläche: 0 Punkte nach der Schwelle „mehr
als 20". Der Eigenbau hat kein Upstream und deshalb null: 3 Punkte.

Dazu ein Befund, der unabhängig von der Wertung gilt: **Beide Bibliotheken
verlangen in ihrer aktuellen Version Flutter ≥ 3.47.0 und Dart ≥ 3.13.0.** Die
einzige Festlegung, die der CI-Job liest, ist `app/.fvmrc` mit `3.44.0`
(`.github/actions/setup-flutter`); `app/pubspec.yaml` trägt die Untergrenze
zusätzlich als eigene Bedingung. Die jeweils letzte Version, die auf unserem
Stand läuft, ist `shadcn_flutter` 0.0.53 (2026-07-14) und forui 0.25.0
(2026-08-02). Eine Übernahme an der Spitze beginnt deshalb mit einem
Flutter-Upgrade, nach dem alle 49 Goldens neu freigegeben werden müssen.

**4 Passung des Themings (Gewicht 3).** Das Kriterium misst Overrides nach Art
von `!important` in `HTokens`: null. Die Widget-Rümpfe lesen 30 verschiedene
Token-Felder an rund 100 Stellen über rund 20 Aufrufe von `HTheme.of(context)`;
die einzigen sieben Farbliterale in den Rümpfen sind `Color(0x00000000)`, also
transparent. Zustandsfarben kommen ausschließlich aus `FlowStateColor` und
`HStateColors`, mit getrennten Varianten für Fläche und Text, und rund 30
Kontrast-Zusicherungen in `tokens_test.dart` binden sie auf beiden Skalen an 3:1
für Flächen und 4,5:1 für Text: 3 Punkte. forui hat in 0.24.0 alle
vordefinierten Farbschemata bis auf zwei entfernt und verweist auf einen
Generator (`dart run forui theme create`); ein erzeugtes Theme wäre eine zweite
Farbquelle neben `HTokens`, von Hand synchron zu halten: 1 Punkt, und dieser
lässt sich ohne Prototyp begründen. `shadcn_flutter` bringt ein `ColorScheme` mit
Radius und Typografie mit; unsere 30 Felder müssten als Adapter darübergelegt
werden, und was das mit den gemessenen Kontrasten macht, ist ohne Prototyp
**nicht** entscheidbar: Der eine Punkt dort fällt unter die zweite Hälfte der
Schwelle und ist eine Schätzung.

**5 Tastatur und Fokus (Gewicht 3).** Rund 200 Vorkommen von
`LogicalKeyboardKey` in 18 Testdateien. `HModal` trägt `FocusScope` und bindet
`Escape` über `DismissIntent`; zehn der 21 Widgets haben eine eigene
Fokusbehandlung; `HFocusRing` zeichnet den zwei Pixel breiten Akzentring
außerhalb des Bedienelements, `HFocusRing.inline` am Rand der geteilten Pille.
Das schließt die Punkte 14, 16 und 17 aus `docs/UX.md` 9: 3 Punkte. Für beide
Bibliotheken gibt es keine Messung, weil der Prototyp-Branch nicht gebaut wurde;
je 2 Punkte nach der Schwelle „dokumentiert, nicht gemessen".

**6 Performance des Verlaufs (Gewicht 2).** `history_table.dart` ist ein
`ListView.builder` mit bekanntem `itemExtent`, einem memoisierten Build je Flow
und einem `Semantics`-Knoten je Zeile statt elf. Die DevTools-Messung über
10.000 Zeilen, die das Kriterium verlangt, steht noch aus: 2 Punkte statt 3.
`shadcn_flutter` bekommt 1 Punkt: Sein `layout/table.dart` wäre bei den
`Semantics`-Knoten ein Rückschritt gegenüber dem, was heute läuft. forui bekommt
ebenfalls 2 Punkte, nicht 1: Es bringt keine Tabelle mit, unsere Methode bliebe
unverändert, und wo sich nichts ändert, darf die Zahl nicht fallen.

**7 Community und Pflege (Gewicht 1).** Zahlen aus den APIs von GitHub und
pub.dev vom 2026-09-04. `sunarya-thito/shadcn_flutter`: 936 Sterne, 26 offene
Issues, letzter Push 2026-08-27, 28 Commits seit 2026-06-06, alle 28 von einem
einzigen Konto; rund 12.300 Downloads in 30 Tagen, 469 Likes, 160 von 160
Punkten: 1 Punkt nach der Schwelle „alles auf einem Konto".
`duobaseio/forui`: 2.327 Sterne, 48 offene Issues, letzter Push 2026-09-03, 95
Commits seit 2026-06-06 von fünf Konten (54, 23 von Renovate, 16, zweimal 1);
rund 23.900 Downloads in 30 Tagen, 432 Likes, 160 von 160 Punkten: 3 Punkte.
Der Eigenbau hat keine Community von außen: ein Konto, also 1 Punkt nach
derselben Schwelle. Das ist die einzige Zeile, in der der Eigenbau klar
verliert.

**8 Lizenz (Gewicht 1).** `shadcn_flutter` steht unter BSD-3-Clause, forui unter
MIT mit OFL-1.1 für die mitgelieferten Schriften; beide sind OSI-anerkannt und
kompatibel mit GPL-3.0-only. Unsere eigene Schicht ist bereits GPL-3.0-only.
Je 3 Punkte; das Kriterium trennt nichts.

**9 Wechselaufwand (Gewicht 2).** Für den Eigenbau null Tage: 3 Punkte. Für
beide Bibliotheken 8,5 bis 11,5 Personentage nach der Aufstellung unten, also
über der Schwelle von acht Tagen: 0 Punkte.

### Die Entscheidungsregel

Das Issue sagt: „bleiben, wenn shadcn ≥ 75 % der gewichteten Punkte und kein
Kriterium mit Gewicht 3 unter 1 Punkt; sonst wechseln." Die Regel wurde
geschrieben, als `shadcn_flutter` der Platzhirsch sein sollte. Sie geht in beiden
Lesarten gleich aus:

- Wörtlich angewandt, auf `shadcn_flutter`: 48,3 % liegt unter 75 %, also nicht
  bestätigen.
- Angewandt auf den tatsächlichen Platzhirsch, unsere eigene Schicht: 88,3 %
  liegt über 75 %, und das schwächste Kriterium mit Gewicht 3 hat 2 Punkte
  (Komponenten), also bleiben.

### Wie belastbar das Ergebnis ist

Vier der neun Kriterien sind für die Bibliotheken **nicht gemessen**, weil der
Prototyp-Branch nicht gebaut wurde: 2 (Fehler), 4 (Theming), 5 (Tastatur und
Fokus) und 6 (Performance). Zusammen wiegen sie 11 der 20 Gewichtspunkte. Das
muss offen gesagt werden, sonst prüft die Matrix nichts und rechtfertigt nur.

Setzt man diese vier für die Bibliotheken auf die volle Punktzahl, also den für
sie günstigsten unbelegten Fall, kommt `shadcn_flutter` auf 48 von 60 (80,0 %)
und forui auf 45 von 60 (75,0 %). **Beide erreichen damit die Schwelle des
Issues.** Der Eigenbau bleibt bei 53 von 60 und liegt weiter vorn, aber die
Regel allein trüge die Entscheidung in diesem Fall nicht mehr.

Was sie dann trägt, sind die Zeilen, die ohne Prototyp feststehen: Kriterium 3
aus den Changelogs (18 und 27 Breaking-Einträge in sechs Releases), die
Kriterien 7 und 8 aus den öffentlichen Registern, Kriterium 9 aus dem Umfang
unseres eigenen Codes und die Flutter-Untergrenze 3.47.0, die keine Wertung hat
und trotzdem jeden Wechsel um zwei bis drei Tage verteuert. Für Kriterium 4 lässt
sich bei forui zusätzlich ohne Prototyp argumentieren (ein erzeugtes Theme wäre
eine zweite Farbquelle neben `HTokens`), bei `shadcn_flutter` nicht; dort bleibt
die 1 eine Schätzung.

Die Gegenprobe in die andere Richtung: Streicht man Kriterium 9 ganz, weil ein
Platzhirsch dort systematisch im Vorteil ist, bleiben 54 mögliche Punkte, und
der Eigenbau kommt auf 47 (87,0 %) gegenüber 29 (53,7 %) und 31 (57,4 %). Die
Entscheidung kippt also weder ohne den Platzhirsch-Bonus noch im günstigsten
unbelegten Fall zu einem klaren Ergebnis für eine Bibliothek. Sie kippt in eine
Lage, in der ein Herausforderer die Schwelle knapp erreicht und dafür 8,5 bis
11,5 Tage verlangt, während die bestehende Schicht 3,5 Tage Restarbeit kostet.
An diesem Satz hängt die Entscheidung wirklich, nicht an der Prozentzahl.

### Was ein Wechsel gekostet hätte

Sollte die Entscheidung später doch anders fallen, ist das die Aufstellung.
Daraus entsteht kein Issue; HUM-035b entfällt. Die Tageszahlen sind
Größenordnungen aus dem Umfang der betroffenen Dateien, keine Messwerte; der
größte Posten nimmt rund 1.100 Zeilen Widget-Rumpf pro Tag bei unveränderten
Signaturen an.

1. Die Flutter-Festlegung von 3.44.0 auf 3.47.0 anheben (`app/.fvmrc`,
   `app/pubspec.yaml`), auf `material_ui` und `cupertino_ui` umstellen, 49
   Goldens neu freigeben: 2 bis 3 Tage.
2. `HTheme` auf das Theme der Bibliothek umstellen, `HTokens` als einzige
   Farbquelle behalten: 1 bis 2 Tage.
3. Die 18 Widget-Rümpfe (rund 3.300 Zeilen) umbauen, Signaturen unverändert,
   damit kein Bildschirm etwas merkt: 3 Tage.
4. Goldens neu freigeben und die Kontrast-Zusicherungen gegen die neuen Flächen
   prüfen: 1 bis 2 Tage.
5. Tastatur und Fokus in Overlays mit den Tastentests in Einklang bringen:
   1 Tag.
6. Die Lokalisierung der Bibliothek registrieren (`ShadcnLocalizations`; sonst
   stürzt zum Beispiel der DatePicker unter `de` ab, `backlog/sprint-4.md`
   HUM-052): 0,5 Tage.

Zusammen 8,5 bis 11,5 Personentage, ohne den wiederkehrenden Upgrade-Tag je
Sprint.

### Folgen dieser Entscheidung

- `app/packages/ui` bleibt auf `package:flutter/widgets.dart`. Weder
  `shadcn_flutter` noch forui steht in `app/pubspec.yaml`; ein Eintrag wäre ein
  Architekturverstoß und wird im Review beanstandet.
- Die Kapselung bleibt und behält ihren Zweck: Features importieren
  `lib/core/ui/ui.dart`, nie ein fremdes Paket. Sie schützt nicht mehr vor einer
  Bibliothek, hält aber die Tür für eine spätere offen.
- Der Upgrade-Tag je Sprint entfällt. Der Puffer HUM-061 ist nicht mehr für
  Brüche in shadcn reserviert.
- Drei Issues bauen den Rest, jedes für sich, Nummern beim Zuschnitt von
  Sprint 3: ein ContextMenu für JSON-Baum und Terminal (1 T), eine Datums- und
  Zeitauswahl für den Zeitraumfilter (2 T), die vertikale Achse für
  `HResizablePanes` (0,5 T).
- Die neun Widgets aus `app/lib/core/ui`, die zur Übergabe vorgemerkt sind
  (`HResizablePanes`, `HCollapsible`, `HDiagnosticCard`, `HoverLabel`,
  `HoldToConfirm`, `FixControl`, `FocusRing`, `ShellGlyphIcon`,
  `SectionPlaceholder`), ziehen nach `app/packages/ui` um. `FocusRing` dort und
  `HFocusRing` im Paket sind zweimal dasselbe; nach dem Umzug bleibt eines.
- `HPill` und `HPanel` haben keinen Aufrufer außerhalb der Galerie. Entweder sie
  bekommen einen, oder sie fliegen raus; ein Wrapper ohne Nutzer ist Ballast, der
  bei jedem Umbau mitwandert.
- Die DevTools-Messung des Verlaufs über 10.000 Zeilen steht noch aus und gehört
  zu HUM-054.
- Die datenlastigen Pakete aus dem Abschnitt „Entscheidung" bleiben unberührt;
  `history_table.dart` zeigt allerdings, dass `two_dimensional_scrollables` für
  den Verlauf nicht gebraucht wird. Ob es für den JSON-Baum kommt, entscheidet
  HUM-030, nicht dieses ADR.
- Der Satz im Abschnitt „Folgen", der für den Fall einer Entscheidung zugunsten
  von forui ein Nachfolge-ADR ankündigt, ist damit erledigt: Die Entscheidung
  begünstigt keine Bibliothek, und die Begründung dafür steht unten unter
  „Verworfene Alternativen dieser Überarbeitung".
- Diese Entscheidung wird neu geöffnet, wenn eine der drei Bedingungen aus „Ohne
  Prototyp entschieden" eintritt.

**Stellen, die dieser Entscheidung noch widersprechen.** Sie stehen hier, damit
niemand sie für gültig hält:

- `backlog/sprint-0.md` 1749 (HUM-008) und `backlog/sprint-4.md` 1476
  (HUM-054) waren Akzeptanzkriterien, die das Gegenteil dieser Regel verlangten
  („shadcn_flutter exakt festgelegt", „shadcn_flutter auf die nächste Version
  anheben"). Beide wurden an dieses ADR angeglichen, minimal und ohne den Rest
  der Kästchen anzufassen.
- `backlog/sprint-2.md` (HUM-035) wurde in seinem Ansatz, seinen Schritten und
  dem Akzeptanzkriterium zum Spike-Branch angeglichen; der falsche Pfad
  `docs/adr/0009-ui-kit.md` ist zu `0009-ui-stack.md` berichtigt.
- `backlog/sprint-0.md` (HUM-008, dazu die Flutter-Version 3.47.0 in den Zeilen
  115 und 208) und `backlog/sprint-1.md` (HUM-019, HUM-020) beschreiben
  `shadcn_flutter` in Kontext, Spezifikation und Fallstricken weiterhin als
  gesetzt. Das bleibt als Geschichte stehen: Es sind die Aufträge, unter denen
  die Arbeit entstand, und dieses ADR ist die Antwort darauf. Der Fallstrick zu
  `ResizablePane` in HUM-020 bleibt zusätzlich als Beleg für Kriterium 2.
- `backlog/sprint-4.md` (Fallstrick in HUM-052 zur Lokalisierung, Verweis in
  HUM-054 auf shadcn-Upgrades) und `backlog/sprint-5.md` (Risikotabelle mit
  Brüchen in shadcn, Zeilen 709, 728, 748, 751; `find.byType(Dialog)` in
  HUM-058, Zeilen 398 und 415) betreffen Sprints, die noch nicht zugeschnitten
  sind. Sie werden angeglichen, wenn der jeweilige Sprint zugeschnitten wird,
  nicht vorher; `BACKLOG.md` 10 ist bereits umgestellt und geht den
  Sprint-Dateien vor.

### Verworfene Alternativen dieser Überarbeitung

- **`shadcn_flutter` bestätigen.** 48,3 % der gewichteten Punkte, also unter der
  Schwelle, die das Issue selbst gesetzt hat; 80,0 % im günstigsten unbelegten
  Fall. Der breiteste Komponentenbestand von allen, aber die Pflege hängt an
  einer Person, die Übernahme beginnt mit einem Flutter-Upgrade, und die
  Komponente, die unser Layout des Anhalten-Bildschirms trägt, hat zwei offene
  Defekte.
- **Zu forui wechseln.** 51,7 %, 75,0 % im günstigsten unbelegten Fall: die beste
  Pflege im Feld, aber 27 Breaking-Einträge in sechs Releases, kein Ersatz für
  die Befehlspalette und ein erzeugtes Theme als zweite Farbquelle neben
  `HTokens`.
- **Einzelne Komponenten aus einer Bibliothek ziehen** (zum Beispiel nur
  Command und ContextMenu). Bringt die ganze Abhängigkeit, das ganze Theme und
  die ganze Flutter-Untergrenze für zwei Widgets mit, die im Eigenbau zusammen
  1 Tag kosten.
- **Ein spezialisiertes Paket je Lücke** (`multi_split_view` für das Resizable
  und Ähnliches). Bleibt erlaubt und ist kein Widerspruch: Der Abschnitt
  „Entscheidung" nimmt die datenlastigen Widgets aus genau solchen Paketen. Für
  die drei offenen Lücken lohnt es sich nicht, weil jede unter zwei Tagen liegt
  und jede Zustandsfarbe und Fokusring aus unseren Tokens übernehmen muss.
- **Den Spike doch bauen und dann entscheiden.** Der Weg, den das Issue
  vorgesehen hatte. Verworfen mit der Begründung im Abschnitt „Ohne Prototyp
  entschieden": Der Zeitrahmen von einem Tag wäre vom Flutter-Upgrade vor der
  ersten Zeile der Portierung aufgebraucht worden, und die Bedingung, an der die
  Entscheidung hängt, steht ohne ihn fest. Der Preis dafür steht in derselben
  Zeile: Vier Kriterien bleiben eine Schätzung.
- **Dieses ADR durch einen Nachfolger ersetzen.** Der Abschnitt „Folgen" sieht
  das für den Fall vor, dass HUM-035 zugunsten von forui ausfällt. Es fällt
  zugunsten keiner Bibliothek aus, und alles andere in diesem ADR (Flutter, die
  Kapselung hinter `packages/ui`, kein WebView, ein Fenster mit angedockten
  Bereichen, riverpod mit Generator, freezed, die spezialisierten Pakete) steht
  unverändert. Ein Nachfolge-ADR hätte 90 % des Textes wiederholt, um einen
  Paketnamen zu streichen.

## Am 2026-09-04 vom Projektinhaber überarbeitet

Der Abschnitt „Entscheidung nach Sprint 2" gilt nicht mehr. Er hatte entschieden,
`shadcn_flutter` nicht zu übernehmen und die eigene Widget-Schicht zu behalten.
Der Projektinhaber hat das am selben Tag überstimmt: **Flutter läuft jederzeit
auf der neuesten stabilen Version, und `shadcn_flutter` wird übernommen.**

Warum die vorige Entscheidung falsch war, unabhängig vom Ergebnis: In seiner
ursprünglichen Fassung nannte dieses ADR `shadcn_flutter` als gesetzt. Das war
eine Vorgabe des Projektinhabers, kein Vorschlag. HUM-035 hat sie ohne Rückfrage
revidiert und damit als Überprüfung ausgegeben, was in Wahrheit die Umkehr der
Entscheidung eines anderen war. Eine Entscheidung, die etwas umkehrt, das der
Projektinhaber festgelegt hat, geht vorher als Frage an ihn, nicht hinterher als
Ergebnis.

### Was jetzt gilt

Flutter steht auf **3.47.2** mit Dart **3.13.2**, festgelegt in `app/.fvmrc`,
und folgt ab jetzt dem neuesten stabilen Stand. Blockiert ein Paket ein Upgrade,
wird das Paket benannt und gelöst, statt das Upgrade zu verschieben. Was der
Wechsel kostete: `intl` von 0.20.2 auf 0.20.3, weil `flutter_localizations` in
3.47.2 seine exakte Festlegung durch `^0.20.3` ersetzt; `freezed` auf 4.0.1,
`riverpod_generator` auf 4.0.9, `json_serializable` auf 6.14.1 und
`build_runner` auf 2.16.1. Die Obergrenzen im Einzelnen, gemessen an den
Pubspecs im Cache: `riverpod_generator` 4.0.3 nimmt `analyzer ^9.0.0`, und
`analyzer` 9.0.0 kennt Sprachversion 3.11, also zwei hinter Dart 3.13; `freezed`
3.2.5 und `json_serializable` 6.13.0 nehmen `analyzer >=9 <11`, und 10.1.0 kennt
3.12, also eine dahinter. `build_runner` 2.15.1 hätte mit `analyzer >=8 <14`
gereicht; sein Upgrade ist beiläufig, nicht erzwungen. Mitgezogen wurden außerdem
die Laufzeitpakete `flutter_riverpod` auf 3.4.3 und `riverpod_annotation` auf
4.0.7 sowie `json_annotation` auf 4.12.0, das `json_serializable` exakt
vorschreibt. Der übrige Code
blieb unberührt: 3.45 und 3.46 bringen keine Breaking Changes, und die drei aus
3.47 betreffen uns nicht.

`shadcn_flutter` ist exakt auf 0.0.54 festgelegt und steht in
`app/packages/ui/pubspec.yaml`, nicht in `app/pubspec.yaml`. Die Naht bleibt
also: Kein Feature importiert die Bibliothek, `tools/check-deps.sh` beanstandet
jedes `package:shadcn_flutter` außerhalb von `app/packages/ui`, und was ein
Bildschirm sieht, sind weiterhin die `H*`-Widgets.

Sie bringt dreizehn weitere Pakete mit und bündelt **8,3 MB eigene Assets** in
jeden Build: 4,4 MB Geist-Schriften in 33 Schnitten, 1,25 MB Icon-Schriften und
3,3 MB Länderflaggen aus `country_flags`. Flutter nimmt die Schriften eines
Pakets mit, gleich ob jemand sie importiert, das Gewicht fällt also auch an, wenn
wir nur einen Teil der Bibliothek nutzen. Für eine Desktop-Anwendung ist das
vertretbar; es gehört trotzdem hier festgehalten, weil
`backlog/CONVENTIONS.md` 4.11 bis heute sagte „Schriften werden nicht
mitgeliefert". Die Lock-Dateien sind seit diesem
Upgrade versioniert: Die dreizehn Pakete kommen mit Caret-Bereichen, und ohne
Lock würde ein Release eines von ihnen den Build ohne einen einzigen Commit
verändern.

### Was die Bibliothek nicht mitbringt

Vor der Übernahme wurden die 165 Dateien des Pakets im Quelltext gelesen, nicht
das README. Das Ergebnis gehört hierher, weil es die Grenze zieht, bis zu der man
sich darauf verlassen kann:

- Sie prüft nirgends ein Kontrastverhältnis. Es gibt eine Heuristik über die
  Helligkeit (`getContrastColor` in `lib/src/util.dart`), die zu einer Farbe eine
  helle oder dunkle Gegenfarbe wählt, aber kein Verhältnis berechnet. Der
  destruktive Knopf im hellen Theme malt deshalb ein festes `Colors.white` auf ein
  Rot mit halber Deckkraft, das sind 1,97:1, mit einem Kommentar daneben, der das
  einräumt.
- Die gesamte Bibliothek hat drei `Semantics`-Knoten, keinen davon an Knopf,
  Checkbox, Tabs oder Badge.
- `AnimationBehavior` kommt nicht vor. Jede ihrer Animationsdauern schrumpft auf
  fünf Prozent, sobald das System reduzierte Bewegung meldet; was an einem
  `Timer` hängt, etwa die Verzögerung der Hover-Karte, behält seine Zeit.
- Es gibt kein Halten zum Bestätigen, keine Mindest-Klickfläche und auf dem
  Desktop keinen gedrückten Zustand: Die einzige Rückmeldung hängt an einem
  Schalter, der unter Linux abgeschaltet ist.

Sie ist also kein Radix: Sie liefert Aussehen und Rahmen, nicht die
Verhaltensschicht darunter. Das ist kein Einwand gegen die Übernahme, sondern die
Beschreibung dessen, was `packages/ui` weiterhin selbst tun muss.

### Was im MVP gilt und was für 1.0.0 zurückkommt

Der Projektinhaber hat Barrierefreiheit als Programm aus dem MVP genommen: erst
beweisen, dass es funktioniert und gut sein kann. Screenreader-Semantik,
WCAG-Zahlen als Testschranke und die Prüfung bei doppelter Textgröße sind damit
auf 1.0.0 verschoben.

Vier Dinge bleiben trotzdem, weil sie nur nach Barrierefreiheit aussehen:

1. **Die Haltedauer behält ihre Zeit.** Meldet das System reduzierte Bewegung,
   kürzt Flutter jede Animationsdauer auf fünf Prozent. Bei uns hängt daran die
   Sicherung, die verhindert, dass ein Klick eine Anfrage hinausschickt;
   gemessen reichten 60 Millisekunden. Das ist ein Sicherheitsdefekt, der
   zufällig über dieselbe Einstellung läuft.
2. **Eine Untergrenze für Lesbarkeit.** 1,97:1 ist nicht „nicht barrierefrei",
   sondern unleserlich. Die Grenze bleibt, aber als Designfrage und deutlich
   lockerer als die 4,5:1, die bis heute galten.
3. **Die Klickflächen der Entscheidung.** Mindestens 28 × 28 px allgemein,
   mindestens 32 × 120 px für Senden und Blockieren. Die Bibliothek kennt keine
   Mindest-Klickfläche; wer bei einer unumkehrbaren Aktion abrutscht, hat kein
   Problem mit Barrierefreiheit, sondern eine gesendete Anfrage.
4. **Ein sichtbarer gedrückter Zustand.** Ein Bedienelement, das bei einem Klick
   nichts tut, fühlt sich kaputt an.

### Was die Übernahme kostete

Genau eine Zusicherung wurde gelockert, und sie ist in `docs/UX.md` 8 genannt:

**Der Fokusring von `HTextField` animiert und reserviert keinen Platz.**
`docs/UX.md` 2.9 verlangt einen Fokusring innerhalb eines Frames, weil einer,
der einblendet, sich wie Eingabeverzögerung liest. Das `TextField` der
Bibliothek verdrahtet seine `FocusOutline` fest, ohne Schalter; es lässt einen
Value-Builder über 150 ms laufen, skaliert die Randbreite und zeichnet über den
eigenen Rand hinaus. Zwei Ringe übereinander wären einer zu viel, also ist es
ihrer. Seine Maße bleiben unsere (zwei Pixel Akzent, zwei Pixel Abstand, gesetzt
über ihr `FocusOutlineTheme` aus `HFocusRingMetrics`), und jedes andere
Bedienelement trägt weiterhin `HFocusRing`, das seinen Platz reserviert und
nicht animiert. Kein Kontrastwert wurde gesenkt.

Vier Fallen wurden entschärft statt übernommen, jede mit einem Test, der ohne die
Sicherung rot wird:

- Ihr `TextField` meldet **jede** Änderung des `TextEditingController` an
  `onChanged`, auch eine, die ein Widget selbst zuweist. Flutter dokumentiert für
  `onChanged` das Gegenteil, und die Bildschirme sind unter diesem Vertrag
  geschrieben: Der Regel-Editor füllt beim Öffnen seine Felder, und die Meldung
  liefe mitten im Build zurück in den Provider.
- Ihr `Clickable` bindet unter jedem Bedienelement vier Pfeiltasten an
  `DirectionalFocusIntent`. Das innerste `Shortcuts` gewinnt, also erreichten
  `ArrowUp` und `ArrowDown` den Bildschirm nicht mehr, solange eine Zeile oder
  ein Knopf den Fokus hielt, und die Warteschlange ist ein einziger Fokusstopp
  mit Navigation darin (`docs/UX.md` 5.2).
- Ihre Actions existieren als `CallbackAction<Intent>`.
  `Actions.maybeFind<ActivateIntent>` kann sie nicht casten: Im Debug-Build
  bricht es in einer Assertion ab, im Release-Build liefert es still `null`.
  Genau diese Frage entscheidet, ob eine Bildschirmtaste feuert, während ein
  Bedienelement den Fokus hält (flutter/flutter#180871, `docs/UX.md` 5.2).
- Ihre `Checkbox` rechnet `enabled ?? onChanged != null`. Wer wie wir die
  Entscheidung außerhalb der Komponente trifft und `onChanged` deshalb leer
  lässt, bekommt `false` und damit den Zweig, der den Rand in `muted` malt:
  **Jedes** nicht angekreuzte Kästchen stünde in der Farbe eines toten. `enabled`
  ist deshalb fest auf `true` gesetzt, und die vier Farben kommen aus den Tokens.

## Betroffene Issues

`HUM-008` (Design-Tokens und `packages/ui`), `HUM-019` (Flutter-Shell),
`HUM-020` (Anhalten-Bildschirm v1), `HUM-030` (Body-Ansichten), `HUM-032`
(Verlaufsbildschirm mit virtualisierter Tabelle), `HUM-042` (Terminal mit
xterm2), `HUM-035` (Bestätigung oder Revision dieser Entscheidung), `HUM-054`
(Golden- und Widget-Tests), `HUM-061` (Puffer).
