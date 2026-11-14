#!/usr/bin/env bash
# The verdict of an escape run in the job summary (HUM-021, shared by the jobs
# escape-tests and escape-tests-podman since HUM-435, and escape-tests-qemu
# since HUM-445).
#
#   scripts/ci/escape-verdict.sh JOB
#
# Says out loud what the run tolerated: a run without red probes is a pass, a
# red run that ESCAPE_ALLOW_FAIL let through is a skip, a run without a
# verdict is a failure. The counts come from field 4 of the fixed-format
# RESULT lines in target/escape/results.txt, never from a substring: the
# detail behind the status is captured command output, and a passing probe
# whose evidence says "fail" must not change the count. The summary never
# softens the verdict: a failed run exits 1. With ESCAPE_NO_SKIP=1 a skipped
# probe is red as well (the Podman job, HUM-435, and the kernel level, HUM-445).
set -euo pipefail

job="${1:?usage: scripts/ci/escape-verdict.sh JOB}"
summary="${GITHUB_STEP_SUMMARY:-/dev/stdout}"

# The placeholder writes its own summary block.
[[ -x tests/escape/run.sh ]] || exit 0
results=target/escape/results.txt
if [[ ! -f "$results" ]]; then
  {
    echo "### $job: failed (no verdict)"
    echo
    echo "No results file: the suite did not get as far as writing one."
  } >> "$summary"
  exit 1
fi
# The job log needs admin rights; an annotation names each red case for
# everyone who can see the run.
# A skip is named too when ESCAPE_NO_SKIP=1 makes it red, with the reason.
awk '$1 == "RESULT" && ($4 == "fail" || $4 == "error" || ($4 == "skip" && strict == 1)) {
  detail = ""
  for (i = 5; i <= NF; i++) detail = detail " " $i
  gsub(/%/, "%25", detail)
  printf "::error title=escape %s::%s/%s %s:%s\n", job, $2, $3, $4, substr(detail, 1, 300) }' \
  job="$job" strict="${ESCAPE_NO_SKIP:-0}" "$results"
count() { awk -v s="$1" '$1 == "RESULT" && $4 == s { n++ } END { print n + 0 }' "$results"; }
total=$(awk '$1 == "RESULT" { n++ } END { print n + 0 }' "$results")
pass=$(count pass)
fail=$(count fail)
skip=$(count skip)
error=$(count error)
if ((total == 0 || error > 0)); then
  verdict="failed (no verdict)"
  note="A suite did not start or did not run to its end. ESCAPE_ALLOW_FAIL does not cover this; the job is red."
elif ((fail == 0 && skip > 0)) && [[ "${ESCAPE_NO_SKIP:-0}" == 1 ]]; then
  verdict="failed"
  note="$skip skipped probe(s), and ESCAPE_NO_SKIP=1 allows none: this backend must run every probe."
elif ((fail == 0)); then
  verdict="passed"
  note="Every probe passed or was skipped."
elif [[ "${ESCAPE_ALLOW_FAIL:-0}" == 1 ]]; then
  verdict="skipped"
  note="$fail red probe(s) tolerated by ESCAPE_ALLOW_FAIL; nothing was enforced. See tests/escape/README.md for the issue behind each one."
  echo "::notice::escape tests not fully enforced; see tests/escape/README.md"
else
  verdict="failed"
  note="$fail red probe(s); see tests/escape/README.md."
fi
{
  echo "### $job: $verdict"
  echo
  echo "| status | cases |"
  echo "|---|---|"
  printf '| %s | %s |\n' pass "$pass" fail "$fail" skip "$skip" error "$error"
  echo
  echo "$note"
} >> "$summary"
case "$verdict" in
  failed*) exit 1 ;;
esac
