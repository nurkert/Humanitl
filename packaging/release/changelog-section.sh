#!/usr/bin/env bash
# Gibt den Abschnitt einer Version aus `CHANGELOG.md` aus, ohne seine
# Ueberschrift (HUM-060). Die Release-Notes ab 0.1.0 beginnen damit.
#
# Aufruf: packaging/release/changelog-section.sh <0.Y.Z> [<changelog>]
#
# Das Format ist Keep a Changelog 1.1.0. Ein Abschnitt beginnt mit einer Zeile
# `## [<version>] - YYYY-MM-DD` und endet vor der naechsten Zeile, die mit
# `## [` beginnt, oder vor dem ersten Linkverweis `[...]: ...` am Ende der
# Datei. Nur `## [` beendet einen Abschnitt: Eine Zeile `## ...` in einem
# Codeblock oder eine Unterueberschrift `### Added` tut es nicht.
#
# Abgelehnt (Exit 1, Grund auf stderr) wird:
# - eine Version ohne Abschnitt,
# - ein Abschnitt ohne gueltiges Datum, etwa der Entwurf `## [0.1.0] - unreleased`:
#   Das Datum setzt der Release-Commit, und erst dann darf der Probelauf gruen
#   werden,
# - ein Abschnitt ohne einen einzigen Eintrag `- ...`.
#
# Ausgegeben wird der Text zwischen Ueberschrift und Ende, ohne Leerzeilen am
# Anfang und am Ende.
set -euo pipefail

[[ $# -eq 1 || $# -eq 2 ]] || { echo "usage: $0 <0.Y.Z> [<changelog>]" >&2; exit 2; }
version="$1"
here="$(cd "$(dirname "$0")" && pwd)"
changelog="${2:-$here/../../CHANGELOG.md}"
# shellcheck source=packaging/release/version.sh
source "$here/version.sh"
require_release_version "$version"
[[ -f "$changelog" ]] || { echo "error: $changelog does not exist" >&2; exit 1; }

python3 -I - "$version" "$changelog" <<'PY'
import datetime
import re
import sys

version, path = sys.argv[1], sys.argv[2]
lines = open(path, encoding="utf-8").read().splitlines()
prefix = f"## [{version}]"
start = None
for i, line in enumerate(lines):
    if line == prefix or line.startswith(prefix + " "):
        if start is not None:
            sys.exit(f"error: {path} has two sections for {version}")
        start = i
if start is None:
    sys.exit(f"error: {path} has no section '## [{version}] - YYYY-MM-DD'")

heading = lines[start]
m = re.fullmatch(r"## \[[0-9.]+\] - (\d{4}-\d{2}-\d{2})", heading)
if not m:
    sys.exit(f"error: the heading '{heading}' in {path} has no release date; "
             f"write '## [{version}] - YYYY-MM-DD' in the release commit")
try:
    datetime.date.fromisoformat(m.group(1))
except ValueError:
    sys.exit(f"error: '{m.group(1)}' in the heading '{heading}' is not a date")

body = []
for line in lines[start + 1:]:
    if line.startswith("## [") or re.match(r"^\[[^\]]+\]: \S", line):
        break
    body.append(line)
while body and not body[0].strip():
    body.pop(0)
while body and not body[-1].strip():
    body.pop()
if not any(re.match(r"^\s*- \S", line) for line in body):
    sys.exit(f"error: the section {version} in {path} has no entry ('- ...')")
print("\n".join(body))
PY
