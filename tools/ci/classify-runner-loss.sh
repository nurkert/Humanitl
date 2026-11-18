#!/usr/bin/env bash
# Hat ein Job dieses Laufs nie einen Runner bekommen? (HUM-531)
#
# Aufruf: tools/ci/classify-runner-loss.sh <job-name>
#
# Ausgabe auf stdout genau eine Zeile `fallback=true` oder `fallback=false`,
# gedacht fuer `>> "$GITHUB_OUTPUT"`; die Begruendung steht auf stderr.
#
# Der Job `classify-macos` in release.yml fragt damit, ob `build-macos`
# abgebrochen wurde, bevor GitHub ihm einen Runner zuteilte, und startet nur
# dann den Rueckfall auf einem anderen macOS-Runner. `needs.<job>.result`
# allein unterscheidet das nicht: `cancelled` sagt nichts ueber den Runner.
# `fallback=true` heisst deshalb, alles zugleich:
#
#   - der Job endete mit `conclusion = cancelled`,
#   - `runner_name` ist leer,
#   - kein Schritt hat begonnen (`steps` leer oder alle ohne `started_at`),
#   - der Check-Run des Jobs traegt die Annotation "The job was not acquired
#     by Runner of type hosted even after multiple attempts". Genau dieser
#     Text stand am 2026-11-17 am abgebrochenen build-macos von v0.4.0.
#
# Ohne die Annotation ist es ein Abbruch von Hand in der Warteschlange, und
# der bleibt ohne Rueckfall. Ein gescheiterter Schritt (`failure` mit Runner)
# und ein Abbruch nach dem Start (`cancelled` mit Runner) ergeben ebenfalls
# `false`: Beides bleibt rot.
#
# Gelesen wird die Actions-API wie in tools/ci-gate.sh:
#   GET /repos/{repo}/actions/runs/{run}/attempts/{attempt}/jobs
#   GET /repos/{repo}/check-runs/{check_run_id}/annotations
#
# Umgebung, vor allem fuer den Test (`tools/tests/classify_runner_loss_test.sh`):
#   HUMANITL_CLASSIFY_RESULT  `needs.<job>.result` im Workflow. Bei `success`
#                         und `skipped` steht das Ergebnis ohne Abfrage fest
#                         (`false`); so haengt ein gruener Hauptpfad nie an
#                         der API.
#   HUMANITL_CLASSIFY_FORCE   `true`: `fallback=true` ohne Abfrage. Das ist
#                         der Schalter des Probelaufs, der den Hauptpfad
#                         ueberspringt (release-dry/hum-NNN-macos-fallback);
#                         release.yml setzt ihn nie fuer einen Tag.
#   HUMANITL_CI_REPO      owner/name (im Workflow github.repository)
#   HUMANITL_CI_RUN_ID, HUMANITL_CI_RUN_ATTEMPT  der Lauf und sein Versuch
#                         (github.run_id, github.run_attempt)
#   HUMANITL_CI_API       Basis der API, Vorgabe https://api.github.com
#   HUMANITL_CI_FETCH     Befehl, der eine URL als Argument nimmt und die
#                         Antwort auf stdout schreibt; Vorgabe ist curl. Der
#                         Test setzt hier ein Skript ein, das Dateien liefert.
#   HUMANITL_CI_TOKEN, GITHUB_TOKEN  optional, als Authorization-Kopf
#   HUMANITL_CLASSIFY_TRIES   wie oft die Annotationen gelesen werden, bevor
#                         ihr Fehlen zaehlt, Vorgabe 3. Sie koennten kurz nach
#                         dem Abbruch noch fehlen.
#   HUMANITL_CLASSIFY_DELAY   Sekunden zwischen zwei Versuchen, Vorgabe 20
#
# Exit-Codes: 0 mit Ausgabe; 2 Aufruf falsch, API nicht lesbar oder der Job
# fehlt in der Liste. Dann gibt es keine Ausgabe, der Job classify-* ist rot,
# und der Lauf bleibt es auch: Ein Rueckfall ohne Grund waere schlimmer.
set -euo pipefail

usage() {
  echo "usage: tools/ci/classify-runner-loss.sh <job-name>" >&2
  exit 2
}

[[ $# -eq 1 ]] || usage
job="$1"
[[ "$job" =~ ^[A-Za-z0-9_.-]+$ ]] || usage

say() { echo "classify-runner-loss: $*" >&2; }
verdict() {
  say "$job: $2"
  echo "fallback=$1"
  exit 0
}

if [[ "${HUMANITL_CLASSIFY_FORCE:-false}" == true ]]; then
  verdict true "forced by the dry-run switch, the main path was skipped on purpose"
fi
case "${HUMANITL_CLASSIFY_RESULT:-}" in
  success | skipped) verdict false "result ${HUMANITL_CLASSIFY_RESULT}, nothing to fall back from" ;;
esac

repo="${HUMANITL_CI_REPO:-}"
run_id="${HUMANITL_CI_RUN_ID:-}"
attempt="${HUMANITL_CI_RUN_ATTEMPT:-}"
api="${HUMANITL_CI_API:-https://api.github.com}"
tries="${HUMANITL_CLASSIFY_TRIES:-3}"
delay="${HUMANITL_CLASSIFY_DELAY:-20}"
[[ "$repo" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || { say "HUMANITL_CI_REPO is not owner/name: '$repo'"; exit 2; }
[[ "$run_id" =~ ^[0-9]+$ ]] || { say "HUMANITL_CI_RUN_ID is not a number: '$run_id'"; exit 2; }
[[ "$attempt" =~ ^[0-9]+$ ]] || { say "HUMANITL_CI_RUN_ATTEMPT is not a number: '$attempt'"; exit 2; }
[[ "$tries" =~ ^[1-9][0-9]*$ ]] || { say "HUMANITL_CLASSIFY_TRIES is not a positive number: '$tries'"; exit 2; }
[[ "$delay" =~ ^[0-9]+$ ]] || { say "HUMANITL_CLASSIFY_DELAY is not a number: '$delay'"; exit 2; }
token="${HUMANITL_CI_TOKEN:-${GITHUB_TOKEN:-}}"

fetch() {
  if [[ -n "${HUMANITL_CI_FETCH:-}" ]]; then
    # Absichtlich ohne Anfuehrungszeichen: der Befehl darf Argumente tragen.
    # shellcheck disable=SC2086
    $HUMANITL_CI_FETCH "$1"
  else
    local auth=()
    [[ -n "$token" ]] && auth=(-H "Authorization: Bearer $token")
    curl -sS --fail --retry 3 --retry-delay 5 \
      -H "Accept: application/vnd.github+json" \
      -H "X-GitHub-Api-Version: 2022-11-28" "${auth[@]}" "$1"
  fi
}

# Die Jobs genau dieses Versuchs: Nach "Re-run failed jobs" zaehlt der neue
# Versuch des Jobs, nicht der abgebrochene des vorigen.
jobs_url="$api/repos/$repo/actions/runs/$run_id/attempts/$attempt/jobs?per_page=100"
jobs_body="$(fetch "$jobs_url")" || { say "cannot read $jobs_url"; exit 2; }

# Eine Zeile: "lost <check-run-url>" oder "kept <grund>".
line="$(CLASSIFY_JOB="$job" python3 -I -c '
import json, os, sys
name = os.environ["CLASSIFY_JOB"]
jobs = [j for j in (json.load(sys.stdin).get("jobs") or []) if j.get("name") == name]
if len(jobs) != 1:
    sys.exit("found %d jobs named %s" % (len(jobs), name))
j = jobs[0]
conclusion = j.get("conclusion") or j.get("status") or "-"
runner = j.get("runner_name") or ""
started = [s.get("name", "?") for s in (j.get("steps") or []) if s.get("started_at")]
if conclusion != "cancelled":
    print("kept conclusion %s" % conclusion)
elif runner:
    print("kept cancelled on runner %s" % runner)
elif started:
    print("kept cancelled after step %s began" % started[0])
else:
    print("lost %s" % (j.get("check_run_url") or ""))
' <<< "$jobs_body")" || { say "the job list of run $run_id attempt $attempt is unusable"; exit 2; }

case "$line" in
  kept\ *) verdict false "${line#kept }" ;;
  lost\ *) check_run_url="${line#lost }" ;;
  *) say "unexpected classification '$line'"; exit 2 ;;
esac

# Nur eine Adresse dieses Repositorys, nie eine beliebige aus der Antwort.
if [[ ! "$check_run_url" =~ ^${api//./\\.}/repos/${repo//./\\.}/check-runs/[0-9]+$ ]]; then
  say "unexpected check_run_url '$check_run_url'"
  exit 2
fi

i=1
while :; do
  ann_body="$(fetch "$check_run_url/annotations?per_page=100")" || { say "cannot read the annotations of $check_run_url"; exit 2; }
  found="$(python3 -I -c '
import json, sys
want = "The job was not acquired by Runner of type hosted even after multiple attempts"
data = json.load(sys.stdin)
if not isinstance(data, list):
    sys.exit("annotations are not a list")
print("yes" if any(want in (a.get("message") or "") for a in data) else "no")
' <<< "$ann_body")" || { say "the annotations of $check_run_url are unusable"; exit 2; }
  if [[ "$found" == yes ]]; then
    verdict true "cancelled before a runner was assigned (GitHub: not acquired by a hosted runner)"
  fi
  if ((i >= tries)); then
    verdict false "cancelled in the queue without the runner annotation, a cancel by hand"
  fi
  i=$((i + 1))
  sleep "$delay"
done
