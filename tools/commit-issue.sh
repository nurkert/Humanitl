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
#   tools/commit-issue.sh --land hum-003-proto-v1
#
#       Nur die zweite Haelfte: fuer einen Branch, dessen CI rot war und dessen
#       Commit inzwischen ergaenzt ist, oder dessen Tor keine Antwort bekam.
#       Der Branch darf dabei in keinem Arbeitsbaum ausgecheckt sein.
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
# `Claude-Session:` angehaengt).
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

land() {
  local branch="$1" issue slug sha main_sha merge rc attempt dry
  [[ "$branch" =~ ^(hum-[0-9]+)-([a-z0-9][a-z0-9-]*)$ ]] \
    || die "branch '$branch' is not of the form hum-NNN-slug"
  issue="$(tr '[:lower:]' '[:upper:]' <<< "${BASH_REMATCH[1]}")"
  slug="${BASH_REMATCH[2]}"
  require_git_240
  git rev-parse --verify --quiet "refs/heads/$branch" > /dev/null || die "no local branch $branch"
  # Das Skript verschiebt und loescht die Referenz des Branches. Ist er
  # irgendwo ausgecheckt, haengte danach dort HEAD in der Luft oder stuende
  # eine Ruecknahme im Index.
  if checked_out "refs/heads/$branch"; then
    die "$branch is checked out in a worktree; switch that worktree to another branch, then run --land $branch"
  fi

  for ((attempt = 1; attempt <= attempts; attempt++)); do
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

    # Der Probelauf laeuft auf GitHub neben ci.yml; gepusht wird er deshalb
    # vor dem ersten Warten. `+`: Der Zweig gehoert diesem Issue, und nach
    # einem Umsetzen zeigt er auf den neuen Commit.
    dry=""
    if release_dry_needed "$sha"; then
      dry="release-dry/${branch%-"$slug"}"
      echo "commit-issue: ${sha:0:12} touches release paths (tools/release-dry-paths.txt); pushing $dry for the release dry run"
      git push -q "$remote" "+$sha:refs/heads/$dry"
    fi

    rc=0
    $gate "$sha" "$ci_timeout" || rc=$?
    case "$rc" in
      0) ;;
      1) die "CI is red for ${sha:0:12}; fix, amend the commit on $branch, switch back, then run --land $branch${dry:+ ($dry stays on $remote; --land pushes it again)}" ;;
      3) die "CI gave no verdict for ${sha:0:12}; run --land $branch again once it does" ;;
      *) die "tools/ci-gate.sh failed with $rc for ${sha:0:12} (API or network?); run --land $branch again" ;;
    esac

    if [[ -n "$dry" ]]; then
      rc=0
      HUMANITL_CI_WORKFLOW=release.yml HUMANITL_CI_BRANCH="$dry" HUMANITL_CI_EVERY_JOB=1 \
        HUMANITL_CI_MAY_SKIP=release $gate "$sha" "$dry_timeout" || rc=$?
      case "$rc" in
        0) ;;
        1) die "the release dry run on $dry is red for ${sha:0:12}; fix, amend the commit on $branch, switch back, then run --land $branch" ;;
        3) die "the release dry run on $dry gave no verdict for ${sha:0:12}; run --land $branch again once it does" ;;
        *) die "tools/ci-gate.sh failed with $rc for the release dry run of ${sha:0:12} (API or network?); run --land $branch again" ;;
      esac
    fi

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
    git push -q "$remote" --delete "$branch" || echo "commit-issue: could not delete $remote/$branch" >&2
    if [[ -n "$dry" ]]; then
      git push -q "$remote" --delete "$dry" || echo "commit-issue: could not delete $remote/$dry" >&2
    fi
    advance_local_main "$merge"
    git update-ref -d "refs/heads/$branch"
    git fetch -q "$remote" main
    git --no-pager log --oneline -1 "$merge"
    return 0
  done
  die "$remote/main kept moving; $branch is pushed, run --land $branch again"
}

# Baut den Commit der <pfade> in einem eigenen Index auf origin/main.
commit_ci() {
  local issue="$1" slug="$2" subject="$3" body="$4" branch main_sha tmp tree commit
  shift 4
  branch="$(tr '[:upper:]' '[:lower:]' <<< "$issue")-$slug"
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
  tmp_index="$(mktemp "${TMPDIR:-/tmp}/commit-issue-index.XXXXXX")"
  trap 'rm -f "$tmp_index"' EXIT
  tmp="$tmp_index"
  GIT_INDEX_FILE="$tmp" git read-tree "$main_sha"
  GIT_INDEX_FILE="$tmp" git add -A -- "$@"
  tree="$(GIT_INDEX_FILE="$tmp" git write-tree)"
  [[ "$tree" != "$(git rev-parse "$main_sha^{tree}")" ]] || die "nothing to commit for $issue"
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
  --land)
    [[ $# -eq 2 ]] || die "usage: tools/commit-issue.sh --land hum-NNN-slug"
    land "$2"
    exit 0
    ;;
esac

issue="${1:?issue id, e.g. HUM-003}"
slug="${2:?branch slug, e.g. proto-v1}"
subject="${3:?commit subject}"
shift 3
[[ $# -gt 0 ]] || die "no paths given for $issue"
body="$(cat)"

if [[ "$mode" == ci ]]; then
  commit_ci "$issue" "$slug" "$subject" "$body" "$@"
else
  commit_local "$issue" "$slug" "$subject" "$body" "$@"
fi
