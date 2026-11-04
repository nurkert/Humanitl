#!/usr/bin/env bash
# Schreibt das Update-Manifest zu einer Release-Datei und signiert es (HUM-382).
#
# Aufruf: packaging/release/sign-update.sh <0.Y.Z> <datei> <manifest-name> <min-os> <out-dir>
#
# Das Manifest ist JSON mit version, url, sha256, size und min_os; die URL
# zeigt auf die Datei unter den Assets genau dieses Releases. Signiert werden
# seine Bytes mit Ed25519 (`openssl pkeyutl -sign -rawin`), die Signatur steht
# roh (64 Bytes) daneben als <manifest-name>.sig. Der Daemon prüft sie gegen
# den öffentlichen Schlüssel in `daemon/crates/proxy/src/self_update.rs`
# (`RELEASE_KEY`), bevor er irgendetwas aus dem Manifest glaubt.
#
# Der private Schlüssel kommt als PEM aus der Umgebung
# (`HUMANITL_UPDATE_SIGNING_KEY`, ein Geheimnis des Repositorys). Er wird nie
# ausgegeben: Er landet in einer Datei mit 0600 in einem eigenen
# Verzeichnis, das am Ende gelöscht wird. Fehlt er, bricht das Skript mit
# REQUIRE_SIGNING=1 ab (Tag-Lauf) und überspringt sonst mit einer Warnung
# (Probelauf). Vor dem Schreiben wird geprüft, dass der Schlüssel zum
# eingebauten öffentlichen passt, und nach dem Signieren, dass die Signatur
# gilt: Ein falsches Geheimnis ergäbe sonst Releases, die keine App annimmt.
set -euo pipefail

[[ $# -eq 5 ]] || { echo "usage: $0 <0.Y.Z> <file> <manifest-name> <min-os> <out-dir>" >&2; exit 2; }
version="$1"
file="$2"
manifest_name="$3"
min_os="$4"
out="$5"
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
# shellcheck source=packaging/release/version.sh
source "$here/version.sh"
require_release_version "$version"
[[ "$manifest_name" =~ ^update-[a-z0-9_-]+\.json$ ]] || { echo "error: bad manifest name '$manifest_name'" >&2; exit 2; }
[[ "$min_os" =~ ^[0-9]+(\.[0-9]+){0,2}$ ]] || { echo "error: bad min-os '$min_os'" >&2; exit 2; }
[[ -f "$file" ]] || { echo "error: $file does not exist" >&2; exit 2; }

if [[ -z "${HUMANITL_UPDATE_SIGNING_KEY:-}" ]]; then
  if [[ "${REQUIRE_SIGNING:-0}" == "1" ]]; then
    echo "::error::HUMANITL_UPDATE_SIGNING_KEY is not set; a release without a signed update manifest cannot be installed from inside the app"
    exit 1
  fi
  echo "::warning::HUMANITL_UPDATE_SIGNING_KEY is not set; $manifest_name is not written (dry run)"
  exit 0
fi

# OpenSSL 3 kann `-rawin`; das LibreSSL von macOS nicht.
openssl_bin="${OPENSSL:-openssl}"
if ! "$openssl_bin" version 2>/dev/null | grep -q '^OpenSSL 3'; then
  if command -v brew >/dev/null && [[ -x "$(brew --prefix openssl@3 2>/dev/null)/bin/openssl" ]]; then
    openssl_bin="$(brew --prefix openssl@3)/bin/openssl"
  else
    echo "error: OpenSSL 3 is needed to sign with Ed25519 (set OPENSSL)" >&2
    exit 1
  fi
fi

expected_key="$(sed -n 's/^pub const RELEASE_KEY: &str = "\(.*\)";$/\1/p' \
  "$root/daemon/crates/proxy/src/self_update.rs")"
[[ -n "$expected_key" ]] || { echo "error: RELEASE_KEY not found in self_update.rs" >&2; exit 1; }

secret_dir="$(mktemp -d)"
trap 'rm -rf "$secret_dir"' EXIT
chmod 0700 "$secret_dir"
key="$secret_dir/key.pem"
( umask 077 && printf '%s\n' "$HUMANITL_UPDATE_SIGNING_KEY" >"$key" )

actual_key="$("$openssl_bin" pkey -in "$key" -pubout -outform DER 2>/dev/null | tail -c 32 | base64)"
if [[ "$actual_key" != "$expected_key" ]]; then
  echo "::error::HUMANITL_UPDATE_SIGNING_KEY does not match RELEASE_KEY in daemon/crates/proxy/src/self_update.rs"
  exit 1
fi

name="$(basename "$file")"
size="$(wc -c <"$file" | tr -d ' ')"
if command -v sha256sum >/dev/null; then
  sha="$(sha256sum "$file" | cut -d' ' -f1)"
else
  sha="$(shasum -a 256 "$file" | cut -d' ' -f1)"
fi
url="https://github.com/nurkert/Humanitl/releases/download/v$version/$name"
mkdir -p "$out"
manifest="$out/$manifest_name"
printf '{"version":"%s","url":"%s","sha256":"%s","size":%s,"min_os":"%s"}\n' \
  "$version" "$url" "$sha" "$size" "$min_os" >"$manifest"

"$openssl_bin" pkeyutl -sign -rawin -inkey "$key" -in "$manifest" -out "$manifest.sig"
"$openssl_bin" pkey -in "$key" -pubout -out "$secret_dir/pub.pem"
"$openssl_bin" pkeyutl -verify -pubin -inkey "$secret_dir/pub.pem" -rawin \
  -in "$manifest" -sigfile "$manifest.sig" >/dev/null
[[ "$(wc -c <"$manifest.sig" | tr -d ' ')" == "64" ]] || { echo "error: the signature is not 64 bytes" >&2; exit 1; }
echo "signed $manifest for $name ($size bytes)"
