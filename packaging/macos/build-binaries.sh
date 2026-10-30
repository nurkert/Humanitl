#!/usr/bin/env bash
# Baut humanitld, humanitl und humanitl-shim für beide Mac-Architekturen und
# verbindet je Programm die beiden zu einem Universal-Binary (HUM-316).
#
# Aufruf: packaging/macos/build-binaries.sh <out-dir>
#
# Der Shim braucht unter macOS keine statische Bindung: Er läuft unter
# Seatbelt von seinem eigenen Pfad im Bündel, nicht in einem leeren
# Dateisystem wie unter bwrap (HUM-313).
set -euo pipefail

[[ $# -eq 1 ]] || { echo "usage: $0 <out-dir>" >&2; exit 2; }
mkdir -p "$1"
out="$(cd "$1" && pwd)"
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root/daemon"
target_dir="${CARGO_TARGET_DIR:-$root/daemon/target}"

targets=(aarch64-apple-darwin x86_64-apple-darwin)
for target in "${targets[@]}"; do
  rustup target add "$target" >/dev/null
  cargo build --locked --release --target "$target" \
    -p humanitld -p humanitl -p humanitl-shim \
    --bin humanitld --bin humanitl --bin humanitl-shim
done
for name in humanitld humanitl humanitl-shim; do
  lipo -create \
    "$target_dir/aarch64-apple-darwin/release/$name" \
    "$target_dir/x86_64-apple-darwin/release/$name" \
    -output "$out/$name"
  chmod 0755 "$out/$name"
  echo "$name: $(lipo -archs "$out/$name")"
done
