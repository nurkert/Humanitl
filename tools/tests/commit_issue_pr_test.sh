#!/usr/bin/env bash
# Test fuer `tools/commit-issue.sh --pr` in einem Wegwerf-Repository.
#
# Ein nacktes Repository spielt `origin`, ein nachgestelltes CI-Tor
# (HUMANITL_CI_GATE) und ein nachgestelltes gh (HUMANITL_GH) spielen GitHub.
# Der gh-Stub merkt sich den Pull Request, schreibt jeden Aufruf mit und
# merged wie GitHub mit Auto-Merge: nur, wenn Auto-Merge verlangt ist, das
# Tor fuer den Kopf des Zweigs gruen war, der Zweig aktuell ist (main ist der
# Elternteil des Kopfes) und, wenn mit --match-head-commit verlangt, der Kopf
# noch genau dieser Commit ist.
#
# Geprueft werden die Argumente, der Koerper des Pull Requests (`Closes #N`
# und die Zusammenfassung des Reviews), das Auto-Merge auf genau den
# getesteten Commit, rot, ein bewegtes main, ein Auto-Merge, das nicht
# merged, und der Probelauf des Releases.
#
# Mutationsbeweis: COMMIT_ISSUE=<pfad> prueft eine veraenderte Kopie. Ohne
# `--match-head-commit "$sha"` im Skript wird der Test rot.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
script="${COMMIT_ISSUE:-$here/../commit-issue.sh}"
template="$here/../../.github/pull_request_template.md"

work="$(mktemp -d "${TMPDIR:-/tmp}/commit-issue-pr-test.XXXXXX")"
fails=0
norules=0
cleanup() {
  if [[ "$fails" -eq 0 ]]; then
    rm -rf "$work"
  else
    for out in "$work"/*/out*; do
      echo "--- $out"
      cat "$out"
    done
    echo "commit_issue_pr_test: Baum bleibt in $work"
  fi
}
trap cleanup EXIT

export GIT_AUTHOR_NAME=Test GIT_AUTHOR_EMAIL=test@example.invalid
export GIT_COMMITTER_NAME=Test GIT_COMMITTER_EMAIL=test@example.invalid
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export HUMANITL_PR_POLL=0 HUMANITL_PR_MERGE_TIMEOUT=30

# Das Tor: Exit-Code aus $D/rc (eine Zeile je Aufruf, die letzte bleibt
# stehen), jede SHA nach $D/shas, eine Zeile nach $D/seq. Ist ein Lauf von
# ci.yml fuer das Ereignis pull_request gruen, steht die SHA in $D/green (die
# Pruefung ci-ok am Pull Request auf GitHub). Liegt $D/race, legt der
# erste Aufruf vorher einen fremden Commit auf origin/main.
cat > "$work/gate" << 'EOF'
#!/usr/bin/env bash
set -euo pipefail
D="$STUB_DIR"
echo "$1" >> "$D/shas"
n=$(wc -l < "$D/shas")
wf="${HUMANITL_CI_WORKFLOW:-ci.yml}" ev="${HUMANITL_CI_EVENT:-push}"
echo "gate workflow=$wf branch=${HUMANITL_CI_BRANCH:-} event=$ev $1" >> "$D/seq"
if [[ -f "$D/race" && "$n" -eq 1 ]]; then
  git clone -q "$D/origin.git" "$D/other"
  echo other > "$D/other/other.txt"
  git -C "$D/other" add other.txt
  git -C "$D/other" commit -q -m "other issue"
  git -C "$D/other" push -q origin main
fi
rc=$(sed -n "${n}p" "$D/rc")
[[ -n "$rc" ]] || rc=$(tail -n 1 "$D/rc")
if [[ "$rc" == 0 && "$wf" == ci.yml && "$ev" == pull_request ]]; then echo "$1" >> "$D/green"; fi
exit "$rc"
EOF
chmod +x "$work/gate"

# Der gh-Stub. Zustand in $D: pr (Nummer), state, head, automerge (SHA oder
# any), never (Auto-Merge merged nie), disable_fails (Abschalten scheitert),
# rules.json (die Regeln auf main, auf die der Stub den --jq-Ausdruck mit jq
# anwendet), calls (ein Aufruf je Zeile), body und title des erstellten,
# edit_body des zuletzt bearbeiteten Pull Requests.
cat > "$work/gh" << 'EOF'
#!/usr/bin/env bash
set -euo pipefail
D="$STUB_DIR"
echo "$*" >> "$D/calls"
echo "gh $*" >> "$D/seq"
origin="$D/origin.git"
case "$1 $2" in
  "api repos/{owner}/{repo}/rules/branches/main")
    [[ "$3" == --jq ]] || exit 2
    jq -r "$4" "$D/rules.json"
    ;;
  "pr edit")
    shift 3
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --title) shift 2 ;;
        --body-file) cp "$2" "$D/edit_body"; shift 2 ;;
        *) echo "gh stub: unknown edit flag $1" >&2; exit 2 ;;
      esac
    done
    ;;
  "pr list")
    if [[ -f "$D/pr" && "$(cat "$D/state")" == OPEN ]]; then cat "$D/pr"; fi
    ;;
  "pr create")
    shift 2
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --base) echo "$2" > "$D/base"; shift 2 ;;
        --head) echo "$2" > "$D/head"; shift 2 ;;
        --title) printf '%s' "$2" > "$D/title"; shift 2 ;;
        --body-file) cp "$2" "$D/body"; shift 2 ;;
        *) echo "gh stub: unknown create flag $1" >&2; exit 2 ;;
      esac
    done
    git -C "$origin" rev-parse --verify --quiet "refs/heads/$(cat "$D/head")" > /dev/null \
      || { echo "head branch not pushed" >&2; exit 1; }
    echo 7 > "$D/pr"
    echo OPEN > "$D/state"
    echo "https://github.invalid/pull/7"
    ;;
  "pr merge")
    [[ "$3" == "$(cat "$D/pr")" ]] || { echo "unknown pr $3" >&2; exit 1; }
    shift 3
    if [[ "$*" == "--disable-auto" ]]; then
      [[ ! -f "$D/disable_fails" ]] || { echo "disable failed" >&2; exit 1; }
      rm -f "$D/automerge"
      exit 0
    fi
    want=any auto=0 method=0
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --auto) auto=1; shift ;;
        --merge) method=1; shift ;;
        --match-head-commit) want="$2"; shift 2 ;;
        *) echo "gh stub: unknown merge flag $1" >&2; exit 2 ;;
      esac
    done
    [[ "$auto" == 1 && "$method" == 1 ]] || { echo "gh stub: only --auto --merge is expected" >&2; exit 2; }
    head="$(git -C "$origin" rev-parse "refs/heads/$(cat "$D/head")")"
    if [[ "$want" != any && "$want" != "$head" ]]; then
      echo "head of the pull request is not $want" >&2
      exit 1
    fi
    echo "$want" > "$D/automerge"
    ;;
  "pr view")
    if [[ "$5" == autoMergeRequest ]]; then
      if [[ -f "$D/automerge" ]]; then echo true; else echo false; fi
      exit 0
    fi
    state="$(cat "$D/state")"
    if [[ "$state" == OPEN && -f "$D/automerge" && ! -f "$D/never" ]]; then
      head="$(git -C "$origin" rev-parse "refs/heads/$(cat "$D/head")")"
      main="$(git -C "$origin" rev-parse refs/heads/main)"
      want="$(cat "$D/automerge")"
      if grep -qx "$head" "$D/green" 2> /dev/null \
        && [[ "$(git -C "$origin" rev-parse "$head^")" == "$main" ]] \
        && [[ "$want" == any || "$want" == "$head" ]]; then
        merge="$(git -C "$origin" commit-tree "$head^{tree}" -p "$main" -p "$head" -m "Merge pull request #7")"
        git -C "$origin" update-ref refs/heads/main "$merge" "$main"
        echo MERGED > "$D/state"
        echo "$merge" > "$D/merge"
        state=MERGED
      fi
    fi
    echo "$state $(cat "$D/merge" 2> /dev/null || true)"
    ;;
  *)
    echo "gh stub: unexpected call: $*" >&2
    exit 2
    ;;
esac
EOF
chmod +x "$work/gh"

# setup <name>: frisches origin und Klon, `main` mit einem Commit, der die
# Vorlage des Pull Requests traegt.
setup() {
  local dir="$work/$1"
  mkdir -p "$dir"
  git init -q --bare -b main "$dir/origin.git"
  git clone -q "$dir/origin.git" "$dir/clone" 2> /dev/null
  mkdir -p "$dir/clone/tools" "$dir/clone/.github"
  cp "$script" "$dir/clone/tools/commit-issue.sh"
  chmod +x "$dir/clone/tools/commit-issue.sh"
  cp "$template" "$dir/clone/.github/pull_request_template.md"
  printf 'packaging/\n' > "$dir/clone/tools/release-dry-paths.txt"
  echo base > "$dir/clone/base.txt"
  git -C "$dir/clone" add -A
  git -C "$dir/clone" commit -q -m base
  git -C "$dir/clone" push -q origin main
  echo 0 > "$dir/rc"
  cat > "$dir/rules.json" << 'JSON'
[{"type": "deletion", "parameters": {}},
 {"type": "required_status_checks", "parameters": {
   "strict_required_status_checks_policy": true,
   "required_status_checks": [{"context": "ci-ok", "integration_id": 15368}]}}]
JSON
  echo "$dir"
}

body='Adds a.txt so the test has something to land.

Review: Codex 1 finding (taken), Antigravity 0 findings.'

pr() { # <dir> <args nach --pr...>; Koerper auf stdin
  local dir="$1"
  shift
  (cd "$dir/clone" && STUB_DIR="$dir" HUMANITL_CI_GATE="$work/gate" HUMANITL_GH="$work/gh" tools/commit-issue.sh --pr "$@")
}
land() { # <dir> <branch>
  (cd "$1/clone" && STUB_DIR="$1" HUMANITL_CI_GATE="$work/gate" HUMANITL_GH="$work/gh" tools/commit-issue.sh --land "$2")
}

check() { # <name> <bedingung als befehl...>
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
has() { grep -qF -- "$2" "$1"; }
count() { grep -cF -- "$2" "$1" 2> /dev/null || true; }
origin_main() { git -C "$1/origin.git" rev-parse main; }
only_main_on_origin() { [[ "$(git -C "$1/origin.git" for-each-ref --format='%(refname)' refs/heads)" == refs/heads/main ]]; }

# 1. Argumente: Alles wird vor dem Bauen geprueft. Nichts wird gepusht, gh
#    wird nicht gerufen, kein lokaler Zweig entsteht.
d="$(setup args)"
base_main="$(origin_main "$d")"
echo a > "$d/clone/a.txt"
for bad in "abc ok subj a.txt" "0 ok subj a.txt" "-5 ok subj a.txt" "42 Bad_Slug subj a.txt" "42 ok subj"; do
  rc=0
  # shellcheck disable=SC2086 # die Faelle sind absichtlich Wortlisten
  pr "$d" $bad <<< "$body" >> "$d/out" 2>&1 || rc=$?
  check "args_refused [$bad]" not same "$rc" 0
done
rc=0
pr "$d" 42 noreview "subj" a.txt <<< "Only a description, no review paragraph." >> "$d/out" 2>&1 || rc=$?
check args_no_review_refused not same "$rc" 0
check args_no_review_says_why has "$d/out" "Review:"
# Ein Issue je Pull Request: Der Body schliesst kein eigenes Issue.
for kw in "Fixes #12" "This closes #12." "resolved: #12"; do
  rc=0
  pr "$d" 42 closes "subj" a.txt <<< "$kw"$'\n\nReview: none.' >> "$d/out" 2>&1 || rc=$?
  check "args_closing_keyword_refused [$kw]" not same "$rc" 0
done
check args_nothing_pushed only_main_on_origin "$d"
check args_main_unchanged same "$(origin_main "$d")" "$base_main"
check args_no_gh_call not test -e "$d/calls"
check args_no_local_branch same "$(git -C "$d/clone" for-each-ref --format=x 'refs/heads/issue-*' | wc -l)" 0

# 2. Gruen: Pull Request mit Titel, Basis, Zweig und Koerper; Auto-Merge auf
#    genau den getesteten Commit; GitHub merged, das Skript meldet den
#    Merge-Commit und raeumt auf.
d="$(setup green)"
echo a > "$d/clone/a.txt"
echo unrelated > "$d/clone/unrelated.txt"
rc=0
pr "$d" 42 green-path "feat(tools): land through a pull request" a.txt <<< "$body" > "$d/out" 2>&1 || rc=$?
tested="$(head -n 1 "$d/shas" 2> /dev/null || true)"
check green_exits_0 same "$rc" 0
check green_gate_saw_one_sha same "$(wc -l < "$d/shas")" 1
check green_branch_name same "$(cat "$d/head" 2> /dev/null)" issue-42-green-path
check green_base_main same "$(cat "$d/base" 2> /dev/null)" main
check green_title_is_subject same "$(cat "$d/title" 2> /dev/null)" "feat(tools): land through a pull request"
check green_body_closes_issue same "$(head -n 1 "$d/body" 2> /dev/null)" "Closes #42"
check green_body_has_review has "$d/body" "Review: Codex 1 finding (taken), Antigravity 0 findings."
check green_body_has_description has "$d/body" "Adds a.txt so the test has something to land."
check green_body_has_checklist has "$d/body" "## Author checklist"
check green_body_without_trailer not has "$d/body" "Co-Authored-By"
# Die Zusammenfassung steht unter "## Review", die Beschreibung darueber.
check green_review_under_heading same "$(awk '/^## Review/ { r = 1 } r && /^Review: /' "$d/body" | wc -l)" 1
check green_description_not_under_review same "$(awk '/^## Review/ { r = 1 } r && /^Adds a.txt/' "$d/body" | wc -l)" 0
check green_automerge_matches_tested has "$d/calls" "pr merge 7 --auto --merge --match-head-commit $tested"
check green_ruleset_checked has "$d/calls" "api repos/{owner}/{repo}/rules/branches/main"
check green_gate_on_pull_request_run has "$d/seq" "gate workflow=ci.yml branch=issue-42-green-path event=pull_request $tested"
# Auto-Merge erst, nachdem das Tor fuer diesen Commit gruen war.
gate_line="$(grep -n "^gate " "$d/seq" | head -n 1 | cut -d: -f1)"
auto_line="$(grep -n -- "--auto" "$d/seq" | head -n 1 | cut -d: -f1)"
check green_automerge_after_gate test "${auto_line:-0}" -gt "${gate_line:-999}"
check green_one_pr_created same "$(count "$d/calls" "pr create")" 1
check green_merged_on_origin same "$(git -C "$d/origin.git" rev-parse main^2)" "$tested"
check green_reports_merge has "$d/out" "#7 merged as $(git -C "$d/origin.git" rev-parse --short=12 main)"
check green_tree_is_tested not has "$d/out" "warning: the tree"
check green_tested_has_only_a same "$(git -C "$d/origin.git" diff --name-only "$tested^" "$tested")" a.txt
check green_branch_deleted_on_origin only_main_on_origin "$d"
check green_local_branch_deleted not git -C "$d/clone" rev-parse --verify --quiet refs/heads/issue-42-green-path
check green_local_main_advanced same "$(git -C "$d/clone" rev-parse main)" "$(origin_main "$d")"
check green_unrelated_untouched same "$(git -C "$d/clone" status --porcelain unrelated.txt)" "?? unrelated.txt"

# 3. Rot: kein Merge, der Pull Request bleibt offen, die Meldung nennt den
#    Weg. Danach --land mit gruenem Tor: kein zweiter Pull Request, altes
#    Auto-Merge abgeschaltet, neues verlangt, gemergt.
d="$(setup red)"
base_main="$(origin_main "$d")"
printf '1\n0\n' > "$d/rc"
echo b > "$d/clone/b.txt"
rc=0
pr "$d" 43 red "subject 43" b.txt <<< "$body" > "$d/out" 2>&1 || rc=$?
check red_exits_nonzero not same "$rc" 0
check red_main_unchanged same "$(origin_main "$d")" "$base_main"
check red_says_land has "$d/out" "--land issue-43-red"
check red_says_pr_open has "$d/out" "#7 stays open"
check red_no_automerge not has "$d/calls" "--auto"
check red_branch_stays git -C "$d/origin.git" rev-parse --verify --quiet refs/heads/issue-43-red
rc=0
land "$d" issue-43-red > "$d/out2" 2>&1 || rc=$?
check red_land_exits_0 same "$rc" 0
check red_land_no_second_pr same "$(count "$d/calls" "pr create")" 1
check red_land_asked_automerge_state has "$d/calls" "pr view 7 --json autoMergeRequest"
check red_land_body_regenerated same "$(head -n 1 "$d/edit_body" 2> /dev/null)" "Closes #43"
check red_land_body_has_review has "$d/edit_body" "Review: Codex 1 finding (taken)"
check red_land_merged same "$(git -C "$d/origin.git" rev-parse main^2)" "$(tail -n 1 "$d/shas")"

# 4. main bewegt sich waehrend der CI: GitHub merged den veralteten Zweig
#    nicht; das Skript setzt um, schaltet das alte Auto-Merge ab, pusht,
#    verlangt Auto-Merge fuer den neuen Commit und wartet erneut.
d="$(setup race)"
touch "$d/race"
echo c > "$d/clone/c.txt"
rc=0
pr "$d" 44 race "subject 44" c.txt <<< "$body" > "$d/out" 2>&1 || rc=$?
first="$(sed -n 1p "$d/shas")"
second="$(sed -n 2p "$d/shas")"
check race_exits_0 same "$rc" 0
check race_gate_saw_two_shas same "$(wc -l < "$d/shas")" 2
check race_second_on_new_main same "$(git -C "$d/origin.git" rev-parse "$second^")" "$(git -C "$d/origin.git" rev-parse main^1)"
check race_automerge_first has "$d/calls" "--match-head-commit $first"
check race_automerge_second has "$d/calls" "--match-head-commit $second"
check race_disabled_before_repush has "$d/calls" "pr merge 7 --disable-auto"
check race_merged_second same "$(git -C "$d/origin.git" rev-parse main^2)" "$second"
check race_other_kept same "$(git -C "$d/origin.git" show main:other.txt)" other

# 5. Auto-Merge merged nicht binnen der Frist: Das Skript sagt es und
#    merged nie selbst daran vorbei.
d="$(setup never)"
base_main="$(origin_main "$d")"
touch "$d/never"
echo e > "$d/clone/e.txt"
rc=0
HUMANITL_PR_MERGE_TIMEOUT=0 pr "$d" 45 never "subject 45" e.txt <<< "$body" > "$d/out" 2>&1 || rc=$?
check never_exits_nonzero not same "$rc" 0
check never_says_so has "$d/out" "auto-merge did not merge #7"
check never_main_unchanged same "$(origin_main "$d")" "$base_main"
check never_pr_still_open same "$(cat "$d/state")" OPEN

# 6. Probelauf des Releases: Auto-Merge erst, wenn ci.yml und der Probelauf
#    gruen sind; der Probelauf heisst release-dry/hum-N.
d="$(setup dry)"
mkdir -p "$d/clone/packaging"
echo p > "$d/clone/packaging/p.txt"
rc=0
pr "$d" 46 dry "subject 46" packaging/p.txt <<< "$body" > "$d/out" 2>&1 || rc=$?
check dry_exits_0 same "$rc" 0
check dry_gate_ran_release has "$d/seq" "gate workflow=release.yml branch=release-dry/hum-46"
dry_gate_line="$(grep -n "workflow=release.yml" "$d/seq" | head -n 1 | cut -d: -f1)"
auto_line="$(grep -n -- "--auto" "$d/seq" | head -n 1 | cut -d: -f1)"
check dry_automerge_after_dry_gate test "${auto_line:-0}" -gt "${dry_gate_line:-999}"
check dry_release_gate_on_push has "$d/seq" "branch=release-dry/hum-46 event=push"
check dry_branch_deleted only_main_on_origin "$d"

# other_commit <dir>: ein fremder Commit auf origin/main.
other_commit() {
  git clone -q "$1/origin.git" "$1/other"
  echo other > "$1/other/other.txt"
  git -C "$1/other" add other.txt
  git -C "$1/other" commit -q -m "other issue"
  git -C "$1/other" push -q origin main
}

# 7. Ein eingeschaltetes Auto-Merge laesst sich nicht abschalten: kein Push,
#    kein neues Tor, kein neues Auto-Merge.
d="$(setup disable)"
printf '1\n0\n' > "$d/rc"
echo f > "$d/clone/f.txt"
pr "$d" 47 disable "subject 47" f.txt <<< "$body" > "$d/out" 2>&1 || true
head_before="$(git -C "$d/origin.git" rev-parse refs/heads/issue-47-disable)"
# Jemand hat Auto-Merge von Hand eingeschaltet, und main hat sich bewegt, so
# dass --land einen neuen Commit pushen wuerde.
echo "$head_before" > "$d/automerge"
touch "$d/disable_fails"
other_commit "$d"
gates_before="$(wc -l < "$d/shas")"
rc=0
land "$d" issue-47-disable > "$d/out2" 2>&1 || rc=$?
check disable_fail_exits_nonzero not same "$rc" 0
check disable_fail_says_nothing_pushed has "$d/out2" "nothing pushed"
check disable_fail_branch_unchanged same "$(git -C "$d/origin.git" rev-parse refs/heads/issue-47-disable)" "$head_before"
check disable_fail_no_new_gate same "$(wc -l < "$d/shas")" "$gates_before"
check disable_fail_no_automerge not has "$d/calls" "--auto"

# 8. Keine Regel auf main, die ci-ok auf einem aktuellen Zweig verlangt:
#    Abbruch vor allem anderen, nie --auto.
for rules in '[]' \
  '[{"type":"required_status_checks","parameters":{"strict_required_status_checks_policy":false,"required_status_checks":[{"context":"ci-ok"}]}}]' \
  '[{"type":"required_status_checks","parameters":{"strict_required_status_checks_policy":true,"required_status_checks":[{"context":"lint"}]}}]'; do
  norules=$((norules + 1))
  d="$(setup "norules$norules")"
  echo "$rules" > "$d/rules.json"
  echo g > "$d/clone/g.txt"
  rc=0
  pr "$d" 48 norules "subject 48" g.txt <<< "$body" > "$d/out" 2>&1 || rc=$?
  check "norules_exits_nonzero [$norules]" not same "$rc" 0
  check "norules_points_to_guide [$norules]" has "$d/out" "docs/guides/maintaining.md"
  check "norules_no_automerge [$norules]" not has "$d/calls" "--auto"
  check "norules_no_pr [$norules]" not has "$d/calls" "pr create"
  check "norules_nothing_pushed [$norules]" only_main_on_origin "$d"
done

# 9. release-dry/hum-N gehoert schon einem anderen (altes HUM-Issue gleicher
#    Nummer): Abbruch, der fremde Zweig bleibt, kein Auto-Merge.
d="$(setup collide)"
git -C "$d/origin.git" update-ref refs/heads/release-dry/hum-49 "$(origin_main "$d")"
mkdir -p "$d/clone/packaging"
echo q > "$d/clone/packaging/q.txt"
rc=0
pr "$d" 49 collide "subject 49" packaging/q.txt <<< "$body" > "$d/out" 2>&1 || rc=$?
check collide_exits_nonzero not same "$rc" 0
check collide_foreign_branch_kept same "$(git -C "$d/origin.git" rev-parse refs/heads/release-dry/hum-49)" "$(origin_main "$d")"
check collide_no_automerge not has "$d/calls" "--auto"

# 10. Der eigene Probelauf-Zweig eines roten Laufs ist kein fremder: --land
#     nach Rot pusht ihn neu und merged.
d="$(setup dryred)"
printf '1\n0\n' > "$d/rc"
mkdir -p "$d/clone/packaging"
echo r > "$d/clone/packaging/r.txt"
pr "$d" 50 dryred "subject 50" packaging/r.txt <<< "$body" > "$d/out" 2>&1 || true
other_commit "$d"
rc=0
land "$d" issue-50-dryred > "$d/out2" 2>&1 || rc=$?
check dryred_land_exits_0 same "$rc" 0
check dryred_land_merged same "$(git -C "$d/origin.git" rev-parse main^2)" "$(tail -n 1 "$d/shas")"

if [[ "$fails" -ne 0 ]]; then
  echo "commit_issue_pr_test: $fails Pruefung(en) rot"
  exit 1
fi
echo "commit_issue_pr_test: alle Pruefungen gruen"
