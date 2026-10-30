---
source: adr/0000-template.md
source-sha256: 0435a820518e0a7d6d33a08f058bd231ae8c1221b63752a369e4ef1b6e5f730b
lang: de
---
# ADR-NNNN · Kurzer Titel als Aussage

Applies to: Humanitl 0.1

Status: Accepted | Superseded by ADR-XXXX | Deprecated
Date: YYYY-MM-DD

## Kontext

Welche Kräfte wirken? Welches Problem erzwingt eine Entscheidung, und warum lässt sich
die Entscheidung nicht aufschieben? Der Abschnitt beschreibt die Lage, ohne die Lösung
vorwegzunehmen. Er nennt die harten Randbedingungen (Zielplattform,
Sicherheitsgarantien, Teamgröße, vorhandene Bausteine), damit ein späterer Leser
versteht, unter welchen Annahmen die Entscheidung fiel. Ändern sich diese Annahmen, ist
das der Anlass für ein Nachfolge-ADR.

## Entscheidung

Ein Satz im Aktiv und im Präsens: „Wir tun X.“ Dann die konkrete Ausgestaltung, genau
genug, um prüfbar zu sein: Typen, Dateipfade, Konfigurationsschlüssel, Statuscodes,
Versionsnummern. Prosa nur dort, wo kein Bezeichner reicht.

## Begründung

Warum diese Option die Kräfte aus dem Kontext am besten auflöst. Jede Behauptung ist so
formuliert, dass sie widerlegbar ist. Wo eine Annahme unsicher ist, sagt der Abschnitt,
wie sie geprüft wird (Test, Spike, Meilenstein).

## Verworfene Alternativen

Ein Absatz oder Listenpunkt je Alternative: was sie gewesen wäre und warum sie verlor.
Eine Alternative ohne Grund für ihr Verlieren ist keine dokumentierte Alternative,
sondern eine Notiz. „Nichts tun“ ist eine legitime Alternative und wird genannt, wenn
sie ernsthaft erwogen wurde.

## Konsequenzen

Was aus der Entscheidung folgt, positiv und negativ. Insbesondere: welche Arbeit sie
schafft, welche Risiken sie einkauft, welche Abhilfe dafür geplant ist, welche Türen
sie schließt und welche sie offenhält. Hier werden auch spätere ADRs genannt, die diese
Entscheidung korrigieren oder verfeinern.

## Betroffene Issues

`HUM-xxx`, `HUM-yyy`: die Issues, die diese Entscheidung umsetzen oder prüfen.
Mindestens eines. Verweis auf `backlog/sprint-N.md`.
