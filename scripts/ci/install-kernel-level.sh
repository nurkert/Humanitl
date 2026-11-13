#!/usr/bin/env bash
# Installiert auf dem CI-Runner die Pakete der Kernel-Stufe und einen
# Gastkernel für den Job kvm-tests (HUM-465, ADR-0021).
#
#   scripts/ci/install-kernel-level.sh DIR
#
# Ubuntu 24.04 bringt QEMU (8.2) und Python mit; es fehlen:
#
# - vhost-device-vsock: in Ubuntu 24.04 nicht paketiert. Debians eigenes
#   Binärpaket 0.3.0-2 (sid), libc6 >= 2.34.
# - virtiofsd mit --readonly: Ubuntus 1.10 kennt die Option nicht, die
#   Wurzel des Gasts wird aber nur lesbar ausgeliefert. Debians 1.13.2 aus
#   trixie, libc6 >= 2.39 wie Ubuntu 24.04.
# - bwrap mit --bind-fd: Ubuntus 0.9.0 kennt es nicht; das macht
#   install-bwrap-overlay.sh (0.12.0 aus trixie).
# - ein Gastkernel, der die Wurzel über virtiofs ohne initramfs einhängen
#   kann. Der Kernel des Runners (linux-azure) hat virtiofs als Modul
#   (CONFIG_VIRTIO_FS=m) und liegt mit 0600 unter /boot; mit ihm käme der Gast
#   nicht bis zum Shim. Debians Cloud-Kernel aus trixie hat virtiofs fest
#   eingebaut (CONFIG_VIRTIO_FS=y) und vsock als .ko.xz.
#
# Jedes Paket wird über snapshot.debian.org fest an seine SHA-1 gebunden und
# gegen eine SHA-256 geprüft, wie in install-bwrap-overlay.sh; das Paket von
# vhost-device-vsock kommt zuerst aus dem Pool von deb.debian.org. Nichts
# wird gebaut. Der Kernel wird nur ausgepackt, nicht installiert: DIR/vmlinuz
# und DIR/modules, für HUMANITL_TEST_VM_KERNEL und HUMANITL_TEST_VM_MODULES.
set -euo pipefail

dir="${1:?usage: scripts/ci/install-kernel-level.sh DIR}"

vsock_deb=vhost-device-vsock_0.3.0-2_amd64.deb
vsock_pool=https://deb.debian.org/debian/pool/main/r/rust-vhost-device-vsock/$vsock_deb
vsock_sha1=dc4ab55abad97f9d11650a377d4360f6eab9f1ef
vsock_sha256=fd36382419cc174f00d9b1691486b581e13e8c1168dc339b86b2a65180a29ce7

fs_deb=virtiofsd_1.13.2-1+deb13u1_amd64.deb
fs_sha1=d004574dc197da07d36e33c99875408cada2b9b3
fs_sha256=ff567a8816512252aea55a286772769f6d99fb8bc1ffcb110a9118cf44857829

kernel_deb=linux-image-6.12.115+deb13.1-cloud-amd64-unsigned_6.12.115-2_amd64.deb
kernel_release=6.12.115+deb13.1-cloud-amd64
kernel_sha1=8d16c3a98838db0beb7c26cfec25f246a93e29a6
kernel_sha256=60403d1deb5ac3854313fa2761f6269c55b1ca010bba0795516cc9a92327e48c

# fetch FILE SHA256 URL...: the first URL that answers with the right bytes.
fetch() {
  local file=$1 sha256=$2
  shift 2
  for url in "$@"; do
    if curl -fsSL --retry 3 -o "$file" "$url" && echo "$sha256  $file" | sha256sum -c --quiet -; then
      return 0
    fi
    echo "::warning::$url did not deliver $(basename "$file") with the pinned SHA-256" >&2
  done
  echo "::error::no source delivered $(basename "$file") with SHA-256 $sha256" >&2
  return 1
}

sudo apt-get update
sudo apt-get install -y qemu-system-x86 python3 libcap-ng0 libseccomp2 uidmap
./scripts/ci/install-bwrap-overlay.sh

work="$(mktemp -d)"
fetch "$work/$vsock_deb" "$vsock_sha256" "$vsock_pool" "https://snapshot.debian.org/file/$vsock_sha1"
fetch "$work/$fs_deb" "$fs_sha256" "https://snapshot.debian.org/file/$fs_sha1"
fetch "$work/$kernel_deb" "$kernel_sha256" "https://snapshot.debian.org/file/$kernel_sha1"
sudo dpkg -i "$work/$vsock_deb" "$work/$fs_deb"

mkdir -p "$dir"
dpkg-deb -x "$work/$kernel_deb" "$work/kernel"
cp "$work/kernel/boot/vmlinuz-$kernel_release" "$dir/vmlinuz"
cp -a "$work/kernel/usr/lib/modules/$kernel_release" "$dir/modules"
grep -qx 'CONFIG_VIRTIO_FS=y' "$work/kernel/boot/config-$kernel_release"
rm -rf "$work"

qemu-system-x86_64 --version | head -n 1
/usr/libexec/virtiofsd --version || true
vhost-device-vsock --version || true
bwrap --version
ls -l "$dir/vmlinuz"
find "$dir/modules" -name 'vsock.ko*'
