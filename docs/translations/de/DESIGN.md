---
source: DESIGN.md
source-sha256: 76795418b5e057fb04c624c908a511228e61cbf42b6ff04c1a50b7d9047c3e65
lang: de
---
# Design

Applies to: Humanitl 0.1

Das ist das Designsystem der Desktop-App von Humanitl: die Richtung, die
Tokens, das Widget-Vokabular und die wenigen Regeln, die alles zusammenhalten.
Es richtet sich an alle, die in `app/` mitarbeiten.

Zwei andere Dokumente decken benachbartes Gelände ab und werden hier nicht
wiederholt:

- [docs/UX.md](UX.md) (deutsch) beschreibt Bildschirm für Bildschirm, wie
  Bewegung, Farbe, Text und Tastatur zusammenwirken, mit der Begründung hinter
  jeder Regel. Wo diese Seite einen Wert nennt, sagt UX.md, wie er eingesetzt
  wird.
- [docs/GLOSSARY.md](GLOSSARY.md) (deutsch) ist die verbindliche Liste der
  UI-Begriffe auf Englisch und Deutsch.

Die Werte leben im Code, in [`app/packages/ui`](../app/packages/ui). Wenn diese
Seite und der Code sich widersprechen, gewinnt der Code, und diese Seite ist
veraltet. Die Token-Tabellen unten sind die Ausgabe von
[`app/packages/ui/tool/dump_tokens.dart`](../app/packages/ui/tool/dump_tokens.dart);
erzeuge sie neu, statt sie von Hand zu bearbeiten:

```sh
cd app/packages/ui && flutter test tool/dump_tokens.dart
```

Das Skript gibt die Tabellen zwischen den Markierungen `BEGIN TOKENS` und
`END TOKENS` aus. Flutter meldet danach „No tests ran" und endet mit Code 79;
das ist erwartet, das Skript hat keine Testfälle.

Die Bildschirmfotos auf dieser Seite sind Kopien der Golden-Bilder in
`app/test/goldens/goldens/ci/`. Das Golden-Gerüst rendert jedes Textzeichen als
vollen Block (die Testschrift Ahem), die Bilder zeigen also Geometrie, Farbe und
Zustand, nicht den Wortlaut.

## Inhalt

- [Richtung „Schleuse"](#direction-airlock)
- [Tokens](#tokens)
- [Komponenten](#components)
- [Layout](#layout)
- [Interaktion](#interaction)
- [Bewegung](#motion)
- [Markenzeichen](#signature-elements)
- [Sprache](#language)
- [Anti-Patterns](#anti-patterns)

## Richtung „Schleuse"

### Stimmung

Ein ruhiger Leitstand. Jede Anfrage des Agenten ist ein Paket in einer Schleuse:
Die Oberfläche macht das Warten sicher und die Entscheidung offensichtlich. Die
Fläche ist dicht, einfarbig und präzise. Farbe erscheint nur, wenn sich ein
Zustand ändert, und dann bedeutet sie diesen Zustand und sonst nichts.

### Vorbilder

| Produkt | Was wir davon übernehmen |
|---|---|
| Linear | Eine getönte neutrale Fläche, 13 px Dichte, Tastatur zuerst |
| Raycast | Die Befehlspalette als wichtigster Weg zu handeln |
| Zed | Haarlinien statt Schatten, eine ruhige Statusleiste |
| Little Snitch | Erlauben oder verweigern, mal Gültigkeit, mal Ziel, mit einem Countdown |

### Gegenbilder

| Muster | Warum nicht |
|---|---|
| Burp und ZAP | Visueller Lärm: Jeder Bereich schreit, nichts ist das eine Wichtige |
| Neon | Farbe als Dekoration konkurriert mit Farbe als Zustand |
| Terminal-Grün | Grün bedeutet hier „erlaubt"; es kann nicht zugleich die Grundfarbe sein |
| Modale Dialoge für Entscheidungen | Eine Entscheidung fällt in der Warteschlange; ein Modal ist destruktiven Bestätigungen vorbehalten |

## Tokens

Alle Tokens erreicht man über `HTokens`, veröffentlicht von `HTheme` und gelesen
mit `HTheme.of(context)`. Es gibt eine dunkle und eine helle Instanz; das dunkle
Theme ist das, für das das Design gezeichnet ist. Skalen (Schrift, Abstände,
Radien, Größen, Bewegung) teilen sich beide Themes. Die Konstanten dahinter sind
`HColors`, `HType`, `HSpace`, `HRadius`, `HSize` und `HMotion` in
[`app/packages/ui/lib/src/tokens/`](../app/packages/ui/lib/src/tokens).

### Farbregeln

- Dunkel zuerst, mit neutralen Grautönen (R = G = B). Bis HUM-355 waren die
  Neutraltöne kühl getönt (Farbton um 230); neben dem Markengelb wirkte das
  kalt, deshalb ist jetzt jede Stufe von Fläche, Linie und Text unbunt, mit der
  Leuchtdichte der alten Stufe, und ein Token-Test hält die OKLab-Buntheit jedes
  Grautons unter 0,004.
- Ein Akzent, das Markengelb `#E8B84B`; im hellen Theme der Ocker `#A57A1C`.
  Text und Glyphen auf dem Akzent sind dunkel, `#161616`, in beiden Themes. Der
  Akzent markiert Fokus, den primären Knopf, Auswahl und Links, und sonst
  nichts. Vor HUM-260 war der Akzent das Blau `#7C9CF5`, das danach bis HUM-355
  die Farbe `info` war.
- Kein Blau neben dem Gelb (HUM-355). `held` ist ein helles Orchidee, `info` ein
  neutrales Silber (dunkel) oder Graphit (hell). Blau und Cyan überleben nur in
  der Terminal-Palette, weil ein Agent, der `ESC [ 34 m` schreibt, Blau meint.
- Zustandsfarben bedeuten Zustände. Rot bedeutet blockiert und sonst nichts,
  deshalb sind Fehler und Secret-Funde orange.
- Tönungen haben höchstens 10 Prozent Alpha (`HColors.tintAlpha`).
- Jede Farbe hat eine Flächen- und eine Textvariante. Flächen (Leisten, Bögen,
  Glyphen, Füllungen) müssen 3:1 gegen jede Oberfläche erreichen; Wörter müssen
  4,5:1 erreichen, auch auf ihrer eigenen Tönung. Die Textvarianten leitet
  `HColorDerivation.textVariant` zur Laufzeit ab, nie werden sie von Hand
  geschrieben.
- Die hellen Zustandsfarben leitet `HColorDerivation.lightState` aus den dunklen
  ab: 12 Prozent weniger HSL-Helligkeit, dann in Schritten von 1 Prozent weiter
  abgesenkt, bis die Farbe auf allen vier hellen Oberflächen 3:1 erreicht.
- Farbe ist nie der einzige Kanal. Jeder Zustand hat auch eine Glyphe (siehe
  [`HStateGlyph`](#hstateglyph)), weil `allowed` und `blocked` im hellen Theme
  unter Deuteranopie 1,01:1 messen.

Die Regeln hinter diesen Zahlen und die Liste der Oberflächen, an denen jede
Farbe gemessen wird, stehen in
[docs/UX.md, Abschnitt 3.3](UX.md#33-colour-means-state) und
[Abschnitt 6](UX.md#6-accessibility-as-numbers).

### Oberflächen, Text und Akzent

| Token | Dunkel | Hell |
|---|---|---|
| `bg0` | `#111111` | `#FBFBFB` |
| `bg1` | `#171717` | `#FFFFFF` |
| `bg2` | `#1E1E1E` | `#F5F5F5` |
| `bg3` | `#262626` | `#ECECEC` |
| `line` | `#2E2E2E` | `#E5E5E5` |
| `lineStrong` | `#3D3D3D` | `#CFCFCF` |
| `fg0` | `#E8E8E8` | `#181818` |
| `fg1` | `#A8A8A8` | `#515151` |
| `fg2` | `#707070` | `#828282` |
| `accent` | `#E8B84B` | `#A57A1C` |
| `accentText` | `#E8B84B` | `#755714` |
| `accentFill` | `#E8B84B` | `#A57A1C` |
| `onAccent` | `#161616` | `#161616` |
| `warning` | `#E8B84B` | `#A57A1C` |
| `warningText` | `#E8B84B` | `#755714` |
| `info` | `#E0E0E0` | `#404040` |
| `infoText` | `#E0E0E0` | `#404040` |

Wofür die Oberflächen da sind: `bg0` ist der Hintergrund der Anwendung, `bg1`
ein Bereich, `bg2` eine erhöhte Fläche in einem Bereich (Zeilen, Eingaben,
Hover), `bg3` die höchste Fläche (Auswahl, gedrückt). `fg0` ist primärer Text,
`fg1` sekundärer, `fg2` tertiärer und wirklich deaktivierter. `warning` hat
absichtlich den Wert des Akzents (Gelb bedeutet Vorsicht, wie auf
Absperrband), deshalb kommt eine Warnung immer mit einer Glyphe oder einem Wort,
nie als Farbe allein.

### Zustandsfarben

| Zustand | Fläche dunkel | Text dunkel | Fläche hell | Text hell |
|---|---|---|---|---|
| `held` | `#D8B1E1` | `#D8B1E1` | `#B56BC6` | `#883B9A` |
| `allowed` | `#4FBF8C` | `#4FBF8C` | `#36956A` | `#26684A` |
| `allowedEdited` | `#57B99F` | `#5BBBA1` | `#3E957E` | `#2B6656` |
| `blocked` | `#E5646E` | `#EB878F` | `#DC303D` | `#A81C27` |
| `timedOut` | `#909090` | `#A7A7A7` | `#717171` | `#5A5A5A` |
| `autoRule` | `#994FBF8C` | `#99E8F7F0` | `#991C4E38` | `#99020705` |
| `passthroughLlm` | `#B48AF0` | `#BD98F2` | `#9154E9` | `#7122E3` |
| `error` | `#F0784F` | `#F28A66` | `#EB4D17` | `#A2340E` |

Achtstellige Werte sind `#AARRGGBB`: `autoRule` ist `allowed` mit 60 Prozent
Alpha. `held` ist seit HUM-355 ein helles Orchidee; von HUM-260 bis HUM-355 war
es das Himmelscyan `#79C9EC`, das neben dem Gelb als Blau wirkte, und davor das
Bernstein `#E0B24A`, das heute mit dem gelben Akzent kollidieren würde. Das
Orchidee ist der eine Farbton, der in beiden Themes, auch unter Protanopie und
Deuteranopie, einen OKLab-Abstand von mindestens 0,09 zu jeder anderen
Zustandsfarbe und zum Akzent hält. `allowedEdited` ist
ein eigenes Blaugrün und trägt in seiner Glyphe einen Stiftpunkt im Akzent.

Jede Zeile des Bildes unten zeigt von links nach rechts: die Zustandsleiste, die
Glyphe, die Beschriftung in der Textfarbe, das Flächenmuster, die Tönung mit
10 Prozent und das Textmuster.

| Dunkel | Hell |
|---|---|
| ![Zustandspalette, dunkel](img/design-state-palette-dark.png) | ![Zustandspalette, hell](img/design-state-palette-light.png) |

### Methodenfarben

HTTP-Methoden leihen sich Farbtöne, sind aber nie Zustände: Ein `DELETE` ist
keine Blockade.

| Methode | Fläche dunkel | Text dunkel | Fläche hell | Text hell |
|---|---|---|---|---|
| GET, HEAD | `#E8B84B` | `#E8B84B` | `#A57A1C` | `#755714` |
| POST | `#B48AF0` | `#BD98F2` | `#9154E9` | `#7122E3` |
| PUT, PATCH | `#D8B1E1` | `#D8B1E1` | `#B56BC6` | `#883B9A` |
| DELETE | `#B3E5646E` | `#B3F5C4C8` | `#B3D42431` | `#B3560F14` |
| andere | `#707070` | `#9E9E9E` | `#828282` | `#5C5C5C` |

`DELETE` ist der Farbton für blockiert mit 70 Prozent Alpha. In einer Liste steht
das Methoden-Badge neutral auf `bg2` (`HMethodBadge(neutral: true)`), damit ein
rötliches Badge neben einer roten Leiste nicht als zwei Blockaden gelesen wird.

### Terminal-Palette

Das Sandbox-Terminal bildet die 16 ANSI-Farben auf dieselbe Leiter ab
(`HTerminalPalette`, HUM-042).

| Platz | Dunkel | Hell |
|---|---|---|
| Hintergrund | `#111111` | `#FBFBFB` |
| Vordergrund | `#E8E8E8` | `#181818` |
| Cursor | `#E8B84B` | `#A57A1C` |
| Auswahl | `#1AE8B84B` | `#1AA57A1C` |
| Schwarz | `#1E1E1E` | `#CFCFCF` |
| Rot | `#E5646E` | `#DC303D` |
| Grün | `#4FBF8C` | `#36956A` |
| Gelb | `#E8B84B` | `#A57A1C` |
| Blau | `#6F8FEF` | `#3864E9` |
| Magenta | `#B48AF0` | `#9154E9` |
| Cyan | `#79C9EC` | `#1C91C5` |
| Weiß | `#A8A8A8` | `#515151` |
| helles Schwarz | `#707070` | `#828282` |
| helles Rot | `#EC9097` | `#BA1F2B` |
| helles Grün | `#74CDA4` | `#28704F` |
| helles Gelb | `#EECA78` | `#795A15` |
| helles Blau | `#9DB3F4` | `#1847D6` |
| helles Magenta | `#D1B7F6` | `#7427E3` |
| helles Cyan | `#A6DBF2` | `#167098` |
| helles Weiß | `#E8E8E8` | `#181818` |

### Typografie

Zwei Familien. Inter für die Oberfläche, mit Tabellenziffern (`tnum`) und dem
einstöckigen `cv11`. JetBrains Mono für URLs, Header, Bodies, Hashes, Regeln und
Methoden-Badges, ohne Ligaturen und mit Tabellenziffern. Die Schriftdateien
werden nicht mitgeliefert; jede Familie hat einen Fallback-Stapel
(`HType.uiFallback`, `HType.monoFallback`).

| Token | Familie | Größe/Zeile (px) | Gewicht |
|---|---|---|---|
| `ui11` | Inter | 11/16 | 400 |
| `ui12` | Inter | 12/16 | 400 |
| `ui13` | Inter | 13/20 | 400 |
| `ui14` | Inter | 14/22 | 400 |
| `ui16` | Inter | 16/24 | 400 |
| `ui20` | Inter | 20/28 | 400 |
| `mono11` | JetBrains Mono | 11/16 | 400 |
| `mono12` | JetBrains Mono | 12/16 | 400 |
| `mono13` | JetBrains Mono | 13/20 | 400 |
| `mono14` | JetBrains Mono | 14/22 | 400 |

Gewichte sind 400, 500 (`.medium`) und 600 (`.semibold`). Es gibt kein 700. Die
Zeilenlänge ist auf 90 Monospace-Zeichen begrenzt (`HSize.measureChars`);
überschüssige Breite eines Bereichs wird Rand, nie Zeilenlänge.

### Abstände

Alles ist ein Vielfaches der Grundeinheit von 4 px.

| Token | px |
|---|---|
| `unit` | 4 |
| `x1` | 4 |
| `x2` | 8 |
| `x3` | 12 |
| `x4` | 16 |
| `x5` | 20 |
| `x6` | 24 |
| `x7` | 28 |
| `x8` | 32 |
| `panelPadding` | 12 |

### Radien

| Token | px | Verwendet für |
|---|---|---|
| `control` | 4 | Knöpfe, Pillen, Eingaben |
| `card` | 6 | Karten, Modals, Sheets |
| `panel` | 0 | Bereiche: eckig |
| `badge` | 2 | Badges |

### Größen

| Token | Wert (px) |
|---|---|
| `headerBar` | 40 |
| `statusBar` | 24 |
| `paneHeaderHeight` | 48 |
| `railItemHeight` | 48 |
| `borderWidth` | 1 |
| `row` | 36 |
| `rowSelected` | 56 |
| `rowHistory` | 28 |
| `rowBody` | 24 |
| `rowActionSlot` | 28 |
| `hitMin` | 28 |
| `hitDecision` | 120 x 32 |
| `paneMinQueue` | 280 |
| `paneMinInspector` | 480 |
| `paneMinContext` | 260 |
| `paneRatio` | 28/44/28 (Prozent) |
| `terminalStrip` | 240 x 28 |
| `measureChars` | 90 (Zeichen) |
| `stateRail` | 4 |
| `selectionRail` | 2 |
| `cautionTape` | 4 |
| `hairline` | 1 |
| `splitter` | 7 |
| `glyph` | 16 |
| `tick` | 14 |
| `ringStroke` | 1.5 |

Zeilenhöhen sind Mindesthöhen, keine festen Höhen: Bei einer Textgröße von 2,0
wächst eine Zeile mit ihrem Text. `rowSelected` (56) ist heute die zweizeilige
Detailzeile im Verlauf; eine Zeile der Warteschlange bleibt in jedem Zustand
36 px hoch.

### Bewegungs-Tokens

| Token | Wert |
|---|---|
| `enter` | cubic-bezier(0.2, 0.0, 0.0, 1.0) |
| `exit` | cubic-bezier(0.4, 0.0, 1.0, 1.0) |
| `arrive` | 180 ms |
| `press` | 120 ms |
| `sweep` | 200 ms |
| `leave` | 220 ms |
| `ruleDraw` | 240 ms |
| `breathe` | 1200 ms |
| `holdToConfirm` | 400 ms |
| `holdToBlock` | 250 ms |
| `stagger` | 30 ms |
| `rearm` | 350 ms |
| `checkStagger` | 120 ms |
| `cautionTape` | 1200 ms |
| `arriveOffset` | 8 px |
| `leaveOffset` | 12 px |
| `breatheBelow` | 0.2 |
| `breatheMinOpacity` | 0.72 |

Wie sich diese zu den sichtbaren Mikrointeraktionen verbinden, steht unter
[Bewegung](#motion).

## Komponenten

Ein Bildschirm baut sein Aussehen nur aus den `H*`-Widgets von
`app/packages/ui`. Dahinter steht `shadcn_flutter`, exakt festgelegt; nur dieses
Paket darf es importieren (`tools/check-deps.sh` erzwingt das). `HTheme` baut das
Theme der Bibliothek aus `HTokens`, es gibt also eine Palette, nicht zwei. Die
Entwickler-Galerie `HGalleryPage` zeigt jedes Token und jedes Widget in beiden
Themes.

Jedes interaktive Widget ist ein Fokusstopp, mindestens `hitMin` (28 px) hoch,
und zeigt Fokus mit `HFocusRing`. Interaktionszustände, die ein Golden-Test mit
einem Zeiger nicht herstellen kann, lassen sich mit einem Argument `preview`
erzwingen (nur Galerie und Tests).

### HButton

Vier Rollen, keine fünfte (`HButtonVariant`):

| Variante | Aussehen | Verwendung |
|---|---|---|
| `primary` | Gefüllt mit `accentFill`, Text in `onAccent` | Die eine Aktion, die eine Fläche will |
| `secondary` | Getönte Fläche mit einer Haarlinie | Eine zweite, gleichrangige Aktion |
| `ghost` | Keine Fläche, bis der Zeiger darüber ist | Leise Aktionen, der Isolationsring |
| `danger` | Der Farbton für blockiert | Destruktive Aktionen, auch Blockieren |

Größen: `sm` hat 28 px Mindesthöhe, `md` 32 px. Zustände: Ruhe, Hover,
gedrückt, fokussiert (Ring außerhalb des Knopfs, 2 px Abstand beim primären
Knopf), deaktiviert (`onPressed: null`). Die Füllung beim Drücken läuft `press`
(120 ms) lang und behält ihre volle Dauer auch bei reduzierter Bewegung.

### HIconButton

Eine Glyphe, die zugleich eine Klickfläche von 28 px ist, für Schließen und
überall, wo eine Beschriftung Lärm wäre. Leise Rolle: in Ruhe keine Fläche,
`bg2` unter dem Zeiger, `bg3` beim Drücken, Fokusring außen. Hat immer ein
`semanticsLabel`.

### HPill

Die geteilte Pille hinter dem [Freigabeventil](#release-valve). Linke Hälfte:
die schlichte Aktion einmal; wer sie `holdToConfirm` (400 ms) lang hält, füllt
sie und führt die gemerkte Variante aus. Rechte Hälfte: ein Chevron, der das
Raster aus Gültigkeit und Ziel öffnet. Beide Hälften sind eigene Fokusstopps;
gehaltenes Enter oder Leertaste tut dasselbe wie der gehaltene Zeiger.
Zustände: Ruhe (6 Prozent Füllung), Hover, gedrückt, Halten (die Füllung wächst
auf 20 Prozent), fokussiert je Hälfte.

### HRow

Eine Zeile der Warteschlange, der Regelliste oder des Verlaufs. Drei Dichten:
`row` (36), `rowHistory` (28), `rowBody` (24), alles Mindesthöhen.

| Zustand | Aussehen |
|---|---|
| Ruhe | 4 px Zustandsleiste links, Glyphe, Titel, Inhalt am Ende |
| Hover | Füllung `bg2`; der Aktionsplatz rechts zeigt seine Aktion |
| Ausgewählt | Füllung `bg3`; der Akzent ersetzt die Zustandsleiste auf vollen 4 px |
| In der Auswahl | Akzentleiste ohne die Cursor-Füllung (Mehrfachauswahl) |
| Fokussiert | Ring auf dem eigenen Rand der Zeile gezeichnet (`HFocusRing.inline`) |

Hover und Auswahl teilen sich nie eine Füllung: Enter sendet die ausgewählte
Zeile, und Senden lässt sich nicht rückgängig machen. Der Aktionsplatz ist immer
reserviert (28 px) und verschiebt nie Text.

| Angehalten | Hover | Ausgewählt |
|---|---|---|
| ![Zeile der Warteschlange, angehalten](img/design-row-held.png) | ![Zeile der Warteschlange, Hover](img/design-row-hover.png) | ![Zeile der Warteschlange, ausgewählt](img/design-row-selected.png) |

### HStateGlyph

Die Glyphe eines Flow-Zustands, 16 px, wahlweise in einem Countdown-Ring
(`ringStroke` 1,5 px). Glyphe je Zustand: `held` Sanduhr, `allowed` Pfeil nach
rechts oben, `allowedEdited` Pfeil mit Akzentstift, `blocked` Schild mit Kreuz,
`timedOut` Uhr mit Kreuz, `autoRule` Blitz, `passthroughLlm` doppelter Chevron,
`error` Warndreieck. Der Ring zeigt das verbleibende Haltebudget als Bogenlänge;
verbrauchte Zeit ist eine Lücke, keine Spur. Er wird nie rot. Unter 20 Prozent
und noch einmal unter 5 Prozent atmet die Glyphe dreimal, nie unter 72 Prozent
Deckkraft, dann ruht sie. Bei reduzierter Bewegung ersetzt ein zweiter,
stillstehender Ring mit 40 Prozent Alpha das Atmen.

### HGlyphIcon

Zeichnet eine `HGlyph` (17 Formen nach den Lucide-Icons) in einer Größe und
Farbe. Die Glyphen werden gemalt, nicht aus einer Icon-Schrift geladen.

### HBadge und HMethodBadge

`HBadge` ist eine kleine getönte Beschriftung: 11/500, Radius 2, 10 Prozent
Flächentönung, Chip 18 px hoch in reservierten 28 px. Flächenfarbe und Textfarbe
sind getrennt (`color` und `textColor`); der Text nimmt als Vorgabe die
Textvariante der Flächenfarbe. Mit `onTap` wird es zum Fokusstopp. `mono: true`
nimmt die Monospace-Familie für Protokoll-Tokens.

`HMethodBadge` ist ein `HBadge` in Großbuchstaben und Monospace im Farbton der
Methode, oder in Listen neutral auf `bg2`.

### HTextField

Eine einzeilige Eingabe auf `bg2` mit Haarlinienrand und dem Akzent als Cursor.
Als Vorgabe Monospace, weil die meisten Eingaben mit etwas verglichen werden
(Hosts, Pfade). Zustände: leer mit Platzhalter in `fg2`, gefüllt, fokussiert
(Ring von der Bibliothek mit unseren Maßen gezeichnet: 2 px Akzent, 2 px
Abstand), deaktiviert. `onChanged` meldet nur, was ein Mensch getippt hat.

### HCheckbox

Ein Kästchen mit seiner Beschriftung und einem optionalen Satz `hint`, der sagt,
was das Ankreuzen kostet. Kästchen 14 px (`tick`). Zustände: aus, an
(Akzentfüllung, Häkchen in `onAccent`), fokussiert, deaktiviert (alles in
`fg2`, kein Akzent).

### HSegmented und HChoiceChips

`HSegmented` ist eine Reihe sich gegenseitig ausschließender Optionen;
`HChoiceChips` ist eine Reihe unabhängiger Schalter. Die gewählte Option bekommt
die höchste Fläche und primären Text, nie die Akzentfüllung: Der Akzent gehört
der einen Aktion eines Bildschirms. Jedes Segment ist ein eigener Fokusstopp,
mindestens 28 px hoch. `HSegment` und `HSegmentOption` sind ihre Bausteine.

### HPanel

Eine dauerhafte Region der Shell: `bg1`, Haarlinienrand, kein Radius, kein
Schatten. Optionaler Titel (13/600) mit Aktionen, darunter eine Haarlinie, 12 px
Innenabstand im Inhalt.

### HSheet

Ein Bereich, der von rechts hereingleitet, als Vorgabe 360 px breit: Regel aus
Anfrage, Verlaufsdetail, Isolationsdetails, Einstellungen. Es stellt nie eine
Frage, die die Warteschlange blockieren würde. Es hält den Fokus, solange es
offen ist, und schließt mit Escape. Titel 16/600, Schließen ist ein
`HIconButton`.

### HModal

Eine zentrierte Karte (Vorgabe 420 px) über einem Scrim. Destruktiven
Bestätigungen vorbehalten: mehr als fünf Flows auf einmal blockieren, eine Regel
mit Gültigkeit „Immer" löschen, eine laufende Sandbox stoppen. Hält den Fokus
gefangen, gibt ihn beim Schließen zurück, Escape schließt es, wenn das Modal
schließbar ist. Titel 16/600, Aktionen rechtsbündig.

### HPopover

Ein kleiner Kasten mit Pfeil, der ein Bedienelement einmal erklärt (ein
Coach-Mark). Er steht im Layout neben seinem Ziel, nie darüber, nimmt keinen
Fokus und bindet keine Taste. Er erscheint mit `arrive` und `arriveOffset`, ohne
zu blinken. Pfeil nach oben oder unten (`HPopoverArrow`).

### HNoticeLayer und HNoticeCard

Hinweise schweben; sie nehmen nie ein Band des Layouts ein (HUM-302).
`HNoticeLayer` legt sie oben rechts über ihre Region, 8 px von den Rändern: für
globale Hinweise das Fenster unterhalb der Kopfzeile der Bereiche (in der echten
Höhe der Kopfzeile, die mit der Textgröße wächst), für ihre Funde die Fläche
unter dem Kopf der Sandbox (nie über den unteren beiden Zeilen des Terminals).
Die Karten der Warteschlange stehen unten rechts in der Listenfläche, damit die
neuesten Zeilen oben anklickbar bleiben. Eine Karte ist so breit wie die Region
abzüglich ihrer Einzüge, höchstens 420 px
(`noticeMaxWidth`); Karten stapeln sich mit 8 px Abstand und scrollen ab einer
begrenzten Höhe. Die Region behält ihre volle Größe, also verschiebt sich nichts,
und kein Terminal wird in der Größe verändert, wenn ein Hinweis kommt oder geht.
`HNoticeCard` ist die Fläche: `bg2`, Haarlinie in `lineStrong`, Radius `card`,
eine optionale Zustandsleiste, kein Schatten. Ihr Schließen (`HNoticeDismiss`)
ist ein `HIconButton` in der Karte, 4 px von ihrer oberen rechten Ecke, mit
seiner Beschriftung beim Hover. Nur die erste Zeile der Karte (ihr `header`, die
Badges einer Diagnose) hält sich davon frei; der Text darunter läuft bis an den
inneren Rand. Eine Diagnose als Hinweis ist eine `HDiagnosticCard` mit
`dense: true` und demselben Schließen in ihrem Platz `dismiss`. Hinweise nehmen
keinen Fokus, binden keine Taste und schließen nicht mit Escape; globale
Hinweise warten, solange irgendeine Ebene (`HLayerScope.openLayers`) offen ist.

### HKeepClear und das schwebende Terminal

`HKeepClear` markiert eine Region, die nichts Schwebendes verdecken darf
(HUM-300): den Bereich der Warteschlange, die Aktionsleiste von Anhalten, jede
Hinweisspalte eines `HNoticeLayer`, jedes `HSheet` und jedes `HModal` in der
Arbeitsfläche (die Befehlspalette nimmt sich aus, sie liegt über allem). Es
zeichnet nichts und nimmt weder Zeiger noch Fokus; eine Region in einem
verborgenen Abschnitt oder ohne Höhe zählt als nichts.
Das schwebende Terminal ist heute das einzige schwebende Element: ein Fenster
nur über den Abschnitten (nie über Leiste, Kopfzeile oder Statusleiste), `bg1`
mit einer Haarlinie in `lineStrong` und dem Radius `card`, kein Schatten. Eine
Titelleiste von 28 px in `bg2` (Titel `ui12` halbfett, daran wird gezogen) trägt
einen Ghost-Knopf zurück zum Abschnitt Sandbox und ein `HIconButton` zum
Schließen; darunter das Terminal mit seinem üblichen Banner, das dort nur den
Knopf für das ganze Fenster behält. Ein Griff von 14 px in der Ecke des Fensters
gegenüber seinem Anker ändert seine Größe zwischen 320 x 200 und 60 % der
Abschnitte (höchstens 960 x 640), an jedem Anker gleich; auf einer mittleren
Achse sitzt er rechts oder unten, und das Fenster wächst nach beiden Seiten.
Sitzt der Griff in der oberen Reihe, lassen Titelleiste und Banner auf seiner
Seite seine 14 px frei (HUM-352). Das Fenster hält sich an einen von neun Ankern
(die Ecken, die Mitten der Ränder, die Mitte), 12 px (`x3`) von den Rändern, an
die es sich hält, als Vorgabe unten rechts. Ruhig losgelassen, rastet es am
nächsten Anker ein; geworfen (ab 400 px/s), am nächsten Anker in Wurfrichtung,
und es gleitet in `arrive` dorthin (HUM-352). Ohne freie Ecke schrumpft es zu
einem Titelstreifen von `terminalStrip`
(240 x 28, HUM-315): `bg2` mit der Haarlinie in `lineStrong` und dem Radius
`card` wie das Fenster, die Bild-im-Bild-Glyphe, „Profil · Tab" in `ui12`
halbfett und ein Ghost-Knopf mit der Glyphe zum Maximieren, der es wieder
öffnet. Der Streifen nimmt die erste freie Ecke in derselben Reihenfolge wie das
Fenster; auf der Seite Sandbox weichen Fenster und Streifen dem Bereich.
Hinweise im Fenster (`HKeepClearExempt`) halten nichts von ihm frei, und die
Leiste mit Senden und Blockieren hält sich nur frei, solange eine Anfrage
wartet.

### HContextMenu

Das Rechtsklick-Menü (`HMenuItem`, `HContextMenuController`). Es erscheint ohne
Bewegung; jeder Eintrag ist mindestens 28 px hoch und füllt sich unter dem
Zeiger. Deaktivierte Einträge bleiben an ihrem Platz, damit sich die Reihenfolge
nie ändert.

### HHairline

Ein Trenner von 1 px, waagerecht oder senkrecht, in `line` oder `lineStrong`. Das
Design nimmt Haarlinien, wo andere Schatten nehmen.

### HFocusRing

Der eine Fokusring des Systems: 2 px Akzent außerhalb des Bedienelements,
gezeigt in einem Frame, nie animiert, nie einen vorhandenen Rand umfärbend. Ein
Abstand von 2 px erscheint nur dort, wo der Ring sonst eine Füllung derselben
Farbe berühren würde (der primäre Knopf). `HFocusRing.inline` zeichnet für
Zeilen von Rand zu Rand auf dem eigenen Rand des Kindes.

### HSkeleton und HWait

`HSkeleton` zeichnet die Haarlinien der Zeilen, die gleich ankommen, in `fg2`,
ohne Bewegung, in der Höhe der Liste, in der es steht. Es gibt in der App keinen
Spinner. `HWait` wendet die beiden Schwellen an: nichts in den ersten 150 ms,
dann das Skeleton für mindestens 400 ms.

### HCautionTape

Das Absperrband: diagonale Streifen in Markengelb und Schwarz, 4 px hoch, die
sich mit einer Periode (16 px) pro 1200 ms seitwärts bewegen. Es ist ein
Markenzeichen, kein Zustand und kein Bedienelement, und steht bei reduzierter
Bewegung still. Wo es erscheinen darf, regelt
[docs/UX.md, Abschnitt 3.6](UX.md#36-the-caution-tape).

### HAnimatedFill

Animiert eine Farbe mit einer Dauer, die das Flag `disableAnimations` der
Plattform übersteht. Verwendet für Füllungen beim Drücken, die bei reduzierter
Bewegung ihre volle Dauer behalten.

### Editor-Markierungen

`HDecoratedTextController`, `HEditorDecoration` und `HEditorDecorations` malen
die drei Markierungen des Editors: `secret` (Wellenlinie in `error`), `pii`
(Wellenlinie in `warning`) und `replaced`, den [Diff-Glow](#diff-glow).

### Theme-Verdrahtung

`HTheme` (mit `HTheme.dark`, `HTheme.light`) veröffentlicht die Tokens;
`HThemeMode` (`dark`, `light`, `system`) bildet die Einstellung `ui.theme` darauf
ab; `hLocalizationsDelegates` liefert die eigenen Texte der Bibliothek.

## Layout

Die Shell besteht aus einer Kopfzeile von 40 px, einer Icon-Leiste links, dem
aktiven Abschnitt und einer Statusleiste von 24 px. Bereiche sind dauerhaft und
in der Größe veränderbar; Sheets kommen von rechts; Modals gibt es nur für
destruktive Bestätigungen.

![Anhalten-Bildschirm mit drei angehaltenen Anfragen](img/design-intercept-layout.png)

### Shell

| Teil | Größe | Hinweise |
|---|---|---|
| Kopfzeile | 40 px | App-Zeichen, Session, Isolationsring, Fenstersteuerung |
| Icon-Leiste | 48 px eingeklappt, 200 px ausgeklappt | Anhalten, Verlauf, Regeln, Sandbox, Audit, Einrichten; `Ctrl+0` schaltet um |
| Statusleiste | 24 px | Verbindung zum Daemon, leiser Text |
| Kopf eines Bereichs | 48 px, Rand eingeschlossen | `paneHeaderHeight`, immer `HPaneHeader`; dieselbe Reihe wie der erste Eintrag der Leiste |
| Eintrag der Leiste | 48 px | `railItemHeight`; beginnt direkt unter der Kopfzeile |
| Markierung des aktiven Abschnitts | 2 px Akzent an der Leiste, volle Höhe des Eintrags | `selectionRail` |

Jeder Kopf eines Bereichs und eines seitlichen Sheets endet auf einer Linie
quer durch das Fenster, und zwei benachbarte Regionen teilen sich einen Trenner.
Die Regeln stehen in [docs/UX.md, Abschnitt 3.7](UX.md#37-layout-grid).

![Icon-Leiste, ausgeklappt](img/design-icon-rail.png)

### Anhalten: drei Bereiche

| Bereich | Vorgabeanteil | Mindestbreite | Inhalt |
|---|---|---|---|
| Warteschlange | 28 % | 280 px | Angehaltene Anfragen, nach Host gruppiert |
| Inspektor | 44 % | 480 px | Die Anfragekarte, die Aktionsleiste oder der Editor |
| Kontext (Domain) | 28 % | 260 px | Was über die Ziel-Domain bekannt ist |

Trenner sind Griffe von 7 px mit einer Haarlinie von 1 px in der Mitte, 2 px
beim Ziehen, und bewegen sich fokussiert um 8 px je Pfeiltaste ihrer Achse; ein
Doppelklick stellt die Vorgabeaufteilung wieder her. Jede Grenze zwischen zwei
Bereichen ist ein solcher, nebeneinander oder übereinander (das Sandbox-Terminal
über seinen Tabs, HUM-305). Die Verhältnisse jedes Bildschirms bleiben in der
UI-Zustandsdatei (`$XDG_STATE_HOME/humanitl/ui-state.json`)
unter `paneSplit.<screen>` erhalten, geschrieben, wenn ein Ziehen endet.

### Zeile der Warteschlange

36 px Mindesthöhe in jedem Zustand, einzeilig: 4 px Zustandsleiste, 16 px
Zustandsglyphe mit ihrem Countdown-Ring, neutrales Methoden-Badge, Host
(13/500), Pfad in Mono 12, in der Mitte gekürzt, Chip mit Funden und der
reservierte Aktionsplatz von 28 px, in dem Hover oder Fokus Blockieren zeigt
(250 ms halten). Die Restzeit ist der Bogen des Rings, keine Ziffern `mm:ss`,
und eine Zeile sendet nie; Senden braucht die vollständige URL auf der Karte.
Hosts werden nie am Ende gekürzt. Der vollständige Aufbau steht in
[docs/UX.md, Abschnitt 3.4](UX.md#34-the-queue-row-written-out). Ein Absperrband
von 4 px läuft unter dem Kopf der Warteschlange, solange mindestens eine Anfrage
angehalten ist.

### Weitere feste Breiten

| Element | Breite |
|---|---|
| `HSheet` Vorgabe | 360 px |
| `HModal` Vorgabe | 420 px |
| Klickfläche der Entscheidung (Senden, Blockieren) | mindestens 120 x 32 px |
| Kleinste Klickfläche | 28 px |

Das Layout Bildschirm für Bildschirm (Verlauf, Regeln, Sandbox, Audit,
Einrichten, leere Zustände) steht in [docs/UX.md](UX.md).

## Interaktion

Die Tastatur ist ein vollwertiger Arbeitsweg: Jeder Zeigerweg hat eine Taste
([docs/UX.md, Abschnitt 5](UX.md#5-keyboard-and-mouse)). Einbuchstabige Tasten
feuern nie, solange ein Textfeld den Fokus hat; die Akkorde (`Ctrl+F`,
`Ctrl+L`) schon. Die beiden Entscheidungstasten ignorieren die
Tastenwiederholung, und Senden ist erst 350 ms nach der Auswahl einer neuen
Anfrage wieder scharf, damit ein gehaltenes Enter die Warteschlange nicht
ungelesen senden kann.

### Tastenkürzel

Das Vokabular ist festgelegt in
[backlog/CONVENTIONS.md, Abschnitt 3.9](../backlog/CONVENTIONS.md#39-flutter-app).
Die Tabelle führt jede Belegung so, wie die App sie in 0.1 hat, geprüft gegen
`app/lib/core/shortcuts/intents.dart`, `app/lib/features/intercept/intents.dart`,
`app/lib/features/history/history_screen.dart` und
`app/lib/features/editor/editor_intents.dart`.

| Taste | Aktion | Intent | Wo |
|---|---|---|---|
| `Enter`, `A`, `Ctrl+F` | Die ausgewählte Anfrage einmal senden, unverändert | `AllowIntent` | Anhalten |
| `B`, `Ctrl+L` | Die ausgewählte Anfrage blockieren | `BlockIntent` | Anhalten |
| `Ctrl+Enter` | Mit der Notiz blockieren, aus dem Notizfeld heraus | `BlockIntent` | Anhalten |
| `E` | Den Editor für die ausgewählte Anfrage öffnen | `EditIntent` | Anhalten |
| `N` | Die Notiz zum Blockieren fokussieren | `NoteIntent` | Anhalten |
| `Shift+Enter` | Das Merk-Raster öffnen, ohne zu entscheiden | `OpenRememberIntent` | Anhalten |
| `1` bis `4` | Gültigkeit beim Merken: einmal, Session, eine Stunde, immer | `RememberDurationIntent` | Anhalten |
| `Shift+1` bis `Shift+4` | Ziel beim Merken: genaue URL, Host, Domain, Host und Methode | `RememberTargetIntent` | Anhalten |
| `Ctrl+Shift+F` | Die Auswahl oder die Gruppe unter dem Cursor senden | `AllowGroupIntent` | Anhalten |
| `Ctrl+Shift+L` | Die Auswahl oder die Gruppe unter dem Cursor blockieren | `BlockGroupIntent` | Anhalten |
| `Ctrl+A` | Die Gruppe unter dem Cursor auswählen | `SelectGroupIntent` | Anhalten |
| `Left`, `Right` | Die Gruppe zuklappen oder öffnen | `ToggleGroupIntent` | Anhalten |
| `Shift+J` | Anfragen einreihen, die während des Lesens ankamen | `MergeArrivalsIntent` | Anhalten |
| `J`, `Down` / `K`, `Up` | Nächste / vorige Zeile | `NextFlowIntent` / `PrevFlowIntent` | Anhalten, Verlauf |
| `Esc` | Das Coach-Mark schließen | `CloseCoachMarkIntent` | Anhalten |
| `S` / `P` / `Esc` | Trotzdem senden / pseudonymisieren / schließen, in der Fund-Pause | `SendAnywayIntent` / `PseudonymizeIntent` / `CloseFindingsPauseIntent` | Fund-Pause |
| `Ctrl+R` | Die Auswahl pseudonymisieren | `PseudonymizeSelectionIntent` | Editor |
| `Ctrl+Enter` | Die editierte Version senden | `SendEditedIntent` | Editor |
| `F3` | Nächster Fund | `NextFindingIntent` | Editor |
| `Esc` | Den Editor schließen | `CloseEditorIntent` | Editor |
| `Enter` | Die ausgewählte Zeile öffnen | `OpenFlowIntent` | Verlauf |
| `/` | Den Filter fokussieren | `FilterIntent` | Verlauf |
| `Alt+Up`, `Alt+Down` | Eine Regel nach oben oder unten schieben | `MoveRuleIntent` | Regeln |
| `Ctrl+1` bis `Ctrl+6` | Zu Anhalten, Verlauf, Regeln, Sandbox, Audit, Einrichten wechseln | `NavIntent` | Überall |
| `Ctrl+K` | Befehlspalette | `PaletteIntent` | Überall |
| `Ctrl+,` | Einstellungen | `SettingsIntent` | Überall |
| `Ctrl+0` | Die Icon-Leiste aus- oder einklappen | `ToggleRailIntent` | Überall |
| `Ctrl+Shift+Enter` (auch `Enter` im Ziffernblock) | Das Terminal über das ganze Fenster zeigen, und zurück (HUM-298) | `ToggleTerminalFullWindowIntent` | Überall, auch im Terminal |
| `Ctrl+Shift+P` | Das Terminal als kleines Fenster über den Abschnitten schweben lassen und zurücklegen (HUM-300); nichts, solange kein Agent läuft | `ToggleTerminalFloatIntent` | Überall, auch im Terminal |
| ``Ctrl+` `` (die Taste links von `1`, nach Lage) | Dem gezeigten Terminal die Tastatur geben, ohne das Layout zu ändern; bei laufendem Agenten und ohne Terminal auf dem Bildschirm lässt es das Terminal zuerst schweben (HUM-303, HUM-300) | `FocusTerminalIntent` | Überall, auch im Terminal |
| `Ctrl` + `+` (auch `Ctrl+=`, `+` im Ziffernblock) / `Ctrl` + `-` (auch `-` im Ziffernblock) | Terminaltext eine Stufe größer / kleiner, 10 bis 28 px, über Neustarts erhalten; `Ctrl` + Mausrad über dem Terminal tut dasselbe (HUM-301) | `TerminalZoomIntent` | Solange ein Terminal gezeigt wird, auch darin |
| `Ctrl+Shift+0` (auch `Ctrl` + `0` im Ziffernblock) | Terminaltext zurück auf seine Vorgabegröße; nicht `Ctrl+0`, das ist die Leiste (HUM-301) | `TerminalZoomIntent` | Solange ein Terminal gezeigt wird, auch darin |
| `Ctrl+Shift+C` / `Ctrl+Shift+V` (`Cmd+C` / `Cmd+V` auf Apple-Plattformen) | Die Auswahl im Terminal kopieren / ins Terminal einfügen; schlichtes `Ctrl+C` und `Ctrl+V` bleiben beim Agenten (HUM-313), außer dass schlichtes `Ctrl+C` kopiert, solange Text ausgewählt ist (HUM-353, nicht auf Apple-Plattformen) | `TerminalClipboardActivator`, kein Intent | Nur im Terminal |
| `Ctrl+Shift+T` | Eine weitere Shell in der laufenden Sandbox als Tab des Terminals öffnen; nur solange der Daemon `sandbox.exec` anbietet und eine Session läuft (HUM-314) | `OpenShellIntent` | Nur im Terminal und seinen Tabs |
| `Ctrl+PageUp` / `Ctrl+PageDown` | Voriger / nächster Tab des Terminals, über das Ende hinweg; ist der Tab des Agenten allein, bleiben die Akkorde beim Agenten (HUM-314) | `SwitchTerminalTabIntent` | Nur im Terminal und seinen Tabs |

Drei Intents aus CONVENTIONS 3.9 haben in 0.1 keine Belegung, weil eine Taste,
die nichts tut, schlimmer ist als keine Taste
([docs/UX.md, Abschnitt 5](UX.md#5-keyboard-and-mouse)):

| Taste in CONVENTIONS 3.9 | Intent | Stand in 0.1 |
|---|---|---|
| `R` | `ScopeIntent` | Nicht belegt; stattdessen öffnet `Shift+Enter` das Merk-Raster |
| `Ctrl+D` | `ToggleDomainPanelIntent` | Intent existiert, nicht belegt |
| `Ctrl+B` | `AddRuleIntent` | Nicht belegt |

CONVENTIONS 3.9 nennt außerdem `DurationIntent(n)` auf `1/2/3` und
`TargetIntent(n)` auf `Shift+1..4`; die App setzt sie als
`RememberDurationIntent` und `RememberTargetIntent` mit vier Gültigkeiten um,
und aus `Ctrl+1..5` wurde `Ctrl+1..6`, als der Abschnitt Einrichten dazukam.

### Entscheidungsregeln

- Senden und Blockieren liegen nie nebeneinander.
- Neue Anfragen stehlen nie den Fokus, und die Liste wird nie unter dem Cursor
  umsortiert; Neuankömmlinge warten hinter einer Pille „+3 neu" (`Shift+J` reiht
  sie ein).
- Anfragen werden nach Host gruppiert. Ein Sammelknopf nennt sein Ziel („14 an
  registry.npmjs.org senden"), nie „Alle senden".
- Bevor eine Entscheidung gemerkt wird, wird die Regel als Satz gezeigt, zum
  Beispiel `allow · GET · *.npmjs.org · this session`.
- Die Vorgabe für die Gültigkeit ist die Session, nicht „Immer".
- Eine gespeicherte Regel bestätigt sich an Ort und Stelle („Regel gespeichert ·
  Rückgängig"), nicht mit einem Toast.
- Eine Anfrage, die durch Regel erlaubt wurde, erscheint nur als Zeile im Feed
  mit einem Regel-Chip und einem Zähler in der Leiste.
- Wenn die Warteschlange von 0 auf 1 geht: eine Desktop-Benachrichtigung mit
  Senden und Blockieren, ein Badge im Tray und bei der Rückkehr ein Banner („Der
  Agent wartet seit 4 Minuten").

Die Aktionsleiste unter der Anfragekarte, mit dem Freigabeventil links und
Blockieren ganz rechts:

![Aktionsleiste](img/design-action-bar.png)

## Bewegung

Bewegung erklärt eine Zustandsänderung; sie dekoriert nicht. Eintritte nutzen
`enter` (`cubic-bezier(0.2, 0, 0, 1)`), Austritte nutzen `exit`
(`cubic-bezier(0.4, 0, 1, 1)`). Bei reduzierter Bewegung fallen Wege auf null und
Schleifen halten an, aber die Dauern, die eine Entscheidung bestätigen
(Ausblenden, Füllung beim Drücken, Schwenk der Leiste), bleiben
([docs/UX.md, Abschnitt 2.10](UX.md#210-reduced-motion)).

| # | Mikrointeraktion | Dauer | Kurve | Was sich bewegt |
|---|---|---|---|---|
| 1 | Ankunft | 180 ms (`arrive`), Zeilen eines Schubs 30 ms versetzt, höchstens 5 gestaffelt | `enter` | Die Zeile gleitet 8 px nach unten an ihren Platz und blendet ein |
| 2 | Countdown | Atemperiode 1200 ms (`breathe`), 3 Atemzüge bei 20 % und noch einmal bei 5 % Rest | ease in-out | Der Ring um die Glyphe schrumpft; die Glyphe atmet, nie unter 72 % Deckkraft, nie rot |
| 3 | Entscheidung | Füllung 120 ms (`press`), Schwenk der Leiste 200 ms (`sweep`) | `enter` | Das gedrückte Bedienelement füllt sich, dann schwenkt die Zustandsleiste auf die neue Zustandsfarbe |
| 4 | Abgang | 220 ms (`leave`) | `exit` | Gleiten und Ausblenden in den ersten 60 %, Zusammenfallen der Höhe in den letzten 60 %; gesendete Zeilen gleiten nach rechts, blockierte und abgelaufene nach links, 12 px |
| 5 | Einzeichnen einer Regel | 240 ms (`ruleDraw`) | `enter` | Eine neue Regelzeile wächst und blendet in die Regelliste ein, einmal |

Halten hat eigene Zeiten: 400 ms (`holdToConfirm`) am Freigabeventil, 250 ms
(`holdToBlock`) für Blockieren in einer Zeile und in der Aktionsleiste. Die drei
Isolationsprüfungen erscheinen im Abstand von 120 ms (`checkStagger`), damit drei
Nachweise als drei gelesen werden, nicht als ein Schalter. Was nie animiert
(Fokusringe, Kontextmenüs, Zeilenhöhen), steht in
[docs/UX.md, Abschnitt 2](UX.md#2-the-motion-language).

## Markenzeichen

### Freigabeventil

Senden ist eine geteilte Pille (`HPill`, genutzt von der Aktionsleiste von
Anhalten). Die linke Hälfte sagt „Senden" und sendet die Anfrage einmal. Die
rechte Hälfte, ein Chevron, öffnet das Raster aus Gültigkeit mal Ziel. Wer die
linke Hälfte 400 ms lang hält, füllt sie grün und sendet die Anfrage mit der
gemerkten Regel; die Beschriftung wechselt während des Haltens auf die gemerkte
Aktion. Senden lässt sich nicht rückgängig machen, deshalb liegt der Schutz in
der Zeit, nicht in einem Modal oder einer kleineren Klickfläche. Gehaltenes
Enter oder Leertaste tut dasselbe von der Tastatur aus; eine
Tastenwiederholung entscheidet nie.

![Freigabeventil beim Halten](img/design-release-valve-holding.png)

### Isolationsring

Ein Ring von 20 px in der Kopfzeile, ein Bogen je Garantie der Sandbox (kein
Netzwerk-Interface, genau ein Socket zu Humanitl, keine neuen Sockets unter
seccomp). Jeder Bogen ist grau, solange er nicht gemessen oder getrennt ist,
gelb und atmend, während er gemessen wird, grün, wenn die Garantie hält, und
rot, wenn nicht. Ein geschlossener grüner Ring ist immer nur drei gemessene
Ergebnisse. Der Ring ist ein Ghost-Knopf: Ein Klick öffnet den Abschnitt Sandbox
auf seinem Tab Isolation, der jede Garantie mit dem Nachweis aufführt, den der
Shim geschrieben hat, die genaue Kommandozeile der Sandbox öffnet und für die
erklärte Ausnahme eine vierte Zeile in der Warnfarbe hinzufügt, zum Beispiel
„Ausnahme: LLM unter 192.168.1.20:11434, Passthrough, geloggt, nie angehalten".

Zustände des Rings von links nach rechts: 3 von 3 bestanden, 2 von 3 mit einem
fehlgeschlagen, kein Bericht (alle fehlgeschlagen), noch nicht gemessen.

![Zustände des Isolationsrings](img/design-isolation-ring-states.png)

Der Tab Isolation mit drei bestandenen Garantien und der Ausnahmezeile:

![Isolationsbereich](img/design-isolation-panel.png)

### Diff-Glow

Im Anfrage-Editor bekommt jede Stelle, die ein Mensch durch ein Pseudonym ersetzt
hat, eine Akzent-Unterstreichung von 1 px und eine Akzentfüllung von 10 Prozent
(`HEditorDecorations.glowAlpha`, `underlineWidth`). Das ist die eine Stelle, an
der der Akzent etwas anderes als Fokus bedeutet. Funde werden anders gezeichnet,
mit einer Wellenlinie: orange für per Prüfsumme bestätigte Secrets, gelb für
personenbezogene Daten, damit sich „schau hier" und „erledigt" ohne Farbe
unterscheiden. Welches Pseudonym was ersetzt hat, steht im Zuordnungsstreifen
unter dem Editor, eingeklappt, bis man ihn öffnet, mit maskiertem Originalwert;
die Zuordnung bleibt auf dem Rechner und erreicht den Agenten nie. BACKLOG.md
Abschnitt 5 skizzierte einen Hover, der Original und Pseudonym nebeneinander
zeigt; 0.1 hat keinen solchen Hover.

![Editor mit ersetzten Stellen](img/design-diff-glow.png)

## Sprache

Englisch ist die Quellsprache, Deutsch ist vom ersten Tag an gleichwertig. Jeder
UI-Text läuft über ARB-Dateien: [`app/l10n/app_en.arb`](../app/l10n/app_en.arb)
ist die Quelle, [`app/l10n/app_de.arb`](../app/l10n/app_de.arb) die
Übersetzung. Deutsch spricht den Nutzer mit „du" an. Protokollbegriffe bleiben in
beiden Sprachen englisch (GET, POST, Header, Body, Status), ebenso alles, was
gespeichert wird (`allow`, `block`, `ask`, `redact` in `rules.yaml` und
Audit-Einträgen).

Zentrale Begriffe (die vollständige Liste steht in
[docs/GLOSSARY.md](GLOSSARY.md#ui-terms)):

| Begriff | Englisch | Deutsch | Hinweis |
|---|---|---|---|
| Zustand angehalten | Held | Angehalten | Nie „abgefangen" |
| Knopf Senden an einer Anfrage | Allow | Senden | Der Knopf sagt, was passiert |
| Erlauben in einer Regel | Allow | Erlauben | Nur in Regeln |
| Blockieren | Block | Blockieren | Nie „Ablehnen" |
| Merken | Remember | Merken | |
| Gültigkeit (einer Regel) | Duration | Gültigkeit | |
| Ziel (Geltungsbereich einer Regel) | Target | Ziel | |
| Diese Session | This session | Diese Session | „Session" bleibt |
| Für immer | Always | Immer | |
| Pseudonymisieren | Pseudonymize | Pseudonymisieren | Nie „Anonymisieren", das rechtlich etwas anderes ist |
| Fund | Finding | Fund | Nie „Treffer" |
| Zeit abgelaufen | Timed out | Zeit abgelaufen | |
| LLM-Durchleitung | LLM passthrough | LLM-Durchleitung | |
| Warteschlange | Queue | Warteschlange | |
| Verlauf | History | Verlauf | |

## Anti-Patterns

1. **Rot als Stimmung.** Rot bedeutet blockiert. Fehler und Secrets sind orange,
   Warnungen gelb.
2. **Modals für Entscheidungen.** Entscheidungen fallen in der Warteschlange;
   Modals sind nur für destruktive Bestätigungen.
3. **Toast-Spam.** Eine Bestätigung erscheint an Ort und Stelle und hängt an dem,
   was sich geändert hat.
4. **Klickflächen unter 28 px.** `hitMin` ist die Untergrenze; Entscheidungen
   bekommen 120 x 32.
5. **Gestapelte Schatten.** Tiefe kommt aus der Leiter der Oberflächen und aus
   Haarlinien.
6. **Hosts am Ende abschneiden.** Das Ende eines Hosts ist der Teil, auf den es
   ankommt (`registry.npmjs.org.example.com`); Pfade werden in der Mitte
   gekürzt, Hosts gar nicht.
7. **Favicons automatisch laden.** Das wäre eine Netzwerkanfrage, die der Agent
   nicht gestellt und der Nutzer nicht freigegeben hat.
8. **Farbverläufe wie im Dashboard.** Nur flache Flächen; Farbe ist dem Zustand
   vorbehalten.
