#!/usr/bin/env bash
# Test fuer tools/ci-gate.sh (HUM-237) gegen eine nachgestellte Actions-API.
#
# Statt curl liefert ein kleines Skript Antworten aus Dateien
# (HUMANITL_CI_FETCH). Jeder Fall legt fest, was die Abfrage der Laeufe der
# Reihe nach zurueckgibt, und prueft Exit-Code und Ausgabe des Tors.
#
# Mutationsbeweis: CI_GATE=<pfad> prueft eine veraenderte Kopie des Skripts.
# Eine Kopie, die `failure` als gruen meldet oder `success` als rot, muss
# diesen Test rot machen.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
gate="${CI_GATE:-$here/../ci-gate.sh}"

work="$(mktemp -d "${TMPDIR:-/tmp}/ci-gate-test.XXXXXX")"
trap 'rm -rf "$work"' EXIT

sha=0123456789abcdef0123456789abcdef01234567
fails=0

# Die nachgestellte API: Abfragen der Laeufe bekommen runs-1.json,
# runs-2.json, ... in dieser Reihenfolge, die letzte Datei bleibt stehen;
# Abfragen der Jobs bekommen jobs.json. Wo runs-N.fail liegt, scheitert die
# N-te Abfrage wie curl bei einem HTTP-Fehler.
cat > "$work/fetch" << 'EOF'
#!/usr/bin/env bash
set -euo pipefail
dir="$CI_GATE_FAKE"
echo "$1" >> "$dir/urls"
case "$1" in
  */actions/workflows/*/runs\?*)
    n=$(($(cat "$dir/count" 2> /dev/null || echo 0) + 1))
    echo "$n" > "$dir/count"
    [[ -f "$dir/runs-$n.fail" ]] && exit 22
    [[ -f "$dir/runs-$n.rate" ]] && { cat "$dir/runs-$n.rate"; exit 75; }
    f="$dir/runs-$n.json"
    while [[ ! -f "$f" && $n -gt 1 ]]; do n=$((n - 1)); f="$dir/runs-$n.json"; done
    [[ -f "$f" ]] && cat "$f" || exit 22
    ;;
  */actions/runs/*/jobs\?*)
    [[ -f "$dir/jobs.fail" ]] && exit 22
    cat "$dir/jobs.json"
    ;;
  *) exit 22 ;;
esac
EOF
chmod +x "$work/fetch"

run_json() { # id status conclusion [run_number] [attempt]
  printf '{"id": %s, "status": "%s", "conclusion": %s, "run_number": %s, "run_attempt": %s, "html_url": "https://example.invalid/runs/%s"}' \
    "$1" "$2" "$3" "${4:-1}" "${5:-1}" "$1"
}

jobs_green='{"jobs": [
  {"name": "rust-test", "conclusion": "success", "steps": []},
  {"name": "goldens", "conclusion": "success", "steps": []}]}'
jobs_red='{"jobs": [
  {"name": "rust-test", "conclusion": "success", "steps": []},
  {"name": "flutter-analyze-test", "conclusion": "failure", "steps": [
    {"number": 9, "name": "Analyze", "conclusion": "success"},
    {"number": 10, "name": "Test", "conclusion": "failure"},
    {"number": 11, "name": "Design system package", "conclusion": "skipped"}]}]}'

# case <name> <erwarteter exit> <muster in der ausgabe> <timeout> -- <runs-1> [<runs-2> ...]
case_() {
  local name="$1" want="$2" pattern="$3" limit="$4"
  shift 5
  local dir="$work/$name"
  mkdir -p "$dir"
  local i=1
  for body in "$@"; do
    if [[ "$body" == RATE* ]]; then
      printf '%s' "${body#RATE}" > "$dir/runs-$i.rate"
    elif [[ "$body" == "FAIL" ]]; then
      : > "$dir/runs-$i.fail"
    else
      printf '%s' "$body" > "$dir/runs-$i.json"
    fi
    i=$((i + 1))
  done
  printf '%s' "${JOBS:-$jobs_green}" > "$dir/jobs.json"
  [[ -z "${JOBS_FAIL:-}" ]] || : > "$dir/jobs.fail"
  local out rc=0
  out="$(CI_GATE_FAKE="$dir" HUMANITL_CI_FETCH="$work/fetch" HUMANITL_CI_REPO=owner/name \
    HUMANITL_CI_POLL=0 HUMANITL_CI_FIRST_WAIT="${FIRST_WAIT:-0}" "$gate" "${REF:-$sha}" "$limit" 2>&1)" || rc=$?
  if [[ "$rc" -ne "$want" ]]; then
    echo "FAIL $name: exit $rc, erwartet $want"
    sed 's/^/    /' <<< "$out"
    fails=$((fails + 1))
  elif [[ -n "$pattern" ]] && ! grep -qE -- "$pattern" <<< "$out"; then
    echo "FAIL $name: Ausgabe ohne /$pattern/"
    sed 's/^/    /' <<< "$out"
    fails=$((fails + 1))
  else
    echo "ok   $name"
  fi
}

runs() { printf '{"workflow_runs": [%s]}' "$(IFS=,; echo "$*")"; }

case_ success 0 'gruen' 60 -- \
  "$(runs "$(run_json 7 completed '"success"')")"

JOBS="$jobs_red" case_ failure 1 'step 10 "Test": failure' 60 -- \
  "$(runs "$(run_json 7 completed '"failure"')")"

# Der rote Job wird genannt, der gruene nicht als Fehler.
JOBS="$jobs_red" case_ failure_names_the_job 1 'flutter-analyze-test: failure' 60 -- \
  "$(runs "$(run_json 7 completed '"failure"')")"

case_ waits_until_completed 0 'Lauf 7 ist in_progress' 60 -- \
  '{"workflow_runs": []}' \
  "$(runs "$(run_json 7 queued null)")" \
  "$(runs "$(run_json 7 in_progress null)")" \
  "$(runs "$(run_json 7 completed '"success"')")"

case_ no_run_is_no_verdict 3 'kein Urteil' 0 -- \
  '{"workflow_runs": []}'

case_ running_past_the_limit_is_no_verdict 3 'kein Urteil' 0 -- \
  "$(runs "$(run_json 7 in_progress null)")"

case_ cancelled_is_no_verdict 3 'cancelled' 60 -- \
  "$(runs "$(run_json 7 completed '"cancelled"')")"

case_ timed_out_is_red 1 'rot' 60 -- \
  "$(runs "$(run_json 7 completed '"timed_out"')")"

# Ein erneut gestarteter Lauf zaehlt, nicht sein roter Vorgaenger.
case_ rerun_wins 0 'Lauf 8: success' 60 -- \
  "$(runs "$(run_json 7 completed '"failure"' 5 1)" "$(run_json 8 completed '"success"' 5 2)")"

# Und ein juengerer roter Lauf schlaegt einen aelteren gruenen.
case_ newest_red_wins 1 'Lauf 9: failure' 60 -- \
  "$(runs "$(run_json 9 completed '"failure"' 6 1)" "$(run_json 8 completed '"success"' 5 1)")"

case_ first_fetch_error 2 'API nicht lesbar' 60 -- FAIL

case_ later_fetch_error_waits 0 'erneut' 60 -- \
  "$(runs "$(run_json 7 in_progress null)")" FAIL \
  "$(runs "$(run_json 7 completed '"success"')")"

REF=no-such-ref-hum237 case_ unknown_ref 2 'kein Commit' 60 -- \
  "$(runs "$(run_json 7 completed '"success"')")"

# Ratengrenze bei der ersten Abfrage: kein Fehler, sondern Warten.
case_ rate_limit_waits 0 'Ratengrenze' 60 -- \
  RATE0 "$(runs "$(run_json 7 completed '"success"')")"

# Die Jobliste scheitert nach dem Ende des Laufs: Das Urteil gilt trotzdem.
JOBS_FAIL=1 case_ jobs_fetch_fails_green 0 'Jobs des Laufs 7 nicht lesbar' 60 -- \
  "$(runs "$(run_json 7 completed '"success"')")"
JOBS_FAIL=1 case_ jobs_fetch_fails_red 1 'rot' 60 -- \
  "$(runs "$(run_json 7 completed '"failure"')")"

# Nach einer ersten Abfrage ohne fertigen Lauf wartet das Tor laenger.
t0=$(date +%s)
FIRST_WAIT=2 case_ first_wait_is_longer 0 'gruen' 60 -- \
  "$(runs "$(run_json 7 in_progress null)")" \
  "$(runs "$(run_json 7 completed '"success"')")"
if (($(date +%s) - t0 >= 2)); then
  echo "ok   first_wait_took_its_time"
else
  echo "FAIL first_wait_took_its_time"
  fails=$((fails + 1))
fi

# Der Weg ueber curl, mit einem nachgestellten curl im PATH: Er schreibt die
# Koepfe und den Koerper aus Dateien in die Ziele von -D und -o und gibt den
# Status aus; die n-te Anfrage bekommt code-n, headers-n, body-n.
mkdir -p "$work/bin"
cat > "$work/bin/curl" << 'EOF'
#!/usr/bin/env bash
dir="$CI_GATE_CURL"
n=$(($(cat "$dir/count" 2> /dev/null || echo 0) + 1))
echo "$n" > "$dir/count"
hdr="" out=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -D) hdr="$2"; shift ;;
    -o) out="$2"; shift ;;
    -H) echo "$2" >> "$dir/request-headers" ;;
  esac
  shift
done
cp "$dir/headers-$n" "$hdr" 2> /dev/null || : > "$hdr"
cp "$dir/body-$n" "$out" 2> /dev/null || : > "$out"
cat "$dir/code-$n"
EOF
chmod +x "$work/bin/curl"

# curl_case <name> <exit> <muster> -- <code:kopfzeilen|...> ...
# Jede Antwort ist "code" oder "code|Kopf: Wert|Kopf: Wert"; der Koerper ist
# fuer Laeufe ein fertiger gruener Lauf, fuer Jobs die gruene Liste.
curl_case() {
  local name="$1" want="$2" pattern="$3"
  shift 4
  local dir="$work/curl-$name" i=1 spec code
  mkdir -p "$dir"
  for spec in "$@"; do
    code="${spec%%|*}"
    echo "$code" > "$dir/code-$i"
    : > "$dir/headers-$i"
    if [[ "$spec" == *"|"* ]]; then
      tr '|' '\n' <<< "${spec#*|}" > "$dir/headers-$i"
    fi
    if [[ "$code" == 200 ]]; then
      if [[ $i -eq $# ]]; then
        printf '%s' "$jobs_green" > "$dir/body-$i"
      else
        runs "$(run_json 7 completed '"success"')" > "$dir/body-$i"
      fi
    fi
    i=$((i + 1))
  done
  local out rc=0
  out="$(PATH="$work/bin:$PATH" CI_GATE_CURL="$dir" HUMANITL_CI_REPO=owner/name \
    HUMANITL_CI_POLL=0 HUMANITL_CI_FIRST_WAIT=0 HUMANITL_CI_TOKEN="${TOKEN:-}" GITHUB_TOKEN= \
    "$gate" "$sha" 60 2>&1)" || rc=$?
  if [[ "$rc" -ne "$want" ]] || ! grep -qE -- "$pattern" <<< "$out"; then
    echo "FAIL $name: exit $rc, erwartet $want, muster /$pattern/"
    sed 's/^/    /' <<< "$out"
    fails=$((fails + 1))
  else
    echo "ok   $name"
  fi
}

TOKEN=secret-hum237 curl_case curl_green 0 'gruen' -- 200 200
if grep -qx "Authorization: Bearer secret-hum237" "$work/curl-curl_green/request-headers"; then
  echo "ok   curl_sends_token"
else
  echo "FAIL curl_sends_token"
  fails=$((fails + 1))
fi
curl_case curl_no_token_no_header 0 'gruen' -- 200 200
if ! grep -q "Authorization" "$work/curl-curl_no_token_no_header/request-headers"; then
  echo "ok   curl_without_token_sends_none"
else
  echo "FAIL curl_without_token_sends_none"
  fails=$((fails + 1))
fi
curl_case curl_403_rate_limit 0 'Ratengrenze' -- \
  "403|x-ratelimit-remaining: 0|x-ratelimit-reset: 0" 200 200
curl_case curl_429_retry_after 0 'warte 0s' -- "429|retry-after: 0" 200 200
curl_case curl_403_forbidden 2 'HTTP 403' -- "403|x-ratelimit-remaining: 41" 200 200
curl_case curl_404 2 'HTTP 404' -- 404 200 200

# Die Abfrage nennt Repository, Workflow, genau diese SHA und nur Push-Laeufe.
if grep -q "/repos/owner/name/actions/workflows/ci.yml/runs?head_sha=$sha&event=push" "$work/success/urls"; then
  echo "ok   url_names_repo_workflow_and_sha"
else
  echo "FAIL url_names_repo_workflow_and_sha"
  sed 's/^/    /' "$work/success/urls"
  fails=$((fails + 1))
fi

if [[ "$fails" -ne 0 ]]; then
  echo "ci_gate_test: $fails Fall/Faelle rot" >&2
  exit 1
fi
echo "ci_gate_test: alle Faelle gruen"
