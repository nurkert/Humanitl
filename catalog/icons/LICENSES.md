# Herkunft und Lizenz der Symbole

Alle SVG-Dateien in diesem Verzeichnis sind für Humanitl gezeichnet und stehen
unter derselben Lizenz wie das übrige Projekt, GPL-3.0-only. Sie benutzen
`currentColor`, tragen keine Farbe und keinen Schatten, und sie sind auf ein
Raster von 24 × 24 gezeichnet, das bei 20 × 20 dargestellt wird.

## Warum hier keine Logos liegen

Ein Katalogeintrag hat kein Marken-Logo, sondern das Symbol seiner Kategorie.
Das hat zwei Gründe, und beide sind Absicht.

Der erste ist rechtlich. Ein Logo ist eine Marke. Wir dürften die Zeichen von
GitHub, Docker oder OpenAI nicht ohne Weiteres mit einem GPL-Programm
ausliefern, und ein nachgezeichnetes Logo wäre dasselbe Problem in schlechterer
Qualität.

Der zweite wiegt schwerer. Ein echtes Logo wirkt wie eine Beglaubigung. Es sagt
„das ist wirklich GitHub", und genau das kann der Katalog nicht wissen: Er
kennt ein Namensmuster, mehr nicht. Der Nachweis, dass die Gegenstelle die ist,
für die sie sich ausgibt, liegt beim Zertifikat und beim Namensvergleich, nicht
bei einem Bild. Ein Kategorie-Symbol behauptet nur, was der Katalog wirklich
weiß: dass hier eine Paket-Registry steht und keine Suchmaschine.

## Zuordnung

Der Schlüssel `icon` eines Eintrags nennt die Datei; heute ist das immer das
Symbol der Kategorie:

| Datei | Kategorie |
|---|---|
| `scm.svg` | `scm` |
| `registry.svg` | `registry` |
| `docs.svg` | `docs` |
| `ci.svg` | `ci` |
| `cloud.svg` | `cloud` |
| `ai.svg` | `ai` |
| `cdn.svg` | `cdn` |
| `search.svg` | `search` |
| `os.svg` | `os` |
| `other.svg` | `other` |
| `globe.svg` | Rückfall, wenn die genannte Datei fehlt |

Die Oberfläche lädt Symbole ausschließlich aus diesem Verzeichnis. Ein Favicon
von der Startseite eines Dienstes zu holen, wäre ein Netzzugriff vor der
Entscheidung und damit genau das, was ADR-006 ausschließt.
