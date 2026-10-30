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
done < <(awk '/^## Status$/ { on = 1; next } /^## / { on = 0 } on' "$root/README.md")
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
> **Early release on the way to 0.1.0.**
> Humanitl @VERSION@ is an interim state before release 0.1.0. The artefacts
> are **not signed**; `SHA256SUMS` protects against a damaged download,
> not against a tampered release. A changelog comes with 0.1.0 (HUM-060);
> signed packages come with 1.0.0.

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

The AppImage `Humanitl-@VERSION@-x86_64.AppImage` needs no installation
either: make it executable and start it; `--cli daemon install` sets up the
service from a copy under `~/.local/lib/humanitl/`, and every later start of
a newer image refreshes that copy.

```sh
chmod +x Humanitl-@VERSION@-x86_64.AppImage
./Humanitl-@VERSION@-x86_64.AppImage --cli daemon install
./Humanitl-@VERSION@-x86_64.AppImage
```

## Install on Fedora and openSUSE (x86_64)

`humanitl-@VERSION@-1.x86_64.rpm` holds the same programs and files as the
.deb. It needs glibc 2.39 or newer, so Fedora 40 and newer or openSUSE
Tumbleweed; RHEL 9 and openSUSE Leap 15 are too old for it. The release run
installs it in a Fedora 44 container; Tumbleweed is not checked.

```sh
sha256sum -c SHA256SUMS --ignore-missing
sudo dnf install ./humanitl-@VERSION@-1.x86_64.rpm
# openSUSE: sudo zypper install --allow-unsigned-rpm ./humanitl-@VERSION@-1.x86_64.rpm
humanitl daemon install
humanitl-app
```

To remove: `humanitl daemon uninstall`, then `sudo dnf remove humanitl`
(openSUSE: `sudo zypper remove humanitl`).

## Install on Arch Linux (AUR)

The AUR package `humanitl-bin` installs the archive
`humanitl-@VERSION@-linux-x86_64.tar.gz` of this release and checks its
SHA-256. The release run builds it with makepkg and installs it in an Arch
Linux container, but the maintainer publishes it to the AUR by hand after the
release: until https://aur.archlinux.org/packages/humanitl-bin shows
@VERSION@, it is not there yet.

```sh
git clone https://aur.archlinux.org/humanitl-bin.git
cd humanitl-bin
makepkg -si
humanitl daemon install
```

To remove: `humanitl daemon uninstall`, then `sudo pacman -Rns humanitl-bin`.

## Install on macOS (Apple Silicon and Intel)

Open `Humanitl-@VERSION@-macos.dmg` and drag Humanitl into Applications.
The app is signed ad hoc, not with an Apple Developer ID, so macOS refuses the
first start: allow it once under System Settings › Privacy & Security ›
Open Anyway, or in a terminal:

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing
xattr -dr com.apple.quarantine /Applications/Humanitl.app
```

On the setup screen, "Set up the service" installs the daemon as a launchd
agent. The command line is inside the app:
`/Applications/Humanitl.app/Contents/MacOS/bin/humanitl`. The sandbox on
macOS is Seatbelt; what it guarantees, and how that differs from Linux, is in
`docs/SECURITY.md`. The release run checks the disk image on a fresh macOS
runner: signature, both architectures, the version, a start of the app, and
the Seatbelt escape tests against the binaries inside it.

## Milestone status (from README.md)

@MILESTONES@

## What is still missing

- No signature, no man pages.
- The systemd user unit `humanitld.service` is hardened as far as the sandbox
  allows, and the package ships `humanitld.socket` for socket activation. Some
  protections are left out on purpose because the sandbox needs them (see
  `docs/INSTALL.md`, section on hardening); the unit is not as locked down as
  a service without a sandbox could be.
- What the release run tests: build, version number of the programs,
  installation and removal without leftovers of the .deb in an Ubuntu 24.04
  container, of the .rpm in a Fedora 44 container and of `humanitl-bin`, built
  with makepkg, in an Arch Linux container, each time a start of the
  application under Xvfb, lintian and rpmlint. A desktop with a real screen is
  not tested.
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
grep -qF -- '--title "Humanitl $VERSION" \' "$workflow" ||
  fail "the release title in release.yml is not 'Humanitl \$VERSION'"
# Seit HUM-273 erscheinen 0.0.x als regulaere Releases: Weder das Anlegen
# noch das Veroeffentlichen darf sie als Vorabversion markieren.
if grep -v '^[[:space:]]*#' "$workflow" | grep -qE -- '--prerelease([^=]|=true|$)'; then
  fail "release.yml still marks the release as a pre-release"
fi
grep -v '^[[:space:]]*#' "$workflow" | grep -qE -- 'gh release edit .*--draft=false --prerelease=false --latest( |$)' ||
  fail "release.yml does not publish the release as latest"
if grep -q -- '--title .*Vorabversion' "$workflow"; then
  fail "the release title in release.yml is German"
fi

# Eine Version ausserhalb des Kanals bricht weiterhin ab.
if "$notes_script" 0.1.0 HEAD >/dev/null 2>&1; then
  fail "0.1.0 is accepted by the 0.0.x channel"
fi

if [[ "$failures" -gt 0 ]]; then
  echo "release-notes-test: $failures failure(s)" >&2
  exit 1
fi
echo "release-notes-test: ok"
