#!/usr/bin/env bash
# Schreibt die Spezifikation eines GitHub-Issues als Markdown: Titel, Labels
# und Koerper.
#
#   tools/issue-spec.sh <N>             nach stdout
#   tools/issue-spec.sh <N> -o <datei>  in eine Datei
#
# Seit dem Umzug der Issues nach GitHub steht die Spezifikation eines Issues
# im Issue selbst, nicht mehr in backlog/sprint-N.md. Agenten und Reviewer,
# die ohne Netz laufen (Codex arbeitet read-only ohne Netz), lesen sie aus der
# Datei, die dieses Skript vorher schreibt. Die Datei gehoert nicht ins
# Repository; sie ist eine Momentaufnahme des Issues.
#
# Umgebung: HUMANITL_GH ersetzt den Befehl gh (der Test setzt einen Stub ein).
set -euo pipefail

gh="${HUMANITL_GH:-gh}"

die() {
  echo "issue-spec: $*" >&2
  exit 1
}

usage="usage: tools/issue-spec.sh <issue-number> [-o file]"
number="" out=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o)
      [[ $# -ge 2 && -n "$2" ]] || die "$usage"
      out="$2"
      shift 2
      ;;
    -h | --help)
      echo "$usage"
      exit 0
      ;;
    *)
      [[ -z "$number" ]] || die "$usage"
      number="$1"
      shift
      ;;
  esac
done
[[ "$number" =~ ^[1-9][0-9]{0,6}$ ]] || die "$usage (got '${number}')"

# gh gibt Zeichenketten aus --jq roh aus, wie `jq -r`. Ohne Labels steht
# "none" da, damit ein Leser nicht raten muss, ob die Zeile fehlt.
# shellcheck disable=SC2016 # Backticks und \( \) gehoeren jq, nicht der Shell.
render='"# #\(.number): \(.title)\n\nLabels: \(if (.labels | length) == 0 then "none" else ([.labels[].name | "`" + . + "`"] | join(", ")) end)\n\n\(.body // "")"'

spec="$($gh issue view "$number" --json title,body,labels,number --jq "$render")" \
  || die "gh issue view $number failed; is gh logged in (gh auth status), and does issue #$number exist?"
[[ -n "$spec" ]] || die "gh issue view $number gave nothing"

if [[ -n "$out" ]]; then
  printf '%s\n' "$spec" > "$out"
  echo "issue-spec: #$number written to $out" >&2
else
  printf '%s\n' "$spec"
fi
