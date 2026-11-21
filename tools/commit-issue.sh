#!/usr/bin/env bash
# Bringt ein Issue als einen Commit auf einem eigenen Branch nach main, wie
# CONTRIBUTING.md es verlangt.
#
#   tools/commit-issue.sh --ci HUM-003 proto-v1 "subject line" <pfade...> <<'MSG'
#   body ...
#   MSG
#
#       Der Standardweg seit HUM-237. Committet <pfade> als einen Commit auf
#       die aktuelle Spitze von origin/main in den Branch hum-003-proto-v1,
#       pusht ihn, wartet auf die CI genau dieses Commits (tools/ci-gate.sh)
#       und merged ihn nur bei Gruen mit --no-ff nach main und pusht main.
#
#   tools/commit-issue.sh --pr 42 proto-v1 "subject line" <pfade...> <<'MSG'
#   body ...
#
#   Review: Codex ..., Antigravity ...
#   MSG
#
#       Der Standardweg fuer Issues mit GitHub-Nummer. Baut den Commit genau
#       wie --ci, in den Branch issue-42-proto-v1, pusht ihn und oeffnet mit
#       `gh pr create` einen Pull Request: Titel ist die Betreffzeile, der
#       Koerper kommt aus .github/pull_request_template.md mit `Closes #42`,
#       der Nachricht unter "What was built" und dem Absatz `Review:` unter
#       "Review". Ohne diesen Absatz, oder mit einem eigenen `Closes #M`,
#       bricht das Skript vor dem Bauen ab, ebenso, wenn keine Regel auf main
#       die Pruefung ci-ok auf einem aktuellen Branch verlangt
#       (docs/guides/maintaining.md). Das Tor ist der pull_request-Lauf von
#       ci.yml fuer genau den gepushten Kopf, bei Release-Pfaden dazu der
#       Probelauf. Erst wenn alle Tore dieses Commits gruen sind, folgt
#       `gh pr merge --auto --merge --match-head-commit <sha>`, und GitHub
#       merged. Das Skript wartet, bis das geschehen ist (hoechstens
#       HUMANITL_PR_MERGE_TIMEOUT Sekunden, Vorgabe 1800). Merged GitHub
#       nicht, sagt es das und merged nie selbst daran vorbei. Der
#       Merge-Commit kommt von GitHub.
#
#   tools/commit-issue.sh --land hum-003-proto-v1
#   tools/commit-issue.sh --land issue-42-proto-v1
#
#       Nur die zweite Haelfte: fuer einen Branch, dessen CI rot war und dessen
#       Commit inzwischen ergaenzt ist, oder dessen Tor keine Antwort bekam.
#       Der Branch darf dabei in keinem Arbeitsbaum ausgecheckt sein. Ein
#       Branch issue-N-* nimmt den Weg ueber den Pull Request; ein offener
#       Pull Request des Branches wird weiterverwendet: Titel und Text kommen
#       neu aus dem aktuellen Commit, ein eingeschaltetes Auto-Merge wird vor
#       dem Push abgeschaltet (scheitert das, wird nicht gepusht) und erst
#       nach gruenen Toren fuer den neuen Commit neu verlangt.
#
#   tools/commit-issue.sh HUM-003 proto-v1 "subject line" <pfade...> <<'MSG'
#
#       Der alte lokale Weg ohne CI: Branch, Commit, lokaler --no-ff-Merge nach
#       main. Nichts wird gepusht. Bleibt fuer die Arbeit ohne Netz.
#
# Warum das Ergebnis der CI fuer den Merge-Commit gilt: Gemergt wird nur, wenn
# origin/main noch der Elternteil des getesteten Commits ist. Ein --no-ff-Merge
# eines Branches, dessen Basis die Spitze von main ist, hat genau den Baum des
# Branches; main bekommt also Byte fuer Byte den Stand, den die CI geprueft
# hat. Hat sich main inzwischen bewegt, wird der eine Commit auf das neue main
# gesetzt, erneut gepusht und erneut geprueft, hoechstens
# HUMANITL_LAND_ATTEMPTS (Vorgabe 3) Mal.
#
# Beim Committen und Umsetzen fassen --ci und --land weder Index noch
# Arbeitsbaum noch ausgecheckten Branch des Aufrufers an: Der Commit entsteht
# in einem eigenen Index, das Umsetzen mit `git merge-tree`. Der Arbeitsbaum
# eines Issues enthaelt fast immer mehr als das Issue (Dateien anderer Issues,
# von `git apply --3way` vorgemerkte Stuecke), und nichts davon darf in den
# Commit rutschen oder einem Umsetzen im Weg stehen. Erst ganz am Ende zieht
# das Skript das lokale main nach, und nur, wenn dabei keine lokale Aenderung
# verloren gehen kann.
#
# Probelauf des Releases (HUM-363): Beruehrt der Commit einen Pfad aus
# `tools/release-dry-paths.txt` (die Liste aus dem Commit und seinem
# Elternteil zusammen; fehlt sie in einem, immer), pusht das
# Skript denselben Commit zusaetzlich als `release-dry/hum-NNN`. Das loest den
# Probelauf von `.github/workflows/release.yml` aus, der RPM, AppImage, AUR und
# macOS baut und prueft, was ci.yml nicht abdeckt. Gemergt wird dann erst,
# wenn ci.yml gruen ist und im Probelauf jeder Job gruen ist ausser dem
# uebersprungenen `release`. Nach dem Merge wird der Zweig wieder geloescht;
# bei rotem Probelauf bleibt er wie der Issue-Zweig stehen.
#
# Umgebung: HUMANITL_REMOTE (Vorgabe origin), HUMANITL_CI_GATE (Befehl, der
# eine SHA nimmt und wie tools/ci-gate.sh endet; der Test ersetzt ihn. Fuer
# den Probelauf bekommt er HUMANITL_CI_WORKFLOW=release.yml,
# HUMANITL_CI_BRANCH, HUMANITL_CI_EVERY_JOB=1 und HUMANITL_CI_MAY_SKIP=release
# in der Umgebung), HUMANITL_CI_TIMEOUT (Sekunden, Vorgabe 3600),
# HUMANITL_RELEASE_DRY_TIMEOUT (Sekunden fuer den Probelauf, Vorgabe 7200),
# HUMANITL_LAND_ATTEMPTS, CLAUDE_SESSION_URL (wird als Zeile
# `Claude-Session:` angehaengt). Nur fuer --pr: HUMANITL_GH (Befehl statt
# gh; der Test setzt einen Stub ein), HUMANITL_PR_MERGE_TIMEOUT,
# HUMANITL_PR_POLL (Sekunden zwischen zwei Blicken auf den Pull Request,
# Vorgabe 30).
#
# Mit --pr heisst der Zweig des Release-Probelaufs weiter release-dry/hum-N,
# denn release.yml kennt nur dieses Muster; N ist dann die Issue-Nummer.
# Zeigt ein solcher Zweig schon auf einen fremden Commit (ein altes
# HUM-Issue gleicher Nummer), bricht das Skript ab, statt ihn zu
# ueberschreiben.
set -euo pipefail
cd "$(dirname "$0")/.."

trailer="Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>"
if [[ -n "${CLAUDE_SESSION_URL:-}" ]]; then
  trailer+=$'\n'"Claude-Session: $CLAUDE_SESSION_URL"
fi
remote="${HUMANITL_REMOTE:-origin}"
gate="${HUMANITL_CI_GATE:-tools/ci-gate.sh}"
ci_timeout="${HUMANITL_CI_TIMEOUT:-3600}"
dry_timeout="${HUMANITL_RELEASE_DRY_TIMEOUT:-7200}"
attempts="${HUMANITL_LAND_ATTEMPTS:-3}"
gh="${HUMANITL_GH:-gh}"
merge_timeout="${HUMANITL_PR_MERGE_TIMEOUT:-1800}"
merge_poll="${HUMANITL_PR_POLL:-30}"
# Globale Ergebnisse der Helfer unten.
sha="" main_sha="" dry="" merge=""
tmp_files=()
# Die Form ${a[@]+...} haelt auch bash 3.2 (macOS) unter set -u bei leerer
# Liste aus.
trap 'rm -f ${tmp_files[@]+"${tmp_files[@]}"}' EXIT

die() {
  echo "commit-issue: $*" >&2
  exit 1
}

fetch_main() {
  git fetch -q "$remote" main
  git rev-parse --verify --quiet "refs/remotes/$remote/main^{commit}" \
    || die "$remote/main not found after fetch"
}

# Ist <ref> in irgendeinem Arbeitsbaum dieses Repositorys ausgecheckt? Die
# Liste wird erst ganz gelesen und dann durchsucht: `grep -q` direkt hinter
# der Pipe beendet sich beim ersten Treffer, und unter pipefail liesse das
# SIGPIPE des Schreibers die Abfrage scheitern.
checked_out() {
  local wts
  wts="$(git worktree list --porcelain)"
  grep -qx "branch $1" <<< "$wts"
}

# `git merge-tree --write-tree --merge-base` gibt es ab git 2.40.
require_git_240() {
  local v major minor
  v="$(git version | sed -nE 's/^git version ([0-9]+)\.([0-9]+).*/\1 \2/p')"
  read -r major minor <<< "$v"
  if [[ -z "$major" ]] || ((major < 2 || (major == 2 && minor < 40))); then
    die "git 2.40 or newer is required (found: $(git version))"
  fi
}

# Setzt den einen Commit von <branch> auf <onto>, ohne Arbeitsbaum. Gibt den
# neuen Commit aus; bei einem Konflikt Abbruch, der Branch bleibt, wie er ist.
# Die Ausgabe von merge-tree landet erst ganz in einer Variablen: Hinter einer
# Pipe mit `head` koennte merge-tree an SIGPIPE sterben und unter pipefail
# einen sauberen Merge als Konflikt melden.
rebase_one() {
  local branch="$1" onto="$2" old parent out tree
  old="$(git rev-parse "refs/heads/$branch")"
  parent="$(git rev-parse "$old^")"
  if ! out="$(git merge-tree --write-tree --merge-base="$parent" "$onto" "$old")"; then
    die "$branch does not rebase cleanly onto $remote/main; rebase it by hand, then run --land $branch"
  fi
  tree="${out%%$'\n'*}"
  git rev-parse --verify --quiet "$tree^{tree}" > /dev/null \
    || die "git merge-tree gave no tree for $branch"
  # Autor und Nachricht bleiben die des urspruenglichen Commits.
  GIT_AUTHOR_NAME="$(git log -1 --format=%an "$old")" \
    GIT_AUTHOR_EMAIL="$(git log -1 --format=%ae "$old")" \
    GIT_AUTHOR_DATE="$(git log -1 --format=%aD "$old")" \
    git commit-tree "$tree" -p "$onto" -F <(git log -1 --format=%B "$old")
}

# Eintrag eines Pfads in <commit> als "<modus> <blob> 0", leer, wenn er dort
# fehlt.
entry_in() {
  local listing
  listing="$(git ls-tree "$1" -- "$2")"
  [[ -n "$listing" ]] || return 0
  awk '{ print $1, $3, 0 }' <<< "${listing%%$'\t'*}"
}

# Alle Index-Eintraege eines Pfads, "<modus> <blob> <stufe>" je Zeile, leer,
# wenn er fehlt. Ein Konflikt hat mehrere Stufen und gleicht damit nie einem
# einzelnen Baumeintrag.
entry_in_index() {
  local listing line
  listing="$(git ls-files -s -- "$1")"
  [[ -n "$listing" ]] || return 0
  while IFS= read -r line; do
    echo "${line%%$'\t'*}"
  done <<< "$listing"
}

# Die Datei im Arbeitsbaum in derselben Form, "<modus> <blob> 0": Modus aus
# dem Ausfuehrungsrecht beziehungsweise 120000 fuer einen Symlink, Blob, wie
# git ihn aus der Datei machen wuerde. Leer, wenn der Pfad fehlt; ein
# Verzeichnis an seiner Stelle gleicht keinem Baumeintrag.
entry_in_worktree() {
  local p="$1" mode=100644
  if [[ -L "$p" ]]; then
    # Das Ziel geht unveraendert in die Pipe: Eine Befehlsersetzung dazwischen
    # schnitte Zeilenumbrueche am Ende ab, und ein Ziel `old\n` sahe aus wie
    # `old`.
    printf '120000 %s 0\n' "$(readlink -n -- "$p" | git hash-object --stdin)"
  elif [[ -f "$p" ]]; then
    [[ -x "$p" ]] && mode=100755
    printf '%s %s 0\n' "$mode" "$(git hash-object -- "$p")"
  elif [[ -e "$p" ]]; then
    echo "not-a-file"
  fi
}

# Zieht das lokale main auf <new> nach, einen Nachfahren davon, ohne eine
# lokale Aenderung zu verlieren.
#
# `git merge --ff-only` reicht nicht: Die Dateien des Issues liegen meist als
# unversionierte oder geaenderte Dateien mit genau dem committeten Inhalt im
# Arbeitsbaum, und git ueberschreibt keine unversionierten Dateien. Ein leerer
# `git status` als Bedingung reichte aus demselben Grund nicht: Er waere im
# Normalfall nie leer, und main bliebe immer stehen.
#
# Stattdessen gilt fuer jeden Pfad, der sich zwischen altem und neuem main
# geaendert hat: Index-Eintrag und Datei im Arbeitsbaum muessen, je fuer sich,
# vollstaendig (Modus, Blob, Stufe) dem alten oder dem neuen Stand gleichen.
# Jeder andere Zustand ist eine lokale Aenderung, vorgemerkt oder nicht, auch
# ein reines `chmod +x`, ein Konflikt oder eine fremde unversionierte Datei an
# einem neuen Pfad; dann bleibt main ganz stehen, und das Skript sagt es. Der
# Merge liegt in dem Fall schon auf origin.
#
# Gilt die Bedingung fuer alle Pfade, wird der Index-Eintrag der neue, und eine
# Datei mit dem alten Stand wird durch den neuen ersetzt. Verloren geht dabei
# nichts: Was ersetzt wird, ist genau der alte main-Stand.
#
# Zwei Feinheiten. Die Pfadliste entsteht mit `--no-renames`: Mit der
# voreingestellten Umbenennungserkennung stuende nur der neue Name darin, der
# alte bliebe in Index und Arbeitsbaum, und der naechste Commit braechte die
# geloeschte Datei zurueck. Und jeder Pfad ist woertlich gemeint
# (GIT_LITERAL_PATHSPECS): Ein Name mit `*`, `?` oder `[` traefe sonst als
# Muster auch andere Dateien, und `git reset` setzte deren vorgemerkte
# Aenderungen mit zurueck. Steht an einem Pfad ein Verzeichnis statt einer
# Datei (oder umgekehrt), gleicht keiner der Eintraege dem anderen, und main
# bleibt stehen.
advance_local_main() {
  local new="$1" old path idx wt e_old e_new paths=()
  local -x GIT_LITERAL_PATHSPECS=1
  old="$(git rev-parse --verify --quiet refs/heads/main)" || return 0
  if ! git merge-base --is-ancestor "$old" "$new"; then
    echo "commit-issue: local main has commits that are not on $remote/main; left as it is" >&2
    return 0
  fi
  if [[ "$(git symbolic-ref -q HEAD || true)" != refs/heads/main ]]; then
    # In einem anderen Arbeitsbaum ausgecheckt, liesse das Verschieben dessen
    # Index auf das alte main zeigen: Jede Aenderung des Merges stuende dort
    # als vorgemerkte Ruecknahme.
    if checked_out refs/heads/main; then
      echo "commit-issue: main moved on $remote; sync yourself with \`git merge --ff-only $remote/main\` in the worktree that has main checked out" >&2
    else
      git update-ref refs/heads/main "$new" "$old"
    fi
    return 0
  fi
  while IFS= read -r -d '' path; do
    paths+=("$path")
    e_old="$(entry_in "$old" "$path")"
    e_new="$(entry_in "$new" "$path")"
    idx="$(entry_in_index "$path")"
    wt="$(entry_in_worktree "$path")"
    if [[ "$idx" != "$e_old" && "$idx" != "$e_new" ]] \
      || [[ "$wt" != "$e_old" && "$wt" != "$e_new" ]]; then
      echo "commit-issue: $path has local changes; main moved on $remote; sync yourself with \`git merge --ff-only $remote/main\`" >&2
      return 0
    fi
  done < <(git diff --no-renames --name-only -z "$old" "$new")
  git update-ref refs/heads/main "$new" "$old"
  for path in "${paths[@]}"; do
    git reset -q "$new" -- "$path"
    if [[ "$(entry_in_worktree "$path")" == "$(entry_in "$old" "$path")" ]]; then
      if [[ -n "$(entry_in "$new" "$path")" ]]; then
        git checkout -q "$new" -- "$path"
      else
        rm -f -- "$path"
      fi
    fi
  done
}

# Beruehrt <sha> einen Pfad aus tools/release-dry-paths.txt? Gezaehlt wird die
# Vereinigung der Listen aus dem Elternteil und aus <sha>, dazu die Liste
# selbst: Ein Commit, der release.yml aendert und zugleich die Liste kuerzt
# oder loescht, entgeht dem Probelauf so nicht. Fehlt die Liste in einem der
# beiden Staende, ist der Probelauf Pflicht, statt still zu entfallen. Mit
# --no-renames steht bei einer Umbenennung auch der alte Pfad in der Liste,
# und die Pfade sind woertlich gemeint, ein Verzeichnis meint alles darunter.
release_dry_needed() {
  local sha="$1" rev list line paths=(tools/release-dry-paths.txt)
  local -x GIT_LITERAL_PATHSPECS=1
  for rev in "$sha^" "$sha"; do
    list="$(git show "$rev:tools/release-dry-paths.txt" 2> /dev/null)" || return 0
    while IFS= read -r line; do
      line="${line%$'\r'}"
      [[ -z "$line" || "$line" == \#* ]] && continue
      paths+=("$line")
    done <<< "$list"
  done
  [[ -n "$(git diff --no-renames --name-only "$sha^" "$sha" -- "${paths[@]}")" ]]
}

# Prueft, dass <branch> genau ein Commit auf einem Commit von origin/main ist,
# setzt ihn bei bewegtem main um und pusht ihn. Setzt die globalen Variablen
# sha (der gepushte Commit) und main_sha.
rebase_and_push() {
  local branch="$1"
  main_sha="$(fetch_main)"
  sha="$(git rev-parse "refs/heads/$branch")"
  # Ein Issue, ein Implementierungs-Commit (CLAUDE.md Schritt 5). Das
  # Umsetzen unten bewegt genau einen Commit, mehr wuerden still
  # zusammengedrueckt.
  if [[ "$(git rev-list --parents -n 1 "$sha" | wc -w)" -ne 2 ]] \
    || ! git merge-base --is-ancestor "$sha^" "$main_sha"; then
    die "$branch must be exactly one commit on top of a commit of $remote/main"
  fi
  if [[ "$(git rev-parse "$sha^")" != "$main_sha" ]]; then
    echo "commit-issue: $remote/main moved; rebasing $branch onto ${main_sha:0:12}"
    sha="$(rebase_one "$branch" "$main_sha")"
    git update-ref "refs/heads/$branch" "$sha"
  fi
  echo "commit-issue: pushing $branch (${sha:0:12})"
  git push -q --force-with-lease="refs/heads/$branch" "$remote" "refs/heads/$branch:refs/heads/$branch"
}

# Pusht <sha> als Zweig des Release-Probelaufs <name>, wenn der Commit einen
# Release-Pfad beruehrt, und setzt dry auf den Zweig, sonst leer. Der
# Probelauf laeuft auf GitHub neben ci.yml; gepusht wird er deshalb vor dem
# ersten Warten. `+`: Der Zweig gehoert diesem Issue, und nach einem Umsetzen
# zeigt er auf den neuen Commit.
#
# Mit <frueher> (nur --pr) darf ein schon vorhandener Zweig gleichen Namens
# nur auf <sha> oder auf <frueher> zeigen, den Kopf des Issue-Zweigs vor
# diesem Push: release-dry/hum-N teilt sich die Nummern mit den alten
# HUM-Issues, und ein fremder Probelauf darf nicht still ueberschrieben werden.
push_release_dry() {
  local sha="$1" name="$2" check_prev="${3+x}" prev="${3:-}" listing there
  dry=""
  if release_dry_needed "$sha"; then
    dry="release-dry/$name"
    if [[ -n "$check_prev" ]]; then
      listing="$(git ls-remote "$remote" "refs/heads/$dry")" || die "could not read $dry on $remote"
      there="${listing%%$'\t'*}"
      if [[ -n "$there" && "$there" != "$sha" && "$there" != "$prev" ]]; then
        die "$dry exists on $remote at ${there:0:12}, a commit of another issue; delete it once nothing needs it, then run --land again"
      fi
    fi
    echo "commit-issue: ${sha:0:12} touches release paths (tools/release-dry-paths.txt); pushing $dry for the release dry run"
    git push -q "$remote" "+$sha:refs/heads/$dry"
  fi
}

# Wartet auf ci.yml und, wenn dry gesetzt ist, auf den Probelauf von <sha>.
# Kehrt nur bei Gruen zurueck; <hint> steht in jeder Meldung bei Rot. <event>
# ist das Ereignis des ci.yml-Laufs, der zaehlt: push fuer --ci, pull_request
# fuer --pr. Der Probelauf ist immer ein Push-Lauf.
gates_or_die() {
  local sha="$1" branch="$2" hint="$3" event="${4:-push}" rc=0 on=""
  # Ein PR-Lauf zaehlt nur fuer den Zweig dieses PR: Zwei PRs mit demselben
  # Kopf-Commit duerfen sich das Tor nicht gegenseitig oeffnen.
  [[ "$event" == pull_request ]] && on="$branch"
  HUMANITL_CI_EVENT="$event" HUMANITL_CI_BRANCH="$on" $gate "$sha" "$ci_timeout" || rc=$?
  case "$rc" in
    0) ;;
    1) die "CI is red for ${sha:0:12}; fix, amend the commit on $branch, switch back, then run --land $branch${dry:+ ($dry stays on $remote; --land pushes it again)}${hint:+; $hint}" ;;
    3) die "CI gave no verdict for ${sha:0:12}; run --land $branch again once it does${hint:+; $hint}" ;;
    *) die "tools/ci-gate.sh failed with $rc for ${sha:0:12} (API or network?); run --land $branch again${hint:+; $hint}" ;;
  esac
  [[ -n "$dry" ]] || return 0
  rc=0
  HUMANITL_CI_WORKFLOW=release.yml HUMANITL_CI_BRANCH="$dry" HUMANITL_CI_EVERY_JOB=1 \
    HUMANITL_CI_MAY_SKIP=release HUMANITL_CI_EVENT=push $gate "$sha" "$dry_timeout" || rc=$?
  case "$rc" in
    0) ;;
    1) die "the release dry run on $dry is red for ${sha:0:12}; fix, amend the commit on $branch, switch back, then run --land $branch${hint:+; $hint}" ;;
    3) die "the release dry run on $dry gave no verdict for ${sha:0:12}; run --land $branch again once it does${hint:+; $hint}" ;;
    *) die "tools/ci-gate.sh failed with $rc for the release dry run of ${sha:0:12} (API or network?); run --land $branch again${hint:+; $hint}" ;;
  esac
}

# Der direkte Weg (--ci): Merge-Commit selbst bauen und nach main pushen.
land_direct() {
  local branch="$1" issue="$2" slug="$3" merge attempt
  for ((attempt = 1; attempt <= attempts; attempt++)); do
    rebase_and_push "$branch"
    push_release_dry "$sha" "${branch%-"$slug"}"
    gates_or_die "$sha" "$branch" ""

    # Gemergt wird nur, solange main der Elternteil des Getesteten ist.
    main_sha="$(fetch_main)"
    if [[ "$(git rev-parse "$sha^")" != "$main_sha" ]]; then
      echo "commit-issue: $remote/main moved while CI ran; testing again on the new main"
      continue
    fi
    merge="$(git commit-tree "$sha^{tree}" -p "$main_sha" -p "$sha" -m "merge: $issue $slug")"
    # Ein schlichter Push: Der Server nimmt nur einen Fast-Forward an, ein
    # Merge, der einem anderen in die Quere kam, scheitert hier, und die
    # Schleife prueft erneut.
    if ! git push -q "$remote" "$merge:refs/heads/main"; then
      echo "commit-issue: the push to $remote/main was refused; testing again on the current main"
      continue
    fi
    finish "$branch" "$merge"
    return 0
  done
  die "$remote/main kept moving; $branch is pushed, run --land $branch again"
}

# Loescht <ref> auf origin. Nach einem Pull Request hat GitHub den Zweig oft
# schon selbst geloescht; ein Fehler zaehlt deshalb nur, wenn der Zweig danach
# noch da ist, und dann steht die Meldung von git dabei.
delete_remote_branch() {
  local ref="$1" err
  if ! err="$(git push -q "$remote" --delete "$ref" 2>&1)"; then
    if git ls-remote --exit-code --heads "$remote" "refs/heads/$ref" > /dev/null 2>&1; then
      echo "commit-issue: could not delete $remote/$ref: $err" >&2
    fi
  fi
}

# Aufraeumen nach dem Merge: Zweige auf origin loeschen, lokales main
# nachziehen, den lokalen Zweig entfernen.
finish() {
  local branch="$1" merge="$2" ref
  for ref in "$branch" ${dry:+"$dry"}; do
    delete_remote_branch "$ref"
  done
  git fetch -q "$remote" main
  advance_local_main "$merge"
  git update-ref -d "refs/heads/$branch"
  git --no-pager log --oneline -1 "$merge"
}

# Nummer des offenen Pull Requests von <branch>, leer, wenn es keinen gibt.
open_pr() {
  local out
  out="$($gh pr list --head "$1" --base main --state open --json number --jq '.[0].number // empty')" \
    || die "gh pr list failed; is gh logged in (gh auth status)?"
  [[ -z "$out" || "$out" =~ ^[0-9]+$ ]] || die "gh pr list gave '$out', not a pull request number"
  echo "$out"
}

# Steht auf stdin ein Schluesselwort, mit dem GitHub ein Issue schliesst
# (close, fix, resolve in jeder Beugung, vor `#N`)? GitHub erkennt es an jeder
# Stelle einer Zeile und ohne Ruecksicht auf die Schreibung.
names_closing_keyword() {
  grep -qiE '(^|[^[:alnum:]_])(close[sd]?|fix(e[sd])?|resolve[sd]?):?[[:space:]]+#[0-9]'
}

# Koerper des Pull Requests aus der Vorlage im Commit und der Nachricht des
# Commits: `Closes #N` in der ersten Zeile, die Absaetze der Nachricht unter
# "What was built", der Absatz, der mit `Review:` beginnt, unter "Review".
# Die Zeilen Co-Authored-By und Claude-Session bleiben im Commit.
pr_body() {
  local sha="$1" number="$2" message template built review out rc=0
  message="$(git log -1 --format=%b "$sha" | grep -vE '^(Co-Authored-By|Claude-Session): ' || true)"
  if names_closing_keyword <<< "$message"; then
    die "the commit message of ${sha:0:12} closes an issue itself; one issue per pull request, and --pr adds 'Closes #$number'"
  fi
  review="$(awk 'BEGIN { RS = "" } /^Review:/ { print; print "" }' <<< "$message")"
  built="$(awk 'BEGIN { RS = "" } !/^Review:/ { print; print "" }' <<< "$message")"
  [[ -n "$review" ]] || die "the commit message of ${sha:0:12} has no paragraph starting with 'Review:'; the review summary belongs in the pull request"
  template="$(git show "$sha:.github/pull_request_template.md" 2> /dev/null || true)"
  [[ -n "$template" ]] || template=$'Closes #\n\n## What was built\n\n## Review\n'
  out="$(PR_NUMBER="$number" PR_BUILT="$built" PR_REVIEW="$review" awk '
    /^Closes #[0-9]*[[:space:]]*$/ && !closes { print "Closes #" ENVIRON["PR_NUMBER"]; closes = 1; next }
    { print }
    /^## What was built[[:space:]]*$/ { print ""; print ENVIRON["PR_BUILT"] }
    /^## Review[[:space:]]*$/ { print ""; print ENVIRON["PR_REVIEW"] }
    END { if (!closes) exit 3 }
  ' <<< "$template")" || rc=$?
  if ((rc == 3)); then
    # Eine Vorlage ohne Zeile `Closes #` darf das Schliessen nicht verlieren.
    printf 'Closes #%s\n\n%s\n' "$number" "$out"
  elif ((rc == 0)); then
    printf '%s\n' "$out"
  else
    die "could not render the pull request body"
  fi
}

# Auto-Merge ohne Pflichtpruefung merged sofort. Vor dem ersten --auto muss
# deshalb feststehen, dass auf main eine Regel gilt, die ci-ok auf einem
# aktuellen Zweig verlangt (docs/guides/maintaining.md). Gefragt wird die
# API nach den Regeln, die auf dem Zweig main tatsaechlich gelten.
require_ruleset() {
  local ok
  ok="$($gh api 'repos/{owner}/{repo}/rules/branches/main' --jq \
    'any(.[]; .type == "required_status_checks"
      and .parameters.strict_required_status_checks_policy == true
      and any(.parameters.required_status_checks[]?; .context == "ci-ok"))')" \
    || die "could not read the rules of main with gh api; is gh logged in (gh auth status)?"
  [[ "$ok" == true ]] \
    || die "no rule on main requires the check ci-ok on an up-to-date branch; auto-merge would merge at once. Set up the ruleset as in docs/guides/maintaining.md first"
}

# Schaltet ein Auto-Merge von #<pr> ab, bevor ein neuer Commit auf den Zweig
# geht. Scheitert das, bricht das Skript vor dem Push ab: Sonst koennte
# GitHub einen Kopf mergen, fuer den noch kein Tor gruen war.
disable_auto_merge() {
  local pr="$1" on
  on="$($gh pr view "$pr" --json autoMergeRequest --jq '.autoMergeRequest != null')" \
    || die "could not read the auto-merge state of #$pr; nothing pushed"
  [[ "$on" == true ]] || return 0
  echo "commit-issue: turning off the auto-merge of #$pr before the push"
  $gh pr merge "$pr" --disable-auto > /dev/null \
    || die "could not turn off the auto-merge of #$pr; nothing pushed. Turn it off on GitHub, then run --land again"
}

# Auto-Merge fuer genau den getesteten Commit, erst wenn alle seine Tore gruen
# sind. --match-head-commit laesst GitHub ablehnen, wenn der Kopf des Pull
# Requests inzwischen ein anderer ist; ohne ihn gaelte das Auto-Merge auch
# fuer einen spaeter gepushten Commit.
enable_auto_merge() {
  local pr="$1" sha="$2"
  echo "commit-issue: auto-merge for #$pr at ${sha:0:12}"
  $gh pr merge "$pr" --auto --merge --match-head-commit "$sha" \
    || die "could not enable auto-merge for #$pr; check 'Allow auto-merge' and the ruleset on main (docs/guides/maintaining.md); never merge around it"
}

# Wartet, bis GitHub den Pull Request gemergt hat, und setzt merge auf den
# Merge-Commit. Rueckgabe 10, wenn main sich bewegt hat, ohne dass <sha>
# darauf liegt: Der Zweig ist dann nicht mehr aktuell, und die Regel auf main
# verlangt einen neuen Lauf auf dem umgesetzten Commit.
wait_merged() {
  local pr="$1" sha="$2" branch="$3" deadline info state oid
  deadline=$((SECONDS + merge_timeout))
  while :; do
    info="$($gh pr view "$pr" --json state,mergeCommit --jq '.state + " " + (.mergeCommit.oid // "")')" \
      || die "gh pr view #$pr failed; run --land $branch again"
    read -r state oid <<< "$info"
    case "$state" in
      MERGED)
        [[ "$oid" =~ ^[0-9a-f]{40}$ ]] || die "#$pr is merged, but gh gave no merge commit ('$oid')"
        merge="$oid"
        return 0
        ;;
      CLOSED) die "#$pr was closed without a merge; $branch stays on $remote" ;;
      OPEN) ;;
      *) die "gh pr view #$pr gave state '$state'" ;;
    esac
    main_sha="$(fetch_main)"
    if [[ "$(git rev-parse "$sha^")" != "$main_sha" ]] && ! git merge-base --is-ancestor "$sha" "$main_sha"; then
      return 10
    fi
    if ((SECONDS >= deadline)); then
      die "CI is green for ${sha:0:12}, but auto-merge did not merge #$pr within ${merge_timeout}s; look at the pull request on GitHub (required check ci-ok, ruleset, 'Allow auto-merge'), never merge around it, then run --land $branch again"
    fi
    sleep "$merge_poll"
  done
}

# Der Weg ueber einen Pull Request (--pr). Das Tor ist der pull_request-Lauf
# von ci.yml fuer genau den gepushten Kopf (head_sha), dazu bei Release-Pfaden
# der Probelauf. Erst wenn alle Tore dieses Commits gruen sind, wird
# Auto-Merge verlangt, mit --match-head-commit; vor jedem neuen Push wird es
# abgeschaltet. GitHub merged dann, weil die Regel auf main ci-ok auf einem
# aktuellen Zweig verlangt, und das Skript wartet auf diesen Merge.
land_pr() {
  local branch="$1" number="$2" pr body_file attempt rc listing prev created
  # Der Koerper muss sich bauen lassen, bevor irgendetwas gepusht wird.
  pr_body "$(git rev-parse "refs/heads/$branch")" "$number" > /dev/null
  require_ruleset
  pr="$(open_pr "$branch")"
  for ((attempt = 1; attempt <= attempts; attempt++)); do
    if [[ -n "$pr" ]]; then
      disable_auto_merge "$pr"
    fi
    listing="$(git ls-remote "$remote" "refs/heads/$branch")" || die "could not read $branch on $remote"
    prev="${listing%%$'\t'*}"
    rebase_and_push "$branch"
    body_file="$(mktemp "${TMPDIR:-/tmp}/commit-issue-pr.XXXXXX")"
    tmp_files+=("$body_file")
    pr_body "$sha" "$number" > "$body_file"
    created=0
    if [[ -z "$pr" ]]; then
      echo "commit-issue: opening the pull request for $branch"
      $gh pr create --base main --head "$branch" --title "$(git log -1 --format=%s "$sha")" --body-file "$body_file" > /dev/null \
        || die "gh pr create failed; $branch is pushed, run --land $branch again"
      pr="$(open_pr "$branch")"
      [[ -n "$pr" ]] || die "gh pr create reported success, but no open pull request for $branch"
      created=1
    fi
    echo "commit-issue: pull request #$pr"
    if ((!created)); then
      # Ein wieder aufgenommener Pull Request beschreibt den aktuellen
      # Commit, nicht den, mit dem er geoeffnet wurde.
      $gh pr edit "$pr" --title "$(git log -1 --format=%s "$sha")" --body-file "$body_file" > /dev/null \
        || die "gh pr edit #$pr failed; run --land $branch again"
    fi
    # Der Probelauf behaelt den Namen release-dry/hum-N, denn release.yml
    # kennt nur dieses Muster; N ist hier die Nummer des GitHub-Issues.
    push_release_dry "$sha" "hum-$number" "$prev"
    gates_or_die "$sha" "$branch" "#$pr stays open without auto-merge" pull_request
    # Erst jetzt, da jedes Tor dieses Commits gruen ist.
    enable_auto_merge "$pr" "$sha"

    rc=0
    wait_merged "$pr" "$sha" "$branch" || rc=$?
    if ((rc == 10)); then
      echo "commit-issue: $remote/main moved before #$pr was merged; testing again on the new main"
      continue
    fi
    echo "commit-issue: #$pr merged as ${merge:0:12}"
    # Die Regel auf main verlangt einen aktuellen Zweig; der Merge-Commit hat
    # dann genau den Baum des getesteten Commits.
    git fetch -q "$remote" main
    if [[ "$(git rev-parse --verify --quiet "$merge^{tree}" || true)" != "$(git rev-parse "$sha^{tree}")" ]]; then
      echo "commit-issue: warning: the tree of ${merge:0:12} differs from the tested ${sha:0:12}" >&2
    fi
    finish "$branch" "$merge"
    return 0
  done
  die "$remote/main kept moving; #$pr is open with $branch pushed, run --land $branch again"
}

land() {
  local branch="$1"
  require_git_240
  git rev-parse --verify --quiet "refs/heads/$branch" > /dev/null || die "no local branch $branch"
  # Das Skript verschiebt und loescht die Referenz des Branches. Ist er
  # irgendwo ausgecheckt, haengte danach dort HEAD in der Luft oder stuende
  # eine Ruecknahme im Index.
  if checked_out "refs/heads/$branch"; then
    die "$branch is checked out in a worktree; switch that worktree to another branch, then run --land $branch"
  fi
  dry=""
  if [[ "$branch" =~ ^(hum-[0-9]+)-([a-z0-9][a-z0-9-]*)$ ]]; then
    land_direct "$branch" "$(tr '[:lower:]' '[:upper:]' <<< "${BASH_REMATCH[1]}")" "${BASH_REMATCH[2]}"
  elif [[ "$branch" =~ ^issue-([1-9][0-9]*)-[a-z0-9][a-z0-9-]*$ ]]; then
    land_pr "$branch" "${BASH_REMATCH[1]}"
  else
    die "branch '$branch' is neither hum-NNN-slug nor issue-N-slug"
  fi
}

# Baut den Commit der <pfade> in einem eigenen Index auf origin/main als
# einzigen Commit von <branch> und bringt ihn nach main.
commit_ci() {
  local branch="$1" label="$2" subject="$3" body="$4" main_sha tmp tree commit
  shift 4
  require_git_240
  git rev-parse --verify --quiet "refs/heads/$branch" > /dev/null && die "branch $branch exists already"
  main_sha="$(fetch_main)"
  # Der Arbeitsbaum fusst auf HEAD. Unterscheidet sich einer der Pfade des
  # Issues zwischen HEAD und dem aktuellen main, drehte ein Commit des
  # Arbeitsbaums auf main zurueck, was main inzwischen hat (CLAUDE.md, Regeln
  # fuer Subagenten: zusammenfuehren, nie kopieren).
  if ! git diff --quiet HEAD "$main_sha" -- "$@"; then
    git --no-pager diff --stat HEAD "$main_sha" -- "$@" >&2
    die "$remote/main changed these paths since HEAD; merge main into your checkout first"
  fi
  tmp="$(mktemp "${TMPDIR:-/tmp}/commit-issue-index.XXXXXX")"
  tmp_files+=("$tmp")
  GIT_INDEX_FILE="$tmp" git read-tree "$main_sha"
  GIT_INDEX_FILE="$tmp" git add -A -- "$@"
  tree="$(GIT_INDEX_FILE="$tmp" git write-tree)"
  [[ "$tree" != "$(git rev-parse "$main_sha^{tree}")" ]] || die "nothing to commit for $label"
  commit="$(git commit-tree "$tree" -p "$main_sha" -m "$subject" -m "$body" -m "$trailer")"
  git update-ref "refs/heads/$branch" "$commit" ""
  echo "commit-issue: $branch at ${commit:0:12} on ${main_sha:0:12}"
  land "$branch"
}

commit_local() {
  local issue="$1" slug="$2" subject="$3" body="$4" branch
  shift 4
  branch="$(tr '[:upper:]' '[:lower:]' <<< "$issue")-$slug"
  git checkout -q -b "$branch"
  git add -- "$@"
  if git diff --cached --quiet; then
    echo "nothing staged for $issue" >&2
    git checkout -q main
    git branch -q -D "$branch"
    exit 1
  fi
  git commit -q -m "$subject" -m "$body" -m "$trailer"
  git checkout -q main
  git merge -q --no-ff "$branch" -m "merge: $issue $slug"
  git branch -q -d "$branch"
  git --no-pager log --oneline -1
}

mode=local
case "${1:-}" in
  --ci) mode=ci; shift ;;
  --pr) mode=pr; shift ;;
  --land)
    [[ $# -eq 2 ]] || die "usage: tools/commit-issue.sh --land hum-NNN-slug|issue-N-slug"
    land "$2"
    exit 0
    ;;
esac

[[ $# -ge 3 ]] || die "usage: tools/commit-issue.sh [--ci HUM-NNN | --pr N] slug \"subject\" paths... <<< body"
issue="$1"
slug="$2"
subject="$3"
shift 3
[[ $# -gt 0 ]] || die "no paths given for $issue"

case "$mode" in
  pr)
    # Alles pruefen, bevor irgendetwas gebaut oder gepusht wird.
    [[ "$issue" =~ ^[1-9][0-9]{0,6}$ ]] || die "--pr takes the number of the GitHub issue, got '$issue'"
    [[ "$slug" =~ ^[a-z0-9][a-z0-9-]*$ ]] || die "slug '$slug' must be lower case letters, digits and dashes"
    body="$(cat)"
    awk 'BEGIN { RS = "" } /^Review:/ { found = 1 } END { exit !found }' <<< "$body" \
      || die "the commit body needs a paragraph starting with 'Review:'; the review summary goes into the pull request"
    if names_closing_keyword <<< "$body"; then
      die "the commit body closes an issue itself; one issue per pull request, and --pr adds 'Closes #$issue'"
    fi
    commit_ci "issue-$issue-$slug" "#$issue" "$subject" "$body" "$@"
    ;;
  ci)
    body="$(cat)"
    commit_ci "$(tr '[:upper:]' '[:lower:]' <<< "$issue")-$slug" "$issue" "$subject" "$body" "$@"
    ;;
  *)
    body="$(cat)"
    commit_local "$issue" "$slug" "$subject" "$body" "$@"
    ;;
esac
