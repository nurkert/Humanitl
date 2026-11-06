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
#   1  der Lauf endete rot (failure, timed_out, action_required, ...), oder
#      er ist gruen, aber ein Job wurde uebersprungen, ohne dass die
#      Pfadregel es erlaubt (HUM-424, siehe unten)
#   2  Aufruf falsch, Commit unbekannt oder die API nicht lesbar
#   3  kein Urteil: kein Lauf binnen der Frist, oder der Lauf wurde abgebrochen
#      (ein neuerer Push auf denselben Branch ersetzt ihn, siehe `concurrency`;
#      das gilt nur auf Branches, auf main hat jeder Commit seinen eigenen
#      Lauf, HUM-238)
#   4  nur unter HUMANITL_CI_EVERY_JOB=1: der Lauf ist gruen, aber die
#      Pfadregel hat Jobs uebersprungen; ein voller Lauf steht aus (HUM-424)
#
# Pfadregel (HUM-424): In ci.yml entscheidet der Job `changes`
# (tools/ci-changes.sh), welche Jobs ein Push braucht; die uebrigen enden mit
# `skipped`, und der Lauf ist trotzdem `success`. Das Tor nimmt einen
# uebersprungenen Job deshalb nur hin, wenn `changes` mit success endete und
# dort der Schritt `skip <job>` lief. Die Ausgaben eines Jobs stehen nicht in
# der oeffentlichen API, die Ergebnisse seiner Schritte schon. Jeder andere
# uebersprungene Job macht einen gruenen Lauf rot: ein gescheitertes
# `changes`, ein Job, dessen Bereich sich geaendert hat, ein Job mit einer
# eigenen Bedingung. Ohne lesbare Jobliste gibt es fuer einen gruenen Lauf
# deshalb kein Urteil (Exit 3).
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
#   HUMANITL_CI_EVENT     Ereignis der Laeufe, die zaehlen, Vorgabe push;
#                         tools/release.sh fragt auch nach workflow_dispatch,
#                         dem vollen Lauf von Hand (HUM-424)
#   HUMANITL_CI_BRANCH    optional: nur Laeufe dieses Zweigs oder Tags zaehlen
#                         (`head_branch` des Laufs; bei einem Tag-Push ist das
#                         der Tag-Name). Noetig, wo derselbe Commit mehrere
#                         Laeufe desselben Workflows hat, etwa der Probelauf
#                         auf `release-dry/0.Y.Z` und danach der Lauf des Tags
#                         `v0.Y.Z` (HUM-363). Ohne den Filter saehe das Tor
#                         nach dem Tag-Push den schon gruenen Probelauf und
#                         meldete gruen, bevor der Tag-Lauf ueberhaupt begann.
#   HUMANITL_CI_EVERY_JOB  1: Ein Lauf mit `success` gilt nur, wenn auch jeder
#                         einzelne Job mit `success` endete. Im Probelauf von
#                         release.yml hiesse alles andere, ein uebersprungener
#                         guard nimmt alle Pruefungen mit, und der Lauf waere
#                         trotzdem gruen (HUM-363). Auch ein nach der
#                         Pfadregel uebersprungener Job zaehlt dann nicht
#                         (Exit 4): Ein Release verlangt den vollen Lauf.
#   HUMANITL_CI_MAY_SKIP  Jobnamen, durch Leerzeichen getrennt, die unter
#                         HUMANITL_CI_EVERY_JOB=1 uebersprungen sein duerfen,
#                         etwa `release` im Probelauf.
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
branch="${HUMANITL_CI_BRANCH:-}"
every_job="${HUMANITL_CI_EVERY_JOB:-0}"
event="${HUMANITL_CI_EVENT:-push}"
[[ "$event" =~ ^[a-z_]+$ ]] || { echo "ci-gate: HUMANITL_CI_EVENT ist kein Ereignis: $event" >&2; exit 2; }
may_skip="${HUMANITL_CI_MAY_SKIP:-}"
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
  CI_GATE_BRANCH="$branch" python3 -I -c '
import json, os, sys
runs = json.load(sys.stdin).get("workflow_runs") or []
want = os.environ.get("CI_GATE_BRANCH", "")
if want:
    runs = [r for r in runs if r.get("head_branch") == want]
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

# Das Urteil ueber die Jobs eines gruenen Laufs. Je Zeile `bad <text>` fuer
# einen Job, der den Lauf rot macht, oder `path <job>` fuer einen, den die
# Pfadregel uebersprang (siehe oben). Ein uebersprungener Job zaehlt nur als
# `path`, wenn der Job `changes` mit success endete und sein Schritt
# `skip <job>` mit success lief. HUMANITL_CI_MAY_SKIP gilt nur unter
# HUMANITL_CI_EVERY_JOB=1. Exit 4, wenn die Liste leer ist.
judge_jobs() {
  CI_GATE_MAY_SKIP="$may_skip" CI_GATE_EVERY="$every_job" python3 -I -c '
import json, os, sys
jobs = json.load(sys.stdin).get("jobs") or []
if not jobs:
    sys.exit(4)
every = os.environ.get("CI_GATE_EVERY") == "1"
may_skip = set(os.environ.get("CI_GATE_MAY_SKIP", "").split()) if every else set()
changes = [j for j in jobs if j.get("name") == "changes"]
plan = set()
if len(changes) == 1 and changes[0].get("conclusion") == "success":
    for s in changes[0].get("steps") or []:
        name = s.get("name") or ""
        if name.startswith("skip ") and s.get("conclusion") == "success":
            plan.add(name[len("skip "):])
fine = ("success",) if every else ("success", "neutral")
for j in sorted(jobs, key=lambda j: j.get("name", "")):
    name = j.get("name", "?")
    conclusion = j.get("conclusion") or j.get("status") or "-"
    if conclusion in fine:
        continue
    if conclusion == "skipped" and name in may_skip:
        continue
    if conclusion == "skipped" and name in plan:
        print("path %s" % name)
        continue
    if conclusion == "skipped":
        print("bad %s: skipped, nicht nach der Pfadregel (changes ohne Schritt \"skip %s\")" % (name, name))
    else:
        print("bad %s: %s" % (name, conclusion))
'
}

# Nur Push-Laeufe (oder das Ereignis aus HUMANITL_CI_EVENT): Ein Lauf fuer
# einen Pull Request derselben SHA prueft einen anderen Stand (den Merge mit
# dem Ziel) und darf das Tor nicht oeffnen.
runs_url="$api/repos/$repo/actions/workflows/$workflow/runs?head_sha=$sha&event=$event&per_page=20"
if [[ -n "$branch" ]]; then
  # Ein Zweigname wie release-dry/0.0.7 traegt einen Schraegstrich.
  runs_url+="&branch=$(python3 -I -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "$branch")"
fi
short="${sha:0:12}"
echo "ci-gate: warte auf $workflow fuer $short${branch:+ auf $branch}${event:+ ($event)} in $repo (Frist ${limit}s)"
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
# Fuer einen roten Lauf ist die Jobliste nur Auskunft; scheitert ihre
# Abfrage, bleibt er rot. Fuer einen gruenen entscheidet sie mit (Pfadregel,
# HUMANITL_CI_EVERY_JOB); eine erreichte Ratengrenze (Exit 75) ist dann wie in
# der Schleife oben kein Fehler, sondern ein Grund zu warten, solange die
# Frist reicht.
jobs_ok=0
while :; do
  frc=0
  jobs_body="$(fetch "$api/repos/$repo/actions/runs/$run_id/jobs?per_page=100")" || frc=$?
  if ((frc == 75)) && [[ "$conclusion" == success ]] && (($(date +%s) - start < limit)); then
    [[ "$jobs_body" =~ ^[0-9]+$ ]] || jobs_body=60
    echo "ci-gate: Ratengrenze der API erreicht, warte ${jobs_body}s auf die Jobliste" >&2
    nap "$jobs_body"
    continue
  fi
  break
done
if ((frc == 0)); then
  if report_jobs <<< "$jobs_body"; then
    jobs_ok=1
  else
    echo "ci-gate: Jobliste nicht lesbar" >&2
  fi
else
  echo "ci-gate: Jobs des Laufs $run_id nicht lesbar; Einzelheiten auf $html" >&2
fi

# Fuer einen gruenen Lauf entscheidet auch die Jobliste. Ohne sie gibt es
# kein Urteil: Ein gruener Lauf, dessen Jobs niemand gesehen hat, koennte
# lauter uebersprungene sein.
if [[ "$conclusion" == success ]]; then
  if ((!jobs_ok)); then
    echo "ci-gate: kein Urteil; die Jobliste des Laufs $run_id fehlt, und ohne sie ist kein uebersprungener Job zu pruefen ($html)" >&2
    exit 3
  fi
  jrc=0
  verdict="$(judge_jobs <<< "$jobs_body")" || jrc=$?
  if ((jrc == 4)); then
    echo "ci-gate: rot; der Lauf $run_id hat keine Jobs, nicht mergen ($html)" >&2
    exit 1
  elif ((jrc != 0)); then
    echo "ci-gate: Jobliste nicht auswertbar ($html)" >&2
    exit 2
  fi
  bad="$(sed -n 's/^bad /  /p' <<< "$verdict")"
  skipped="$(sed -n 's/^path //p' <<< "$verdict" | tr '\n' ' ')"
  skipped="${skipped% }"
  if [[ -n "$bad" ]]; then
    echo "ci-gate: rot; der Lauf ist gruen, aber nicht jeder Job:" >&2
    echo "$bad" >&2
    exit 1
  fi
  if [[ -n "$skipped" ]]; then
    echo "ci-gate: nach der Pfadregel uebersprungen: $skipped"
    if [[ "$every_job" == 1 ]]; then
      echo "ci-gate: kein voller Lauf; jeder Job muss gelaufen sein ($html)" >&2
      exit 4
    fi
  fi
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
