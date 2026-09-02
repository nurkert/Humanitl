#!/usr/bin/env bash
# Verify the ADR directory: numbering, index completeness, MADR headings.
#
# Checks (HUM-009):
#   1. every Markdown file except README.md is named NNNN-kebab-title.md
#   2. numbers start at 0001, are contiguous and unique (0000 is the template)
#   3. the template and the index exist
#   4. every ADR file has a row in README.md, and every row points at a file
#   5. every ADR has the seven MADR headings plus Status and Datum
#   6. every ADR names at least one HUM- issue
#
# Usage:
#   docs/adr/check.sh              check this directory
#   docs/adr/check.sh DIR          check another directory (used by --self-test)
#   docs/adr/check.sh --self-test  prove that every check fails on a broken copy
#
# Meant to be called from scripts/ci/lint-docs.sh (the HUM-007 extension
# point), so that `make docs-lint` covers the ADR directory. Runs standalone too.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fail=0

err() {
  echo "check-adr: $*" >&2
  fail=1
}

check_dir() {
  local adr_dir="$1"
  local index="$adr_dir/README.md"
  local template="$adr_dir/0000-template.md"

  [[ -f "$template" ]] || err "missing template: 0000-template.md"
  [[ -f "$index" ]] || err "missing index: README.md"

  # --- collect ADR files -----------------------------------------------------
  # Every Markdown file except the index is a candidate, so a misnamed file
  # (0011_Bad_Name.md, 19-foo.md, adr-nineteen.md) is reported instead of being
  # skipped by a narrow glob.
  local files=() path name
  for path in "$adr_dir"/*.md; do
    [[ -e "$path" ]] || continue
    name="${path##*/}"
    [[ "$name" == "README.md" ]] && continue
    files+=("$name")
  done

  if [[ ${#files[@]} -eq 0 ]]; then
    err "no ADR files found in $adr_dir"
    return 1
  fi

  local adrs=() f
  for f in "${files[@]}"; do
    if [[ ! "$f" =~ ^[0-9]{4}-[a-z0-9]+(-[a-z0-9]+)*\.md$ ]]; then
      err "bad file name (want NNNN-kebab-title.md): $f"
      continue
    fi
    [[ "$f" == "0000-template.md" ]] && continue
    adrs+=("$f")
  done

  # --- numbering: contiguous from 0001, no duplicates ------------------------
  local -A seen=()
  local highest=0 num n
  for f in "${adrs[@]}"; do
    num="${f:0:4}"
    if [[ -n "${seen[$num]:-}" ]]; then
      err "duplicate ADR number $num: ${seen[$num]} and $f"
    else
      seen[$num]="$f"
    fi
    n=$((10#$num))
    if [[ "$n" -eq 0 ]]; then
      err "$f: 0000 is reserved for the template"
    fi
    [[ "$n" -gt "$highest" ]] && highest="$n"
  done

  for ((n = 1; n <= highest; n++)); do
    num="$(printf '%04d' "$n")"
    [[ -n "${seen[$num]:-}" ]] || err "missing ADR number $num (numbers must be contiguous from 0001)"
  done

  # --- index completeness ----------------------------------------------------
  for f in "${adrs[@]}"; do
    grep -q "($f)" "$index" || err "not listed in README.md: $f"
  done

  local linked
  while IFS= read -r linked; do
    [[ "$linked" == "0000-template.md" ]] && continue
    [[ -f "$adr_dir/$linked" ]] || err "README.md links a missing file: $linked"
  done < <(grep -o '([0-9][0-9][0-9][0-9]-[a-z0-9-]*\.md)' "$index" | tr -d '()' | sort -u)

  # --- per-file structure ----------------------------------------------------
  local headings=("## Kontext" "## Entscheidung" "## Begründung" \
                  "## Verworfene Alternativen" "## Konsequenzen" "## Betroffene Issues")
  local h
  for f in "${adrs[@]}" "0000-template.md"; do
    path="$adr_dir/$f"
    [[ -f "$path" ]] || continue

    head -n1 "$path" | grep -qE '^# ADR-([0-9]{4}|NNNN) · .+' \
      || err "$f: first line must be '# ADR-NNNN · Titel'"

    grep -qE '^Status: .+' "$path" || err "$f: missing 'Status:' line"
    grep -qE '^Datum: .+' "$path" || err "$f: missing 'Datum:' line"

    for h in "${headings[@]}"; do
      grep -qxF "$h" "$path" || err "$f: missing heading '$h'"
    done

    if [[ "$f" != "0000-template.md" ]]; then
      num="${f:0:4}"
      head -n1 "$path" | grep -q "ADR-$num" \
        || err "$f: title number does not match the file name"
      grep -q 'HUM-[0-9]\{3\}' "$path" || err "$f: names no HUM- issue"
      grep -qE '^Status: (Accepted|Superseded by ADR-[0-9]{4}|Deprecated)$' "$path" \
        || err "$f: Status must be Accepted, 'Superseded by ADR-NNNN' or Deprecated"
    fi
  done

  if [[ "$fail" -eq 0 ]]; then
    echo "check-adr: ${#adrs[@]} ADRs, template and index are consistent"
  fi
  return "$fail"
}

# --- self-test ---------------------------------------------------------------
# Each case copies the real directory, breaks exactly one thing and expects the
# named error. The unmodified copy must pass. Mutations run inside the copy.

highest_adr() {
  ls [0-9][0-9][0-9][0-9]-*.md | grep -v '^0000-' | sort | tail -n1
}

mut_none()             { :; }
mut_underscore()       { touch 0011_Bad_Name.md; }
mut_two_digits()       { touch 19-foo.md; }
mut_no_number()        { touch adr-nineteen.md; }
mut_uppercase()        { touch 0099-Bad_Name.md; }
mut_duplicate()        { cp 0001-*.md 0001-duplicate.md; }
mut_ghost_link()       { printf '| [0098](0098-ghost.md) | Ghost | Accepted | x |\n' >> README.md; }
mut_missing_heading()  { sed -i '/^## Konsequenzen$/d' 0001-*.md; }
mut_lowercase_status() { sed -i 's/^Status: Accepted$/Status: accepted/' 0001-*.md; }
mut_title_mismatch()   { sed -i '1s/ADR-0001/ADR-0002/' 0001-*.md; }
mut_no_issue()         { sed -i 's/HUM-[0-9]\{3\}/HUM-x/g' 0001-*.md; }

# Move the highest ADR two numbers up, keeping title and index consistent, so
# the only defect is the gap.
mut_gap() {
  local hi num rest new
  hi="$(highest_adr)"
  num="${hi:0:4}"
  rest="${hi#*-}"
  new="$(printf '%04d' $((10#$num + 2)))"
  mv "$hi" "$new-$rest"
  sed -i "1s/ADR-$num/ADR-$new/" "$new-$rest"
  sed -i "s/$hi/$new-$rest/g; s/\[$num\]/[$new]/g" README.md
}

# Add a well-formed next ADR that the index does not know.
mut_unlisted() {
  local hi num new
  hi="$(highest_adr)"
  num="${hi:0:4}"
  new="$(printf '%04d' $((10#$num + 1)))"
  sed "1s/ADR-$num/ADR-$new/" "$hi" > "$new-unlisted.md"
}

run_case() {
  local name="$1" want="$2" mutation="$3"
  local dir="$tmp/$name" out rc=0
  cp -r "$here" "$dir"
  (cd "$dir" && "$mutation")
  out="$(bash "${BASH_SOURCE[0]}" "$dir" 2>&1)" || rc=$?
  if [[ -z "$want" ]]; then
    if [[ "$rc" -ne 0 ]]; then
      echo "self-test: $name: expected pass, got exit $rc:" >&2
      echo "$out" >&2
      failures=1
    fi
  elif [[ "$rc" -eq 0 ]]; then
    echo "self-test: $name: expected failure, got pass" >&2
    failures=1
  elif [[ "$out" != *"$want"* ]]; then
    echo "self-test: $name: expected message containing '$want', got:" >&2
    echo "$out" >&2
    failures=1
  fi
}

self_test() {
  local failures=0
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT

  run_case unmodified       ''                                    mut_none
  run_case underscore-name  'bad file name'                       mut_underscore
  run_case two-digit-number 'bad file name'                       mut_two_digits
  run_case no-number        'bad file name'                       mut_no_number
  run_case uppercase-title  'bad file name'                       mut_uppercase
  run_case duplicate-number 'duplicate ADR number 0001'           mut_duplicate
  run_case gap-in-numbering 'missing ADR number'                  mut_gap
  run_case unlisted-file    'not listed in README.md'             mut_unlisted
  run_case ghost-link       'links a missing file: 0098-ghost.md' mut_ghost_link
  run_case missing-heading  "missing heading '## Konsequenzen'"   mut_missing_heading
  run_case lowercase-status 'Status must be'                      mut_lowercase_status
  run_case title-mismatch   'title number does not match'         mut_title_mismatch
  run_case no-issue         'names no HUM- issue'                 mut_no_issue

  if [[ "$failures" -eq 0 ]]; then
    echo "check-adr: self-test ok (13 cases)"
  fi
  return "$failures"
}

case "${1:-}" in
  --self-test) self_test ;;
  "")          check_dir "$here" ;;
  *)           check_dir "$(cd "$1" && pwd)" ;;
esac
