---
source: UX.md
source-sha256: 69b32867067c5ce8533ee58f304b965e0cc79b2fce64785d5b306a17a714387e
lang: de
---
# Humanitl — UX-Leitlinien

Applies to: Humanitl 0.1

> Zweck: Die Oberfläche soll eine Entscheidung so leicht machen, dass ein Mensch sie stundenlang treffen kann, ohne zu ermüden, und so schwer, dass er sie nicht aus Versehen trifft. Dieses Dokument legt fest, wie Bewegung, Farbe, Text und Tastatur zusammenwirken, damit jeder Bildschirm von HUM-028 bis HUM-034 dieselbe Sprache spricht. Es ergänzt `BACKLOG.md` Abschnitt 5 (Richtung „Schleuse"), `docs/ARCHITECTURE.md` (Schichten) und `app/packages/ui` (Werte).

Im Konfliktfall gilt: `packages/ui` gewinnt bei den Werten, dieses Dokument bei ihrer Verwendung. Wo eine Regel hier ein Token bräuchte, das es nicht gibt, steht sie in Abschnitt 9. Tokens werden nie nebenbei geändert.

## 1. Warum es dieses Dokument gibt

Ein Mensch entscheidet hier über den Netzwerkverkehr eines anderen, unter laufender Uhr, stundenlang. Jede Bewegung, jede Farbe und jedes Wort auf dem Bildschirm kostet Aufmerksamkeit, die er für die nächste Entscheidung braucht. Dieses Dokument legt fest, wofür diese Aufmerksamkeit ausgegeben werden darf, und macht jede solche Entscheidung prüfbar.

Es steht neben `backlog/CONVENTIONS.md` 4.13, das festhält, warum dieses Programm Vertrauen ausstrahlen muss und wie das gemessen wird: nie mehr behaupten als bewiesen, Genauigkeit, wo es darauf ankommt, Zurückhaltung, Vorhersagbarkeit, keine Dark Patterns in keine Richtung, Fehler mit Grund und Abhilfe, sichtbares Handwerk. Was hier steht, ist die gestalterische Umsetzung dieses Versprechens; wo beide Dokumente dasselbe berühren, gewinnt 4.13 bei der Aussage und dieses Dokument bei der Umsetzung.

### 1.1 Wofür die gesparte Aufmerksamkeit ausgegeben wird

Die meisten Regeln hier verbieten etwas. Das ist die halbe Arbeit; die andere Hälfte besteht darin, das freigewordene Budget an einer Stelle pro Bildschirm auszugeben, statt es zu sparen. Ein Programm, das nur spart, wird zu einem sauberen Abklatsch ohne eigene Handschrift.

Jeder Bildschirm hat deshalb einen benannten Moment, in dem er schön sein darf. Genau einen:

| Bildschirm | Der Moment | Woran du ihn erkennst |
|---|---|---|
| Anhalten | die Schleusenkammer im Augenblick der Entscheidung | Die Leiste der entschiedenen Zeile zieht auf volle Sättigung auf und ist für 200 ms die einzige gesättigte Fläche im ganzen Bereich. Alles andere bleibt neutral, damit dieses eine Aufziehen etwas bedeutet. |
| Regeln | die Reihenfolge als lesbare Kette | Die Regeln stehen als Sätze untereinander und werden von oben nach unten gelesen wie ein Text; die erste, die passt, gewinnt, und man sieht das, ohne es zu wissen. Der Probelauf macht diese Kette an echtem Verkehr sichtbar. |
| Verlauf | Dichte als Beleg | Zehntausend Zeilen im gleichmäßigen 28-px-Takt, ohne Lücke, ohne Ladebereich. Die Menge selbst ist die Aussage: Nichts ging unbeobachtet durch. |
| Domain-Bereich | die Antwort in einer Zeile | „Bekannt" oder „unbekannt" steht als erstes Wort, in der größten Schrift des Bereichs, bevor irgendein Detail folgt. |
| Einrichten und Sandbox | das Urteil über die drei Garantien | Drei Zeilen, drei Zustände, keine Erklärung davor. Die Erklärung steht darunter, für wen sie will. |

Was in diesen fünf Momenten nicht vorkommt, bekommt keine Sättigung, keine Bewegung und keine große Schrift. Das ist der Handel.

---

## 2. Die Bewegungssprache

### 2.1 Die Prüffrage

Jede Animation beantwortet genau eine von vier Fragen: Was ist angekommen? Was ist gegangen? Was hat sich an Ort und Stelle geändert? Was habe ich gerade ausgelöst? Eine Animation, die keine dieser Fragen beantwortet, wird gelöscht, nicht nachjustiert. Diese Frage ist die Review-Schranke für HUM-029 bis HUM-034.

Jede Dauer, jede Kurve und jede Strecke kommt aus `HMotion`. Eine `Duration`, eine `Curve` oder ein Pixelversatz als Literal in einer Feature-Datei ist ein Defekt, auch wenn die Zahl stimmt: Eine Tabelle ist der einzige Weg, auf dem fünf ungebaute Bildschirme dieselbe Sprache behalten.

### 2.2 Die Tabelle

| Änderung auf dem Bildschirm | Was sie erklärt | Dauer | Kurve | Token | Weg |
|---|---|---|---|---|---|
| Zeile kommt in der Warteschlange an | „Ein Paket ist von außen angekommen" | 180 ms | `enter` | `HMotion.arrive` | 8 px von oben, `HMotion.arriveOffset`, plus Einblenden |
| Mehrere Zeilen im selben Frame | „Es sind mehrere, eine nach der anderen" | +30 ms pro Zeile, höchstens 5 | `enter` | `HMotion.stagger`, `HMotion.staggerMax` | wie oben, gestaffelt |
| Der Zähler der Warteschlange erhält eine Ankunft | „Es ist etwas gekommen, auch wenn du es nicht siehst" | 2 × 120 ms, einmal hin und zurück | `easeInOut` | `HMotion.press` | Tönung des `HBadge` im Kopf der Warteschlange von `held` 10 % auf `held` 20 % und zurück; genau dieses eine Widget, auch bei eingefrorener Warteschlange. Die Ziffer darin springt, sie animiert nie (2.9). |
| Bedienelement wird gedrückt (Maus oder Taste) | „Dein Griff ist angekommen" | 120 ms | `enter` | `HMotion.press` | Füllung |
| Leiste wechselt den Zustand | „Der Zustand hat sich geändert" | 200 ms | `enter` | `HMotion.sweep` | Aufziehen von oben nach unten, nie eine Überblendung |
| Halten bestätigt ein Blockieren | „Das war gewollt" | 250 ms | linear | `HMotion.holdToBlock` | Füllung von links |
| Halten füllt das Freigabeventil | „Das gilt für die ganze Session" | 400 ms | linear | `HMotion.holdToConfirm` | Füllung von links |
| Entscheidung unterwegs | „Ich habe gehandelt, die Antwort steht aus" | — | — | — | keine Bewegung. Die Leiste behält ihren aufgezogenen Zustand, die Glyphe ist getauscht, der Streifen fehlt noch. Kommt die Antwort nicht innerhalb von `HMotion.confirm`, nimmt das Inline-`Diagnostic` aus 4.4 den Platz des Streifens ein. |
| Abgewiesener Tastendruck | „Diese Taste gilt hier gerade nicht" | 120 ms | `enter` | `HMotion.press` | Das Bedienelement, das gehandelt hätte, zeigt seine Füllung und leert sich wieder; der Grund steht daneben (5.3). Nie ein Schütteln, nie ein rotes Aufblitzen. |
| Entschiedene Zeile geht | „Diese Anfrage ist draußen gesendet oder blockiert" | 220 ms | `exit` | `HMotion.leave` | 12 px zur Seite (`HMotion.leaveOffset`), dann Höhe auf null |
| Auswahl wechselt | „Ich schaue jetzt auf diese Zeile" | ein Frame | — | — | Wechsel der Füllung, kein Wechsel der Höhe |
| Abschnitt oder Gruppe klappt auf | „Darunter ist mehr" | 180 ms | `enter` | `HMotion.arrive` | Höhe |
| Seitenleiste klappt auf oder zu | „Hier sind die Namen" oder „zurück zu nur den Symbolen" | 180 ms | `enter` auf, `exit` zu | `HMotion.arrive` | Breite der Icon-Leiste von 48 auf 200 px; die Beschriftung wird in voller Breite gesetzt, nur aufgedeckt, und ab halber Breite eingeblendet. Bei reduzierter Bewegung springt die Breite, das Einblenden behält seine Dauer (2.10, HUM-254). |
| Absperrband läuft | „Hier ist die Schranke" oder „die Suche läuft" | 1200 ms pro Periode, endlos, solange die Ursache besteht | linear | `HMotion.cautionTape`, `HMotion.cautionTapePeriod` | Streifenmuster seitwärts um 16 px pro Periode; das Widget selbst steht still. Bei reduzierter Bewegung stehen die Streifen still (3.6, HUM-260). |
| Sheet gleitet von rechts herein | „Das gehört zu dem, was links steht" | 180 ms | `enter` | `HMotion.arrive` | 8 px von rechts (`HMotion.arriveOffset`) plus Einblenden; der Hintergrund bleibt stehen |
| Schwebendes Terminal rastet an seinem Anker ein | „Hier hält es jetzt" | 180 ms | `enter` | `HMotion.arrive` | Nur der Ort, von dort, wo es losgelassen wurde, zu seinem Anker; ein Ablegen, das auch die Größe ändert, springt, damit die Session nicht in jedem Frame neu bemessen wird. Bei reduzierter Bewegung springt es (2.10, HUM-352). |
| Modal erscheint | „Hier musst du zuerst antworten" | 180 ms | `enter` | `HMotion.arrive` | kein Weg, nur Einblenden des Modals und Abdunkeln des Hintergrunds; ein Modal, das hereinfliegt, behauptet eine Richtung, die es nicht hat |
| Befehlspalette öffnet sich | „Das Handbuch ist da" | ein Frame | — | — | keine Bewegung. Wer sie aufruft, tippt schon. |
| Wechsel der Ansicht (`Ctrl+1..5`) und Tab-Wechsel | „Ich bin jetzt woanders" | ein Frame | — | — | keine Überblendung im `IndexedStack`, keine Verschiebung. Der Wechsel ist Navigation, kein Ereignis. |
| Rückkehr-Banner erscheint | „Etwas ist passiert, während du weg warst" | 180 ms | `enter` | `HMotion.arrive` | wie eine Ankunft: 8 px von oben plus Einblenden |
| Neue Regel erscheint in der Liste | „Diese Regel gibt es jetzt" | 240 ms | `enter` | `HMotion.ruleDraw` | Zeichnen der Zeile |
| Countdown unter einer Schwelle | „Schau jetzt hin" | 1200 ms pro Atemzug, 3 Atemzüge | `easeInOut` | `HMotion.breathe`, `HMotion.breatheCycles` | Deckkraft zwischen `HMotion.breatheMinOpacity` und 1,0 |
| Fehler | „Hier ist etwas schiefgegangen" | — | — | — | keine Bewegung; das Diagnostic blendet über `HMotion.arrive` in einen reservierten Platz ein |

### 2.3 Richtung ist Bedeutung

Gesendet und nach Bearbeitung gesendet gleiten nach rechts, blockiert, Zeit abgelaufen und Fehler nach links. Sonst bewegt sich kein Widget irgendwo im Programm horizontal, außer dem Sheet, das von der Kante kommt, an der es hängt. Das Absperrband (3.6) ist keine Ausnahme: Es steht still, nur sein Streifenmuster läuft, und ein Muster ist eine Füllung, keine Position (nächster Absatz).

Die Achsenregel bindet die **Position** eines Widgets, nicht seine **Füllung**. Eine Füllung, die von links wächst, ist Fortschritt und nicht Richtung: Sie sagt „so weit bin ich gekommen", nicht „dorthin geht es". Deshalb dürfen beide Haltefüllungen von links laufen, ohne die Regel zu brechen, und deshalb füllt sich nichts im Programm von rechts, von oben oder aus der Mitte; eine zweite Füllrichtung wäre eine zweite Bedeutung für dieselbe Geste.

Eine einzige Bedeutung für eine einzige Bewegungsachse ist am Rand des Blickfelds lesbar, und dort steht die Warteschlange meistens.

### 2.4 Der Abgang hat zwei Phasen

Gleiten und Ausblenden laufen in den ersten sechzig Prozent von `HMotion.leave`, das Zusammenfallen der Höhe in den letzten sechzig (`HMotion.leaveGlideFraction`, die Phasen überlappen). Auf einer gemeinsamen `easeIn`-Kurve steht die Zeile etwa 170 ms still und springt erst in den letzten Frames zur Seite; genau die Richtung, die die ganze Bedeutung trägt, ist dann unsichtbar.

Das Zusammenfallen bleibt, und es kostet: Eine Liste, deren Zeilen ihre Höhe animieren, kann ihre Zeilenhöhe nicht im Voraus kennen. Die Warteschlange behält deshalb `AnimatedList` und verzichtet auf `itemExtent`; die Extent-Pflicht aus Abschnitt 7 gilt dort nicht, wohl aber im Verlauf und in den Body-Ansichten, wo n groß ist und keine Zeile ihre Höhe ändert. `AnimatedList` und `SliverAnimatedList` kennen weder `itemExtent` noch `prototypeItem`; wer beides zugleich verlangt, verlangt etwas, das Flutter nicht bietet. Die Rechnung geht auf, weil die Warteschlange durch das Anhaltebudget begrenzt ist, während der Verlauf zehntausend Zeilen trägt (Abweichung in Abschnitt 8).

Eine gehende Zeile ist ein eingefrorenes Bild: Sie beobachtet keinen Provider, lädt kein Detail, ihr Countdown hält ab dem Augenblick der Entscheidung an, und sie behält die Höhe, die sie in diesem Augenblick hatte. Eine Zeile, die während ihres Abgangs weiterzählt, behauptet etwas Falsches. „Eingefroren" ist wörtlich gemeint und braucht ein eigenes Widget mit aufgelösten Werten statt eines `ref` (Abschnitt 9).

### 2.5 Das Aufziehen gehört zum Griff, der Streifen zur Antwort

Das Aufziehen der Leiste beginnt mit der Entscheidung selbst und läuft die vollen 200 ms, unabhängig davon, wann der Daemon antwortet. Der Bestätigungsstreifen erscheint erst, wenn die Antwort da ist. Zwei Gutachten widersprechen sich hier, „zeig die Wirkung sofort" gegen „behaupte nichts vor der Antwort", und beide haben recht, weil sie von zwei verschiedenen Dingen sprechen: Das Aufziehen sagt „dein Griff ist angekommen", der Streifen sagt „das ist geschehen". Über einen Unix-Socket dauert `Decide` ein paar Millisekunden; an `DecisionSending` gekoppelt, füllt sich die Leiste zu etwa zwei Prozent und leert sich wieder, sodass der eine Moment, für den es das Produkt gibt, ohne sichtbare Bewegung stattfindet.

Zwischen Aufziehen und Streifen liegt der Zustand, der am längsten dauert, wenn etwas schiefgeht: Die Entscheidung ist unterwegs. Er hat bewusst keine eigene Bewegung. Die Leiste steht aufgezogen, die Glyphe ist getauscht, der Streifen fehlt. Kommt innerhalb von `HMotion.confirm` keine Antwort, nimmt das `Diagnostic` aus 4.4 den Platz ein, den der Streifen gehabt hätte, und die Zeile geht nicht.

### 2.6 Die Wirkung steht innerhalb von 400 ms, nicht 3 s

Was die App selbst entscheidet, steht innerhalb von 400 ms nach dem Tastendruck: Leiste aufgezogen, Zustandsglyphe getauscht, Countdown eingefroren. Was von der Antwort abhängt, steht innerhalb von 400 ms nach der Antwort: der Bestätigungssatz. Die Trennung ist nicht kosmetisch; die App hat die Latenz des Daemons nicht in der Hand und darf deshalb keine Frist versprechen, die sie nicht halten kann.

Die drei Sekunden, bis die Zeile geht (`HMotion.confirm`), sind ein Ruhezustand und kein Wartezustand. Drei Sekunden tote Zeit zwischen Griff und Bewegung reißen den Zusammenhang auseinander: Wenn die Zeile fällt, ist der Mensch schon weiter, und die Bewegung hat keine sichtbare Ursache mehr.

### 2.7 Der Atem ist eine Flagge, keine Skala

Die verbleibende Zeit steht in der Bogenlänge des Rings. Der Atem sagt nur „schau jetzt hin": drei Atemzüge, wenn der Flow unter 20 % seines Budgets fällt, drei weitere bei 5 %, dann Ruhe. Er läuft über `Curves.easeInOut`, nicht auf einem linearen Controller mit `repeat(reverse: true)`; eine Dreieckswelle hat an beiden Enden eine Ecke, und das Auge liest das als Blinken. Er bringt die Glyphe nie unter `HMotion.breatheMinOpacity` (0,72); heute fällt sie auf 0,45, was die dringendste Anfrage zur am schlechtesten sichtbaren macht.

Die Phase kommt aus der Uhrzeit modulo `HMotion.breathe`, damit Gruppenkopf und Zeilen im Gleichtakt atmen, ohne einen anwendungsweiten Ticker zu brauchen. Eine Glyphe, die gerade die Schwelle überschreitet, übernimmt diese Phase aber **nicht sofort**: Sie beginnt ihren ersten Atemzug bei Deckkraft 1,0 und läuft erst danach in die gemeinsame Phase. Sonst beginnt die Flagge, je nach Sekunde, mit einem Sprung von 1,0 auf 0,72 in einem Frame, genau mit dem Blinken, das dieser Abschnitt verbietet, und das auf der dringendsten Zeile von allen.

Fällt eine Schwelle, während die Ansicht unsichtbar ist, verfallen die drei Atemzüge. Sie werden nicht nachgeholt. Der Atem sagt „schau jetzt hin", und dieses Jetzt ist vorbei; was bleiben muss, bleibt im Bogen, im Semantics-Wert und in der Warnung aus 4.8.

### 2.8 Nichts bewegt sich unter dem lesenden Auge

Eine Ankunft darf nie eine Zeile oberhalb des Zeigers oder oberhalb des Scroll-Ankers einfügen. Solange der Zeiger im Bereich der Warteschlange ist, die letzte Tastaturnavigation weniger als `HMotion.freezeAfterKey` (2 s) zurückliegt oder eine Mehrfachauswahl besteht, bleibt die Reihenfolge eingefroren; neue Flows erhöhen nur die Pill „+n neu". Die Pill liegt halbtransparent über der obersten Zeile und wird nie in die Spalte eingefügt; eine Pill im Kopf würde jede Zeile um ihre eigene Höhe nach unten schieben, und genau dagegen gibt es das Einfrieren. Zusammengeführt wird per Klick oder `HMotion.freezeAfterPointer` (500 ms) nach Verlassen des Bereichs.

Die Pill ist erreichbar, nicht nur sichtbar: Sie ist ein Fokus-Halt und hat eine Taste (`Shift+J`, „zu den neuen"). Ohne Taste könnte jemand an der Tastatur die Ankünfte nie zusammenführen, ohne zur Maus zu greifen. Im eingefrorenen Zustand bleibt die Auswahl auf ihrer Zeile und wandert beim Zusammenführen nicht mit; der Mensch hat sie dorthin gesetzt, nicht die Liste.

**Entschieden ist nicht entfernt.** Die beiden Wörter benennen zwei verschiedene Ereignisse, und nur eines davon gibt Platz frei:

- **Entschieden** heißt: Die Zeile bleibt an ihrem Platz im Snapshot, bis `HMotion.confirm` abgelaufen ist. Sie gilt nicht als entfernt, schiebt nichts, und in dieser Zeit trägt sie den Bestätigungsstreifen (3.4). Der Beleg gehört zu der Zeile, die ihn erzeugt hat, also muss die Zeile so lange bleiben.
- **Entfernt** heißt: Die Zeile verlässt den Snapshot sofort und nimmt den Abgang aus 2.4. Das gilt für das Ende des Bestätigungsfensters, für einen Zeitablauf und für eine Entscheidung von anderswo (Regel, Benachrichtigung, zweites Fenster). Entfernte Zeilen geben nur unterhalb des Zeigers Platz frei und schieben nichts herein.

Höchstens drei entschiedene Zeilen ruhen gleichzeitig. Kommt eine vierte Entscheidung, geht die älteste sofort, ohne ihr Fenster abzusitzen. Ohne diese Grenze würde ein schnell abgearbeiteter Schwall die halbe sichtbare Liste mit Belegen für Erledigtes füllen. Ruhende Belege behalten ihre Position; sie sortieren sich weder nach oben noch nach unten, denn das wäre eine Bewegung, die keine der vier Fragen aus 2.1 beantwortet.

### 2.9 Was nie animiert

- **Text, den jemand liest.** URL, Pfad, Header-Tabellen, Body-Ansichten, Regelsatz, Diff. Sie ändern sich sofort oder gar nicht. Dieses Programm verlangt, dass eine URL gelesen wird, bevor über sie entschieden wird; bewegter Text kostet eine zweite Fixation.
- **Zahlen.** Countdown, Zähler der Warteschlange, Tray-Zähler, Fenstertitel. Ziffern springen; sie rollen, klappen und skalieren nicht. Die Fläche um eine Zahl darf pulsieren (2.2), die Zahl selbst nie.
- **Fokusringe.** Ein Frame. Ein Ring, der einblendet, wirkt wie Eingabeverzögerung.
- **Zeilenhöhen.** Eine Zeile ändert ihre Höhe nicht, weil sich ihr Zustand ändert: 36 px als Mindesthöhe in jedem Zustand, siehe 3.4. Dass dieselbe Zeile bei größerer Textskalierung höher ist, ist keine Animation, sondern Layout.
- **Direkte Manipulation.** Splitter, Scrollen, Textauswahl und Umsortieren per Ziehen folgen dem Zeiger eins zu eins. Nur die freigewordene Lücke animiert. Jede Dämpfung zwischen Hand und Pixel wirkt, als widerspräche das Programm der Hand.
- **Hover.** Hover tönt (`HMotion.press`) und deckt Bedienmöglichkeiten in bereits reserviertem Raum auf. Hover bewegt nichts und ändert keine Größe. Ein gefülltes Bedienelement tritt beim Hover sichtbar aus seinem Ruhezustand heraus und beim Drücken noch einmal aus dem Hover: zum Hover mindestens 0,04 in OKLab (`HColorDerivation.interactionStepMin`); zum Drücken tragen Farbe (mindestens 0,025, `interactionJumpMin`) und ein Ring den Schritt gemeinsam; der Ring liegt im immer vorhandenen Ein-Pixel-Rand und ändert nur seine Farbe (HUM-272). Das gilt in beiden Themes, ohne dass das Wort seine 4,5:1 oder die Füllung ihre 3:1 als Fläche verliert.
- **Leerlauf.** Es gibt keine Umgebungs-, Leerlauf- oder Dauerbewegung außer dem begrenzten Atem aus 2.7 und dem Absperrband aus 3.6, das nur läuft, solange seine Ursache besteht. Kein Spinner, nirgends (2.11).

### 2.9b Der gemeinsame Übergang

Ein `Hero` wird nicht erzwungen. Er beantwortet genau eine Frage, „wohin ist das Ding gegangen, das ich gerade angeschaut habe?", und gehört deshalb an die zwei Stellen, an denen das Auge sonst springen würde:

| Von | Nach | Was fliegt |
|---|---|---|
| Zeile in der Warteschlange | Karte im Detailbereich | der Host-Chip mit seinem Leistenstreifen |
| entschiedene Karte | Zeile im Verlauf | derselbe Chip, sobald beide Bildschirme nebeneinander sichtbar sind |

Alles andere kommt mit den Tokens aus Abschnitt 2.2 aus. Drei Bedingungen, sonst entfällt der Übergang ersatzlos: Quelle und Ziel sind gleichzeitig auf dem Bildschirm, das fliegende Element trägt in beiden Zuständen dieselbe Bedeutung, und die Strecke ist kürzer als eine Bildschirmdiagonale. Bei reduzierter Bewegung entfällt er immer; die Ankunft am Ziel zeigt dann dasselbe Aufziehen wie jede andere Auswahl.

Ein gemeinsamer Übergang darf nie das einzige Zeichen eines Zustandswechsels sein: Wer erst nach der Animation auf den Bildschirm schaut, muss dieselbe Aussage in Farbe, Glyphe und Text finden. Eine Animation, die keine Frage des Nutzers beantwortet, ist ein Defekt, keine Dekoration.

### 2.10 Reduzierte Bewegung

Alle Strecken und alle Schleifen laufen über `HReducedMotion` in `motion.dart`; kein Widget entscheidet selbst, ob es sich bewegen darf. Bei reduzierter Bewegung entfällt der Weg, nicht die Rückmeldung: `arriveOffset` und `leaveOffset` werden null, während Ausblenden, Tastenfüllung und Aufziehen der Leiste ihre vollen Dauern behalten.

Der Abgang ist der Fall, in dem „weniger Weg" versehentlich die Rückmeldung mitnimmt. Eine Zeile, deren Höhe sofort auf null fällt, hat nichts mehr, was ausblenden könnte; das Ausblenden liefe 220 ms über nichts. Bei reduzierter Bewegung behält die gehende Zeile deshalb ihre volle Höhe und blendet an Ort und Stelle über `HMotion.leave` aus; erst dann wird sie in einem Frame entfernt. Das Zusammenfallen entfällt, das Ausblenden bleibt; das ist die Regel, nicht ihre Ausnahme.

Jede Schleife bekommt einen ruhenden Ersatz statt gar nichts: Die atmende Glyphe wird zu einem dauerhaft doppelten Ring bei `HMotion.reducedRingAlpha`. Die 20-%-Schwelle ist Information, und wer Animationen abgeschaltet hat, darf sie nicht verlieren.

### 2.11 Warten

Warten ist ein Zustand wie jeder andere und bekommt deshalb eine Sprache, kein Symbol. Die Regel gilt überall gleich:

- Unterhalb von `HMotion.waitVisible` (150 ms) passiert nichts. Eine Anzeige, die kürzer als eine Reaktionszeit sichtbar ist, wird als Flackern gelesen.
- Ab 150 ms behält der Bereich sein Layout und zeichnet die Haarlinien-Skelette der erwarteten Zeilen: die Höhe der Zieldichte, `fg2`, keine Bewegung. Das Skelett sagt, wie viel gleich kommt und wo es stehen wird. Ein Spinner sagt nur, dass etwas läuft.
- Was einmal erschienen ist, bleibt mindestens `HMotion.waitMinVisible` (400 ms) stehen. Sonst erzeugt eine Antwort kurz nach der Schwelle genau das Flackern, das die Schwelle verhindern soll.
- Bei der Ankunft wird nichts verschoben: Das Skelett wird durch die Zeile ersetzt, die es beschrieben hat, am selben Ort, in einem Frame.
- Ein Fehlschlag während des Wartens ist ein Fehler und landet nach 4.4 an derselben Stelle, an der das Skelett stand; nie als Toast, nie bildschirmfüllend, solange der Rest des Bildschirms gültig ist.

Wo das im Einzelnen sitzt:

| Wartender Vorgang | Ort | Skelett |
|---|---|---|
| Body wird geladen und geparst (bis 8 MiB, HUM-030) | im Body-Bereich, unter dem schon sichtbaren Umschalter | zehn Zeilen der Zieldichte 24 px; bei `tooLarge` erscheint stattdessen sofort die Karte mit Größe und Funden |
| Probelauf einer Regel (HUM-033) | im Probelauf-Bereich, nie global | drei Ergebniszeilen zu 28 px unter der unveränderten Überschrift; der Editor bleibt benutzbar |
| Verlauf lädt mehr (HUM-032) | unter der letzten geladenen Zeile | so viele 28-px-Skelette, wie die Seite groß ist; die Tabelle springt bei der Ankunft nicht |
| Export (HUM-032) | im Export-Menü, am auslösenden Eintrag | kein Skelett, sondern Fortschritt in Zeilen („1.284 von 5.000"), weil die Zahl bekannt ist |
| Entscheidung unterwegs | in der Zeile und in der Aktionsleiste | keins. Die Leiste steht aufgezogen; das ist die Anzeige (2.5). |
| Daemon antwortet beim Start noch nicht | bildschirmfüllend | Splash nach denselben zwei Schwellen (4.2) |

---

## 3. Hierarchie und Farbe

### 3.1 Das eine Wichtige pro Bildschirm

Ein Bildschirm hat genau ein größtes Textelement, genau ein gefülltes Bedienelement und genau einen leeren Zustand. Alles andere ist Ghost, getönt oder neutral.

„Gefüllt" heißt: Das Bedienelement trägt den Akzent als Fläche, nicht als Umriss; entweder als volle Akzentfläche mit `onAccent`-Text (`HButtonVariant.primary`) oder als Akzent-Tönung an der Token-Obergrenze (`HColors.tintAlpha`, 10 %), wie das Freigabeventil. Alles andere ist Ghost: Rand oder Text, keine Fläche.

| Bildschirm | Das eine Wichtige | Größte Schrift | Einziges gefülltes Bedienelement |
|---|---|---|---|
| Anhalten | die URL der ausgewählten Anfrage | `mono14` im Kopf der Karte | Freigabeventil |
| Verlauf | die gefilterte Liste | `mono14` im Kopf des Details | keines; der Akzent gehört dem Filterfeld im Fokus |
| Regeln | die Reihenfolge der Regeln | `ui14` für die Regel als Satz | `+ New rule`, solange die Liste leer ist, sonst `Save` im Editor |
| Einrichten und Sandbox | der Zustand der drei Garantien | `ui16` für das Urteil | die Aktion des obersten Diagnostic |

Die Regel gilt pro **Bildschirm**, nicht pro Bereich. Ein Bereich hat kein eigenes Wichtigstes, das mit dem des Bildschirms konkurrieren könnte: Die erste Zeile des Kontextbereichs in Anhalten trägt die Antwort „bekannt oder unbekannt" in `ui14` und bleibt damit unter der URL im Kopf der Karte. In `ui16` stünde sie über dem Gegenstand der Entscheidung, und das selbst dann, wenn sich `mono14` und `ui16` nicht allein über die Zahl vergleichen lassen; im Zweifel gewinnt die URL, weil es auf dem Bildschirm um sie geht.

Ein leerer Bildschirm ist die Ausnahme, die die Regel braucht: Solange ein Bildschirm keinen Inhalt hat, ist seine wichtigste Anlege-Aktion gefüllt, damit überhaupt ein Weg nach vorn sichtbar ist; sobald er Inhalt hat, wird sie Ghost, und die Füllung geht an das Bedienelement der Entscheidung. Genauer: **ein gefülltes Bedienelement pro Entscheidungskontext**. Ein Modal, ein Sheet und der Bildschirm darunter sind drei Kontexte, aber nie gleichzeitig aktiv.

Heute fällt die Skala auf 11/12/13 zusammen: `ui20` steht nur auf Platzhaltern, `ui14` und `mono14` nirgends, und die URL, der Gegenstand der Entscheidung, läuft in `mono13`. Von nun an gehören `ui16` und `ui20` jeweils dem Wichtigsten, nicht dem Unfertigsten.

### 3.2 Der Dichte-Rhythmus

- Basis 4 px. Jeder Abstand ist ein Vielfaches davon; `x1/2 = 2 px` gibt es nicht.
- Innenabstand der Bereiche 12 px. Jeder rechte Rand endet bei 12 px, auch der des Warteschlangenkopfs, damit Zähler und Countdowns in einer Spalte fluchten.
- Drei Zeilendichten, nicht mehr: 36 px in Warteschlange und Regeln, 28 px in der Verlaufstabelle, 24 px in Body-Ansichten. Alle drei sind Mindesthöhen (Abschnitt 6, Textskalierung), und alle drei brauchen ein Token (Abschnitt 9).
- **Satzbreite höchstens 90 Monospace-Zeichen**, für Fließtext und für einzeilige URL- und Pfadfelder. Überschüssige Bereichsbreite wird Rand, nie Zeilenlänge. Bei 2560 px liefe eine URL sonst über 137 Zeichen, und das Auge sucht den Anfang der nächsten Zeile.
- **Code, Hex und Tabellen brechen nie um.** Roher Body, Hex-Ansicht und die Verlaufstabelle scrollen horizontal und bekommen keine Satzbreite. Ein Umbruch dort erzeugt optische Zeilen, die die Byte-Offsets der Funde verschieben (HUM-030, Fallstrick „wrap off"); ein falsch platzierter Fund ist schlimmer als eine lange Zeile.
- Bereiche 28/44/28 bis 1800 px Fensterbreite. Darüber friert der Inspektor bei Satzbreite plus Rand ein, Warteschlange und Kontext nehmen den Rest. Unter 1280 px klappt der Kontextbereich zu und ist über `Ctrl+D` erreichbar.
- Die Naht zwischen zwei Bereichen gehört dem Splitter. Ein Bereich zeichnet weder einen linken noch einen rechten Rand; zwischen zwei Bereichen liegt genau eine Haarlinie, und der Griff des Splitters liegt unsichtbar darüber (3.7).
- Ein Bildschirm zeigt einen leeren Zustand, nicht einen pro Bereich. Solange der Hauptbereich leer ist, bleiben die Seitenbereiche still.

Die Satzbreite wird als Zeichenzahl angegeben, nicht als Pixelbreite, weil die Breite von der installierten Schrift abhängt. Für wen trotzdem rechnet: JetBrains Mono und jeder Fallback der Kette laufen mit etwa 0,6 em Vorschub, also etwa 700 px bei `mono13` und etwa 650 px bei `mono12`; `queue_row.dart` rechnet mit 7,3 px pro Zeichen bei 12 px. Diese Zahlen sind Veranschaulichung; die Zeichenzahl ist normativ (Abschnitt 9).

### 3.3 Farbe bedeutet Zustand

Der Akzent markiert, was den Fokus nehmen oder angeklickt werden kann: `HColors.accent` (#E8B84B) und `HColors.lAccent` (#A57A1C). Seit HUM-260 ist er das Gelb der Marke, das App-Icon ist eine schwarze Faust auf einer gelben Kachel (#EBC05A bis #D49E34), im hellen Theme als Ocker, weil Gelb auf Weiß unlesbar ist. Was auf dem Akzent steht, Text wie Glyphe, ist dunkel: `HColors.onAccent` (#161616), in beiden Themes, nie Weiß. Die Zahlen: Akzent auf `bg0` bis `bg3` mindestens 8,21:1, Ocker auf `lBg0` bis `lBg3` mindestens 3,29:1 (Flächenschwelle), `onAccent` auf dem Akzent 9,81:1, auf dem Ocker 4,65:1. Ein Wort in Akzentfarbe steht in `accentText`, nie in `accent`. Inerter Text ist nie Akzent; eine akzentfarbene URL, die niemand öffnen kann, bringt dem Auge bei, dass Akzent nichts bedeutet, und dann brechen Fokus, Auswahl und Primärknopf mit. Hover und Drücken des Primärknopfs verschieben nicht nur die Helligkeit, denn das schmale Fenster des Ockers lässt dafür kaum Raum, sondern auch Chroma und Farbton, immer zum Kräftigeren, nie zum Blasseren: Hover heller und gesättigter (`accentFillHover`), Drücken nah am Ocker, gesättigter und, wo das Wort es zulässt, dunkler (`accentFillPressed`), dazu der Ring `accentPressedRing` (`onAccent` zu 75 Prozent, mindestens 3:1 gegen die gedrückte Füllung). Im hellen Theme dreht der Farbton zum Orange, im dunklen bleibt er gelb (HUM-272).

Zustandsfarben stehen nur auf Zustand, aus `HStateColors` über `HTheme.of(context).state` oder `tokens.stateColor(state)`:

| Zustand | Token | Dunkel | Hell |
|---|---|---|---|
| `held` | `HColors.held` | #D8B1E1 (bis HUM-355 #79C9EC, bis HUM-260 #E0B24A) | abgeleitet, `HStateColors.light` (#B56BC6) |
| `allowed` | `HColors.allowed` | #4FBF8C | abgeleitet |
| `allowedEdited` | `HColors.allowedEdited` | #57B99F plus Akzentpunkt | abgeleitet |
| `blocked` | `HColors.blocked` | #E5646E | abgeleitet |
| `timedOut` | `HColors.timedOut` | #909090 (bis HUM-355 #8A90A2) | abgeleitet |
| `autoRule` | `HColors.autoRule` | `allowed` zu 60 % | abgeleitet |
| `passthroughLlm` | `HColors.passthrough` | #B48AF0 | abgeleitet |
| `error` und Secret | `HColors.secret` | #F0784F | abgeleitet |

Neben den Zuständen gibt es seit HUM-260 zwei Töne, die kein Zustand eines Flows sind und deshalb nicht in `HStateColors`, sondern in `HSurfaceColors` stehen:

| Bedeutung | Token | Dunkel | Hell |
|---|---|---|---|
| Aufmerksamkeit: Warnung, offene Funde, beschreibbarer Mount, ein noch nicht abgeschlossener Vorgang | `colors.warning`, Wort `colors.warningText` | #E8B84B (gleich dem Akzent) | #A57A1C (gleich dem Akzent) |
| Information: Diagnostic der Stufe `info` | `colors.info`, Wort `colors.infoText` | #E0E0E0 (bis HUM-355 #7C9CF5) | #404040 (bis HUM-355 #5B7FE6) |

Regeln dazu:

1. **Volle Sättigung nur, wo zwei Zustände nebeneinanderstehen.** In der Warteschlange von Anhalten steht konstruktionsbedingt nur `held`; fünfzehn volle `held`-Leisten wiederholen eine Tatsache und werden zum Lautesten auf dem Bildschirm, während die eigentliche Entscheidung neutral bleibt. Dort trägt die Leiste `tokens.tint(tokens.stateColor(state))` (10 % Alpha). Im Augenblick der Entscheidung zieht sie auf volle Sättigung auf; dann ist sie die einzige gesättigte Fläche im Bereich. Der Verlauf behält die volle Sättigung, weil dort acht Zustände gemischt stehen.
2. **Farbe ist nie der einzige Kanal.** Jede Zeile, die Zustand trägt, trägt Glyphe und Farbe. Im hellen Theme messen `allowed` und `blocked` unter Deuteranopie 1,01:1; eine 4 px breite Farbe allein sagt dort nichts. Die Glyphentabelle gibt es in `flow_state.dart` schon, und sie wurde bisher nicht benutzt.
3. **Kein Farbwechsel durch Interpolation.** `held` #D8B1E1, linear zu `blocked` #E5646E überblendet, liefe durch ein Rosa nahe #DE8BA8, das zu keinem Zustand gehört, und zeigte einen Zustand, den es nie gab (mit dem Cyan-`held` vor HUM-355 war es ein blasses Malve, #AF96AD). Deshalb zieht die Leiste auf, statt überzublenden.
4. **Methoden-Badges sind in Listen neutral** (`fg1` auf `bg2`). Die Methodenfarben borgen Zustandsfarben (PUT/PATCH ist `held`, DELETE ist `blocked` zu 70 %, POST ist `passthrough`); in einer Liste liest das Auge ein rötliches Badge neben einer roten Leiste als zwei Blockierungen, nicht als Verb und Zustand. Nur das eine Badge im Kopf der Karte bekommt eine Farbe.
5. **Rahmenelemente, die sich nicht ändern können, tragen `fg1` oder `fg2`.** Eine Zustandsanzeige hat mindestens zwei Werte, sonst ist sie keine. „Anhalten AN" und der Verbindungspunkt sind heute immer grün, weil die Shell nur im verbundenen Zustand gebaut wird; zwei dauerhaft grüne Marken bringen dem Auge bei, Grün zu übersehen, und Grün ist die Farbe der einen Aktion, auf die es ankommt. Entweder anschließen oder entfernen.
6. **Rot heißt blockiert.** Fehler und Funde sind orange (`HColors.secret`). Rot als Stimmung gibt es nicht.
7. **In Body-Ansichten sind Funde die einzige Farbigkeit.** JSON-Schlüssel, Werte und Hex bleiben auf der `fg`-Leiter. Ein Body ist der Ort, an dem ein Secret binnen Sekunden auffallen muss; drei Farben für Datentypen lassen den Fund mit der Syntax konkurrieren. Das weicht von HUM-030 ab, siehe Abschnitt 8.
8. **Ein Bedienelement, das Zustand zählt, ist trotzdem Akzent.** `+n new`, der Fund-Chip, der Gruppenzähler und der Sammelknopf tragen alle eine Zahl über angehaltene Anfragen, und sie bleiben Akzent, weil man sie berühren kann. Der Zustand, den sie zählen, steht als Glyphe darin, nie als Füllung. Sonst zerfällt der Akzent in so viele Bedeutungen, wie es Zähler gibt.
9. **Was nur anzeigt und nichts auslöst, trägt Zustandsfarbe.** Das Tray-Icon ist kein Bedienelement, sondern die einzige Zustandsanzeige der Session, solange das Fenster nicht sichtbar ist; sein Zähler steht deshalb in der Farbe von `held`, nicht im Akzent (Abweichung von HUM-034, siehe Abschnitt 8). Regel 8 und Regel 9 zusammen: Eine Fläche zum Berühren ist Akzent, eine Fläche zum Lesen ist Zustand.
10. **Die ruhende `held`-Leiste ist von der 3:1-Regel ausgenommen.** Eine 10-%-Tönung von `held` auf `bg1` misst 1,21:1; 3:1 erreichte sie erst bei 46 % Alpha, und bei 50 % ist sie keine Tönung mehr, sondern die gesättigte Fläche, die Regel 1 der Entscheidung vorbehält. Dort gruppiert die Leiste nur; Zustand und Frist tragen Glyphe, Beschriftung und Semantik. Die Ausnahme gilt namentlich für die ruhende Zustandsleiste in der Warteschlange und für nichts sonst; die aufgezogene Leiste, die Auswahlleiste und jede Leiste im Verlauf erreichen 3:1. `HColorDerivation.tint` deckelt bei `HColors.tintAlpha`; eine stärkere Leiste wäre keine Ermessensfrage, sondern ein neues Token (Abschnitt 9).

11. **Akzent und `held` sind nie dieselbe Farbfamilie.** Bis HUM-260 war `held` bernsteinfarben und der Akzent blau; seit der Akzent das Gelb der Marke ist, trug `held` ein kühles Himmels-Cyan und seit HUM-355 eine helle Orchidee, weil das Cyan neben dem Gelb als Blau gelesen wurde. Ein gelber Zähler, der „hier berühren" bedeutet, und eine gelbe Leiste, die „wartet" bedeutet, wären genau die Verwechslung, die Regel 8 und 9 ausschließen. Der Abstand von `held` zu jeder anderen Zustandsfarbe und zum Akzent wird in `tokens_test.dart` gemessen, in OKLab, normal und unter simulierter Protanopie und Deuteranopie (Machado 2009), und beträgt überall mindestens 0,08; das alte Bernstein läge neben dem neuen Akzent bei 0,02. Unter Protanopie fallen die Orchidee und das Violett von `passthrough` auf dasselbe Blau; dort trennt sie nur die Helligkeit, und `held` ist deshalb heller als `passthrough`. Die Orchidee ist der eine Farbton, der das in beiden Themes hält: Ein gesättigtes Magenta fällt auf `passthrough`, Petrol auf `allowedEdited`, und ein weißes `held` leitet sich im hellen Theme zu einem Grau ab, das Deuteranopie nicht vom abgeleiteten Petrol unterscheiden kann.

12. **Gelb heißt Aufmerksamkeit, Silber heißt Information, Orchidee heißt angehalten.** Warnungen tragen `warning`, das Gelb der Marke, wie das Absperrband (3.6); Diagnostics der Stufe `info` tragen `info`, seit HUM-355 ein neutrales Silber (im hellen Theme Graphit) statt des Blaus, das bis HUM-260 der Akzent war, weil kein freier Farbton neben dem Gelb zu jeder Zustandsfarbe Abstand hält. Silber liegt nah an `fg0` und Graphit nah an `lFg1`, also wird ein `info`-Befund nie allein an der Farbe erkannt: Er trägt immer seine Glyphe und sein Wort; `held` steht jetzt nur noch auf einer angehaltenen Anfrage. Vorher trugen Warnungen das Bernstein von `held`, und nach dem Wechsel von `held` zu Cyan hätte sich eine Cyan-Warnung neben einer gelben Information verkehrt herum gelesen. Weil `warning` im Wert mit dem Akzent zusammenfällt, steht eine Warnung **nie als Farbe allein**: immer mit Glyphe (`triangleAlert`) oder Text, ein Fund im Text mit der Welle statt der geraden Linie der Ersetzung. Ein gelbes Bedienelement mit einer Warnung ist trotzdem ein Bedienelement (Regel 8); das Freigabeventil mit einem offenen Fund ist eines, und Beschriftung und Glyphe sagen, warum es anders aussieht.

### 3.4 Die Zeile der Warteschlange, ausgeschrieben

Weil die meisten Regeln hier zusammenlaufen, steht sie einmal vollständig ausgeschrieben:

```
| 4 Rail | 8 | 16 state glyph with countdown ring | 8 | Method neutral | 8 |
  Host ui13/500 | 8 | Path mono12, shortened in the middle | 8 | Findings chip | 8 | Action slot 28 | 12 |
```

- **Höhe: mindestens 36 px, in jedem Zustand, ohne zweite Zeile.** Eine Auswahl, die von 36 auf 56 px wächst, schiebt bei jedem `J` alles darunter um 20 px nach unten; die zweite Zeile wiederholt außerdem Größe, Content-Type und Wartezeit, die die Karte zwei Bereiche weiter ohnehin zeigt. 36 px sind eine Mindesthöhe und keine feste: Bei `TextScaler.linear(2.0)` misst `ui13` allein 40 px Zeilenhöhe, und eine feste Höhe würde den Überlauf stillschweigend verschlucken. Die Zeile wächst also mit der Skalierung, aber nie mit ihrem Zustand.
- **Was wann weicht.** Feste Breiten verbrauchen 108 px Rahmen plus Methoden-Badge (GET etwa 36 px, DELETE etwa 56 px) plus Fund-Chip (etwa 24 px); bei `HSize.paneMinQueue` (280 px) bleiben für Host und Pfad zusammen etwa 100 px. Deshalb steht die Reihenfolge, in der gekürzt wird, hier und nicht im Ermessen des Layouts: Der Host bekommt eine Untergrenze von 12 em und wird darüber hinaus in der Mitte gekürzt; der Pfad weicht zuerst und verschwindet unter etwa 360 px Bereichsbreite ganz; der Fund-Chip schrumpft unter etwa 320 px zu einem 4-px-Punkt in `HColors.secret`. Der Host geht als Letztes; er ist die Antwort auf „wohin".
- **Die Leiste:** `held` als Tönung, entschieden in voller Farbe, ausgewählt in Akzent über die vollen 4 px. Die Auswahl **ersetzt** die Zustandsleiste, sie überlagert nicht deren linke Hälfte; das steht schon in der Dokumentation von `spacing.dart` und funktioniert, sobald die Zeile ihre Glyphe trägt.
- **Die Füllung:** Hover `bg2`, Auswahl `bg3`. Nie dieselbe Farbe. `Enter` und `A` senden die ausgewählte Zeile, und Senden ist unumkehrbar; sieht eine Zeile unter dem Zeiger genauso aus wie die ausgewählte, liest jemand die eine und sendet die andere. `bg2` steht genau für diesen Zwischenschritt in der Leiter und wurde in Zeilen bisher nicht benutzt.
- **Kein `mm:ss` in der Zeile.** Die verbleibende Zeit steht im Bogen des Rings und im Semantics-Wert; die Ziffern gehören zur ausgewählten Anfrage in der Karte. Fünfzehn gleichzeitig laufende Ziffern sind das Gegenteil eines ruhigen Leitstands.
- **Der Aktionsplatz** ist immer 28 px breit und in Ruhe leer. Hover **und** Fokus blenden dort das Blockieren ein; es ersetzt nichts, verschiebt nichts und lässt keinen Text neu umbrechen. Heute tauscht die Zeile beim Hover etwa 33 px Countdown gegen etwa 97 px Aktionen, bricht dabei die Titelzeile neu um und legt „Allow" direkt unter den Zeiger.
- **Aus der Zeile ist nur Blockieren möglich, nie Senden**, und Blockieren verlangt 250 ms Halten. Senden ist unumkehrbar und braucht die URL; die steht in der Karte. Das hält auch die Regel ein, dass Senden und Blockieren nie nebeneinanderstehen; im schmalsten Bereich wären 280 px sonst zu wenig für einen ehrlichen Abstand.
- **Der Bestätigungsstreifen ist die Zeile, keine Box darüber.** Während des Bestätigungsfensters (`HMotion.confirm`) tauscht die entschiedene Zeile ihren Inhalt gegen den Streifentext, auf derselben Grundlinie, in derselben Höhe: Leiste in voller Zustandsfarbe, Zustandsglyphe voll gesättigt links, dann der Satz in `mono12`. Keine zweite Zeile, keine zusätzliche Box, keine Höhenänderung, kein neuer Umbruch. Danach geht die Zeile nach 2.4.

### 3.5 Auswahl, Mehrfachauswahl, Gruppenkopf

Die Mehrfachauswahl ist zugleich Einfrierbedingung (2.8) und Schutzstufe (5.4). Sie braucht deshalb ein eigenes, eindeutiges Erscheinungsbild, und sie bekommt es über die Leiste, nicht über die Füllung:

- **Die Leiste sagt, was ausgewählt ist.** Jedes Mitglied einer Auswahl trägt die Akzentleiste über die vollen 4 px, ob eines oder zwölf.
- **Die Füllung sagt, wo der Cursor ist.** Hover `bg2`, die zuletzt angeklickte oder mit `J`/`K` erreichte Zeile `bg3`, genau eine Zeile im ganzen Bereich. Ein Mitglied ohne Cursor bleibt ungefüllt; die Leiste allein trägt die Zugehörigkeit.
- **Es erscheint kein zweites gefülltes Bedienelement.** Bei einer Auswahl größer als eins beschriftet sich das Freigabeventil um ("Allow 5 selected" / "5 ausgewählte senden") und bleibt das eine gefüllte Bedienelement des Bildschirms (3.1). Ein zusätzlicher Sammelknopf daneben wäre das zweite.

Der Gruppenkopf ist eine Zeile und hält die Zeilenregel ein: Im 28-px-Aktionsplatz zeigt er nur `Block {n}`. `Allow {n}` steht dort nicht; dieselbe Begründung wie in 3.4, verstärkt um den Faktor n. Sobald eine Gruppe ausgewählt ist, übernimmt die Aktionsleiste das Senden, und die Karte daneben zeigt, worüber entschieden wird: Host, Methodenmix, Pfade, Funde insgesamt. Das weicht von HUM-029 ab, siehe Abschnitt 8.

### 3.6 Das Absperrband

Humanitl ist die Schranke, an der der Verkehr anhält. Sein Zeichen ist ein gelb-schwarzes Absperrband: diagonale Streifen in Markengelb (`HColors.accent`) und `HColors.onAccent`, die gleichmäßig seitwärts laufen (`HCautionTape`, HUM-260). Es ist das einzige Element des Programms, das endlos läuft, und darf deshalb nur dort stehen, wo es etwas bedeutet.

**Wo es steht.** An genau zwei Stellen:

1. **Über der Warteschlange, solange mindestens eine Anfrage angehalten ist.** Vier Pixel hoch (`HSize.cautionTape`) unter dem Kopf des Warteschlangenbereichs. Es kommt mit der ersten angehaltenen Anfrage und geht mit der letzten. Die Schranke steht, solange jemand an ihr wartet.
2. **Als unbestimmte Fortschrittsanzeige der Netzwerksuche in der Einrichtung** („Suche läuft"), solange die Suche läuft. Dort ersetzt es die Haarlinie, die früher hin und her lief, und sagt dasselbe: Es arbeitet, ohne einen Fortschrittsstand zu behaupten.

**Wo es nie steht.**

- Nie in einer Zeile, auf einer Karte, in einem Body oder über Text, den jemand liest (2.8, 2.9). Es ist Rand, nie Inhalt.
- Nie als Zustandsanzeige. Das Band zeigt nicht, *wie viele* warten oder *welche*; das tun Zähler, Leiste und Glyphe. Es darf nie der einzige Kanal einer Aussage sein: Wer es nicht sieht, verliert nichts, was nicht im Zähler im Kopf der Warteschlange steht.
- Nie als Bedienelement. Es nimmt keinen Fokus, hat keine Semantik (`ExcludeSemantics`) und reagiert auf keinen Zeiger. Dass es gelb ist wie der Akzent, ist die Marke, keine Einladung zum Berühren (3.3, Regel 8); deshalb trägt es immer auch die schwarzen Streifen und nie eine Akzentfläche allein.
- Nie in einer Zustandsfarbe und nie in einer Farbe, die sich je Theme ändert. Es ist ein Band, kein Token der Leiter; im hellen Theme ist es dasselbe Gelb und Schwarz wie im dunklen.
- Nie an einer dritten Stelle, ohne dass dieser Abschnitt sie nennt. Ein Band an jeder Ecke ist Tapete.

**Wie es läuft.** Linear, eine Periode (16 px, ein gelber und ein schwarzer Streifen) in `HMotion.cautionTape` (1200 ms), also etwa 13 px pro Sekunde: Man sieht, dass es läuft, und es zieht nicht am Auge. Nahtlos: Eine Verschiebung um eine Periode zeichnet dasselbe Bild wie keine. Sein Platz ist immer reserviert, auch ohne das Band, dann steht dort die Haarlinie, damit nichts darunter springt, wenn es kommt oder geht (2.8). Es baut sich im Lauf nicht neu, sondern malt nur seine eigenen Pixel hinter einer `RepaintBoundary`; ohne Ursache ist es nicht im Baum und hat keinen Ticker, und in einer unsichtbaren Ansicht verstummt sein Ticker über `TickerMode`.

**Reduzierte Bewegung** (2.10): Die Streifen stehen still, das Band bleibt. Der Weg entfällt, die Aussage bleibt.

**Tests und Goldens** halten das Band über `HCautionTape.debugFreeze` an, gesetzt in `app/test/flutter_test_config.dart`; ein endloses Band liefe sonst in jedes Zeitlimit von `pumpAndSettle`, und ein Golden zeigt es als Standbild bei Phase null.


### 3.7 Layout-Raster

Ein Raster für die ganze Shell (HUM-286). Früher erfand jede Fläche ihre eigene Geometrie: Der Kopf des Einstellungs-Sheets war höher als der Kopf der Warteschlange daneben, die Aktionsleiste saß eingerückt in einer eigenen Box, der aktive Eintrag der Leiste hielt eine Lücke zur Linie darüber, und die Regelliste zog ein paar Pixel unter der Werkzeugleiste eine zweite Linie. Die Regeln unten sind der Grund, warum nichts davon zurückkommen kann.

**Zeilen.** Unter dem Fensterkopf (`headerBar`, 40 px) beginnt jeder Bereich auf derselben Linie. Die erste Zeile ist der Bereichskopf, `paneHeaderHeight` (48 px) hoch, seine untere Kante eingeschlossen. Warteschlange, Anfragekarte, Kontextbereich, Editor, Verlauf, Regeln, Sandbox und jedes Seiten-Sheet (Einstellungen, Regel aus Anfrage, Verlaufsdetail, Audit-Eintrag, Kommandozeile) benutzen ihn. Die Leiste hat dieselbe Zeile: Ihre Einträge sind `railItemHeight` (48 px, gleich dem Kopf) hoch und beginnen direkt unter dem Fensterkopf, sodass „Anhalten" in der Leiste und „Warteschlange" im Bereich auf einer Linie stehen.

**Ein Widget, eine Höhe.** Ein Bereichskopf ist `HPaneHeader` und nichts anderes. Er setzt die Höhe aus dem Token und zentriert seinen Inhalt senkrecht; er hat waagerechten Innenabstand (12 px) und keinen senkrechten. Ein Kopf, der seine Höhe über eigenen Innenabstand erreichte, änderte sich mit der Schrift, und die Linie darunter liefe nicht mehr in den Kopf nebenan weiter. Die Höhe ist fest, und sie folgt der Textskalierung einmal für alle: `HPaneHeader.rowHeight` skaliert das Token mit dem umgebenden Text-Scaler (auf 1 bis 2 begrenzt, auf ganze Pixel gerundet), jeder Kopf und jeder Eintrag der Leiste nimmt diesen einen Wert, und Inhalt, der nicht passt, wird abgeschnitten, statt seine Linie aus der Zeile zu drücken. Eine zweizeilige URL in der Anfragekarte passt bei jeder Skalierung bis zum Doppelten. Die Trennlinie unter dem Kopf gehört dazu; in der Warteschlange ist das das Absperrband oder, ohne es, die Haarlinie in denselben 4 px (3.6).

**Ränder.** Jeder Rand und jede Trennlinie ist `borderWidth` (1 px, die Haarlinie). Zwei benachbarte Regionen teilen sich eine Trennlinie, und sie gehört der oberen (oder, nebeneinander, dem Splitter). Was unter einem Kopf steht, beginnt direkt unter dessen Linie und zeichnet keinen eigenen oberen Rand; ein verschachtelter Container fügt nie eine Linie über der Linie hinzu, unter der er schon steht. Ein Block, über dem ein Trenner steht, lässt ihn fallen, wenn er an den Anfang seiner Liste rückt (die mitgelieferten Regeln unter dem Kopf von Regeln).

**Hervorhebungen und Leisten schließen bündig ab.** Der aktive Eintrag der Leiste füllt seine ganze Zeile von Kante zu Kante, Füllung und Akzentmarke von der Linie darüber bis zum nächsten Eintrag, ohne Abstand. Die Aktionsleiste des Inspektors ist eine Fußzeile ihres Bereichs: volle Breite von Naht zu Naht, bis hinunter zur Statusleiste, von der Karte durch einen Rand getrennt, ohne innere Karte und ohne Einzug. Die Nähte zwischen Bereichen sind eine Haarlinie, nie ein Streifen Hintergrund neben einer Linie (3.2).

**Sheets hängen zwischen den Leisten.** Ein Seiten-Sheet beginnt unter dem Fensterkopf und endet an der Statusleiste, wie ein Bereich; sein Kopf ist derselbe `HPaneHeader`, sodass die Trennlinie bei offenem Sheet quer durch das Fenster weiterläuft. Ein Bildschirm hängt sein Sheet an seiner eigenen Oberkante ein, nie innerhalb einer Region unter seinem Kopf (den schmalen Regeleditor von Regeln hängt der Bildschirm ein, nicht die Liste).

**Ansichten ohne Kopf.** Einrichten und Audit beginnen mit Inhalt, nicht mit einem Bereichskopf (ein Dokument und eine Statuskarte). Sie beginnen mit dem Innenabstand der Bereiche unter dem Fensterkopf. Bekommt eine von ihnen einen Kopf, ist es ein `HPaneHeader`.

**Geprüft durch** `app/test/features/shell/layout_grid_test.dart`: In Anhalten (mit und ohne Einstellungs-Sheet), Verlauf, Regeln und Sandbox beginnt bei Textskalierung 1 und 2 jeder sichtbare Bereichskopf unter dem Fensterkopf und endet auf derselben Linie, und der aktive Eintrag der Leiste füllt seine Zeile; das Einstellungs-Sheet, das schmale Regeleditor-Sheet und das Sandbox-Befehls-Sheet beginnen unter dem Fensterkopf, und ihre Kopflinie setzt die daneben fort. Zwischen zwei Bereichen geht Tab Bereich, Splitter, Bereich (`app/test/core/ui/design_system_test.dart`).

---

## 4. Den Menschen führen

Prinzip 7 aus `BACKLOG.md` gilt wörtlich: kein Zustand ohne Grund in Alltagssprache, kein Fehler ohne „warum" und „was jetzt". Alle Texte kommen aus ARB, keiner steht im Code. Englisch ist die Quellsprache, Deutsch die Übersetzung; jeder Satz, den dieses Dokument festlegt, steht deshalb in beiden Sprachen, Englisch zuerst, in der Reihenfolge, in der die ARB-Dateien ihn führen.

### 4.1 Leere Zustände

Es gibt zwei leere Zustände, und wer sie verwechselt, verspricht dem Menschen eine Zukunft, die es nicht gibt.

**Leer, weil noch nichts passiert ist.** Der Satz nennt das nächste Ereignis, nie das Fehlen: „`<subject>` erscheint hier, wenn `<trigger>`". Die Wörter „nichts", „kein" und „noch" kommen darin nicht vor. Ein leerer Bildschirm ist der Bildschirm, den ein neuer Nutzer am längsten sieht, und die einzige Lehrfläche, die das Programm umsonst bekommt.

**Leer, weil ein Filter nichts trifft.** Hier gibt es keine kommende Zukunft; die Menge ist da, und der Filter schneidet sie weg. Der Satz nennt den Filter, die Zahl der passenden Einträge und den Weg zurück, und der Weg zurück ist ein Akzent-Bedienelement, nicht Teil des Satzes: en "`host:foo` matches 0 of 1,284 requests · Reset filter", de "`host:foo` trifft 0 von 1.284 Anfragen · Filter zurücksetzen". Das gilt im Verlauf und in Regeln.

**Leer als Tatsache über ein abgeschlossenes Objekt** ist keiner der beiden. Eine Anfrage ohne Query-Parameter bekommt nie welche; dort ist "No parameters." / "Keine Parameter." richtig, und der Katalog der verbotenen Wörter gilt nicht. Die Unterscheidung ist prüfbar: Gibt es ein Ereignis, das die Fläche füllen würde, ist es ein leerer Zustand; gibt es keins, ist es eine Aussage.

Die leere Warteschlange zeigt zusätzlich die drei Tasten, einmal und nie wieder. Die Reihenfolge beginnt mit der umkehrbaren Taste, und `Enter` nennt seine Folge: en "`B` block · `Enter` send (final) · `J`/`K` move", de "`B` blockieren · `Enter` senden (endgültig) · `J`/`K` bewegen". Eine Lehrfläche, die mit der unumkehrbaren Taste beginnt, bringt genau das Falsche zuerst bei.

Heute verletzen fünf ausgelieferte Schlüssel die erste Formel, und sie werden zusammen mit dem Bildschirm ersetzt, der sie benutzt:

| Schlüssel | heute | stattdessen |
|---|---|---|
| `shellSectionPlaceholder` | "Nothing here yet." / "Hier ist noch nichts." | der Bildschirm sagt, was er tun wird, oder fehlt (4.2, letzter Absatz) |
| `interceptEmptyTitle` | "No request is waiting" / "Keine Anfrage wartet" | en "The queue is open", de "Die Schleuse ist offen" |
| `interceptEmptyHint` | "The agent works without a network." / "Der Agent arbeitet ohne Netz." | en "Every request the agent makes appears here.", de "Jede Anfrage des Agenten erscheint hier." „ohne Netz" liest sich als „ohne Sicherheitsnetz" und behauptet außerdem etwas Falsches, solange keine Session läuft (4.2) |
| `interceptCardEmptyTitle` | "Nothing selected" / "Nichts ausgewählt" | en "Pick a request", de "Eine Anfrage wählen" |
| `shellIsolationUnknown` | "Isolation: not checked yet" / "Isolation: noch nicht geprüft" | en "Isolation: check running", de "Isolation: Prüfung läuft"; und läuft keine, nennt der Satz die Aktion, die eine startet |

`interceptQueryEmpty`, `interceptHeadersEmpty` und `interceptBodyEmpty` bleiben, wie sie sind: Sie sind Aussagen über eine abgeschlossene Anfrage. Damit die verbotenen Wörter nicht mit dem nächsten Bildschirm zurückkommen, prüft ein ARB-Lint sie in beiden Sprachen und kennt diese drei Ausnahmen namentlich (Abschnitt 9).

### 4.2 Vor der ersten Anfrage

Der erste Bildschirm ist nicht die leere Warteschlange, sondern die Frage, ob überhaupt jemand antwortet. Vier Zustände, jeder mit Text, Ort und genau einer Aktion:

1. **Der Daemon antwortet noch nicht.** Splash, bildschirmfüllend, nach den Schwellen aus 2.11: unter `HMotion.waitVisible` nichts, danach sichtbar und mindestens `HMotion.waitMinVisible` lang stehend. Kein Spinner, kein Fortschrittsbalken für etwas, dessen Dauer niemand kennt. Ein Splash, der bei einem schnellen Start aufblitzt, ist schlimmer als kein Splash.
2. **Der Daemon fehlt beim Start.** Einrichtungsbildschirm. Das `Diagnostic` ist das eine Wichtige des Bildschirms (3.1): `why` vom Daemon oder aus dem Transportfehler, `fix` als gefülltes Bedienelement. Nichts anderes auf diesem Bildschirm konkurriert darum.
3. **Der Daemon läuft, aber keine Session.** Die Warteschlange sagt nicht, der Agent arbeite ohne Netz; niemand arbeitet. Sie nennt das nächste Ereignis, das der Mensch auslöst: en "Once a session runs, every request the agent makes appears here.", de "Sobald eine Session läuft, erscheint hier jede Anfrage des Agenten." Die einzige Aktion ist en "Start session" / de "Session starten", und sie ist gefüllt, weil der Bildschirm leer ist (3.1).
4. **Die Verbindung bricht ab, während die Warteschlange läuft.** Die Shell bleibt. Der Einrichtungsbildschirm ersetzt sie nur bei einem Kaltstart, nie mitten in der Arbeit; wer zwölf wartende Anfragen auf dem Bildschirm hat, verlöre sonst den Bildschirm und erführe nicht, was mit dem Agenten geschehen ist. Stattdessen: Die Warteschlange wird als eingefrorener Snapshot markiert (Zeitstempel im Kopf, alle Countdowns halten an, alle Entscheidungstasten still), darüber ein Banner mit Grund, Folge für den Agenten und der einen Aktion en "Reconnect" / de "Erneut verbinden".

Sobald es Verkehr gibt, aber nichts ausgewählt ist, zeigt der Kontextbereich die Zusammenfassung der Session, die zu seiner Ansicht gehört (gesehene Hosts, die fünf häufigsten, Funde insgesamt); ein Bereich, dessen einziger Inhalt seine eigene Überschrift ist, wirkt kaputt.

Internes Vokabular steht nie auf dem Bildschirm: keine Sprintnummern, keine Issue-Kennungen, nicht das Wort „Platzhalter". Ein Bedienelement, das noch nichts tut, sagt, was es tun wird, oder fehlt. Die Zielgruppe sind Gestalter und Berater; „Ab Sprint 2 verfügbar" sagt ihnen, dass das Produkt unfertig ist, nicht, was es kann.

### 4.3 Warum diese Anfrage wartet

Der Satz wird aus dem Grund des Anhaltens des Flows gebaut, nicht aus einer Konstante:

| Grund | en | de |
|---|---|---|
| keine Regel passte | "Held: no rule matches · default: ask" | "Angehalten: keine Regel · Vorgabe: fragen" (HUM-052: kürzer, damit der Satz in der Aktionsleiste nicht abgeschnitten wird) |
| eine Regel sagt nachfragen | "Held: rule `<rule sentence>` says ask" | "Angehalten: Regel `<Regelsatz>` sagt fragen" |
| ein Fund | "Held: an AWS access key was found in the body" | "Angehalten: im Body steht ein AWS-Zugangsschlüssel" |
| unbekannt | nennt, was bekannt ist, und erfindet keinen Grund | ebenso |

Der `kind`-Bezeichner des Funds steht nie im Satz. Er ist internes Vokabular (4.2) und gehört in die Detailzeile und in den Semantics-Wert; der Satz nennt den übersetzten Namen und den Ort des Funds.

Heute steht dort für jeden Flow derselbe Text, zentriert zwischen Blockieren und Senden, an der auffälligsten Stelle des Bildschirms, und er wird zur Lüge, sobald HUM-022 echte `ask`-Regeln liefert. In diese Mitte gehört, was sich ändert: der Grund und die verbleibende Zeit mit benannter Folge, en "auto-blocks in 1:47", de "wird in 1:47 blockiert", nie ein nacktes `mm:ss`, dessen Richtung und Ausgang man raten muss, und nie ein Partizip („blockiert in 1:47"), das offenlässt, ob es schon geschehen ist.

### 4.4 Diagnostics

Jeder Fehlerpfad liefert ein `Diagnostic`, nie einen nackten String.

- Der `why`-Platz trägt den Satz des Daemons. Der allgemeine Text der App ist der Titel, nicht der Grund. Heute steht der echte Grund in der Mono-Detailzeile und eine Konstante in `why`; das bricht beide Hälften von Prinzip 7 auf einmal.
- `fix` wird immer weitergegeben, wenn das `Diagnostic` eine `FixAction` trägt. Ein `Diagnostic` mit einer `FixAction` und ohne sichtbare Aktion ist ein Defekt.
- Es ist am Ort des Fehlschlags verankert: inline unter dem Bedienelement bei einer einzelnen Entscheidung, als Banner über der Liste mit `Reload` bei einem Datenfehler, an der Stelle des Skeletts bei einem fehlgeschlagenen wartenden Vorgang (2.11), bildschirmfüllend nur, wenn gar nichts angezeigt werden kann. Abstand zwischen Fehler und Ursache ist der Grund, warum Menschen die fehlgeschlagene Aktion wiederholen, statt sie zu beheben.
- Ein Dokumentationslink ist Akzent, wenn er anklickbar ist. Eine URL, die niemand öffnen kann, ist `fg1` mono mit einem Kopierknopf daneben.

### 4.5 Was „Rückgängig" bedeutet

„Rückgängig" macht immer die Regel rückgängig, nie die Anfrage. Der Streifen lautet „Regel gespeichert · Rückgängig" für `HMotion.undoWindow` (10 s) und sagt im selben Atemzug, dass die Anfrage schon draußen ist. Danach verschwindet nur der Streifen; die Regel bleibt im Bildschirm Regeln löschbar. Für Senden und Blockieren selbst gibt es kein Rückgängig; daher der Halteschutz und das Scharfschalten aus 5.4. Ein Rückgängig, dessen Reichweite jemand falsch einschätzt, ist schlimmer als keins, weil er sich im nächsten Schwall darauf verlassen wird.

### 4.6 Bewegung und Text sagen dasselbe

Der Streifen, der in der entschiedenen Zeile steht, nennt die Folge, nicht die Aktion: en "Sent to api.github.com · 2.1 KB", de "Gesendet an api.github.com · 2,1 KB". Blockiert lautet en "Blocked. The agent may retry.", de "Blockiert. Der Agent kann es erneut versuchen." Kein Toast begleitet eine Bewegung; Toast plus Bewegung sind zwei Ankündigungen desselben Ereignisses.

Nach dem Bestätigungsfenster ist der Streifen weg, aber die Aktion war unumkehrbar. Die Statusleiste trägt die letzte Entscheidung deshalb dauerhaft als eine Zeile, Zustandsglyphe, Host, Uhrzeit, bis die nächste sie ersetzt. Drei Sekunden sind zu kurz für die einzige Spur einer Aktion, die 4.5 nicht zurücknehmen kann.

**Die Verben.** Drei Orte, drei Formulierungen, und sie werden nicht vermischt:

| Ort | en | de | warum |
|---|---|---|---|
| Bedienelement | "Allow" | "Senden" | Das Bedienelement benennt die Aktion am Objekt. Der deutsche Nutzer sendet eine Anfrage, er erlaubt sie nicht. |
| Regel | "allow" | "Erlauben" | Die Regel benennt die Richtlinie, nicht die einzelne Aktion; sie gilt für alles Künftige. |
| Streifen | "Sent to {host} · {size}" | "Gesendet an {host} · {size}" | Der Streifen benennt die Folge, im Perfekt, mit Ziel und Umfang. |

Damit ist entschieden, dass der deutsche Knopf nicht „Erlauben" sagt, obwohl HUM-028 den Regelsatz mit „Erlauben · ∗ · …" beginnt (Abweichung in Abschnitt 8). `BACKLOG.md` Abschnitt 5 legt diese Teilung schon fest; hier steht, warum sie kein Fehler ist.

Vor „Merken" steht die Regel als Satz, damit sie geprüft werden kann, die Vorgabedauer ist `session`, und ein Bedienelement mit der Beschriftung „Alle senden" gibt es nur als Palettenbefehl, der eine Host-Liste öffnet.

Solange das Raster merkt, stehen deshalb **beide** Sätze unter den Bedienelementen, einer unter dem anderen: der Satz der Regel, die Senden anlegt, und darunter der, den Blockieren anlegt (HUM-249). Welcher Satz dort steht, hängt weder vom Zeiger noch vom Fokus ab. Ein Tastenkürzel fragt keinen von beiden: Mit der Maus über „Blockieren" legt `A` eine Regel zum Erlauben an; mit dem Fokus auf dem Ventil legt `Ctrl+L` eine Regel zum Blockieren an; und stünde nur der Satz der hervorgehobenen Aktion da, wäre die gespeicherte Regel eine, die niemand gelesen hat. Ein Kürzel, das erst den Satz zeigt und einen zweiten Druck verlangt, wurde verworfen: Es bricht den Rhythmus der Warteschlange und bräuchte einen Zustand, den Zeiger, Fokus und Tasten gemeinsam pflegen. Ein neutraler Satz ohne Aktion ebenso: Die Regel benennt die Richtlinie (Tabelle oben), und eine Regel ohne Verb ist keine, die sich prüfen lässt. Die Sätze brechen um und werden nie mit Auslassungspunkten gekürzt, auch nicht bei 480 px Bereichsbreite und doppelter Schriftgröße: Ein gekürzter Satz ist einer, dessen Ende niemand gelesen hat, und die Dauer steht am Ende. Dasselbe gilt für den Regelsatz im Modal (5.4). Die Zeile wird dadurch höher, aber nur zusammen mit dem Raster, das ohnehin eine eigene Zeile nimmt; der Zeiger, der zwischen den Knöpfen wandert, ändert keinen Text mehr (2.8). Bei geschlossenem Raster legt das Halten des Ventils eine Regel für die Session an; ihr Satz steht ab dem Drücken, solange der Finger unten ist, also die 400 ms vor dem Speichern, an der Stelle des Grunds des Anhaltens. Ändert sich während des Haltens, woraus die Regel gebaut wird, Dauer, Ziel, Anfrage, Domain, speichert das Halten nichts und sagt, warum; es speichert nie eine andere Regel als die, deren Satz es gezeigt hat. Die Sätze nehmen höchstens ein Drittel des Bereichs; was darüber hinausgeht, scrollt, und eine Entscheidung, die eine dieser Regeln speichern würde, wird abgewiesen, bis das Ende der Sätze im Blick war. Dasselbe im Modal: Es bleibt im Fenster, sein Inhalt scrollt, und Bestätigen geht erst, wenn der Inhalt bis zum Ende im Blick war. Gespeichert wird genau die Regel, deren Satz dort stand (`ShownRule`, HUM-058).

### 4.7 Wenn ein Secret mitgeht

Eine Anfrage, die wegen eines Funds angehalten ist, ist der eine Fall, der nicht ruhig sein darf. Sie darf nicht wie eine Routineanfrage aussehen und bekommt deshalb vier Abweichungen vom Normalfall, und nur diese vier:

- **In der Zeile** steht der Fund-Chip in `HColors.secret` statt in `fg2`. Das ist die einzige Farbigkeit, die eine ruhende Zeile der Warteschlange tragen darf; der Rest der Zeile bleibt neutral.
- **In der Aktionsleiste** nennt der Grund des Anhaltens Art und Ort des Funds in Alltagssprache (4.3), und das Freigabeventil wechselt auf `warning`, trägt die Warnglyphe (`triangleAlert`) vor seiner Beschriftung und beschriftet sich um. Die Glyphe ist seit HUM-260 nötig, weil `warning` im Wert dem Akzent gleicht und die Farbe allein ein Ventil mit Fund nicht mehr von einer Routineanfrage unterscheidet: en "Send with 2 findings", de "Senden mit 2 Funden" (wie schon in HUM-049 geplant).
- **Senden verlangt dieselbe Haltebestätigung wie Blockieren**, solange mindestens ein Fund ungeklärt ist, dazu einen Satz, der die Folge nennt: en "An AWS access key goes to api.example.com", de "Ein AWS-Zugangsschlüssel geht an api.example.com". Nur wer weiß, was wohin geht, drückt und hält.
- **Ein Klick, `Enter` oder `A` öffnen bei einer einzelnen Anfrage die Pause mit den offenen Funden** (HUM-049), statt nur abzuweisen. Sie ersetzt den unteren Teil der Karte, wächst in 200 ms auf und listet je Fund die Art, den gekürzten Wert und den Ort; am Fuß stehen „Trotzdem senden" (`S`), „Pseudonymisieren" (`P`, öffnet den Editor mit allen Funden ersetzt) und „Blockieren" (`B`), `Esc` schließt sie. Die Haltebestätigung bleibt der Weg ohne Pause. Für eine Gruppe gibt es keine Pause: Dort fehlt die Liste der Funde, und es bleibt beim Halten oder einer Taste.

Ausdrücklich **kein** Modal. Die Regel „nie ein Modal für eine einzelne Entscheidung" (5.4) gilt hier weiter; ein Modal wäre auch der falsche Schutz, weil man es wegklicken kann, ohne den Satz zu lesen. Der Schutz sitzt in der Zeit, nicht in der Fläche (5.4). Abweichung in Abschnitt 8.

### 4.8 Wenn die Zeit abläuft

Ein Zeitablauf ist keine Entscheidung eines Menschen, sondern das Fehlen einer. Er ist deshalb das Ereignis, das die meiste Spur hinterlassen muss, und heute hinterlässt er die wenigste.

- **Vorher** warnt die App einmal, an der Zeile und in der Aktionsleiste des betroffenen Flows, nie als Toast: Host, Folge und verbleibende Zeit in einem Satz, en "registry.npmjs.org auto-blocks in 0:30", de "registry.npmjs.org wird in 0:30 blockiert". Sobald der Daemon Verlängern kennt, steht genau eine Verlängerung daneben. Bis dahin ist die Warnung mit benannter Folge das Minimum.
- **Im Augenblick des Ablaufs** zieht die Leiste wie bei jeder anderen Entscheidung auf `timedOut` auf, und die Zeile bleibt drei Sekunden als graue Zeile mit dem Streifen en "Blocked (timed out)" / de "Blockiert (Zeit abgelaufen)" stehen. Das steht schon in HUM-029; hier steht, dass es dieselbe Choreografie ist wie bei einer menschlichen Entscheidung, weil die Folge für den Agenten dieselbe ist.
- **Danach** trägt die Statusleiste dauerhaft einen Zähler: en "{n} timed out this session", de "{n} abgelaufen in dieser Session". Ein Klick filtert den Verlauf darauf. Wer fünf Minuten weg war, muss bei der Rückkehr sehen, dass sechs Anfragen abgewiesen wurden, ohne den Verlauf zu öffnen und ohne sich an ein Banner zu erinnern, das er nie gesehen hat.

### 4.9 Tray, Benachrichtigung und Rückkehr

Das ist die einzige Fläche, die der Mensch sieht, während der Agent feststeckt. Sie bekommt deshalb eigene Regeln und keine Zeile am Rand.

**Die Benachrichtigung** hat dieselbe Hierarchie wie ein Bildschirm (3.1): Der Host ist das eine Wichtige und steht groß; Methode und Pfad sind zweitrangig; die verbleibende Zeit steht als Wort, nie als `mm:ss`, en "about 4 minutes left", de "noch etwa 4 Minuten". Eine Benachrichtigung ist ein Standbild im Fenstermanager eines anderen; eine Ziffer darin ist ab der ersten Sekunde falsch. Bei fokussiertem Fenster erscheint keine Benachrichtigung: Was auf dem Bildschirm steht, wird nicht zusätzlich angekündigt (dieselbe Regel wie Toast plus Bewegung in 4.6). Mehrere Ankünfte aktualisieren dieselbe Nachricht, sie stapeln sich nicht.

**Das Tray-Icon** ist eine Zustandsanzeige und trägt Zustandsfarbe (3.3, Regel 9):

| Zustand | Icon | Farbe |
|---|---|---|
| nichts wartet | `idle` | `fg2`, neutral |
| 1 bis 9 warten, 10 und mehr | `held_n`, `held_9plus` | Farbe von `held` mit Ziffer |
| ein Zeitablauf hat seit dem letzten Fokus blockiert | `alert` | `blocked` |

**Das Fenster** trägt keine Titelleiste des Toolkits (HUM-255). Die Kopfleiste der App ist die Titelleiste: Ziehen auf ihrer freien Fläche, auf dem Schriftzug, dem Ansichtstitel oder den Badges bewegt das Fenster, ein Doppelklick maximiert oder stellt wieder her, und ihre Bedienelemente bleiben Bedienelemente. Rechts stehen Minimieren, Maximieren und Schließen als Ghost-Knöpfe mit Hover und Semantik-Beschriftungen, auch auf Splash und Einrichtung eines Kaltstarts (dort in einer schmalen Leiste, die zugleich Griff ist), auch unter Kachel-Fenstermanagern, die die ersten beiden ignorieren dürfen. Schließen tut genau, was Schließen über den Fenstermanager tut; es ist nicht das „Beenden" des Tray-Menüs.

**Die Rückkehr** spielt nichts nach. Wer nach fünfzehn Ankünften zurückkommt, sieht die Warteschlange, wie sie ist, ohne Ankunftsanimationen für Dinge, die vor Minuten geschehen sind; eine Animation beantwortet „was ist gerade angekommen", und nichts davon ist gerade angekommen (2.1). Was während der Abwesenheit geschah, steht in zwei ruhigen Formen: Das Rückkehr-Banner nennt die längste Wartezeit und führt zum ältesten Flow, der Zähler der Zeitabläufe in der Statusleiste nennt die Abweisungen (4.8).

### 4.10 Einrichten als kurzes Onboarding

Einrichten ist ein kurzes Onboarding und keine Liste von Karten (HUM-287). Der Eigentümer hat das Design am 2026-10-24 aus drei gerenderten Varianten gewählt: V1, ein senkrechter Stepper, dessen aktueller Schritt sich an Ort und Stelle öffnet, mit zwei Dingen aus den anderen. Aus V3 kommt die Überschrift des offenen Schritts in Alltagssprache, aus V2 das „Danach: …" neben seiner Primäraktion. Die Bilder sind `v1-*.png`, `v2-*.png` und `v3-*.png` mit ihrer `README.md` in der Übergabe des Issues, und `impl-*.png` zeigt, was gebaut wurde.

**Vier Schritte, in der Reihenfolge der Arbeit.** Hintergrunddienst, Modellserver, Projektordner, Sandbox-Prüfungen. Der Agent, den der Daemon gefunden hat (HUM-278), ist eine Zeile des Modellschritts, weil der Agent und sein Modell das sind, was eine Session laufen lässt. Er bleibt eine der Zeilen des Rechners und zählt dort, sodass ein Modellserver, der geantwortet hat, nie wegen einer anderen Prüfung als ungemessen gemeldet wird.

**Grün ist eine Zeile.** Ein grüner Schritt zeigt Marke, Titel, Zustandswort und seinen Beleg auf einer Zeile in der Zeilenhöhe des Rasters (`HSize.paneHeader`), mit einem ruhigen „Details", das ihn öffnet. Jeder Schritt, der nicht vorn steht, steht ebenfalls als eine Zeile. Hat ein blockierender Schritt eine Abhilfe mit eigenem Knopf, bleibt dieser Knopf auf seiner Zeile (die Ordnerauswahl aus HUM-282), sodass eine zweite Abhilfe weiterhin ein Klick ist.

**Der erste Schritt, der nicht grün ist, ist offen.** Allein die Reihenfolge entscheidet, welcher Schritt das ist; der Bildschirm bewertet keinen Schritt. Der offene Schritt ist ein erhabenes Feld mit einer Überschrift in Alltagssprache („Der Modellserver verlangt einen gültigen Zugangsschlüssel", „Den Ordner wählen, in dem der Agent arbeitet"), den eigenen Bedienelementen des Schritts und seinem Befund innerhalb des Schritts mit Code, Ursache, Detail und Abhilfe (4.4), gezeichnet als Zustandsleiste und nicht als Karte. Seine Aktion ist der einzige Primärknopf auf dem Bildschirm und nimmt den Fokus: die Abhilfe, wo sie einen eigenen Knopf hat („Einstellungen öffnen" über HUM-275, „Ordner wählen", „Dienst installieren"), sonst „Ansprechen", „Erneut verbinden" oder „Erneut prüfen". Daneben steht der Schritt danach („Danach: Projektordner") oder „Danach: Session starten".

**Ein Ordner, einmal gewählt (HUM-293).** Einen Ordner in Einrichten, auf dem Sandbox-Bildschirm oder in den Einstellungen zu wählen, schreibt `sandbox.work_dir` über `SetConfig` in die `config.toml` des Nutzers; der Daemon prüft den Ordner vor dem Schreiben mit denselben Regeln wie bei einem Start und schreibt ihn mit aufgelösten Links, sodass ein Link, der später woandershin zeigt, das Projekt nicht verschiebt. Ein abgewiesener Ordner wird nicht gespeichert, sein `SANDBOX_006` steht im Projektschritt, und der vorher gespeicherte Ordner bleibt. Die App, der Desktop-Starter und `humanitl sandbox run`/`humanitl run` nehmen alle den gespeicherten Ordner; `--work` überschreibt ihn für einen Aufruf, und das aktuelle Verzeichnis zählt nur, solange nichts gespeichert ist. Kein Ordner lebt nur im Speicher des Daemons, sodass App und Kommandozeile nie in zwei verschiedenen Ordnern starten. Eine laufende Session behält ihren Ordner; der neue gilt für den nächsten Start.

**Eine Wahl, ein Knopf.** Solange der Projektschritt nach einem Ordner fragt (kein Ordner oder einer, den der Daemon abweist), ist sein einziges Bedienelement dafür das „Ordner wählen" des Befunds als Primärknopf; die Ordnerauswahl mit ihrem eigenen „Ordner wählen…" erscheint erst, wenn ein Ordner gilt, um ihn oder seinen Modus zu ändern.

**Der eine Kontakt, den Einrichten von sich aus aufnimmt.** Wenn Einrichten sich öffnet, bittet es den Daemon einmal, den in der Konfiguration gespeicherten Modellserver anzusprechen (Entscheidung des Eigentümers 2026-10-24). Das ist dasselbe `ProbeLlm` wie „Ansprechen", mit dem gespeicherten Schlüssel, und der Daemon schickt den Schlüssel nur an den Origin von `llm.endpoint` (HUM-288). Es fragt nur, solange das Feld den gespeicherten Endpunkt zeigt und kein Ergebnis für diesen Endpunkt vorliegt. Es fragt erneut bei „Erneut prüfen" und wenn sich der gespeicherte Endpunkt ändert, nach einer Änderung der Einstellungen oder einem Wechsel des gespeicherten Servers. Es fragt nie nach einem Timer und spricht nie eine Adresse an, die jemand ins Feld getippt hat. Jedes Ergebnis trägt einen Fingerabdruck dessen, was es gemessen hat: die Adresse, den aktiven gespeicherten Server, die Referenz seines Schlüssels, ob ein Schlüssel gespeichert ist und wie oft diese App den Schlüssel geändert hat. Der Modellschritt zählt ein Ergebnis nur, wenn sein Fingerabdruck der dessen ist, was das Feld jetzt zeigt. Ein Ergebnis für eine andere Adresse, einen anderen Server oder einen anderen Schlüssel ist dort „nicht gemessen", ob die Adresse getippt, später gespeichert oder während der Probe entfernt wurde und ob der Schlüssel in den Einstellungen ersetzt oder gelöscht wurde. Eine Änderung irgendeines Teils lässt die Kontaktregel erneut fragen. Der gespeicherte Endpunkt ist der, den die Einstellungen schreiben (`llm.endpoint`), kein Sandbox-Snapshot. Alle Proben („Ansprechen", eine aus der Suche übernommene Adresse und der Kontakt, den Einrichten von sich aus aufnimmt) laufen über einen Controller, der seine Anfragen nummeriert, und nur die neueste Anfrage darf das Ergebnis setzen, sodass eine späte Antwort nie eine neuere ersetzt. Sonst geht nichts hinaus: keine Suche im Netzwerk und keine andere Adresse. Solange kein Endpunkt gespeichert ist, spricht das Öffnen von Einrichten gar nichts an. Das ersetzt das frühere Versprechen aus HUM-075, dass das Öffnen von Einrichten nichts anspricht.

**Bereit.** Mit vier grünen Zeilen sagt die Überschrift „Humanitl ist bereit", und es steht kein Befund auf dem Bildschirm. Wegen des einen Kontakts oben erreicht ein Rechner, auf dem alles eingerichtet ist, diesen Zustand ohne Klick. Die eine Primäraktion ist „Session starten" oder „Zur Warteschlange", solange schon eine Session läuft und der Start gesperrt ist.

**Nichts muss scrollen.** Bei 1280×800 stehen der offene Schritt, seine Aktion und die Zeilen darunter im Zustand des Eigentümers vom 2026-10-23 (Schlüssel abgewiesen, kein Ordner) über der Statusleiste. Bei Textskalierung 2 scrollt der Bildschirm weiterhin.

**Wo die App öffnet.** Die App öffnet Einrichten, wenn die erste feststehende Antwort einen Schritt hat, der den Start blockiert oder noch erfragt wird (`SetupState.needsSetup`), und sonst Anhalten. Ein erster Start hat keinen Projektordner, und ein fehlender Ordner blockiert (`CONFIG_013`). Der erste Start ist deshalb der erste Fall des zweiten, und der Client hält kein eigenes „erster Start"-Flag. Ein Modellserver, den niemand anzusprechen gebeten hat, blockiert nicht, sodass ein gewöhnlicher zweiter Start die Warteschlange öffnet.

**Geprüft durch** `app/test/features/setup/setup_onboarding_test.dart` und die Goldens `setup_firstrun`, `setup_partial` und `setup_ready_*`.

---

## 5. Tastatur und Maus

### 5.1 Parität

- Jede Zeigergeste hat eine Tastenentsprechung. Auch Drücken und Halten: Das Freigabeventil ist über eine Taste oder eine zweistufige Bestätigung erreichbar, sonst ist das Kennzeichen-Element ohne Maus unbenutzbar. Auch die Pill „+n neu" (2.8).
- Jedes Element, das auf Antippen reagiert, ist mit `Tab` erreichbar und mit `Enter` und `Space` auslösbar. Ein nackter `GestureDetector` oder `MouseRegion` ist nie der einzige Eingabeweg.
- Nichts, was eine Aktion ausführt, erscheint nur beim Hover. Zeilenaktionen erscheinen auch bei Fokus und bleiben im Semantics-Baum, wenn sie unsichtbar sind.
- Das Kürzel steht auf dem Bedienelement, das es ausführt: dauerhaft auf den beiden Hauptentscheidungen, beim Hover auf allem anderen, nach dem `HoverLabel`-Muster der Icon-Leiste. Sonst kann jemand mit der Maus `Enter`, `A`, `B`, `J` und `K` nicht durch Klicken entdecken.
- Die Befehlspalette ist das Handbuch: Sie listet jede per Tastatur erreichbare Aktion mit ihrem Kürzel in der Zeile, nicht nur die fünf Navigationseinträge.

### 5.2 Wem die Taste gehört

Kein `Shortcuts`-Widget über fokussierbaren Bedienelementen darf `Enter`, `Space` oder einen einzelnen Buchstaben so binden, dass das fokussierte Bedienelement leer ausgeht. Heute sendet jemand an der Tastatur, der „Blockieren" fokussiert und `Enter` drückt, die Anfrage; mit `Space` blockiert dasselbe Bedienelement korrekt, und die falsche der beiden Tasten ist die, nach der jeder Desktop-Nutzer zuerst greift.

Der Mechanismus, der das behebt, ist genau einer, und er steht hier ausgeschrieben, weil der naheliegende nicht funktioniert. `Action.overridable` hilft nicht: Eine überschreibbare Aktion wird vom nächsten **Vorfahren** überschrieben, nie von einem fokussierten Nachfahren, und die Kollision entsteht ohnehin früher; `Shortcuts` bildet `Enter` auf `AllowIntent` ab, bevor irgendein Nachschlagen einer Aktion läuft.

Stattdessen: Die Aktionen `AllowIntent` und `BlockIntent` des Bildschirms überschreiben `isActionEnabled` und geben `false` zurück, solange `FocusManager.instance.primaryFocus` auf einem Bedienelement sitzt, das `ActivateIntent` behandelt. `ShortcutManager.handleKeypress` gibt für eine deaktivierte Aktion `KeyEventResult.ignored` zurück; die Taste fällt damit zu den Standardbindungen von `WidgetsApp` durch, wo `Enter` `ActivateIntent` auslöst, und das fokussierte Bedienelement gewinnt. Die billigere Variante, falls sich das im Test als brüchig erweist: `Enter` auf Bildschirmebene gar nicht binden, nur `A` und `B`, und `Enter` überall „aktivieren" bedeuten lassen.

Die Warteschlange ist ein einziger Fokus-Halt mit Pfeiltasten-Navigation darin. Solange der Fokus in der Warteschlange ist, gilt `Enter` = senden; liegt er auf einem Bedienelement, gilt `Enter` für dieses Bedienelement. Das widerspricht nicht der Regel aus 3.4, dass aus der Zeile nur blockiert wird: Die Zeile ist nicht das Bedienelement, die **Auswahl** ist der Gegenstand. `Enter` sendet den ausgewählten Flow, dessen URL die Karte zeigt, und die Zeile selbst trägt keine Senden-Möglichkeit, die man mit dem Zeiger treffen könnte.

**Im Terminal stellt sich dieselbe Frage, und die Antwort ist umgekehrt.** Dort
gehört jede Taste dem Agenten, Rücktaste, Pfeile, Enter, `Ctrl+C` als `0x03`,
`Ctrl+P` als `0x10`, mit genau sieben Ausnahmen: `Ctrl+1` bis `Ctrl+6` und
`Ctrl+K`. Ohne sie käme man aus einer bildschirmfüllenden TUI nur mit der
Maus heraus, und das verletzte 5.1. (Spätere Issues haben die Zoom-, Kopier- und
Einfüge-Kombinationen unten ergänzt; und solange sichtbarer Text ausgewählt ist,
kopiert ein einfaches `Ctrl+C`, statt `0x03` zu senden, HUM-353.) Der Weg dorthin ist nicht der
naheliegende: Der Emulator übersetzt `Ctrl+K` in `0x0b` und `Ctrl+6` in `0x1e`
und meldet die Taste als behandelt, sodass das Ereignis kein `Shortcuts`
darüber mehr erreicht; `KeyEventResult.skipRemainingHandlers` hielte den
Emulator an, beendete die Reise aber genauso. `TerminalPane._onKey` schlägt die Taste
deshalb selbst in `shellShortcuts()` nach und löst den Intent über
`Actions` aus. Gemessen in `a_shortcut_of_the_application_survives_the_terminal`
für alle drei Fälle (`Ctrl+1`, `Ctrl+6`, `Ctrl+K`).

`Ctrl+Shift+Enter` (HUM-298, auch mit dem `Enter` des Ziffernblocks) ist
eine weitere Ausnahme und nimmt denselben Weg: Sie zeigt das Terminal über das
ganze Fenster, ohne Leiste, Ansichten und Statusleiste, und bringt die
Ansicht und das Layout zurück, die vorher da waren. Der Knopf am rechten Ende des
Streifens über dem Terminal tut dasselbe und nennt die Kombination in seinem Tooltip.
`Esc` ist keine Ausnahme; es bleibt beim Agenten, weil OpenCode und pi
damit abbrechen. Gemessen in
`the_shortcut_works_with_the_terminal_focused_and_esc_reaches_the_agent`.
Die Kombination ist auch in der Warteschlange frei: `Enter` sendet, `Ctrl+Enter` blockiert
und `Shift+Enter` öffnet das Merken-Raster, und jede davon verlangt ihre
Modifikatoren exakt, sodass `Ctrl+Shift+Enter` nichts entscheidet
(`the_chord_decides_nothing_in_the_focused_queue`). Nicht `Ctrl+Shift+F`: Das
ist das Gruppen-Senden der Warteschlange.

**Das Terminal nimmt die Tastatur beim Hover und per Kürzel (HUM-303).** Der
Zeiger über dem Emulator gibt ihm sofort die Tastatur, ohne Klick, in
seinem Bereich, über das ganze Fenster und schwebend (HUM-300). Zwei Ausnahmen
halten den Fokus, wo er ist. Eine Ebene über der Arbeit, ein Dialog, ein Sheet, ein
Menü, die Befehlspalette, hält die Tastatur in einem eigenen Fokus-Bereich,
und das Terminal nimmt den Fokus nur von einem Knoten im eigenen Bereich oder einem der
Bereiche um ihn herum; das deckt jede solche Ebene ohne Liste ab, und das
eigene Kontextmenü des Terminals, das sich unter dem Zeiger öffnet und ihn
durchlässt, ist der Fall, der das braucht. Jede solche Ebene nimmt die Tastatur,
wenn sie sich öffnet, auch vom Terminal, und gibt sie zurück, wenn sie sich schließt
(`HLayerScope`); `autofocus` allein würde dem weichen, was den Fokus schon
hält, und `Esc` im Menü des Terminals erreichte den Agenten. Eine
über einem Sheet geöffnete Palette gibt die Tastatur an das Sheet zurück und
bietet das Terminal nicht an. Und ein Textfeld, das den Fokus hat, behält
ihn, während der Zeiger das Terminal nur überquert; wer dann das Terminal will,
klickt es an oder drückt das Kürzel. Es gibt bewusst keine Verzögerung von
etwa 150 ms: Sie ließe einen Moment, in dem der Zeiger auf dem Terminal ist
und ein für den Agenten gemeintes `Enter` noch die ausgewählte Anfrage in der
Warteschlange sendet. Fokus sofort, und die Entscheidungstasten feuern nie, weil sich der Fokus durch
Hover bewegt hat (`enter_goes_to_the_terminal_only_once_the_focus_moved`).

``Ctrl+` `` gibt die Tastatur dem angezeigten Terminal und ändert
nichts am Layout. Ohne Terminal auf dem Bildschirm und mit laufendem Agenten
lässt es das Terminal schweben (HUM-300) und gibt dem schwebenden Terminal die
Tastatur; ohne Agenten tut es nichts. Es fragt nach der physischen Taste
links von `1`, weil der Backtick auf vielen Belegungen eine Tottaste ist oder hinter
`AltGr` liegt; auf einer deutschen Tastatur ist die Kombination `Strg+^`. Es ist eine weitere
Ausnahme im Terminal und nimmt den Weg der anderen über
`TerminalPane._onKey`, sodass der Agent es nie erhält. Eine Ebene
über der Arbeit hält den Fokus auch gegen das Kürzel; ein Textfeld tut das
nicht, weil das Kürzel bewusst fragt. Die Palette listet es, solange ein
Terminal angezeigt wird.

**Das Terminal schwebt als Bild im Bild (HUM-300).** Während ein
Agent läuft, lässt `Ctrl+Shift+P` oder der Knopf links vom Ganzfenster-Knopf im
Streifen über dem Terminal das Terminal als kleines Fenster über
der jeweils angezeigten Ansicht schweben, sodass der Agent im Blick bleibt, während seine Anfragen
entschieden werden. Es ist dieselbe Session und derselbe Emulator mit seinem
Scrollback; genau ein Ort zeigt es. **Auf der Sandbox-Seite dockt es an**
(HUM-315): Solange diese Seite angezeigt wird, steht ein schwebendes Terminal dort in seinem Bereich,
und das Verlassen der Seite lässt es wieder schweben. Das Andocken für die Seite ist ein
Zustand der Ansicht, keine Wahl: Der Umschalter und die Palette sagen weiterhin, dass es
schwebt. Ein Terminal, das im Fenster die Tastatur hatte, behält sie im Bereich;
das Verlassen der Seite überlässt die Tastatur der angeforderten Ansicht. Ein Agent, der
endet, während die Seite angezeigt wird, dockt das Terminal endgültig an. Das Fenster
hat eine Titelleiste zum Ziehen, einen Knopf zurück zur Sandbox-Ansicht und einen
Schließen-Knopf (dieselbe Kombination); es rastet an einem von neun Ankern ein (den
Ecken, den Mitten der Kanten, der Mitte, HUM-352): ruhig losgelassen am
nächsten; geworfen am nächsten Anker in Wurfrichtung, sodass ein
kurzes Schnippen aus einer Ecke ihren Nachbarn erreicht, statt zurückzufallen. Ein
Ziehen, das nach einer Bewegung abgebrochen wird, zählt als ruhiges Ablegen und behält den
nächsten Anker; ein Gleiten, das eine freizuhaltende Region kreuzen würde, springt. Ein
Griff in der Ecke gegenüber dem Anker ändert die Größe zwischen 320 x 200 und
60 % der Ansichten (höchstens 960 x 640), an jedem Anker gleich, und Anker
und Größe überstehen einen Neustart in der Zustandsdatei der Oberfläche. Die Vorgabe-Ecke ist unten rechts: In
Anhalten ist das der Fuß des Kontextbereichs, die eine Ecke ohne
Entscheidung. **Es verdeckt nie, was klickbar bleiben muss.** Die Warteschlange mit ihren
Zeilen und Hinweisen, die Leiste mit Senden und Blockieren, die Hinweise der Shell, ein
Sheet und ein Modal markieren sich als freizuhaltende Regionen (`HKeepClear`);
die Leiste nur, solange eine Anfrage wartet, und die Hinweise innerhalb des Fensters nie
(HUM-352);
das Fenster nimmt die gewählte Ecke in der gewählten Größe, sonst die kleinste
Größe, sonst die Ecke daneben, darüber oder gegenüber. **Ohne freie Ecke
schrumpft es zu einem schmalen Titelstreifen** (HUM-315, `HSize.terminalStrip`, 240 x 28):
das Profil der Sandbox oder „Terminal", der aktive Tab und ein Knopf; er
nimmt die erste Ecke in derselben Reihenfolge, die ihm Platz lässt, und verdeckt ebenfalls nie,
was klickbar bleiben muss. Der Streifen oder sein Knopf öffnet das
Fenster in der nächsten freien Ecke oder, wenn keine frei ist, legt das Terminal zurück
in seinen Bereich und zeigt die Sandbox-Seite. Nur ohne Platz selbst für den
Streifen (ein Modal über der Arbeit) tritt es zurück. Es hängt nur über den Ansichten,
nie über der Leiste, dem Kopf oder der Statusleiste, und das Einstellungs-Sheet
und die Befehlspalette liegen darüber. **Die Tastatur bleibt, wo sie ist.**
Das Öffnen des Fensters aus der Warteschlange lässt die Tastatur in der Warteschlange, und
`Enter` sendet weiterhin; das Terminal im Fenster nimmt die Tastatur nach
der Regel oben, Hover, ``Ctrl+` ``, ein Klick, und dann funktioniert jedes Kürzel
der Shell von drinnen weiter, `Ctrl+Shift+P` eingeschlossen. Ein Terminal,
das in seinem Bereich die Tastatur hatte, nimmt sie mit. **Das Ende.** Wenn der Agent
endet, zeigt das Fenster das Ende vier Sekunden lang so, wie es der Bereich täte, und
legt das Terminal dann von selbst zurück in seinen Bereich. Ohne laufenden Agenten tut die
Kombination nichts, und der Knopf ist deaktiviert. „Zum Terminal" der leeren
Warteschlange (HUM-306) tut seit HUM-315, was ``Ctrl+` `` tut: Es lässt das
Terminal über der Warteschlange schweben und gibt ihm die Tastatur, oder auf der Sandbox-Seite
gibt es die Tastatur dem Terminal in seinem Bereich. Die Palette listet den Umschalter,
solange ein Agent läuft oder das Terminal schwebt. Das ganze Fenster
(`Ctrl+Shift+Enter`) kehrt zum schwebenden Fenster zurück, wenn es von
dort geöffnet wurde. `Ctrl+Shift+P` erreicht keinen Agenten: Ein Linux-Terminal sendet es als `0x10`
wie `Ctrl+P`, das beim Agenten bleibt.

**Der Terminaltext lässt sich zoomen (HUM-301).** `Ctrl` mit `+` macht ihn eine
Stufe größer, mit `-` eine Stufe kleiner, zwischen 10 und 28 px; die Vorgabe sind
die 13 px von `mono13`. `+` zählt mit oder ohne `Shift`, weil es auf einer US-
Tastatur `Shift+=` ist, und ebenso `Ctrl+=` und das Plus des Ziffernblocks.
`-` zählt nur ohne `Shift`: `Ctrl+Shift+-` ist `Ctrl+_`, Rückgängig in
readline, und bleibt beim Agenten. Das Zurücksetzen ist `Ctrl+Shift+0` (auch `Ctrl`
mit der `0` des Ziffernblocks), **nicht `Ctrl+0`**: Diese Kombination, das Zurücksetzen jedes
Browsers und Terminals, ist hier die Leiste (HUM-254), in jeder Ansicht und von
innerhalb des Terminals; eine Kombination mit zwei Bedeutungen, je nachdem, wo der
Fokus sitzt, kostete den, der die Leiste will, einen kleineren Text und umgekehrt.
`Ctrl` + Mausrad über dem Emulator zoomt ebenfalls, und der Scrollback
bewegt sich dabei nicht. Die Zoom-Kombinationen sind weitere Ausnahmen im
Terminal und nehmen den Weg der anderen über `TerminalPane._onKey`, sodass
der Agent sie nie erhält; außerhalb des Terminals wirken sie nur, solange ein
Terminal mit einer Session angezeigt wird, sonst geht die Taste unverändert weiter. Die
Größe übersteht einen Neustart in der Zustandsdatei der Oberfläche, und nach einem Zoom
zählt der Emulator seine Spalten und Zeilen neu und schickt sie auf dem
gedrosselten Resize-Weg hinauf (HUM-305), sodass der Agent neu zeichnet. Die Palette listet
die drei Befehle, solange ein Terminal angezeigt wird.

**Kopieren und Einfügen im Terminal (HUM-313)** funktionieren wie in jedem Linux-
Terminal. `Ctrl+Shift+C` kopiert die Auswahl, `Ctrl+Shift+V` fügt ein;
ein einfaches `Ctrl+C` bleibt der Interrupt des Agenten und ein einfaches `Ctrl+V` das
Quoted Insert von readline, beide erreichen den Agenten. Eine Ausnahme seit
HUM-353: Solange sichtbarer Text ausgewählt ist, kopiert ein einfaches `Ctrl+C` ihn wie
`Ctrl+Shift+C`, hebt die Auswahl auf und sendet nichts; ohne Auswahl (eine
aus dem Blick gescrollte Auswahl wird verworfen) ist es wieder `0x03`. Auf Apple-
Plattformen kopiert `Cmd+C`, und
`Ctrl+C` geht immer an den Agenten. Die beiden Kombinationen wirken nur,
solange das Terminal die Tastatur hat, und nehmen den Weg der anderen
Ausnahmen über `TerminalPane._onKey`; keine Bindung der Anwendung
benutzt sie anderswo. Die mittlere Taste fügt ein, solange das Programm
die Maus nicht selbst nimmt. Der Rechtsklick öffnet das Menü mit „Kopieren"
(ohne Auswahl deaktiviert), „Einfügen" und „Alles auswählen", das für
`Ctrl+A` einspringt, den Zeilenanfang für den Agenten. Ein Einfügen wird geklammert, solange
das Programm darum gebeten hat (`DECSET 2004`), sodass mehrere Zeilen als eine
Eingabe ankommen, und jede Escape-Sequenz im eingefügten Text wird verworfen, sodass ein
`ESC [201~` die Klammer nicht vorzeitig beenden kann. Auswählen allein kopiert nichts:
Flutter bietet unter Linux keine Primärauswahl, und die Zwischenablage wird
nur durch eine ausdrückliche Aktion gefüllt.

**Weitere Shells öffnen sich als Tabs des Terminals (HUM-314).** Solange eine Session
läuft und der Daemon `sandbox.exec` in seinen Fähigkeiten nennt, steht eine Reihe von Tabs
unter dem Banner: „Agent" zuerst, immer da, dann ein Tab pro
weiterer Shell, dann „+ Shell". Ohne die Fähigkeit fehlt die Reihe, und
das Terminal sieht aus wie vorher. „+ Shell" oder `Ctrl+Shift+T` bittet den Daemon
um eine interaktive Shell neben dem Agenten (`Exec`), mit der Größe der
Ansicht, und zeigt sie in einem neuen Tab; `Ctrl+PageUp` und `Ctrl+PageDown` gehen die
Tabs reihum durch, über das Ende hinweg, und ein Wechsel gibt die Tastatur dem Tab, der
erscheint. Jeder Tab ist dasselbe Terminal mit allem oben, Zoom, Kopieren
und Einfügen, die Tastatur unter dem Zeiger und die verweigerte Zwischenablage des
Agenten eingeschlossen, und das schwebende Fenster und das ganze Fenster zeigen den Tab,
der aktiv ist. Der Daemon erlaubt vier weitere Shells; der Knopf stoppt bei
vier Tabs, und wenn der Daemon abweist (`TERM_002`, auch für eine Shell, die dieses
Fenster nicht zeigt), schwebt der Befund als Hinweis über dem Terminal.
Eine Shell, die sich beendet, behält ihren Tab mit dem Exit-Code, und ein Klick schließt ihn.
Die Schließen-Glyphe beendet nur die Ansicht: Der Daemon hat keinen Aufruf, der eine
Shell beendet, also endet eine laufende mit ihrem Prozess oder mit der Session. Wenn
die Session endet, geht jeder Shell-Tab mit. Die erste Shell bringt einen
einmaligen Hinweis mit: Dort keine Secrets eintippen, der Agent kann diese Shell lesen
(`docs/SECURITY.md` 4). Die Tab-Kombinationen wirken nur, solange sie können: Ohne
die Fähigkeit tut `Ctrl+Shift+T` nichts, und mit dem Tab des Agenten allein
bleiben `Ctrl+PageUp` und `Ctrl+PageDown` beim Agenten.

### 5.3 Jede gebundene Taste tut etwas

Jeder Aktivator in `interceptShortcuts()` hat eine Aktion im selben Bildschirm. Ein Widget-Test vergleicht die beiden Tastenmengen und schlägt fehl, wenn sie sich unterscheiden; eine Bindung ohne Aktion wird gelöscht, nicht deaktiviert. Heute sind `/`, `Ctrl+D` und `N` gebunden und stumm.

Jeder angenommene Tastendruck erzeugt innerhalb von 100 ms einen sichtbaren Frame; die Tastenentscheidung zeigt auf dem passenden Bedienelement der Aktionsleiste dieselbe 120-ms-Füllung wie ein Mausklick. Ein abgewiesener Druck bekommt eine eigene, ruhige Abweisung: Das Bedienelement, das gehandelt hätte, zeigt seine Füllung und leert sich wieder, und der Grund steht daneben. Nie Stille, nie ein Schütteln, nie ein rotes Aufblitzen. Für einen blinden Nutzer wäre eine rein sichtbare Abweisung eine stumme; die Abweisung wird deshalb höflich angesagt (Abschnitt 6). Heute gibt ein zweites `A` während einer laufenden Entscheidung gar nichts zurück, während gleichzeitig die Ghost-Knöpfe verschwinden, und ein schnelles `A A` wirkt, als sei das Programm eingefroren.

### 5.4 Die Sorgfalt wächst mit Reichweite und Umkehrbarkeit

Gefahr ist kein Maß: Niemand weiß im Voraus, ob ein `POST` an einen unbekannten Host schlimm ist. Umkehrbarkeit ist eines. Deshalb hat der Schutz zwei Achsen, und deshalb ist Blockieren Ghost, obwohl es die scheinbar härtere Aktion ist; der Agent darf es erneut versuchen. Senden ist gefüllt, weil es die Aktion ist, auf die es ankommt; sein Schutz sitzt in der Zeit, nicht in der Fläche.

**Reichweite:**

| Reichweite | Schutz |
|---|---|
| eine Anfrage | 250 ms Halten (`HMotion.holdToBlock`) |
| zwei bis fünf Anfragen | 250 ms Halten |
| mehr als fünf oder eine Regel mit `forever` | Modal |

Nie ein Modal für eine einzelne Entscheidung: Ein Modal pro Klick zerstört den Rhythmus der Warteschlange. Ein Modal hält den Fokus in sich gefangen, fokussiert die nicht zerstörerische Aktion, schließt bei `Escape` und bringt die Entscheidungskürzel des Bildschirms zum Schweigen, solange es offen ist.

**Umkehrbarkeit.** Senden ist unumkehrbar (4.5), `Enter` ist eine einzelne Taste, und die Auswahl springt nach jeder Entscheidung zum nächsten angehaltenen Flow. Drei Vorkehrungen halten das auseinander:

1. **Scharfschalten.** `Enter` und `A` feuern erst, wenn die URL des ausgewählten Flows mindestens `HMotion.rearm` (350 ms) in der Karte gestanden hat. Jede neue Auswahl schaltet neu scharf, auch die, die das Programm nach einer Entscheidung selbst setzt. Blockieren bleibt sofort verfügbar. Eine Sakkade zu einer neuen Zeile plus eine Fixation dauert etwa 300 ms; wer schneller entscheidet, hat nicht gelesen.
2. **Tastenwiederholung entscheidet nie.** Ein `KeyDownEvent` mit `repeat == true` wird verworfen. Nach einer angenommenen Entscheidung bleiben `Enter`, `A` und `B` gesperrt, bis das passende `KeyUpEvent` angekommen ist.
3. **Die Auswahl wartet.** Sie rückt nach einer Entscheidung nicht weiter, solange noch eine Entscheidungstaste gedrückt ist. Sonst schaltet Regel 1 gegen einen Finger scharf, der gar nicht losgelassen hat.

**Wenn die Aufzeichnung nicht schreibt** (HUM-250), lässt sich ein Senden zusätzlich später nicht mehr nachschlagen. Der Daemon meldet die Störung (`RECORDER_001`, `RECORDER_003`) und ihr Ende (`RECORDER_005`); solange sie andauert, zeigt die Statusleiste "Recording disrupted" (de "Aufzeichnung gestört") in der Fehlerfarbe, jede angehaltene Karte und jede seither angekommene trägt „Wird nicht aufgezeichnet", und die erste Freigabe der Session hält an einer Pause an der Stelle der Entscheidungsleiste an, wie die Pause mit offenen Funden: kein Modal. „Unaufgezeichnet senden" (`S`) nimmt erst nach `HMotion.rearm` an und gilt dann für den Rest der Session; `Esc` schließt die Pause, ohne zu senden.

Pflichttest: `Enter` 500 ms gedrückt halten erzeugt genau ein `Decide`, nicht mehrere. Ohne diese drei Vorkehrungen sendet ein gedrückt gehaltenes `Enter` die halbe Warteschlange ungelesen; heute ist der einzige Schutz `isSending`, und der endet mit der Antwort des Daemons nach ein paar Millisekunden.

Die beiden Entscheidungen sind außerdem größer als alles andere: `HSize.hitMin` (28 px) ist die Untergrenze für Nebensächliches, Senden und Blockieren messen mindestens 32 px in der Höhe und 120 px in der Breite, wie HUM-028 es für das Freigabeventil ohnehin nennt. Ein 28-px-Ziel neben einem anderen 28-px-Ziel ist genau die Geometrie, in der ein hastiger Klick danebengeht, und was hier daneben liegt, ist die unumkehrbare Aktion.

Das Notizfeld beim Blockieren (HUM-072) ist vorübergehend: verborgen bis `N`, geschlossen bei `Escape` oder bei einer Entscheidung, nie eine dauerhafte Zeile der Aktionsleiste. Ein dauerhaft sichtbares optionales Textfeld fügt der Leiste einen Fokus-Halt, einen Rand und einen Zeichenzähler hinzu und lädt zum Tippen ein, wo der normale Weg ein Tastendruck ist.

---

## 6. Barrierefreiheit in Zahlen

> **Stand 2026-09-04: zurückgestellt bis 1.0.0.** Der Projekteigentümer hat
> entschieden, dass das MVP zuerst beweisen soll, dass das Ding hält und
> gut sein kann; Barrierefreiheit als Programm kommt danach. Was in diesem
> Abschnitt steht, bleibt das Ziel und bleibt lesenswert, blockiert aber kein
> Issue mehr: die Ansagen für Screenreader, der zweite Kanal in seiner vollen
> Abstufung, die Prüfung bei `TextScaler.linear(2.0)` und die 4,5:1 als
> Testschranke ruhen. Wo eine dieser Zahlen heute schon erreicht ist,
> bleibt sie; zurückgestellt heißt „nicht darauf warten", nicht „wieder herausreißen,
> was erreicht ist".
>
> **Vier Dinge gelten weiter**, weil sie nur nach Barrierefreiheit aussehen und
> in Wahrheit Sicherheit oder schlichte Benutzbarkeit sind:
>
> 1. Eine Haltedauer behält ihre volle Zeit, auch wenn die Plattform
>    `disableAnimations` meldet. Flutter kürzt sie sonst auf fünf Prozent, und
>    dann sendet ein gewöhnlicher Klick, wo ein Halten verlangt war; gemessen
>    reichten 60 Millisekunden.
> 2. Die Zielflächen der Entscheidung bleiben: mindestens 28 × 28 px
>    allgemein, mindestens 32 × 120 px für Senden und Blockieren. Wer bei einer
>    unumkehrbaren Aktion abrutscht, hat kein
>    Barrierefreiheitsproblem, sondern eine gesendete Anfrage.
> 3. Text bleibt über etwa 3:1. Das ist kein Standard mehr, sondern
>    Lesbarkeit: `shadcn_flutter` malt seinen destruktiven Knopf mit 1,97:1,
>    und das ist für alle unlesbar, nicht nur für manche.
> 4. Ein Klick zeigt eine sichtbare Reaktion. Ein Bedienelement, das nichts tut,
>    fühlt sich kaputt an.
>
> Siehe ADR-0009, Abschnitt „Am 2026-09-04 vom
> Projektinhaber überarbeitet".


- **Kontrast.** Text erreicht 4,5:1 gegen die Fläche, auf der er wirklich steht, also auch gegen eine 10-%-Tönung und gegen eine Füllung, auf allen vier Flächen beider Leitern. Flächen und Leisten erreichen 3:1, mit der einen benannten Ausnahme der ruhenden `held`-Leiste (3.3, Regel 10). Der Test in `app/packages/ui/test/tokens_test.dart` prüft 3:1 für Zustandsfarben als Fläche und seit HUM-355 4,5:1 für ihren Text auf einer Tönung und auf einer Füllung über alle vier Flächen beider Leitern. Gemessene Verstöße heute: DELETE-Beschriftung auf ihrer Tönung über `bg3` 2,65:1, `held`-Beschriftung hell auf ihrer Tönung 2,75:1.
- **Sekundärtext.** `fg2` misst 3,06:1 auf `bg3`, `lFg2` 3,25:1 auf `lBg3` (HUM-355; vorher 3,02:1 und 3,24:1). Beide sind wirklich deaktivierten Bedienelementen vorbehalten. Jeder Satz, den jemand lesen soll, ist `fg1` oder besser; heute betrifft das den leeren Zustand der Warteschlange und die Titel der aufklappbaren Abschnitte.
- **Fokus.** 2 px Akzentring außerhalb des eigenen Rands. Nie durch Umfärben eines vorhandenen Rands, nie in der Farbe, mit der das Bedienelement gefüllt ist. Auf dem Primärknopf misst der Fokusrand heute 1,00:1 gegen die Füllung; jemand an der Tastatur sieht dort keinen Unterschied.
- **Zielflächen.** Mindestens 28 × 28 logische Pixel (`HSize.hitMin`), für Senden und Blockieren mindestens 32 × 120 px (5.4), gemessen im Widget-Test, nicht behauptet. Bekannter Verstoß: Der Augen-Umschalter, der ein maskiertes Secret aufdeckt, misst 28 × 19,2 px.
- **Textskalierung.** Bis `TextScaler.linear(2.0)` ohne `RenderFlex`-Überlauf und ohne abgeschnittenen Absatz. Keine Box, die Text enthält, hat eine feste Höhe; alle Zeilenhöhen aus 3.2 sind Mindesthöhen, und die Aktionsleiste bricht in eine Spalte um, wenn die skalierten Inhalte nicht mehr nebeneinanderpassen. Widget-Test und Golden bei 2.0 für Zeile der Warteschlange, Aktionsleiste, Kopf und Statusleiste; feste Höhen verschlucken den Überlauf stillschweigend, ohne diesen Test gibt es also keinen Fehlschlag, nur überlappenden Text.
- **Zweiter Kanal, abgestuft.** Die Glyphe steht überall neben der Zustandsfarbe, ohne Ausnahme. Die übersetzte Beschriftung in Alltagssprache steht überall, wo Platz für sie reserviert ist: Karte, Detailansicht, Gruppenkopf, Aktionsleiste, Bestätigungsstreifen. Wo eine Spalte 28 px misst, wie die Zustandsspalte der Verlaufstabelle, tragen Semantics-Beschriftung und Tooltip das Wort, und sie tragen es **immer**, in jeder Ansicht. Ohne diese Abstufung verbreitert der eine die Spalte, und der andere hakt die Checkliste falsch ab. Im hellen Theme messen `allowed` und `blocked` unter Deuteranopie 1,01:1; die Glyphe ist deshalb Pflicht und keine Dekoration.
- **Was angesagt wird**; heute gibt es im Programm weder `liveRegion` noch `SemanticsService.announce`:
  - **Ankünfte gebündelt, höflich**, höchstens eine Ansage pro zwei Sekunden: en "3 new, oldest GET api.github.com", de "3 neu, älteste GET api.github.com". Fünfzehn Ankünfte in zwanzig Sekunden brächten sonst fünfzehn volle URLs ins Ohr.
  - **Eine selbst ausgelöste Entscheidung höflich**, mit Host, Umfang und ob ein Rückgängig-Fenster läuft. Nur was der Mensch nicht ausgelöst hat, wird nachdrücklich angesagt: die Vorwarnung vor dem Zeitablauf und der eingetretene Zeitablauf.
  - **Ein abgewiesener Tastendruck höflich**, mit dem Grund aus 5.3.
  - **Das Anhaltebudget** nur an den Schwellen 5 min, 1 min, 30 s und 10 s, und nachdrücklich nur bei 5 %.
- **Keine Beschriftung ändert sich öfter als einmal pro Sekunde**, und der Countdown ist keine Live-Region. Eine Beschriftung, die sich viermal pro Sekunde ändert, ist die hörbare Form des Nörgelns. Die verbleibende Zeit steht deshalb nicht in der **Beschriftung** der Zeile, sondern in ihrem **Semantics-Wert** und wird nur an den vier genannten Schwellen erneut gesprochen; eine Beschriftung mit `mm:ss` ändert sich einmal pro Sekunde und wird von jedem Screenreader auf der fokussierten Zeile jedes Mal vollständig wiederholt. Zugleich ist der Countdown nicht mehr über `ExcludeSemantics` verborgen: Die Zeile trägt Zustand, Index, Gesamtzahl, Methode, Host und Pfad in der Beschriftung und die verbleibende Zeit im Wert. Ein Widget darf nie der einzige Träger einer Frist und zugleich von der Semantik ausgeschlossen sein.
- **Reduzierte Bewegung** nach 2.10.
- **Ein Zeitablauf ist keine Entscheidung eines Menschen**, nach 4.8.

---

## 7. Leistung als Budget

Ein Budget, das kein Test prüfen kann, ist eine Absichtserklärung. Die Tabelle trennt deshalb zwei Arten. **Schranken** laufen in `flutter test`, blockieren die CI und messen nur, was sich in einer Fake-Async-Umgebung ohne Rasterizer bestimmen lässt: Build-Zahlen, Element-Zahlen, Aufruf-Zahlen. **Messungen** laufen in `integration_test` mit `IntegrationTestWidgetsFlutterBinding.watchPerformance` unter Xvfb oder in DevTools, werden im PR berichtet und blockieren nicht: Frame- und Rasterzeiten, Speicher. `AutomatedTestWidgetsFlutterBinding` rastert nie, also ist jedes `FrameTiming` dort bedeutungslos.

| Fall | Budget | Art |
|---|---|---|
| 200 angehaltene Zeilen, fünf Ankünfte in einem Frame | kein Frame über 16 ms; p95 über 120 Frames im Profile-Build auf der in `CONTRIBUTING.md` genannten Maschine | Messung |
| eine Entscheidung | zwei Zeilen-Builds, nicht zweihundert | Schranke |
| eine Sekunde Uhr | höchstens ein Build pro sichtbarer Zeile | Schranke |
| ein Splitter-Frame | kein Build der Karte, ihres Kopfs und ihrer Body-Vorschau | Schranke |
| 8-MiB-Body, 10.000 Verlaufszeilen | kein sichtbares Ruckeln | Messung |
| 8-MiB-Body, 10.000 Verlaufszeilen | Speicher unter 300 MB | Messung, DevTools; `ProcessInfo.currentRss` sieht den GPU-Speicher nicht |

Regeln, die diese Budgets halten:

- **Zwei Uhren, nicht eine.** Die **UI-Uhr** (`nowProvider`, `HMotion.clockTick`, ganze Sekunden) treibt Beschriftungen und Ringe und steht still, sobald ihre Ansicht unsichtbar ist. Die **Richtlinien-Uhr** ist grob (5 s), hält nie an und löst nur aus, was auch in einem anderen Fenster gelten muss: die Vorwarnung vor dem Zeitablauf (4.8), den Tray-Zähler und die Ansagen (Abschnitt 6). Wer beide zu einer macht, tötet durch Sparen genau die zwei Fähigkeiten, für die es das Programm gibt, während der Mensch woanders ist.
- **Der Ring bekommt keinen eigenen Controller pro Zeile.** Über fünf Minuten bewegt sich das Bogenende eines 16-px-Rings um etwa 0,15 px pro Sekunde, also etwa 0,003 px pro Frame; zweihundert Ticker für Subpixel sind kein Budget, sondern eine Rechnung ohne Gegenwert. Der Ring liest dieselbe Sekundenuhr wie alles andere. Erst unter `HMotion.ringSmoothBelow` (60 s) bekommt er einen `AnimationController`, weil der Bogen dort etwa 0,8 px pro Sekunde läuft und die Sekundenschritte als Ruckeln sichtbar werden. Einen eigenen Controller braucht ohnehin nur, was unterhalb einer Sekunde etwas sagt: die Haltefüllung und das Aufziehen der Leiste.
- **Sichtbarkeit ist ein Provider, kein privates Feld.** Der Bildschirm Anhalten berechnet das Flag heute in `_InterceptScreenState`, wo niemand es lesen kann; es wird ein Provider, den die UI-Uhr beobachtet. `nowProvider` verliert dabei `keepAlive`; heute behauptet sein Doc-Kommentar, der Timer halte mit dem letzten Beobachter an, und `@Riverpod(keepAlive: true)` sorgt dafür, dass er es nicht tut.
- **Ein Provider, der filtert, sortiert oder gruppiert, sieht nie eine Uhr.** Sonst läuft pro Tick eine O(n log n)-Projektion über die gesamte Flow-Map, und n ist der Verkehr der ganzen Session, nicht die Länge der Warteschlange. Zeitabhängige Zeilen bekommen einen einmaligen Timer.
- **Die Flow-Map ist begrenzt.** Entschiedene Flows fallen heraus, sobald ihr Abgangsfenster vorbei ist; der Verlauf paginiert vom Daemon, nicht aus dieser Map. Eine wachsende Map, die pro Ereignis vollständig kopiert wird, macht aus einem Ereignis eine O(n)-Kopie.
- **Jeder abgeleitete Sammlungs-Provider gibt einen Typ mit Wertgleichheit zurück**, nie eine nackte `List` oder `Map`. Das Modell ist `QueueSnapshot` mit `listEquals`.
- **Skalare mit `.select`.** Nie `.length` auf einer beobachteten Sammlung.
- **Zwei Builds pro Entscheidung sind nur mit Memoisierung erreichbar.** `SliverChildBuilderDelegate.shouldRebuild` gibt bedingungslos `true` zurück, und ein `setState` des Bereichs pro Snapshot baut jede gebaute Zeile neu. Der Bereich hält deshalb `List<FlowId>` und merkt sich **eine** `QueueRow`-Widget-Instanz pro `FlowId`; die Zeile beobachtet `flowProvider(id)` und `selectedFlowIdProvider.select(...)` selbst. `Element.updateChild` hält bei einem identischen Kind-Widget an, und das ist der einzige Weg zu der Zahl. `flowProvider(FlowId)` gibt es noch nicht (Abschnitt 9).
- **Eine Zeile beobachtet nur Provider, die auf ihre eigene `FlowId` verschlüsselt sind.** Geteilter Zustand wie die laufende Entscheidung wird dort gelesen, wo er angezeigt wird.
- **Zeigerpräsenz und Einfrierzustand bleiben im `State` des Bereichs.** Die eingefrorene Reihenfolge ist Ansichtszustand und stirbt mit dem Bereich; nur der Zähler der ausstehenden Ankünfte ist ein Provider, weil Pill und Ansage ihn teilen. Zeigerbewegungen erreichen den Provider-Graphen nie. Das weicht von HUM-029 ab, siehe Abschnitt 8.
- **Fortlaufender Ziehzustand liegt in einem `ValueNotifier`**, auf den nur das Layout-Widget hört. Ein Provider-Schreibvorgang pro Zeigerbewegung baut den ganzen Bildschirm 120-mal pro Sekunde neu.
- **Feste Zeilenhöhen kennt die Liste dort, wo n groß ist und keine Zeile ihre Höhe ändert**: der Verlauf über `FixedSpanExtent`, Body- und Hex-Ansichten über `itemExtent`. Die Warteschlange von Anhalten ist ausgenommen (2.4), weil sie `AnimatedList` braucht und weil ihre Zeilenhöhe bei größerer Textskalierung ohnehin keine Konstante ist. Ohne Extent kostet Scrollen lineare statt konstanter Zeit; bei zehntausend Zeilen ist das der Unterschied, bei zweihundert nicht.
- **Animations-Wrapper verlassen den Baum**, sobald `animation.isCompleted` gilt. Eine Lazy-Liste baut nur ihren Viewport, also trägt eine Warteschlange nicht sechshundert überflüssige Render-Objekte, sondern etwa zwei pro sichtbarer Zeile, in einem 660 px hohen Bereich etwa sechsunddreißig. Genau die werden pro Frame bezahlt, und genau deshalb verschwinden sie. `CurvedAnimation` ist ein `late final`-Feld eines `State` und wird entsorgt, nie ein Ausdruck in `build`.
- **`FadeTransition` und `SlideTransition` statt `Opacity` plus `Transform` in einem `AnimatedBuilder`.** Die Transitions bauen den Kind-Baum nicht pro Frame neu; der Umweg über `AnimatedBuilder` tut es, und das an den zwei Stellen, die ohnehin am teuersten sind.
- **`RepaintBoundary` genau um die Inseln mit eigener Uhr**: Countdown-Ring, Aufziehen der Karte, atmende Glyphe, und um die ankommende und die gehende Zeile, weil dort pro Frame eine Opacity-Ebene bezahlt wird. Nirgends sonst; anderswo kostet sie nur eine Textur.
- **`Shortcuts`- und `Actions`-Maps sind `late final`-Felder**, keine Ausdrücke in `build`. Kinder, die ein neu bauender Elternteil unverändert durchreicht, sind `const` oder Felder.
- **Kein Formatieren, Kürzen oder Analysieren in einem Layout-Callback.** Einmal gegen die Constraints rechnen und gegen diese Constraints cachen; `middleEllipsis` legt pro Aufruf eine volle Rune-Liste an.
- **Alles, was einen Body berührt**, läuft oberhalb von 64 KiB in `Isolate.run` und gibt nur einfache Dart-Werte zurück.

Wie ein Test das beweist:

1. **Rebuild-Zähler (Schranke).** Eine statische Zählervariable im `build` des Widgets, eine Zustandsänderung über den Container, die genaue Zahl geprüft. Pflicht pro Bildschirm: `QueueRow` pro Entscheidung, `HeaderBar` pro Ereignis, `InterceptScreen` pro Splitter-Frame, Verlaufszeile pro Live-Aktualisierung. Der Umfang der Rebuilds ist für Compiler und Goldens unsichtbar und verschlechtert sich sonst stillschweigend.
2. **Lastschranke (Schranke).** Der Fake-Daemon kennt `burst:200`. Ein stehender Widget-Test nagelt damit die Zahl der `QueueRow`-Builds pro Sekunde Fake-Zeit und pro Entscheidung fest.
3. **Frame-Budget (Messung).** Für 10.000 Verlaufszeilen und einen 8-MiB-Body misst ein `integration_test` unter Xvfb Frame- und Rasterzeiten; die Zahl steht im PR. HUM-030 misst heute nur UI-Thread-Zeiten über `SchedulerBinding`, und HUM-032 führt die Frames als Risiko mit einer DevTools-Messung; beides ist weniger, als diese Tabelle verlangt, und wird entsprechend erweitert.

---

## 8. Wo dieses Dokument von einer Spezifikation abweicht

Damit die Abweichung prüfbar bleibt, steht sie hier und nicht zwischen den Zeilen.

- **HUM-028:** Der Bestätigungsstreifen gehört zur entschiedenen Zeile, nicht in die Karte, und er ist keine zusätzliche 28-px-Box, sondern der getauschte Inhalt derselben Zeile (3.4). In der Karte würde er drei Sekunden lang genau die Stelle blockieren, an der die nächste Entscheidung fällt.
- **HUM-028:** Das Freigabeventil trägt die Akzent-Tönung an der Token-Obergrenze (10 %), nicht 12 %. `HColorDerivation.tint` deckelt bei `HColors.tintAlpha`; ein Wert, den die Token-Schicht nicht erzeugen kann, ist keine Spezifikation, und zwei Prozent Alpha sieht niemand.
- **HUM-028 und `BACKLOG.md` 5:** Das deutsche Bedienelement sagt „Senden", nicht „Erlauben"; „Erlauben" gehört in den Regelsatz (4.6).
- **HUM-020 und `BACKLOG.md` 5:** Die Zeile der Warteschlange bleibt in jedem Zustand 36 px hoch und hat keine zweite Zeile, weil diese Zeile nur wiederholt, was die Karte daneben zeigt, und dabei die Liste unter dem Auge wandern lässt. 36 px sind eine Mindesthöhe, die mit der Textskalierung wächst.
- **HUM-020:** Aus der Zeile ist nur Blockieren möglich. Senden ist unumkehrbar und verlangt die URL, und die steht in der Karte.
- **HUM-029:** Der Gruppenkopf zeigt nur `Block {n}`; die Aktionsleiste übernimmt `Allow {n}`, sobald eine Gruppe ausgewählt ist (3.5).
- **HUM-029:** Die eingefrorene Reihenfolge lebt im `State` des Warteschlangenbereichs, nicht in einem `queueFreezeProvider`; nur der Zähler der ausstehenden Ankünfte ist ein Provider (Abschnitt 7).
- **HUM-029 und Abschnitt 7:** Die Warteschlange von Anhalten behält `AnimatedList` und verzichtet auf `itemExtent`; die Extent-Pflicht gilt im Verlauf und in den Body-Ansichten (2.4).
- **HUM-030:** Werte im JSON-Baum bleiben auf der `fg`-Leiter; Funde sind die einzige Farbigkeit.
- **2.9, Leerlauf:** Das Absperrband (3.6) läuft endlos, solange seine Ursache besteht. Es ist neben dem begrenzten Atem die einzige Dauerbewegung; der Projekteigentümer hat sich am 2026-10-20 dafür als Zeichen der Marke entschieden (HUM-260). Es steht nur an den zwei Stellen, die 3.6 nennt.
- **HUM-034:** Der Tray-Zähler steht in der Farbe von `held`, nicht als Akzentpunkt. Das Tray-Icon ist eine Zustandsanzeige, kein Bedienelement (3.3, Regel 9).
- **HUM-049:** Mit mindestens einem ungeklärten Fund verlangt Senden dieselbe Haltebestätigung wie Blockieren, dazu einen Satz, der die Art des Secrets und den Ziel-Host nennt, aber kein Modal (4.7).
- **`BACKLOG.md` 5:** Die Glyphe atmet begrenzt (zwei Schwellen, je drei Atemzüge) statt dauerhaft, sie hellt auf, statt zu verblassen, und eine Glyphe, die eine Schwelle überschreitet, beginnt ihren ersten Atemzug bei voller Deckkraft (2.7).
- **`packages/ui`:** Die ruhende `held`-Leiste in der Warteschlange ist namentlich von der 3:1-Regel ausgenommen (3.3, Regel 10). Jede andere Fläche und jede andere Leiste erreicht sie.
- **2.9 und `HTextField`:** Der Fokusring dieses **einen** Bedienelements blendet über 150 ms ein und reserviert keinen Raum, sondern zeichnet über seinen eigenen Rand hinaus. 2.9 verlangt einen Ring in einem Frame, weil einer, der einblendet, wie Eingabeverzögerung wirkt. Seit der Überarbeitung von ADR-0009 steht das Eingabefeld auf dem `TextField` von `shadcn_flutter`, und das baut seine `FocusOutline` fest ein, ohne Schalter. Zwei Ringe übereinander wären einer zu viel, also ist es ihrer. Seine Maße sind unsere: zwei Pixel Akzent, zwei Pixel Abstand, gesetzt über `FocusOutlineTheme` aus `HFocusRingMetrics`. Jedes andere Bedienelement trägt weiterhin `HFocusRing`, das seinen Raum reserviert und nicht animiert. Bei reduzierter Bewegung fällt das Einblenden des Bibliotheksrings auf 7,5 ms, weil sein Animationsbaustein ohne `animationBehavior` gebaut ist; das sichtbare Ergebnis ist dann wieder ein Frame.

---

## 9. Was das Designsystem noch braucht

> **Stand 2026-09-04.** Ein eigener Durchgang hat den größten Teil dieser
> Liste geschlossen: `HRow` mit Zustandsglyphe, Fokus und Aktionsplatz,
> `HFocusRing`, `HSkeleton` und `HWait`, `HTextField`, `HSegmented`,
> `HCheckbox`, die fehlenden Größen-Tokens, fünf Glyphen und vor allem der
> Kontrastdefekt: Zustandsfarben haben jetzt eine eigene Textvariante, die
> auf allen vier Flächen beider Leitern und auf Tönung, Hover- und Druckfüllung 4,5:1 erreicht,
> während die Flächenfarben bleiben, wie sie waren. Was noch
> unten steht und nicht als geschlossen markiert ist, gilt weiter.
>
> **Nachtrag 2026-09-04, zweiter Durchgang.** Zwei Reviews brachten neunzehn
> Befunde; sie sind behoben, und jeder hat einen Test, der ohne
> die Behebung rot wird. Der Atem endet wieder bei voller Deckkraft (`repeat(count:)`
> zählt Halbzyklen), die geteilte Pill malt Wort und Chevron in der
> Textvariante ihrer Fläche, der Akzent hat mit `accentText` und `accentFill`
> dieselbe Trennung von Fläche und Wort wie jede Zustandsfarbe, der Fokusring
> hält zwei Pixel Fläche frei, wo er sonst auf einer Füllung läge, die sich nicht
> von ihm unterscheiden lässt, also auf dem Primärknopf, gemessen und
> nicht aufgezählt (`HFocusRing.needsGap`), die Haltefüllung ist ein
> Token (`HColors.fillHoldAlpha`) und steht in der Liste, gegen die die
> Textableitung rechnet, `HSheet` hält den Fokus gefangen und schließt bei `Escape`,
> die Kopfzeile eines aufklappbaren Abschnitts und der Splitter der Bereiche sind
> Fokus-Halte mit Tastaturbedienung, und `HMotion` hat die zwei Fristen aufgenommen,
> die vorher als Literale in `core/ui` standen. Die
> Haltefüllung in der Textableitung verschiebt die Textpalette leicht; die
> eingefrorenen Tabellen in `tokens_test.dart` sind entsprechend neu.
>
> **Nachtrag 2026-09-04, dritter Durchgang.** Zehn weitere Befunde, ebenfalls
> jeder mit einem Test, der ohne die Absicherung rot wird. Die Haltefüllung der
> Pill lag auf der eigenen Tönung der Pill und ergab zusammengesetzt 0,248 statt 0,20;
> ihr Alpha wird jetzt zurückgerechnet (`HColorDerivation.alphaOver`), und
> `holdFill` in `core/ui` nimmt denselben Weg. Eine deaktivierte Checkbox und
> ein deaktiviertes Segment tragen `fg2` statt `fg0` und keinen Akzent mehr;
> die Checkbox füllt sich mit `accentFill` wie der Primärknopf. Die Tastenfüllung
> behält ihre 120 ms auch bei abgeschalteten Animationen: `AnimatedContainer`
> baut seinen Controller ohne `animationBehavior`, also
> übernimmt `HAnimatedFill` die Farbe von Knopf, Zeile und Segment. Die gemerkte
> Variante der Pill hat einen Tastaturweg (`Enter` oder Space gehalten),
> der Rand der Pill trägt denselben Ton wie ihr Wort, die
> Kopfzeile eines aufklappbaren Abschnitts hat eine Mindesthöhe statt einer Höhe und
> ihre Kurve als Feld statt als Ausdruck in `build`, und die Breite des
> Splitter-Griffs ist ein Token (`HSize.splitter`, ungerade, damit die
> Haarlinie auf einem ganzen Pixel liegt). Seither hat `core/ui` einen
> eigenen Test bei doppelter Textskalierung; er fand den Kopf der
> Diagnosekarte, der jetzt umbricht, statt abzuschneiden.
>
> Eine Entscheidung dieses Durchgangs geht gegen den ersten Anschein: Die
> Zustandsglyphe behält die **Flächenfarbe**. Der Doc-Kommentar von
> `stateTextColor` zählte sie zu dem, was die Textvariante nimmt, und der Code
> tat es nicht; korrigiert wurde der Kommentar. Eine Glyphe ist eine
> Grafik, ihre Schwelle ist das 3:1 aus Abschnitt 6, und die
> Flächenpalette hält das auf jeder Fläche beider Leitern. Mit der Textvariante
> verlöre ausgerechnet `autoRule` seinen Ton, die Farbe trägt 60 % Deckkraft,
> sie erreicht 4,5:1 nur nah an Weiß (dunkel) oder Schwarz (hell), und die
> Glyphe ist der Kanal, der die Farbe für Farbenblinde verdoppelt (3.3).

Gewachsen sind: `motion.dart` (`leaveOffset`, `stagger`, `staggerMax`, `leaveGlideFraction`, `holdToBlock`, `rearm`, `confirm`, `undoWindow`, `freezeAfterKey`, `freezeAfterPointer`, `clockTick`, `waitVisible`, `waitMinVisible`, `ringSmoothBelow`, `breatheBelowUrgent`, `breatheCycles`, `breatheMinOpacity`, `reducedRingAlpha`, `hoverLabel`, `copyFeedback` sowie `HReducedMotion`), `colors.dart` (`fillHoverAlpha`, `fillPressedAlpha`, `fillHoldAlpha`, `textMinContrast`, `areaMinContrast`, `fillAlphas`, `textBackgrounds`, `textVariant`, `readableFill`), `spacing.dart` (`rowHistory`, `rowBody`, `rowActionSlot`, `hitDecision`, `measureChars`, `splitterActive`, `splitterStep`), `flow_state.dart` (Glyphe und ARB-Schlüssel pro Zustand) und `tokens.dart` (`stateText`, `methodText`, `accentText`, `accentFill`, `stateTextOf`). Mit HUM-260 kamen `HColors.onAccent`, `HColors.warning`, `HColors.info` (samt hellen Varianten und `colors.warningText`, `colors.infoText`), `HMotion.cautionTape`, `HMotion.cautionTapePeriod`, `HSize.cautionTape` und das Widget `HCautionTape`. Alles, was folgt, ist offen und wird in eigenen Issues entschieden, nie nebenbei.

**Tokens:**

1. ~~`HSize.rowSelected` (56) wird nach 3.4 von keiner Zeile mehr benutzt. Entweder löschen oder für die Detailzeile des Verlaufs umwidmen.~~ Geschlossen am 2026-09-04: für die zweizeilige Detailzeile des Verlaufs umgewidmet.
2. ~~`HSize.selectionRail` (2) entfällt, sobald die Auswahl die Zustandsleiste ersetzt, statt sie zu überlagern.~~ Geschlossen am 2026-09-04: Die Auswahl ersetzt die Leiste über alle vier Pixel, und das Token markiert jetzt die aktive Ansicht der Icon-Leiste.
3. ~~Die drei Zeilendichten aus 3.2 brauchen drei Tokens: `HSize.rowHistory` (28), `HSize.rowBody` (24) und `HSize.rowActionSlot` (28).~~ Geschlossen am 2026-09-04: Alle drei stehen in `spacing.dart`, der Aktionsplatz borgt nicht mehr `hitMin`.
4. ~~Eine eigene Untergrenze für die beiden Entscheidungen: `HSize.hitDecision` (32 px Höhe, 120 px Breite) neben `HSize.hitMin` (5.4).~~ Geschlossen am 2026-09-04.
5. ~~Die Methodenfarben erreichen als Text auf ihrer eigenen Tönung nicht 4,5:1 (DELETE dunkel 2,65:1). Entweder Beschriftung und Flächenfarbe getrennt führen oder die Farbtöne anpassen.~~ Geschlossen am 2026-09-04: `HMethodColors.darkText` und `lightText` führen die Abkürzung getrennt von der Fläche, abgeleitet über `HColorDerivation.textVariant`.
6. ~~Der Countdown-Ring nimmt `tokens.colors.line` als Spur; im hellen Theme misst der Bogen dagegen 2,84:1.~~ Entschieden am 2026-09-04, zugunsten der Lücke: Der Ring zeichnet jetzt nur den verbleibenden Bogen, die verbrauchte Strecke bleibt Fläche. Eine Spur hätte zwei Bedingungen zugleich erfüllen müssen, auf jeder Bereichsfläche sichtbar sein und dem Bogen 3:1 lassen, und die abgeleitete helle Spur erfüllte die erste nicht: Sie maß 1,01:1 bis 1,26:1 gegen die vier hellen Flächen und war auf keinem Bereich zu sehen. Der Bogen allein erreicht als Zustandsfarbe seine 3:1 auf jeder Fläche beider Leitern. Das Token `ringTrack` ist damit wieder weg.
7. ~~Es gibt kein Token für die Obergrenze der Satzbreite aus 3.2. Vorschlag: eine Zeichenzahl (90), keine Pixelbreite.~~ Geschlossen am 2026-09-04: `HSize.measureChars` (90) und `HSize.measureWidth`.
8. Eine Leiste, die 3:1 erreicht, bräuchte ein eigenes Alpha-Token weit über `HColors.tintAlpha`. Dieses Dokument will es nicht (3.3, Regel 10); wer es trotzdem will, führt das Token ein und darf `tint()` dafür nicht missbrauchen.

**Widgets in `packages/ui`:**

9. ~~Eine gehende Zeile braucht ein eigenes Widget ohne `ref`: `QueueRowSnapshot` (oder `HRow.frozen`).~~ Geschlossen am 2026-09-04, und ohne zweites Widget: `HRow` beobachtet nichts und nimmt ausschließlich aufgelöste Werte, Zustand, Glyphe, Methode, Host, Pfad, verbleibende Zeit als String. So dient dieselbe Klasse der lebenden und der gehenden Zeile, und der Abgang zeichnet ein eingefrorenes Bild mit angehaltenem Countdown (2.4). Der Doc-Kommentar von `HRow` sagt dasselbe; die beiden Texte widersprachen sich, bis dieser angeglichen wurde.
10. ~~`HRow` malt Hover und Auswahl beide `bg3`. Hover muss `bg2` werden (3.4).~~ Geschlossen am 2026-09-04.
11. ~~`HRow` benutzt `HMotion.sweep` für Hover-Tönung und Höhe. Hover gehört zu `HMotion.press`, die Höhe animiert nicht.~~ Geschlossen am 2026-09-04.
12. ~~`HRow` hat keinen Platz für eine Zustandsglyphe und ist nicht fokussierbar. Dazu ein Platz für die Leiste der Mehrfachauswahl (3.5).~~ Geschlossen am 2026-09-04; der Kopf dieses Abschnitts sagt dasselbe.
13. ~~`HMethodBadge` braucht eine neutrale Variante (`fg1` auf `bg2`) für Listen; die farbige bleibt dem Kopf der Karte vorbehalten.~~ Geschlossen am 2026-09-04.
14. ~~`HButton` zeichnet den Fokus durch Umfärben des vorhandenen 1-px-Rands. Gebraucht wird ein 2-px-Akzentring außen; dafür reichen zwei `BoxDecoration`s, ein neues Token ist nicht nötig.~~ Geschlossen am 2026-09-04: `HFocusRing` zeichnet den Ring außen, `HFocusRing.inline` am Rand der geteilten Pill.
15. `HStateGlyph`: `easeInOut`, die Untergrenze `HMotion.breatheMinOpacity`, `HMotion.breatheCycles` pro Schwelle, der Beginn bei voller Deckkraft und der ruhende Ersatzring bei reduzierter Bewegung sind seit 2026-09-04 vorhanden; das Ende des Atems ebenso (`repeat(reverse: true, count:)` zählt Halbzyklen, eine ungerade Zahl endet bei der niedrigsten Deckkraft). Offen bleibt nur die Phase: Die Glyphe hat einen eigenen Controller, statt ihre Phase aus der Uhrzeit modulo `HMotion.breathe` zu nehmen. Zwei Zeilen, die im selben Moment unter die Schwelle fallen, atmen deshalb im Gleichtakt, zwei aus verschiedenen Momenten nicht.
16. ~~`HModal` hat weder einen `FocusScope` noch eine `Escape`-Bindung.~~ Geschlossen am 2026-09-04: `FocusScope` mit `autofocus`, `Escape` über `DismissIntent`. `HSheet` hatte dieselbe Lücke und hat seit 2026-09-04 dieselbe Behandlung; ohne sie lief `Tab` durch den Bildschirm, den das Sheet verdeckte.
17. ~~`HPill`, `HIconButton`, `HRow` und `HBadge` mit `onTap` sind nicht fokussierbar; `HButton` ist heute das einzige fokussierbare Bedienelement im System.~~ Geschlossen am 2026-09-04: Zehn der 21 Widgets tragen eine eigene Fokusbehandlung, darunter alle vier.
18. ~~`HBadge.chipHeight` und `HButtonSize` sind feste Höhen und laufen bei `TextScaler` 2.0 über. Mindesthöhen statt Höhen.~~ Geschlossen am 2026-09-04; seit demselben Tag deckt der Test bei doppelter Skalierung auch `HPill`, `HSegmented`, `HIconButton` und `HModal` ab.
19. ~~Ein Skelett-Widget für 2.11: Haarlinien in der Höhe einer Zieldichte, `fg2`, ohne Bewegung, mit den zwei eingebauten Schwellen `waitVisible` und `waitMinVisible`.~~ Geschlossen am 2026-09-04: `HSkeleton` und `HWait`.

**In `app/lib`, außerhalb dieses Dokuments zu entscheiden:**

20. `flowProvider(FlowId)` gibt es nicht; Abschnitt 7 braucht ihn für die Memoisierung der Zeilen.
21. Die Sichtbarkeit einer Ansicht braucht einen Provider; heute ist sie ein privates Feld von `_InterceptScreenState`.
22. `nowProvider` verliert `keepAlive` und wird die UI-Uhr; die Richtlinien-Uhr aus Abschnitt 7 kommt daneben.
23. `ConnectionGate` ersetzt heute die ganze Shell, sobald die Verbindung ausfällt, auch mitten in der Arbeit. 4.2 Punkt 4 verlangt, dass der Einrichtungsbildschirm nur bei einem Kaltstart übernimmt.

**Tests:**

24. ~~Der Kontrasttest prüft Farbe auf Fläche bei 3:1. Er braucht Text auf Tönung und Text auf Füllung bei 4,5:1.~~ Geschlossen am 2026-09-04: `every state colour carries text at 4.5:1 in both themes`, `every method hue carries its verb at 4.5:1 in both themes` und `every fill of the system is a fill the derivation saw` in `packages/ui/test/tokens_test.dart`; die Ausnahme aus 3.3 Regel 10 steht namentlich im Test `the area palette is exactly what it claims`.
25. Es gibt keinen Test, der beweist, dass jede Zustandsfarbe von Glyphe **und** Beschriftung begleitet wird. (Die zweite Hälfte dieses Punkts, ein Test bei `TextScaler` 2.0, ist mit Punkt 18 erledigt: `packages/ui` prüft die Skalierung seit 2026-09-04 für zehn Bedienelemente, `app/lib/core/ui` für seine drei seit demselben Tag.)
26. Ein ARB-Lint prüft die verbotenen Wörter aus 4.1 in beiden Sprachen und kennt die drei Aussage-Schlüssel als Ausnahmen.
27. Ein Widget-Test hält `Enter` 500 ms gedrückt und erwartet genau ein `Decide` (5.4).

**Was die Bibliothek liefert und was nicht (ADR-0009, „Am 2026-09-04 vom Projektinhaber überarbeitet"):**

`shadcn_flutter` ist seit 2026-09-04 übernommen und liefert Resizable,
Command Palette, Sheet und ContextMenu. Toast und Menubar entfallen trotzdem:
Ein Toast widerspricht 2.11, 4.6 und 4.8, und die zwei Wege zu jeder Aktion
sind die Icon-Leiste und die Befehlspalette (5.1), keine Menüleiste.

Die Bibliothek liefert das Aussehen, nicht die Verhaltensschicht darunter. Sie
berechnet nirgends einen Kontrastwert, kennt `AnimationBehavior` nicht,
hat in der ganzen Bibliothek drei `Semantics`-Knoten, kein
Halten zum Bestätigen, keine Mindest-Zielfläche und auf dem Desktop keinen
gedrückten Zustand. Was davon im MVP trotzdem gilt, steht im ADR: Die
Haltedauer behält ihre Zeit, Text bleibt über etwa 3:1, und ein Klick zeigt
eine Reaktion. Der Rest ist bis 1.0.0 zurückgestellt.

28. ~~Ein Kontextmenü fehlt.~~ Geschlossen am 2026-09-04: `shadcn_flutter`
    bringt eines mit. Es wird zweimal gebraucht, „Wert kopieren" und „Pfad kopieren" im
    JSON-Baum und Kopieren/Einfügen im Terminal; es wird in `packages/ui` gekapselt,
    nicht in einem der beiden Features, weil beide dieselbe Tastaturbehandlung und
    denselben Fokusring brauchen.
29. ~~Ein Datum-Uhrzeit-Picker fehlt.~~ Geschlossen am 2026-09-04: Die Bibliothek
    bringt einen mit. Beim Kapseln gilt weiterhin, dass die Uhrzeit im
    Zeitraumfilter des Audit-Bildschirms in Monospace steht, weil sie Teil des
    Belegs ist (`backlog/CONVENTIONS.md` 4.13).
30. ~~`HResizablePanes` kennt nur die waagerechte Achse.~~ Geschlossen am
    2026-09-04: Die Bibliothek kennt beide Achsen. Ihre zwei offenen Bugs zur
    senkrechten Achse (#427, #428) waren der Grund, warum HUM-035 sie als Risiko führte;
    beim Kapseln ist zu prüfen, ob sie uns betreffen. Seit
    HUM-305 hat unser eigenes `HResizablePanes` ebenfalls die senkrechte Achse (die
    Sandbox-Ansicht stapelt Terminal und Tabs); die Bibliothek ist weiterhin nicht
    gekapselt.
31. `FocusRing` in `app/lib/core/ui` und `HFocusRing` in `packages/ui` sind
    dasselbe zweimal. Der Kommentar, der das mit einem Zustand begründete,
    den Punkt 14 beseitigt hat, wurde am 2026-09-04 korrigiert; die Datei
    bleibt, weil fünf Bedienelemente der Aktionsleiste und des Verlaufs sie benutzen
    (`block_button`, `note_field`, `release_valve`, `remember_grid`,
    `history_table`). Bis zum Umzug liest sie ihre Geometrie aus `HFocusRing`,
    damit die beiden Ringe nicht auseinanderdriften; ein Test hält die beiden
    Abstände gleich. Wenn die Übergabe-Widgets umziehen, bleibt einer der beiden.
32. Kein Bildschirm hat je `HPill` und `HPanel` aufgerufen; sie stehen nur in
    der Galerie. Entweder bekommen sie einen Aufrufer, oder sie entfallen.

---

## 10. Checkliste vor „fertig"

- [ ] Jede Animation beantwortet eine der vier Fragen aus 2.1; jede Dauer, Kurve und Strecke kommt aus `HMotion`, kein Literal in einer Feature-Datei.
- [ ] Jede Überlagerung, Sheet, Modal, Palette, Banner, Ansichts- und Tab-Wechsel, nimmt ihre Zeile aus 2.2 und erfindet keine eigene Bewegung.
- [ ] Nichts bewegt sich, was gelesen wird; keine Zahl, kein Fokusring und keine Zeilenhöhe animiert wegen eines Zustandswechsels.
- [ ] Strecken und Schleifen laufen über `HReducedMotion`, jede Schleife hat einen ruhenden Ersatz, und der Abgang behält bei reduzierter Bewegung seine Rückmeldung (2.10).
- [ ] Jeder wartende Vorgang folgt 2.11: nichts unter 150 ms, danach ein Skelett in der Zieldichte, mindestens 400 ms stehend, kein Spinner, keine Verschiebung bei der Ankunft.
- [ ] Der Bildschirm hat genau ein gefülltes Bedienelement pro Entscheidungskontext, einen leeren Zustand und ein größtes Textelement, und das ist das Wichtigste darauf.
- [ ] Jede Zustandsfarbe wird von der Glyphe begleitet, von der Beschriftung in Alltagssprache überall, wo Platz reserviert ist, und von Semantics und Tooltip immer; Akzent nur auf Fokussierbarem oder Berührbarem; Rahmenelemente ohne zweiten Wert tragen `fg1`.
- [ ] Alle Abstände sind Vielfache von 4, der rechte Rand misst 12 px, die Satzbreite höchstens 90 Monospace-Zeichen, außer in Code, Hex und Tabellen, die horizontal scrollen und nie umbrechen.
- [ ] Jeder leere Zustand nennt das nächste Ereignis oder, bei einem Filter, die Zahl der passenden Einträge und den Weg zurück; kein „nichts", kein „noch", kein internes Vokabular, jeder Text aus ARB in beiden Sprachen.
- [ ] Die vier Zustände vor der ersten Anfrage aus 4.2 sind gebaut, und der Einrichtungsbildschirm ersetzt die Shell nur bei einem Kaltstart.
- [ ] Ein Zeitablauf hinterlässt eine Vorwarnung am Flow, drei Sekunden graue Zeile und einen dauerhaften Zähler in der Statusleiste (4.8).
- [ ] Eine Anfrage mit einem Fund trägt den Chip in `HColors.secret`, den Grund in Alltagssprache und die Haltebestätigung beim Senden (4.7).
- [ ] Jeder Fehlerpfad zeigt ein `Diagnostic` mit dem `why` des Daemons und mit `fix`, verankert am Ort der Aktion.
- [ ] Jeder Aktivator hat eine Aktion, jeder Zeigerweg eine Taste, jeder angenommene Druck eine sichtbare Reaktion innerhalb von 100 ms, jeder abgewiesene eine ruhige und eine angesagte.
- [ ] Weder `Enter` noch `Space` noch ein einzelner Buchstabe gewinnt gegen ein fokussiertes Bedienelement; ein Test vergleicht beide Tastenmengen, und ein zweiter hält `Enter` 500 ms gedrückt und erwartet ein `Decide`.
- [ ] Nach jeder neuen Auswahl ist Senden für `HMotion.rearm` entschärft, Tastenwiederholung entscheidet nie, und die Auswahl rückt nicht weiter, solange eine Entscheidungstaste gedrückt ist.
- [ ] Kontrast: Text ≥ 4,5:1 auch auf Tönung und Füllung, Flächen ≥ 3:1 mit der einen benannten Ausnahme; Zielflächen ≥ 28 × 28 px, Senden und Blockieren ≥ 32 × 120 px, gemessen im Test.
- [ ] Widget-Test und Golden bei `TextScaler` 2.0 ohne Überlauf; Semantics tragen Zustand, Index und Gesamtzahl in der Beschriftung und die Frist im Wert; keine Beschriftung ändert sich öfter als einmal pro Sekunde; Ankünfte werden gebündelt.
- [ ] Ein Rebuild-Zähler-Test nagelt die Builds pro Entscheidung, pro Sekunde Uhr und pro Splitter-Frame fest; die Frame- und Speicherzahlen stehen als Messung im PR, nicht als Schranke im Unit-Test.
- [ ] `make check` grün, Goldens grün, und jede Abweichung von einer Spezifikation steht in Abschnitt 8.
