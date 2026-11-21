#!/usr/bin/env bash
# Test fuer tools/issue-spec.sh mit einem nachgestellten gh (HUMANITL_GH).
#
# Der Stub nimmt den Ausdruck, den das Skript an `--jq` gibt, und wendet ihn
# mit `jq -r` auf ein festes JSON an, so wie gh es mit der Antwort von GitHub
# taete. Geprueft wird die Ausgabe nach stdout und mit -o, die Labels, ein
# Issue ohne Labels und die Ablehnung falscher Argumente.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
script="$here/../issue-spec.sh"

command -v jq > /dev/null || {
  echo "issue_spec_test: jq fehlt" >&2
  exit 1
}

work="$(mktemp -d "${TMPDIR:-/tmp}/issue-spec-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT
fails=0

cat > "$work/gh" << 'EOF'
#!/usr/bin/env bash
set -euo pipefail
echo "$*" >> "$STUB_DIR/calls"
[[ "$1 $2" == "issue view" ]] || exit 2
[[ -f "$STUB_DIR/issue-$3.json" ]] || { echo "could not resolve to an Issue" >&2; exit 1; }
[[ "$4" == --json && "$6" == --jq ]] || exit 2
jq -r "$7" "$STUB_DIR/issue-$3.json"
EOF
chmod +x "$work/gh"

cat > "$work/issue-12.json" << 'EOF'
{"number": 12, "title": "Start Burp with the right settings",
 "labels": [{"name": "type:feature"}, {"name": "area:cli"}],
 "body": "Formerly HUM-584\n\n### Context\n\nText with `code` and \"quotes\"."}
EOF
cat > "$work/issue-13.json" << 'EOF'
{"number": 13, "title": "No labels", "labels": [], "body": null}
EOF

run() { STUB_DIR="$work" HUMANITL_GH="$work/gh" "$script" "$@"; }
check() {
  local name="$1"
  shift
  if "$@" > /dev/null 2>&1; then
    echo "ok   $name"
  else
    echo "FAIL $name"
    fails=$((fails + 1))
  fi
}
not() { ! "$@"; }
same() { [[ "$1" == "$2" ]]; }

expected='# #12: Start Burp with the right settings

Labels: `type:feature`, `area:cli`

Formerly HUM-584

### Context

Text with `code` and "quotes".'

got="$(run 12)"
check stdout_rendered same "$got" "$expected"
check asks_for_four_fields grep -qF "issue view 12 --json title,body,labels,number --jq" "$work/calls"

run 12 -o "$work/spec.md" 2> "$work/err"
check file_rendered same "$(cat "$work/spec.md")" "$expected"
check file_names_target grep -qF "$work/spec.md" "$work/err"

check no_labels_says_none same "$(run 13)" $'# #13: No labels\n\nLabels: none'

for bad in "" "abc" "0" "12 13" "-o"; do
  # shellcheck disable=SC2086 # die Faelle sind absichtlich Wortlisten
  check "refuses [$bad]" not run $bad
done
check unknown_issue_fails not run 99
run 99 -o "$work/none.md" 2> /dev/null || true
check unknown_issue_writes_no_file not test -e "$work/none.md"

if [[ "$fails" -ne 0 ]]; then
  echo "issue_spec_test: $fails Pruefung(en) rot"
  exit 1
fi
echo "issue_spec_test: alle Pruefungen gruen"
