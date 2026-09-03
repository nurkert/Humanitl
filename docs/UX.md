# Humanitl — UX-Leitbild

> Zweck: Die Oberfläche soll eine Entscheidung so leicht machen, dass ein Mensch sie stundenlang trifft, ohne müde zu werden, und so schwer, dass er sie nicht versehentlich trifft. Dieses Dokument legt fest, wie Bewegung, Farbe, Text und Tastatur zusammenspielen, damit jeder Screen von HUM-028 bis HUM-034 dieselbe Sprache spricht. Es ergänzt `BACKLOG.md` Abschnitt 5 (Richtung „Airlock"), `docs/ARCHITECTURE.md` (Schichten) und `app/packages/ui` (Werte).

Bei Widerspruch gilt: `packages/ui` gewinnt für Werte, dieses Dokument für ihre Verwendung. Wo eine Regel hier ein Token bräuchte, das es nicht gibt, steht sie in Abschnitt 9. Token werden nie beiläufig geändert.

## 1. Warum es dieses Dokument gibt

Ein Mensch entscheidet hier über fremden Netzwerkverkehr, unter einer laufenden Uhr, über Stunden. Jede Bewegung, jede Farbe und jedes Wort auf dem Schirm kostet Aufmerksamkeit, die er für die nächste Entscheidung braucht. Dieses Dokument legt fest, wofür diese Aufmerksamkeit ausgegeben werden darf, und macht jede Festlegung prüfbar.

Es steht neben `backlog/CONVENTIONS.md` 4.13, wo festgehalten ist, warum dieses Programm Vertrauen ausstrahlen muss und woran man das misst: nie mehr behaupten als bewiesen ist, Genauigkeit dort, wo sie zählt, Zurückhaltung, Vorhersagbarkeit, keine dunklen Muster in beide Richtungen, Fehler mit Grund und Abhilfe, sichtbares Handwerk. Was hier steht, ist die gestalterische Ausführung jener Zusage; wo beide Dokumente dasselbe berühren, gewinnt 4.13 in der Aussage und dieses Dokument in der Ausführung.

### 1.1 Wofür die gesparte Aufmerksamkeit ausgegeben wird

Die meisten Regeln hier verbieten etwas. Das ist die halbe Arbeit; die andere Hälfte ist, den frei gewordenen Etat an einer Stelle je Screen auszugeben, statt ihn zu sparen. Ein Programm, das nur spart, wird ein sauberes Derivat ohne eigene Handschrift.

Jeder Screen hat deshalb einen benannten Moment, an dem er schön sein darf. Genau einen:

| Screen | Der Moment | Woran man ihn erkennt |
|---|---|---|
| Intercept | die Schleusenkammer im Augenblick der Entscheidung | Der Rail der entschiedenen Zeile wischt auf volle Sättigung und ist für 200 ms die einzige gesättigte Fläche im ganzen Pane. Alles andere bleibt neutral, damit dieser eine Wisch etwas bedeutet. |
| Rules | die Reihenfolge als lesbare Kette | Die Regeln stehen als Sätze untereinander und werden von oben nach unten gelesen wie ein Text; die erste, die zutrifft, gewinnt, und das sieht man, ohne es zu wissen. Der Dry-Run macht diese Kette an echtem Verkehr sichtbar. |
| History | die Dichte als Beleg | Zehntausend Zeilen in gleichmäßiger 28-px-Kadenz, ohne Lücke, ohne Ladefläche. Die Menge selbst ist die Aussage: nichts ist unbeobachtet durchgegangen. |
| Domain-Panel | die Antwort in einer Zeile | „Bekannt" oder „unbekannt" steht als erstes Wort, in der größten Type des Panes, bevor irgendein Detail folgt. |
| Setup und Sandbox | das Urteil über die drei Garantien | Drei Zeilen, drei Zustände, keine Erklärung davor. Die Erklärung steht darunter, für den, der sie will. |

Was in diesen fünf Momenten nicht vorkommt, bekommt keine Sättigung, keine Bewegung und keine große Type. Das ist der Handel.

---

## 2. Die Bewegungssprache

### 2.1 Die Prüffrage

Jede Animation beantwortet genau eine von vier Fragen: Was ist angekommen? Was ist gegangen? Was hat sich an Ort und Stelle geändert? Was habe ich gerade ausgelöst? Eine Animation, die keine dieser Fragen beantwortet, wird gelöscht, nicht abgestimmt. Diese Frage ist das Review-Gate für HUM-029 bis HUM-034.

Jede Dauer, jede Kurve und jede Strecke kommt aus `HMotion`. Eine `Duration`, eine `Curve` oder ein Pixelversatz als Literal in einer Feature-Datei ist ein Defekt, auch wenn die Zahl stimmt: eine Tabelle ist der einzige Weg, auf dem fünf ungebaute Screens dieselbe Sprache behalten.

### 2.2 Die Tabelle

| Änderung auf dem Schirm | Was sie erklärt | Dauer | Kurve | Token | Weg |
|---|---|---|---|---|---|
| Zeile kommt in der Queue an | „Ein Paket ist von außen eingetroffen" | 180 ms | `enter` | `HMotion.arrive` | 8 px von oben, `HMotion.arriveOffset`, plus Einblenden |
| Mehrere Zeilen im selben Frame | „Es sind mehrere, nacheinander" | +30 ms je Zeile, höchstens 5 | `enter` | `HMotion.stagger`, `HMotion.staggerMax` | wie oben, versetzt |
| Der Queue-Zähler erhält eine Ankunft | „Es ist etwas gekommen, auch wenn du es nicht siehst" | 2 × 120 ms, ein Hin und Zurück | `easeInOut` | `HMotion.press` | Tönung des `HBadge` im Queue-Kopf von `held` 10 % auf `held` 20 % und zurück; genau dieses eine Widget, auch bei eingefrorener Queue. Die Ziffer darin springt, sie animiert nie (2.9). |
| Control wird gedrückt (Maus oder Taste) | „Dein Griff ist angekommen" | 120 ms | `enter` | `HMotion.press` | Füllung |
| Rail wechselt den Zustand | „Der Zustand hat sich geändert" | 200 ms | `enter` | `HMotion.sweep` | Wisch von oben nach unten, nie eine Überblendung |
| Halten bestätigt eine Blockierung | „Das war gewollt" | 250 ms | linear | `HMotion.holdToBlock` | Füllung von links |
| Halten füllt die Release Valve | „Das gilt für die ganze Sitzung" | 400 ms | linear | `HMotion.holdToConfirm` | Füllung von links |
| Entscheidung unterwegs | „Ich habe gegriffen, die Antwort steht aus" | — | — | — | keine Bewegung. Der Rail hält seinen gewischten Zustand, das Glyph ist getauscht, der Streifen fehlt noch. Bleibt die Antwort über `HMotion.confirm` aus, nimmt das inline-`Diagnostic` aus 4.4 den Platz des Streifens. |
| Abgelehnter Tastendruck | „Diese Taste gilt hier gerade nicht" | 120 ms | `enter` | `HMotion.press` | Das Control, das gehandelt hätte, zeigt seine Füllung und läuft wieder leer; der Grund steht daneben (5.3). Nie ein Rütteln, nie ein rotes Blitzen. |
| Entschiedene Zeile geht | „Diese Anfrage ist erlaubt bzw. blockiert draußen" | 220 ms | `exit` | `HMotion.leave` | 12 px zur Seite (`HMotion.leaveOffset`), dann Höhe auf null |
| Auswahl wechselt | „Ich sehe jetzt diese Zeile an" | ein Frame | — | — | Füllungswechsel, keine Höhenänderung |
| Abschnitt oder Gruppe klappt auf | „Darunter liegt mehr" | 180 ms | `enter` | `HMotion.arrive` | Höhe |
| Sheet fährt von rechts ein | „Das gehört zu dem, was links steht" | 180 ms | `enter` | `HMotion.arrive` | 8 px von rechts (`HMotion.arriveOffset`) plus Einblenden; der Hintergrund bleibt stehen |
| Modal erscheint | „Hier musst du erst antworten" | 180 ms | `enter` | `HMotion.arrive` | kein Weg, nur Einblenden des Modals und Abdunkeln des Hintergrunds; ein Modal, das hereinfliegt, behauptet eine Richtung, die es nicht hat |
| Command Palette öffnet | „Das Handbuch ist da" | ein Frame | — | — | keine Bewegung. Wer sie ruft, tippt bereits. |
| Sektionswechsel (`Ctrl+1..5`) und Tabwechsel | „Ich bin jetzt woanders" | ein Frame | — | — | keine Überblendung im `IndexedStack`, keine Verschiebung. Der Wechsel ist Navigation, kein Ereignis. |
| Rückkehr-Banner erscheint | „Während du weg warst, ist etwas passiert" | 180 ms | `enter` | `HMotion.arrive` | wie eine Ankunft: 8 px von oben plus Einblenden |
| Neue Regel erscheint in der Liste | „Diese Regel gibt es jetzt" | 240 ms | `enter` | `HMotion.ruleDraw` | Zeichnen der Zeile |
| Countdown unterhalb einer Schwelle | „Sieh jetzt hin" | 1200 ms je Zug, 3 Züge | `easeInOut` | `HMotion.breathe`, `HMotion.breatheCycles` | Deckkraft zwischen `HMotion.breatheMinOpacity` und 1,0 |
| Fehler | „Hier ist etwas schiefgegangen" | — | — | — | keine Bewegung; das Diagnostic blendet über `HMotion.arrive` in einen reservierten Slot ein |

### 2.3 Richtung ist Bedeutung

Erlaubt und erlaubt-nach-Bearbeitung gleiten nach rechts, blockiert, abgelaufen und Fehler nach links. Sonst bewegt sich nirgends im Programm ein Widget waagerecht — außer dem Sheet, das aus der Kante kommt, an der es hängt.

Die Achsenregel bindet die **Position** eines Widgets, nicht seine **Füllung**. Eine Füllung, die von links wächst, ist Fortschritt und keine Richtung: sie sagt „so weit bin ich", nicht „dorthin geht es". Deshalb dürfen beide Halte-Füllungen von links laufen, ohne die Regel zu brechen, und deshalb füllt nichts im Programm von rechts, von oben oder aus der Mitte — eine zweite Füllrichtung wäre eine zweite Bedeutung für dieselbe Geste.

Eine einzige Bedeutung für eine einzige Bewegungsachse ist am Rand des Blickfelds lesbar, und dort steht die Queue meistens.

### 2.4 Der Abgang hat zwei Phasen

Gleiten und Ausblenden laufen in den ersten sechzig Prozent von `HMotion.leave`, das Zusammenfallen der Höhe in den letzten sechzig (`HMotion.leaveGlideFraction`, die Phasen überlappen). Auf einer gemeinsamen `easeIn`-Kurve steht die Zeile rund 170 ms still und springt erst in den letzten Frames zur Seite; genau die Richtung, die die ganze Bedeutung trägt, ist dann unsichtbar.

Der Kollaps bleibt, und er kostet: eine Liste, deren Zeilen ihre Höhe animieren, kann ihre Zeilenhöhe nicht vorab kennen. Die Queue behält deshalb `AnimatedList` und verzichtet auf `itemExtent`; die Extent-Pflicht aus Abschnitt 7 gilt dort nicht, sondern in History und in den Body-Ansichten, wo n groß ist und keine Zeile die Höhe wechselt. `AnimatedList` und `SliverAnimatedList` kennen weder `itemExtent` noch `prototypeItem`; wer beides zugleich verlangt, verlangt etwas, das Flutter nicht anbietet. Die Rechnung geht auf, weil die Queue durch das Haltebudget beschränkt ist, während History zehntausend Zeilen trägt (Abweichung in Abschnitt 8).

Eine gehende Zeile ist ein eingefrorenes Abbild: sie beobachtet keinen Provider, lädt kein Detail, ihr Countdown steht ab dem Augenblick der Entscheidung, und sie behält die Höhe, die sie in diesem Augenblick hatte. Eine Zeile, die während ihres Abgangs weiterzählt, behauptet etwas Falsches. „Eingefroren" ist wörtlich gemeint und braucht ein eigenes Widget mit aufgelösten Werten statt eines `ref` (Abschnitt 9).

### 2.5 Der Wisch gehört dem Griff, der Streifen der Antwort

Der Rail-Wisch startet mit der Entscheidung selbst und läuft die vollen 200 ms durch, unabhängig davon, wann der Daemon antwortet. Der Bestätigungsstreifen erscheint erst, wenn die Antwort da ist. Zwei Gutachten widersprechen sich hier — „zeige die Wirkung sofort" gegen „behaupte nichts vor der Antwort" —, und beide behalten recht, weil sie von zwei verschiedenen Dingen sprechen: der Wisch sagt „dein Griff ist angekommen", der Streifen sagt „das ist passiert". Über einen Unix-Socket dauert `Decide` wenige Millisekunden; an `DecisionSending` gekoppelt füllt der Rail rund zwei Prozent und läuft wieder leer, also findet der eine Moment, für den das Produkt existiert, ohne sichtbare Bewegung statt.

Zwischen Wisch und Streifen liegt der Zustand, der am längsten dauert, wenn etwas schiefgeht: die Entscheidung ist unterwegs. Er hat bewusst keine eigene Bewegung. Der Rail steht gewischt, das Glyph ist getauscht, der Streifen fehlt. Kommt innerhalb von `HMotion.confirm` keine Antwort, nimmt das `Diagnostic` aus 4.4 den Platz ein, den der Streifen bekommen hätte, und die Zeile geht nicht.

### 2.6 Die Wirkung steht in 400 ms, nicht in 3 s

Was die App allein entscheidet, steht innerhalb von 400 ms nach dem Tastendruck: Rail gewischt, Zustands-Glyph getauscht, Countdown eingefroren. Was von der Antwort abhängt, steht innerhalb von 400 ms nach der Antwort: der Bestätigungssatz. Die Trennung ist nicht kosmetisch — die App kontrolliert die Latenz des Daemons nicht und darf deshalb keine Frist versprechen, die sie nicht halten kann.

Die drei Sekunden bis zum Abgang der Zeile (`HMotion.confirm`) sind ein Ruhezustand und kein Wartezustand. Drei Sekunden Totzeit zwischen Griff und Bewegung zerreißen den Zusammenhang: wenn die Zeile fällt, ist der Mensch schon weiter, und die Bewegung hat keinen sichtbaren Anlass mehr.

### 2.7 Der Atem ist eine Flagge, keine Skala

Die verbleibende Zeit steht in der Bogenlänge des Rings. Der Atem sagt nur „jetzt hinsehen": drei Züge, wenn der Flow 20 % seines Budgets unterschreitet, drei weitere bei 5 %, dann Ruhe. Er läuft über `Curves.easeInOut`, nicht auf einem linearen Controller mit `repeat(reverse: true)` — eine Dreieckwelle hat an beiden Enden eine Ecke, und die liest das Auge als Blinken. Er nimmt das Glyph nie unter `HMotion.breatheMinOpacity` (0,72); heute fällt es auf 0,45, was die dringendste Anfrage zur am schlechtesten sichtbaren macht.

Die Phase kommt aus der Uhrzeit modulo `HMotion.breathe`, damit Gruppenkopf und Zeilen im Takt atmen, ohne dass es einen anwendungsweiten Ticker braucht. Ein Glyph, das die Schwelle gerade überschreitet, übernimmt diese Phase aber **nicht sofort**: es startet seinen ersten Zug bei Deckkraft 1,0 und läuft erst danach in die gemeinsame Phase ein. Sonst beginnt die Flagge, je nach Sekunde, mit einem Sprung von 1,0 auf 0,72 in einem Frame — genau das Blinken, das dieser Abschnitt verbietet, und ausgerechnet an der dringendsten Zeile.

Fällt eine Schwelle, während die Sektion unsichtbar ist, verfallen die drei Züge. Sie werden nicht nachgeholt. Der Atem sagt „jetzt hinsehen", und dieses Jetzt ist vorbei; was bleiben muss, bleibt im Bogen, im Semantics-Value und in der Warnung aus 4.8.

### 2.8 Nichts bewegt sich unter dem lesenden Auge

Eine Ankunft darf nie eine Zeile über dem Zeiger oder über dem Scroll-Anker einfügen. Solange der Zeiger im Queue-Pane steht, die letzte Tastaturnavigation weniger als `HMotion.freezeAfterKey` (2 s) her ist oder eine Mehrfachauswahl besteht, bleibt die Reihenfolge eingefroren; neue Flows erhöhen nur die Pille „+n neu". Die Pille liegt über der obersten Zeile, halbtransparent, und wird nie in die Spalte eingefügt — eine Pille im Kopf schöbe jede Zeile um ihre eigene Höhe nach unten, also genau das, wogegen das Einfrieren existiert. Zusammengeführt wird bei Klick oder `HMotion.freezeAfterPointer` (500 ms) nach dem Verlassen des Panes.

Die Pille ist erreichbar, nicht nur sichtbar: sie ist ein Fokusstopp und hat eine Taste (`Shift+J`, „zu den neuen"). Ohne Taste kann ein Tastaturnutzer die Ankünfte nie zusammenführen, ohne zur Maus zu greifen. Während des Einfrierens bleibt die Auswahl auf ihrer Zeile und wandert beim Zusammenführen nicht mit; der Mensch hat sie dorthin gesetzt, nicht die Liste.

**Entschieden ist nicht entfernt.** Die beiden Wörter bezeichnen zwei verschiedene Ereignisse, und nur eines gibt Platz frei:

- **Entschieden** heißt: die Zeile bleibt an ihrem Platz im Schnappschuss, bis `HMotion.confirm` abgelaufen ist. Sie zählt nicht als entfernt, sie schiebt nichts, und sie trägt in dieser Zeit den Bestätigungsstreifen (3.4). Der Beleg gehört zur Zeile, die ihn erzeugt hat, also muss die Zeile so lange stehen bleiben.
- **Entfernt** heißt: die Zeile verlässt den Schnappschuss sofort und nimmt den Abgang aus 2.4. Das gilt für den Ablauf des Bestätigungsfensters, für einen Timeout und für eine Fremdentscheidung (Regel, Notification, zweites Fenster). Entfernte Zeilen geben unterhalb des Zeigers nur Platz frei und schieben nichts hinein.

Höchstens drei entschiedene Zeilen ruhen gleichzeitig. Trifft eine vierte Entscheidung ein, geht die älteste sofort, ohne ihr Fenster auszusitzen. Ohne diese Grenze belegt ein schnell abgearbeiteter Schwall die halbe sichtbare Liste mit Belegen für bereits Erledigtes. Ruhende Belege behalten ihre Position; sie sortieren sich weder nach oben noch nach unten, weil das eine Bewegung wäre, die keine der vier Fragen aus 2.1 beantwortet.

### 2.9 Was nie animiert

- **Text, den jemand liest.** URL, Pfad, Header-Tabellen, Body-Ansichten, Regelsatz, Diff. Sie wechseln sofort oder gar nicht. Dieses Programm verlangt, eine URL zu lesen, bevor über sie entschieden wird; bewegter Text kostet eine zweite Fixation.
- **Zahlen.** Countdown, Queue-Zähler, Tray-Zähler, Fenstertitel. Ziffern springen, sie rollen, kippen und skalieren nicht. Die Fläche um eine Zahl darf pulsen (2.2), die Zahl selbst nie.
- **Fokusringe.** Ein Frame. Ein einblendender Ring liest sich als Eingabeverzögerung.
- **Zeilenhöhen.** Eine Zeile wechselt ihre Höhe nicht, weil sich ihr Zustand ändert: 36 px als Mindesthöhe in jedem Zustand, siehe 3.4. Dass dieselbe Zeile bei größerer Textskalierung höher ist, ist keine Animation, sondern Layout.
- **Direkte Manipulation.** Splitter, Scrollen, Textauswahl und Drag-Reorder folgen dem Zeiger eins zu eins. Nur die freigewordene Lücke animiert. Jede Dämpfung zwischen Hand und Pixel liest sich, als widerspräche das Programm der Hand.
- **Hover.** Hover färbt (`HMotion.press`) und deckt Affordanzen in bereits reserviertem Platz auf. Hover verschiebt nichts und ändert keine Größe.
- **Leerlauf.** Es gibt keine Ambient-, Idle- oder Dauerbewegung außer dem begrenzten Atem aus 2.7. Kein Spinner, nirgends (2.11).

### 2.9b Der geteilte Übergang

Ein `Hero` wird nicht erzwungen. Er beantwortet genau eine Frage — „wohin ist das gegangen, was ich eben angesehen habe?" — und gehört deshalb an die zwei Stellen, an denen der Blick sonst springt:

| Von | Nach | Was fliegt |
|---|---|---|
| Zeile in der Queue | Karte im Detail-Pane | der Host-Chip mit seinem Rail-Streifen |
| entschiedene Karte | Zeile in der History | derselbe Chip, sobald beide Screens sichtbar nebeneinander stehen |

Alles andere kommt mit den Tokens aus Abschnitt 2.2 aus. Drei Bedingungen, sonst entfällt der Übergang ersatzlos: Quelle und Ziel sind gleichzeitig auf dem Schirm, das fliegende Element trägt in beiden Zuständen dieselbe Bedeutung, und die Strecke ist kürzer als eine Bildschirmdiagonale. Unter reduzierter Bewegung entfällt er immer; die Ankunft am Ziel zeigt dann derselbe Wisch wie jede andere Auswahl.

Ein geteilter Übergang darf nie das einzige Zeichen für einen Zustandswechsel sein: Wer den Screen erst nach der Animation ansieht, muss dieselbe Aussage noch in Farbe, Glyph und Text finden. Eine Animation, die keine Frage des Nutzers beantwortet, ist ein Fehler, kein Schmuck.

### 2.10 Reduzierte Bewegung

Alle Strecken und alle Schleifen laufen über `HReducedMotion` in `motion.dart`; kein Widget entscheidet für sich, ob es sich bewegen darf. Unter reduzierter Bewegung fällt der Weg weg, nicht die Rückmeldung: `arriveOffset` und `leaveOffset` werden null, während Ausblenden, Tastenfüllung und Rail-Wisch ihre vollen Dauern behalten.

Der Abgang ist der Fall, in dem „weniger Weg" die Rückmeldung versehentlich mitnimmt. Eine Zeile, deren Höhe sofort auf null fällt, hat nichts mehr, was ausblenden könnte; das Ausblenden liefe 220 ms lang über nichts. Unter reduzierter Bewegung behält die gehende Zeile deshalb ihre volle Höhe und blendet an Ort und Stelle über `HMotion.leave` aus; erst danach wird sie in einem Frame entfernt. Der Kollaps entfällt, das Ausblenden bleibt — das ist die Regel, nicht ihre Ausnahme.

Jede Schleife bekommt einen ruhenden Ersatz statt gar nichts: aus dem atmenden Glyph wird ein dauerhaft doppelter Ring bei `HMotion.reducedRingAlpha`. Die 20-%-Schwelle ist Information, und wer Animationen abgeschaltet hat, darf sie nicht verlieren.

### 2.11 Warten

Warten ist ein Zustand wie jeder andere und bekommt deshalb eine Sprache, nicht ein Symbol. Die Regel gilt überall gleich:

- Unter `HMotion.waitVisible` (150 ms) passiert nichts. Eine Anzeige, die kürzer sichtbar ist als eine Reaktionszeit, wird als Flackern gelesen.
- Ab 150 ms behält der Pane sein Layout und zeichnet die Haarlinien-Skelette der erwarteten Zeilen: die Höhe der Zieldichte, `fg2`, keine Bewegung. Das Skelett sagt, wie viel gleich kommt und wo es stehen wird. Ein Spinner sagt nur, dass etwas läuft.
- Was einmal erschienen ist, bleibt mindestens `HMotion.waitMinVisible` (400 ms) stehen. Sonst erzeugt eine Antwort kurz nach der Schwelle genau das Flackern, das die Schwelle verhindern soll.
- Beim Eintreffen wird nichts verschoben: das Skelett wird durch die Zeile ersetzt, die es beschrieben hat, an derselben Stelle, in einem Frame.
- Ein Fehlschlag beim Warten ist ein Fehler und landet nach 4.4 am selben Ort, an dem das Skelett stand — nie als Toast, nie ganzflächig, solange der Rest des Screens gilt.

Wo das im Einzelnen sitzt:

| Wartender Vorgang | Ort | Skelett |
|---|---|---|
| Body wird geladen und geparst (bis 8 MiB, HUM-030) | im Body-Pane, unter dem bereits sichtbaren Umschalter | zehn Zeilen der Zieldichte 24 px; bei `tooLarge` erscheint stattdessen sofort die Karte mit Größe und Findings |
| Dry-Run einer Regel (HUM-033) | im Dry-Run-Panel, nie global | drei Ergebniszeilen 28 px unter der unveränderten Überschrift; der Editor bleibt bedienbar |
| History lädt nach (HUM-032) | unter der letzten geladenen Zeile | so viele 28-px-Skelette, wie die Seite groß ist; die Tabelle springt beim Eintreffen nicht |
| Export (HUM-032) | im Export-Menü, am auslösenden Eintrag | kein Skelett, sondern Fortschritt in Zeilen („1.284 von 5.000"), weil die Zahl bekannt ist |
| Entscheidung unterwegs | in der Zeile und in der Aktionsleiste | keines. Der Rail steht gewischt; das ist die Anzeige (2.5). |
| Daemon antwortet beim Start noch nicht | ganzflächig | Splash nach denselben zwei Schwellen (4.2) |

---

## 3. Hierarchie und Farbe

### 3.1 Das eine Wichtige pro Screen

Ein Screen hat genau ein größtes Textelement, genau ein gefülltes Control und genau einen Leerzustand. Alles andere ist Ghost, getönt oder neutral.

„Gefüllt" heißt: das Control trägt den Akzent als Fläche, nicht als Umriss — entweder als volle Akzentfläche mit `onAccent`-Text (`HButtonVariant.primary`) oder als Akzent-Tönung am Token-Deckel (`HColors.tintAlpha`, 10 %), wie die Release Valve. Alles andere ist Ghost: Rahmen oder Text, keine Fläche.

| Screen | Das eine Wichtige | Größte Type | Einziges gefülltes Control |
|---|---|---|---|
| Intercept | die URL der ausgewählten Anfrage | `mono14` im Kartenkopf | Release Valve |
| History | die gefilterte Liste | `mono14` im Detailkopf | keines; der Akzent gehört dem Filterfeld im Fokus |
| Rules | die Reihenfolge der Regeln | `ui14` für die Regel als Satz | `+ New rule`, solange die Liste leer ist, sonst `Save` im Editor |
| Setup und Sandbox | der Zustand der drei Garantien | `ui16` für das Urteil | die Aktion des obersten Diagnostics |

Die Regel gilt je **Screen**, nicht je Pane. Ein Pane hat kein eigenes Wichtigstes, das mit dem des Screens konkurrieren dürfte: die erste Zeile des Kontext-Panes im Intercept trägt die Antwort „bekannt oder unbekannt" auf `ui14` und bleibt damit unter der URL im Kartenkopf. Auf `ui16` läge sie über dem Gegenstand der Entscheidung, und zwar auch dann, wenn `mono14` und `ui16` sich nicht allein an der Zahl vergleichen lassen — im Zweifel gewinnt die URL, weil der Screen von ihr handelt.

Ein leerer Screen ist die Ausnahme, die die Regel braucht: solange ein Screen keinen Inhalt hat, ist seine primäre Erzeugungsaktion gefüllt, damit überhaupt ein Weg sichtbar ist; sobald er Inhalt hat, wird sie Ghost und die Füllung geht an das Control der Entscheidung. Genauer: **ein gefülltes Control je Entscheidungskontext**. Ein Modal, ein Sheet und der Screen darunter sind drei Kontexte, aber nie gleichzeitig aktiv.

Die Skala fällt heute auf 11/12/13 zusammen: `ui20` steht nur auf Platzhaltern, `ui14` und `mono14` nirgends, und die URL — der Gegenstand der Entscheidung — läuft auf `mono13`. `ui16` und `ui20` gehören ab jetzt dem jeweils Wichtigsten, nicht dem Unfertigsten.

### 3.2 Der Dichte-Rhythmus

- Basis 4 px. Jeder Abstand ist ein Vielfaches davon; `x1/2 = 2 px` gibt es nicht.
- Panel-Padding 12 px. Jede rechte Rinne endet bei 12 px, auch die des Queue-Kopfs, damit Zähler und Countdowns in einer Spalte fluchten.
- Drei Zeilendichten, nicht mehr: 36 px in Queue und Rules, 28 px in der History-Tabelle, 24 px in Body-Ansichten. Alle drei sind Mindesthöhen (Abschnitt 6, Textskalierung), und alle drei brauchen ein Token (Abschnitt 9).
- **Textmaß höchstens 90 Monospace-Zeichen** — für Fließtext und für einzeilige URL- und Pfadfelder. Überschüssige Panebreite wird Rinne, nie Zeilenlänge. Bei 2560 px läuft eine URL sonst über 137 Zeichen, und das Auge sucht den nächsten Zeilenanfang.
- **Code, Hex und Tabellen brechen nie um.** Raw-Body, Hex-Ansicht und die History-Tabelle scrollen waagerecht und bekommen kein Maß. Ein Umbruch erzeugt dort visuelle Zeilen, die die Byte-Offsets der Findings verschieben (HUM-030, Fallstrick „Wrap aus"); eine falsch platzierte Fundstelle ist schlimmer als eine lange Zeile.
- Panes 28/44/28 bis 1800 px Fensterbreite. Darüber friert der Inspector bei Maß plus Rinne ein, Queue und Kontext nehmen den Rest. Unter 1280 px klappt der Kontext-Pane zu und ist über `Ctrl+D` erreichbar.
- Die Naht zwischen zwei Panes gehört dem Splitter. Ein Pane zeichnet keinen linken und keinen rechten Rand; heute stehen an jeder Naht zwei Haarlinien 3 px auseinander.
- Ein Screen zeigt einen Leerzustand, nicht einen pro Pane. Solange der Hauptpane leer ist, bleiben die Nebenpanes still.

Das Maß steht als Zeichenzahl, nicht als Pixelbreite, weil die Breite von der installierten Schrift abhängt. Wer trotzdem rechnet: JetBrains Mono und jeder Fallback der Kette laufen auf rund 0,6 em Vorschub, also rund 700 px bei `mono13` und rund 650 px bei `mono12`; `queue_row.dart` rechnet mit 7,3 px je Zeichen bei 12 px. Diese Zahlen sind Illustration, normativ ist die Zeichenzahl (Abschnitt 9).

### 3.3 Farbe bedeutet Zustand

Der Akzent markiert, was Fokus nehmen oder geklickt werden kann: `HColors.accent` (#7C9CF5) und `HColors.lAccent` (#5B7FE6). Inerter Text ist nie Akzent — eine akzentfarbene URL, die niemand öffnen kann, bringt dem Auge bei, dass Akzent nichts bedeutet, und dann bricht Fokus, Auswahl und Primärbutton gleich mit.

Zustandsfarben stehen nur an Zustand, aus `HStateColors` über `HTheme.of(context).state` bzw. `tokens.stateColor(state)`:

| Zustand | Token | Dunkel | Hell |
|---|---|---|---|
| `held` | `HColors.held` | #E0B24A | abgeleitet, `HStateColors.light` |
| `allowed` | `HColors.allowed` | #4FBF8C | abgeleitet |
| `allowedEdited` | `HColors.allowedEdited` | #57B99F plus Akzentpunkt | abgeleitet |
| `blocked` | `HColors.blocked` | #E5646E | abgeleitet |
| `timedOut` | `HColors.timedOut` | #8A90A2 | abgeleitet |
| `autoRule` | `HColors.autoRule` | `allowed` bei 60 % | abgeleitet |
| `passthroughLlm` | `HColors.passthrough` | #B48AF0 | abgeleitet |
| `error` und Secret | `HColors.secret` | #F0784F | abgeleitet |

Regeln dazu:

1. **Volle Sättigung nur, wo zwei Zustände nebeneinander stehen.** In der Intercept-Queue steht per Konstruktion nur `held`; fünfzehn volle Amber-Rails wiederholen eine Tatsache und werden das Lauteste auf dem Schirm, während die eigentliche Entscheidung neutral bleibt. Dort trägt die Rail `tokens.tint(tokens.stateColor(state))` (10 % Alpha). Im Augenblick der Entscheidung wischt sie auf volle Sättigung — dann ist sie die einzige gesättigte Fläche im Pane. History behält volle Sättigung, weil dort acht Zustände gemischt stehen.
2. **Farbe ist nie der einzige Kanal.** Jede Zeile, die Zustand trägt, trägt Glyph und Farbe. Im hellen Theme messen `allowed` und `blocked` unter Deuteranopie 1,01:1; eine 4 px breite Farbe allein sagt dort nichts. Die Glyph-Tabelle existiert bereits in `flow_state.dart` und wird bisher nicht benutzt.
3. **Kein Farbwechsel durch Interpolation.** `held` #E0B24A nach `blocked` #E5646E gelerpt liefe durch das Orange, das `error` und `secret` gehört, und zeigte einen Zustand, den es nie gab. Deshalb wischt der Rail, statt zu überblenden.
4. **Method-Badges sind in Listen neutral** (`fg1` auf `bg2`). Die Methoden-Hues borgen sich Zustandsfarben (PUT/PATCH ist `held`, DELETE ist `blocked` bei 70 %, POST ist `passthrough`); in einer Liste liest das Auge ein rötliches Badge neben einer roten Rail als zwei Blöcke, nicht als ein Verb und einen Zustand. Hue bekommt nur das eine Badge im Kartenkopf.
5. **Chrome, das sich nicht ändern kann, trägt `fg1` oder `fg2`.** Ein Zustandsanzeiger hat mindestens zwei Werte, sonst ist er keiner. „Intercept an" und der Verbindungspunkt sind heute immer grün, weil die Shell nur im verbundenen Zustand baut; zwei dauerhaft grüne Marken bringen dem Auge bei, Grün zu übersehen, und Grün ist die Farbe der einen Handlung, auf die es ankommt. Entweder verdrahten oder entfernen.
6. **Rot heißt blockiert.** Fehler und Findings sind Orange (`HColors.secret`). Rot als Ambiente gibt es nicht.
7. **In Body-Ansichten sind Findings die einzige Chroma.** JSON-Schlüssel, Werte und Hex bleiben auf der `fg`-Leiter. Ein Body ist der Ort, an dem ein Secret in Sekunden auffallen muss; drei Farben für Datentypen lassen die Fundstelle mit der Syntax konkurrieren. Das weicht von HUM-030 ab, siehe Abschnitt 8.
8. **Ein Control, das Zustand zählt, ist trotzdem Akzent.** `+n neu`, der Findings-Chip, der Gruppenzähler und der Batch-Button tragen alle eine Zahl über gehaltene Anfragen — und sie bleiben Akzent, weil man sie anfassen kann. Der Zustand, den sie zählen, steht als Glyph darin, nie als Füllung. Sonst zerfällt der Akzent in so viele Bedeutungen, wie es Zähler gibt.
9. **Was nur anzeigt und nichts auslöst, trägt Zustandsfarbe.** Das Tray-Icon ist kein Control, sondern der einzige Zustandsanzeiger der Sitzung, solange das Fenster nicht sichtbar ist; sein Zähler steht deshalb in `held`-Amber, nicht im Akzent (Abweichung von HUM-034, siehe Abschnitt 8). Regel 8 und Regel 9 zusammen: Fläche zum Anfassen ist Akzent, Fläche zum Ablesen ist Zustand.
10. **Die ruhende `held`-Rail ist von der 3:1-Regel ausgenommen.** Eine 10-%-Tönung von `held` auf `bg1` misst gemessene 1,19:1; auf 3:1 käme sie erst bei knapp 50 % Alpha, und bei 50 % ist sie keine Tönung mehr, sondern die gesättigte Fläche, die Regel 1 der Entscheidung vorbehält. Die Rail gruppiert dort nur; Zustand und Frist tragen Glyph, Label und Semantik. Die Ausnahme gilt namentlich für die ruhende Zustands-Rail in der Queue und für nichts sonst — die gewischte Rail, die Auswahl-Rail und jede Rail in History erreichen 3:1. `HColorDerivation.tint` deckelt bei `HColors.tintAlpha`; eine stärkere Rail wäre kein Aufruf, sondern ein neues Token (Abschnitt 9).

### 3.4 Die Queue-Zeile, ausgeschrieben

Weil hier die meisten Regeln zusammenlaufen, steht sie einmal vollständig da:

```
| 4 Rail | 8 | 16 Zustands-Glyph mit Countdown-Ring | 8 | Method neutral | 8 |
  Host ui13/500 | 8 | Pfad mono12, mittig gekürzt | 8 | Findings-Chip | 8 | Aktionsslot 28 | 12 |
```

- **Höhe: mindestens 36 px, in jedem Zustand, ohne zweite Zeile.** Eine Auswahl, die von 36 auf 56 px wächst, schiebt bei jedem `J` alles darunter um 20 px; die zweite Zeile wiederholt außerdem Größe, Content-Type und Wartezeit, die die Karte zwei Panes weiter ohnehin zeigt. 36 px ist eine Mindesthöhe und keine feste: bei `TextScaler.linear(2.0)` misst `ui13` allein 40 px Zeilenhöhe, und eine feste Höhe schluckte den Overflow still. Die Zeile wächst also mit der Skalierung, aber nie mit ihrem Zustand.
- **Was wann weicht.** Feste Breite verbrauchen 108 px Chrome plus Method-Badge (GET rund 36 px, DELETE rund 56 px) plus Findings-Chip (rund 24 px); bei `HSize.paneMinQueue` (280 px) bleiben für Host und Pfad zusammen rund 100 px. Deshalb steht die Reihenfolge, in der gekürzt wird, hier und nicht im Ermessen des Layouts: der Host bekommt eine Untergrenze von 12 em und wird darüber hinaus mittig gekürzt; der Pfad gibt zuerst nach und verschwindet unter rund 360 px Panebreite ganz; der Findings-Chip fällt unter rund 320 px auf einen 4-px-Punkt in `HColors.secret` zusammen. Der Host ist das Letzte, was geht — er ist die Antwort auf „wohin".
- **Die Rail:** `held` als Tönung, entschieden in voller Farbe, ausgewählt in Akzent über die vollen 4 px. Die Auswahl **ersetzt** die Zustands-Rail, sie überlagert nicht ihre linke Hälfte; das steht so bereits in der Doku von `spacing.dart` und funktioniert, sobald die Zeile ihr Glyph trägt.
- **Die Füllung:** Hover `bg2`, Auswahl `bg3`. Nie dieselbe Farbe. `Enter` und `A` erlauben die ausgewählte Zeile, und Erlauben ist unumkehrbar; wenn eine überfahrene Zeile genauso aussieht wie die ausgewählte, liest jemand die eine und sendet die andere. `bg2` steht für genau diese Zwischenstufe in der Leiter und wird in Zeilen bisher nicht benutzt.
- **Kein `mm:ss` in der Zeile.** Die verbleibende Zeit steht im Bogen des Rings und im Semantics-Value; die Ziffern gehören der ausgewählten Anfrage in der Karte. Fünfzehn gleichzeitig laufende Ziffern sind das Gegenteil eines ruhigen Kontrollraums.
- **Der Aktionsslot** ist immer 28 px breit und bei Ruhe leer. Hover **und** Fokus blenden dort das Blockieren ein; er ersetzt nichts, verschiebt nichts und lässt keinen Text neu umbrechen. Heute tauscht die Zeile bei Hover rund 33 px Countdown gegen rund 97 px Aktionen, bricht dabei die Titelzeile neu um und stellt „Erlauben" direkt unter den Zeiger.
- **Aus der Zeile heraus wird nur blockiert, nie erlaubt**, und Blockieren verlangt 250 ms Halten. Erlauben ist unumkehrbar und braucht die URL; die steht in der Karte. Das hält zugleich die Regel ein, dass Erlauben und Blockieren nie nebeneinander stehen — im schmalsten Pane wären 280 px sonst zu wenig für einen ehrlichen Abstand.
- **Der Bestätigungsstreifen ist die Zeile, nicht ein Kasten darüber.** Während des Bestätigungsfensters (`HMotion.confirm`) tauscht die entschiedene Zeile ihren Inhalt gegen den Streifentext, auf derselben Grundlinie, in derselben Höhe: Rail in voller Zustandsfarbe, Zustands-Glyph voll gesättigt links, dann der Satz in `mono12`. Keine zweite Zeile, kein zusätzlicher Kasten, keine Höhenänderung, kein neuer Umbruch. Danach geht die Zeile nach 2.4.

### 3.5 Auswahl, Mehrfachauswahl, Gruppenkopf

Die Mehrfachauswahl ist zugleich eine Einfrierbedingung (2.8) und eine Schutzstufe (5.4). Sie braucht deshalb eine eigene, eindeutige Erscheinung, und sie bekommt sie über die Rail, nicht über die Füllung:

- **Die Rail sagt, was ausgewählt ist.** Jedes Mitglied einer Auswahl trägt die Akzent-Rail über die vollen 4 px, ob eines oder zwölf.
- **Die Füllung sagt, wo der Cursor steht.** Hover `bg2`, die zuletzt angeklickte oder mit `J`/`K` erreichte Zeile `bg3` — genau eine Zeile im ganzen Pane. Ein Mitglied ohne Cursor bleibt ungefüllt; die Rail allein trägt die Mitgliedschaft.
- **Es erscheint kein zweites gefülltes Control.** Bei einer Auswahl größer eins beschriftet sich die Release Valve um („Allow 5 selected" / „5 ausgewählte senden") und bleibt das eine gefüllte Control des Screens (3.1). Ein zusätzlicher Batch-Button neben ihr wäre der zweite.

Der Gruppenkopf ist eine Zeile und hält die Zeilenregel ein: er zeigt im 28-px-Aktionsslot nur `Block {n}`. `Allow {n}` steht dort nicht — dieselbe Begründung wie in 3.4, verstärkt um den Faktor n. Sobald eine Gruppe ausgewählt ist, übernimmt die Aktionsleiste das Erlauben, und die Karte daneben zeigt, worüber entschieden wird: Host, Methodenmix, Pfade, Findings-Summe. Das weicht von HUM-029 ab, siehe Abschnitt 8.

---

## 4. Den Menschen anleiten

Prinzip 7 aus `BACKLOG.md` gilt wörtlich: kein Zustand ohne Klartext-Grund, kein Fehler ohne „Warum" und „Was jetzt". Alle Texte kommen aus ARB, keiner steht im Code. Englisch ist die Quellsprache, Deutsch die Übersetzung; jeder Satz, den dieses Dokument festlegt, steht deshalb in beiden Sprachen, Englisch zuerst, in der Reihenfolge, in der die ARB-Dateien ihn tragen.

### 4.1 Leerzustände

Es gibt zwei Leerzustände, und sie verwechseln heißt, dem Menschen eine Zukunft zu versprechen, die es nicht gibt.

**Leer, weil noch nichts passiert ist.** Der Satz nennt das nächste Ereignis, nie die Abwesenheit: „`<Subjekt>` erscheint hier, wenn `<Auslöser>`". Die Wörter „Nichts", „keine" und „noch" kommen darin nicht vor. Ein leerer Screen ist der am längsten sichtbare Screen, den ein neuer Nutzer sieht, und die einzige Lehrfläche, die das Programm geschenkt bekommt.

**Leer, weil ein Filter nichts trifft.** Hier gibt es keine kommende Zukunft; die Menge ist da und der Filter schneidet sie weg. Der Satz nennt den Filter, die Trefferzahl und den Rückweg, und der Rückweg ist ein Akzent-Control, kein Satzteil: en „`host:foo` matches 0 of 1,284 requests · Reset filter", de „`host:foo` trifft 0 von 1.284 Anfragen · Filter zurücksetzen". Das gilt in History und in Rules.

**Leer als Tatsache über ein fertiges Objekt** ist keines von beidem. Eine Anfrage ohne Query-Parameter bekommt nie welche; dort ist „No parameters." / „Keine Parameter." richtig und der Verbotswortkatalog gilt nicht. Die Unterscheidung ist prüfbar: gibt es ein Ereignis, das die Fläche füllen würde, ist es ein Leerzustand; gibt es keines, ist es eine Aussage.

Die leere Queue zeigt zusätzlich die drei Tasten, einmal und nie wieder. Die Reihenfolge beginnt mit der umkehrbaren Taste, und `Enter` nennt seine Folge: en „`B` block · `Enter` send (final) · `J`/`K` move", de „`B` blockieren · `Enter` senden (endgültig) · `J`/`K` bewegen". Eine Lehrfläche, die mit der unumkehrbaren Taste anfängt, bringt genau das Falsche zuerst bei.

Heute verstoßen fünf ausgelieferte Schlüssel gegen die erste Formel, und sie werden mit dem Screen ersetzt, der sie benutzt:

| Schlüssel | heute | statt dessen |
|---|---|---|
| `shellSectionPlaceholder` | „Nothing here yet." / „Hier ist noch nichts." | der Screen sagt, was er tun wird, oder er fehlt (4.2, letzter Absatz) |
| `interceptEmptyTitle` | „No request is waiting" / „Keine Anfrage wartet" | en „The queue is open", de „Die Schleuse ist offen" |
| `interceptEmptyHint` | „The agent works without a network." / „Der Agent arbeitet ohne Netz." | en „Every request the agent makes appears here.", de „Jede Anfrage des Agenten erscheint hier." — „ohne Netz" liest sich als „ohne Sicherheitsnetz" und behauptet außerdem etwas Falsches, solange keine Sitzung läuft (4.2) |
| `interceptCardEmptyTitle` | „Nothing selected" / „Nichts ausgewählt" | en „Pick a request", de „Eine Anfrage wählen" |
| `shellIsolationUnknown` | „Isolation: not checked yet" / „Isolation: noch nicht geprüft" | en „Isolation: check running", de „Isolation: Prüfung läuft" — und wenn keine läuft, nennt der Satz die Aktion, die eine startet |

`interceptQueryEmpty`, `interceptHeadersEmpty` und `interceptBodyEmpty` bleiben, wie sie sind: sie sind Aussagen über eine fertige Anfrage. Damit die Verbotswörter nicht mit dem nächsten Screen zurückkommen, prüft ein ARB-Lint sie in beiden Sprachen und kennt diese drei Ausnahmen namentlich (Abschnitt 9).

### 4.2 Vor der ersten Anfrage

Der erste Bildschirm ist nicht die leere Queue, sondern die Frage, ob überhaupt jemand antwortet. Vier Zustände, jeder mit Text, Ort und genau einer Aktion:

1. **Der Daemon antwortet noch nicht.** Splash, ganzflächig, nach den Schwellen aus 2.11: unter `HMotion.waitVisible` nichts, danach sichtbar und mindestens `HMotion.waitMinVisible` stehend. Kein Spinner, kein Fortschrittsbalken für etwas, dessen Dauer niemand kennt. Ein Splash, der bei einem schnellen Start aufblitzt, ist schlimmer als kein Splash.
2. **Der Daemon fehlt beim Start.** Setup-Screen. Das `Diagnostic` ist das eine Wichtige des Screens (3.1): `why` aus dem Daemon oder aus dem Transportfehler, `fix` als gefülltes Control. Nichts anderes auf diesem Screen konkurriert darum.
3. **Der Daemon läuft, aber keine Sitzung.** Die Queue sagt nicht, der Agent arbeite ohne Netz — es arbeitet niemand. Sie nennt das nächste Ereignis, das der Mensch auslöst: en „Once a session runs, every request the agent makes appears here.", de „Sobald eine Sitzung läuft, erscheint hier jede Anfrage des Agenten." Die einzige Aktion ist en „Start session" / de „Sitzung starten", und sie ist gefüllt, weil der Screen leer ist (3.1).
4. **Die Verbindung bricht während laufender Queue ab.** Die Shell bleibt stehen. Der Setup-Screen ersetzt sie nur beim Kaltstart, nie mitten in der Arbeit — wer zwölf wartende Anfragen auf dem Schirm hat, verliert sonst den Bildschirm und erfährt nicht, was mit dem Agenten passiert ist. Statt dessen: die Queue wird als eingefrorener Schnappschuss markiert (Zeitstempel im Kopf, alle Countdowns stehen, alle Entscheidungstasten still), darüber ein Banner mit Grund, Folge für den Agenten und der einen Aktion en „Reconnect" / de „Erneut verbinden".

Sobald Verkehr da ist, aber nichts ausgewählt, zeigt der Kontext-Pane die Zusammenfassung der Sitzung, die seine Sektion besitzt (gesehene Hosts, die häufigsten fünf, Findings gesamt) — ein Pane, dessen einziger Inhalt seine eigene Überschrift ist, liest sich als kaputt.

Internes Vokabular steht nie auf dem Schirm: keine Sprintnummern, keine Issue-Kennungen, kein Wort „Platzhalter". Ein Control, das noch nichts tut, sagt, was es tun wird, oder fehlt. Zielgruppe sind Designer und Berater; „Ab Sprint 2 verfügbar" sagt ihnen, dass das Produkt unfertig ist, nicht, was es kann.

### 4.3 Warum diese Anfrage wartet

Der Satz wird aus dem Haltegrund des Flows gebaut, nicht aus einer Konstanten:

| Grund | en | de |
|---|---|---|
| keine Regel traf zu | „Held: no rule matches · default: ask" | „Angehalten: keine Regel trifft zu · Vorgabe: fragen" |
| eine Regel sagt fragen | „Held: rule `<Regelsatz>` says ask" | „Angehalten: Regel `<Regelsatz>` sagt fragen" |
| ein Finding | „Held: an AWS access key was found in the body" | „Angehalten: im Body steht ein AWS-Zugangsschlüssel" |
| unbekannt | benennt, was bekannt ist, und erfindet keinen Grund | dito |

Der `kind`-Bezeichner des Findings steht nie im Satz. Er ist internes Vokabular (4.2) und gehört in die Detailzeile und in den Semantics-Value; der Satz nennt den übersetzten Namen und den Fundort.

Heute steht dort für jeden Flow derselbe Text, zentriert zwischen Blockieren und Erlauben, an der aufmerksamkeitsstärksten Stelle des Screens — und er wird zur Lüge, sobald HUM-022 echte `ask`-Regeln liefert. Aus dieser Mitte gehört Wechselndes: der Grund und die verbleibende Zeit mit ihrer Folge benannt — en „auto-blocks in 1:47", de „wird in 1:47 blockiert" —, nie ein blankes `mm:ss`, dessen Richtung und Ausgang man erraten muss, und nie ein Partizip („blockiert in 1:47"), das offenlässt, ob es schon geschehen ist.

### 4.4 Diagnostics

Jeder Fehlerpfad liefert ein `Diagnostic`, nie einen nackten String.

- Der `why`-Slot trägt den Satz des Daemons. Der generische Text der App ist der Titel, nicht der Grund. Heute steht der echte Grund in der Mono-Detailzeile und im `why` eine Konstante; damit sind beide Hälften von Prinzip 7 auf einmal gebrochen.
- `fix` wird immer übergeben, wenn das `Diagnostic` eine `FixAction` trägt. Ein `Diagnostic` mit `FixAction` und ohne sichtbare Aktion ist ein Defekt.
- Verankert wird am Ort des Fehlschlags: inline unter dem Control bei einer einzelnen Entscheidung, als Banner über der Liste mit `Reload` bei einem Datenfehler, an der Stelle des Skeletts bei einem fehlgeschlagenen Wartevorgang (2.11), ganzflächig nur, wenn gar nichts angezeigt werden kann. Abstand zwischen Fehler und Ursache ist der Grund, warum Menschen die fehlgeschlagene Aktion wiederholen, statt sie zu beheben.
- Ein Doku-Link ist Akzent, wenn er klickbar ist. Eine URL, die niemand öffnen kann, ist `fg1` mono mit einem Kopierknopf daneben.

### 4.5 Was „Rückgängig" heißt

„Rückgängig" macht immer die Regel rückgängig, nie die Anfrage. Der Streifen liest „Regel gespeichert · Rückgängig" für `HMotion.undoWindow` (10 s) und sagt im selben Atemzug, dass die Anfrage bereits draußen ist. Danach verschwindet nur der Streifen; die Regel bleibt im Rules-Screen löschbar. Für Erlauben und Blockieren selbst gibt es kein Rückgängig — deshalb der Halteschutz und die Armierung aus 5.4. Ein Rückgängig, dessen Reichweite jemand falsch rät, ist schlimmer als keines, weil er sich beim nächsten Schwall darauf verlässt.

### 4.6 Bewegung und Text sagen dasselbe

Der Streifen, der in der entschiedenen Zeile steht, nennt die Folge, nicht die Handlung: en „Sent to api.github.com · 2.1 KB", de „Gesendet an api.github.com · 2,1 KB". Blockiert liest en „Blocked. The agent may retry.", de „Blockiert. Der Agent kann es erneut versuchen." Kein Toast begleitet eine Bewegung; Toast plus Bewegung sind zwei Ankündigungen desselben Ereignisses.

Nach dem Bestätigungsfenster ist der Streifen weg, aber die Handlung war unumkehrbar. Die Statusleiste trägt deshalb die letzte Entscheidung dauerhaft als eine Zeile — Zustands-Glyph, Host, Uhrzeit — bis die nächste sie ersetzt. Drei Sekunden sind zu kurz für die einzige Spur einer Handlung, die 4.5 nicht zurücknehmen kann.

**Die Verben.** Drei Orte, drei Formulierungen, und sie werden nicht vermischt:

| Ort | en | de | warum |
|---|---|---|---|
| Control | „Allow" | „Senden" | Das Control nennt die Handlung am Objekt. Der deutsche Nutzer sendet eine Anfrage, er erlaubt sie nicht. |
| Regel | „allow" | „Erlauben" | Die Regel nennt die Politik, nicht die Einzelhandlung; sie gilt für alles Künftige. |
| Streifen | „Sent to {host} · {size}" | „Gesendet an {host} · {size}" | Der Streifen nennt die Folge, im Perfekt, mit Ziel und Umfang. |

Damit steht fest, dass auf dem deutschen Button nicht „Erlauben" steht, auch wenn HUM-028 den Regelsatz mit „Erlauben · ∗ · …" beginnt (Abweichung in Abschnitt 8). `BACKLOG.md` Abschnitt 5 legt diesen Split bereits fest; hier steht, warum er kein Fehler ist.

Vor „Merken" steht die Regel als Satz, damit sie geprüft werden kann, die Vorgabedauer ist `session`, und ein Control mit der Aufschrift „Allow all" existiert nur als Palette-Befehl, der eine Host-Liste öffnet.

### 4.7 Wenn ein Secret mitgeht

Eine Anfrage, die wegen eines Findings gehalten wird, ist der eine Fall, der nicht ruhig sein darf. Sie darf nicht aussehen wie eine Routine-Anfrage, und sie bekommt deshalb drei Abweichungen vom Normalfall — und nur diese drei:

- **In der Zeile** steht der Findings-Chip in `HColors.secret` statt in `fg2`. Das ist die einzige Chroma, die eine ruhende Queue-Zeile tragen darf; der Rest der Zeile bleibt neutral.
- **In der Aktionsleiste** nennt der Haltegrund Art und Fundort in Klartext (4.3), und die Release Valve wechselt auf Amber und beschriftet sich um: en „Send with 2 findings", de „Senden mit 2 Findings" (so bereits in HUM-049 vorgesehen).
- **Erlauben verlangt dieselbe Halte-Bestätigung wie Blockieren**, solange mindestens ein Finding ungelöst ist, plus einen Satz, der die Folge benennt: en „An AWS access key goes to api.example.com", de „Ein AWS-Zugangsschlüssel geht an api.example.com". Erst wer weiß, was wohin geht, hält gedrückt.

Ausdrücklich **kein** Modal. Die Regel „nie ein Modal für eine einzelne Entscheidung" (5.4) gilt hier weiter; ein Modal wäre auch der falsche Schutz, weil es sich wegklicken lässt, ohne den Satz zu lesen. Der Schutz sitzt in der Zeit, nicht in der Fläche (5.4). Abweichung in Abschnitt 8.

### 4.8 Wenn die Zeit abläuft

Ein Timeout ist keine Entscheidung eines Menschen, sondern das Ausbleiben einer. Er ist deshalb das Ereignis, das die meiste Spur hinterlassen muss, und heute hinterlässt er die wenigste.

- **Vorher** warnt die App einmal, an der Zeile und in der Aktionsleiste des betroffenen Flows, nie als Toast: Host, Folge und Restzeit in einem Satz — en „registry.npmjs.org auto-blocks in 0:30", de „registry.npmjs.org wird in 0:30 blockiert". Sobald der Daemon Verlängern kennt, steht daneben genau eine Verlängerung. Bis dahin ist die Warnung mit benannter Folge das Minimum.
- **Im Augenblick des Ablaufs** wischt der Rail auf `timedOut` wie bei jeder anderen Entscheidung, und die Zeile bleibt drei Sekunden als graue Zeile mit dem Streifen en „Blocked (timed out)" / de „Blockiert (Zeit abgelaufen)" stehen. So steht es bereits in HUM-029; hier steht, dass es dieselbe Choreografie ist wie bei einer menschlichen Entscheidung, weil die Folge für den Agenten dieselbe ist.
- **Danach** trägt die Statusleiste dauerhaft einen Zähler: en „{n} timed out this session", de „{n} abgelaufen in dieser Sitzung". Ein Klick filtert History darauf. Wer fünf Minuten weg war, muss beim Zurückkommen sehen, dass sechs Anfragen abgewiesen wurden, ohne History zu öffnen und ohne sich an ein Banner zu erinnern, das er nie gesehen hat.

### 4.9 Tray, Notification und Rückkehr

Das ist die einzige Oberfläche, die der Mensch sieht, während der Agent hängt. Sie bekommt deshalb eigene Regeln und nicht eine Zeile am Rand.

**Die Notification** hat dieselbe Hierarchie wie ein Screen (3.1): der Host ist das eine Wichtige und steht groß; Methode und Pfad sind sekundär; die Restzeit steht als Wort, nie als `mm:ss` — en „about 4 minutes left", de „noch etwa 4 Minuten". Eine Notification ist ein Standbild in einem fremden Fenstermanager; eine Ziffer darin ist ab der ersten Sekunde falsch. Bei fokussiertem Fenster erscheint keine Notification: was auf dem Schirm steht, wird nicht zusätzlich angekündigt (dieselbe Regel wie Toast plus Bewegung in 4.6). Mehrere Ankünfte aktualisieren dieselbe Meldung, sie stapeln nicht.

**Das Tray-Icon** ist ein Zustandsanzeiger und trägt Zustandsfarbe (3.3, Regel 9):

| Zustand | Icon | Farbe |
|---|---|---|
| nichts wartet | `idle` | `fg2`, neutral |
| 1 bis 9 warten, 10 und mehr | `held_n`, `held_9plus` | `held`-Amber mit Ziffer |
| seit dem letzten Fokus hat ein Timeout blockiert | `alert` | `blocked` |

**Die Rückkehr** spielt nichts nach. Wer nach fünfzehn Ankünften zurückkommt, sieht die Queue, wie sie ist, ohne Ankunftsanimationen für Dinge, die vor Minuten passiert sind — eine Animation beantwortet „was ist gerade angekommen", und nichts davon ist gerade angekommen (2.1). Was in der Abwesenheit geschah, steht in zwei ruhigen Formen: das Rückkehr-Banner nennt die längste Wartezeit und führt zum ältesten Flow, der Timeout-Zähler in der Statusleiste nennt die Abweisungen (4.8).

---

## 5. Tastatur und Maus

### 5.1 Parität

- Jede Zeigergeste hat eine Tastenentsprechung. Auch Press-and-hold: die Release Valve ist über eine Taste oder eine zweistufige Bestätigung erreichbar, sonst ist das Signature-Element ohne Maus unbenutzbar. Auch die Pille „+n neu" (2.8).
- Jedes Element, das auf Tippen reagiert, ist mit `Tab` erreichbar und mit `Enter` und `Leertaste` auslösbar. Ein nacktes `GestureDetector` oder `MouseRegion` ist nie der einzige Eingabeweg.
- Nichts, was eine Aktion ausführt, erscheint nur bei Hover. Zeilenaktionen erscheinen auch bei Fokus und bleiben im Semantics-Baum, wenn sie unsichtbar sind.
- Der Shortcut steht auf dem Control, das ihn ausführt: dauerhaft auf den beiden Hauptentscheidungen, bei Hover auf allem anderen, nach dem `HoverLabel`-Muster der Icon-Rail. Ein Mausnutzer kann `Enter`, `A`, `B`, `J` und `K` sonst nicht durch Klicken entdecken.
- Die Command Palette ist das Handbuch: sie listet jede über Tastatur erreichbare Aktion mit ihrem Shortcut in der Zeile, nicht nur die fünf Navigationseinträge.

### 5.2 Wem die Taste gehört

Kein `Shortcuts`-Widget oberhalb fokussierbarer Controls darf `Enter`, `Leertaste` oder einen einzelnen Buchstaben so binden, dass das fokussierte Control leer ausgeht. Heute erlaubt ein Tastaturnutzer, der auf „Blockieren" fokussiert und `Enter` drückt, die Anfrage — mit `Leertaste` blockiert dasselbe Control korrekt, und die falsche der beiden Tasten ist die, nach der jeder Desktop-Nutzer zuerst greift.

Der Mechanismus, der das behebt, ist genau einer, und er steht hier ausgeschrieben, weil der naheliegende nicht funktioniert. `Action.overridable` hilft nicht: eine überschreibbare Action wird vom nächsten **Vorfahren** überschrieben, nie von einem fokussierten Nachfahren, und die Kollision entsteht ohnehin früher — `Shortcuts` bildet `Enter` auf `AllowIntent` ab, bevor irgendeine Action-Suche läuft.

Statt dessen: die `AllowIntent`- und `BlockIntent`-Actions des Screens überschreiben `isActionEnabled` und liefern `false`, solange `FocusManager.instance.primaryFocus` auf einem Control sitzt, das `ActivateIntent` behandelt. `ShortcutManager.handleKeypress` gibt für eine deaktivierte Action `KeyEventResult.ignored` zurück; die Taste fällt damit an die Standardbindungen von `WidgetsApp` durch, wo `Enter` `ActivateIntent` auslöst, und das fokussierte Control gewinnt. Die billigere Variante, falls sich das im Test als brüchig erweist: `Enter` gar nicht auf Screen-Ebene binden, nur `A` und `B`, und `Enter` überall „aktivieren" bedeuten lassen.

Die Queue ist ein einziger Fokusstopp mit Pfeiltasten-Navigation darin. Solange der Fokus in der Queue steht, gilt `Enter` = erlauben; steht er auf einem Control, gilt `Enter` für dieses Control. Das widerspricht nicht der Regel aus 3.4, dass aus der Zeile heraus nur blockiert wird: die Zeile ist nicht das Control, die **Auswahl** ist der Gegenstand. `Enter` erlaubt den ausgewählten Flow, dessen URL die Karte zeigt, und die Zeile selbst trägt keine Erlauben-Affordanz, die man mit dem Zeiger treffen könnte.

### 5.3 Jede gebundene Taste tut etwas

Jeder Aktivator in `interceptShortcuts()` hat eine Action im selben Screen. Ein Widget-Test vergleicht die beiden Tastenmengen und schlägt bei Ungleichheit fehl; eine Bindung ohne Action wird gelöscht, nicht stillgelegt. Heute sind `/`, `Ctrl+D` und `N` gebunden und stumm.

Jeder angenommene Tastendruck erzeugt einen sichtbaren Frame innerhalb von 100 ms; die Tastenentscheidung zeigt dieselbe 120-ms-Füllung auf dem passenden Control der Aktionsleiste wie ein Mausklick. Ein abgelehnter Druck bekommt seine eigene, leise Ablehnung — das Control, das gehandelt hätte, zeigt seine Füllung und läuft wieder leer, und der Grund steht daneben. Nie Stille, nie ein Rütteln, nie ein rotes Blitzen. Für einen blinden Nutzer wäre eine rein sichtbare Ablehnung eine stumme; die Ablehnung wird deshalb höflich angesagt (Abschnitt 6). Heute liefert ein zweites `A` während einer laufenden Entscheidung gar nichts zurück, während gleichzeitig die Ghost-Buttons verschwinden, und ein schnelles `A A` liest sich, als sei das Programm eingefroren.

### 5.4 Sorgfalt wächst mit Reichweite und Umkehrbarkeit

Gefahr ist kein Maß: niemand weiß vorab, ob ein `POST` an einen unbekannten Host schlimm ist. Umkehrbarkeit ist eines. Deshalb hat der Schutz zwei Achsen, und deshalb ist Blockieren Ghost, obwohl es die scheinbar härtere Handlung ist — der Agent darf es erneut versuchen. Erlauben ist gefüllt, weil es die Handlung ist, um die es geht; sein Schutz sitzt in der Zeit, nicht in der Fläche.

**Reichweite:**

| Reichweite | Schutz |
|---|---|
| eine Anfrage | 250 ms Halten (`HMotion.holdToBlock`) |
| zwei bis fünf Anfragen | 250 ms Halten |
| mehr als fünf, oder eine Regel mit `forever` | Modal |

Nie ein Modal für eine einzelne Entscheidung: Modal pro Klick zerstört den Rhythmus der Queue. Ein Modal fängt den Fokus in sich, fokussiert die nicht-zerstörerische Aktion, schließt auf `Escape` und legt die Entscheidungs-Shortcuts des Screens still, solange es offen ist.

**Umkehrbarkeit.** Erlauben ist unumkehrbar (4.5), `Enter` ist eine einzelne Taste, und die Auswahl springt nach jeder Entscheidung auf den nächsten gehaltenen Flow. Drei Festlegungen halten das auseinander:

1. **Armierung.** `Enter` und `A` feuern erst, wenn die URL des ausgewählten Flows mindestens `HMotion.rearm` (350 ms) in der Karte stand. Jede neue Auswahl armiert neu — auch die, die das Programm selbst nach einer Entscheidung setzt. Blockieren bleibt sofort verfügbar. Eine Sakkade auf eine neue Zeile plus eine Fixation dauert rund 300 ms; wer schneller entscheidet, hat nicht gelesen.
2. **Tastenwiederholung entscheidet nie.** Ein `KeyDownEvent` mit `repeat == true` wird verworfen. Nach einer angenommenen Entscheidung bleiben `Enter`, `A` und `B` gesperrt, bis das zugehörige `KeyUpEvent` eingetroffen ist.
3. **Die Auswahl wartet.** Sie springt nach einer Entscheidung nicht weiter, solange eine Entscheidungstaste noch gedrückt ist. Sonst armiert Regel 1 gegen einen Finger, der gar nicht losgelassen hat.

Pflichttest: `Enter` 500 ms gedrückt halten erzeugt genau ein `Decide`, nicht mehrere. Ohne diese drei Sätze erlaubt gedrückt gehaltenes `Enter` die halbe Queue ungelesen — heute ist der einzige Schutz `isSending`, und der endet mit der Antwort des Daemons nach wenigen Millisekunden.

Die beiden Entscheidungen sind außerdem größer als alles andere: `HSize.hitMin` (28 px) ist die Untergrenze für Nebensächliches, Erlauben und Blockieren messen mindestens 32 px Höhe und 120 px Breite, wie HUM-028 es für die Release Valve ohnehin nennt. Ein 28-px-Ziel neben einem anderen 28-px-Ziel ist genau die Geometrie, in der ein hastiger Klick daneben geht — und daneben liegt hier die unumkehrbare Handlung.

Das Notizfeld beim Blockieren (HUM-072) ist vorübergehend: verborgen bis `N`, geschlossen bei `Escape` oder bei einer Entscheidung, nie eine dauerhafte Zeile der Aktionsleiste. Ein dauerhaft sichtbares optionales Textfeld fügt der Leiste einen Fokusstopp, einen Rahmen und einen Zeichenzähler hinzu und lädt zum Tippen ein, wo der Normalweg ein Tastendruck ist.

---

## 6. Barrierefreiheit als Zahlen

- **Kontrast.** Text erreicht 4,5:1 gegen die Fläche, auf der er wirklich steht — also auch gegen einen 10-%-Tint und gegen eine Füllung, auf allen vier Flächen beider Leitern. Flächen und Rails erreichen 3:1, mit der einen benannten Ausnahme der ruhenden `held`-Rail (3.3, Regel 10). Der Test in `app/packages/ui/test/tokens_test.dart` prüft heute nur 3:1 für Zustandsfarben als Fläche; er wird auf Text-auf-Tint und Text-auf-Füllung erweitert und schlägt unter 4,5:1 fehl. Gemessene Verstöße heute: DELETE-Label auf seinem Tint über `bg3` 2,65:1, `held`-Label hell auf seinem Tint 2,75:1.
- **Sekundärtext.** `fg2` misst 3,02:1 auf `bg3`, `lFg2` 3,24:1 auf `lBg3`. Beide sind für wirklich deaktivierte Controls reserviert. Jeder Satz, den jemand lesen soll, ist `fg1` oder besser; das betrifft heute den Leerzustand der Queue und die Titel der Klappabschnitte.
- **Fokus.** 2 px Akzentring außerhalb des eigenen Rahmens. Nie durch Umfärben eines vorhandenen Rahmens, nie in der Farbe, mit der das Control gefüllt ist. Auf dem Primärbutton misst der Fokusrahmen heute 1,00:1 gegen die Füllung — ein Tastaturnutzer sieht dort keinen Unterschied.
- **Hit-Targets.** Mindestens 28 × 28 logische Pixel (`HSize.hitMin`), für Erlauben und Blockieren mindestens 32 × 120 px (5.4), im Widget-Test gemessen, nicht behauptet. Bekannter Verstoß: der Eye-Toggle, der ein maskiertes Secret aufdeckt, misst 28 × 19,2 px.
- **Textskalierung.** Bis `TextScaler.linear(2.0)` ohne `RenderFlex`-Overflow und ohne abgeschnittenen Absatz. Kein Kasten, der Text enthält, hat eine feste Höhe; alle Zeilenhöhen aus 3.2 sind Mindesthöhen, und die Aktionsleiste bricht in eine Spalte um, wenn die skalierten Inhalte nicht mehr nebeneinander passen. Widget-Test und Golden bei 2.0 für Queue-Zeile, Aktionsleiste, Header und Statusleiste — feste Höhen schlucken den Overflow still, also gibt es ohne diesen Test keinen Fehlschlag, nur überlappenden Text.
- **Zweiter Kanal, abgestuft.** Das Glyph steht überall neben der Zustandsfarbe, ohne Ausnahme. Das übersetzte Klartext-Label steht überall dort, wo Platz dafür reserviert ist: Karte, Detailansicht, Gruppenkopf, Aktionsleiste, Bestätigungsstreifen. Wo eine Spalte 28 px misst wie die Zustandsspalte der History-Tabelle, tragen Semantics-Label und Tooltip das Wort — und die tragen es **immer**, in jeder Ansicht. Ohne diese Abstufung verbreitert einer die Spalte und einer hakt die Checkliste falsch ab. Im hellen Theme messen `allowed` und `blocked` unter Deuteranopie 1,01:1; das Glyph ist deshalb Pflicht und nicht Zierde.
- **Angesagt wird** — heute gibt es weder `liveRegion` noch `SemanticsService.announce` im Programm:
  - **Ankünfte gebündelt, höflich**, höchstens eine Ansage je zwei Sekunden: en „3 new, oldest GET api.github.com", de „3 neu, älteste GET api.github.com". Fünfzehn Ankünfte in zwanzig Sekunden ergäben sonst fünfzehn volle URLs im Ohr.
  - **Eine selbst ausgelöste Entscheidung höflich**, mit Host, Umfang und der Angabe, ob ein Rückgängig-Fenster läuft. Bestimmt angesagt wird nur, was der Mensch nicht ausgelöst hat: die Timeout-Vorwarnung und der eingetretene Timeout.
  - **Ein abgelehnter Tastendruck höflich**, mit dem Grund aus 5.3.
  - **Das Haltebudget** nur an den Schwellen 5 min, 1 min, 30 s und 10 s, und nur bei 5 % bestimmt.
- **Kein Label ändert sich häufiger als einmal pro Sekunde**, und der Countdown ist kein Live-Bereich. Ein Label, das sich viermal je Sekunde ändert, ist die hörbare Form des Nörgelns. Die verbleibende Zeit steht deshalb nicht im **Label** der Zeile, sondern in ihrem **Semantics-Value**, und wird nur an den vier genannten Schwellen neu ausgesprochen; ein Label mit `mm:ss` ändert sich einmal je Sekunde und wird von jedem Screenreader auf der fokussierten Zeile jedes Mal vollständig wiederholt. Zugleich wird der Countdown nicht mehr per `ExcludeSemantics` versteckt: die Zeile trägt Zustand, Index, Gesamtzahl, Methode, Host und Pfad im Label und die verbleibende Zeit im Value. Ein Widget darf nie der einzige Träger einer Frist und dabei aus der Semantik ausgeschlossen sein.
- **Reduzierte Bewegung** nach 2.10.
- **Ein Timeout ist keine Entscheidung eines Menschen**, nach 4.8.

---

## 7. Leistung als Budget

Ein Budget, das kein Test prüfen kann, ist eine Absichtserklärung. Die Tabelle trennt deshalb zwei Sorten. **Gatter** laufen in `flutter test`, blockieren CI und messen nur, was in einer Fake-Async-Umgebung ohne Rasterizer bestimmbar ist: Build-Zahlen, Element-Zahlen, Aufrufzahlen. **Messung** läuft in `integration_test` mit `IntegrationTestWidgetsFlutterBinding.watchPerformance` unter Xvfb oder in DevTools, wird im PR berichtet und blockiert nicht: Frame- und Rasterzeiten, Speicher. `AutomatedTestWidgetsFlutterBinding` rastert nie, also ist dort jede `FrameTiming` bedeutungslos.

| Fall | Budget | Art |
|---|---|---|
| 200 gehaltene Zeilen, fünf Ankünfte in einem Frame | kein Frame über 16 ms; p95 über 120 Frames im Profile-Build der in `CONTRIBUTING.md` genannten Maschine | Messung |
| eine Entscheidung | zwei Zeilen-Builds, nicht zweihundert | Gatter |
| eine Sekunde Uhr | höchstens ein Build je sichtbarer Zeile | Gatter |
| ein Splitter-Frame | kein Build der Karte, ihres Kopfs und ihrer Body-Vorschau | Gatter |
| 8 MiB Body, 10k History-Zeilen | kein sichtbares Ruckeln | Messung |
| 8 MiB Body, 10k History-Zeilen | Speicher unter 300 MB | Messung, DevTools; `ProcessInfo.currentRss` sieht den GPU-Speicher nicht |

Regeln, die diese Budgets halten:

- **Zwei Uhren, nicht eine.** Die **UI-Uhr** (`nowProvider`, `HMotion.clockTick`, ganze Sekunden) treibt Labels und Ringe und steht still, sobald ihre Sektion unsichtbar ist. Die **Politik-Uhr** ist grob (5 s), hält nie an und feuert nur, was auch in einem anderen Fenster gelten muss: die Timeout-Vorwarnung (4.8), den Tray-Zähler und die Ansagen (Abschnitt 6). Wer beide zu einer macht, tötet mit dem Sparen genau die zwei Fähigkeiten, für die das Programm existiert, während der Mensch woanders ist.
- **Der Ring bekommt keinen eigenen Controller je Zeile.** Über fünf Minuten wandert das Bogenende eines 16-px-Rings rund 0,15 px je Sekunde, also rund 0,003 px je Frame; zweihundert Ticker für Subpixel sind kein Budget, sondern eine Rechnung ohne Gegenwert. Der Ring liest dieselbe Sekundenuhr wie alles andere. Erst unter `HMotion.ringSmoothBelow` (60 s) bekommt er einen `AnimationController`, weil der Bogen dort rund 0,8 px je Sekunde läuft und die Sekundenschritte als Ruckeln sichtbar werden. Einen eigenen Controller braucht ohnehin nur, was unterhalb einer Sekunde etwas aussagt: die Halte-Füllung und der Rail-Wisch.
- **Die Sichtbarkeit ist ein Provider, kein privates Feld.** Der Intercept-Screen berechnet den Flag heute in `_InterceptScreenState`, wo ihn niemand lesen kann; er wird zu einem Provider, den die UI-Uhr beobachtet. `nowProvider` verliert dabei `keepAlive` — heute behauptet sein Doc-Kommentar, der Timer stoppe mit dem letzten Beobachter, und `@Riverpod(keepAlive: true)` sorgt dafür, dass er es nicht tut.
- **Ein Provider, der filtert, sortiert oder gruppiert, sieht nie eine Uhr.** Sonst läuft eine O(n log n)-Projektion über die gesamte Flow-Map je Tick, und n ist der Verkehr der ganzen Sitzung, nicht die Länge der Queue. Zeitabhängige Zeilen bekommen einen einmaligen Timer.
- **Die Flow-Map ist beschränkt.** Entschiedene Flows fallen heraus, sobald ihr Exit-Fenster vorbei ist; History paginiert vom Daemon, nicht aus dieser Map. Eine anwachsende Map, die je Ereignis vollständig kopiert wird, macht aus einem Ereignis eine O(n)-Kopie.
- **Jeder abgeleitete Collection-Provider gibt einen Typ mit Wertgleichheit zurück**, nie eine nackte `List` oder `Map`. Vorbild ist `QueueSnapshot` mit `listEquals`.
- **Skalare mit `.select`.** Nie `.length` auf einer beobachteten Collection.
- **Zwei Builds je Entscheidung sind nur mit Memoisierung erreichbar.** `SliverChildBuilderDelegate.shouldRebuild` liefert unbedingt `true`, und ein `setState` des Panes je Schnappschuss baut jede gebaute Zeile neu. Das Pane hält deshalb `List<FlowId>` und merkt sich je `FlowId` **eine** `QueueRow`-Widget-Instanz; die Zeile beobachtet `flowProvider(id)` und `selectedFlowIdProvider.select(...)` selbst. `Element.updateChild` bricht bei identischem Kind-Widget ab, und das ist der einzige Weg zu der Zahl. `flowProvider(FlowId)` gibt es noch nicht (Abschnitt 9).
- **Eine Zeile beobachtet nur Provider, die auf ihre eigene `FlowId` geschlüsselt sind.** Geteilter Zustand wie die laufende Entscheidung wird dort gelesen, wo er angezeigt wird.
- **Zeigeranwesenheit und Einfrier-Zustand bleiben im `State` des Panes.** Die eingefrorene Reihenfolge ist Sichtzustand und stirbt mit dem Pane; nur der Zähler der ausstehenden Ankünfte ist ein Provider, weil ihn die Pille und die Ansage teilen. Zeigerbewegungen erreichen den Provider-Graphen nie. Das weicht von HUM-029 ab, siehe Abschnitt 8.
- **Kontinuierlicher Drag-Zustand liegt in einem `ValueNotifier`**, den nur das Layout-Widget hört. Ein Provider-Schreibvorgang je Zeigerbewegung baut den ganzen Screen 120-mal je Sekunde neu.
- **Feste Zeilenhöhen sind der Liste bekannt, wo n groß ist und keine Zeile die Höhe wechselt**: History über `FixedSpanExtent`, Body- und Hex-Ansichten über `itemExtent`. Die Intercept-Queue ist ausgenommen (2.4), weil sie `AnimatedList` braucht und weil ihre Zeilenhöhe bei größerer Textskalierung ohnehin keine Konstante ist. Ohne Extent kostet Scrollen linear statt konstant — bei zehntausend Zeilen ist das der Unterschied, bei zweihundert nicht.
- **Animationswrapper verlassen den Baum**, sobald `animation.isCompleted` gilt. Eine lazy Liste baut nur ihr Viewport, also trägt eine Queue nicht sechshundert überflüssige Renderobjekte, sondern rund zwei je sichtbarer Zeile — in einem 660 px hohen Pane etwa sechsunddreißig. Genau die zahlt man je Frame, und genau deshalb verschwinden sie. `CurvedAnimation` ist ein `late final`-Feld eines `State` und wird disposed, nie ein Ausdruck in `build`.
- **`FadeTransition` und `SlideTransition` statt `Opacity` plus `Transform` in einem `AnimatedBuilder`.** Die Transitions bauen den Kindbaum nicht je Frame neu; der `AnimatedBuilder`-Umweg tut es, und zwar an den beiden Stellen, die ohnehin am teuersten sind.
- **`RepaintBoundary` genau um die Inseln mit eigener Uhr**: Countdown-Ring, Kartenwisch, atmendes Glyph — und um die ankommende und die gehende Zeile, weil dort je Frame eine Deckkraftschicht bezahlt wird. Sonst nirgends; anderswo kostet sie nur eine Textur.
- **`Shortcuts`- und `Actions`-Maps sind `late final`-Felder**, keine Ausdrücke in `build`. Kinder, die ein rebauender Elternteil unverändert durchreicht, sind `const` oder Felder.
- **Keine Formatierung, Kürzung oder Analyse in einem Layout-Callback.** Einmal gegen die Constraints rechnen und gegen diese Constraints cachen; `middleEllipsis` allokiert je Aufruf eine volle Runen-Liste.
- **Alles, was einen Body anfasst**, läuft über 64 KiB in `Isolate.run` und gibt nur einfache Dart-Werte zurück.

Wie ein Test das beweist:

1. **Rebuild-Zähler (Gatter).** Eine statische Zählvariable im `build` des Widgets, eine Zustandsänderung durch den Container, die exakte Zahl geprüft. Pflicht je Screen: `QueueRow` je Entscheidung, `HeaderBar` je Ereignis, `InterceptScreen` je Splitter-Frame, History-Zeile je Live-Aktualisierung. Rebuild-Umfang ist für Compiler und Goldens unsichtbar und regressiert sonst still.
2. **Lastgatter (Gatter).** Der Fake-Daemon kennt `burst:200`. Ein stehender Widget-Test hält damit die Zahl der `QueueRow`-Builds je Sekunde Fake-Zeit und je Entscheidung fest.
3. **Frame-Budget (Messung).** Für 10k History-Zeilen und einen 8-MiB-Body misst ein `integration_test` unter Xvfb Frame- und Rasterzeiten; die Zahl steht im PR. HUM-030 misst heute nur UI-Thread-Zeiten über `SchedulerBinding` und HUM-032 führt die Frames als Risiko mit DevTools-Messung — beides ist weniger, als diese Tabelle verlangt, und wird entsprechend erweitert.

---

## 8. Wo dieses Dokument von einer Spezifikation abweicht

Damit die Abweichung prüfbar bleibt, steht sie hier und nicht zwischen den Zeilen.

- **HUM-028:** Der Bestätigungsstreifen gehört an die entschiedene Zeile, nicht in die Karte, und er ist kein zusätzlicher 28-px-Kasten, sondern der getauschte Inhalt derselben Zeile (3.4). In der Karte blockierte er drei Sekunden lang genau den Platz, an dem die nächste Entscheidung fällt.
- **HUM-028:** Die Release Valve trägt die Akzent-Tönung am Token-Deckel (10 %), nicht 12 %. `HColorDerivation.tint` deckelt bei `HColors.tintAlpha`; ein Wert, den die Token-Schicht nicht erzeugen kann, ist keine Spezifikation, und zwei Prozent Alpha sieht niemand.
- **HUM-028 und `BACKLOG.md` 5:** Auf dem deutschen Control steht „Senden", nicht „Erlauben"; „Erlauben" gehört dem Regelsatz (4.6).
- **HUM-020 und `BACKLOG.md` 5:** Die Queue-Zeile bleibt in jedem Zustand 36 px hoch und hat keine zweite Zeile, weil diese Zeile nur wiederholt, was die Karte daneben zeigt, und dabei die Liste unter dem Auge wandern lässt. 36 px ist dabei eine Mindesthöhe, die mit der Textskalierung wächst.
- **HUM-020:** Aus der Zeile heraus wird nur blockiert. Erlauben ist unumkehrbar und verlangt die URL, und die steht in der Karte.
- **HUM-029:** Der Gruppenkopf zeigt nur `Block {n}`; `Allow {n}` übernimmt die Aktionsleiste, sobald eine Gruppe ausgewählt ist (3.5).
- **HUM-029:** Die eingefrorene Reihenfolge lebt im `State` des Queue-Panes, nicht in einem `queueFreezeProvider`; nur der Zähler der ausstehenden Ankünfte ist ein Provider (Abschnitt 7).
- **HUM-029 und Abschnitt 7:** Die Intercept-Queue behält `AnimatedList` und verzichtet auf `itemExtent`; die Extent-Pflicht gilt in History und in den Body-Ansichten (2.4).
- **HUM-030:** Werte im JSON-Baum bleiben auf der `fg`-Leiter; einzige Chroma sind Findings.
- **HUM-034:** Der Tray-Zähler steht in `held`-Amber, nicht als Akzentpunkt. Das Tray-Icon ist ein Zustandsanzeiger, kein Control (3.3, Regel 9).
- **HUM-049:** Erlauben verlangt bei mindestens einem ungelösten Finding dieselbe Halte-Bestätigung wie Blockieren, plus einen Satz, der Secret-Typ und Zielhost benennt — aber kein Modal (4.7).
- **`BACKLOG.md` 5:** Das Glyph atmet begrenzt (zwei Schwellen, je drei Züge) statt dauerhaft, es hellt auf, statt zu verblassen, und ein Glyph, das eine Schwelle überschreitet, startet seinen ersten Zug bei voller Deckkraft (2.7).
- **`packages/ui`:** Die ruhende `held`-Rail in der Queue ist namentlich von der 3:1-Regel ausgenommen (3.3, Regel 10). Jede andere Fläche und jede andere Rail erreicht sie.

---

## 9. Was das Designsystem noch braucht

Ergänzt wurde bisher nur `app/packages/ui/lib/src/tokens/motion.dart`: `leaveOffset`, `stagger`, `staggerMax`, `leaveGlideFraction`, `holdToBlock`, `rearm`, `confirm`, `undoWindow`, `freezeAfterKey`, `freezeAfterPointer`, `clockTick`, `waitVisible`, `waitMinVisible`, `ringSmoothBelow`, `breatheBelowUrgent`, `breatheCycles`, `breatheMinOpacity`, `reducedRingAlpha` sowie `HReducedMotion`. Alles Folgende ist offen und wird in eigenen Issues entschieden, nie beiläufig.

**Token:**

1. `HSize.rowSelected` (56) benutzt nach 3.4 keine Zeile mehr. Entweder streichen oder auf die History-Detailzeile umwidmen.
2. `HSize.selectionRail` (2) entfällt, sobald die Auswahl die Zustands-Rail ersetzt statt sie zu überlagern.
3. Die drei Zeilendichten aus 3.2 brauchen drei Token: `HSize.rowHistory` (28), `HSize.rowBody` (24) und `HSize.rowActionSlot` (28). Der Aktionsslot borgt sich heute `hitMin`, das etwas anderes bedeutet. Ohne diese Token schreibt der erste History-Screen eine 28 in eine Feature-Datei, und die Regel gegen Literale aus 2.1 bricht an dem Dokument, das sie aufgestellt hat.
4. Eine eigene Untergrenze für die beiden Entscheidungen: `HSize.hitDecision` (32 px Höhe, 120 px Breite) neben `HSize.hitMin` (5.4).
5. Die Methoden-Hues erreichen als Text auf ihrem eigenen Tint keine 4,5:1 (DELETE dunkel 2,65:1). Entweder Label- und Flächenfarbe getrennt führen oder die Hues nachziehen.
6. Der Countdown-Ring nimmt `tokens.colors.line` als Spur; im hellen Theme misst der Bogen dagegen 2,84:1. Ein neues Token `ringTrack` wäre eine Lösung; die verbrauchte Strecke als Lücke statt als Spur zu zeichnen ist die andere und braucht kein Token — deshalb der Vorschlag.
7. Für die Maß-Obergrenze aus 3.2 gibt es kein Token. Vorschlag: eine Zeichenzahl (90), keine Pixelbreite, weil die Breite von der installierten Schrift abhängt.
8. Eine Rail, die 3:1 erreicht, bräuchte ein eigenes Alpha-Token weit oberhalb von `HColors.tintAlpha`. Dieses Dokument will sie nicht (3.3, Regel 10); wer sie doch will, führt das Token ein und darf `tint()` nicht dafür missbrauchen.

**Widgets in `packages/ui`:**

9. Eine gehende Zeile braucht ein eigenes Widget ohne `ref`: `QueueRowSnapshot` (oder `HRow.frozen`) nimmt aufgelöste Werte — Zustand, Glyph, Methode, Host, Pfad, Restzeit als Zeichenkette — und beobachtet nichts. Der Abgang zeichnet dieses Widget, nie die lebende Zeile (2.4).
10. `HRow` malt Hover und Auswahl beide `bg3`, mit einem Kommentar, der das begründet. Hover muss `bg2` werden (3.4).
11. `HRow` benutzt `HMotion.sweep` für Hover-Tönung und Höhe. Hover gehört `HMotion.press`, die Höhe animiert nicht.
12. `HRow` hat keinen Slot für ein Zustands-Glyph und ist nicht fokussierbar. Beides braucht 3.4 und 5.1. Dazu ein Slot für die Mehrfachauswahl-Rail (3.5).
13. `HMethodBadge` braucht eine neutrale Variante (`fg1` auf `bg2`) für Listen; die farbige bleibt dem Kartenkopf.
14. `HButton` zeichnet Fokus durch Umfärben des vorhandenen 1-px-Rahmens. Gebraucht wird ein 2-px-Akzentring außerhalb; zwei `BoxDecoration` reichen dafür, ein neues Token nicht.
15. `HStateGlyph` atmet linear, endlos und bis 0,45 Deckkraft. Gebraucht: `easeInOut`, Untergrenze `HMotion.breatheMinOpacity`, `HMotion.breatheCycles` je Schwelle, Phase aus der Uhrzeit modulo `HMotion.breathe` mit eigenem Start bei voller Deckkraft (2.7) und der ruhende Ersatzring unter reduzierter Bewegung.
16. `HModal` hat weder `FocusScope` noch `Escape`-Bindung.
17. `HPill`, `HIconButton`, `HRow` und `HBadge` mit `onTap` sind nicht fokussierbar; `HButton` ist heute das einzige fokussierbare Control im System.
18. `HBadge.chipHeight` und `HButtonSize` sind feste Höhen und laufen bei `TextScaler` 2.0 über. Mindesthöhen statt Höhen.
19. Ein Skelett-Widget für 2.11: Haarlinien in der Höhe einer Zieldichte, `fg2`, ohne Bewegung, mit den beiden Schwellen `waitVisible` und `waitMinVisible` eingebaut, damit sie nicht in jedem Screen neu erfunden werden.

**In `app/lib`, außerhalb dieses Dokuments zu entscheiden:**

20. `flowProvider(FlowId)` gibt es nicht; Abschnitt 7 braucht ihn für die Memoisierung der Zeilen.
21. Die Sichtbarkeit einer Sektion braucht einen Provider; heute ist sie ein privates Feld von `_InterceptScreenState`.
22. `nowProvider` verliert `keepAlive` und wird die UI-Uhr; die Politik-Uhr aus Abschnitt 7 kommt daneben.
23. `ConnectionGate` ersetzt heute die ganze Shell, sobald die Verbindung fehlschlägt — auch mitten in der Arbeit. 4.2 Punkt 4 verlangt, dass der Setup-Screen nur beim Kaltstart übernimmt.

**Tests:**

24. Der Kontrast-Test prüft Farbe-auf-Fläche mit 3:1. Er braucht Text-auf-Tint und Text-auf-Füllung mit 4,5:1, sonst rutscht jedes Badge-Label wieder unter AA durch. Die Ausnahme aus 3.3 Regel 10 steht namentlich im Test, nicht als aufgeweichte Schwelle.
25. Es gibt keinen Test, der belegt, dass jede Zustandsfarbe von Glyph und Label begleitet ist, und keinen bei `TextScaler` 2.0.
26. Ein ARB-Lint prüft die Verbotswörter aus 4.1 in beiden Sprachen und kennt die drei Aussage-Schlüssel als Ausnahme.
27. Ein Widget-Test hält `Enter` 500 ms gedrückt und erwartet genau ein `Decide` (5.4).

---

## 10. Checkliste vor „fertig"

- [ ] Jede Animation beantwortet eine der vier Fragen aus 2.1; jede Dauer, Kurve und Strecke kommt aus `HMotion`, kein Literal in einer Feature-Datei.
- [ ] Jedes Overlay — Sheet, Modal, Palette, Banner, Sektions- und Tabwechsel — nimmt seine Zeile aus 2.2 und erfindet keine eigene Bewegung.
- [ ] Nichts bewegt sich, was gerade gelesen wird; keine Zahl, kein Fokusring und keine Zeilenhöhe animiert wegen eines Zustandswechsels.
- [ ] Strecken und Schleifen laufen über `HReducedMotion`, jede Schleife hat einen ruhenden Ersatz, und der Abgang behält unter reduzierter Bewegung seine Rückmeldung (2.10).
- [ ] Jeder Wartevorgang folgt 2.11: nichts unter 150 ms, danach Skelett in der Zieldichte, mindestens 400 ms stehend, kein Spinner, keine Verschiebung beim Eintreffen.
- [ ] Der Screen hat genau ein gefülltes Control je Entscheidungskontext, einen Leerzustand und ein größtes Textelement, und das ist das Wichtigste darauf.
- [ ] Jede Zustandsfarbe wird vom Glyph begleitet, vom Klartext-Label überall dort, wo Platz reserviert ist, und von Semantics und Tooltip immer; Akzent nur auf Fokussierbarem oder Anfassbarem; Chrome ohne zweiten Wert trägt `fg1`.
- [ ] Alle Abstände sind Vielfache von 4, die rechte Rinne misst 12 px, das Textmaß höchstens 90 Monospace-Zeichen — außer in Code, Hex und Tabellen, die waagerecht scrollen und nie umbrechen.
- [ ] Jeder Leerzustand nennt das nächste Ereignis oder, bei einem Filter, Trefferzahl und Rückweg; kein „Nichts", kein „noch", kein internes Vokabular, jeder Text aus ARB in beiden Sprachen.
- [ ] Die vier Zustände vor der ersten Anfrage aus 4.2 sind gebaut, und der Setup-Screen ersetzt die Shell nur beim Kaltstart.
- [ ] Ein Timeout hinterlässt eine Vorwarnung am Flow, drei Sekunden graue Zeile und einen dauerhaften Zähler in der Statusleiste (4.8).
- [ ] Eine Anfrage mit Finding trägt den Chip in `HColors.secret`, den Grund in Klartext und die Halte-Bestätigung auf dem Erlauben (4.7).
- [ ] Jeder Fehlerpfad zeigt ein `Diagnostic` mit dem `why` des Daemons und mit `fix`, verankert am Ort der Aktion.
- [ ] Jeder Aktivator hat eine Action, jeder Zeigerweg eine Taste, jeder angenommene Druck eine sichtbare Reaktion in 100 ms, jeder abgelehnte eine leise und eine angesagte.
- [ ] Weder `Enter` noch `Leertaste` noch ein einzelner Buchstabe gewinnt gegen ein fokussiertes Control; ein Test vergleicht beide Tastenmengen, und ein zweiter hält `Enter` 500 ms gedrückt und erwartet ein `Decide`.
- [ ] Erlauben ist nach jeder neuen Auswahl `HMotion.rearm` lang unscharf, Tastenwiederholung entscheidet nie, und die Auswahl springt nicht weiter, solange eine Entscheidungstaste gedrückt ist.
- [ ] Kontrast: Text ≥ 4,5:1 auch auf Tint und Füllung, Flächen ≥ 3:1 mit der einen benannten Ausnahme; Hit-Targets ≥ 28 × 28 px, Erlauben und Blockieren ≥ 32 × 120 px, im Test gemessen.
- [ ] Widget-Test und Golden bei `TextScaler` 2.0 ohne Overflow; Semantics tragen Zustand, Index und Gesamtzahl im Label und die Frist im Value; kein Label ändert sich häufiger als einmal je Sekunde; Ankünfte sind gebündelt.
- [ ] Ein Rebuild-Zähler-Test hält die Builds je Entscheidung, je Sekunde Uhr und je Splitter-Frame fest; die Frame- und Speicherzahlen stehen als Messung im PR, nicht als Gatter im Unit-Test.
- [ ] `make check` grün, Goldens grün, und jede Abweichung von einer Spezifikation steht in Abschnitt 8.
