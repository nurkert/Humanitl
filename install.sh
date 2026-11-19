#!/bin/sh
# Humanitl installer for Linux (x86_64) and macOS.
#
#   curl -fsSL https://humanitl.de/install | sh
#
# Read it first, then run it. Options go after `sh -s --`:
#
#   curl -fsSL https://humanitl.de/install | sh -s -- --dry-run
#
# What it does, by system:
#   Debian, Ubuntu and derivatives  adds the APT repository apt.nurkert.de
#       (key to /usr/share/keyrings/, source to /etc/apt/sources.list.d/) and
#       installs the .deb of the release, checked against SHA256SUMS; later
#       versions come from the repository with apt upgrade. The key is checked
#       against the SHA-256 pinned in this script before it is used.
#   Fedora, RHEL, Rocky, Alma  downloads the .rpm of the release and installs
#       it with dnf.
#   Arch, Manjaro  installs the AUR package humanitl-bin through paru or yay,
#       if one of them is there and the package is published; otherwise the
#       archive.
#   openSUSE and any other Linux  unpacks the release archive (.tar.gz) to
#       ~/.local/lib/humanitl and links the programs into ~/.local/bin, without
#       root. The .rpm is not offered on openSUSE until a release check
#       confirms its dependency names there.
#   macOS  mounts the .dmg, copies Humanitl.app to /Applications (to
#       ~/Applications when /Applications is not writable) and marks the copy
#       as downloaded (com.apple.quarantine), so that Gatekeeper checks it
#       exactly as after a download in a browser. The script never removes or
#       bypasses that check. In a terminal it waits until you have allowed the
#       first start, then offers the setup; with --yes it names it as pending.
#   Windows, other systems, Linux on other architectures: a message, nothing
#       is changed.
#
# It runs as your user and calls sudo only for the package step (APT, dnf),
# printing the command first, and only after every file is downloaded and
# checked. Before it changes anything it shows every step and asks; without a
# terminal it stops unless --yes is given. Running it again replaces an
# existing installation of the same kind.
#
# Options:
#   --yes, -y            answer every question with yes
#   --dry-run            show what would happen, change nothing
#   --version X.Y.Z      install this release instead of the newest
#   --method M           apt, rpm, aur, tarball or dmg instead of the detected one
#   --prefix DIR         target of the archive method (default ~/.local)
#   --no-setup           do not offer `humanitl daemon install` and
#                        `humanitl sandbox check` afterwards
#   --help, -h           this text
#
# Checksums and signatures: every file comes over HTTPS from the GitHub
# release and is checked against the release's SHA256SUMS; a missing line or
# a wrong sum stops the script before anything is installed. Releases before
# 1.0.0 are not signed, so the checksum protects against a damaged download,
# not against a tampered release. Signatures come with 1.0.0; this script
# will then check them as well.
#
# Environment, for mirrors and tests:
#   HUMANITL_RELEASES_URL  releases base, default
#                          https://github.com/nurkert/Humanitl/releases
#                          (https:// or file://; files are read from
#                          <base>/download/v<version>/)
#   HUMANITL_APT_URL       APT repository, default https://apt.nurkert.de
#   HUMANITL_APT_KEY_SHA256  SHA-256 of that repository's KEY.gpg, default the
#                          pin in main(); only for a mirror with its own key
#   HUMANITL_OS_RELEASE    os-release file, default /etc/os-release
#
# Source: https://github.com/nurkert/Humanitl/blob/main/install.sh
# License: GPL-3.0-or-later
#
# The whole script is a set of functions; only the last line runs anything,
# so a download cut short executes nothing.

set -u

# ---- Shared with uninstall.sh (HUM-550): output, questions, platform -------

say() { printf '%s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}
# An unsupported system: say why, change nothing.
refuse() {
  printf '%s\n' "$*" "Nothing was changed." >&2
  exit 1
}

# Whether a terminal is there to ask on. The script itself usually arrives on
# stdin through a pipe, so questions go to /dev/tty, never to stdin.
have_tty() { (true </dev/tty) 2>/dev/null; }

# ask <question>: 0 for yes. --yes answers for the user and says so.
ask() {
  if [ "$opt_yes" = 1 ]; then
    say "$1 [y/N] y (--yes)"
    return 0
  fi
  printf '%s [y/N] ' "$1" >/dev/tty
  answer=
  read -r answer </dev/tty || answer=
  case $answer in
    [yY] | [yY][eE][sS]) return 0 ;;
  esac
  return 1
}

# try <command...>: in the plan only shown, otherwise shown and run, with its
# exit status. Its stdin is the terminal or nothing, never the pipe that
# carries this script.
try() {
  if [ "$phase" = plan ]; then
    printf '    %s\n' "$*"
    return 0
  fi
  printf '+ %s\n' "$*"
  if [ "$tty" = 1 ]; then
    "$@" </dev/tty
  else
    "$@" </dev/null
  fi
}

# run <command...>: like try, but a failure stops the script.
run() {
  try "$@" || die "this step failed: $*"
}

# as_root <command...>: the package step, through sudo unless already root.
as_root() {
  if [ -n "$sudo" ]; then
    run "$sudo" "$@"
  else
    run "$@"
  fi
}

unquote() {
  uq=$1
  uq=${uq#\"}
  uq=${uq%\"}
  uq=${uq#\'}
  uq=${uq%\'}
  printf '%s' "$uq"
}

# Reads ID, ID_LIKE and PRETTY_NAME without executing the file.
read_os_release() {
  osr=${HUMANITL_OS_RELEASE:-/etc/os-release}
  if [ ! -r "$osr" ] && [ -z "${HUMANITL_OS_RELEASE:-}" ]; then
    osr=/usr/lib/os-release
  fi
  [ -r "$osr" ] || return 0
  while IFS= read -r osr_line || [ -n "$osr_line" ]; do
    case $osr_line in
      ID=*) distro_id=$(unquote "${osr_line#ID=}") ;;
      ID_LIKE=*) distro_like=$(unquote "${osr_line#ID_LIKE=}") ;;
      PRETTY_NAME=*) distro_name=$(unquote "${osr_line#PRETTY_NAME=}") ;;
    esac
  done <"$osr"
}

detect_platform() {
  os=$(uname -s 2>/dev/null) || os=unknown
  arch=$(uname -m 2>/dev/null) || arch=unknown
  distro_id=
  distro_like=
  distro_name=
  case $os in
    Linux)
      case $arch in
        x86_64 | amd64) ;;
        *) refuse "Humanitl for Linux is built for x86_64 only; this machine is $arch." ;;
      esac
      read_os_release
      [ -n "$distro_name" ] || distro_name=${distro_id:-Linux}
      ;;
    Darwin)
      case $arch in
        arm64 | x86_64) ;;
        *) refuse "Humanitl for macOS runs on Apple Silicon and Intel; this machine is $arch." ;;
      esac
      distro_name="macOS $(sw_vers -productVersion 2>/dev/null || :)"
      ;;
    MINGW* | MSYS* | CYGWIN* | Windows*)
      refuse "Windows ($os) is not supported. Humanitl runs on Linux (x86_64) and macOS."
      ;;
    *)
      refuse "${os:-This system} is not supported. Humanitl runs on Linux (x86_64) and macOS."
      ;;
  esac
}

# The family of a Linux distribution from ID, then ID_LIKE: debian, fedora,
# arch, suse or other.
linux_family() {
  set -f
  for word in $distro_id $distro_like; do
    case $word in
      debian | ubuntu) family=debian ;;
      fedora | rhel | centos | rocky | almalinux) family=fedora ;;
      arch | archlinux | manjaro | endeavouros) family=arch ;;
      suse | opensuse | opensuse-* | sles) family=suse ;;
      *) continue ;;
    esac
    set +f
    printf '%s' "$family"
    return 0
  done
  set +f
  printf other
}

aur_helper() {
  for helper in paru yay; do
    if command -v "$helper" >/dev/null 2>&1; then
      printf '%s' "$helper"
      return 0
    fi
  done
  return 1
}

# ---- End of the shared part -------------------------------------------------

usage() {
  cat <<'EOF'
Usage: curl -fsSL https://humanitl.de/install | sh -s -- [options]

  --yes, -y          answer every question with yes
  --dry-run          show what would happen, change nothing
  --version X.Y.Z    install this release instead of the newest
  --method M         apt, rpm, aur, tarball or dmg instead of the detected one
  --prefix DIR       target of the archive method (default ~/.local)
  --no-setup         do not offer `humanitl daemon install` and
                     `humanitl sandbox check` afterwards
  --help, -h         this text
EOF
}

parse_args() {
  opt_yes=0
  opt_dry=0
  opt_version=
  opt_method=
  opt_prefix=
  opt_setup=1
  while [ $# -gt 0 ]; do
    case $1 in
      -y | --yes) opt_yes=1 ;;
      --dry-run) opt_dry=1 ;;
      --no-setup) opt_setup=0 ;;
      -h | --help)
        usage
        exit 0
        ;;
      --version | --method | --prefix)
        [ $# -ge 2 ] || die "$1 needs a value"
        set_option "$1" "$2"
        shift
        ;;
      --version=* | --method=* | --prefix=*) set_option "${1%%=*}" "${1#*=}" ;;
      *)
        usage >&2
        die "unknown option: $1"
        ;;
    esac
    shift
  done
}

set_option() {
  case $1 in
    --version)
      opt_version=${2#v}
      valid_version "$opt_version" || die "--version wants X.Y.Z, not '$2'"
      ;;
    --method)
      case $2 in
        apt | rpm | aur | tarball | dmg) opt_method=$2 ;;
        *) die "--method wants apt, rpm, aur, tarball or dmg, not '$2'" ;;
      esac
      ;;
    --prefix)
      case $2 in
        /*) opt_prefix=${2%/} ;;
        *) die "--prefix wants an absolute path, not '$2'" ;;
      esac
      ;;
  esac
}

valid_version() {
  case $1 in
    *[!0-9.]* | *.*.*.* | .* | *. | *..*) return 1 ;;
    *.*.*) return 0 ;;
  esac
  return 1
}

# Only HTTPS, or file:// for a local mirror or a test.
check_url() {
  case $1 in
    https://* | file://*) ;;
    *) die "refusing $1: downloads only over HTTPS" ;;
  esac
}

# fetch <url> <file>
fetch() {
  case $1 in
    https://*) curl -fsSL --proto '=https' --proto-redir '=https' --retry 3 --connect-timeout 20 -o "$2" "$1" ;;
    file://*) curl -fsSL --proto '=file' -o "$2" "$1" ;;
    *) die "refusing $1: downloads only over HTTPS" ;;
  esac
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sum_out=$(sha256sum <"$1") || return 1
  elif command -v shasum >/dev/null 2>&1; then
    sum_out=$(shasum -a 256 <"$1") || return 1
  elif command -v openssl >/dev/null 2>&1; then
    sum_out=$(openssl dgst -sha256 -r <"$1") || return 1
  else
    die "no tool for SHA-256 found (sha256sum, shasum or openssl)"
  fi
  printf '%s' "${sum_out%% *}"
}

# sum_for <asset> <SHA256SUMS>: the sum of exactly that file name, or nothing.
sum_for() {
  while IFS= read -r sums_line || [ -n "$sums_line" ]; do
    sums_hash=${sums_line%% *}
    sums_name=${sums_line#* }
    sums_name=${sums_name# }
    sums_name=${sums_name#\*}
    [ "$sums_name" = "$1" ] || continue
    case $sums_hash in
      *[!0-9a-f]*) continue ;;
    esac
    [ ${#sums_hash} -eq 64 ] || continue
    printf '%s' "$sums_hash"
    return 0
  done <"$2"
}

# fetch_verified <asset>: SHA256SUMS first, then the file; stops unless the
# file's sum matches its line exactly.
fetch_verified() {
  fv_base="$releases_url/download/v$version"
  if [ "$phase" = plan ]; then
    printf '    %s\n' "download $fv_base/$1" "check it against $fv_base/SHA256SUMS"
    return 0
  fi
  say "Downloading SHA256SUMS of $version"
  fetch "$fv_base/SHA256SUMS" "$tmp/SHA256SUMS" || die "could not download $fv_base/SHA256SUMS"
  fv_want=$(sum_for "$1" "$tmp/SHA256SUMS")
  [ -n "$fv_want" ] || die "SHA256SUMS of $version has no line for $1. Nothing was installed."
  say "Downloading $1"
  fetch "$fv_base/$1" "$tmp/$1" || die "could not download $fv_base/$1"
  fv_have=$(sha256_of "$tmp/$1") || die "could not compute the checksum of $1"
  if [ "$fv_have" != "$fv_want" ]; then
    rm -f "$tmp/$1"
    die "checksum mismatch for $1: SHA256SUMS says $fv_want, the download has $fv_have. Nothing was installed."
  fi
  say "Checksum matches SHA256SUMS: $1"
}

# dearmor <KEY.gpg> <keyring>: the base64 body of an armored OpenPGP key,
# decoded, which is what `gpg --dearmor` writes and what apt's signed-by reads
# from a .gpg file. Only the encoding changes; the input has been checked
# against the pinned SHA-256 before. No gnupg needed.
dearmor() {
  tr -d '\r' <"$1" | sed -e '1,/^[[:space:]]*$/d' -e '/^=/,$d' -e '/^-----END/,$d' | base64 -d >"$2" 2>/dev/null &&
    [ -s "$2" ]
}

need() {
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 || die "this needs '$tool', which is not installed"
  done
}

# glibc 2.39 or newer: the Linux builds come from Ubuntu 24.04
# (packaging/release/INSTALL.txt).
check_glibc() {
  libc=$(getconf GNU_LIBC_VERSION 2>/dev/null) || libc=
  case $libc in
    "glibc "*) libc=${libc#glibc } ;;
    *) refuse "Humanitl for Linux needs the GNU C library (glibc 2.39 or newer); this system has none (musl?)." ;;
  esac
  libc_major=${libc%%.*}
  libc_minor=${libc#*.}
  libc_minor=${libc_minor%%.*}
  case $libc_major$libc_minor in
    '' | *[!0-9]*) return 0 ;;
  esac
  if [ "$libc_major" -lt 2 ] || { [ "$libc_major" -eq 2 ] && [ "$libc_minor" -lt 39 ]; }; then
    refuse "glibc $libc is too old; Humanitl needs 2.39 or newer (Debian 13, Ubuntu 24.04, Fedora 40)."
  fi
}

choose_method() {
  if [ "$os" = Darwin ]; then
    [ -z "$opt_method" ] || [ "$opt_method" = dmg ] || die "--method $opt_method is for Linux; on macOS only dmg"
    method=dmg
    method_why="macOS"
    return 0
  fi
  [ "$opt_method" != dmg ] || die "--method dmg is for macOS"
  family=$(linux_family)
  helper=$(aur_helper) || helper=
  if [ -n "$opt_method" ]; then
    method=$opt_method
    method_why="--method $opt_method"
    case $method in
      apt) command -v apt-get >/dev/null 2>&1 || die "--method apt needs apt-get" ;;
      rpm) command -v dnf >/dev/null 2>&1 || die "--method rpm needs dnf" ;;
      aur)
        [ -n "$helper" ] || die "--method aur needs paru or yay"
        [ -z "$opt_version" ] || die "--version does not apply to the AUR package, which follows the newest release"
        [ "$(id -u)" != 0 ] || die "--method aur cannot run as root; makepkg refuses it"
        ;;
    esac
    return 0
  fi
  case $family in
    debian)
      if command -v apt-get >/dev/null 2>&1; then
        method=apt
        method_why="$distro_name uses APT"
      else
        method=tarball
        method_why="no apt-get found"
      fi
      ;;
    fedora)
      if command -v dnf >/dev/null 2>&1; then
        method=rpm
        method_why="$distro_name uses dnf"
      else
        method=tarball
        method_why="no dnf found"
      fi
      ;;
    arch)
      method=tarball
      if [ -z "$helper" ]; then
        method_why="no AUR helper (paru, yay) found"
      elif [ "$(id -u)" = 0 ]; then
        method_why="AUR helpers do not run as root"
      elif ! "$helper" -Si humanitl-bin </dev/null >/dev/null 2>&1; then
        method_why="humanitl-bin is not in the AUR (yet)"
      elif [ -n "$opt_version" ]; then
        method_why="--version pins a release; the AUR package follows the newest"
      else
        method=aur
        method_why="$helper finds humanitl-bin in the AUR"
      fi
      ;;
    suse)
      method=tarball
      method_why="the .rpm is not checked on openSUSE yet"
      ;;
    *)
      method=tarball
      method_why="no package for $distro_name"
      ;;
  esac
}

resolve_version() {
  version=$opt_version
  [ -z "$version" ] || return 0
  [ "$method" != aur ] || return 0
  case $releases_url in
    https://*) ;;
    *) die "with $releases_url as the releases base, --version is required" ;;
  esac
  latest=$(curl -fsSLI --proto '=https' --proto-redir '=https' --retry 3 --connect-timeout 20 \
    -o /dev/null -w '%{url_effective}' "$releases_url/latest") || latest=
  version=${latest##*/}
  version=${version#v}
  valid_version "$version" || die "could not find the newest release at $releases_url/latest; pass --version X.Y.Z"
}

setup_root() {
  sudo=
  case $method in
    apt | rpm) ;;
    *) return 0 ;;
  esac
  [ "$(id -u)" = 0 ] && return 0
  command -v sudo >/dev/null 2>&1 || die "the package step needs root, and sudo is not installed; run this as root"
  sudo=sudo
}

# ---- The methods. Each runs twice: once as the plan, once for real. --------

# Everything is downloaded and checked first, still without root: the key
# against its pinned SHA-256, the .deb of the release against SHA256SUMS. Only
# then come the steps with sudo. The first installation is exactly that .deb;
# later versions arrive from the repository with apt upgrade.
steps_apt() {
  apt_keyring=/usr/share/keyrings/nurkert-archive-keyring.gpg
  apt_list=/etc/apt/sources.list.d/nurkert.list
  apt_line="deb [signed-by=$apt_keyring] $apt_url stable main"
  apt_deb="humanitl_${version}_amd64.deb"
  if [ "$phase" = plan ]; then
    printf '    %s\n' "download $apt_url/KEY.gpg, check its SHA-256 against the pin in this script"
  else
    fetch "$apt_url/KEY.gpg" "$tmp/KEY.gpg" || die "could not download $apt_url/KEY.gpg"
    apt_key_have=$(sha256_of "$tmp/KEY.gpg") || die "could not compute the checksum of KEY.gpg"
    if [ "$apt_key_have" != "$apt_key_sha256" ]; then
      die "the key from $apt_url/KEY.gpg has SHA-256 $apt_key_have, but this script expects $apt_key_sha256. Nothing was installed. If the repository changed its key on purpose, a new version of this script names the new one."
    fi
    dearmor "$tmp/KEY.gpg" "$tmp/keyring.gpg" || die "could not decode $apt_url/KEY.gpg"
    say "Key matches the pin: $apt_url/KEY.gpg"
  fi
  fetch_verified "$apt_deb"
  # apt reads a local package as its own user _apt; the temporary directory is
  # private to you until here.
  run chmod 0755 "$tmp"
  run chmod 0644 "$tmp/$apt_deb"
  if [ "$phase" = run ] && [ -f "$apt_keyring" ] &&
    [ "$(sha256_of "$apt_keyring")" = "$(sha256_of "$tmp/keyring.gpg")" ]; then
    say "The key is already in $apt_keyring"
  else
    [ -d /usr/share/keyrings ] || as_root mkdir -p /usr/share/keyrings
    as_root install -m 0644 "$tmp/keyring.gpg" "$apt_keyring"
  fi
  if [ -f "$apt_list" ] && grep -qxF "$apt_line" "$apt_list" 2>/dev/null; then
    [ "$phase" = plan ] || say "The source is already in $apt_list"
  else
    [ "$phase" = plan ] || printf '%s\n' "$apt_line" >"$tmp/nurkert.list"
    as_root install -m 0644 "$tmp/nurkert.list" "$apt_list"
  fi
  as_root apt-get update
  as_root apt-get install -y "$tmp/$apt_deb"
  cli=/usr/bin/humanitl
}

steps_rpm() {
  rpm_asset="humanitl-$version-1.x86_64.rpm"
  fetch_verified "$rpm_asset"
  as_root dnf install -y "$tmp/$rpm_asset"
  cli=/usr/bin/humanitl
}

steps_aur() {
  aur_flags=
  if [ "$opt_yes" = 1 ]; then
    case $helper in
      paru) aur_flags="--noconfirm --skipreview" ;;
      yay) aur_flags="--noconfirm --answerdiff None --answerclean None --answeredit None" ;;
    esac
  fi
  if [ "$phase" = plan ] && [ "$opt_yes" = 1 ]; then
    printf '    %s\n' "(--yes: $helper installs without asking and skips the review of the PKGBUILD)"
  fi
  # shellcheck disable=SC2086 # the flags are separate words on purpose
  run "$helper" -S $aur_flags humanitl-bin
  cli=/usr/bin/humanitl
}

# The archive is unpacked into a fresh directory next to its target and then
# renamed into place, so a failure leaves the previous installation as it was:
# the old one is moved aside, and moved back if the new one cannot take its
# place. Only directories this run created are removed. INSTALLED-FILES lists
# what uninstall.sh (HUM-550) may remove.
steps_tarball() {
  tb_name="humanitl-$version-linux-x86_64"
  tb_asset="$tb_name.tar.gz"
  tb_dest="$prefix/lib/humanitl"
  fetch_verified "$tb_asset"
  run mkdir -p "$prefix/lib" "$prefix/bin"
  stage_dir "$prefix/lib/.humanitl-new"
  staged_new=$stage_path
  run tar -xzf "$tmp/$tb_asset" -C "$staged_new"
  if [ "$phase" = run ] && [ ! -x "$staged_new/$tb_name/bin/humanitl" ]; then
    die "the archive does not contain $tb_name/bin/humanitl; nothing was replaced"
  fi
  replace "$staged_new/$tb_name" "$tb_dest" "$prefix/lib/.humanitl-old"
  run ln -sfn "$tb_dest/bin/humanitl" "$prefix/bin/humanitl"
  run ln -sfn "$tb_dest/bin/humanitld" "$prefix/bin/humanitld"
  run ln -sfn "$tb_dest/app/humanitl" "$prefix/bin/humanitl-app"
  if [ "$phase" = plan ]; then
    printf '    %s\n' "write $tb_dest/INSTALLED-FILES"
  else
    printf '%s\n' "# Written by install.sh; uninstall.sh removes exactly these paths." \
      "$tb_dest" "$prefix/bin/humanitl" "$prefix/bin/humanitld" "$prefix/bin/humanitl-app" \
      >"$tb_dest/INSTALLED-FILES"
  fi
  cli="$prefix/bin/humanitl"
}

# stage_dir <base>: a new, empty directory <base>.XXXXXX next to the target,
# on the same file system, so the final rename is atomic. Sets stage_path.
stage_dir() {
  if [ "$phase" = plan ]; then
    printf '    %s\n' "mktemp -d $1.XXXXXX"
    stage_path="$1.XXXXXX"
    return 0
  fi
  printf '+ %s\n' "mktemp -d $1.XXXXXX"
  stage_path=$(mktemp -d "$1.XXXXXX") || die "could not create a directory next to $1"
}

# replace <new> <target> <old-base>: <new> takes the place of <target>. An
# existing <target> waits in a fresh <old-base>.XXXXXX and goes back if the
# rename fails; it is removed only after the new one is in place.
replace() {
  staged_old=
  if [ -e "$2" ]; then
    stage_dir "$3"
    staged_old=$stage_path
    run mv "$2" "$staged_old/"
  fi
  if ! try mv "$1" "$2"; then
    restore_old "$2"
    die "could not move the new version to $2; the previous one is back in place"
  fi
  if [ -n "$staged_old" ]; then
    run rm -rf "$staged_old"
  fi
  staged_old=
  [ -z "$staged_new" ] || run rm -rf "$staged_new"
  staged_new=
}

# restore_old <target>: the waiting previous installation goes back, if any.
restore_old() {
  [ "$phase" = run ] && [ -n "${staged_old:-}" ] || return 0
  for ro_item in "$staged_old"/*; do
    [ -e "$ro_item" ] || continue
    [ -e "$1" ] || mv "$ro_item" "$1" || warn "could not move $ro_item back to $1"
  done
  rmdir "$staged_old" 2>/dev/null || :
  staged_old=
}

# Refuses to overwrite what this script did not put there.
check_tarball_target() {
  tb_dest="$prefix/lib/humanitl"
  if [ -e "$tb_dest" ] && [ ! -f "$tb_dest/INSTALLED-FILES" ]; then
    die "$tb_dest exists but was not installed by this script; move it away first"
  fi
  for tb_link in humanitl humanitld humanitl-app; do
    tb_path="$prefix/bin/$tb_link"
    if [ -e "$tb_path" ] || [ -L "$tb_path" ]; then
      case $(readlink "$tb_path" 2>/dev/null) in
        "$tb_dest"/*) ;;
        *) die "$tb_path exists and does not point into $tb_dest; move it away first" ;;
      esac
    fi
  done
}

# The app is copied to a fresh hidden directory, marked as downloaded, and
# only then renamed into place: there is never an unmarked Humanitl.app.
steps_dmg() {
  dmg_asset="Humanitl-$version-macos.dmg"
  fetch_verified "$dmg_asset"
  [ "$apps" = /Applications ] || run mkdir -p "$apps"
  run mkdir -p "$tmp/mnt"
  run hdiutil attach -nobrowse -readonly -noautoopen -mountpoint "$tmp/mnt" "$tmp/$dmg_asset"
  mounted=1
  if [ "$phase" = run ] && [ ! -d "$tmp/mnt/Humanitl.app" ]; then
    die "the disk image does not contain Humanitl.app"
  fi
  stage_dir "$apps/.Humanitl.app.new"
  staged_new=$stage_path
  run ditto "$tmp/mnt/Humanitl.app" "$staged_new/Humanitl.app"
  detach_image
  # Gatekeeper checks an app on first start only when it carries this
  # attribute. A browser sets it on every download, curl does not; it is set
  # here the same way (flags;time;agent;), on every file of the bundle as when
  # copied from a downloaded disk image.
  if [ "$phase" = plan ]; then
    quarantine="0081;<time>;humanitl-install;"
  else
    quarantine="0081;$(printf '%x' "$(date +%s)");humanitl-install;"
  fi
  run xattr -r -w com.apple.quarantine "$quarantine" "$staged_new/Humanitl.app"
  replace "$staged_new/Humanitl.app" "$app" "$apps/.Humanitl.app.old"
  cli="$app/Contents/MacOS/bin/humanitl"
}

# Spotlight or another reader may still hold the freshly copied image for a
# moment, and hdiutil detach then fails with "Resource busy". The image is
# read-only and ours, so after two polite tries it is detached with -force.
detach_image() {
  if try hdiutil detach "$tmp/mnt"; then
    mounted=0
    return 0
  fi
  sleep 2
  if try hdiutil detach "$tmp/mnt"; then
    mounted=0
    return 0
  fi
  sleep 2
  run hdiutil detach -force "$tmp/mnt"
  mounted=0
}

# ---- Before and after ----------------------------------------------------------

existing() {
  case $method in
    dmg)
      [ -d "$app" ] || return 0
      ex_version=$(defaults read "$app/Contents/Info" CFBundleShortVersionString 2>/dev/null) || ex_version=
      say "Already installed: $app ${ex_version:+($ex_version)}; it will be replaced."
      ;;
    tarball)
      [ -d "$prefix/lib/humanitl" ] || return 0
      say "Already installed: $prefix/lib/humanitl; it will be replaced."
      ;;
    *)
      command -v humanitl >/dev/null 2>&1 || return 0
      ex_version=$(humanitl --version </dev/null 2>/dev/null) || ex_version=
      say "Already installed: $(command -v humanitl) ${ex_version:+($ex_version)}; the package step replaces or upgrades it."
      ;;
  esac
}

show_plan() {
  say "Humanitl installer"
  say "  System:  $distro_name ($arch)"
  case $method in
    apt) say "  Method:  APT repository $apt_url ($method_why)" ;;
    rpm) say "  Method:  .rpm package with dnf ($method_why)" ;;
    aur) say "  Method:  AUR package humanitl-bin with $helper ($method_why)" ;;
    tarball) say "  Method:  archive to $prefix, without root ($method_why)" ;;
    dmg) say "  Method:  Humanitl.app to $apps" ;;
  esac
  say "  Version: ${version:-the newest one in the package source}"
  existing
  say ""
  case $method in
    apt)
      say "The key of $apt_url is pinned in this script (SHA-256 of KEY.gpg); APT checks"
      say "the repository's signature with it. The first installation is the .deb of the"
      say "release, checked against SHA256SUMS; releases before 1.0.0 are not signed, so"
      say "that checksum protects against a damaged download, not a tampered release."
      ;;
    aur) say "makepkg checks the release archive against the SHA-256 sums in the PKGBUILD." ;;
    *)
      say "Releases before 1.0.0 are not signed. The checksum from SHA256SUMS protects"
      say "against a damaged download, not against a tampered release."
      ;;
  esac
  if [ "$method" = tarball ] && [ "$(id -u)" = 0 ]; then
    say ""
    warn "running as root: the archive goes to $prefix and would serve root, not you."
    warn "Better run this as your own user; as root, call it by its full path $prefix/bin/humanitl."
  fi
  say ""
  if [ -n "$sudo" ]; then
    say "Steps (lines starting with sudo ask for your password):"
  else
    say "Steps:"
  fi
}

after_install() {
  say ""
  if [ "$method" = dmg ]; then
    after_install_macos
    return 0
  fi
  if [ "$method" != tarball ]; then
    # The package puts humanitl in /usr/bin; the one your PATH finds is the one
    # you will call, so that one is checked, and a different one is named.
    cli_path=$(command -v humanitl 2>/dev/null) || cli_path=
    if [ -n "$cli_path" ] && [ "$cli_path" != "$cli" ]; then
      warn "$cli_path comes before $cli in your PATH"
      cli=$cli_path
    fi
  fi
  if installed=$("$cli" --version </dev/null 2>&1); then
    say "Installed: $installed ($cli)"
  else
    warn "$cli --version failed: $installed"
  fi
  if [ "$method" = tarball ]; then
    case ":$PATH:" in
      *":$prefix/bin:"*) ;;
      *) say "Add $prefix/bin to your PATH to call humanitl by name." ;;
    esac
    if ! command -v bwrap >/dev/null 2>&1; then
      say "The sandbox needs bubblewrap (bwrap), which is not installed; install the"
      say "package bubblewrap of your distribution."
    fi
  fi
  if [ "$(id -u)" = 0 ]; then
    say "Running as root: set up the service as your own user, not as root:"
    say "  humanitl daemon install"
    say "  humanitl sandbox check"
    return 0
  fi
  if [ "$opt_setup" = 0 ]; then
    say "Next: humanitl daemon install, then humanitl sandbox check."
    return 0
  fi
  offer_setup
}

offer_setup() {
  if ask "Set up the background service now (humanitl daemon install)?"; then
    try "$cli" daemon install || warn "humanitl daemon install failed; the program itself is installed"
  fi
  if ask "Check the sandbox now (humanitl sandbox check)?"; then
    try "$cli" sandbox check || warn "humanitl sandbox check found a problem; humanitl doctor names the fix"
  fi
}

# macOS refuses the first start of an app that is not notarized, as after a
# download in a browser. The person allows it once; only then can the command
# line inside the app run, and only then is setup offered.
after_install_macos() {
  say "Humanitl.app is in $apps."
  say ""
  say "Releases before 1.0.0 are not signed with an Apple Developer ID and not"
  say "notarized, so macOS refuses the first start, exactly as after a download in"
  say "a browser. To allow it once: open Humanitl, then System Settings >"
  say "Privacy & Security > \"Open Anyway\" (German: \"Dennoch öffnen\")."
  say "This script does not remove that check."
  if [ "$tty" = 1 ] && [ "$opt_yes" = 0 ] && [ "$opt_setup" = 1 ]; then
    say ""
    if ask "Open Humanitl now?"; then
      try open "$app" || warn "could not open $app; open it from $apps"
    fi
    for mac_round in 1 2; do
      printf '%s' "Once macOS lets Humanitl start, press Enter. " >/dev/tty
      mac_enter=
      read -r mac_enter </dev/tty || :
      : "$mac_enter"
      if "$cli" --version </dev/null >/dev/null 2>&1; then
        say "Installed: $("$cli" --version </dev/null 2>&1) ($cli)"
        offer_setup
        return 0
      fi
      [ "$mac_round" = 2 ] || say "macOS still blocks the command line $cli. Allow it the same way under Privacy & Security, then press Enter again."
    done
  fi
  say ""
  say "Setup is pending. Once macOS lets Humanitl start, set up the background"
  say "service in the app (\"Install and start\") or in a terminal:"
  say "  \"$cli\" daemon install"
  say "  \"$cli\" sandbox check"
}

cleanup() {
  if [ "$phase" = run ]; then
    if [ -n "${staged_old:-}" ]; then
      case $method in
        dmg) restore_old "$app" ;;
        tarball) restore_old "$prefix/lib/humanitl" ;;
      esac
    fi
    if [ -n "${staged_new:-}" ] && [ -d "$staged_new" ]; then
      rm -rf "$staged_new"
    fi
  fi
  if [ "${mounted:-0}" = 1 ]; then
    hdiutil detach -quiet "$tmp/mnt" >/dev/null 2>&1 || :
  fi
  if [ -n "${tmp:-}" ] && [ -d "$tmp" ]; then
    rm -rf "$tmp"
  fi
}

main() {
  phase=plan
  tmp=
  mounted=0
  sudo=
  helper=
  apps=
  app=
  cli=
  staged_new=
  staged_old=
  parse_args "$@"
  detect_platform
  tty=0
  if have_tty; then
    tty=1
  elif [ "$opt_dry" = 0 ] && [ "$opt_yes" = 0 ]; then
    die "no terminal to ask on. Read the script, then run it with --yes, or try --dry-run first:
  curl -fsSL https://humanitl.de/install | sh -s -- --yes"
  fi

  releases_url=${HUMANITL_RELEASES_URL:-https://github.com/nurkert/Humanitl/releases}
  releases_url=${releases_url%/}
  apt_url=${HUMANITL_APT_URL:-https://apt.nurkert.de}
  apt_url=${apt_url%/}
  # SHA-256 of https://apt.nurkert.de/KEY.gpg, the key "Nurkert APT
  # <apt@nurkert.de>", fingerprint 0FE4 E9CF DD78 E971 5BBF  01F6 454A D85D A868 B52A.
  # When that key changes: check the new fingerprint with
  # `curl -fsSL https://apt.nurkert.de/KEY.gpg | gpg --show-keys`, put the
  # output of `curl -fsSL https://apt.nurkert.de/KEY.gpg | sha256sum` here and
  # publish the website; until then this script refuses the new key.
  apt_key_sha256=${HUMANITL_APT_KEY_SHA256:-494ddc464b098c3e76a940516d4831adfc3d046440c93dd16bdde0f4c85d1489}
  check_url "$releases_url"
  check_url "$apt_url"
  prefix=${opt_prefix:-${HOME:?HOME is not set}/.local}

  choose_method
  [ -z "$opt_prefix" ] || [ "$method" = tarball ] || die "--prefix applies to the archive method (tarball) only"
  case $method in
    dmg)
      need curl hdiutil ditto xattr mktemp
      if [ -w /Applications ]; then apps=/Applications; else apps="$HOME/Applications"; fi
      app="$apps/Humanitl.app"
      ;;
    tarball)
      check_glibc
      need curl tar gzip readlink
      check_tarball_target
      ;;
    aur) ;;
    apt)
      check_glibc
      need curl tr sed base64
      ;;
    *)
      check_glibc
      need curl
      ;;
  esac
  setup_root
  resolve_version

  show_plan
  # The plan names the temporary directory before it exists.
  # shellcheck disable=SC2016
  tmp='$TMPDIR/humanitl-install.XXXXXX'
  "steps_$method"
  say ""
  if [ "$opt_dry" = 1 ]; then
    say "Dry run: nothing was changed."
    exit 0
  fi
  ask "Go ahead?" || {
    say "Stopped. Nothing was changed."
    exit 1
  }

  trap cleanup EXIT
  trap 'exit 130' INT TERM
  tmp=$(mktemp -d "${TMPDIR:-/tmp}/humanitl-install.XXXXXX") || die "could not create a temporary directory"
  phase=run
  "steps_$method"
  after_install
}

main "$@"
