#!/usr/bin/env bash
# Documentation gate for the security documents (HUM-007).
# Hooked into `make check` locally and into the CI job `rust-check`.
#
# It enforces three things:
#   1. docs/SECURITY.md and docs/THREAT-MODEL.md exist and are not empty.
#   2. Neither of them contains a TODO or TBD line. A security document that
#      admits an unwritten section is not a document, it is a promise.
#   3. Every ESC-N referenced in them names a real escape test under
#      tests/escape/ (file names per backlog/CONVENTIONS.md 3.11).
#
# Since HUM-059 it also enforces, for the release documentation:
#   4. every page under docs/ starts with an H1 and the line
#      "Applies to: Humanitl 0.1" on line 3,
#   5. README.md carries the twelve mandatory sections in order,
#   6. SECURITY.md and docs/SECURITY.md carry the same reporting line,
#   7. every relative link and #fragment in README*.md, SECURITY.md and docs/
#      resolves (scripts/ci/check-doc-links.py, offline).
#
# Extension point: HUM-009 adds the ADR heading check to this script.
set -euo pipefail
cd "$(dirname "$0")/../.."

DOCS=(docs/SECURITY.md docs/THREAT-MODEL.md)
ESCAPE_DIR="tests/escape"
ESC_MIN=1
ESC_MAX=5
fail=0

for doc in "${DOCS[@]}"; do
  if [[ ! -s "$doc" ]]; then
    echo "lint-docs: missing or empty: $doc" >&2
    fail=1
    continue
  fi
  if grep -nE '(TODO|TBD)' "$doc" >&2; then
    echo "lint-docs: placeholder above in $doc; write the section out" >&2
    fail=1
  fi
done

# ADR-Verzeichnis: Nummern lückenlos, Index vollständig (HUM-009).
if ! docs/adr/check.sh; then fail=1; fi

if ((fail == 0)); then
  refs="$(grep -hoE 'ESC-[0-9]+' "${DOCS[@]}" | sort -u || true)"
  harness=0
  if [[ -d "$ESCAPE_DIR" ]]; then
    harness="$(find "$ESCAPE_DIR" -maxdepth 1 -name 'esc-*.sh' | wc -l)"
  fi

  for ref in $refs; do
    n="${ref#ESC-}"
    if ((10#$n < ESC_MIN || 10#$n > ESC_MAX)); then
      echo "lint-docs: unknown escape test $ref (known: ESC-$ESC_MIN..ESC-$ESC_MAX, CONVENTIONS 3.11)" >&2
      fail=1
      continue
    fi
    if ((harness > 0)) && ! compgen -G "$ESCAPE_DIR/esc-$n-*.sh" >/dev/null; then
      echo "lint-docs: $ref is referenced but $ESCAPE_DIR/esc-$n-*.sh does not exist" >&2
      fail=1
    fi
  done

  if ((harness == 0)); then
    echo "SKIP lint-docs escape files: $ESCAPE_DIR has no esc-*.sh yet (HUM-006); IDs were still range-checked"
  fi
fi

# Der gebuendelte Domain-Katalog gegen sein Schema (HUM-031). Die Datei wird
# von zwei Seiten gelesen, vom Daemon und als Asset von der Oberflaeche; ein
# Tippfehler darin faellt sonst erst im Panel auf.
#
# `check-jsonschema` ist optional wie rustfmt und clippy im Makefile: fehlt es,
# sagt das Skript das laut und laeuft weiter. Ein stiller Durchlauf waere
# schlimmer als eine Luecke, die man sieht. Damit die Pruefung in CI wirklich
# bindet, muss der Workflow das Werkzeug installieren
# (`pipx install check-jsonschema`); bis dahin steht dort der Hinweis.
CATALOG_SCHEMA="catalog/domains.schema.json"
CATALOG_FILE="catalog/domains.yaml"
if [[ -f "$CATALOG_SCHEMA" && -f "$CATALOG_FILE" ]]; then
  if command -v check-jsonschema >/dev/null 2>&1; then
    if check-jsonschema --schemafile "$CATALOG_SCHEMA" "$CATALOG_FILE"; then
      echo "lint-docs: $CATALOG_FILE matches $CATALOG_SCHEMA"
    else
      echo "lint-docs: $CATALOG_FILE does not match $CATALOG_SCHEMA" >&2
      fail=1
    fi
  else
    echo "SKIP lint-docs catalog schema: check-jsonschema not found (pipx install check-jsonschema); $CATALOG_FILE was NOT validated"
    if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
      echo "::notice::lint-docs skipped the catalog schema check: check-jsonschema is not installed on this runner"
    fi
  fi
fi

# Dokumentation zum Release (HUM-059).
#
# 1. Jede Seite unter docs/ beginnt mit einer H1 und nennt in Zeile 3 den
#    Stand, für den sie gilt. Eine Seite ohne diese Zeile ist eine, die
#    niemand gegen den Code abgeglichen hat.
APPLIES_TO="Applies to: Humanitl 0.1"
while IFS= read -r -d '' page; do
  first="$(sed -n 1p "$page")"
  third="$(sed -n 3p "$page")"
  if [[ "$first" != "# "* ]]; then
    echo "lint-docs: $page: line 1 must be an H1 ('# Title')" >&2
    fail=1
  fi
  if [[ "$third" != "$APPLIES_TO" ]]; then
    echo "lint-docs: $page: line 3 must be '$APPLIES_TO'" >&2
    fail=1
  fi
done < <(find docs -name '*.md' -print0 | sort -z)

# 2. Das README hat die zwölf Pflichtabschnitte, in dieser Reihenfolge und
#    ohne weitere Abschnitte der zweiten Ebene dazwischen. Die ersten beiden,
#    Einzeiler und Screenshot, stehen vor der ersten Überschrift der zweiten
#    Ebene; die übrigen zehn sind diese Überschriften.
ONE_LINER="Humanitl lets a sandboxed AI coding agent use the internet, one approved request at a time."
head_part="$(sed '/^## /,$d' README.md)"
if [[ "$head_part" != *"$ONE_LINER"* ]]; then
  echo "lint-docs: README.md: the one-liner must stand before the first section" >&2
  fail=1
fi
if [[ "$head_part" != *"(docs/img/intercept.png)"* ]]; then
  echo "lint-docs: README.md: the screenshot docs/img/intercept.png must stand before the first section" >&2
  fail=1
fi
README_SECTIONS=$'Why\nThe three guarantees\nWhat it does not protect against\nInstall\nFirst session\nCLI\nHow it works\nStatus\nContributing\nLicense'
actual="$(grep '^## ' README.md | sed 's/^## //')"
if [[ "$actual" != "$README_SECTIONS" ]]; then
  echo "lint-docs: README.md sections differ from the HUM-059 list:" >&2
  diff <(printf '%s\n' "$README_SECTIONS") <(printf '%s\n' "$actual") >&2 || true
  fail=1
fi

# 3. Der Meldeweg steht im Root-SECURITY.md (GitHub-Konvention) und in
#    docs/SECURITY.md wortgleich.
REPORT_LINE="Contact: see GitHub security advisories for this repository (private vulnerability reporting)."
for doc in SECURITY.md docs/SECURITY.md; do
  if ! grep -qF "$REPORT_LINE" "$doc" 2>/dev/null; then
    echo "lint-docs: $doc: missing the reporting line '$REPORT_LINE'" >&2
    fail=1
  fi
done

# 3b. Der Prüfbefehl der zweiten Garantie ist in README und SECURITY derselbe
#     wie `esc_find_sockets` in tests/escape/lib.sh: `-xtype s` statt `-type s`
#     (der gebundene Socket liegt über einer leeren Datei) und kein `-xdev`
#     (/work, /tmp, /dev/shm sind eigene Mounts). Ein Befehl, der in der
#     Sandbox nichts findet, wäre ein Beleg, der nichts belegt.
FIND_SOCKETS="find / \\( -path /proc -o -path /sys -o \\( -path '/dev/*' ! -path /dev/shm ! -path '/dev/shm/*' \\) \\) -prune -o -xtype s -print 2>/dev/null"
for doc in README.md README.de.md docs/SECURITY.md; do
  if ! grep -qF -- "$FIND_SOCKETS" "$doc"; then
    echo "lint-docs: $doc: the guarantee-2 check must be the esc_find_sockets form: $FIND_SOCKETS" >&2
    fail=1
  fi
  if grep -nE 'find /[^`]*(-xdev|[^x]-type s)' "$doc" >&2; then
    echo "lint-docs: $doc: a socket search with -xdev or -type s above finds nothing in the sandbox" >&2
    fail=1
  fi
done

# 4. Relative Links und Anker in README und docs/ zeigen auf etwas, das es
#    gibt. Erst der Selbsttest, damit ein kaputter Prüfer nicht grün meldet.
if ! python3 -I scripts/ci/check-doc-links.py --self-test; then fail=1; fi
if ! python3 -I scripts/ci/check-doc-links.py; then fail=1; fi

if ((fail == 0)); then
  echo "lint-docs: ok (${#DOCS[@]} documents)"
fi
exit "$fail"
