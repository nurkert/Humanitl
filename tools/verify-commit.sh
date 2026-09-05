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
sha="$(git rev-parse --short=8 "$commit")"

# Der Auscheckpfad haengt am Commit, nicht an einem festen Namen. Waehrend
# mehrere Agenten arbeiten, laufen hier durchaus zwei Pruefungen zugleich; mit
# einem festen Pfad hat die zweite den Baum der ersten geloescht, waehrend
# diese darin testete, und beim Beenden nahm jede den Baum der anderen mit.
# Der Aufraeumschritt unten fasst deshalb nur noch den eigenen Baum an.
tree="${HUMANITL_VERIFY_TREE:-/tmp/hv-$sha}"

# Das Zielverzeichnis liegt bewusst nicht unter /tmp. Dort steht ein tmpfs von
# 16 GB, ein vollstaendiger Build des Workspace belegt 18 bis 36 GB, und ein
# volles tmpfs laesst rustc mit "failed to parse process output" abbrechen --
# ein Fehler, der wie ein Codefehler aussieht und keiner ist. Am 2026-09-05 ist
# die Pruefung genau daran zweimal gescheitert.
#
# Gemeinsam bleibt es trotzdem: ein eigenes Verzeichnis je Lauf kostet dieselben
# 18 bis 36 GB noch einmal, und die Sperre oben laesst ohnehin nur einen Lauf
# zugleich daran.
target="${HUMANITL_VERIFY_TARGET:-$HOME/.cache/humanitl/verify-target}"

if [[ ${#tree} -gt 60 ]]; then
  echo "verify-commit: $tree ist zu lang für einen Unix-Socket in den Escape-Tests" >&2
  exit 2
fi

# Die Sperre haengt am Zielverzeichnis, nicht am Baum: zwei Laeufe mit
# getrennten Baeumen, aber gemeinsamem Zielverzeichnis bauen dieselben
# Binaries an dieselbe Stelle. Cargo nimmt dort zwar seine eigene Sperre, aber
# die Escape-Tests und die Sandbox-Tests starten Programme neben sich, ausser
# jeder Cargo-Sperre; tauscht der andere Lauf sie zwischendurch aus, scheitern
# sie mit einem Fehler, der wie ein Codefehler aussieht. Genau so ist am
# 2026-09-05 `sigint_reaches_the_agent_and_keeps_its_exit_code` gefallen,
# waehrend derselbe Test einzeln sechs Mal unter Last 25 in 50 ms durchlief.
lock="${target%/}.lock"
mkdir -p "$(dirname "$lock")"
exec {lockfd}> "$lock"
if ! flock -n "$lockfd"; then
  echo "verify-commit: $target ist von einem anderen Lauf belegt; dieser Lauf endet ohne Ergebnis" >&2
  echo "verify-commit: warte auf ihn oder setze HUMANITL_VERIFY_TARGET auf ein eigenes Verzeichnis" >&2
  exit 3
fi

cleanup() {
  git worktree remove --force "$tree" > /dev/null 2>&1 || true
  git worktree prune > /dev/null 2>&1 || true
}
trap cleanup EXIT

git worktree remove --force "$tree" > /dev/null 2>&1 || true
rm -rf "$tree"
git worktree add -q --detach "$tree" "$commit"
echo "verify-commit: $(git -C "$tree" rev-parse --short HEAD) in $tree"

# Werkzeuge, die nicht im Systempfad stehen, aber jeder Schritt braucht:
# rustfmt und clippy liegen in der Toolchain, protoc und protoc-gen-dart holt
# sich `scripts/gen-proto.sh`, und mit STRICT=1 ist ein fehlendes protoc ein
# Fehler statt eines Ueberspringens. Fehlt eines davon, sagt das Skript es,
# statt den Schritt still scheitern zu lassen.
for dir in \
  "$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin" \
  "$HOME/.pub-cache/bin" \
  "${HUMANITL_PROTOC_BIN:-}"; do
  [[ -n "$dir" && -d "$dir" ]] && PATH="$dir:$PATH"
done
export PATH
for tool in cargo rustfmt protoc protoc-gen-dart flutter; do
  command -v "$tool" > /dev/null || echo "verify-commit: $tool fehlt im PATH; der zugehoerige Schritt wird scheitern" >&2
done

fail=0
step() {
  local name="$1"; shift
  echo "verify-commit: $name"
  if ! (cd "$tree" && CARGO_TARGET_DIR="$target" STRICT=1 ESCAPE_ALLOW_FAIL="${ESCAPE_ALLOW_FAIL:-}" "$@"); then
    echo "verify-commit: $name schlug fehl" >&2
    fail=1
  fi
}

step "Format und Lints" make rust-fmt rust-clippy
step "Bau und Tests" make rust-build rust-test
# Wie der CI-Job rust-test: ohne private Items, damit ein oeffentlicher
# Doc-Kommentar, der auf ein privates Item verweist, hier auffaellt.
step "Dokumentation" env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --manifest-path daemon/Cargo.toml
step "Abhängigkeiten" make deps-lint
step "Dokumente" make docs-lint
step "Lizenzen" make rust-deny
# Wie im CI-Job escape-tests: eine rote Probe ist erlaubt, solange sie zu einem
# Issue gehoert, das noch aussteht; eine Sandbox, die gar nicht startet, nicht.
export ESCAPE_ALLOW_FAIL=1
# Die Oberflaeche gehoert dazu: Ein geaendertes Proto erzeugt neuen Dart-Code,
# und eine neue Variante im Kern fehlt der App, bis jemand sie nachtraegt. Ohne
# diesen Schritt faellt das erst in der CI auf (so geschehen am 2026-09-03).
step "Flutter" make flutter-analyze flutter-test

step "Escape-Tests" bash tests/escape/run.sh

if [[ "$fail" -ne 0 ]]; then
  echo "verify-commit: der Commit ist nicht grün; nicht pushen" >&2
  exit 1
fi
echo "verify-commit: grün"
