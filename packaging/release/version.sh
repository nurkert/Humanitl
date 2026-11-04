# shellcheck shell=bash
# Gemeinsame Pruefung der Versionsnummer fuer die Skripte unter
# `packaging/release`, `packaging/deb` und den anderen Paketformaten. Wird mit
# `source` geladen, nie ausgefuehrt.
#
# Der Kanal dieser Skripte sind die Versionen 0.Y.Z: die Vorabversionen 0.0.N
# und ab 0.1.0 die Releases vor 1.0.0 (HUM-060). Die Nummer kommt aus einem
# Git-Tag oder aus einer Eingabe von Hand, also aus einer Quelle, der niemand
# trauen muss: Sie wird hier gegen ein enges Muster geprueft, bevor sie in
# einen Dateinamen, eine Control-Datei oder einen Befehl gelangt.
#
# 1.0.0 und alles danach lehnt das Muster ab: Ab 1.0.0 sind Releases signiert
# (Entscheidung des Inhabers, 2026-10-22), und solange es die Signatur nicht
# gibt, soll kein Werkzeug versehentlich eine 1.x schneiden.

# Bricht mit Exit 1 ab, wenn "$1" keine Version der Form 0.Y.Z ist.
#
# Die Ziffern stehen einzeln im Muster und nicht als [0-9]: Unter einer
# UTF-8-Locale passt ein Bereich in bash auch auf Ziffern anderer Schriften
# (etwa arabisch-indische oder hochgestellte), und dann waere "0.0.١" eine
# gueltige Version. Fuehrende Nullen sind wie bei SemVer verboten.
require_release_version() {
  local candidate="${1:-}"
  if [[ ! "$candidate" =~ ^0\.(0|[123456789][0123456789]{0,5})\.(0|[123456789][0123456789]{0,5})$ ]]; then
    echo "error: '$candidate' is not a 0.Y.Z version; this channel knows 0.0.N and 0.Y.Z, not 1.0.0, which needs signatures" >&2
    exit 1
  fi
}

# Exit 0, wenn "$1" eine Vorabversion 0.0.N ist (Release-Notes ohne
# Changelog-Abschnitt); sonst 1. Setzt eine schon gepruefte Version voraus.
is_prerelease_version() {
  [[ "${1:-}" == 0.0.* ]]
}
