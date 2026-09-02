# ADR-0009 · UI-Stack: Flutter mit shadcn_flutter, gekapselt hinter `packages/ui`
Status: Accepted
Datum: 2026-09-02

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

## Betroffene Issues

`HUM-008` (Design-Tokens und `packages/ui`), `HUM-019` (Flutter-Shell),
`HUM-020` (Intercept-Screen v1), `HUM-030` (Body-Ansichten), `HUM-032`
(History-Screen mit virtualisierter Tabelle), `HUM-042` (Terminal mit xterm2),
`HUM-035` (Bestätigung oder Revision dieser Entscheidung), `HUM-054` (Golden-
und Widget-Tests), `HUM-061` (Puffer).
