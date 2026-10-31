#!/usr/bin/env bash
# Test fuer `tools/commit-issue.sh --ci` (HUM-237) in einem Wegwerf-Repository.
#
# Ein nacktes Repository spielt `origin`, ein nachgestelltes CI-Tor
# (HUMANITL_CI_GATE) gibt den Exit-Code aus einer Datei zurueck und schreibt
# jede gepruefte SHA mit. Geprueft wird, was auf `main` landet: nur nach
# gruenem Tor, genau der getestete Baum, und nach einem Wechsel von `main`
# erst nach einem zweiten Tor auf dem neuen Stand.
#
# Mutationsbeweis: COMMIT_ISSUE=<pfad> prueft eine veraenderte Kopie.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
script="${COMMIT_ISSUE:-$here/../commit-issue.sh}"

work="$(mktemp -d "${TMPDIR:-/tmp}/commit-issue-test.XXXXXX")"
fails=0
# Bei einem roten Lauf bleibt der Baum stehen und die Ausgaben werden gezeigt.
cleanup() {
  if [[ "$fails" -eq 0 ]]; then
    rm -rf "$work"
  else
    for out in "$work"/*/out; do
      echo "--- $out"
      cat "$out"
    done
    echo "commit_issue_test: Baum bleibt in $work"
  fi
}
trap cleanup EXIT

export GIT_AUTHOR_NAME=Test GIT_AUTHOR_EMAIL=test@example.invalid
export GIT_COMMITTER_NAME=Test GIT_COMMITTER_EMAIL=test@example.invalid
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1

# Das Tor: Exit-Code aus $GATE_DIR/rc (eine Zeile je Aufruf, die letzte
# bleibt stehen), jede SHA nach $GATE_DIR/shas. Liegt $GATE_DIR/race, legt der
# erste Aufruf vorher einen fremden Commit auf origin/main, so als haette ein
# anderes Issue waehrend des CI-Laufs gemergt; die Datei steht in race
# (leer: other.txt).
cat > "$work/gate" << 'EOF'
#!/usr/bin/env bash
set -euo pipefail
echo "$1" >> "$GATE_DIR/shas"
n=$(wc -l < "$GATE_DIR/shas")
# HUM-363: welcher Lauf gemeint ist, und welche Zweige origin in dem
# Augenblick hat.
refs="$(git -C "$GATE_DIR/origin.git" for-each-ref --format='%(refname)=%(objectname)' refs/heads | tr '\n' ' ')"
echo "workflow=${HUMANITL_CI_WORKFLOW:-ci.yml} branch=${HUMANITL_CI_BRANCH:-} every=${HUMANITL_CI_EVERY_JOB:-} skip=${HUMANITL_CI_MAY_SKIP:-} sha=$1 refs=$refs" >> "$GATE_DIR/calls"
if [[ -f "$GATE_DIR/race" && "$n" -eq 1 ]]; then
  f=$(cat "$GATE_DIR/race")
  f=${f:-other.txt}
  git clone -q "$GATE_DIR/origin.git" "$GATE_DIR/other"
  case "$f" in
    mv:*) # mv:alt:neu benennt eine Datei um
      IFS=: read -r _ from to <<< "$f"
      git -C "$GATE_DIR/other" mv -- "$from" "$to" ;;
    ln:*) # ln:pfad:ziel biegt einen Symlink um
      IFS=: read -r _ link target <<< "$f"
      ln -sfn -- "$target" "$GATE_DIR/other/$link"
      git -C "$GATE_DIR/other" add -- "$link" ;;
    *)
      echo other > "$GATE_DIR/other/$f"
      git -C "$GATE_DIR/other" --literal-pathspecs add -- "$f" ;;
  esac
  git -C "$GATE_DIR/other" commit -q -m "other issue"
  git -C "$GATE_DIR/other" push -q origin main
fi
rc=$(sed -n "${n}p" "$GATE_DIR/rc")
[[ -n "$rc" ]] || rc=$(tail -n 1 "$GATE_DIR/rc")
exit "$rc"
EOF
chmod +x "$work/gate"

# setup <name>: frisches origin und Klon, `main` mit einem Commit.
setup() {
  local dir="$work/$1"
  mkdir -p "$dir"
  git init -q --bare -b main "$dir/origin.git"
  git clone -q "$dir/origin.git" "$dir/clone" 2> /dev/null
  mkdir -p "$dir/clone/tools"
  cp "$script" "$dir/clone/tools/commit-issue.sh"
  # Ohne Liste der Release-Pfade waere jeder Commit ein Fall fuer den
  # Probelauf (HUM-363); die Faelle davor beruehren `packaging/` nicht.
  printf 'packaging/\n' > "$dir/clone/tools/release-dry-paths.txt"
  echo base > "$dir/clone/base.txt"
  git -C "$dir/clone" add -A
  git -C "$dir/clone" commit -q -m base
  git -C "$dir/clone" push -q origin main
  echo "$dir"
}

ci() { # <dir> <issue> <slug> <paths...>; Koerper auf stdin
  local dir="$1"
  shift
  (cd "$dir/clone" && GATE_DIR="$dir" HUMANITL_CI_GATE="$dir/../gate" tools/commit-issue.sh --ci "$1" "$2" "subject $1" "${@:3}")
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

# 1. Gruen: ein Merge-Commit auf origin/main, zweiter Elternteil ist der
#    getestete Commit, der Baum ist genau dessen Baum.
d="$(setup green)"
echo 0 > "$d/rc"
echo a > "$d/clone/a.txt"
echo unrelated > "$d/clone/unrelated.txt"
# Eine vorgemerkte Datei eines anderen Issues: Sie darf weder in den Commit
# rutschen noch aus dem Index des Aufrufers verschwinden.
echo staged > "$d/clone/staged.txt"
git -C "$d/clone" add staged.txt
rc=0
(export CLAUDE_SESSION_URL=https://example.invalid/session/1
  ci "$d" HUM-901 green a.txt <<< "body") > "$d/out" 2>&1 || rc=$?
tested="$(head -n 1 "$d/shas" 2> /dev/null || true)"
head_main="$(git -C "$d/origin.git" rev-parse main)"
check green_exits_0 same "$rc" 0
check green_gate_saw_one_sha same "$(wc -l < "$d/shas")" 1
check green_main_is_a_merge same "$(git -C "$d/origin.git" rev-list --parents -n 1 main | wc -w)" 3
check green_second_parent_is_tested same "$(git -C "$d/origin.git" rev-parse main^2)" "$tested"
check green_tree_is_tested_tree same "$(git -C "$d/origin.git" rev-parse "main^{tree}")" \
  "$(git -C "$d/origin.git" rev-parse "$tested^{tree}")"
check green_merge_message same "$(git -C "$d/origin.git" log -1 --format=%s main)" "merge: HUM-901 green"
check green_commit_has_only_its_path same "$(git -C "$d/origin.git" diff --name-only main^1 main)" a.txt
check green_unrelated_file_stays_in_worktree test -f "$d/clone/unrelated.txt"
check green_branch_deleted_on_origin not git -C "$d/origin.git" rev-parse --verify --quiet refs/heads/hum-901-green
check green_local_main_follows same "$(git -C "$d/clone" rev-parse main)" "$head_main"
# Die Datei des Issues ist danach sauber, die fremden bleiben, wie sie waren.
check green_worktree_clean_but_unrelated same "$(git -C "$d/clone" status --porcelain)" \
  "$(printf 'A  staged.txt\n?? unrelated.txt')"
check green_trailer_current grep -q "Co-Authored-By: Claude Opus 5.5 (1M context)" \
  <(git -C "$d/origin.git" log -1 --format=%B main^2)
check green_trailer_session grep -qx "Claude-Session: https://example.invalid/session/1" \
  <(git -C "$d/origin.git" log -1 --format=%B main^2)

# 2. Rot: main bleibt, wo es war; der Zweig liegt auf origin zum Nachbessern.
d="$(setup red)"
echo 1 > "$d/rc"
before="$(git -C "$d/origin.git" rev-parse main)"
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-902 red a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check red_exits_nonzero not same "$rc" 0
check red_main_unchanged same "$(git -C "$d/origin.git" rev-parse main)" "$before"
check red_branch_pushed git -C "$d/origin.git" rev-parse --verify --quiet refs/heads/hum-902-red
check red_says_so grep -q "CI is red" "$d/out"

# 2b. Kein Urteil (abgebrochen): ebenso kein Merge.
d="$(setup noverdict)"
echo 3 > "$d/rc"
before="$(git -C "$d/origin.git" rev-parse main)"
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-905 noverdict a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check noverdict_exits_nonzero not same "$rc" 0
check noverdict_main_unchanged same "$(git -C "$d/origin.git" rev-parse main)" "$before"

# 3. main bewegt sich waehrend des CI-Laufs: zweites Tor auf dem neuen Stand,
#    und der Merge sitzt auf dem fremden Commit.
d="$(setup race)"
printf '0\n0\n' > "$d/rc"
: > "$d/race"
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-903 race a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check race_exits_0 same "$rc" 0
check race_gate_ran_twice same "$(wc -l < "$d/shas")" 2
second="$(sed -n 2p "$d/shas")"
check race_second_parent_is_second_tested same "$(git -C "$d/origin.git" rev-parse main^2)" "$second"
check race_first_parent_is_other same "$(git -C "$d/origin.git" log -1 --format=%s main^1)" "other issue"
check race_tree_has_both git -C "$d/origin.git" cat-file -e "main:other.txt"
check race_tree_has_both_a git -C "$d/origin.git" cat-file -e "main:a.txt"
check race_tree_is_tested_tree same "$(git -C "$d/origin.git" rev-parse "main^{tree}")" \
  "$(git -C "$d/origin.git" rev-parse "$second^{tree}")"
# Der lokale Stand folgt, die Datei des anderen Issues erscheint im Baum.
check race_local_main_follows same "$(git -C "$d/clone" rev-parse main)" \
  "$(git -C "$d/origin.git" rev-parse main)"
check race_worktree_clean same "$(git -C "$d/clone" status --porcelain)" ""
check race_other_file_checked_out test -f "$d/clone/other.txt"

# 4. Ein Pfad des Issues hat sich auf main geaendert, seit HEAD abzweigte:
#    kein Commit, kein Push, statt main stillschweigend zurueckzudrehen.
d="$(setup stale)"
echo 0 > "$d/rc"
git clone -q "$d/origin.git" "$d/other" 2> /dev/null
echo newer > "$d/other/base.txt"
git -C "$d/other" commit -q -am "newer base"
git -C "$d/other" push -q origin main
before="$(git -C "$d/origin.git" rev-parse main)"
echo mine > "$d/clone/base.txt"
rc=0
ci "$d" HUM-904 stale base.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check stale_exits_nonzero not same "$rc" 0
check stale_main_unchanged same "$(git -C "$d/origin.git" rev-parse main)" "$before"
check stale_nothing_pushed not git -C "$d/origin.git" rev-parse --verify --quiet refs/heads/hum-904-stale
check stale_gate_not_called test ! -s "$d/shas"

# 5. main ist in einem anderen Arbeitsbaum ausgecheckt: der Merge landet auf
#    origin, aber der fremde Arbeitsbaum behaelt einen stimmigen Index.
d="$(setup elsewhere)"
echo 0 > "$d/rc"
git -C "$d/clone" checkout -q -b work
git -C "$d/clone" worktree add -q "$d/wt" main
before="$(git -C "$d/clone" rev-parse main)"
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-906 elsewhere a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check elsewhere_exits_0 same "$rc" 0
check elsewhere_merged_on_origin same "$(git -C "$d/origin.git" log -1 --format=%s main)" "merge: HUM-906 elsewhere"
check elsewhere_local_main_untouched same "$(git -C "$d/clone" rev-parse main)" "$before"
check elsewhere_other_worktree_clean same "$(git -C "$d/wt" status --porcelain)" ""

land() { # <dir> <branch>
  (cd "$1/clone" && GATE_DIR="$1" HUMANITL_CI_GATE="$1/../gate" tools/commit-issue.sh --land "$2")
}

# 6. Eine vorgemerkte Aenderung an einem Pfad, den der Merge auf main
#    veraendert: Das lokale main bleibt stehen, und die vorgemerkte Fassung
#    ueberlebt in Index und Arbeitsbaum.
d="$(setup stagedmain)"
printf '0\n0\n' > "$d/rc"
echo base.txt > "$d/race"
echo mine > "$d/clone/base.txt"
git -C "$d/clone" add base.txt
before="$(git -C "$d/clone" rev-parse main)"
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-907 stagedmain a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check stagedmain_exits_0 same "$rc" 0
check stagedmain_merged_on_origin same "$(git -C "$d/origin.git" log -1 --format=%s main)" "merge: HUM-907 stagedmain"
check stagedmain_local_main_stays same "$(git -C "$d/clone" rev-parse main)" "$before"
check stagedmain_staged_survives same "$(git -C "$d/clone" show :base.txt)" mine
check stagedmain_worktree_survives same "$(cat "$d/clone/base.txt")" mine
check stagedmain_says_so grep -q "has local changes" "$d/out"

# 6b. Dasselbe mit einem vorgemerkten `chmod +x` ohne Inhaltsaenderung: Der
#     Blob gleicht dem alten main, nur der Modus nicht.
d="$(setup stagedmode)"
printf '0\n0\n' > "$d/rc"
echo base.txt > "$d/race"
git -C "$d/clone" update-index --chmod=+x base.txt
before="$(git -C "$d/clone" rev-parse main)"
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-912 stagedmode a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check stagedmode_exits_0 same "$rc" 0
check stagedmode_merged_on_origin same "$(git -C "$d/origin.git" log -1 --format=%s main)" "merge: HUM-912 stagedmode"
check stagedmode_local_main_stays same "$(git -C "$d/clone" rev-parse main)" "$before"
check stagedmode_mode_survives same "$(git -C "$d/clone" ls-files -s base.txt | cut -d' ' -f1)" 100755
check stagedmode_says_so grep -q "has local changes" "$d/out"
check stagedmode_content_kept same "$(cat "$d/clone/base.txt")" base

# 6c. Ein nicht vorgemerktes `chmod +x`: Index gleicht dem alten main, nur die
#     Datei im Arbeitsbaum traegt ein anderes Recht. Auch das bleibt.
d="$(setup unstagedmode)"
printf '0\n0\n' > "$d/rc"
echo base.txt > "$d/race"
chmod +x "$d/clone/base.txt"
before="$(git -C "$d/clone" rev-parse main)"
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-913 unstagedmode a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check unstagedmode_exits_0 same "$rc" 0
check unstagedmode_merged_on_origin same "$(git -C "$d/origin.git" log -1 --format=%s main)" "merge: HUM-913 unstagedmode"
check unstagedmode_local_main_stays same "$(git -C "$d/clone" rev-parse main)" "$before"
check unstagedmode_mode_kept test -x "$d/clone/base.txt"
check unstagedmode_content_kept same "$(cat "$d/clone/base.txt")" base
check unstagedmode_index_untouched same "$(git -C "$d/clone" ls-files -s base.txt | cut -d' ' -f1)" 100644
check unstagedmode_says_so grep -q "sync yourself with \`git merge --ff-only origin/main\`" "$d/out"

# 6d. Der fremde Merge benennt eine Datei um, die im Arbeitsbaum noch unter
#     altem Namen liegt: Danach ist der alte Name aus Index und Arbeitsbaum
#     verschwunden, der neue da, und der Status ist sauber.
d="$(setup rename)"
printf '0\n0\n' > "$d/rc"
echo "mv:base.txt:moved.txt" > "$d/race"
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-914 rename a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check rename_exits_0 same "$rc" 0
check rename_local_main_follows same "$(git -C "$d/clone" rev-parse main)" "$(git -C "$d/origin.git" rev-parse main)"
check rename_old_gone_from_index same "$(git -C "$d/clone" ls-files base.txt)" ""
check rename_old_gone_from_worktree test ! -e "$d/clone/base.txt"
check rename_new_there test -f "$d/clone/moved.txt"
check rename_status_clean same "$(git -C "$d/clone" status --porcelain)" ""

# 6e. Ein Pfad mit Musterzeichen: Der Merge aendert `f?`, der Aufrufer hat
#     eine Aenderung an `fa` vorgemerkt. Als Muster traefe `f?` auch `fa`;
#     woertlich genommen bleibt `fa` vorgemerkt, und main zieht nach.
d="$(setup glob)"
printf '0\n0\n' > "$d/rc"
echo old > "$d/clone/f?"
echo old > "$d/clone/fa"
git -C "$d/clone" --literal-pathspecs add -- "f?" fa
git -C "$d/clone" commit -q -m "glob files"
git -C "$d/clone" push -q origin main
echo "f?" > "$d/race"
echo mine > "$d/clone/fa"
git -C "$d/clone" add fa
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-915 glob a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check glob_exits_0 same "$rc" 0
check glob_local_main_follows same "$(git -C "$d/clone" rev-parse main)" "$(git -C "$d/origin.git" rev-parse main)"
check glob_other_staged_survives same "$(git -C "$d/clone" show :fa)" mine
check glob_merged_file_updated same "$(cat "$d/clone/f?")" other

# 6f. Ein Symlink, dessen Ziel lokal von `old` auf `old\n` gebogen wurde,
#     waehrend der fremde Merge ihn auf `new` biegt: Das Ziel mit dem
#     Zeilenumbruch ist eine lokale Aenderung und bleibt.
d="$(setup symlink)"
printf '0\n0\n' > "$d/rc"
ln -s old "$d/clone/link"
git -C "$d/clone" add link
git -C "$d/clone" commit -q -m "link"
git -C "$d/clone" push -q origin main
echo "ln:link:new" > "$d/race"
rm "$d/clone/link"
ln -s "old"$'\n' "$d/clone/link"
before="$(git -C "$d/clone" rev-parse main)"
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-916 symlink a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check symlink_exits_0 same "$rc" 0
check symlink_local_main_stays same "$(git -C "$d/clone" rev-parse main)" "$before"
check symlink_target_kept same "$(readlink -n "$d/clone/link"; echo .)" $'old\n.'
check symlink_says_so grep -q "has local changes" "$d/out"

# 7. --land nach roter CI: Commit ergaenzt, zweiter Lauf gruen, Merge des
#    ergaenzten Commits.
d="$(setup redfix)"
printf '1\n0\n' > "$d/rc"
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-908 redfix a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
old="$(git -C "$d/clone" rev-parse hum-908-redfix)"
echo fixed > "$d/clone/a.txt"
blob="$(git -C "$d/clone" hash-object -w a.txt)"
tree="$(printf '100644 blob %s\ta.txt\n' "$blob" | cat <(git -C "$d/clone" ls-tree "$old" | grep -v $'\ta.txt$') - | git -C "$d/clone" mktree)"
amended="$(git -C "$d/clone" commit-tree "$tree" -p "$old^" -m "subject fixed")"
git -C "$d/clone" update-ref refs/heads/hum-908-redfix "$amended"
rc2=0
land "$d" hum-908-redfix > "$d/out2" 2>&1 || rc2=$?
check redfix_first_red not same "$rc" 0
check redfix_land_exits_0 same "$rc2" 0
check redfix_merges_amended same "$(git -C "$d/origin.git" rev-parse main^2)" "$amended"
check redfix_content_fixed same "$(git -C "$d/origin.git" show main:a.txt)" fixed

# 8. --land verweigert einen Branch, der in einem Arbeitsbaum ausgecheckt ist:
#    keine Referenz bewegt, nichts gepusht.
d="$(setup checkedout)"
echo 1 > "$d/rc"
echo a > "$d/clone/a.txt"
ci "$d" HUM-909 checkedout a.txt <<< "body" > "$d/out" 2>&1 || true
git -C "$d/clone" worktree add -q "$d/wt" hum-909-checkedout
echo 0 > "$d/rc"
ref="$(git -C "$d/clone" rev-parse hum-909-checkedout)"
gates="$(wc -l < "$d/shas")"
rc=0
land "$d" hum-909-checkedout > "$d/out2" 2>&1 || rc=$?
check checkedout_refused not same "$rc" 0
check checkedout_ref_kept same "$(git -C "$d/clone" rev-parse hum-909-checkedout)" "$ref"
check checkedout_no_gate same "$(wc -l < "$d/shas")" "$gates"
check checkedout_main_unchanged not git -C "$d/origin.git" cat-file -e main:a.txt
check checkedout_worktree_head_ok same "$(git -C "$d/wt" rev-parse HEAD)" "$ref"

# 9. Ein Branch mit zwei Commits wird nicht gelandet.
d="$(setup twocommits)"
echo 0 > "$d/rc"
git -C "$d/clone" checkout -q -b hum-910-twocommits
echo 1 > "$d/clone/one.txt"; git -C "$d/clone" add one.txt; git -C "$d/clone" commit -q -m one
echo 2 > "$d/clone/two.txt"; git -C "$d/clone" add two.txt; git -C "$d/clone" commit -q -m two
git -C "$d/clone" checkout -q main
rc=0
land "$d" hum-910-twocommits > "$d/out" 2>&1 || rc=$?
check twocommits_refused not same "$rc" 0
check twocommits_not_pushed not git -C "$d/origin.git" rev-parse --verify --quiet refs/heads/hum-910-twocommits
check twocommits_says_so grep -q "exactly one commit" "$d/out"

# 10. origin weist den ersten Push nach main ab (ein Merge kam dazwischen):
#     zweiter Lauf des Tors, dann Merge.
d="$(setup refused)"
echo 0 > "$d/rc"
cat > "$d/origin.git/hooks/pre-receive" << 'EOF'
#!/bin/sh
while read -r old new ref; do
  if [ "$ref" = refs/heads/main ] && [ ! -f "$GIT_DIR/refused-once" ]; then
    : > "$GIT_DIR/refused-once"
    echo "refused once" >&2
    exit 1
  fi
done
EOF
chmod +x "$d/origin.git/hooks/pre-receive"
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-911 refused a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check refused_exits_0 same "$rc" 0
check refused_gate_ran_twice same "$(wc -l < "$d/shas")" 2
check refused_merged same "$(git -C "$d/origin.git" log -1 --format=%s main)" "merge: HUM-911 refused"

# 11. HUM-363: Der Commit beruehrt einen Pfad aus tools/release-dry-paths.txt.
#     Neben dem Issue-Zweig steht release-dry/hum-NNN auf demselben Commit,
#     das Tor wartet nach ci.yml auch auf den Probelauf mit jedem Job gruen,
#     und nach dem Merge ist der Zweig wieder weg.
with_release_paths() { # <dir>: legt die Liste auf main
  printf '# Kommentar\n\npackaging/\n' > "$1/clone/tools/release-dry-paths.txt"
  git -C "$1/clone" add tools/release-dry-paths.txt
  git -C "$1/clone" commit -q -m "release paths"
  git -C "$1/clone" push -q origin main
}
d="$(setup drygreen)"
with_release_paths "$d"
printf '0\n0\n' > "$d/rc"
mkdir -p "$d/clone/packaging"
echo pkg > "$d/clone/packaging/build.sh"
rc=0
ci "$d" HUM-917 drygreen packaging/build.sh <<< "body" > "$d/out" 2>&1 || rc=$?
tested="$(head -n 1 "$d/shas" 2> /dev/null || true)"
check drygreen_exits_0 same "$rc" 0
check drygreen_two_gates same "$(wc -l < "$d/shas")" 2
check drygreen_same_sha same "$(sed -n 2p "$d/shas")" "$tested"
check drygreen_first_is_ci grep -q "^workflow=ci.yml branch= " <(sed -n 1p "$d/calls")
check drygreen_second_is_dry grep -q "^workflow=release.yml branch=release-dry/hum-917 every=1 skip=release sha=$tested" \
  <(sed -n 2p "$d/calls")
check drygreen_dry_branch_at_tested grep -q "refs/heads/release-dry/hum-917=$tested" <(sed -n 2p "$d/calls")
check drygreen_dry_pushed_before_ci grep -q "refs/heads/release-dry/hum-917=$tested" <(sed -n 1p "$d/calls")
check drygreen_merged same "$(git -C "$d/origin.git" log -1 --format=%s main)" "merge: HUM-917 drygreen"
check drygreen_dry_branch_deleted not git -C "$d/origin.git" rev-parse --verify --quiet refs/heads/release-dry/hum-917

# 12. Roter Probelauf bei gruener CI: kein Merge, beide Zweige bleiben.
d="$(setup dryred)"
with_release_paths "$d"
printf '0\n1\n' > "$d/rc"
before="$(git -C "$d/origin.git" rev-parse main)"
mkdir -p "$d/clone/packaging"
echo pkg > "$d/clone/packaging/build.sh"
rc=0
ci "$d" HUM-918 dryred packaging/build.sh <<< "body" > "$d/out" 2>&1 || rc=$?
check dryred_exits_nonzero not same "$rc" 0
check dryred_main_unchanged same "$(git -C "$d/origin.git" rev-parse main)" "$before"
check dryred_branch_kept git -C "$d/origin.git" rev-parse --verify --quiet refs/heads/hum-918-dryred
check dryred_dry_branch_kept git -C "$d/origin.git" rev-parse --verify --quiet refs/heads/release-dry/hum-918
check dryred_says_so grep -q "release dry run on release-dry/hum-918 is red" "$d/out"

# 13. Die Liste liegt auf main, der Commit beruehrt keinen ihrer Pfade: kein
#     Probelauf, ein Tor.
d="$(setup drynone)"
with_release_paths "$d"
echo 0 > "$d/rc"
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-919 drynone a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check drynone_exits_0 same "$rc" 0
check drynone_one_gate same "$(wc -l < "$d/shas")" 1
check drynone_no_dry_branch not grep -q "release-dry" "$d/calls"

# 14. Ein Commit, der packaging/ aendert und zugleich die Liste kuerzt, bekommt
#     den Probelauf trotzdem: Die Liste des Elternteils zaehlt mit, und die
#     Liste selbst steht in ihr.
d="$(setup dryshrink)"
printf '0\n0\n' > "$d/rc"
printf 'nothing/\n' > "$d/clone/tools/release-dry-paths.txt"
mkdir -p "$d/clone/packaging"
echo pkg > "$d/clone/packaging/build.sh"
rc=0
ci "$d" HUM-920 dryshrink tools/release-dry-paths.txt packaging/build.sh <<< "body" > "$d/out" 2>&1 || rc=$?
check dryshrink_exits_0 same "$rc" 0
check dryshrink_two_gates same "$(wc -l < "$d/shas")" 2

# 14b. Dasselbe mit geloeschter Liste.
d="$(setup drydelete)"
printf '0\n0\n' > "$d/rc"
rm "$d/clone/tools/release-dry-paths.txt"
mkdir -p "$d/clone/packaging"
echo pkg > "$d/clone/packaging/build.sh"
rc=0
ci "$d" HUM-921 drydelete tools/release-dry-paths.txt packaging/build.sh <<< "body" > "$d/out" 2>&1 || rc=$?
check drydelete_exits_0 same "$rc" 0
check drydelete_two_gates same "$(wc -l < "$d/shas")" 2

# 14c. Fehlt die Liste schon auf main, laeuft der Probelauf fuer jeden Commit,
#      auch fuer einen, der keinen Release-Pfad beruehrt.
d="$(setup drynolist)"
printf '0\n0\n' > "$d/rc"
git -C "$d/clone" rm -q tools/release-dry-paths.txt
git -C "$d/clone" commit -q -m "no list"
git -C "$d/clone" push -q origin main
echo a > "$d/clone/a.txt"
rc=0
ci "$d" HUM-922 drynolist a.txt <<< "body" > "$d/out" 2>&1 || rc=$?
check drynolist_exits_0 same "$rc" 0
check drynolist_two_gates same "$(wc -l < "$d/shas")" 2

# 14d. ci.yml rot bei laufendem Probelauf: Die Meldung sagt, dass auch
#      release-dry/hum-NNN auf origin bleibt.
d="$(setup dryci)"
echo 1 > "$d/rc"
mkdir -p "$d/clone/packaging"
echo pkg > "$d/clone/packaging/build.sh"
rc=0
ci "$d" HUM-923 dryci packaging/build.sh <<< "body" > "$d/out" 2>&1 || rc=$?
check dryci_exits_nonzero not same "$rc" 0
check dryci_says_dry_stays grep -q "release-dry/hum-923 stays on origin" "$d/out"

if [[ "$fails" -ne 0 ]]; then
  echo "commit_issue_test: $fails Pruefung(en) rot" >&2
  exit 1
fi
echo "commit_issue_test: alle Pruefungen gruen"
