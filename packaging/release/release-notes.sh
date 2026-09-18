#!/usr/bin/env bash
# Schreibt die Release-Notes einer 0.0.x-Vorabversion nach stdout.
#
# Aufruf: packaging/release/release-notes.sh <0.0.N> <git-ref>
#
# Die Meilensteine kommen aus der Tabelle "Project status and roadmap" in
# README.md, nicht aus diesem Skript: Was dort als "delivered" steht, steht
# hier als geliefert, und eine Aenderung der Tabelle aendert die naechsten
# Notes mit. Die Commit-Liste ist die erste-Eltern-Linie von `main` seit dem
# vorigen v0.0.*-Tag, also je Issue die eine Merge-Zeile.
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
words = {"delivered": "geliefert", "in progress": "in Arbeit", "planned": "geplant"}
for name, what, status in rows:
    mark = "x" if status == "delivered" else " "
    print(f"- [{mark}] **{name}** ({words.get(status, status)}): {what}")
PY
)"

cat <<EOF
> **Vorabversion vor dem MVP. Nicht fuer den produktiven Einsatz.**
> Humanitl $version ist ein Zwischenstand vor Release 0.1.0. Die Artefakte sind
> **nicht signiert**; \`SHA256SUMS\` schuetzt vor einem beschaedigten Download,
> nicht vor einem manipulierten Release. Signierte Pakete, Changelog und
> AppImage kommen mit 0.1.0 (HUM-060).

Gebaut aus Commit \`$short\` auf \`main\`, nachdem dessen CI-Lauf gruen war.

## Installieren (amd64)

Gebaut und im Container geprueft auf Ubuntu 24.04. Neuere Systeme mit
denselben Bibliotheken (etwa Debian 13) sollten es ebenfalls installieren;
geprueft ist das nicht.

\`\`\`sh
sha256sum -c SHA256SUMS --ignore-missing
sudo apt install ./humanitl_${version}_amd64.deb
systemctl --user enable --now humanitld.service   # startet den Daemon, nur fuer dich
humanitl sandbox check                             # drei gruene Zeilen?
humanitl-app
\`\`\`

Ein frisches Ubuntu 24.04 sperrt unprivilegierte User-Namespaces ueber
AppArmor; dann meldet die Sandbox \`SANDBOX_003\`, und \`humanitl doctor\` nennt
die Abhilfe.

Das Paket richtet keinen Dienst von selbst ein. Entfernen:
\`systemctl --user disable --now humanitld.service\`, dann \`sudo apt purge humanitl\`.
Das Archiv \`humanitl-${version}-linux-x86_64.tar.gz\` laeuft ohne Installation;
\`INSTALL.txt\` darin beschreibt, wie.

## Stand der Meilensteine (aus README.md)

$milestones

## Was noch fehlt

- Kein AppImage, keine Signatur, keine Man-Pages.
- Die systemd-Nutzer-Unit \`humanitld.service\` ist weder socket-aktiviert noch
  gehaertet: Die Schutzzeilen aus HUM-053 (\`ProtectSystem=strict\`,
  \`ProtectKernel*\`, \`LockPersonality\`, \`MemoryDenyWriteExecute\`,
  \`SystemCallArchitectures=native\` und weitere) fehlen noch. Beides kommt
  mit HUM-053, sobald es gegen die Sandbox getestet ist.
- Getestet ist im Release-Lauf: Bau, Versionsnummer der Programme, Installation
  und rueckstandsfreies Entfernen des .deb in einem Ubuntu-24.04-Container,
  ein Start der Anwendung unter Xvfb, lintian. Nicht getestet ist ein Desktop
  mit echtem Bildschirm.

## Commits seit ${previous:-dem Beginn des Projekts}

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
  echo "- ... und $((count - limit)) weitere"
fi
