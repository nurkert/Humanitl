#!/usr/bin/env bash
# Prueft das RPM mit rpmlint, im Wegwerf-Container mit Fedora wie
# `check-install.sh` (HUM-339), das Gegenstueck zu `packaging/deb/lintian.sh`.
#
# Aufruf (als root, im Container): rpmlint.sh <rpm>
#
# Fehler und Warnungen lassen den Lauf scheitern. Jede Meldung, die bleiben
# darf, steht mit Begruendung als `addFilter` in
# `packaging/rpm/humanitl.rpmlintrc`. Anders als die lintian-Overrides reist
# die Datei nicht im Paket mit; rpm kennt dafuer keinen Ort.
#
# Lokal auf Apple Silicon (colima, linux/amd64 unter qemu) scheitert das
# `tar` von Fedora beim Auspacken mit "Cannot mkdir: Function not implemented",
# und rpmlint meldet dann einen Lesefehler des Pakets. Das ist die Emulation,
# nicht das RPM; auf x86_64 und in der CI laeuft die Pruefung.
set -euo pipefail

[[ $# -eq 1 ]] || { echo "usage: $0 <rpm>" >&2; exit 2; }
rpm_file="$(readlink -f "$1")"
here="$(cd "$(dirname "$0")" && pwd)"
[[ "$(id -u)" -eq 0 ]] || { echo "error: run as root inside a throwaway container" >&2; exit 1; }

if ! command -v rpmlint >/dev/null; then
  dnf install -y -q --setopt=install_weak_deps=False rpmlint
fi
rpmlint --version
# rpmlint endet bei Warnungen mit 0 und bei Fehlern nicht immer mit einem
# eigenen Code; entschieden wird deshalb an seiner Zusammenfassung.
set +e
rpmlint -r "$here/humanitl.rpmlintrc" "$rpm_file" 2>&1 | tee /tmp/rpmlint.out
rc=${PIPESTATUS[0]}
set -e
summary="$(grep -E '[0-9]+ packages and [0-9]+ specfiles checked' /tmp/rpmlint.out | tail -n 1 || true)"
[[ -n "$summary" ]] || { echo "error: rpmlint printed no summary (exit $rc)" >&2; exit 1; }
errors="$(sed -n 's/.* \([0-9][0-9]*\) errors\{0,1\}, .*/\1/p' <<<"$summary")"
warnings="$(sed -n 's/.* \([0-9][0-9]*\) warnings\{0,1\}.*/\1/p' <<<"$summary")"
if [[ "$errors" != 0 || "$warnings" != 0 ]]; then
  echo "error: rpmlint reports $errors error(s) and $warnings warning(s) that humanitl.rpmlintrc does not explain" >&2
  exit 1
fi
echo "== rpmlint: no errors, no warnings"
