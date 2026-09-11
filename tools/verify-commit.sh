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
# Loest das Argument einmal auf, bevor irgendetwas angelegt wird: ein Tippfehler
# soll hier scheitern und nicht erst, wenn schon ein Baum steht. Die Zeile, die
# unten ausgegeben wird, kommt aus dem Baum selbst -- sie belegt, was wirklich
# ausgecheckt ist, statt zu wiederholen, wonach gefragt wurde.
git rev-parse --verify --quiet "$commit^{commit}" > /dev/null || {
  echo "verify-commit: $commit ist kein Commit in diesem Repository" >&2
  exit 2
}

# Der Auscheckpfad ist fest. Er hing eine Zeit lang am Commit, weil zwei
# gleichzeitige Pruefungen sich mit einem festen Pfad den Baum weggeloescht
# haben -- das war, bevor es die Sperre weiter unten gab. Die wird jetzt
# genommen, bevor irgendetwas am Baum passiert, also laeuft ohnehin nur eine
# Pruefung zugleich, und ein Baum je Commit kauft nichts mehr.
#
# Er kostet dafuer: Cargo bezieht seine Fingerabdruecke auf den absoluten Pfad
# der Quellen, also war jeder Lauf unter einem neuen Pfad ein vollstaendiger
# Neubau des Workspace -- rund zehn Minuten und zweistellige Gigabyte, jedes
# Mal.
#
# Warm bleibt mit dem festen Pfad nicht der Baum: Den loescht jeder Lauf und
# checkt ihn neu aus. Warm bleibt das Zielverzeichnis. Weil der Quellpfad
# konstant ist, verlieren die Fingerabdruecke ihre Gueltigkeit nicht mehr, und
# die Fremdkisten, die den Grossteil der Bauzeit ausmachen, bleiben uebersetzt.
#
# Wer zwei Pruefungen nebeneinander braucht, setzt HUMANITL_VERIFY_TREE und
# HUMANITL_VERIFY_TARGET **beide** auf eigene Pfade. Jede der beiden Ressourcen
# hat ihre eigene Sperre; wer nur eine Variable setzt, teilt die andere
# Ressource weiterhin und wird an deren Sperre abgewiesen, statt sie zu
# zerlegen.
tree="${HUMANITL_VERIFY_TREE:-/tmp/humanitl-verify}"

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

# Beide Pfade absolut und ohne Schraegstrich am Ende, bevor irgendetwas daraus
# gebaut wird. Drei Dinge haengen daran, und alle drei sind schon einmal
# schiefgegangen oder koennten es:
#
# 1. Die Sperrdatei heisst `<pfad>.lock`. Bei `/tmp/baum//` liesse `${p%/}` einen
#    Schraegstrich stehen, die Sperre hiesse `/tmp/baum/.lock` und laege damit
#    **im** Baum -- den dieser Lauf gleich loescht. Eine geloeschte Sperrdatei
#    sperrt nichts mehr: Der naechste Lauf legt einen neuen Inode an demselben
#    Pfad an und bekommt die Sperre sofort.
# 2. Ein relativer `HUMANITL_VERIFY_TARGET` wird an drei Stellen verschieden
#    aufgeloest: hier gegen die Repository-Wurzel, weiter unten von cargo gegen
#    `$tree`, und in `tests/escape/run.sh` gegen `daemon/`.
# 3. Die Laengenpruefung unten misst den Pfad, der wirklich benutzt wird.
if ! command -v realpath > /dev/null; then
  echo "verify-commit: realpath fehlt (coreutils); ohne es sind die Pfade nicht eindeutig" >&2
  exit 2
fi
tree="$(realpath -m "$tree")"
target="$(realpath -m "$target")"

# Baum und Zielverzeichnis muessen zwei verschiedene Orte sein, und das Ziel darf
# nicht im Baum liegen.
#
# Gleicher Pfad: Beide Sperren waeren dieselbe Datei, aber zwei offene
# Dateibeschreibungen. `flock` sperrt je Beschreibung, also wiese der Lauf sich
# selbst ab -- mit der Meldung, ein anderer Lauf sei zugange, den es nicht gibt.
#
# Ziel im Baum: `rm -rf "$tree"` nimmt die Sperrdatei des Ziels mit, und danach
# sperrt sie nichts mehr (siehe Punkt 1 oben).
if [[ "$tree" == "$target" ]]; then
  echo "verify-commit: Baum und Zielverzeichnis sind derselbe Pfad ($tree)" >&2
  echo "verify-commit: setze HUMANITL_VERIFY_TREE und HUMANITL_VERIFY_TARGET auf zwei Orte" >&2
  exit 2
fi
if [[ "$target" == "$tree"/* ]]; then
  echo "verify-commit: das Zielverzeichnis liegt im Baum ($target in $tree)" >&2
  echo "verify-commit: der Baum wird bei jedem Lauf geloescht, seine Sperre mit ihm" >&2
  exit 2
fi

# Der Baum wird gleich mit `rm -rf` geloescht. Ein Tippfehler in
# HUMANITL_VERIFY_TREE ist damit kein Tippfehler mehr, sondern ein Datenverlust:
# `HUMANITL_VERIFY_TREE=$HOME` loeschte das Heimatverzeichnis, und gegen `/`
# schuetzt allein, dass GNU `rm` die Wurzel bewahrt. Deshalb vier Verbote, bevor
# irgendetwas geloescht wird. Sie sind absichtlich grob: Dieses Werkzeug legt
# seinen Baum an einer Wegwerfstelle an, nicht in einem Verzeichnis, das jemandem
# gehoert.
repo_root="$(realpath -m .)"
for forbidden in / "$HOME" "$repo_root"; do
  if [[ "$tree" == "$forbidden" ]]; then
    echo "verify-commit: $tree wird bei jedem Lauf geloescht; das ist kein Wegwerfpfad" >&2
    exit 2
  fi
done
if [[ "$repo_root" == "$tree"/* ]]; then
  echo "verify-commit: $tree enthaelt dieses Repository und wird bei jedem Lauf geloescht" >&2
  exit 2
fi
# Ein vorhandener Pfad darf nur weg, wenn er von diesem Werkzeug stammt: ein
# Arbeitsbaum dieses Repositories (er traegt eine Datei `.git`, keinen Ordner).
if [[ -e "$tree" && ! -f "$tree/.git" ]]; then
  echo "verify-commit: $tree existiert und ist kein Arbeitsbaum dieses Repositories" >&2
  echo "verify-commit: dieser Lauf wuerde ihn loeschen; waehle einen anderen Pfad" >&2
  exit 2
fi

if [[ ${#tree} -gt 60 ]]; then
  echo "verify-commit: $tree ist zu lang für einen Unix-Socket in den Escape-Tests" >&2
  exit 2
fi

# Zwei Sperren, eine je geteilter Ressource, beide vor dem ersten Schreiben.
#
# Das **Zielverzeichnis**: Zwei Laeufe mit getrennten Baeumen, aber gemeinsamem
# Zielverzeichnis bauen dieselben Binaries an dieselbe Stelle. Cargo nimmt dort
# zwar seine eigene Sperre, aber die Escape-Tests und die Sandbox-Tests starten
# Programme neben sich, ausser jeder Cargo-Sperre; tauscht der andere Lauf sie
# zwischendurch aus, scheitern sie mit einem Fehler, der wie ein Codefehler
# aussieht. Genau so ist am 2026-09-05
# `sigint_reaches_the_agent_and_keeps_its_exit_code` gefallen, waehrend
# derselbe Test einzeln sechs Mal unter Last 25 in 50 ms durchlief.
#
# Der **Baum**: Der Lauf loescht ihn beim Anlegen und beim Aufraeumen. Zwei
# Laeufe darin nehmen einander die Quellen weg, waehrend der andere darin testet.
# Am 2026-09-05 ist genau das passiert, und der Schaden war nicht ein roter Lauf,
# sondern ein Lauf, der nie endete: Dem `frontend_server` von Dart wurde sein
# Arbeitsverzeichnis unter den Fuessen geloescht, er rief `Uri.base`, das ruft
# `getcwd()`, und er starb mit
# `PathNotFoundException: Getting current working directory failed, path = ''`.
# Der Testlaeufer wartete danach auf einen Uebersetzer, den es nicht mehr gab.
# Der Prozess stand am naechsten Tag noch, 25 Stunden, bei null Prozent CPU, mit
# acht verwaisten `flutter_tester` daneben. Ein Lauf, der haengt, sagt niemandem
# etwas -- deswegen diese Sperre und nicht bloss ein Baum je Commit.
#
# Eine Sperre allein reichte nur, solange beide Pfade zusammen voreingestellt
# sind. Wer eine Variable setzt und die andere nicht, teilte sonst genau eine der
# zwei Ressourcen ungeschuetzt.
#
# **Warum `flock` als Wrapper und nicht `exec {fd}>` im Skript.** Ein so
# geoeffneter Deskriptor ist nicht `close-on-exec`: Auf bash 5.3.9 nachgemessen,
# er taucht in jedem `exec`-ten Kind wieder auf. Dieses Skript startet `cargo`,
# `flutter`, Daemons und Sandboxen; ueberlebt eines dieser Kinder den Lauf --
# und verwaiste Prozesse hat dieses Repository schon gesehen --, haelt es die
# Sperre weiter, und jeder spaetere Lauf endet mit 3, obwohl niemand mehr prueft.
# `flock --close` schliesst den Deskriptor, bevor es den Befehl startet; die
# Sperre haelt der `flock`-Prozess selbst, so lange sein Kind laeuft, und kein
# Enkel erbt sie. Das Skript ruft sich dafuer einmal unter beiden Sperren neu
# auf; `HUMANITL_VERIFY_LOCKED` bricht die Rekursion.
tree_lock="${tree}.lock"
target_lock="${target}.lock"
if [[ -z "${HUMANITL_VERIFY_LOCKED:-}" ]]; then
  mkdir -p "$(dirname "$tree_lock")" "$(dirname "$target_lock")"
  export HUMANITL_VERIFY_LOCKED=1
  # Die schon aufgeloesten Pfade, nicht die urspruenglichen: Das Kind rechnet
  # sonst `realpath` noch einmal, und ein Symlink, der zwischen beiden Aufrufen
  # umgebogen wird, liesse es unter einem anderen Pfad arbeiten als dem, dessen
  # Sperre gehalten wird (HUM-147, Review).
  export HUMANITL_VERIFY_TREE="$tree" HUMANITL_VERIFY_TARGET="$target"
  # 3 heisst „nicht bekommen". Das Skript selbst endet mit 0, 1 oder 2, also ist
  # die Zahl eindeutig.
  set +e
  flock --close --nonblock --conflict-exit-code 3 "$tree_lock" \
    flock --close --nonblock --conflict-exit-code 3 "$target_lock" \
      "$0" "$@"
  rc=$?
  set -e
  if [[ $rc -eq 3 ]]; then
    echo "verify-commit: $tree oder $target ist von einem anderen Lauf belegt; dieser Lauf endet ohne Ergebnis" >&2
    echo "verify-commit: warte auf ihn oder setze HUMANITL_VERIFY_TREE und HUMANITL_VERIFY_TARGET beide auf eigene Pfade" >&2
  fi
  exit "$rc"
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

# Wie die CI (`.github/workflows/ci.yml`, `CARGO_INCREMENTAL: "0"`): kein
# inkrementeller Bau. Jeder Lauf beginnt in einem neuen Worktree, die
# Inkrement-Artefakte halfen ihm kaum und wuchsen mit jedem geprueften Commit;
# am 2026-09-11 lagen 30 GB davon im Zielverzeichnis (HUM-147). Was schon da
# ist, liest mit dieser Einstellung niemand mehr, also geht es weg -- hier,
# waehrend dieser Lauf die Sperre des Zielverzeichnisses haelt.
#
# Nur in einem Zielverzeichnis von Cargo, erkennbar an `.rustc_info.json`, das
# Cargo in die Wurzel jedes Zielverzeichnisses schreibt (`CACHEDIR.TAG` fehlt
# in aelteren, gemessen am 2026-09-11 in diesem hier). Zeigt
# HUMANITL_VERIFY_TARGET versehentlich auf einen Ordner, der jemandem gehoert,
# verschwindet dort kein Unterordner, der zufaellig `incremental` heisst.
if [[ -f "$target/.rustc_info.json" ]]; then
  find "$target" -mindepth 2 -maxdepth 2 -type d -name incremental -prune \
    -exec rm -rf {} + 2> /dev/null || true
fi

fail=0
step() {
  local name="$1"; shift
  echo "verify-commit: $name"
  if ! (cd "$tree" && CARGO_TARGET_DIR="$target" CARGO_INCREMENTAL=0 STRICT=1 ESCAPE_ALLOW_FAIL="${ESCAPE_ALLOW_FAIL:-}" "$@"); then
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
