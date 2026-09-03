# Gebündelte Daten des Domain-Panels

Alles in diesem Verzeichnis wird mit Humanitl ausgeliefert und zur Laufzeit nur
gelesen. Es gibt keinen automatischen Abruf: kein Favicon, kein `og:title`,
keine Rangliste aus dem Netz (ADR-006). Wer im Panel eine Vorschau will, klickt
sie in einer späteren Fassung ausdrücklich an, und dann holt sie der Host, nicht
die Sandbox.

| Datei | Was drinsteht |
|---|---|
| `domains.yaml` | Der Katalog: Namensmuster, Kategorie, Beschreibung, Quelle |
| `domains.schema.json` | Die Form von `domains.yaml`, für die Prüfung in CI |
| `icons/*.svg` | Kategorie-Symbole, siehe `icons/LICENSES.md` |
| `ranks-top100k.csv.gz` | Verbreitungsränge, siehe `RANKS-LICENSE` |
| `RANKS-LICENSE` | Herkunft, Prüfsummen und Lizenz der Rangliste |

Gelesen wird das alles von der Crate `humanitl-catalog`
(`daemon/crates/catalog`). Die Oberfläche bündelt dieselbe `domains.yaml` und
dieselben Symbole als Asset, damit sie Text und Bild ohne einen weiteren
Aufruf hat; über den Ereignisstrom reist nur die Kennung `catalog_id`.

## Der Apex und die eingebaute Suffix-Liste

Den Apex, also die registrierbare Domain zu einem Host, liefert die Crate `psl`
mit einer einkompilierten Public Suffix List. Auch das ist gebündelt: es wird
nichts nachgeladen.

Weil die Liste zur Bibliothek gehört, ändert sich der Apex mit jeder Fassung
der Crate. `github.io` steht zum Beispiel im privaten Abschnitt, deshalb ist der
Apex von `a.b.github.io` die Domain `b.github.io` und nicht `github.io`; käme
oder ginge so ein Eintrag unbemerkt, stünde plötzlich eine andere Domain im
Panel und eine Regel `**.<apex>` deckte etwas anderes ab als vorher. Der Apex
ist eine Aussage, die ein Mensch zu sehen bekommt, also darf sie sich nicht
zwischen zwei Builds von selbst ändern.

`daemon/Cargo.toml` pinnt `psl` deshalb exakt auf `=2.1.228`. Ein Wechsel der
Version ist ein eigener Commit; er gehört in den Changelog des Releases, weil
er die Antwort auf eine Nutzerfrage ändern kann.

## Was ein Eintrag behauptet

Ein Eintrag ordnet einem Namensmuster einen Dienst zu und nennt unter `source`
die Seite des Betreibers, auf der das nachlesbar ist. Mehr sagt er nicht. Er
sagt insbesondere nicht, dass eine Anfrage an diesen Dienst unbedenklich sei;
über dieselbe Verbindung geht ein `git clone` und ein Datenabfluss.

Ein Name, der in keinem Muster vorkommt, ist **unbekannt**. Das ist eine
Aussage über den Katalog, nicht über den Dienst: Der Katalog kennt heute 34
Einträge und wächst in HUM-059 auf etwa zweihundert. Die Oberfläche schreibt
deshalb „Not in catalog" und nicht „unbekannter Anbieter", und sie lässt die
Karte gestrichelt, damit niemand die Leerstelle für ein Grün hält.

## Eine Zeile ändern

1. `domains.yaml` bearbeiten. Die Beschreibung steht in `en` und `de`; `en` ist
   die Quelle. `source` ist Pflicht und muss die Behauptung tatsächlich stützen.
2. `check-jsonschema --schemafile catalog/domains.schema.json catalog/domains.yaml`
3. `cargo test -p humanitl-catalog` — die Tests laden die ausgelieferten Dateien
   und prüfen unter anderem, dass jedes Symbol existiert, dass keine zwei
   Einträge dasselbe Muster beanspruchen und dass die Prüfsummen in
   `RANKS-LICENSE` zur Rangliste daneben passen.

Dieselbe Schema-Prüfung läuft in `scripts/ci/lint-docs.sh`, also in
`make check`. Fehlt `check-jsonschema` auf dem Rechner, sagt das Skript das und
läuft weiter.
