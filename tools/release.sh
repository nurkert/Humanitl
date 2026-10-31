#!/usr/bin/env bash
# Schneidet eine Vorabversion 0.0.N so, dass ihr Tag nie scheitert (HUM-363).
#
# Aufruf: tools/release.sh 0.0.N [commit]
#
# Ein Tag v0.0.N wird nie verschoben (CONTRIBUTING.md). Scheitert der Lauf von
# `.github/workflows/release.yml` nach dem Tag, bleibt die Nummer ohne Release,
# und die Liste der Releases auf GitHub hat eine Luecke; so ging es v0.0.14.
# Deshalb setzt dieses Skript den Tag erst, wenn derselbe Commit den ganzen
# Release-Bau schon einmal bestanden hat:
#
#   1. N ist genau die naechste Nummer: eins mehr als der hoechste
#      v0.0.*-Tag auf origin, keine Luecke, keine Wiederholung.
#   2. Der Commit ist die Spitze von origin/main (wer einen Commit nennt,
#      muss genau diese Spitze nennen), und sein ci.yml-Lauf auf main ist
#      gruen (tools/ci-gate.sh).
#   3. Der Commit wird als Zweig `release-dry/0.0.N` gepusht. Das loest den
#      Probelauf von release.yml aus: alle Bau- und Pruefjobs mit genau
#      dieser Version, ohne den Job `release`. Das Tor wartet, bis jeder Job
#      gruen ist (`release` darf und muss uebersprungen sein).
#   4. Erst dann entsteht der annotierte Tag `v0.0.N` ("Humanitl 0.0.N") auf
#      demselben Commit und wird gepusht; der Probelauf-Zweig wird geloescht.
#   5. Das Skript wartet auf den Lauf des Tags und nennt die Adresse des
#      Releases.
#
# Ist der Probelauf rot, endet das Skript mit dem roten Job und Schritt, und
# es gibt keinen Tag. Der Zweig bleibt stehen: War es ein Ausreisser, laesst
# sich der Probelauf in der Actions-Oberflaeche neu starten, und ein erneuter
# Aufruf mit derselben Nummer wertet den neuen Versuch aus (der Push desselben
# Commits ist dann ein No-op). War es ein Fehler, kommt die Behebung auf main,
# und der erneute Aufruf prueft den neuen Commit.
#
# Scheitert der Tag-Lauf trotz gruenem Probelauf (ein Ausreisser), sagt das
# Skript es und verweist auf genau diesen Lauf: dort in der Actions-Oberflaeche
# "Re-run failed jobs". Der Tag bleibt, wo er ist; er wird nie verschoben.
#
# Exit-Codes: 0 Release veroeffentlicht; 1 abgelehnt oder rot; 2 Aufruf
# falsch; 3 kein Urteil binnen der Frist (dann erneut aufrufen oder den Lauf
# beobachten).
#
# Umgebung, vor allem fuer den Test (`tools/tests/release_test.sh`):
#   HUMANITL_REMOTE          Vorgabe origin
#   HUMANITL_CI_GATE         Befehl wie tools/ci-gate.sh (<sha> <frist>); er
#                            bekommt HUMANITL_CI_WORKFLOW, HUMANITL_CI_BRANCH,
#                            HUMANITL_CI_EVERY_JOB und HUMANITL_CI_MAY_SKIP
#                            in der Umgebung
#   HUMANITL_CI_TIMEOUT      Frist fuer ci.yml, Vorgabe 3600 s
#   HUMANITL_RELEASE_TIMEOUT Frist je Lauf von release.yml, Vorgabe 7200 s
#   HUMANITL_CI_REPO         owner/name fuer die Adresse des Releases; sonst
#                            aus `git remote get-url`
set -euo pipefail
cd "$(dirname "$0")/.."

remote="${HUMANITL_REMOTE:-origin}"
gate="${HUMANITL_CI_GATE:-tools/ci-gate.sh}"
ci_timeout="${HUMANITL_CI_TIMEOUT:-3600}"
release_timeout="${HUMANITL_RELEASE_TIMEOUT:-7200}"

die() {
  echo "release: $*" >&2
  exit 1
}

# Kein Urteil: Exit 3, damit ein Aufrufer Warten von Ablehnen unterscheidet.
undecided() {
  echo "release: $*" >&2
  exit 3
}

[[ $# -ge 1 && $# -le 2 ]] || { echo "usage: tools/release.sh 0.0.N [commit]" >&2; exit 2; }
version="$1"
# Dasselbe enge Muster wie in release.yml und packaging/release/version.sh;
# die Ziffern stehen einzeln, damit keine Ziffer einer anderen Schrift passt.
if [[ ! "$version" =~ ^0\.0\.(0|[123456789][0123456789]{0,5})$ ]]; then
  echo "release: '$version' is not a 0.0.N version (0.1.0 is HUM-060)" >&2
  exit 2
fi
n="${BASH_REMATCH[1]}"
tag="v$version"
dry="release-dry/$version"

repo="${HUMANITL_CI_REPO:-}"
if [[ -z "$repo" ]]; then
  url="$(git remote get-url "$remote" 2> /dev/null || true)"
  repo="$(sed -nE 's#^(git@|ssh://git@|https://)github\.com[:/]([^/]+/[^/]+)$#\2#p' <<< "$url")"
  repo="${repo%.git}"
fi

# Der hoechste v0.0.*-Tag auf dem Server, als Zahl; -1, wenn es keinen gibt.
# Gefragt wird origin, nicht die lokalen Tags: Ein lokal fehlender oder lokal
# zusaetzlicher Tag soll die Nummer nicht verschieben.
latest_remote_n() {
  local refs
  refs="$(git ls-remote --tags --refs "$remote" 'refs/tags/v0.0.*')" || die "cannot list the tags of $remote"
  python3 -I -c '
import re, sys
best = -1
for line in sys.stdin.read().splitlines():
    m = re.fullmatch(r"[0-9a-f]+\trefs/tags/v0\.0\.(0|[1-9][0-9]{0,5})", line)
    if m:
        best = max(best, int(m.group(1)))
print(best)
' <<< "$refs"
}

# Die Nummer muss genau die naechste sein.
check_next() {
  local latest
  latest="$(latest_remote_n)"
  if ((n <= latest)); then
    die "$tag is not new: $remote already has v0.0.$latest; the next version is 0.0.$((latest + 1))"
  fi
  if ((n != latest + 1)); then
    if ((latest < 0)); then
      die "$remote has no v0.0.* tag yet; the first version is 0.0.0, not $version"
    fi
    die "$version leaves a gap after v0.0.$latest; the next version is 0.0.$((latest + 1))"
  fi
}

check_next

git fetch -q "$remote" main
sha="$(git rev-parse --verify --quiet "refs/remotes/$remote/main^{commit}")" || die "$remote/main not found after fetch"
if [[ $# -eq 2 ]]; then
  wanted="$(git rev-parse --verify --quiet "$2^{commit}")" || die "$2 is not a commit"
  if [[ "$wanted" != "$sha" ]]; then
    die "$2 (${wanted:0:12}) is not the tip of $remote/main (${sha:0:12}); only the tip of main is released"
  fi
fi
# Ein lokaler Tag desselben Namens wird nur hingenommen, wenn er genau der
# ist, den dieses Skript setzen wuerde (ein abgebrochener frueherer Aufruf):
# annotiert, "Humanitl 0.0.N", auf diesem Commit. Auf origin gibt es ihn nach
# check_next nicht.
if git rev-parse --verify --quiet "refs/tags/$tag" > /dev/null; then
  if [[ "$(git for-each-ref --format='%(objecttype) %(contents:subject)' "refs/tags/$tag")" != "tag Humanitl $version" ]] \
    || [[ "$(git rev-parse "refs/tags/$tag^{commit}")" != "$sha" ]]; then
    die "a local tag $tag exists that this script did not set on ${sha:0:12}; delete it with \`git tag -d $tag\` (it is not on $remote)"
  fi
fi
echo "release: $version on ${sha:0:12}, the tip of $remote/main"

# run_gate <frist> [VAR=wert ...]: ruft das Tor mit der Umgebung
# auf und gibt seinen Exit-Code zurueck, ohne unter set -e abzubrechen.
gate_rc=0
run_gate() {
  local frist="$1"
  shift
  gate_rc=0
  env "$@" $gate "$sha" "$frist" || gate_rc=$?
}

run_gate "$ci_timeout" HUMANITL_CI_WORKFLOW=ci.yml HUMANITL_CI_BRANCH=main HUMANITL_CI_EVERY_JOB=0 HUMANITL_CI_MAY_SKIP=
case "$gate_rc" in
  0) ;;
  1) die "ci.yml is red for ${sha:0:12} on main; fix main first, nothing was pushed" ;;
  3) undecided "ci.yml gave no verdict for ${sha:0:12} yet; run tools/release.sh $version again once it does" ;;
  *) die "tools/ci-gate.sh failed with $gate_rc for ${sha:0:12} (API or network?); nothing was pushed" ;;
esac

# `+`: Der Zweig gehoert diesem Skript. Ein Probelauf eines aelteren Commits
# mit derselben Nummer wird ersetzt; derselbe Commit ist ein No-op und laesst
# einen in der Oberflaeche neu gestarteten Probelauf gelten.
echo "release: pushing $dry (${sha:0:12}) for the release dry run"
git push -q "$remote" "+$sha:refs/heads/$dry"

run_gate "$release_timeout" HUMANITL_CI_WORKFLOW=release.yml HUMANITL_CI_BRANCH="$dry" HUMANITL_CI_EVERY_JOB=1 HUMANITL_CI_MAY_SKIP=release
case "$gate_rc" in
  0) ;;
  1) die "the release dry run on $dry is red; no tag was set. Fix it on main and run tools/release.sh $version again, or, for a flake, re-run the failed jobs of that dry run in the Actions UI and run tools/release.sh $version again" ;;
  3) undecided "the release dry run on $dry gave no verdict; no tag was set. Run tools/release.sh $version again once it finished" ;;
  *) die "tools/ci-gate.sh failed with $gate_rc for the dry run (API or network?); no tag was set" ;;
esac

# Zwischen der ersten Pruefung und hier liegt der ganze Probelauf; hat in der
# Zeit jemand anderes eine Nummer vergeben, entstuende sonst eine Luecke oder
# eine Dublette. Der Push des Tags scheitert ohnehin, wenn es ihn schon gibt.
check_next
# Getaggt wird der Commit, der den Probelauf bestanden hat, auch wenn main
# inzwischen weiter ist: Genau er ist vollstaendig geprueft und liegt auf main,
# eine Luecke entsteht dadurch nicht. Nur ein umgeschriebenes main (der Commit
# ist dort nicht mehr enthalten) verbietet den Tag.
git fetch -q "$remote" main
if ! git merge-base --is-ancestor "$sha" "refs/remotes/$remote/main"; then
  die "${sha:0:12} is no longer on $remote/main (main was rewritten during the dry run); no tag was set"
fi
if [[ "$(git rev-parse "refs/remotes/$remote/main")" != "$sha" ]]; then
  echo "release: $remote/main moved on during the dry run; $tag goes on the verified commit ${sha:0:12}, not on the newest tip"
fi
if ! git rev-parse --verify --quiet "refs/tags/$tag" > /dev/null; then
  git tag -a "$tag" -m "Humanitl $version" "$sha"
fi
echo "release: pushing the tag $tag (${sha:0:12})"
git push -q "$remote" "refs/tags/$tag:refs/tags/$tag" || die "the push of $tag was refused; nothing changed on $remote"
git push -q "$remote" --delete "$dry" || echo "release: could not delete $remote/$dry; delete it by hand" >&2

run_gate "$release_timeout" HUMANITL_CI_WORKFLOW=release.yml HUMANITL_CI_BRANCH="$tag" HUMANITL_CI_EVERY_JOB=1 HUMANITL_CI_MAY_SKIP=
case "$gate_rc" in
  0)
    if [[ -n "$repo" ]]; then
      echo "release: published https://github.com/$repo/releases/tag/$tag"
    else
      echo "release: published $tag"
    fi
    ;;
  1)
    echo "release: the release run of $tag failed although its dry run was green (a flake?)." >&2
    echo "release: open that run in the Actions UI (link above) and choose \"Re-run failed jobs\"." >&2
    echo "release: the tag $tag stays where it is; never move or re-push it." >&2
    exit 1
    ;;
  3)
    echo "release: the release run of $tag gave no verdict yet; watch it in the Actions UI. The tag stays." >&2
    exit 3
    ;;
  *)
    echo "release: tools/ci-gate.sh failed with $gate_rc for the run of $tag (API or network?); the tag is pushed, watch the run in the Actions UI." >&2
    exit 3
    ;;
esac
