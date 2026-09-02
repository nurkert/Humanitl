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

if ((fail == 0)); then
  echo "lint-docs: ok (${#DOCS[@]} documents)"
fi
exit "$fail"
