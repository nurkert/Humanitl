#!/usr/bin/env bash
# Schreibt `SHA256SUMS` fuer alle Artefakte in <dist-dir> und prueft die
# Datei gleich gegen die Artefakte. Der Name ist der, den HUM-060 fuer 0.1.0
# vorsieht; dort kommt die minisign-Signatur der Datei hinzu.
#
# Aufruf: packaging/release/checksums.sh <dist-dir>
set -euo pipefail

[[ $# -eq 1 ]] || { echo "usage: $0 <dist-dir>" >&2; exit 2; }
cd "$1"
shopt -s nullglob
files=(*.deb *.tar.gz)
[[ ${#files[@]} -gt 0 ]] || { echo "error: no .deb or .tar.gz in $1" >&2; exit 1; }
sha256sum "${files[@]}" >SHA256SUMS
sha256sum -c SHA256SUMS
cat SHA256SUMS
