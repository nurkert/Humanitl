#!/usr/bin/env bash
# Prüft den Zustand, der wirklich committet ist, statt des Arbeitsbaums.
#
# Der Arbeitsbaum enthält beim Entwickeln fast immer mehr als der Commit:
# Dateien anderer Issues, noch nicht hinzugefügte Register-Einträge, Profile,
# die zu einem Test gehören. `make check` im Arbeitsbaum ist deshalb grün,
# während derselbe Stand auf `main` nicht baut. Dieses Skript checkt einen
# Commit in einen eigenen, leeren Baum aus und fährt dort die Prüfungen der CI.
#
# Aufruf: tools/verify-commit.sh [commit-ish]   (Vorgabe: HEAD)
#
# Der Auscheckpfad ist bewusst kurz: Die Escape-Tests legen einen Unix-Socket
# darunter an, und ein Socket-Pfad darf 108 Zeichen nicht überschreiten.
set -euo pipefail
cd "$(dirname "$0")/.."

commit="${1:-HEAD}"
tree="${HUMANITL_VERIFY_TREE:-/tmp/humanitl-verify}"
target="${HUMANITL_VERIFY_TARGET:-/tmp/humanitl-verify-target}"

if [[ ${#tree} -gt 60 ]]; then
  echo "verify-commit: $tree ist zu lang für einen Unix-Socket in den Escape-Tests" >&2
  exit 2
fi

cleanup() {
  git worktree remove --force "$tree" > /dev/null 2>&1 || true
  git worktree prune > /dev/null 2>&1 || true
}
trap cleanup EXIT

cleanup
rm -rf "$tree"
git worktree add -q --detach "$tree" "$commit"
echo "verify-commit: $(git -C "$tree" rev-parse --short HEAD) in $tree"

fail=0
step() {
  local name="$1"; shift
  echo "verify-commit: $name"
  if ! (cd "$tree" && CARGO_TARGET_DIR="$target" STRICT=1 "$@"); then
    echo "verify-commit: $name schlug fehl" >&2
    fail=1
  fi
}

step "Format und Lints" make rust-fmt rust-clippy
step "Bau und Tests" make rust-build rust-test
step "Abhängigkeiten" make deps-lint
step "Dokumente" make docs-lint
step "Lizenzen" make rust-deny
step "Escape-Tests" bash tests/escape/run.sh

if [[ "$fail" -ne 0 ]]; then
  echo "verify-commit: der Commit ist nicht grün; nicht pushen" >&2
  exit 1
fi
echo "verify-commit: grün"
