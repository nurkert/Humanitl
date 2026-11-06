#!/usr/bin/env bash
# Welche Jobs von .github/workflows/ci.yml ein Push braucht (HUM-424).
#
# Aufruf:
#   tools/ci-changes.sh [--main] --full <head>
#   tools/ci-changes.sh [--main] --base <commit> <head>
#   tools/ci-changes.sh [--main] --merge-base-with <commit> <head>
#
# Der Job `changes` in ci.yml ruft dieses Skript auf und gibt seine Zeilen
# `schluessel=wert` als Ausgaben an die anderen Jobs weiter. Jeder schwere Job
# laeuft nur, wenn seine Zeile `true` sagt. Die Regeln stehen nur hier, damit
# Workflow, Tor (tools/ci-gate.sh) und Test dieselbe Wahrheit sehen.
#
# Verglichen wird `git diff --name-only <basis> <head>`:
#   --base            die Basis selbst, auf main der vorige Stand
#                     (github.event.before). Ist sie leer, lauter Nullen, kein
#                     Commit oder kein Vorfahr von <head> (erster Push, Push mit
#                     Gewalt), laeuft alles.
#   --merge-base-with die gemeinsame Basis mit diesem Commit, auf Issue-Zweigen
#                     origin/main. Fehlt sie, laeuft alles.
#   --full            alles, ohne Vergleich (workflow_dispatch mit full=true).
#   --main            der Lauf gehoert zu main: Dort laeuft macOS fuer jede
#                     Aenderung am Code, auf Zweigen nur fuer die macOS-nahen.
#
# Bereiche, je geaenderter Datei der erste Treffer:
#   none   website/**: Kein Job von ci.yml liest sie; die Seite baut
#          .github/workflows/website.yml.
#   backlog backlog/**, BACKLOG.md: nur rust-test, denn
#          daemon/crates/config/tests/config_readers.rs liest die Sprint-Dateien
#          und BACKLOG.md.
#   docs   docs/**, images/**, LICENSE, *.md im Wurzelverzeichnis. Die Tests
#          von Daemon und App lesen Seiten aus docs/ (CONFIG.md, INSTALL.md,
#          DIAGNOSTICS.md, cli.md, reference/), parity-check vergleicht die
#          erzeugten Seiten; diese Jobs laufen deshalb auch hier.
#   app    app/**
#   rust   daemon/**, tests/**
#   beide  proto/**, catalog/**, rules/**, profiles/**, agents/**,
#          fixtures/**, packaging/**: Daemon und App lesen sie.
#   full   tools/**, scripts/**, .github/**, Makefile, CHANGELOG.md und jeder
#          Pfad, den keine Regel kennt (auch Punktdateien im
#          Wurzelverzeichnis). CHANGELOG.md, weil der Commit eines Releases oft
#          nur ihn aendert und tools/release.sh einen vollen Lauf verlangt.
#
# Was welcher Bereich startet, steht in der Liste `jobs` unten. lint und
# changes selbst laufen immer.
#
# macOS-nah (Bereich area-macos) ist eine Datei unter app/, daemon/, tests/
# oder packaging/, deren Pfad macos, darwin, seatbelt oder launchd enthaelt,
# ein Manifest oder Lockfile (Cargo.toml, Cargo.lock, rust-toolchain.toml,
# daemon/.cargo/, pubspec.yaml, pubspec.lock, app/.fvmrc), oder eine Datei,
# deren alter oder neuer Inhalt eines der Merkmale in `mac_marker` traegt.
# Das ist eine Schaetzung zugunsten des Laufs: Der Job macos auf main faengt
# auf, was sie verfehlt, und der Probelauf von release.yml baut macOS vor
# jedem Tag.
set -euo pipefail

usage() {
  echo "usage: tools/ci-changes.sh [--main] (--full | --base <commit> | --merge-base-with <commit>) <head>" >&2
  exit 2
}

on_main=0
mode=""
arg=""
while [[ $# -gt 1 ]]; do
  case "$1" in
    --main) on_main=1; shift ;;
    --full) mode=full; shift ;;
    --base | --merge-base-with)
      [[ $# -ge 3 ]] || usage
      mode="${1#--}"
      arg="$2"
      shift 2
      ;;
    *) usage ;;
  esac
done
[[ $# -eq 1 && -n "$mode" ]] || usage
head="$(git rev-parse --verify --quiet "$1^{commit}")" || { echo "ci-changes: $1 is not a commit" >&2; exit 2; }

# Merkmale fuer Code, der nur auf macOS anders laeuft oder anders uebersetzt.
mac_marker='macos|darwin|seatbelt|launchd|apple|target_os|target_vendor|cfg\(unix|cfg\(not\(unix|cfg!\(unix'

full=0
why=""
base=""
case "$mode" in
  full) full=1; why="full run requested" ;;
  base)
    if [[ -z "$arg" || "$arg" =~ ^0+$ ]]; then
      full=1; why="no previous commit (first push)"
    elif ! base="$(git rev-parse --verify --quiet "$arg^{commit}")"; then
      full=1; why="previous commit $arg unknown"
    elif ! git merge-base --is-ancestor "$base" "$head"; then
      full=1; why="previous commit ${arg:0:12} is not an ancestor (force push)"
    fi
    ;;
  merge-base-with)
    if ! other="$(git rev-parse --verify --quiet "$arg^{commit}")" \
      || ! base="$(git merge-base "$other" "$head")"; then
      full=1; why="no merge base with $arg"
    fi
    ;;
esac

declare -A area=([rust]=0 [app]=0 [docs]=0 [backlog]=0 [macos]=0)
changed=0
mac_why=""

# Jeder Fehler von git endet in einem vollen Lauf, nie in einem stillen
# Auslassen (fail closed): Ein `git diff`, das an einer fehlenden Basis
# scheitert, haette sonst eine leere Liste geliefert und jeden Job
# uebersprungen.
fall_back() {
  full=1
  why="$1"
  echo "::warning title=ci-changes::$1; running everything" >&2
}

# Inhalt einer Datei in einem Commit, leer wenn sie dort fehlt. Exit 1, wenn
# git ihn nicht liefern kann (etwa ein Blob, den ein partieller Klon nicht
# nachladen kann); das Fehlen der Datei klaert ls-tree, das nur Baeume liest.
blob() {
  local listed
  listed="$(git ls-tree --name-only "$1" -- "$2")" || return 1
  [[ -n "$listed" ]] || return 0
  git cat-file -p "$1:$2" || return 1
}

mac_check() {
  local f="$1"
  ((area[macos])) && return 0
  case "$f" in
    *[Mm][Aa][Cc][Oo][Ss]* | *[Dd]arwin* | *[Ss]eatbelt* | *[Ll]aunchd* \
      | */Cargo.toml | */Cargo.lock | daemon/rust-toolchain.toml | daemon/.cargo/* \
      | */pubspec.yaml | */pubspec.lock | app/.fvmrc)
      area[macos]=1; mac_why="$f"; return 0 ;;
  esac
  # Nur Code und Bau-Dateien: Ein ARB-Text oder ein Golden, das macOS nennt,
  # uebersetzt sich auf macOS nicht anders.
  case "$f" in
    *.rs | *.dart | *.swift | *.m | *.mm | *.h | *.c | *.cc | *.cpp | *.sh | *.py \
      | *.toml | *.plist | *.xcconfig | *.entitlements | *.pbxproj | *.cmake | CMakeLists.txt | */CMakeLists.txt) ;;
    *) return 0 ;;
  esac
  local text="" more
  if [[ -n "$base" ]]; then
    text="$(blob "$base" "$f")" || { fall_back "cannot read $f in ${base:0:12}"; return 0; }
  fi
  more="$(blob "$head" "$f")" || { fall_back "cannot read $f in ${head:0:12}"; return 0; }
  if grep -qiE "$mac_marker" <<< "$text"$'\n'"$more"; then
    area[macos]=1
    mac_why="$f (content)"
  fi
}

files=""
if ((!full)); then
  # Erst die ganze Liste, dann ihr Exit-Code: Eine Prozess-Substitution
  # verschluckte ihn.
  files="$(git diff --no-renames --name-only "$base" "$head")" \
    || fall_back "git diff ${base:0:12} ${head:0:12} failed"
fi
if ((!full)); then
  while IFS= read -r f; do
    [[ -n "$f" ]] || continue
    changed=$((changed + 1))
    case "$f" in
      website/*) ;;
      backlog/* | BACKLOG.md) area[backlog]=1 ;;
      docs/* | images/* | LICENSE) area[docs]=1 ;;
      app/*) area[app]=1; mac_check "$f" ;;
      daemon/* | tests/*) area[rust]=1; mac_check "$f" ;;
      proto/* | catalog/* | rules/* | profiles/* | agents/* | fixtures/*)
        area[rust]=1; area[app]=1 ;;
      packaging/*) area[rust]=1; area[app]=1; mac_check "$f" ;;
      tools/* | scripts/* | .github/* | Makefile | CHANGELOG.md)
        full=1; why="${why:-pipeline file $f}" ;;
      # Ein Glob in `case` sieht ueber `/` hinweg: Erst ein unbekanntes
      # Verzeichnis abfangen, dann bleibt fuer *.md nur das Wurzelverzeichnis.
      */*) full=1; why="${why:-unknown path $f}" ;;
      *.md) area[docs]=1 ;;
      *) full=1; why="${why:-unknown path $f}" ;;
    esac
  done <<< "$files"
fi

if ((full)); then
  area[rust]=1 area[app]=1 area[docs]=1 area[backlog]=1 area[macos]=1
fi
rust=${area[rust]} app=${area[app]} docs=${area[docs]} backlog=${area[backlog]} mac=${area[macos]}
code=$((rust || app))

b() { (($1)) && echo true || echo false; }

# Die Jobs, in der Reihenfolge von ci.yml. Wer einen Job hinzufuegt, ergaenzt
# ihn hier, im Job `changes` (Ausgabe und Schritt `skip <job>`) und am Job
# selbst; tools/tests/ci_plan_test.py prueft, dass die drei zusammenpassen.
if ((on_main)); then
  macos_job=$code
else
  macos_job=$mac
fi
jobs=(
  "rust-check=$(b "$rust")"
  "rust-test=$(b "$((rust || docs || backlog))")"
  "macos=$(b "$((full || macos_job))")"
  "deps-lint=$(b "$((code || docs))")"
  "proto-lint-and-gen=$(b "$code")"
  "flutter-analyze-test=$(b "$((code || docs))")"
  "goldens=$(b "$app")"
  "escape-tests=$(b "$rust")"
  "e2e=$(b "$rust")"
  "e2e-xvfb=$(b "$code")"
  "e2e-agent=$(b "$rust")"
  "e2e-m4=$(b "$code")"
  "e2e-m4-package=$(b "$code")"
  "parity-check=$(b "$((code || docs))")"
)

echo "full=$(b "$full")"
echo "area-rust=$(b "$rust")"
echo "area-app=$(b "$app")"
echo "area-docs=$(b "$docs")"
echo "area-backlog=$(b "$backlog")"
echo "area-macos=$(b "$mac")"
printf '%s\n' "${jobs[@]}"

{
  if ((full)); then
    echo "ci-changes: everything runs: $why"
  else
    echo "ci-changes: $changed file(s) changed since ${base:0:12}"
    echo "ci-changes: rust=$(b "$rust") app=$(b "$app") docs=$(b "$docs") backlog=$(b "$backlog") macos=$(b "$mac")${mac_why:+ (first macOS-relevant: $mac_why)}"
  fi
  for j in "${jobs[@]}"; do
    [[ "${j#*=}" == true ]] || echo "ci-changes: skip ${j%=*}"
  done
} >&2
