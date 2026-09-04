# ADR-0009 · UI-Stack: Flutter mit eigener Wrapper-Schicht `packages/ui`
Status: Accepted
Datum: 2026-09-02, zweimal revidiert am 2026-09-04 (HUM-035, dann der Projekteigentümer)

## Kontext

Die Oberfläche von Humanitl ist ein Kontrollraum, kein Formular. Sie zeigt eine
Warteschlange angehaltener Requests mit laufenden Countdowns, eine virtualisierte
History über Zehntausende Zeilen, einen Diff-Editor für die Pseudonymisierung,
ein eingebettetes Terminal, JSON-Bäume, Hex-Ansichten und ein Panel mit
Live-Isolationsprüfungen. Das ist deutlich mehr, als eine Widget-Bibliothek für
Geschäftsanwendungen abdeckt.

Zugleich soll das Ergebnis ruhig aussehen und nicht wie ein Hacker-Werkzeug
(Prinzip 4): Farbe bedeutet Zustand, nie Dekoration; Entscheidungen passieren
inline und nie in einem Modal. Zielplattform ist der Linux-Desktop unter X11 und
Wayland, ausgeliefert als `.deb` und AppImage.

Zwei Risiken sind von Anfang an sichtbar: Eine junge Widget-Bibliothek kann
brechen oder unbetreut liegenbleiben, und eine falsche Wahl im Sprint 0 wäre
teuer, wenn sie sich erst im dritten Sprint zeigt.

## Entscheidung

> **Zweimal revidiert am 2026-09-04.** HUM-035 hatte diesen Abschnitt
> ersetzt und `shadcn_flutter` verworfen; der Projekteigentümer hat das am
> selben Tag zurückgenommen. Es gilt der Abschnitt „Revidiert am 2026-09-04
> durch den Projekteigentümer" ganz unten: `shadcn_flutter` wird
> aufgenommen, gepinnt auf 0.0.54 und ausschließlich in
> `app/packages/ui`. Alles Übrige dieses Abschnitts gilt unverändert. Der
> ursprüngliche Wortlaut bleibt stehen, damit die Kette nachvollziehbar
> ist — auch der letzte Absatz, der die Überprüfung am Ende von Sprint 2
> ankündigt: Sie hat stattgefunden, zweimal.

Die Oberfläche ist Flutter (Desktop/GTK). Für das Chrome — Resizable Panes,
Command Palette, Sheet, Toast, ContextMenu, Menubar — kommt
[shadcn_flutter](https://pub.dev/packages/shadcn_flutter) zum Einsatz, mit
**exakt gepinnter Version** und **vollständig gekapselt hinter dem lokalen Paket
`app/packages/ui`**. Kein Feature importiert `shadcn_flutter` direkt; alle
Bildschirme benutzen die Wrapper `HButton`, `HPill`, `HBadge`, `HPanel`, `HRow`,
`HModal`, `HSheet` und die Design-Tokens `HTokens`. Ein Upgrade der Bibliothek
ist ein eigenes, geplantes Vorhaben: ein Upgrade-Tag pro Sprint.

Die datenlastigen Widgets kommen ausdrücklich **nicht** aus shadcn, sondern aus
spezialisierten Paketen:

| Aufgabe | Paket |
|---|---|
| Virtualisierte Tabelle, Baum | `two_dimensional_scrollables` |
| Code- und Body-Editor | `re_editor` |
| Terminal | `xterm2` |
| Diff für den Pseudonymisierungs-Editor | `diff_match_patch` |

Zustand: `flutter_riverpod` 3 mit Generator (`riverpod_annotation`,
`riverpod_generator`), Modelle mit `freezed` 4, `FlowEvent` als sealed class.
Der Datenfluss geht in eine Richtung: Event-Stream → Provider → Widget. Widgets
rufen den `DaemonClient` nur über Provider-Methoden auf, nie direkt.

**Kein WebView auf Linux.** Eine Domain-Vorschau ist entweder eine Karte aus dem
gebündelten Katalog oder ein vom Daemon geliefertes Bild.

**Ein Fenster mit gedockten Panes**, kein Multi-Window. Multi-Window ist in
Flutter noch experimentell.

Die Wahl shadcn_flutter gegenüber [forui](https://pub.dev/packages/forui) wird
**Ende Sprint 2 überprüft** (HUM-035), nachdem der erste echte Intercept-Screen
existiert: bestätigen oder wechseln.

## Begründung

> **Zweimal revidiert am 2026-09-04.** Die Absätze zu `shadcn_flutter` und
> zur Überprüfung am Ende von Sprint 2 sind beantwortet. Die Begründung des
> heutigen Standes steht im Abschnitt „Revidiert am 2026-09-04 durch den
> Projekteigentümer"; der Abschnitt „Entscheidung nach Sprint 2" ist
> Historie. Alles Übrige gilt unverändert.

Flutter liefert für den Linux-Desktop ein vollständiges Rendering ohne
GTK-Widget-Zoo, eine sehr gute Animationsschicht (Ankunfts- und
Verlassen-Animationen der Queue-Karten sind Teil des Entwurfs) und ein
Golden-Test-Werkzeug, mit dem sich ein visuelles Konzept gegen Regressionen
sichern lässt. Ein Fenster mit gedockten Panes ist ohne Kampf umsetzbar.

shadcn_flutter deckt genau die Chrome-Elemente ab, die man sonst selbst baut und
dabei schlechter macht: Command Palette, Resizable, Sheet, Toast, Menubar. Der
Wert liegt in der Konsistenz dieser Elemente untereinander, nicht in einzelnen
Widgets.

Die Kapselung hinter `packages/ui` ist der eigentliche Kern dieser Entscheidung.
Sie kostet einen Tag Arbeit und macht die Bibliothekswahl reversibel: Ein
Wechsel zu forui, zu einer anderen Bibliothek oder zu eigenen Widgets berührt
dann ein Paket, nicht acht Features. Genau deshalb ist die Überprüfung Ende
Sprint 2 (HUM-035) ein billiges Versprechen und keine leere Geste.

Die datenlastigen Widgets aus Spezialpaketen zu nehmen, folgt derselben Logik
wie ADR-0001: Nichts Eigenes, wo Bewährtes existiert, aber auch keine
Verrenkung, um alles aus einer Quelle zu beziehen. Eine virtualisierte Tabelle
über Zehntausende Zeilen ist ein eigenes Problem und wird von einem Paket
gelöst, das nur das tut.

Kein WebView, weil er unter Linux die größte Abhängigkeit und die größte
Angriffsfläche des ganzen Programms wäre — und weil das UI genau die Seite
rendern würde, über die der Nutzer gerade entscheidet. Das ist dieselbe
Überlegung wie in ADR-0006 zur automatischen Domain-Vorschau.

riverpod mit Generator statt handgeschriebener Provider, weil die Provider-Namen
verbindlich festgelegt sind (`backlog/CONVENTIONS.md` 3.9) und der Generator
Tippfehler und vergessene Invalidierungen zu Kompilierfehlern macht. freezed für
Modelle, weil die Dart-Seite ein Spiegel der Rust-Kerntypen ist und
Wertsemantik plus erschöpfendes Pattern-Matching braucht.

## Verworfene Alternativen

> **Zweimal revidiert am 2026-09-04.** Der Punkt „forui statt
> shadcn_flutter" ist entschieden: `shadcn_flutter`. Die Liste steht als
> Historie; maßgeblich ist der Abschnitt „Revidiert am 2026-09-04 durch den
> Projekteigentümer".

- **GTK4 direkt (Rust `gtk4-rs` oder Vala).** Nativ, leichtgewichtig, gute
  Systemintegration. Verliert an der Menge der Spezialwidgets: virtualisierte
  Tabelle, Diff-Editor, Terminal und Animationen wären großenteils Eigenbau, und
  Golden-Tests fehlen.
- **Tauri oder Electron.** Bringt eine Browser-Engine als Abhängigkeit mit —
  dieselbe Angriffsfläche, die der WebView-Verzicht gerade vermeidet. Für ein
  Sicherheitswerkzeug die falsche Richtung, unabhängig von der Paketgröße.
- **Qt/QML.** Technisch tragfähig, aber Lizenz- und Paketierungsfragen und ein
  drittes Ökosystem neben Rust und Dart.
- **forui statt shadcn_flutter.** Ernsthaft erwogen und nicht ausgeschlossen:
  Die Entscheidung wird Ende Sprint 2 mit einem echten Bildschirm als Beleg
  überprüft. Die Wrapper-Schicht macht den Wechsel lokal.
- **Alle Widgets selbst bauen.** Maximale Kontrolle, aber der Aufwand fließt in
  Chrome statt in die Fachlichkeit. Widerspricht Prinzip 2.
- **Multi-Window (Queue und History getrennt).** Wäre für den Arbeitsablauf
  angenehm, ist in Flutter aber noch experimentell. Vertagt, nicht verworfen.
- **Terminal als externes Fenster.** Hätte `xterm2` erspart, aber die
  Statuszeile „ein Request wird gehalten" im Terminal des Agenten (ADR-0014)
  und das Mitlesen im UI wären dann nicht möglich.

## Konsequenzen

> **Zweimal revidiert am 2026-09-04.** Die vier Punkte, die
> `shadcn_flutter` betreffen — Pinning, Upgrade-Tag, Import-Verbot,
> Restrisiko —, gelten wieder, in der Fassung des Abschnitts „Revidiert am
> 2026-09-04 durch den Projekteigentümer": Pin auf 0.0.54, Import nur in
> `app/packages/ui`, von `tools/check-deps.sh` erzwungen. Ein Nachfolge-ADR
> für forui entfällt, weil forui nicht gewählt wurde. Goldens,
> `DaemonClient`-Bindung und `HTokens` gelten unverändert.

- `app/packages/ui` ist verbindlich: Ein direkter `shadcn_flutter`-Import in
  einem Feature ist ein Architekturverstoß und wird im Review beanstandet.
- Die Version von shadcn_flutter ist exakt gepinnt (etwa `0.0.54`); ein Upgrade
  ist ein eigener, getesteter Schritt pro Sprint, kein Nebeneffekt eines
  `pub upgrade`.
- Golden-Tests (`alchemist`) sichern die visuellen Zustände ab: Queue-Zeile in
  drei Zuständen, Request-Karte, Aktionsleiste, Domain-Panel bekannt und
  unbekannt, Isolation-Panel — je Sprache, weil DE und EN unterschiedlich lange
  Texte haben.
- Jeder Bildschirm ist gegen das Interface `DaemonClient` gebaut, nicht gegen
  gRPC. Widget-Tests laufen gegen `FakeDaemonClient`, ohne Daemon.
- Die Design-Tokens (`HTokens`) sind Dart-Konstanten aus `BACKLOG.md` 5;
  Zustandsfarben kommen ausschließlich über `FlowStateColor.of(state)`, damit
  Farbe nie dekorativ verwendet wird.
- HUM-035 ist ein echtes Entscheidungs-Issue mit zwei möglichen Ausgängen. Fällt
  es zugunsten von forui aus, wird dieser ADR nicht überschrieben, sondern durch
  einen Nachfolge-ADR ersetzt (Status dann `Superseded by`).
- Ein Restrisiko bleibt: shadcn_flutter ist jung. Der Puffer in HUM-061 ist
  ausdrücklich auch für shadcn-Breakage reserviert.

## Entscheidung nach Sprint 2 (2026-09-04)

> **Dieser Abschnitt ist Historie.** Er hält fest, wie HUM-035 entschieden
> hat und woran gemessen wurde. Sein Ergebnis — keine Bibliothek — gilt
> nicht mehr; der Projekteigentümer hat es am selben Tag zurückgenommen.
> Maßgeblich ist der Abschnitt „Revidiert am 2026-09-04 durch den
> Projekteigentümer". Die Messungen hier bleiben gültig und lesenswert, denn
> sie beschreiben, was die Bibliothek kostet.

HUM-035 sollte die Wahl zwischen `shadcn_flutter` und forui bestätigen oder
revidieren. Beim Nachsehen war die Lage eine andere als die, für die das Issue
geschrieben wurde: `shadcn_flutter` ist nie in `app/pubspec.yaml` eingetragen
worden. `app/packages/ui` steht seit Sprint 0 auf reinem
`package:flutter/widgets.dart` und ist in Sprint 1 und 2 zu einer eigenen
Schicht gewachsen. Drei Bildschirme laufen darauf — Intercept mit
Aktionsleiste, Rules, History. Die Frage ist deshalb nicht mehr „shadcn oder
forui", sondern „bleiben, `shadcn_flutter` aufnehmen oder forui aufnehmen".

**Entscheidung: Wir bleiben bei der eigenen Wrapper-Schicht `app/packages/ui`
auf `package:flutter/widgets.dart` und nehmen weder `shadcn_flutter` noch forui
auf.** Die Kapselungsregel aus dem Abschnitt „Entscheidung" gilt unverändert
weiter; nur der Name der Bibliothek hinter dem Wrapper fällt weg.

### Begründung

Ein Wechsel kostet 8,5 bis 11,5 Personentage und erspart 3,5 Tage Eigenbau.
Beide Bibliotheken verlangen am Kopf Flutter 3.47.0, unser Pin steht auf
3.44.0; die Aufnahme begänne also mit einer Anhebung, die alle 49 Goldens neu
abnehmen lässt. In den letzten sechs Releases stehen 18 Breaking-Punkte bei
`shadcn_flutter` und 27 bei forui, die meisten davon auf unserer Fläche. Die
eigene Schicht hat null Workarounds, null Overrides in `HTokens`, rund 30
Kontrast-Zusicherungen und rund 200 Tastenbezüge; sie erreicht 88,3 % der
gewichteten Punkte, `shadcn_flutter` 48,3 % und forui 51,7 %. Was fehlt —
ContextMenu, Datum-Zeit-Wähler, senkrechtes Resizable — sind drei Widgets,
keine Bibliothek. Die Kapselung bleibt, damit die Entscheidung umkehrbar bleibt.

### Entschieden ohne Prototyp

Das Issue sah einen Branch `spike/forui` vor, Zeitbox ein Tag, und ein
Akzeptanzkriterium dazu. Der Branch ist nicht gebaut worden; das ist eine
bewusste Abweichung, protokolliert in `backlog/CONVENTIONS.md` 4.20 und
in `backlog/sprint-2.md` unter HUM-035 nachgezogen. Der Grund ist die eine
Bedingung, die ohne Prototyp feststeht und die ein Prototyp auch nicht
verändert hätte: Beide Bibliotheken verlangen am Kopf Flutter 3.47.0, der Pin
steht auf 3.44.0. Die Zeitbox wäre für die Anhebung draufgegangen, bevor die
erste Zeile Port geschrieben ist.

**Was das kostet.** Vier Kriterien — 2 (Bugs), 4 (Theming), 5 (Tastatur und
Fokus) und 6 (Performance), zusammen 11 der 20 Gewichtspunkte — bleiben für
beide Bibliotheken Schätzung statt Messung. Der Abschnitt „Wie belastbar das
Ergebnis ist" rechnet vor, wie weit sie das Ergebnis tragen könnten.

**Woran ein Fehlurteil auffiele.** An drei Beobachtungen, jede ohne neuen
Aufwand: Der Eigenbau braucht in einem Sprint mehr als drei neue
Chrome-Elemente auf einmal; die Pflege der Schicht kostet über einen Sprint
gemessen mehr als ein Fünftel der UI-Zeit; oder eine der beiden Bibliotheken
erreicht 1.0 mit einer Stabilitätszusage und hebt damit Kriterium 3 auf. Trifft
eines davon ein, ist der Spike nachzuholen, und dann misst er genau die vier
Kriterien oben.

### Woher die Zahlen kommen

Alle Zahlen aus dem Repository sind am 2026-09-04 im Arbeitsbaum gemessen, auf
dem Stand von Commit `2c26ad9` zuzüglich der noch nicht festgeschriebenen
Arbeit an Sprint 2 (die drei Bildschirme und `packages/ui` sind zu diesem
Zeitpunkt Arbeitsbaum, nicht Commit; ein reiner Commit-Anker hätte sie nicht
erfasst). An diesem Baum wird parallel weitergebaut, und die Zählungen sind
innerhalb eines Tages um einige Prozent gewandert. Wo eine Größe wandert, steht
sie deshalb als gerundete Größenordnung; genau steht nur, was ein abzählbarer
Bestand ist. Neben jeder Größe steht, wie sie gezählt wird:

- **Zeilen:** `wc -l` über `*.dart`, ohne `.dart_tool`.
- **Widgets:** Klassen, die in `app/packages/ui/lib/src/widgets/` auf Spalte 0
  mit `class H` beginnen, ohne die reine Datenklasse `HSegmentOption`.
- **Benutzung je Bildschirm:** Vorkommen des Bezeichners mit Wortgrenze in
  `app/lib/features/<screen>/`, verschiedene Namen gezählt, nicht Aufrufe.
- **Goldens:** PNG unter `app/test/goldens/goldens/ci`. Das Verzeichnis
  `app/test/goldens/failures/` steht in `app/.gitignore` und enthält Artefakte
  fehlgeschlagener Läufe; es zählt nicht mit.
- **Testfälle:** Vorkommen von `test(` und `testWidgets(` in `app/test` und
  `app/packages/ui/test`, dazu getrennt die `goldenTest(`-Aufrufe.
- **Tastenbezüge:** Vorkommen von `LogicalKeyboardKey` in denselben beiden
  Verzeichnissen.
- **Token-Felder und Lesestellen:** Treffer des Musters
  `tokens\.[a-zA-Z]*\.[a-zA-Z0-9]*` in
  `app/packages/ui/lib/src/widgets/*.dart`; verschiedene Treffer sind die
  Felder, alle Treffer die Lesestellen.
- **Kontrast-Zusicherungen:** Zeilen in `app/packages/ui/test/tokens_test.dart`
  mit `greaterThanOrEqualTo(3.0)`, `greaterThanOrEqualTo(4.5)`,
  `lessThan(3.0)`, `lessThan(4.5)` oder `closeTo(`.
- **Breaking-Punkte:** bei `shadcn_flutter` die Aufzählungspunkte auf oberster
  Ebene unter einer Überschrift `### Breaking` beziehungsweise
  `### Breaking Changes`, bei forui die Zeilen mit der Auszeichnung
  `**Breaking**`. Wo ein Punkt aufhört und der nächste anfängt, ist Ermessen:
  Zählt man verschachtelte Unterpunkte mit, kommt `shadcn_flutter` auf 26 statt
  18 (0.0.50 allein auf 16 statt 8) und forui 0.24.0 auf etwa 23 statt 13. Die
  Rangfolge ändert das nicht.
- **Fremde Zahlen** (Sterne, Issues, Commits, Versionen, Lizenzen, Downloads)
  kommen von der GitHub- und der pub.dev-API, abgerufen am 2026-09-04.

Der Stand am Messtag: `app/packages/ui` hat rund 9 900 Zeilen Dart, davon rund
6 400 in `lib/` (18 Widget-Dateien mit rund 3 300 Zeilen, rund 1 900 Zeilen
Token, 1 044 Zeilen Galerie, 78 Zeilen Theme) und rund 3 500 in `test/`.

### Was die drei Bildschirme wirklich gebraucht haben

Die Schicht bietet 21 Widgets in `app/packages/ui` (dazu `HTheme` als
Theme-Wirt und `HGalleryPage`) und neun weitere in `app/lib/core/ui`, die als
Handoff für das Paket vorgemerkt sind.

| Bildschirm | verschiedene Widgets der Schicht | die häufigsten |
|---|---|---|
| Intercept | 15 | `HButton`, `HBadge`, `HGlyphIcon`, `HHairline`, `HRow`, `FocusRing` |
| Rules | 14 | `HButton`, `HGlyphIcon`, `HTextField`, `HHairline`, `HSegmented` |
| History | 17 | `HButton`, `HRow`, `HHairline`, `HDiagnosticCard`, `HWait` |

Zusammen sind das 24 verschiedene Widgets aus beiden Verzeichnissen. Ohne
Aufrufer außerhalb der Galerie sind genau zwei: `HPill` und `HPanel`. Beides
sind Wrapper, die dieser ADR ursprünglich als Kern der Schicht genannt hat; die
Aktionsleiste hat den geteilten Pill (`release_valve.dart`) stattdessen selbst
gebaut, weil sie das Halten mit Füllung braucht. `HAnimatedFill` und `HSegment`
tauchen in der Tabelle ebenfalls nicht auf, sind aber keine Karteileichen: Sie
sind Bausteine von `HButton`, `HSegmented` und `HRow`.

### Was noch fehlt

Aus `docs/UX.md` 9 und den Handoff-Notizen in `app/lib/core/ui`, gegen die
sieben Elemente, die dieser ADR von einer Bibliothek erwartet hat. Die
Tagesangaben sind Größenordnungen aus dem Umfang der jeweiligen Aufgabe, keine
gemessenen Werte.

| Element | Stand am 2026-09-04 | Aufwand Eigenbau |
|---|---|---|
| Resizable | `HResizablePanes` (`app/lib/core/ui`), waagerecht, ein Aufrufer | senkrechte Achse 0,5 d |
| Command Palette | `app/lib/features/shell/widgets/command_palette.dart`, 243 Zeilen | fertig |
| Sheet | `HSheet`, drei Aufrufe | fertig |
| Segmented | `HSegmented` (sechs Aufrufe), `HChoiceChips` (einer) | fertig |
| ContextMenu | fehlt; gebraucht für den JSON-Baum („Copy value", „Copy path", HUM-030) und das Terminal (Rechtsklick Copy/Paste, HUM-042) | 1 d |
| Toast | wird nicht gebaut: `docs/UX.md` 2.11, 4.6 und 4.8 verbieten ihn, BACKLOG.md 5 führt „Toast-Spam" als Anti-Pattern | entfällt |
| Menubar | wird nicht gebraucht: Icon-Rail und Command Palette sind die beiden Wege, kein Dokument verlangt eine Menüleiste | entfällt |

Damit bleiben von den sieben Elementen des ADR **fünf gebrauchte** übrig:
Resizable, Command Palette, Sheet, Segmented, ContextMenu. Toast und Menubar
fallen für alle Kandidaten gleich weg, nicht nur für den Eigenbau; sie zählen
deshalb in Kriterium 1 in keiner Spalte. Eine Datentabelle steht in der Liste
des ADR gar nicht und zählt ebenfalls nirgends: `history_table.dart` löst das
mit rund 890 Zeilen — `ListView.builder` mit bekannter `itemExtent`, ein
memoisierter Aufbau je Flow, ein `Semantics`-Knoten je Zeile statt elf.

Offen bleiben 3,5 Personentage Eigenbau: ContextMenu 1 d, Datum-Zeit-Wähler
2 d (Zeitraum-Filter des Audit-Screens, Sprint 4), senkrechte Achse des
Resizable 0,5 d. Diese Zahl ist die Bezugsgröße für alles Weitere.

### Der Punkte-Maßstab

Das Issue gibt 0 bis 3 Punkte je Kriterium vor, aber keine Bedeutung dafür.
Ohne Schwellen wäre jede Einzelwertung ein Prosa-Urteil, deshalb stehen sie
hier. Sie gelten für alle drei Spalten gleich.

| Kriterium | 3 Punkte | 2 | 1 | 0 |
|---|---|---|---|---|
| 1 Komponenten (von den fünf gebrauchten selbst geliefert) | fünf | vier | zwei oder drei | höchstens eines |
| 2 Getroffene Bugs | keine, und durch eigene Tests belegt | keine aufgezeichnet, aber unbelegt | mindestens ein offener Fehler in einer Komponente, die wir tragend einsetzen | mehr als drei, oder einer ohne Ausweichweg |
| 3 Breaking-Punkte in sechs Releases | null | 1 bis 5 | 6 bis 20 | mehr als 20 |
| 4 Theming-Passung | null Overrides, `HTokens` die einzige Farbquelle, durch Tests gebunden | einzelne Overrides, eine Quelle | eine zweite Farbquelle nötig, oder ohne Prototyp nicht entscheidbar | die Bibliothek erzwingt eine eigene Farbleiter |
| 5 Tastatur und Fokus | im Repository gemessen, Overlays durch Tests gedeckt | dokumentiert, nicht gemessen | Lücken dokumentiert | keine Fokusbehandlung |
| 6 Performance History | mit DevTools über 10 000 Zeilen gemessen, unter 16 ms | Verfahren belegt, nicht gemessen | wäre gegenüber heute ein Rückschritt | messbar über 16 ms |
| 7 Community und Wartung | über 50 Commits in 90 Tagen von mindestens drei Konten | mindestens 20 Commits von mindestens zwei Konten | weniger, oder alles an einem Konto | in 90 Tagen kein Commit |
| 8 Lizenz | OSI-anerkannt und mit GPL-3.0-only verträglich | verträglich mit Auflagen | unklar | unverträglich |
| 9 Wechselaufwand | null Tage | bis 3 Tage | bis 8 Tage | über 8 Tage |

### Die Matrix

Kriterien und Gewichte aus `backlog/sprint-2.md`, HUM-035. Höchstpunktzahl 60.
Versionsstand 2026-09-04: `shadcn_flutter` 0.0.54 (veröffentlicht 2026-08-27),
forui 0.26.0 (veröffentlicht 2026-08-24), eigene Schicht wie oben beschrieben.

| Kriterium | Gew. | Eigenbau | shadcn 0.0.54 | forui 0.26.0 |
|---|---:|---:|---:|---:|
| 1 Komponenten (fünf gebrauchte) | 3 | 2 | 3 | 2 |
| 2 Bugs in Sprint 1–2 getroffen | 3 | 3 | 1 | 2 |
| 3 Breaking Changes in den letzten sechs Releases | 2 | 3 | 1 | 0 |
| 4 Theming-Passung zu BACKLOG.md 5 | 3 | 3 | 1 | 1 |
| 5 Tastatur und Fokus | 3 | 3 | 2 | 2 |
| 6 Performance History | 2 | 2 | 1 | 2 |
| 7 Community und Wartung | 1 | 1 | 1 | 3 |
| 8 Lizenz | 1 | 3 | 3 | 3 |
| 9 Wechselaufwand | 2 | 3 | 0 | 0 |
| **Gewichtete Summe** | **20** | **53** | **29** | **31** |
| **Anteil** | | **88,3 %** | **48,3 %** | **51,7 %** |

Die Belege, Kriterium für Kriterium:

**1 Komponenten (Gewicht 3).** Gezählt werden die fünf gebrauchten Elemente,
für jede Spalte nach derselben Regel: Was die Spalte selbst liefert, zählt; was
sie nicht liefert, bliebe Eigenbau. `shadcn_flutter` liefert alle fünf —
`control/command.dart`, `layout/resizable.dart` (zehn Fundstellen von
`Axis.vertical`), `overlay/drawer.dart`, `form/multiple_choice.dart`,
`menu/context_menu.dart` — 5 von 5, 3 Punkte. forui liefert Resizable, Sheet,
ContextMenu und mit `tabs`/`select_group` einen Ersatz für Segmented, aber
keine Command Palette — 4 von 5, 2 Punkte. Der Eigenbau hat Command Palette,
Sheet, Segmented und das waagerechte Resizable, es fehlt das ContextMenu — 4
von 5, 2 Punkte. Eine großzügigere Lesart, die forui den fehlenden Command
durchgehen lässt, brächte dort 3 Punkte und 34 von 60; die Rangfolge bleibt.

**2 Getroffene Bugs (Gewicht 3).** `grep -rn "WORKAROUND\|TODO\|FIXME\|HACK"
app/packages/ui` findet null Treffer. Abgesichert ist das durch über 500
Aufrufe von `test(` und `testWidgets(`, dazu 26 `goldenTest(` in sechs Dateien
mit 49 abgelegten Golden-Bildern — 3 Punkte. Für beide Bibliotheken ist die
Zahl der getroffenen Bugs null, weil keine je eingebunden war; das ist kein
Qualitätsbeleg, und der Punktwert ist hier eine Schätzung, keine Messung. Die
einzige aufgezeichnete Evidenz steht in `backlog/sprint-1.md`, HUM-020:
`ResizablePane` in `shadcn_flutter` 0.0.54 hat die offenen Fehler #427 und
#428, mit `multi_split_view` als Ausweichweg — und das trifft genau die
Komponente, die das Intercept-Layout trägt. Deshalb 1 Punkt für
`shadcn_flutter` und 2 für forui, gegen das nichts Vergleichbares vermerkt ist.

**3 Breaking Changes (Gewicht 2).** Aus den Changelogs, Stand 2026-09-04, nach
der oben genannten Zählregel. `shadcn_flutter` 0.0.49 bis 0.0.54: 18
Breaking-Punkte, davon acht in 0.0.50 (das Auswahlmodell von `NavigationBar`
wechselt von Index auf Wert, mehrere Parameter entfallen) und zehn in 0.0.54
(Material und Cupertino aus dem Paket entfernt, `ShadcnApp` setzt keine
Material-Vorfahren mehr, Untergrenze Flutter 3.47.0). Uns beträfen elf davon:
die Navigation ist unsere Icon-Rail, und die Material-Entfernung trifft jede
App — 1 Punkt nach der Schwelle „6 bis 20". forui 0.24.0 bis 0.26.0, samt der
drei Patch-Versionen genau sechs Veröffentlichungen: 27 Breaking-Punkte, davon
13 in 0.24.0 (`FThemes` entfernt, alle vordefinierten Farbschemata bis auf
`FColors.neutralLight` und `FColors.neutralDark` entfernt,
`FBadgeContentStyle`, `FCardContentStyle`, `FDialogContentStyle` entfernt,
`FCard.raw` und `FDialog.raw` umbenannt), zehn in 0.25.0 und vier in 0.26.0
(Wechsel auf `material_ui` und `cupertino_ui`, `forui_assets` heißt jetzt
`forui_lucide`, Untergrenze Flutter 3.47.0). Das trifft Badge, Card, Dialog,
Theme und Textfeld, also genau unsere Fläche — 0 Punkte nach der Schwelle „mehr
als 20". Der Eigenbau hat kein Upstream und damit null — 3 Punkte.

Dazu ein Befund, der unabhängig von der Punktzahl gilt: **beide Bibliotheken
verlangen in ihrer aktuellen Fassung Flutter ≥ 3.47.0 und Dart ≥ 3.13.0.** Der
einzige Pin, den der CI-Job liest, ist `app/.fvmrc` mit `3.44.0`
(`.github/actions/setup-flutter`); `app/pubspec.yaml` führt die Untergrenze
zusätzlich als eigenen Constraint. Die jeweils letzte auf unserem Stand
lauffähige Fassung ist `shadcn_flutter` 0.0.53 (2026-07-14) und forui 0.25.0
(2026-08-02). Eine Aufnahme am Kopf beginnt also mit einer Flutter-Anhebung,
die alle 49 Goldens neu abnehmen lässt.

**4 Theming-Passung (Gewicht 3).** Das Kriterium misst `!important`-artige
Overrides in `HTokens`: null. Die Widget-Körper lesen 30 verschiedene
Token-Felder an rund 100 Stellen über rund 20 `HTheme.of(context)`-Aufrufe; die
einzigen sieben Farbliterale in den Körpern sind `Color(0x00000000)`, also
durchsichtig. Zustandsfarben kommen ausschließlich aus `FlowStateColor` und
`HStateColors`, mit getrennter Flächen- und Textvariante, und rund 30
Kontrast-Zusicherungen in `tokens_test.dart` binden sie an 3:1 für Flächen und
4,5:1 für Text auf beiden Leitern — 3 Punkte. forui hat in 0.24.0 alle
vordefinierten Farbschemata bis auf zwei entfernt und verweist auf einen
Generator (`dart run forui theme create`); ein erzeugtes Theme wäre eine zweite
Farbquelle neben `HTokens`, von Hand synchron zu halten — 1 Punkt, und dieser
eine ist ohne Prototyp begründbar. `shadcn_flutter` bringt ein `ColorScheme`
mit Radius und Typografie; unsere 30 Felder müssten als Adapter darübergelegt
werden, und was das mit den gemessenen Kontrasten macht, ist ohne Prototyp
**nicht** entscheidbar — der eine Punkt fällt dort unter die zweite Hälfte der
Schwelle und ist eine Schätzung.

**5 Tastatur und Fokus (Gewicht 3).** Rund 200 Vorkommen von
`LogicalKeyboardKey` in 18 Testdateien. `HModal` trägt `FocusScope` und bindet
`Escape` über `DismissIntent`; zehn der 21 Widgets haben eigene
Fokus-Behandlung; `HFocusRing` zeichnet den zwei Pixel breiten Akzentring
außerhalb des Controls, `HFocusRing.inline` auf der Kante der geteilten Pille.
Damit sind die Punkte 14, 16 und 17 aus `docs/UX.md` 9 geschlossen — 3 Punkte.
Für beide Bibliotheken gibt es keine Messung, weil der Prototyp-Branch nicht
gebaut wurde; je 2 Punkte nach der Schwelle „dokumentiert, nicht gemessen".

**6 Performance History (Gewicht 2).** `history_table.dart` ist ein
`ListView.builder` mit bekannter `itemExtent`, einem memoisierten Aufbau je
Flow und einem `Semantics`-Knoten je Zeile statt elf. Die DevTools-Messung über
10 000 Zeilen, die das Kriterium verlangt, steht aus — 2 Punkte statt 3.
`shadcn_flutter` bekommt 1 Punkt: Sein `layout/table.dart` wäre bei den
Semantics-Knoten ein Rückschritt gegenüber dem, was heute läuft. forui bekommt
ebenfalls 2 Punkte, nicht 1: Es bringt keine Tabelle mit, unser Verfahren
bliebe unverändert, und wo sich nichts ändert, darf die Zahl nicht fallen.

**7 Community und Wartung (Gewicht 1).** Zahlen der GitHub- und pub.dev-API vom
2026-09-04. `sunarya-thito/shadcn_flutter`: 936 Sterne, 26 offene Issues,
letzter Push 2026-08-27, 28 Commits seit 2026-06-06, alle 28 von einem einzigen
Konto; rund 12 300 Downloads in 30 Tagen, 469 Likes, 160 von 160 Punkten — 1
Punkt nach der Schwelle „alles an einem Konto". `duobaseio/forui`: 2 327
Sterne, 48 offene Issues, letzter Push 2026-09-03, 95 Commits seit 2026-06-06
von fünf Konten (54, 23 durch Renovate, 16, zweimal 1); rund 23 900 Downloads
in 30 Tagen, 432 Likes, 160 von 160 Punkten — 3 Punkte. Der Eigenbau hat keine
fremde Gemeinde: ein Konto, also 1 Punkt nach derselben Schwelle. Das ist die
einzige Zeile, in der der Eigenbau klar verliert.

**8 Lizenz (Gewicht 1).** `shadcn_flutter` steht unter BSD-3-Clause, forui unter
MIT mit OFL-1.1 für die mitgelieferten Schriften; beide sind OSI-anerkannt und
mit GPL-3.0-only verträglich. Die eigene Schicht ist bereits GPL-3.0-only. Je 3
Punkte; das Kriterium trennt nichts.

**9 Wechselaufwand (Gewicht 2).** Für den Eigenbau null Tage — 3 Punkte. Für
beide Bibliotheken 8,5 bis 11,5 Personentage nach der Aufstellung unten, also
über der Schwelle von acht Tagen — 0 Punkte.

### Die Entscheidungsregel

Das Issue schreibt: „bleiben, wenn shadcn ≥ 75 % der gewichteten Punkte und kein
Kriterium mit Gewicht 3 unter 1 Punkt; sonst wechseln." Die Regel wurde
geschrieben, als `shadcn_flutter` der Amtsinhaber sein sollte. Sie fällt in
beiden Lesarten gleich aus:

- Nach dem Buchstaben angewendet, auf `shadcn_flutter`: 48,3 % liegt unter
  75 %, also nicht bestätigen.
- Auf den tatsächlichen Amtsinhaber angewendet, die eigene Schicht: 88,3 %
  liegt über 75 %, und das schwächste Kriterium mit Gewicht 3 hat 2 Punkte
  (Komponenten), also bleiben.

### Wie belastbar das Ergebnis ist

Vier der neun Kriterien sind für die Bibliotheken **nicht gemessen**, weil der
Prototyp-Branch nicht gebaut wurde: 2 (Bugs), 4 (Theming), 5 (Tastatur und
Fokus) und 6 (Performance). Zusammen wiegen sie 11 der 20 Gewichtspunkte. Das
muss offen dastehen, sonst prüft die Matrix nichts, sondern rechtfertigt nur.

Setzt man diese vier für die Bibliotheken auf die volle Punktzahl — also den
für sie günstigsten unbelegten Fall —, dann kommt `shadcn_flutter` auf 48 von
60 (80,0 %) und forui auf 45 von 60 (75,0 %). **Beide erreichen damit die
Schwelle des Issues.** Der Eigenbau bleibt bei 53 von 60 und liegt weiter vorn,
aber die Regel allein trüge die Entscheidung in diesem Fall nicht mehr.

Was sie dann trägt, sind die Zeilen, die ohne Prototyp feststehen: Kriterium 3
aus den Changelogs (18 und 27 Breaking-Punkte in sechs Releases), Kriterium 7
und 8 aus den öffentlichen Registern, Kriterium 9 aus dem Umfang unseres
eigenen Codes, und die Flutter-Untergrenze 3.47.0, die keine Punktzahl hat und
trotzdem jeden Wechsel um zwei bis drei Tage teurer macht. Für Kriterium 4
lässt sich für forui zusätzlich ohne Prototyp argumentieren — ein erzeugtes
Theme wäre eine zweite Farbquelle neben `HTokens` —, für `shadcn_flutter`
lässt es sich nicht; dort bleibt die 1 eine Schätzung.

Die Gegenprobe in die andere Richtung: Streicht man Kriterium 9 ganz, weil ein
Amtsinhaber dort systematisch im Vorteil ist, bleiben 54 mögliche Punkte, und
der Eigenbau kommt auf 47 (87,0 %) gegen 29 (53,7 %) und 31 (57,4 %). Die
Entscheidung kippt also weder ohne den Amtsinhaber-Bonus noch im günstigsten
unbelegten Fall zu einem klaren Ergebnis für eine Bibliothek. Sie kippt in eine
Lage, in der ein Herausforderer die Schwelle knapp erreicht und dafür 8,5 bis
11,5 Tage verlangt, während der Bestand 3,5 Tage Restarbeit kostet. Das ist der
Satz, an dem die Entscheidung wirklich hängt — nicht die Prozentzahl.

### Was ein Wechsel gekostet hätte

Falls die Entscheidung später doch fällt, ist dies die Aufstellung. Kein Issue
davon wird angelegt; HUM-035b entfällt. Die Tagesangaben sind Größenordnungen
aus dem Umfang der betroffenen Dateien, keine gemessenen Werte; der größte
Posten rechnet mit rund 1 100 Zeilen Widget-Körper je Tag bei unveränderten
Signaturen.

1. Flutter-Pin von 3.44.0 auf 3.47.0 anheben (`app/.fvmrc`, `app/pubspec.yaml`),
   Umstellung auf `material_ui` und `cupertino_ui`, 49 Goldens neu abnehmen —
   2 bis 3 Tage.
2. `HTheme` auf das Theme der Bibliothek umstellen, `HTokens` als einzige
   Farbquelle erhalten — 1 bis 2 Tage.
3. Die 18 Widget-Körper umstellen (rund 3 300 Zeilen), Signaturen unverändert,
   damit kein Bildschirm es merkt — 3 Tage.
4. Goldens neu abnehmen und die Kontrast-Zusicherungen gegen die neuen Flächen
   prüfen — 1 bis 2 Tage.
5. Tastatur und Fokus in Overlays gegen die Tastentests nachziehen — 1 Tag.
6. Die Lokalisierung der Bibliothek registrieren (`ShadcnLocalizations`; sonst
   stürzt etwa der DatePicker unter `de` ab, `backlog/sprint-4.md` HUM-052) —
   0,5 Tage.

Summe 8,5 bis 11,5 Personentage, ohne den wiederkehrenden Upgrade-Tag pro
Sprint.

### Folgen dieser Entscheidung

- `app/packages/ui` bleibt auf `package:flutter/widgets.dart`. Weder
  `shadcn_flutter` noch forui stehen in `app/pubspec.yaml`; ein Eintrag wäre ein
  Architekturverstoß und wird im Review beanstandet.
- Die Kapselung bleibt bestehen und behält ihren Sinn: Features importieren
  `lib/core/ui/ui.dart`, nie ein fremdes Paket. Sie schützt jetzt nicht mehr vor
  einer Bibliothek, sondern hält die Tür für eine spätere offen.
- Der Upgrade-Tag pro Sprint entfällt. Der Puffer HUM-061 ist nicht mehr für
  shadcn-Breakage reserviert.
- Drei Issues bauen den Rest, jedes einzeln, Nummern beim Schnitt von Sprint 3:
  ein ContextMenu für JSON-Baum und Terminal (1 d), ein Datum-Zeit-Wähler für
  den Zeitraum-Filter (2 d), die senkrechte Achse für `HResizablePanes` (0,5 d).
- Die neun Widgets aus `app/lib/core/ui`, die als Handoff vorgemerkt sind
  (`HResizablePanes`, `HCollapsible`, `HDiagnosticCard`, `HoverLabel`,
  `HoldToConfirm`, `FixControl`, `FocusRing`, `ShellGlyphIcon`,
  `SectionPlaceholder`), wandern nach `app/packages/ui`. `FocusRing` dort und
  `HFocusRing` im Paket sind dieselbe Sache doppelt; beim Umzug bleibt eine.
- `HPill` und `HPanel` haben außerhalb der Galerie keinen Aufrufer. Entweder sie
  bekommen einen oder sie fallen weg; ein Wrapper ohne Nutzer ist Ballast, der
  bei jedem Umbau mitwandert.
- Die DevTools-Messung der History über 10 000 Zeilen steht weiter aus und
  gehört zu HUM-054.
- Die datenlastigen Pakete aus dem Abschnitt „Entscheidung" bleiben
  unberührt; `history_table.dart` zeigt allerdings, dass
  `two_dimensional_scrollables` für die History nicht gebraucht wird. Ob es für
  den JSON-Baum kommt, entscheidet HUM-030 und nicht dieser ADR.
- Der Satz im Abschnitt „Konsequenzen", der für den Fall einer Entscheidung
  zugunsten von forui einen Nachfolge-ADR ankündigt, ist damit erledigt: Die
  Entscheidung fällt zugunsten keiner Bibliothek, und die Begründung dafür steht
  unten unter „Verworfene Alternativen dieser Revision".
- Diese Entscheidung wird wieder aufgemacht, wenn eine der drei Bedingungen aus
  „Entschieden ohne Prototyp" eintritt.

**Stellen, die dieser Entscheidung noch widersprechen.** Sie sind hier
aufgezählt, damit niemand sie für gültig hält:

- `backlog/sprint-0.md` 1749 (HUM-008) und `backlog/sprint-4.md` 1476 (HUM-054)
  waren Akzeptanzkriterien, die das Gegenteil dieser Regel verlangten
  („shadcn_flutter exakt gepinnt", „shadcn_flutter-Bump auf die nächste
  Version"). Beide sind mit diesem ADR nachgezogen worden, minimal und ohne den
  Rest der Kästchen zu berühren.
- `backlog/sprint-2.md` (HUM-035) ist im Vorgehen, in den Schritten und im
  Akzeptanzkriterium zum Spike-Branch nachgezogen; die falsche Pfadangabe
  `docs/adr/0009-ui-kit.md` ist auf `0009-ui-stack.md` korrigiert.
- `backlog/sprint-0.md` (HUM-008, dazu die Flutter-Version 3.47.0 in den Zeilen
  115 und 208) und `backlog/sprint-1.md` (HUM-019, HUM-020) beschreiben
  `shadcn_flutter` in Kontext, Spezifikation und Fallstricken weiter als
  gesetzt. Das bleibt als Historie stehen: Es sind die Aufträge, unter denen
  gearbeitet wurde, und dieser ADR ist die Antwort darauf. Der Fallstrick zu
  `ResizablePane` in HUM-020 bleibt zusätzlich als Beleg für Kriterium 2 stehen.
- `backlog/sprint-4.md` (HUM-052 Fallstrick zur Lokalisierung, HUM-054 Bezug auf
  shadcn-Upgrades) und `backlog/sprint-5.md` (Risikotabelle mit
  shadcn-Breakage, Zeilen 709, 728, 748, 751; `find.byType(Dialog)` in HUM-058,
  Zeilen 398 und 415) betreffen Sprints, die noch nicht geschnitten sind. Sie
  werden beim Schnitt des jeweiligen Sprints nachgezogen, nicht vorher;
  `BACKLOG.md` 10 ist bereits umgestellt und gilt vor den Sprint-Dateien.

### Verworfene Alternativen dieser Revision

- **`shadcn_flutter` bestätigen.** 48,3 % der gewichteten Punkte, damit unter
  der Schwelle, die das Issue selbst gesetzt hat; im günstigsten unbelegten Fall
  80,0 %. Der breiteste Komponentensatz von allen, aber die Wartung hängt an
  einer Person, die Aufnahme beginnt mit einer Flutter-Anhebung, und die
  Komponente, die unser Intercept-Layout trägt, hat zwei offene Fehler.
- **Auf forui wechseln.** 51,7 %, im günstigsten unbelegten Fall 75,0 %: die
  beste Wartung im Feld, aber 27 Breaking-Punkte in sechs Releases, kein Ersatz
  für die Command Palette, und ein erzeugtes Theme als zweite Farbquelle neben
  `HTokens`.
- **Einzelne Komponenten aus einer Bibliothek ziehen** (etwa nur Command und
  ContextMenu). Bringt die ganze Abhängigkeit, das ganze Theme und die ganze
  Flutter-Untergrenze für zwei Widgets, die zusammen 1 Tag Eigenbau kosten.
- **Ein spezialisiertes Paket je Lücke** (`multi_split_view` für das Resizable
  und Ähnliches). Bleibt erlaubt und ist kein Widerspruch: Der Abschnitt
  „Entscheidung" holt die datenlastigen Widgets aus genau solchen Paketen. Für
  die drei offenen Lücken lohnt es nicht, weil jede unter zwei Tagen liegt und
  jede Zustandsfarbe und Fokusring aus unseren Token beziehen muss.
- **Den Spike doch bauen und dann entscheiden.** Der Weg, den das Issue
  vorgesehen hat. Verworfen mit der Begründung im Abschnitt „Entschieden ohne
  Prototyp": Die Zeitbox von einem Tag wäre vor der ersten Zeile Port für die
  Flutter-Anhebung draufgegangen, und die Bedingung, an der die Entscheidung
  hängt, steht ohne ihn fest. Der Preis dafür steht in derselben Zeile: vier
  Kriterien bleiben Schätzung.
- **Diesen ADR durch einen Nachfolger ersetzen.** Der Abschnitt „Konsequenzen"
  sieht das für den Fall vor, dass HUM-035 zugunsten von forui ausfällt. Er
  fällt zugunsten von keiner Bibliothek aus, und alles Übrige dieses ADR —
  Flutter, die Kapselung hinter `packages/ui`, kein WebView, ein Fenster mit
  gedockten Panes, riverpod mit Generator, freezed, die Spezialpakete — steht
  unverändert. Ein Nachfolge-ADR hätte 90 % des Textes wiederholt, um einen
  Paketnamen zu streichen.

## Revidiert am 2026-09-04 durch den Projekteigentümer

Der Abschnitt „Entscheidung nach Sprint 2" gilt nicht mehr. Er hat entschieden,
`shadcn_flutter` nicht aufzunehmen und die eigene Widget-Schicht zu behalten.
Der Projekteigentümer hat das am selben Tag überstimmt: **Flutter läuft auf der
jeweils neuesten stabilen Fassung, und `shadcn_flutter` wird aufgenommen.**

Warum die vorige Entscheidung falsch war, unabhängig vom Ergebnis: Dieser ADR
hat in seiner ursprünglichen Fassung `shadcn_flutter` als gesetzt benannt. Das
war eine Vorgabe des Projekteigentümers, kein Vorschlag. HUM-035 hat sie
revidiert, ohne zu fragen — und hat damit als Prüfung ausgegeben, was in
Wahrheit die Rücknahme einer fremden Entscheidung war. Eine Entscheidung, die
etwas zurücknimmt, das der Projekteigentümer gesetzt hat, geht ihm als Frage
vor, nicht als Ergebnis nach.

### Was jetzt gilt

Flutter steht auf **3.47.2** mit Dart **3.13.2**, gepinnt in `app/.fvmrc`, und
folgt künftig dem neuesten stabilen Stand. Blockiert ein Paket eine Anhebung,
wird das Paket benannt und gelöst, statt die Anhebung zu vertagen. Der Wechsel
kostete: `intl` von 0.20.2 auf 0.20.3, weil `flutter_localizations` in 3.47.2
seinen exakten Pin durch `^0.20.3` ersetzt; `freezed` auf 4.0.1,
`riverpod_generator` auf 4.0.9, `json_serializable` auf 6.14.1 und
`build_runner` auf 2.16.1. Die Deckel im Einzelnen, an den Pubspecs im Cache
gemessen: `riverpod_generator` 4.0.3 nimmt `analyzer ^9.0.0`, und `analyzer`
9.0.0 kennt Sprachversion 3.11, also zwei hinter Dart 3.13; `freezed` 3.2.5 und
`json_serializable` 6.13.0 nehmen `analyzer >=9 <11`, und 10.1.0 kennt 3.12,
also eine hinter. `build_runner` 2.15.1 hätte mit `analyzer >=8 <14` gereicht;
seine Anhebung ist Mitnahme, kein Zwang. Mitgesprungen sind ausserdem die
Laufzeit-Pakete `flutter_riverpod` auf 3.4.3 und `riverpod_annotation` auf
4.0.7 sowie `json_annotation` auf 4.12.0, das `json_serializable` exakt
vorschreibt. Der übrige Code
war unberührt: 3.45 und 3.46 führen keine Breaking Changes, und die drei aus
3.47 treffen uns nicht.

`shadcn_flutter` ist exakt auf 0.0.54 gepinnt und steht in
`app/packages/ui/pubspec.yaml`, nicht in `app/pubspec.yaml`. Die Naht bleibt
also: kein Feature importiert die Bibliothek, `tools/check-deps.sh` beanstandet
jeden `package:shadcn_flutter` ausserhalb von `app/packages/ui`, und was ein
Bildschirm sieht, sind weiterhin die `H*`-Widgets.

Sie bringt dreizehn weitere Pakete mit, und sie bündelt **8,3 MB eigener
Assets** in jeden Build: 4,4 MB Geist-Schriften in 33 Schnitten, 1,25 MB
Icon-Schriften, und 3,3 MB Länderflaggen aus `country_flags`. Flutter nimmt die
Schriften eines Pakets unabhängig davon mit, ob jemand sie importiert, das
Gewicht fällt also auch dann an, wenn wir nur einen Teil der Bibliothek
benutzen. Für eine Desktop-Anwendung ist das tragbar; es gehört trotzdem hier
notiert, weil `backlog/CONVENTIONS.md` 4.11 bis heute „Fonts werden nicht
gebündelt" sagte. Die Lock-Dateien sind seit dieser Anhebung versioniert: die
dreizehn Pakete kommen mit Caret-Bereichen, und ohne Lock änderte eine
Veröffentlichung eines davon den Bau ohne einen einzigen Commit.

### Was die Bibliothek nicht mitbringt

Vor der Aufnahme wurden die 165 Dateien des Pakets im Quelltext gelesen, nicht
die README. Das Ergebnis gehört hierher, weil es die Grenze zieht, bis zu der
man sich auf sie verlassen kann:

- Sie prüft nirgends ein Kontrastverhältnis. Es gibt eine Heuristik über die
  Helligkeit (`getContrastColor` in `lib/src/util.dart`), die aus einer Farbe
  eine helle oder dunkle Gegenfarbe wählt, aber kein Verhältnis nachrechnet.
  Der Destructive-Button im hellen Thema malt deshalb fest `Colors.white` auf
  ein Rot mit halber Deckkraft, also 1,97:1, mit einem Kommentar daneben, der
  das einräumt.
- Die gesamte Bibliothek hat drei `Semantics`-Knoten, keinen davon auf Button,
  Checkbox, Tabs oder Badge.
- `AnimationBehavior` kommt nicht vor. Jede ihrer Animationsdauern kollabiert
  auf fünf Prozent, sobald das System reduzierte Bewegung meldet; was an einem
  `Timer` hängt, etwa die Verzögerung der Hover-Karte, behält seine Zeit.
- Es gibt kein Halten-zum-Bestätigen, keine Mindest-Trefferfläche, und auf dem
  Desktop keinen gedrückten Zustand: die einzige Rückmeldung hängt an einem
  Schalter, der unter Linux ausgeschaltet ist.

Sie ist damit kein Radix: sie liefert Aussehen und Chrome, nicht die
Verhaltensschicht darunter. Das ist kein Einwand gegen ihre Aufnahme, sondern
die Beschreibung dessen, was `packages/ui` weiterhin selbst tun muss.

### Was im MVP gilt und was zu 1.0.0 zurückkommt

Der Projekteigentümer hat die Barrierefreiheit als Programm aus dem MVP
genommen: erst beweisen, dass es geht und gut sein kann. Screenreader-Semantik,
WCAG-Zahlen als Testgatter und die Prüfung bei doppelter Textskalierung sind
damit bis 1.0.0 vertagt.

Vier Dinge bleiben trotzdem, weil sie nur wie Barrierefreiheit aussehen:

1. **Die Haltedauer behält ihre Zeit.** Meldet das System reduzierte Bewegung,
   kürzt Flutter jede Animationsdauer auf fünf Prozent. Daran hängt bei uns die
   Sicherung, die verhindert, dass ein Klick eine Anfrage hinausschickt;
   gemessen genügten 60 Millisekunden. Das ist ein Sicherheitsfehler, der
   zufällig über dieselbe Einstellung läuft.
2. **Eine Untergrenze für Lesbarkeit.** 1,97:1 ist nicht „nicht barrierefrei",
   sondern unlesbar. Die Grenze bleibt, aber als Gestaltungsfrage und deutlich
   lockerer als die 4,5:1, die bis heute galten.
3. **Die Trefferflächen der Entscheidung.** Mindestens 28 × 28 px allgemein,
   mindestens 32 × 120 px für Erlauben und Blockieren. Die Bibliothek kennt
   keine Mindest-Trefferfläche; wer sich bei einer unumkehrbaren Handlung
   vergreift, hat kein Barrierefreiheitsproblem, sondern eine gesendete
   Anfrage.
4. **Ein sichtbarer gedrückter Zustand.** Ein Control, das auf einen Klick
   nichts tut, fühlt sich kaputt an.

## Betroffene Issues

`HUM-008` (Design-Tokens und `packages/ui`), `HUM-019` (Flutter-Shell),
`HUM-020` (Intercept-Screen v1), `HUM-030` (Body-Ansichten), `HUM-032`
(History-Screen mit virtualisierter Tabelle), `HUM-042` (Terminal mit xterm2),
`HUM-035` (Bestätigung oder Revision dieser Entscheidung), `HUM-054` (Golden-
und Widget-Tests), `HUM-061` (Puffer).
