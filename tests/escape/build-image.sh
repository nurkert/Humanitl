#!/usr/bin/env sh
# Baut das Testabbild der Escape-Suite und nennt es über seinen Digest
# (HUM-455).
#
#   tests/escape/build-image.sh [NAME]
#
# NAME ist der Name im lokalen Speicher von Podman, Vorgabe
# localhost/humanitl-escape. Er muss mit localhost/ beginnen: Das Abbild wird
# hier gebaut und nie aus einer Registry gezogen, und ein Name, der eine echte
# Registry nennt, stünde für ein Abbild, das es dort nicht gibt.
#
# Die letzte Zeile auf stdout ist der Verweis NAME@sha256:<hex>, so wie
# HUMANITL_ESCAPE_IMAGE und `escape-launch --image` ihn nehmen; alles andere
# geht nach stderr. Lokal den Lauf der CI nachstellen:
#
#   ref=$(./tests/escape/build-image.sh) &&
#       HUMANITL_ESCAPE_BACKEND=podman HUMANITL_ESCAPE_IMAGE="$ref" ./tests/escape/run.sh
#
# Gezogen wird nur hier, beim Bau, die per Digest festgelegte Basis aus
# tests/escape/image/Containerfile und die Pakete darüber. Das geschieht vor
# dem Start des Daemons und außerhalb jeder Sandbox; der Lauf mit dem Abbild
# zieht nichts.
#
# Vor der Ausgabe prüft das Skript, was der Adapter vor jedem Start prüft
# (verdict in daemon/crates/sandbox/src/podman/image.rs): `podman image
# inspect NAME@sha256:<hex>` findet das Abbild, und sein `Digest` oder einer
# seiner `RepoDigests` ist genau dieser Digest; seine Umgebung setzt keine
# Variable mit HUMANITL_. Ein lokal gebautes Abbild, das Podman unter dem
# Verweis nicht fände, endete sonst erst im Lauf als `sandbox_not_started`.
#
# Exit 0 mit dem Verweis, 1 wenn Bau oder Prüfung scheitern, 2 bei falschem
# Aufruf.
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
CONTEXT="$HERE/image"
NAME="${1:-localhost/humanitl-escape}"
TAG="$NAME:latest"

if [ $# -gt 1 ]; then
    echo "usage: tests/escape/build-image.sh [NAME]" >&2
    exit 2
fi
case "$NAME" in
localhost/*[!a-z0-9._/-]* | localhost/)
    echo "build-image: $NAME is not a plain image name below localhost/" >&2
    exit 2
    ;;
localhost/*) ;;
*)
    echo "build-image: $NAME must start with localhost/; this image is built here and never pulled" >&2
    exit 2
    ;;
esac
if ! command -v podman > /dev/null 2>&1; then
    echo "build-image: no podman on this machine" >&2
    exit 1
fi

# Der Kontext ist nur das Verzeichnis des Containerfile: nichts aus dem
# Arbeitsbaum gelangt in das Abbild.
echo "build-image: building $TAG from $CONTEXT/Containerfile" >&2
podman build --pull=missing --file "$CONTEXT/Containerfile" --tag "$TAG" "$CONTEXT" >&2

digest=$(podman image inspect --format '{{.Digest}}' "$TAG")
case "$digest" in
sha256:*) ;;
*)
    echo "build-image: podman names no sha256 digest for $TAG (got '$digest')" >&2
    exit 1
    ;;
esac
hex=${digest#sha256:}
if [ "${#hex}" -ne 64 ] || [ -n "$(printf '%s' "$hex" | tr -d '0-9a-f')" ]; then
    echo "build-image: the digest of $TAG is not 64 hex digits: $digest" >&2
    exit 1
fi
reference="$NAME@$digest"

# Dieselbe Frage wie der Adapter, mit dem Verweis samt Digest.
if ! found=$(podman image inspect --format '{{.Digest}}{{range .RepoDigests}} {{.}}{{end}}' "$reference" 2>&1); then
    echo "build-image: podman does not find $reference: $found" >&2
    exit 1
fi
matched=0
for entry in $found; do
    case "$entry" in
    "$digest" | *"@$digest") matched=1 ;;
    esac
done
if [ "$matched" != 1 ]; then
    echo "build-image: $reference resolves to an image without that digest: $found" >&2
    exit 1
fi
# Zwei Schritte statt einer Pipe: Ohne `pipefail` verdeckte `|| true` sonst
# ein gescheitertes `inspect`.
env_lines=$(podman image inspect --format '{{range .Config.Env}}{{println .}}{{end}}' "$reference") || {
    echo "build-image: podman cannot read the environment of $reference" >&2
    exit 1
}
reserved=$(printf '%s\n' "$env_lines" | grep '^HUMANITL_' || true)
if [ -n "$reserved" ]; then
    echo "build-image: the image sets a variable the shim reads: $reserved" >&2
    exit 1
fi

echo "build-image: $reference" >&2
printf '%s\n' "$reference"
