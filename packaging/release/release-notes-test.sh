#!/usr/bin/env bash
# Test fuer release-notes.sh (HUM-225): Die Notes jeder Vorabversion sind
# Englisch, mit den Zustaenden der Meilensteine so, wie sie in README.md stehen.
#
# Aufruf: packaging/release/release-notes-test.sh
#
# Das Skript laeuft gegen den Stand, in dem dieser Test liegt (HEAD und
# README.md dort); es braucht kein Netz, aber die ganze Historie samt Tags
# (CI: `fetch-depth: 0`); in einem flachen Klon bricht es sofort ab.
#
# Der Kern ist ein Vergleich mit dem vollstaendigen erwarteten Text. Jeder neue
# oder geaenderte Satz, auch ein deutscher, laesst ihn scheitern; wer die Notes
# bewusst aendert, aendert den erwarteten Text hier mit. Ersetzt wird vorher nur,
# was bei jedem Lauf anders ist: die Version und der Commit. Drei Teile werden
# eigens geprueft, jeder ohne Ausnahme fuer Prosa:
#
# - die Meilensteine: jede Zeile im Wortlaut gegen die Tabelle in README.md,
#   Beschreibung und Anzahl der Zeilen eingeschlossen,
# - die Ueberschrift der Commit-Liste: bis auf den vorigen Tag im Wortlaut,
# - die Commit-Liste selbst: nur Commit-Zeilen, keine andere Prosa.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
notes_script="$here/release-notes.sh"

failures=0
fail() {
  echo "FAIL: $*" >&2
  failures=$((failures + 1))
}

# Ohne die ganze Historie samt Tags prueft der Test nichts Echtes: Ein flacher
# Klon kennt einen Commit, ein Klon ohne Tags den "Beginn des Projekts", und
# beides stimmte mit dem Skript ueberein, ohne dass jemand es gemessen haette.
# Deshalb steht diese Pruefung ganz vorn und bricht mit einer Anweisung ab,
# statt gruen zu werden; nichts anderes laeuft gegen einen solchen Klon
# (CI: `fetch-depth: 0` am Checkout, siehe ci.yml, Job deps-lint).
if [[ "$(git -C "$root" rev-parse --is-shallow-repository)" == "true" ]]; then
  echo "FAIL: shallow clone: this test compares the commit list with git log and needs the full history (git fetch --unshallow --tags; CI: fetch-depth: 0)" >&2
  exit 1
fi
if [[ -z "$(git -C "$root" tag --list 'v0.0.*')" ]]; then
  echo "FAIL: no v0.0.* tag in this clone: the test needs the tags of the pre-releases (git fetch --tags; CI: fetch-depth: 0)" >&2
  exit 1
fi

notes="$("$notes_script" 0.0.3 HEAD)"

# Die Meilensteine, so wie sie aus der Tabelle in README.md folgen muessen:
# `- [x] **<Name>** (<Zustand>): <Beschreibung>`, ein Haken nur bei "delivered".
expected_milestones=""
milestone_rows=0
while IFS='|' read -r _ name what status _; do
  name="$(sed -E 's/^ +| +$//g' <<<"$name")"
  what="$(sed -E 's/^ +| +$//g' <<<"$what")"
  status="$(sed -E 's/^ +| +$//g' <<<"$status")"
  [[ "$name" =~ ^M[0-9] ]] || continue
  # Jeder Zustand ist eines der drei Woerter der Tabelle. Welche davon
  # vorkommen, ist Sache der README: Sind einmal alle Meilensteine geliefert,
  # bleibt dieser Test gruen.
  case "$status" in
    delivered | "in progress" | planned) ;;
    *) fail "unknown milestone state in README.md: '$status'" ;;
  esac
  mark=" "
  [[ "$status" == "delivered" ]] && mark="x"
  expected_milestones+="- [$mark] **$name** ($status): $what"$'\n'
  milestone_rows=$((milestone_rows + 1))
done < <(awk '/^## Project status and roadmap/ { on = 1; next } /^## / { on = 0 } on' "$root/README.md")
expected_milestones="${expected_milestones%$'\n'}"
[[ "$milestone_rows" -gt 0 ]] || fail "no milestone rows found in README.md"

# Die Notes in drei Teile: alles vor der Commit-Ueberschrift, die Ueberschrift,
# die Zeilen danach.
heading_line="$(grep -n '^## Commits since ' <<<"$notes" | head -n 1 | cut -d: -f1 || true)"
if [[ -z "$heading_line" ]]; then
  fail "missing heading: ## Commits since ..."
  heading_line=1
fi
prose="$(head -n "$((heading_line - 1))" <<<"$notes")"
heading="$(sed -n "${heading_line}p" <<<"$notes")"
commit_lines="$(tail -n +"$((heading_line + 1))" <<<"$notes")"

# Die Meilenstein-Zeilen stehen zwischen ihrer Ueberschrift und der naechsten.
milestones="$(
  awk '/^## Milestone status/ { on = 1; next } /^## / { on = 0 } on && NF' <<<"$prose"
)"
if [[ "$milestones" != "$expected_milestones" ]]; then
  fail "the milestone rows differ from the README table (diff: README, notes)"
  diff <(echo "$expected_milestones") <(echo "$milestones") >&2 || true
fi

# Die Prosa ohne die Teile, die sich von Lauf zu Lauf aendern.
normalized="$(
  sed -E 's/0\.0\.3/@VERSION@/g; s/`[0-9a-f]{12}`/`@SHA@`/' <<<"$prose" |
    awk '
      /^## Milestone status/ { print; print ""; print "@MILESTONES@"; print ""; skip = 1; next }
      /^## / { skip = 0 }
      !skip { print }
    '
)"

expected="$(
  cat <<'EOF'
> **Pre-release before the MVP. Not for productive use.**
> Humanitl @VERSION@ is an interim state on the way to release 0.1.0. The
> artefacts are **not signed**; `SHA256SUMS` protects against a damaged
> download, not against a tampered release. Signed packages, a changelog and
> an AppImage come with 0.1.0 (HUM-060).

Built from commit `@SHA@` on `main`, after its CI run was green.

## Install (amd64)

Built and checked in a container on Ubuntu 24.04. Newer systems with the same
libraries (Debian 13, for example) should install it as well; that is not
checked.

```sh
sha256sum -c SHA256SUMS --ignore-missing
sudo apt install ./humanitl_@VERSION@_amd64.deb
humanitl daemon install       # activates the socket of the daemon, for you only
humanitl daemon status
humanitl sandbox check        # three green lines?
humanitl-app
```

A fresh Ubuntu 24.04 blocks unprivileged user namespaces through AppArmor;
the sandbox then reports `SANDBOX_003`, and `humanitl doctor` names the fix.

The package sets nothing up by itself; `humanitl daemon install` only
activates the units it ships. To remove: `humanitl daemon uninstall`, then
`sudo apt purge humanitl`. The archive
`humanitl-@VERSION@-linux-x86_64.tar.gz` runs without installation;
`INSTALL.txt` inside it explains how.

## Milestone status (from README.md)

@MILESTONES@

## What is still missing

- No AppImage in the release assets, no signature, no man pages.
- The systemd user unit `humanitld.service` is hardened as far as the sandbox
  allows, and the package ships `humanitld.socket` for socket activation. Some
  protections are left out on purpose because the sandbox needs them (see
  `docs/INSTALL.md`, section on hardening); the unit is not as locked down as
  a service without a sandbox could be.
- What the release run tests: build, version number of the programs,
  installation and removal without leftovers of the .deb in an Ubuntu 24.04
  container, a start of the application under Xvfb, lintian. A desktop with a
  real screen is not tested.
EOF
)"

if [[ "$normalized" != "$expected" ]]; then
  fail "the notes differ from the expected English text (diff: expected, actual)"
  diff <(echo "$expected") <(echo "$normalized") >&2 || true
fi

# Die Ueberschrift der Commit-Liste: der vorige Tag ist das Einzige, was wechselt.
heading_re='^## Commits since (v0\.0\.[0-9]+|the start of the project)$'
if ! [[ "$heading" =~ $heading_re ]]; then
  fail "the commits heading is not '## Commits since <tag>': $heading"
fi

# Die Commit-Liste: eine Leerzeile, dann genau das, was `git log` fuer den Bereich
# seit dem vorigen Tag liefert, und nichts sonst. Der Bereich kommt aus der
# Ueberschrift (oben geprueft); dass der Tag der naechste davor ist, prueft
# `git describe` hier einzeln nach. Die Obergrenze der Liste sind 300 Zeilen,
# danach steht eine Zeile "... and N more".
previous="${heading#\#\# Commits since }"
if [[ "$previous" == "the start of the project" ]]; then
  range="HEAD"
  git -C "$root" describe --tags --abbrev=0 --match 'v0.0.*' 'HEAD^' >/dev/null 2>&1 &&
    fail "the heading says no tag precedes HEAD, but one does"
else
  range="$previous..HEAD"
  [[ "$(git -C "$root" describe --tags --abbrev=0 --match 'v0.0.*' 'HEAD^' 2>/dev/null || true)" == "$previous" ]] ||
    fail "the heading names '$previous', which is not the tag before HEAD"
fi
expected_commits="$(git -C "$root" log --first-parent --no-decorate --format='- `%h` %s' -n 300 "$range")"
commit_count="$(git -C "$root" rev-list --count --first-parent "$range")"
if [[ "$commit_count" -gt 300 ]]; then
  expected_commits+=$'\n'"- ... and $((commit_count - 300)) more"
fi
[[ "$commit_count" -gt 0 ]] || fail "no commits in the range '$range'"
expected_section=$'\n'"$expected_commits"
if [[ "$commit_lines" != "$expected_section" ]]; then
  fail "the commit list differs from git log $range (diff: git log, notes)"
  diff <(echo "$expected_section") <(echo "$commit_lines") >&2 || true
fi

# Der Titel des Releases steht im Workflow und ist das Erste, was ein Leser
# sieht: ebenfalls Englisch.
workflow="$root/.github/workflows/release.yml"
grep -qF -- '--title "Humanitl $VERSION (pre-release)"' "$workflow" ||
  fail "the release title in release.yml is not the English '(pre-release)'"
if grep -q -- '--title .*Vorabversion' "$workflow"; then
  fail "the release title in release.yml is German"
fi

# Eine Version ausserhalb des Kanals bricht weiterhin ab.
if "$notes_script" 0.1.0 HEAD >/dev/null 2>&1; then
  fail "0.1.0 is accepted by the pre-release channel"
fi

if [[ "$failures" -gt 0 ]]; then
  echo "release-notes-test: $failures failure(s)" >&2
  exit 1
fi
echo "release-notes-test: ok"
