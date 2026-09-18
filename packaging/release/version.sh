# shellcheck shell=bash
# Gemeinsame Pruefung der Versionsnummer fuer die Skripte unter
# `packaging/release` und `packaging/deb`. Wird mit `source` geladen, nie
# ausgefuehrt.
#
# Der Kanal dieser Skripte sind die Vorabversionen 0.0.N. Die Nummer kommt aus
# einem Git-Tag oder aus einer Eingabe von Hand, also aus einer Quelle, der
# niemand trauen muss: Sie wird hier gegen ein enges Muster geprueft, bevor sie
# in einen Dateinamen, eine Control-Datei oder einen Befehl gelangt. 0.1.0 und
# alles danach gehoert HUM-060 (`backlog/sprint-5.md`).

# Bricht mit Exit 1 ab, wenn "$1" keine Version der Form 0.0.N ist.
#
# Die Ziffern stehen einzeln im Muster und nicht als [0-9]: Unter einer
# UTF-8-Locale passt ein Bereich in bash auch auf Ziffern anderer Schriften
# (etwa arabisch-indische oder hochgestellte), und dann waere "0.0.١" eine
# gueltige Version.
require_prerelease_version() {
  local candidate="${1:-}"
  if [[ ! "$candidate" =~ ^0\.0\.(0|[123456789][0123456789]{0,5})$ ]]; then
    echo "error: '$candidate' is not a 0.0.N version; this channel only knows 0.0.N (0.1.0 is HUM-060)" >&2
    exit 1
  fi
}
