#!/usr/bin/env bash
# Baut die drei Programme einer Vorabversion und legt sie in <ausgabe>/ ab.
#
# Aufruf: packaging/release/build-binaries.sh <ausgabe>
#
# `humanitld` und `humanitl` kommen aus `[profile.release]`, der Shim aus
# `[profile.shim]` und statisch gelinkt, genau wie im CI-Job `rust-test` und in
# `tests/escape/run.sh` (HUM-012): Er ist das einzige Programm, das in die
# Sandbox eingehaengt wird, und dort gibt es keine Bibliotheken des Hosts.
#
# `--locked` ist Pflicht: Ohne die Sperre koennte die Vorabversion andere
# Abhaengigkeiten enthalten als der Stand, den CI geprueft hat.
#
# `CARGO_TARGET_DIR` wird respektiert; ohne die Variable baut Cargo nach
# `daemon/target`.
set -euo pipefail

[[ $# -eq 1 ]] || { echo "usage: $0 <out-dir>" >&2; exit 2; }
out="$1"
root="$(cd "$(dirname "$0")/../.." && pwd)"
mkdir -p "$out"
out="$(cd "$out" && pwd)"

cd "$root/daemon"
target_dir="${CARGO_TARGET_DIR:-$root/daemon/target}"

cargo build --locked --release -p humanitld -p humanitl --bin humanitld --bin humanitl

# Der Shim: musl, wenn Ziel und Linker da sind, sonst glibc mit +crt-static.
# -C relocation-model=static macht daraus ein ET_EXEC statt eines statischen
# PIE; erst dann antwortet ldd mit "not a dynamic executable".
musl="$(uname -m)-unknown-linux-musl"
target_flag=()
shim="$target_dir/shim/humanitl-shim"
if command -v musl-gcc >/dev/null && command -v rustup >/dev/null \
  && rustup target add "$musl" >/dev/null 2>&1; then
  target_flag=(--target "$musl")
  shim="$target_dir/$musl/shim/humanitl-shim"
else
  echo "note: $musl or its linker is not available, building the shim with +crt-static"
fi
cargo rustc --locked -p humanitl-shim --bin humanitl-shim --profile shim \
  "${target_flag[@]}" -- -C target-feature=+crt-static -C relocation-model=static

linkage="$(ldd "$shim" 2>&1 || true)"
case "$linkage" in
  *"not a dynamic executable"*) ;;
  *) echo "error: the shim is not statically linked: $linkage" >&2; exit 1 ;;
esac
bytes="$(wc -c <"$shim")"
if [[ "$bytes" -ge 2097152 ]]; then
  echo "error: the shim is $bytes bytes, the limit is 2097152" >&2
  exit 1
fi

install -m 0755 "$target_dir/release/humanitld" "$out/humanitld"
install -m 0755 "$target_dir/release/humanitl" "$out/humanitl"
install -m 0755 "$shim" "$out/humanitl-shim"
# Gestrippt wird erst beim Verpacken (`packaging/release/tidy-elf.sh`), an
# der Kopie im Paket und im Archiv.
ls -l "$out"
