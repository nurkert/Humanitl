#!/usr/bin/env bash
# install.sh in frischen Containern der unterstuetzten Linux-Systeme (HUM-528).
#
# Aufruf: tools/tests/install_sh_containers.sh [<system>...]
#   Systeme: debian ubuntu fedora opensuse arch generic (ohne Angabe alle)
#
# Je System ein frischer Container. Darin laeuft install.sh als normaler
# Nutzer `tester` (sudo ohne Passwort, wo ein Paketweg es braucht), gegen
# einen lokalen Stellvertreter der Release-Dateien (HUMANITL_RELEASES_URL,
# file://) und, auf Debian und Ubuntu, gegen ein lokales, signiertes
# APT-Repository (HUMANITL_APT_URL) mit dem .deb des Releases. Die Dateien
# sind die echten des neuesten Releases, mit dessen SHA256SUMS.
#
# Je System:
#   1. --dry-run: Exit 0, danach kein Paket, keine Quelle, kein Schluessel,
#      nichts unter ~/.local, kein Rest in /tmp.
#   2. --yes --no-setup: installiert auf dem erwarteten Weg (apt: das .deb
#      des Releases nach Pruefung gegen SHA256SUMS, ohne gnupg im Container;
#      rpm; tarball).
#   3. derselbe Aufruf noch einmal: ersetzt die Installation.
#   4. `humanitl --version` nennt die Version des Releases.
#
# generic ist ein Debian-Abbild mit einer os-release, die keine bekannte
# Distribution nennt: der Archiv-Weg fuer "sonstiges Linux". Arch nimmt
# ebenfalls das Archiv, weil im Container kein AUR-Helfer liegt; openSUSE,
# weil das .rpm dort noch nicht geprueft ist.
#
# Umgebung:
#   ENGINE          docker oder podman; ohne Angabe, was da ist
#   INSTALL_ASSETS  Verzeichnis mit SHA256SUMS, .deb, .rpm und .tar.gz einer
#                   Version; ohne Angabe laedt das Skript das neueste Release
#   INSTALL_VERSION die Version dieser Dateien (mit INSTALL_ASSETS Pflicht)
#
# Die Abbilder sind ueber ihren Digest festgelegt, wie in release.yml, und
# voll benannt, damit podman keinen Kurznamen aufloest.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"

declare -A image=(
  [debian]='docker.io/library/debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2'
  [ubuntu]='docker.io/library/ubuntu:24.04@sha256:b3cc40b72b93588182b5410f723c7aaf142363311c2aa993d8a453ddcbb3ae15'
  [fedora]='docker.io/library/fedora:44@sha256:43b29f65a41eb9c35e1cd5323e3bdf3b655c2357a9f4f1ff2f9c2798e5045d80'
  [opensuse]='docker.io/opensuse/tumbleweed:latest@sha256:39db4a0b5baf86c2d02366c27396832e5fcbcb67ba56efcf263b46892a99681a'
  [arch]='docker.io/library/archlinux:base@sha256:d4594bb2300cba11f41c2f27923eb56f63d9988909c402f9c95517d1e362c899'
  [generic]='docker.io/library/debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2'
)
declare -A method=([debian]=apt [ubuntu]=apt [fedora]=rpm [opensuse]=tarball [arch]=tarball [generic]=tarball)

systems=("$@")
[[ ${#systems[@]} -gt 0 ]] || systems=(debian ubuntu fedora opensuse arch generic)
for s in "${systems[@]}"; do
  [[ -n "${image[$s]:-}" ]] || { echo "unknown system: $s" >&2; exit 2; }
done

engine="${ENGINE:-}"
if [[ -z "$engine" ]]; then
  if command -v docker > /dev/null; then engine=docker; elif command -v podman > /dev/null; then engine=podman; else
    echo "install_sh_containers: neither docker nor podman" >&2
    exit 2
  fi
fi

work="$(mktemp -d "${TMPDIR:-/tmp}/install-sh-containers.XXXXXX")"
trap 'rm -rf "$work"' EXIT

# ---- Die Release-Dateien ---------------------------------------------------------

if [[ -n "${INSTALL_ASSETS:-}" ]]; then
  version="${INSTALL_VERSION:?INSTALL_VERSION is required with INSTALL_ASSETS}"
  assets="$(cd "$INSTALL_ASSETS" && pwd)"
else
  latest="$(curl -fsSLI --proto '=https' -o /dev/null -w '%{url_effective}' https://github.com/nurkert/Humanitl/releases/latest)"
  version="${latest##*/v}"
  assets="$work/assets"
  mkdir -p "$assets"
  for f in SHA256SUMS "humanitl_${version}_amd64.deb" "humanitl-${version}-1.x86_64.rpm" "humanitl-${version}-linux-x86_64.tar.gz"; do
    curl -fsSL --proto '=https' --retry 3 -o "$assets/$f" "https://github.com/nurkert/Humanitl/releases/download/v$version/$f"
  done
fi
rel="$work/releases/download/v$version"
mkdir -p "$rel"
cp "$assets/SHA256SUMS" "$assets/humanitl_${version}_amd64.deb" "$assets/humanitl-${version}-1.x86_64.rpm" \
  "$assets/humanitl-${version}-linux-x86_64.tar.gz" "$rel/"
(cd "$rel" && sha256sum -c --ignore-missing SHA256SUMS)
chmod -R a+rX "$work/releases"
echo "install_sh_containers: release $version, engine $engine"

# ---- Im Container ------------------------------------------------------------------

# Laeuft als root im Container: richtet ein, ruft install.sh als tester auf
# und prueft. $SYSTEM, $METHOD und $VERSION kommen von aussen.
cat > "$work/inner.sh" << 'INNER'
#!/bin/sh
set -eu
step() { printf '\n=== %s: %s\n' "$SYSTEM" "$*"; }
fail() {
  printf 'FAIL %s: %s\n' "$SYSTEM" "$*" >&2
  exit 1
}

step "prepare the container"
case $SYSTEM in
  debian | ubuntu | generic)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    pkgs="curl ca-certificates"
    [ "$SYSTEM" = generic ] || pkgs="$pkgs sudo apt-utils gnupg"
    # shellcheck disable=SC2086
    apt-get install -y -qq --no-install-recommends $pkgs > /dev/null
    ;;
  fedora) dnf -y -q install sudo util-linux > /dev/null ;;
  opensuse) zypper -n -q in curl tar gzip shadow util-linux > /dev/null ;;
  arch) ;;
esac
useradd -m tester
if command -v sudo > /dev/null; then
  echo 'tester ALL=(ALL) NOPASSWD: ALL' > /etc/sudoers.d/tester
  chmod 0440 /etc/sudoers.d/tester
fi

env_line="HUMANITL_RELEASES_URL=file:///releases"
if [ "$METHOD" = apt ]; then
  step "a signed local APT repository with the .deb of the release"
  mkdir -p /srv/apt/pool/main /srv/apt/dists/stable/main/binary-amd64
  cp "/releases/download/v$VERSION/humanitl_${VERSION}_amd64.deb" /srv/apt/pool/main/
  cd /srv/apt
  apt-ftparchive packages pool > dists/stable/main/binary-amd64/Packages
  apt-ftparchive -o APT::FTPArchive::Release::Suite=stable -o APT::FTPArchive::Release::Codename=stable \
    -o APT::FTPArchive::Release::Components=main -o APT::FTPArchive::Release::Architectures=amd64 \
    release dists/stable > dists/stable/Release
  GNUPGHOME=$(mktemp -d)
  export GNUPGHOME
  gpg --batch --quiet --passphrase '' --quick-gen-key 'Humanitl test repository' default default never
  gpg --batch --quiet --armor --export > /srv/apt/KEY.gpg
  gpg --batch --quiet --clearsign -o dists/stable/InRelease dists/stable/Release
  chmod -R a+rX /srv/apt
  cd /
  # install.sh braucht gnupg nicht: Es prueft den Schluessel gegen eine
  # SHA-256 und dekodiert ihn selbst. Ohne gpg im Container zeigt sich das.
  # Die Programme werden nur beiseitegelegt: Paketnamen und Anbieter von
  # `gpg` unterscheiden sich zwischen Debian und Ubuntu.
  for prog in gpg gpg2; do
    while hash -r && bin=$(command -v "$prog"); do mv "$bin" "$bin.hidden-for-install-test"; done
  done
  key_sha=$(sha256sum < /srv/apt/KEY.gpg | cut -d' ' -f1)
  env_line="$env_line HUMANITL_APT_URL=file:///srv/apt HUMANITL_APT_KEY_SHA256=$key_sha"
fi
if [ "$SYSTEM" = generic ]; then
  printf 'NAME="Example Linux"\nID=example\nPRETTY_NAME="Example Linux 1"\n' > /etc/example-os-release
  env_line="$env_line HUMANITL_OS_RELEASE=/etc/example-os-release"
fi

as_tester() { su -s /bin/sh tester -c "cd ~ && $env_line sh /src/install.sh $*"; }
installed_state() {
  {
    ls /usr/share/keyrings/nurkert-archive-keyring.gpg /etc/apt/sources.list.d/nurkert.list 2> /dev/null || :
    command -v humanitl || :
    ls -d /home/tester/.local/lib/humanitl /home/tester/.local/bin/humanitl 2> /dev/null || :
    ls -d /tmp/humanitl-install.* 2> /dev/null || :
    if command -v rpm > /dev/null; then rpm -q humanitl 2> /dev/null || :; fi
    if command -v dpkg-query > /dev/null; then dpkg-query -W humanitl 2> /dev/null || :; fi
  } | grep -v 'is not installed' || :
}

step "--dry-run changes nothing"
before=$(installed_state)
as_tester --dry-run --version "$VERSION" > /tmp/dry.out 2>&1 || {
  cat /tmp/dry.out
  fail "--dry-run failed"
}
cat /tmp/dry.out
grep -q 'Dry run: nothing was changed' /tmp/dry.out || fail "--dry-run did not say so"
after=$(installed_state)
[ "$before" = "$after" ] || fail "--dry-run changed something: $after"
[ -z "$after" ] || fail "the fresh container already has: $after"

case $METHOD in
  apt) want='Method:  APT repository' ;;
  rpm) want='Method:  .rpm package with dnf' ;;
  tarball) want='Method:  archive to /home/tester/.local' ;;
esac
grep -qF "$want" /tmp/dry.out || fail "expected method $METHOD ($want)"

for round in 1 2; do
  step "install, round $round"
  as_tester --yes --no-setup --version "$VERSION" || fail "install round $round failed"
done
[ -z "$(ls -d /tmp/humanitl-install.* 2> /dev/null)" ] || fail "temporary directory left behind"

step "humanitl --version"
case $METHOD in
  tarball) cli=/home/tester/.local/bin/humanitl ;;
  *) cli=humanitl ;;
esac
out=$(su -s /bin/sh tester -c "$cli --version") || fail "$cli --version failed: $out"
echo "$out"
case $out in
  *"$VERSION"*) ;;
  *) fail "$cli --version does not name $VERSION" ;;
esac
case $METHOD in
  apt)
    grep -qx "deb \[signed-by=/usr/share/keyrings/nurkert-archive-keyring.gpg\] file:///srv/apt stable main" \
      /etc/apt/sources.list.d/nurkert.list || fail "source line"
    [ "$(dpkg-query -W -f '${Version}' humanitl)" = "$VERSION" ] || fail "dpkg version"
    ;;
  rpm) rpm -q "humanitl-$VERSION" > /dev/null || fail "rpm -q" ;;
  tarball) [ -f /home/tester/.local/lib/humanitl/INSTALLED-FILES ] || fail "INSTALLED-FILES" ;;
esac
echo "ok   $SYSTEM ($METHOD)"
INNER

fails=()
for s in "${systems[@]}"; do
  echo "::group::install.sh on $s" 2> /dev/null || :
  if "$engine" run --rm --pull=missing \
    -e SYSTEM="$s" -e METHOD="${method[$s]}" -e VERSION="$version" \
    -v "$repo/install.sh:/src/install.sh:ro" \
    -v "$work/releases:/releases:ro" \
    -v "$work/inner.sh:/inner.sh:ro" \
    "${image[$s]}" sh /inner.sh; then
    echo "ok   $s"
  else
    echo "FAIL $s"
    fails+=("$s")
  fi
  echo "::endgroup::" 2> /dev/null || :
done

if [[ ${#fails[@]} -gt 0 ]]; then
  echo "install_sh_containers: red on ${fails[*]}"
  exit 1
fi
echo "install_sh_containers: green on ${systems[*]}"
