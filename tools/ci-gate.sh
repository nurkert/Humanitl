#!/usr/bin/env bash
# Wartet auf den CI-Lauf eines Commits und sagt, ob er gruen ist (HUM-237).
#
# Aufruf: tools/ci-gate.sh <sha|branch> [timeout-sekunden]
#
# Das Skript ist das Tor zwischen einem gepushten Issue-Zweig und `main`: Erst
# wenn der Lauf von `.github/workflows/ci.yml` fuer genau diesen Commit mit
# `success` endet, darf der Zweig gemergt werden. Es liest die oeffentliche
# Actions-API von GitHub mit `curl`; `gh` fehlt auf dem Entwicklungsrechner,
# und ein Token braucht ein oeffentliches Repository fuer diese Abfrage nicht.
#
# Ausgabe: eine Zeile je Job mit seinem Ergebnis, bei einem roten Job dazu der
# Name des Schritts, der fiel. Das Job-Log selbst verlangt Admin-Rechte; der
# Schrittname und die Annotationen auf der Seite des Laufs genuegen meistens.
#
# Exit-Codes:
#   0  der Lauf endete mit success
#   1  der Lauf endete rot (failure, timed_out, action_required, ...)
#   2  Aufruf falsch, Commit unbekannt oder die API nicht lesbar
#   3  kein Urteil: kein Lauf binnen der Frist, oder der Lauf wurde abgebrochen
#      (ein neuerer Push auf denselben Branch ersetzt ihn, siehe `concurrency`;
#      das gilt nur auf Branches, auf main hat jeder Commit seinen eigenen
#      Lauf, HUM-238)
#
# Umgebung, vor allem fuer den Test (`tools/tests/ci_gate_test.sh`):
#   HUMANITL_CI_REPO      owner/name; sonst aus `git remote get-url origin`
#   HUMANITL_CI_API       Basis der API, Vorgabe https://api.github.com
#   HUMANITL_CI_FETCH     Befehl, der eine URL als Argument nimmt und die
#                         Antwort auf stdout schreibt; Vorgabe ist curl. Der
#                         Test setzt hier ein Skript ein, das Dateien liefert.
#                         Vertrag: Exit 0 mit dem JSON, Exit 75 bei erreichter
#                         Ratengrenze mit den Sekunden bis zum Ende der Sperre
#                         auf stdout, jeder andere Exit ist ein Fehler.
#   HUMANITL_CI_POLL      Sekunden zwischen zwei Abfragen, Vorgabe 60. Ohne
#                         Token erlaubt die API 60 Anfragen je Stunde und
#                         Adresse; alle Agenten des Rechners teilen sie sich.
#   HUMANITL_CI_FIRST_WAIT  Sekunden vor der zweiten Abfrage, wenn die erste
#                         noch keinen fertigen Lauf fand, Vorgabe 300. Ein Lauf
#                         braucht ohnehin rund zehn Minuten; frueheres Fragen
#                         kostet nur Anfragen.
#   HUMANITL_CI_TOKEN, GITHUB_TOKEN  optional; ist eines gesetzt, geht es als
#                         Authorization-Kopf mit (5000 statt 60 Anfragen je
#                         Stunde). Noetig ist es nicht.
#   HUMANITL_CI_WORKFLOW  Dateiname des Workflows, Vorgabe ci.yml
set -euo pipefail
cd "$(dirname "$0")/.."

usage() {
  echo "usage: tools/ci-gate.sh <sha|branch> [timeout-seconds]" >&2
  exit 2
}

[[ $# -ge 1 && $# -le 2 ]] || usage
ref="$1"
limit="${2:-3600}"
[[ "$limit" =~ ^[0-9]+$ ]] || usage
poll="${HUMANITL_CI_POLL:-60}"
[[ "$poll" =~ ^[0-9]+$ ]] || { echo "ci-gate: HUMANITL_CI_POLL ist keine Zahl: $poll" >&2; exit 2; }
workflow="${HUMANITL_CI_WORKFLOW:-ci.yml}"
api="${HUMANITL_CI_API:-https://api.github.com}"

# Eine volle SHA wird so genommen, wie sie ist: Sie muss nicht lokal vorliegen,
# etwa wenn jemand den Lauf eines Commits abfragt, den er nie geholt hat. Alles
# andere (Zweig, kurze SHA, `HEAD`) loest git auf. Ein Zweigname meint den
# lokalen Zweig; wer ihn gepusht hat, hat denselben Commit auf beiden Seiten.
if [[ "$ref" =~ ^[0-9a-f]{40}$ ]]; then
  sha="$ref"
else
  sha="$(git rev-parse --verify --quiet "$ref^{commit}")" || {
    echo "ci-gate: $ref ist kein Commit in diesem Repository" >&2
    exit 2
  }
fi

repo="${HUMANITL_CI_REPO:-}"
if [[ -z "$repo" ]]; then
  url="$(git remote get-url origin 2> /dev/null || true)"
  # git@github.com:owner/name.git und https://github.com/owner/name(.git)
  repo="$(sed -nE 's#^(git@|ssh://git@|https://)github\.com[:/]([^/]+/[^/]+)$#\2#p' <<< "$url")"
  repo="${repo%.git}"
fi
if [[ ! "$repo" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
  echo "ci-gate: Repository nicht erkannt (origin: ${url:-keins}); setze HUMANITL_CI_REPO=owner/name" >&2
  exit 2
fi

first_wait="${HUMANITL_CI_FIRST_WAIT:-300}"
[[ "$first_wait" =~ ^[0-9]+$ ]] || { echo "ci-gate: HUMANITL_CI_FIRST_WAIT ist keine Zahl: $first_wait" >&2; exit 2; }
token="${HUMANITL_CI_TOKEN:-${GITHUB_TOKEN:-}}"

# Abfrage mit curl nach dem Vertrag von HUMANITL_CI_FETCH (siehe oben). Die
# Antwortkoepfe werden mitgelesen, damit eine erreichte Ratengrenze von einem
# echten Fehler zu unterscheiden ist: 429, oder 403 mit
# `x-ratelimit-remaining: 0` oder mit `retry-after`.
fetch_curl() {
  local tmp code rc=0 wait auth=()
  [[ -n "$token" ]] && auth=(-H "Authorization: Bearer $token")
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/ci-gate.XXXXXX")"
  code="$(curl -sS --retry 3 --retry-delay 5 -D "$tmp/headers" -o "$tmp/body" -w '%{http_code}' \
    -H "Accept: application/vnd.github+json" \
    -H "X-GitHub-Api-Version: 2022-11-28" "${auth[@]}" "$1")" || code=000
  case "$code" in
    2??) cat "$tmp/body" ;;
    403 | 429)
      wait="$(python3 -I -c '
import sys, time
h = {}
for line in open(sys.argv[1], errors="replace"):
    if ":" in line:
        k, v = line.split(":", 1)
        h[k.strip().lower()] = v.strip()
retry = h.get("retry-after", "")
if retry.isdigit():
    print(int(retry))
elif h.get("x-ratelimit-remaining") == "0" and h.get("x-ratelimit-reset", "").isdigit():
    print(max(0, int(h["x-ratelimit-reset"]) - int(time.time())) + 1)
elif sys.argv[2] == "429":
    print(60)
' "$tmp/headers" "$code")"
      if [[ -n "$wait" ]]; then
        echo "$wait"
        rc=75
      else
        echo "ci-gate: HTTP $code fuer $1" >&2
        rc=1
      fi
      ;;
    *)
      echo "ci-gate: HTTP $code fuer $1" >&2
      rc=1
      ;;
  esac
  rm -rf "$tmp"
  return "$rc"
}

fetch() {
  if [[ -n "${HUMANITL_CI_FETCH:-}" ]]; then
    # Absichtlich ohne Anfuehrungszeichen: der Befehl darf Argumente tragen.
    # shellcheck disable=SC2086
    $HUMANITL_CI_FETCH "$1"
  else
    fetch_curl "$1"
  fi
}

# Schlaeft hoechstens bis zur Frist.
nap() {
  local left=$((limit - ($(date +%s) - start)))
  ((left > 0)) || return 0
  sleep "$(($1 < left ? $1 : left))"
}

# Der juengste Lauf des Workflows fuer diesen Commit, als eine Zeile
# "<id> <status> <conclusion> <url>", oder "none". Juengster heisst: hoechste
# run_number, bei gleicher Nummer der hoechste Versuch; so zaehlt ein
# erneut gestarteter Lauf und nicht sein roter Vorgaenger.
latest_run() {
  python3 -I -c '
import json, sys
runs = json.load(sys.stdin).get("workflow_runs") or []
if not runs:
    print("none")
else:
    r = max(runs, key=lambda r: (r.get("run_number", 0), r.get("run_attempt", 0)))
    print(r["id"], r.get("status") or "-", r.get("conclusion") or "-", r.get("html_url") or "-")
'
}

# Je Job eine Zeile; bei rotem Job die Schritte, die nicht gruen endeten.
report_jobs() {
  python3 -I -c '
import json, sys
jobs = json.load(sys.stdin).get("jobs") or []
for j in sorted(jobs, key=lambda j: j.get("name", "")):
    name = j.get("name", "?")
    conclusion = j.get("conclusion") or j.get("status") or "-"
    print("  %s: %s" % (name, conclusion))
    if conclusion in ("success", "skipped", "neutral"):
        continue
    for s in j.get("steps") or []:
        if s.get("conclusion") in ("failure", "timed_out", "cancelled"):
            print("    step %s \"%s\": %s" % (s.get("number", "?"), s.get("name", "?"), s["conclusion"]))
'
}

# Nur Push-Laeufe: Ein Lauf fuer einen Pull Request derselben SHA prueft einen
# anderen Stand (den Merge mit dem Ziel) und darf das Tor nicht oeffnen.
runs_url="$api/repos/$repo/actions/workflows/$workflow/runs?head_sha=$sha&event=push&per_page=20"
short="${sha:0:12}"
echo "ci-gate: warte auf $workflow fuer $short in $repo (Frist ${limit}s)"
start=$(date +%s)
last=""
first=1
polls=0
while :; do
  # Scheitert schon die erste Abfrage, stimmt meist etwas am Aufruf (falsches
  # Repository, kein Netz), und eine Stunde Warten verdeckte das nur. Spaeter
  # ist ein Fehler eher voruebergehend; dann wartet das Skript weiter bis zur
  # Frist. Eine erreichte Ratengrenze (Exit 75) ist nie ein Fehler: Das Skript
  # schlaeft bis zum Ende der Sperre und fragt dann erneut.
  frc=0
  body="$(fetch "$runs_url")" || frc=$?
  if ((frc == 75)); then
    [[ "$body" =~ ^[0-9]+$ ]] || body=60
    echo "ci-gate: Ratengrenze der API erreicht, warte ${body}s" >&2
    if (($(date +%s) - start >= limit)); then
      echo "ci-gate: kein Urteil binnen ${limit}s fuer $short" >&2
      exit 3
    fi
    nap "$body"
    continue
  fi
  if ((frc != 0)); then
    if ((first)); then
      echo "ci-gate: API nicht lesbar: $runs_url" >&2
      exit 2
    fi
    echo "ci-gate: Abfrage gescheitert, versuche es in ${poll}s erneut" >&2
    if (($(date +%s) - start >= limit)); then
      echo "ci-gate: kein Urteil binnen ${limit}s fuer $short" >&2
      exit 3
    fi
    nap "$poll"
    continue
  fi
  first=0
  polls=$((polls + 1))
  if ! line="$(latest_run <<< "$body")"; then
    echo "ci-gate: Antwort der API ist kein JSON der erwarteten Form" >&2
    exit 2
  fi
  read -r run_id status conclusion html <<< "$line"
  if [[ "$status" == "completed" ]]; then
    break
  fi
  # Nur bei einem Wechsel eine Zeile, sonst fuellt eine Stunde Warten das
  # Terminal mit 120 gleichen Zeilen.
  if [[ "$line" != "$last" ]]; then
    if [[ "$run_id" == "none" ]]; then
      echo "ci-gate: noch kein Lauf fuer $short"
    else
      echo "ci-gate: Lauf $run_id ist $status ($html)"
    fi
    last="$line"
  fi
  if (($(date +%s) - start >= limit)); then
    echo "ci-gate: kein Urteil binnen ${limit}s fuer $short (zuletzt: $line)" >&2
    exit 3
  fi
  # Nach der ersten Abfrage einmal laenger warten: Ein frisch gepushter Lauf
  # braucht ohnehin Minuten, und jede Anfrage zaehlt gegen die Grenze.
  if ((polls == 1 && first_wait > poll)); then
    nap "$first_wait"
  else
    nap "$poll"
  fi
done

echo "ci-gate: Lauf $run_id: $conclusion ($html)"
# Die Jobliste ist nur Auskunft; das Urteil steht schon im Lauf. Scheitert
# ihre Abfrage, entscheidet das Skript trotzdem und sagt, was fehlt.
if jobs_body="$(fetch "$api/repos/$repo/actions/runs/$run_id/jobs?per_page=100")"; then
  report_jobs <<< "$jobs_body" || echo "ci-gate: Jobliste nicht lesbar" >&2
else
  echo "ci-gate: Jobs des Laufs $run_id nicht lesbar; Einzelheiten auf $html" >&2
fi

case "$conclusion" in
  success)
    echo "ci-gate: gruen"
    exit 0
    ;;
  cancelled | stale | skipped)
    echo "ci-gate: kein Urteil ($conclusion); ein neuerer Lauf ersetzt ihn oder er wurde abgebrochen" >&2
    exit 3
    ;;
  *)
    echo "ci-gate: rot ($conclusion); nicht mergen" >&2
    exit 1
    ;;
esac
