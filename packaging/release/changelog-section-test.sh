#!/usr/bin/env bash
# Test fuer changelog-section.sh (HUM-060): Fuer eine Version kommt genau ihr
# Abschnitt aus dem Changelog, ohne Ueberschrift; ein Entwurf ohne Datum, ein
# falsches Datum, ein leerer oder doppelter Abschnitt brechen ab. Dazu die
# Form der echten CHANGELOG.md: Sie beginnt mit `## [Unreleased]`.
#
# Aufruf: packaging/release/changelog-section-test.sh
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
script="$here/changelog-section.sh"

work="$(mktemp -d "${TMPDIR:-/tmp}/changelog-section-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT

failures=0
fail() {
  echo "FAIL: $*" >&2
  failures=$((failures + 1))
}

cat >"$work/changelog.md" <<'EOF'
# Changelog

## [Unreleased]

- unreleased line

## [0.1.10] - 2027-01-02

- the wrong version

## [0.1.1] - 2026-12-01

### Fixed

- a fix

```text
## this heading is in a code block and does not end the section
```

## [0.1.0] - 2026-11-20

- first

[0.1.1]: https://example.invalid/v0.1.1
EOF

got="$("$script" 0.1.1 "$work/changelog.md")" || fail "0.1.1 is refused"
expected="$(
  cat <<'EOF'
### Fixed

- a fix

```text
## this heading is in a code block and does not end the section
```
EOF
)"
if [[ "$got" != "$expected" ]]; then
  fail "the section 0.1.1 differs (diff: expected, actual)"
  diff <(echo "$expected") <(echo "$got") >&2 || true
fi

got="$("$script" 0.1.0 "$work/changelog.md")" || fail "0.1.0 is refused"
[[ "$got" == "- first" ]] || fail "the last section runs into the link references: '$got'"

refuses() { # <name> <version> <changelog-text>
  printf '%s\n' "$3" >"$work/case.md"
  if "$script" "$2" "$work/case.md" >/dev/null 2>&1; then
    fail "$1: accepted"
  fi
}
refuses missing 0.2.0 "$(cat "$work/changelog.md")"
refuses draft 0.1.0 $'## [0.1.0] - unreleased\n\n- first'
refuses no_date 0.1.0 $'## [0.1.0]\n\n- first'
refuses bad_date 0.1.0 $'## [0.1.0] - 2026-13-01\n\n- first'
refuses no_entry 0.1.0 $'## [0.1.0] - 2026-11-20\n\nNothing yet.'
refuses twice 0.1.0 $'## [0.1.0] - 2026-11-20\n\n- a\n\n## [0.1.0] - 2026-11-21\n\n- b'
refuses signed_only 1.0.0 $'## [1.0.0] - 2026-11-20\n\n- a'
refuses bad_version 0.1 $'## [0.1] - 2026-11-20\n\n- a'

# Die echte Datei: Der erste Abschnitt ist [Unreleased], damit jede Aenderung
# einen Platz hat, bevor ihre Version feststeht.
first="$(grep -m 1 '^## \[' "$root/CHANGELOG.md" || true)"
[[ "$first" == "## [Unreleased]" ]] || fail "CHANGELOG.md does not start with '## [Unreleased]': '$first'"

if [[ "$failures" -gt 0 ]]; then
  echo "changelog-section-test: $failures failure(s)" >&2
  exit 1
fi
echo "changelog-section-test: ok"
