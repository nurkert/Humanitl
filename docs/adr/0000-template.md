# ADR-NNNN · Kurzer Titel im Aussagesatz
Status: Accepted | Superseded by ADR-XXXX | Deprecated
Datum: JJJJ-MM-TT

## Kontext

Welche Kräfte wirken? Welches Problem zwingt zu einer Entscheidung, und warum
lässt sich die Entscheidung nicht vertagen? Der Abschnitt beschreibt die Lage
ohne die Lösung vorwegzunehmen. Er nennt die harten Randbedingungen
(Zielplattform, Sicherheitsgarantien, Teamgröße, vorhandene Bausteine), damit ein
späterer Leser versteht, unter welchen Annahmen entschieden wurde. Ändern sich
diese Annahmen, ist das der Anlass für einen Nachfolge-ADR.

## Entscheidung

Ein Satz im Aktiv und Präsens: „Wir tun X." Danach die konkrete Ausprägung, so
präzise, dass sie prüfbar ist: Typen, Dateipfade, Konfigurationsschlüssel,
Statuscodes, Versionsnummern. Prosa nur dort, wo kein Bezeichner reicht.

## Begründung

Warum diese Option die Kräfte aus dem Kontext am besten auflöst. Jede Behauptung
so formuliert, dass sie falsifizierbar ist. Wo eine Annahme unsicher ist, wird
gesagt, wie sie überprüft wird (Test, Spike, Milestone).

## Verworfene Alternativen

Pro Alternative ein Absatz oder Listenpunkt: was sie gewesen wäre und warum sie
verloren hat. Eine Alternative ohne Verlustgrund ist keine dokumentierte
Alternative, sondern eine Notiz. „Nichts tun" ist eine legitime Alternative und
wird genannt, wenn sie ernsthaft erwogen wurde.

## Konsequenzen

Was aus der Entscheidung folgt, positiv wie negativ. Insbesondere: welche Arbeit
sie erzeugt, welche Risiken sie einkauft, welche Mitigation dafür vorgesehen ist,
welche Türen sie zumacht und welche sie offen hält. Hier steht auch, welche
späteren ADRs diese Entscheidung korrigieren oder verfeinern.

## Betroffene Issues

`HUM-xxx`, `HUM-yyy` — die Issues, die diese Entscheidung umsetzen oder prüfen.
Mindestens eines. Bezug zu `backlog/sprint-N.md`.
