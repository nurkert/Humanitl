#!/usr/bin/env bash
# Installiert auf dem CI-Runner ein bubblewrap mit `--overlay` (HUM-350).
#
# Ubuntu 24.04 liefert bubblewrap 0.9.0; `--overlay` kam mit 0.11.0. Ohne es
# antwortet jede Wegwerf-Sandbox mit SANDBOX_030, und die Tests, die sie mit
# echtem bwrap starten (`ipc/tests/sandbox_start.rs`, ESC-5), prüften nichts.
# Das Paket kommt aus Debian 13 (trixie), über snapshot.debian.org fest an
# seine SHA-1 gebunden und gegen eine SHA-256 geprüft, im selben Geist wie die
# gepinnten Werkzeuge in rust-test. Abhängigkeiten: libc6 >= 2.38, libcap2,
# libselinux1; Ubuntu 24.04 bringt alle mit.
set -euo pipefail

deb=bubblewrap_0.12.0-1~deb13u1_amd64.deb
sha1=bc858614dea46c779fe2b1a1d40643de47dd4f9c
sha256=70aca4fa8daeacb677ec00e8063eb586f08ae3d94b1f11e684370b5524c43431

curl -fsSL -o "/tmp/$deb" "https://snapshot.debian.org/file/$sha1"
echo "$sha256  /tmp/$deb" | sha256sum -c -
sudo dpkg -i "/tmp/$deb"
bwrap --version
bwrap --help 2>&1 | grep -q -- '--overlay' || {
  echo "::error::the installed bubblewrap has no --overlay" >&2
  exit 1
}
