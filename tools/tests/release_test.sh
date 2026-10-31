#!/usr/bin/env bash
# Test fuer tools/release.sh (HUM-363) in einem Wegwerf-Repository.
#
# Ein nacktes Repository spielt `origin` mit den Tags v0.0.1 und v0.0.2, ein
# nachgestelltes Tor (HUMANITL_CI_GATE) gibt je Lauf einen Exit-Code aus einer
# Datei zurueck: rc-ci fuer ci.yml auf main, rc-dry fuer den Probelauf auf
# release-dry/0.0.N, rc-tag fuer den Lauf des Tags. Jeder Aufruf schreibt
# seine Umgebung und den Stand von origin in diesem Augenblick mit (ob der
# Probelauf-Zweig und der Tag dort stehen). Geprueft wird vor allem, was nie
# geschehen darf: ein Tag nach rotem Probelauf, eine Luecke in den Nummern,
# ein Tag auf etwas anderem als der Spitze von main.
#
# Mutationsbeweis: RELEASE=<pfad> prueft eine veraenderte Kopie des Skripts.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
script="${RELEASE:-$here/../release.sh}"

work="$(mktemp -d "${TMPDIR:-/tmp}/release-test.XXXXXX")"
fails=0
cleanup() {
  if [[ "$fails" -eq 0 ]]; then
    rm -rf "$work"
  else
    echo "release_test: Baum bleibt in $work"
  fi
}
trap cleanup EXIT

export GIT_AUTHOR_NAME=Test GIT_AUTHOR_EMAIL=test@example.invalid
export GIT_COMMITTER_NAME=Test GIT_COMMITTER_EMAIL=test@example.invalid
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1

# Das Tor. Welcher Lauf gemeint ist, sagt HUMANITL_CI_BRANCH. Liegt
# $GATE_DIR/race-tag, setzt jemand anderes waehrend des Probelaufs den Tag
# mit dieser Nummer, so als liefe ein zweiter Aufruf daneben.
cat > "$work/gate" << 'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "${HUMANITL_CI_BRANCH:-}" in
  main) kind=ci ;;
  release-dry/*) kind=dry ;;
  v*) kind=tag ;;
  *) kind=other ;;
esac
refs="$(git -C "$GATE_DIR/origin.git" for-each-ref --format='%(refname)' | tr '\n' ' ')"
echo "$kind workflow=${HUMANITL_CI_WORKFLOW:-} branch=${HUMANITL_CI_BRANCH:-} every=${HUMANITL_CI_EVERY_JOB:-} skip=${HUMANITL_CI_MAY_SKIP:-} sha=$1 refs=$refs" >> "$GATE_DIR/calls"
if [[ "$kind" == dry && -f "$GATE_DIR/race-tag" ]]; then
  t="$(cat "$GATE_DIR/race-tag")"
  git -C "$GATE_DIR/origin.git" tag "$t" main
fi
# race-main: advance haengt einen Commit an main, rewrite ersetzt main durch
# einen Commit ohne den geprueften.
if [[ "$kind" == dry && -f "$GATE_DIR/race-main" ]]; then
  o="$GATE_DIR/origin.git"
  tree="$(git -C "$o" rev-parse 'main^{tree}')"
  case "$(cat "$GATE_DIR/race-main")" in
    advance) parent="$(git -C "$o" rev-parse main)" ;;
    rewrite) parent="$(git -C "$o" rev-parse main~2)" ;;
  esac
  new="$(git -C "$o" commit-tree "$tree" -p "$parent" -m moved)"
  git -C "$o" update-ref refs/heads/main "$new"
fi
exit "$(cat "$GATE_DIR/rc-$kind" 2> /dev/null || echo 0)"
EOF
chmod +x "$work/gate"

# setup <name>: origin mit drei Commits auf main, v0.0.1 und v0.0.2 (annotiert)
# auf den ersten beiden.
setup() {
  local dir="$work/$1" i
  mkdir -p "$dir"
  git init -q --bare -b main "$dir/origin.git"
  git clone -q "$dir/origin.git" "$dir/clone" 2> /dev/null
  mkdir -p "$dir/clone/tools"
  cp "$script" "$dir/clone/tools/release.sh"
  for i in 1 2 3; do
    echo "$i" > "$dir/clone/file.txt"
    git -C "$dir/clone" add -A
    git -C "$dir/clone" commit -q -m "commit $i"
    if ((i < 3)); then
      git -C "$dir/clone" tag -a "v0.0.$i" -m "Humanitl 0.0.$i"
    fi
  done
  git -C "$dir/clone" push -q --tags origin main
  echo "$dir"
}

rel() { # <dir> <args...>
  local dir="$1"
  shift
  (cd "$dir/clone" && GATE_DIR="$dir" HUMANITL_CI_GATE="$dir/../gate" HUMANITL_CI_REPO=owner/name \
    tools/release.sh "$@")
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
calls() { [[ -f "$1/calls" ]] && wc -l < "$1/calls" | tr -d ' ' || echo 0; }
on_origin() { git -C "$1/origin.git" rev-parse --verify --quiet "$2" > /dev/null; }
call_line() { grep "^$2 " "$1/calls" 2> /dev/null || true; }

# 1. Gruen: ci, Probelauf, Tag; der Tag ist annotiert, sitzt auf der Spitze
#    von main, und der Probelauf-Zweig ist danach weg.
d="$(setup green)"
tip="$(git -C "$d/origin.git" rev-parse main)"
rc=0
rel "$d" 0.0.3 > "$d/out" 2>&1 || rc=$?
check green_exits_0 same "$rc" 0
check green_three_gates same "$(calls "$d")" 3
check green_order same "$(cut -d' ' -f1 "$d/calls" | tr '\n' ' ')" "ci dry tag "
check green_ci_is_main grep -q "workflow=ci.yml branch=main every=0 skip= sha=$tip" <(call_line "$d" ci)
check green_dry_every_job grep -q "workflow=release.yml branch=release-dry/0.0.3 every=1 skip=release sha=$tip" <(call_line "$d" dry)
check green_dry_branch_there_during_dry grep -q "refs/heads/release-dry/0.0.3" <(call_line "$d" dry)
check green_no_tag_during_dry not grep -q "refs/tags/v0.0.3" <(call_line "$d" dry)
check green_tag_gate_every_job grep -q "workflow=release.yml branch=v0.0.3 every=1 skip= sha=$tip" <(call_line "$d" tag)
check green_tag_there_during_tag_run grep -q "refs/tags/v0.0.3" <(call_line "$d" tag)
check green_tag_on_tip same "$(git -C "$d/origin.git" rev-parse 'v0.0.3^{commit}')" "$tip"
check green_tag_annotated same "$(git -C "$d/origin.git" cat-file -t v0.0.3)" tag
check green_tag_message same "$(git -C "$d/origin.git" for-each-ref --format='%(contents:subject)' refs/tags/v0.0.3)" "Humanitl 0.0.3"
check green_dry_branch_deleted not on_origin "$d" refs/heads/release-dry/0.0.3
check green_names_release_url grep -q "https://github.com/owner/name/releases/tag/v0.0.3" "$d/out"

# 2. Roter Probelauf: kein Tag, weder auf origin noch lokal; der Lauf des
#    Tags wird nie abgewartet, der Zweig bleibt fuer einen neuen Versuch.
d="$(setup reddry)"
echo 1 > "$d/rc-dry"
rc=0
rel "$d" 0.0.3 > "$d/out" 2>&1 || rc=$?
check reddry_exits_1 same "$rc" 1
check reddry_no_tag_on_origin not on_origin "$d" refs/tags/v0.0.3
check reddry_no_local_tag not git -C "$d/clone" rev-parse --verify --quiet refs/tags/v0.0.3
check reddry_no_tag_gate same "$(call_line "$d" tag)" ""
check reddry_branch_kept on_origin "$d" refs/heads/release-dry/0.0.3
check reddry_says_so grep -q "dry run on release-dry/0.0.3 is red; no tag was set" "$d/out"

# 2b. Probelauf ohne Urteil: ebenso kein Tag, Exit 3.
d="$(setup undecided)"
echo 3 > "$d/rc-dry"
rc=0
rel "$d" 0.0.3 > "$d/out" 2>&1 || rc=$?
check undecided_exits_3 same "$rc" 3
check undecided_no_tag not on_origin "$d" refs/tags/v0.0.3

# 3. Luecke: 0.0.4 nach v0.0.2 wird verweigert, bevor irgendetwas laeuft.
d="$(setup gap)"
rc=0
rel "$d" 0.0.4 > "$d/out" 2>&1 || rc=$?
check gap_refused same "$rc" 1
check gap_no_gate same "$(calls "$d")" 0
check gap_nothing_pushed not on_origin "$d" refs/heads/release-dry/0.0.4
check gap_says_next grep -q "the next version is 0.0.3" "$d/out"

# 3b. Wiederholung: 0.0.2 gibt es schon.
d="$(setup again)"
rc=0
rel "$d" 0.0.2 > "$d/out" 2>&1 || rc=$?
check again_refused same "$rc" 1
check again_no_gate same "$(calls "$d")" 0
check again_says_so grep -q "is not new" "$d/out"

# 3c. Die Nummer zaehlt nach origin: Ein nur lokaler Tag v0.0.3 macht 0.0.4
#     nicht zur naechsten.
d="$(setup localtag)"
git -C "$d/clone" tag -a v0.0.3 -m "local only" main
rc=0
rel "$d" 0.0.4 > "$d/out" 2>&1 || rc=$?
check localtag_gap_refused same "$rc" 1
check localtag_no_gate same "$(calls "$d")" 0

# 3d. Ein lokaler Tag mit der neuen Nummer, den nicht dieses Skript gesetzt
#     hat, wird nicht gepusht; einer, den es gesetzt hat, wird genommen.
d="$(setup foreignlocal)"
git -C "$d/clone" tag -a v0.0.3 -m "something else" main
rc=0
rel "$d" 0.0.3 > "$d/out" 2>&1 || rc=$?
check foreignlocal_refused same "$rc" 1
check foreignlocal_no_gate same "$(calls "$d")" 0
check foreignlocal_not_pushed not on_origin "$d" refs/tags/v0.0.3
d="$(setup ownlocal)"
git -C "$d/clone" tag -a v0.0.3 -m "Humanitl 0.0.3" main
rc=0
rel "$d" 0.0.3 > "$d/out" 2>&1 || rc=$?
check ownlocal_exits_0 same "$rc" 0
check ownlocal_pushed on_origin "$d" refs/tags/v0.0.3

# 4. Ziel ist nicht die Spitze von main.
d="$(setup nottip)"
rc=0
rel "$d" 0.0.3 main^ > "$d/out" 2>&1 || rc=$?
check nottip_refused same "$rc" 1
check nottip_no_gate same "$(calls "$d")" 0
check nottip_no_tag not on_origin "$d" refs/tags/v0.0.3
check nottip_says_so grep -q "is not the tip of origin/main" "$d/out"

# 4b. Die Spitze ausdruecklich genannt ist dasselbe wie keine Angabe.
d="$(setup tipnamed)"
rc=0
rel "$d" 0.0.3 "$(git -C "$d/origin.git" rev-parse main)" > "$d/out" 2>&1 || rc=$?
check tipnamed_exits_0 same "$rc" 0

# 4c. Das lokale main hinkt hinterher: getaggt wird trotzdem die Spitze von
#     origin/main, nicht der lokale Stand.
d="$(setup behind)"
git clone -q "$d/origin.git" "$d/other" 2> /dev/null
echo 4 > "$d/other/file.txt"
git -C "$d/other" commit -q -am "commit 4"
git -C "$d/other" push -q origin main
rc=0
rel "$d" 0.0.3 > "$d/out" 2>&1 || rc=$?
check behind_exits_0 same "$rc" 0
check behind_tags_origin_tip same "$(git -C "$d/origin.git" rev-parse 'v0.0.3^{commit}')" \
  "$(git -C "$d/origin.git" rev-parse main)"

# 5. ci.yml rot: nichts gepusht.
d="$(setup redci)"
echo 1 > "$d/rc-ci"
rc=0
rel "$d" 0.0.3 > "$d/out" 2>&1 || rc=$?
check redci_refused same "$rc" 1
check redci_one_gate same "$(calls "$d")" 1
check redci_no_dry_branch not on_origin "$d" refs/heads/release-dry/0.0.3
check redci_no_tag not on_origin "$d" refs/tags/v0.0.3

# 6. Der Tag-Lauf scheitert trotz gruenem Probelauf: Der Tag bleibt, das
#    Skript verweist auf "Re-run failed jobs".
d="$(setup redtag)"
echo 1 > "$d/rc-tag"
rc=0
rel "$d" 0.0.3 > "$d/out" 2>&1 || rc=$?
check redtag_exits_1 same "$rc" 1
check redtag_tag_stays on_origin "$d" refs/tags/v0.0.3
check redtag_says_rerun grep -q "Re-run failed jobs" "$d/out"
check redtag_says_never_move grep -q "never move" "$d/out"

# 7. Waehrend des Probelaufs vergibt jemand anderes die Nummer: kein zweiter
#    Tag, kein Ueberschreiben.
d="$(setup race)"
echo v0.0.3 > "$d/race-tag"
rc=0
rel "$d" 0.0.3 > "$d/out" 2>&1 || rc=$?
check race_refused same "$rc" 1
check race_no_tag_gate same "$(call_line "$d" tag)" ""
check race_foreign_tag_untouched same "$(git -C "$d/origin.git" cat-file -t v0.0.3)" commit

# 7b. Waehrend des Probelaufs erscheint v0.0.4: v0.0.3 kaeme danach aus der
#     Reihe und wird nicht mehr gesetzt.
d="$(setup racenext)"
echo v0.0.4 > "$d/race-tag"
rc=0
rel "$d" 0.0.3 > "$d/out" 2>&1 || rc=$?
check racenext_refused same "$rc" 1
check racenext_no_tag not on_origin "$d" refs/tags/v0.0.3

# 7c. main bewegt sich waehrend des Probelaufs weiter: Der Tag sitzt auf dem
#     geprueften Commit, nicht auf der neuen Spitze, und das Skript sagt es.
d="$(setup mainmoved)"
tip="$(git -C "$d/origin.git" rev-parse main)"
echo advance > "$d/race-main"
rc=0
rel "$d" 0.0.3 > "$d/out" 2>&1 || rc=$?
check mainmoved_exits_0 same "$rc" 0
check mainmoved_tag_on_verified same "$(git -C "$d/origin.git" rev-parse 'v0.0.3^{commit}')" "$tip"
check mainmoved_main_is_newer not same "$(git -C "$d/origin.git" rev-parse main)" "$tip"
check mainmoved_says_so grep -q "goes on the verified commit" "$d/out"

# 7d. main wird waehrend des Probelaufs umgeschrieben: kein Tag.
d="$(setup mainrewritten)"
echo rewrite > "$d/race-main"
rc=0
rel "$d" 0.0.3 > "$d/out" 2>&1 || rc=$?
check mainrewritten_refused same "$rc" 1
check mainrewritten_no_tag not on_origin "$d" refs/tags/v0.0.3
check mainrewritten_says_so grep -q "no longer on origin/main" "$d/out"

# 8. Keine Version der Form 0.0.N.
d="$(setup badversion)"
for v in 0.1.0 0.0.03 v0.0.3 "0.0.3;x"; do
  rc=0
  rel "$d" "$v" > "$d/out" 2>&1 || rc=$?
  check "badversion_${v//[^a-z0-9]/_}_usage" same "$rc" 2
done
check badversion_no_gate same "$(calls "$d")" 0

if [[ "$fails" -ne 0 ]]; then
  echo "release_test: $fails Pruefung(en) rot" >&2
  exit 1
fi
echo "release_test: alle Pruefungen gruen"
