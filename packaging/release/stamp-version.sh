#!/usr/bin/env bash
# Setzt die Versionsnummer einer 0.0.x-Vorabversion in den Rust-Workspace.
#
# Aufruf: packaging/release/stamp-version.sh 0.0.3
#
# Das Skript schreibt in `daemon/Cargo.toml` (Abschnitt `[workspace.package]`),
# in `daemon/Cargo.lock` (die Pakete des Workspace, also die ohne
# `source`-Zeile) und in `app/pubspec.yaml`. Alles nur im Auscheckstand eines Release-Laufs: Im
# Repository bleibt `0.0.0` stehen, bis HUM-060 mit `VERSION` die eine Quelle
# aller Versionsstellen einfuehrt. Deshalb verweigert das Skript die Arbeit
# ausserhalb von CI, es sei denn, der Aufruf sagt mit `--allow-local`
# ausdruecklich, dass er in einer Wegwerfkopie laeuft.
#
# Die Lock-Datei wird mitgezogen, damit `cargo build --locked` weiter gilt:
# Eine Lock-Datei, die nicht zur Cargo.toml passt, liesse `--locked` scheitern,
# und ohne `--locked` koennte der Release andere Abhaengigkeiten enthalten als
# der gepruefte Stand (HUM-060, Fallstricke). Schreibt das Skript hier etwas
# falsch, faellt es also im naechsten Schritt auf und nicht erst beim Nutzer.
set -euo pipefail

usage() {
  echo "usage: $0 [--allow-local] <0.0.N>" >&2
  exit 2
}

allow_local=0
if [[ "${1:-}" == "--allow-local" ]]; then
  allow_local=1
  shift
fi
[[ $# -eq 1 ]] || usage
version="$1"

# shellcheck source=packaging/release/version.sh
source "$(dirname "$0")/version.sh"
require_prerelease_version "$version"

if [[ "${CI:-}" != "true" && "$allow_local" -ne 1 ]]; then
  echo "error: refusing to edit daemon/Cargo.toml outside CI; pass --allow-local in a throwaway copy" >&2
  exit 1
fi

root="$(cd "$(dirname "$0")/../.." && pwd)"
VERSION="$version" ROOT="$root" python3 - <<'PY'
import os
import re
import sys

version = os.environ["VERSION"]
root = os.environ["ROOT"]

manifest = os.path.join(root, "daemon", "Cargo.toml")
with open(manifest, encoding="utf-8") as fh:
    lines = fh.read().split("\n")

section = None
hits = 0
for i, line in enumerate(lines):
    header = re.match(r"^\s*\[([^\]]+)\]\s*$", line)
    if header:
        section = header.group(1).strip()
        continue
    if section == "workspace.package" and re.match(r'^version\s*=\s*"[^"]*"\s*$', line):
        lines[i] = f'version = "{version}"'
        hits += 1
if hits != 1:
    sys.exit(f"error: expected one version line in [workspace.package] of {manifest}, found {hits}")
with open(manifest, "w", encoding="utf-8") as fh:
    fh.write("\n".join(lines))

lock = os.path.join(root, "daemon", "Cargo.lock")
with open(lock, encoding="utf-8") as fh:
    text = fh.read()
blocks = text.split("\n[[package]]\n")
stamped = []
for n, block in enumerate(blocks):
    if n == 0 or re.search(r"^source = ", block, re.M):
        continue
    name = re.search(r'^name = "([^"]+)"$', block, re.M)
    new, count = re.subn(r'^version = "[^"]*"$', f'version = "{version}"', block, count=1, flags=re.M)
    if count != 1 or not name:
        sys.exit(f"error: workspace package block without name or version in {lock}")
    blocks[n] = new
    stamped.append(name.group(1))
if not stamped:
    sys.exit(f"error: no workspace packages found in {lock}")
with open(lock, "w", encoding="utf-8") as fh:
    fh.write("\n[[package]]\n".join(blocks))

print(f"stamped {version} into [workspace.package] and {len(stamped)} lock entries: {', '.join(sorted(stamped))}")

# app/pubspec.yaml: `flutter build --build-name` setzt die Version des Baus
# auch ohne diese Zeile, `flutter test` aber nimmt FLUTTER_BUILD_NAME nur aus
# pubspec.yaml und laesst sich die Variable nicht per --dart-define geben.
# Damit der Test des Exports im Release-Lauf dieselbe Version sieht wie die
# App, steht sie auch hier.
pubspec = os.path.join(root, "app", "pubspec.yaml")
with open(pubspec, encoding="utf-8") as fh:
    text = fh.read()
text, count = re.subn(r"^version: .*$", f"version: {version}", text, count=1, flags=re.M)
if count != 1:
    sys.exit(f"error: no top-level version line in {pubspec}")
with open(pubspec, "w", encoding="utf-8") as fh:
    fh.write(text)
print(f"stamped {version} into app/pubspec.yaml")
PY
