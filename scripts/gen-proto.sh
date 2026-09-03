#!/usr/bin/env bash
# Erzeugt den Code aus `proto/humanitl/v1/` (HUM-003).
#
# Rust braucht dieses Skript nicht: `build.rs` der Crate `humanitl-ipc`
# uebersetzt die Proto bei jedem `cargo build` mit `protox` nach `OUT_DIR`,
# ohne `protoc` und ohne `buf`. Hier erneuert `cargo xtask proto` nur den
# eingecheckten `proto/descriptor.binpb`, den `tests/proto_contract.rs` und
# der Drift-Check in CI lesen.
#
# Dart braucht `protoc` und `protoc-gen-dart` in genau der Version
# PLUGIN_VERSION: `proto/generated.sha256` ist der Hash ueber die erzeugten
# Dateien, und CI vergleicht ihn mit dem eingecheckten Stand. Fehlt ein
# Werkzeug oder stimmt die Version nicht, sagt das Skript, was zu tun ist,
# und endet mit 0. Mit STRICT=1 oder CI=true endet es stattdessen mit 1; so
# laeuft CI hart und der Arbeitsplatz weich. Hart heisst auch: die Version
# von `protoc-gen-dart` muss sich ueber `dart pub global list` belegen
# lassen. Ein Plugin unbekannter Herkunft ist in CI ein Fehler, am
# Arbeitsplatz eine Warnung.
set -euo pipefail
cd "$(dirname "$0")/.."

# Gehoert zu `protobuf: ^6.0.0` in app/pubspec.yaml (docs/PROTOCOL.md 2).
# `.github/actions/setup-flutter` liest genau diese Zeile; Form beibehalten.
PLUGIN_VERSION="25.0.0"
DART_OUT="app/lib/core/ipc/generated"
HASH_FILE="proto/generated.sha256"
PROTO_FILES=(
  proto/humanitl/v1/common.proto
  proto/humanitl/v1/rules.proto
  proto/humanitl/v1/humanitl.proto
)

strict=""
if [[ -n "${STRICT:-}" || "${CI:-}" == "true" ]]; then
  strict=1
fi

# $1: Meldung. Im harten Modus Exit 1, sonst Exit 0 mit Hinweis.
fail_or_skip() {
  echo "$1" >&2
  if [[ -n "$strict" ]]; then
    echo "STRICT/CI set: refusing to skip Dart generation." >&2
    exit 1
  fi
  echo "Descriptor refreshed; Dart skipped."
  exit 0
}

echo "==> rust: proto/descriptor.binpb"
( cd daemon && cargo run --quiet -p humanitl-xtask -- proto )

echo "==> dart: $DART_OUT"
missing=""
command -v protoc >/dev/null 2>&1 || missing="$missing protoc"
command -v protoc-gen-dart >/dev/null 2>&1 || missing="$missing protoc-gen-dart"
if [[ -n "$missing" ]]; then
  fail_or_skip "SKIP dart:$missing not found in PATH.
Install:
  sudo apt install protobuf-compiler        # protoc
  dart pub global activate protoc_plugin $PLUGIN_VERSION
  export PATH=\"\$PATH:\$HOME/.pub-cache/bin\"  # protoc-gen-dart"
fi

# Die Plugin-Version bestimmt die Ausgabe und damit den Hash. `dart pub global
# list` kennt nur Plugins aus pub. Eine andere Version ist immer ein Grund,
# nicht zu erzeugen. Eine Version, die sich nicht belegen laesst, ist im
# harten Modus ein Fehler (der Hash waere dann nichts wert) und am
# Arbeitsplatz eine Warnung. `|| true`, weil ein fehlendes `dart` hier nur
# "unbelegt" heisst, nicht Abbruch.
installed="$({ dart pub global list 2>/dev/null || true; } | awk '$1 == "protoc_plugin" { print $2 }')"
if [[ -n "$installed" && "$installed" != "$PLUGIN_VERSION" ]]; then
  fail_or_skip "SKIP dart: protoc_plugin $installed is active, $PLUGIN_VERSION is required:
  dart pub global activate protoc_plugin $PLUGIN_VERSION"
fi
if [[ -z "$installed" ]]; then
  if [[ -n "$strict" ]]; then
    echo "protoc-gen-dart is in PATH, but \`dart pub global list\` shows no protoc_plugin; cannot verify it is $PLUGIN_VERSION." >&2
    echo "STRICT/CI set: refusing to generate with an unverified plugin. Install it via pub:" >&2
    echo "  dart pub global activate protoc_plugin $PLUGIN_VERSION" >&2
    exit 1
  fi
  echo "note: protoc-gen-dart was not installed via pub; cannot verify it is $PLUGIN_VERSION" >&2
fi

rm -rf "$DART_OUT"
mkdir -p "$DART_OUT"
protoc --proto_path=proto --dart_out=grpc:"$DART_OUT" "${PROTO_FILES[@]}"

# Drift-Wachhund fuer CI: `generated/` ist gitignored, der Hash nicht. Ohne
# Dateien gibt es keinen Hash: `sha256sum` ohne Argumente laese stdin und
# schriebe den Hash ueber nichts als `-`, und der Wachhund waere blind.
mapfile -t dart_files < <(cd "$DART_OUT" && find . -type f -name '*.dart' | LC_ALL=C sort)
if (( ${#dart_files[@]} == 0 )); then
  echo "protoc wrote no .dart files to $DART_OUT; refusing to write $HASH_FILE" >&2
  exit 1
fi
# Erst in eine temporaere Datei daneben, dann `mv`: ein Abbruch mittendrin
# laesst nie einen halben Hash zurueck, den `git diff` fuer echt hielte.
hash_tmp="$(mktemp "$HASH_FILE.XXXXXX")"
trap 'rm -f "$hash_tmp"' EXIT
( cd "$DART_OUT" && sha256sum -- "${dart_files[@]}" ) > "$hash_tmp"
mv -f "$hash_tmp" "$HASH_FILE"
trap - EXIT

# `git diff --exit-code` in CI sieht nur Dateien, die Git kennt. Ein nicht
# eingecheckter Hash waere ein Wachhund, der nie bellt.
if [[ -n "$strict" ]] && ! git ls-files --error-unmatch "$HASH_FILE" >/dev/null 2>&1; then
  echo "$HASH_FILE is not tracked by git; commit it, or the drift check cannot see it." >&2
  exit 1
fi

echo "proto generated ($(protoc --version), protoc_plugin ${installed:-unverified})"
