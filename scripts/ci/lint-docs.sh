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

if ((fail == 0)); then
  echo "lint-docs: ok (${#DOCS[@]} documents)"
fi
exit "$fail"
