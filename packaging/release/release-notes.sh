#!/usr/bin/env bash
# Schreibt die Release-Notes einer 0.0.x-Vorabversion nach stdout.
#
# Aufruf: packaging/release/release-notes.sh <0.0.N> <git-ref>
#
# Die Ausgabe ist Englisch (HUM-225): Die Releases lesen Menschen, die kein
# Deutsch sprechen. Nur diese Kommentare bleiben deutsch.
#
# Die Meilensteine kommen aus der Tabelle "Project status and roadmap" in
# README.md, nicht aus diesem Skript: Die Zustaende ("delivered", "in
# progress", "planned") stehen hier so, wie sie dort stehen, und eine
# Aenderung der Tabelle aendert die naechsten Notes mit. Die Commit-Liste ist
# die erste-Eltern-Linie von `main` seit dem vorigen v0.0.*-Tag, also je Issue
# die eine Merge-Zeile.
#
# Braucht die ganze Historie samt Tags (`fetch-depth: 0`).
set -euo pipefail

[[ $# -eq 2 ]] || { echo "usage: $0 <0.0.N> <git-ref>" >&2; exit 2; }
version="$1"
ref="$2"
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
# shellcheck source=packaging/release/version.sh
source "$here/version.sh"
require_prerelease_version "$version"

commit="$(git -C "$root" rev-parse --verify "$ref^{commit}")"
short="$(git -C "$root" rev-parse --short=12 "$commit")"

# Der vorige Tag dieses Kanals, vom Elternteil aus gesucht, damit ein Tag, der
# genau auf diesem Commit sitzt, nicht sich selbst findet.
previous="$(git -C "$root" describe --tags --abbrev=0 --match 'v0.0.*' "$commit^" 2>/dev/null || true)"

milestones="$(python3 - "$root/README.md" <<'PY'
import re
import sys

text = open(sys.argv[1], encoding="utf-8").read()
start = text.find("## Project status and roadmap")
if start < 0:
    sys.exit("error: README.md has no section 'Project status and roadmap'")
rows = []
for line in text[start:].splitlines()[1:]:
    if line.startswith("## "):
        break
    m = re.match(r"^\|\s*(M\d+[^|]*?)\s*\|\s*([^|]*?)\s*\|\s*([^|]*?)\s*\|\s*$", line)
    if m:
        rows.append(m.groups())
if not rows:
    sys.exit("error: no milestone rows in the roadmap table of README.md")
for name, what, status in rows:
    mark = "x" if status == "delivered" else " "
    print(f"- [{mark}] **{name}** ({status}): {what}")
PY
)"

cat <<EOF
> **Pre-release before the MVP. Not for productive use.**
> Humanitl $version is an interim state on the way to release 0.1.0. The
> artefacts are **not signed**; \`SHA256SUMS\` protects against a damaged
> download, not against a tampered release. Signed packages, a changelog and
> an AppImage come with 0.1.0 (HUM-060).

Built from commit \`$short\` on \`main\`, after its CI run was green.

## Install (amd64)

Built and checked in a container on Ubuntu 24.04. Newer systems with the same
libraries (Debian 13, for example) should install it as well; that is not
checked.

\`\`\`sh
sha256sum -c SHA256SUMS --ignore-missing
sudo apt install ./humanitl_${version}_amd64.deb
humanitl daemon install       # activates the socket of the daemon, for you only
humanitl daemon status
humanitl sandbox check        # three green lines?
humanitl-app
\`\`\`

A fresh Ubuntu 24.04 blocks unprivileged user namespaces through AppArmor;
the sandbox then reports \`SANDBOX_003\`, and \`humanitl doctor\` names the fix.

The package sets nothing up by itself; \`humanitl daemon install\` only
activates the units it ships. To remove: \`humanitl daemon uninstall\`, then
\`sudo apt purge humanitl\`. The archive
\`humanitl-${version}-linux-x86_64.tar.gz\` runs without installation;
\`INSTALL.txt\` inside it explains how.

## Milestone status (from README.md)

$milestones

## What is still missing

- No AppImage in the release assets, no signature, no man pages.
- The systemd user unit \`humanitld.service\` is hardened as far as the sandbox
  allows, and the package ships \`humanitld.socket\` for socket activation. Some
  protections are left out on purpose because the sandbox needs them (see
  \`docs/INSTALL.md\`, section on hardening); the unit is not as locked down as
  a service without a sandbox could be.
- What the release run tests: build, version number of the programs,
  installation and removal without leftovers of the .deb in an Ubuntu 24.04
  container, a start of the application under Xvfb, lintian. A desktop with a
  real screen is not tested.

## Commits since ${previous:-the start of the project}

EOF

if [[ -n "$previous" ]]; then
  range="$previous..$commit"
else
  range="$commit"
fi
count="$(git -C "$root" rev-list --count --first-parent "$range")"
limit=300
# shellcheck disable=SC2016 # Die Backticks sind Markdown, keine Ersetzung.
git -C "$root" log --first-parent --no-decorate --format='- `%h` %s' -n "$limit" "$range"
if [[ "$count" -gt "$limit" ]]; then
  echo "- ... and $((count - limit)) more"
fi
