#!/usr/bin/env bash
# Test fuer tools/ci/classify-runner-loss.sh (HUM-531) gegen eine
# nachgestellte Actions-API.
#
# Jeder Fall ist ein Verzeichnis unter tools/tests/fixtures/classify-runner-loss/
# mit der Job-Liste eines Versuchs (jobs.json) und den Annotationen des
# Check-Runs von build-macos (annotations.json). Statt curl liefert ein kleines
# Skript diese Dateien aus (HUMANITL_CI_FETCH), wie in ci_gate_test.sh. Der
# Fall no-runner ist der echte build-macos von v0.4.0 (Lauf 44120430252,
# Versuch 1), den GitHub am 2026-11-17 ohne Runner abbrach. failed-step und
# cancelled-started tragen absichtlich dieselbe Annotation: So zeigt jeder
# Fall, dass die Bedingung an der Job-Liste allein schon entscheidet, und die
# Annotation nicht stellvertretend fuer sie einspringt.
#
# Mutationsbeweis: CLASSIFY=<pfad> prueft eine veraenderte Kopie des Skripts.
# Eine Kopie, die nur `conclusion != success` fragt, muss failed-step rot
# machen; eine ohne die Pruefung der Annotation hand-cancel.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
classify="${CLASSIFY:-$here/../ci/classify-runner-loss.sh}"
fixtures="$here/fixtures/classify-runner-loss"

work="$(mktemp -d "${TMPDIR:-/tmp}/classify-runner-loss-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT
fails=0

# Die nachgestellte API. Jede Abfrage wird mitgeschrieben.
cat > "$work/fetch" << 'EOF'
#!/usr/bin/env bash
set -euo pipefail
echo "$1" >> "$FAKE_LOG"
case "$1" in
  */actions/runs/*/attempts/*/jobs\?*) cat "$FAKE_DIR/jobs.json" ;;
  */check-runs/*/annotations\?*) cat "$FAKE_DIR/annotations.json" ;;
  *) exit 22 ;;
esac
EOF
chmod +x "$work/fetch"

# run <fixture> [VAR=wert ...]: setzt out, err, rc und log.
run() {
  local fixture="$1"
  shift
  log="$work/log-$fixture-$RANDOM"
  : > "$log"
  rc=0
  out="$(env FAKE_DIR="$fixtures/$fixture" FAKE_LOG="$log" HUMANITL_CI_FETCH="$work/fetch" \
    HUMANITL_CI_API=https://api.github.com HUMANITL_CI_REPO=nurkert/Humanitl \
    HUMANITL_CI_RUN_ID=44120430252 HUMANITL_CI_RUN_ATTEMPT=1 \
    HUMANITL_CLASSIFY_TRIES=2 HUMANITL_CLASSIFY_DELAY=0 \
    "$@" "$classify" build-macos 2> "$work/err")" || rc=$?
  err="$(cat "$work/err")"
}

check() { # <name> <bedingung als befehl...>
  local name="$1"
  shift
  if "$@" > /dev/null 2>&1; then
    echo "ok   $name"
  else
    echo "FAIL $name"
    echo "    rc=$rc out=$out"
    sed 's/^/    /' <<< "$err"
    fails=$((fails + 1))
  fi
}
same() { [[ "$1" == "$2" ]]; }
not_grep() { ! grep -q "$@"; }

# 1. Abgebrochen ohne Runner, ohne Schritte, mit der Annotation: Rueckfall.
run no-runner
check no_runner_falls_back same "$out" "fallback=true"
check no_runner_exits_0 same "$rc" 0
check no_runner_reads_this_attempt grep -q "/actions/runs/44120430252/attempts/1/jobs?per_page=100" "$log"
check no_runner_reads_its_check_run grep -q "/check-runs/134170130703/annotations" "$log"

# 2. Dasselbe ohne die Annotation: Abbruch von Hand in der Warteschlange.
#    Die Annotationen werden HUMANITL_CLASSIFY_TRIES-mal gelesen.
run hand-cancel
check hand_cancel_no_fallback same "$out" "fallback=false"
check hand_cancel_exits_0 same "$rc" 0
check hand_cancel_says_by_hand grep -q "cancel by hand" <<< "$err"
check hand_cancel_retries same "$(grep -c annotations "$log")" 2

# 3. Gescheiterter Schritt auf einem Runner: bleibt rot, kein Rueckfall.
run failed-step
check failed_step_no_fallback same "$out" "fallback=false"
check failed_step_no_annotation_read not_grep annotations "$log"
check failed_step_says_failure grep -q "conclusion failure" <<< "$err"

# 4. Nach dem Start abgebrochen (Runner, begonnene Schritte): kein Rueckfall.
run cancelled-started
check cancelled_started_no_fallback same "$out" "fallback=false"
check cancelled_started_says_runner grep -q "cancelled on runner" <<< "$err"

# 5. Gruen: kein Rueckfall.
run success
check success_no_fallback same "$out" "fallback=false"

# Ohne Runner, aber ein Schritt hat begonnen (started_at gesetzt): kein
# Rueckfall, auch mit der Annotation.
mkdir -p "$work/fx/step-began"
python3 -I -c '
import json, sys
d = json.load(open(sys.argv[1]))
for j in d["jobs"]:
    if j["name"] == "build-macos":
        j["steps"] = [{"name": "Set up job", "status": "completed", "conclusion": "cancelled", "number": 1, "started_at": "2026-11-17T16:00:01Z"}]
json.dump(d, open(sys.argv[2], "w"))
' "$fixtures/no-runner/jobs.json" "$work/fx/step-began/jobs.json"
cp "$fixtures/no-runner/annotations.json" "$work/fx/step-began/"
fixtures_saved="$fixtures"
fixtures="$work/fx"
run step-began
fixtures="$fixtures_saved"
check step_began_no_fallback same "$out" "fallback=false"

# Abkuerzungen ohne Abfrage: ein gruener oder uebersprungener Hauptpfad
# haengt nie an der API, und der Schalter des Probelaufs erzwingt den Rueckfall.
run no-runner HUMANITL_CLASSIFY_RESULT=success
check result_success_no_fallback same "$out" "fallback=false"
check result_success_no_api same "$(wc -l < "$log" | tr -d ' ')" 0
run no-runner HUMANITL_CLASSIFY_RESULT=skipped
check result_skipped_no_fallback same "$out" "fallback=false"
run success HUMANITL_CLASSIFY_FORCE=true
check force_falls_back same "$out" "fallback=true"
check force_no_api same "$(wc -l < "$log" | tr -d ' ')" 0
# needs.<job>.result = cancelled fragt die API wie ohne Angabe.
run no-runner HUMANITL_CLASSIFY_RESULT=cancelled
check result_cancelled_asks_api same "$out" "fallback=true"

# Fehler geben keine Ausgabe und Exit 2: kein Rueckfall aus Versehen.
run no-runner HUMANITL_CI_FETCH=false
check api_down_exit_2 same "$rc" 2
check api_down_no_output same "$out" ""
run no-runner HUMANITL_CI_REPO=other/repo
check foreign_check_run_url_refused same "$rc" 2
check foreign_check_run_url_no_output same "$out" ""
rc=0
out="$("$classify" 2> /dev/null)" || rc=$?
check no_job_name_usage same "$rc" 2
rc=0
out="$(HUMANITL_CI_FETCH="$work/fetch" FAKE_DIR="$fixtures/no-runner" FAKE_LOG=/dev/null HUMANITL_CI_REPO=nurkert/Humanitl \
  HUMANITL_CI_RUN_ID=1 HUMANITL_CI_RUN_ATTEMPT=1 "$classify" check-macos 2> /dev/null)" || rc=$?
check unknown_job_exit_2 same "$rc" 2

if [[ "$fails" -ne 0 ]]; then
  echo "classify_runner_loss_test: $fails Pruefung(en) rot" >&2
  exit 1
fi
echo "classify_runner_loss_test: alle Pruefungen gruen"
