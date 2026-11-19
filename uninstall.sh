#!/bin/sh
# Humanitl uninstaller for Linux (x86_64) and macOS, the counterpart of
# https://humanitl.de/install.
#
#   curl -fsSL https://humanitl.de/uninstall | sh
#
# Read it first, then run it. Options go after `sh -s --`:
#
#   curl -fsSL https://humanitl.de/uninstall | sh -s -- --dry-run
#
# It finds how Humanitl is installed instead of guessing, and lists every
# finding: the package humanitl in dpkg or rpm, humanitl-bin or humanitl in
# pacman, the archive install.sh unpacked under ~/.local (or --prefix) with its
# list INSTALLED-FILES, Humanitl.app in /Applications or ~/Applications. When
# it finds nothing, it says so and changes nothing.
#
# Then, each step shown first and asked for:
#   1. If an agent session runs, it names it and stops. It never ends one;
#      you do that yourself, then run the script again.
#   2. `humanitl daemon uninstall` stops and removes the background service
#      (systemd user unit, or the launchd agent on macOS), while the program
#      is still there to do it.
#   3. The program goes the way it came: apt-get remove --purge (the package
#      only, nothing in your home), dnf remove, the AUR helper or pacman -R,
#      exactly the paths in INSTALLED-FILES, the app. sudo only for the
#      package step, and the command is printed first. With the APT package,
#      on a question of its own asked before that step, the source and the
#      key of apt.nurkert.de go in the same sudo call.
#
# The session check and the service step need the installed command line to
# run; when it does not, the script stops before changing anything and says
# how to make it run.
#
# Your data stays: configuration, recordings, the project register, the agent
# depot and the audit chain. A package manager does not touch your home
# either. Only --purge deletes that data, and only after it listed every
# location with its size and you said yes; without a terminal only together
# with --yes. The locations come from the program itself
# (`humanitl config paths`), never from a list in this script. Each is
# resolved to the directory it really is, before the list and again before
# deleting; a location that is or holds your home, or does not end in
# humanitl or Humanitl, stops the script. Project folders are never part of it.
#
# Options:
#   --yes, -y            answer every question with yes
#   --dry-run            show what would happen, change nothing
#   --method M           only this way: apt, rpm, aur, tarball or dmg
#   --prefix DIR         where the archive method installed (default ~/.local)
#   --purge              also delete your data, after listing it and asking
#   --help, -h           this text
#
# Environment, for tests:
#   HUMANITL_OS_RELEASE    os-release file, default /etc/os-release
#   HUMANITL_PACKAGE_BIN   where a package puts humanitl, default /usr/bin
#
# Source: https://github.com/nurkert/Humanitl/blob/main/uninstall.sh
# License: GPL-3.0-or-later
#
# The whole script is a set of functions; only the last line runs anything,
# so a download cut short executes nothing.

set -u

# ---- Shared by install.sh and uninstall.sh (HUM-550), kept equal by a test ---

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
Usage: curl -fsSL https://humanitl.de/uninstall | sh -s -- [options]

  --yes, -y          answer every question with yes
  --dry-run          show what would happen, change nothing
  --method M         only this way: apt, rpm, aur, tarball or dmg
  --prefix DIR       where the archive method installed (default ~/.local)
  --purge            also delete your data, after listing it and asking
  --help, -h         this text
EOF
}

parse_args() {
  opt_yes=0
  opt_dry=0
  opt_method=
  opt_prefix=
  opt_purge=0
  while [ $# -gt 0 ]; do
    case $1 in
      -y | --yes) opt_yes=1 ;;
      --dry-run) opt_dry=1 ;;
      --purge) opt_purge=1 ;;
      -h | --help)
        usage
        exit 0
        ;;
      --method | --prefix)
        [ $# -ge 2 ] || die "$1 needs a value"
        set_option "$1" "$2"
        shift
        ;;
      --method=* | --prefix=*) set_option "${1%%=*}" "${1#*=}" ;;
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
    --method)
      case $2 in
        apt | rpm | aur | tarball | dmg) opt_method=$2 ;;
        *) die "--method wants apt, rpm, aur, tarball or dmg, not '$2'" ;;
      esac
      ;;
    --prefix)
      case $2 in
        /*)
          opt_prefix=${2%/}
          [ -n "$opt_prefix" ] || die "--prefix / is no place install.sh puts an archive; name the directory, for example --prefix /opt/humanitl"
          ;;
        *) die "--prefix wants an absolute path, not '$2'" ;;
      esac
      ;;
  esac
}

# ---- What is installed ----------------------------------------------------------

# wanted <method>: whether --method leaves this way in.
wanted() { [ -z "$opt_method" ] || [ "$opt_method" = "$1" ]; }

# Every way Humanitl is installed here; only reads. Sets found_apt, found_rpm,
# found_aur (the package name), found_tarball (its directory) and found_dmg
# (one app per line).
detect() {
  found_apt=
  found_rpm=
  found_aur=
  found_tarball=
  found_dmg=
  if [ "$os" = Darwin ]; then
    if wanted dmg; then
      for dt_apps in /Applications "$HOME/Applications"; do
        [ -d "$dt_apps/Humanitl.app" ] || continue
        found_dmg="$found_dmg$dt_apps/Humanitl.app
"
      done
    fi
    return 0
  fi
  # shellcheck disable=SC2016 # ${Status} is a dpkg-query field, not a variable
  if wanted apt && command -v dpkg-query >/dev/null 2>&1 &&
    [ "$(dpkg-query -W -f '${Status}' humanitl 2>/dev/null </dev/null)" = "install ok installed" ]; then
    found_apt=humanitl
  fi
  if wanted rpm && command -v rpm >/dev/null 2>&1 && rpm -q humanitl </dev/null >/dev/null 2>&1; then
    found_rpm=humanitl
  fi
  if wanted aur && command -v pacman >/dev/null 2>&1; then
    for dt_pkg in humanitl-bin humanitl; do
      if pacman -Q "$dt_pkg" </dev/null >/dev/null 2>&1; then
        found_aur=$dt_pkg
        break
      fi
    done
  fi
  if wanted tarball && [ -f "$prefix/lib/humanitl/INSTALLED-FILES" ]; then
    found_tarball="$prefix/lib/humanitl"
  fi
}

found_any() {
  [ -n "$found_apt$found_rpm$found_aur$found_tarball$found_dmg" ]
}

# Reads INSTALLED-FILES and refuses anything install.sh never writes there:
# the directory itself and the three links in $prefix/bin. Sets tb_links.
check_tarball_list() {
  tb_links=
  tb_dir_listed=0
  while IFS= read -r tb_line || [ -n "$tb_line" ]; do
    case $tb_line in
      '' | '#'*) continue ;;
      "$found_tarball") tb_dir_listed=1 ;;
      "$prefix/bin/humanitl" | "$prefix/bin/humanitld" | "$prefix/bin/humanitl-app")
        tb_links="$tb_links$tb_line
"
        ;;
      *) refuse "$found_tarball/INSTALLED-FILES names $tb_line, which install.sh never writes there; remove that installation by hand." ;;
    esac
  done <"$found_tarball/INSTALLED-FILES"
  [ "$tb_dir_listed" = 1 ] || refuse "$found_tarball/INSTALLED-FILES does not name $found_tarball; remove that installation by hand."
}

check_apps() {
  [ -n "$found_dmg" ] || return 0
  while IFS= read -r ca_app; do
    [ -n "$ca_app" ] || continue
    [ -w "${ca_app%/*}" ] || refuse "${ca_app%/*} is not writable for you, so $ca_app cannot go from here; move it to the Trash in the Finder."
  done <<EOF
$found_dmg
EOF
}

# The command line of a found installation that runs, for the questions to
# the program and for `daemon uninstall`. Empty if none runs.
choose_cli() {
  cli=
  cc_list=
  [ -z "$found_apt$found_rpm$found_aur" ] || cc_list="$package_bin/humanitl
"
  [ -z "$found_tarball" ] || cc_list="$cc_list$found_tarball/bin/humanitl
"
  if [ -n "$found_dmg" ]; then
    while IFS= read -r cc_app; do
      [ -z "$cc_app" ] || cc_list="$cc_list$cc_app/Contents/MacOS/bin/humanitl
"
    done <<EOF
$found_dmg
EOF
  fi
  while IFS= read -r cc_cli; do
    [ -n "$cc_cli" ] || continue
    if [ -x "$cc_cli" ] && "$cc_cli" --version </dev/null >/dev/null 2>&1; then
      cli=$cc_cli
      return 0
    fi
    [ -n "$cli_blocked" ] || cli_blocked=$cc_cli
  done <<EOF
$cc_list
EOF
}

setup_root() {
  sudo=
  [ -n "$found_apt$found_rpm$found_aur" ] || return 0
  [ "$(id -u)" = 0 ] && return 0
  command -v sudo >/dev/null 2>&1 || die "the package step needs root, and sudo is not installed; run this as root"
  sudo=sudo
}

# ---- Questions to the program ------------------------------------------------------

# Stops when an agent session runs, and when that cannot be told. Asks
# `humanitl daemon status`, which says agent_session none, running or unknown;
# no daemon at all (DAEMON_001) means no session. Ends nothing.
check_sessions() {
  [ -n "$cli" ] || return 0
  cs_out=$("$cli" --json daemon status </dev/null 2>/dev/null)
  cs_rc=$?
  case $cs_out in
    *'"agent_session":"none"'*) return 0 ;;
    *'"agent_session":"running"'*)
      say "An agent session is running:"
      "$cli" sessions </dev/null 2>&1 | sed 's/^/  /'
      refuse "This script ends no session. End it yourself (leave the agent, or stop it in the app), then run this again."
      ;;
  esac
  if [ "$cs_rc" -ne 0 ]; then
    case $cs_out in
      *'"code":"DAEMON_001"'*) return 0 ;;
    esac
  fi
  refuse "Could not tell whether an agent session runs ($cli --json daemon status said: ${cs_out:-nothing}, exit $cs_rc). Make sure none runs; \`$cli daemon uninstall\` stops the service, then run this again."
}

# Asks the program where your data lies (`humanitl config paths`): a header
# line, then one line per location with its kind and its path. Sets data_list
# to "<kind> <path>" lines. Returns 1 when the program cannot say or says
# something else than exactly one row for each kind of this system; with
# --purge that stops the script before any change.
read_locations() {
  data_list=
  [ -n "$cli" ] || return 1
  rl_out=$("$cli" config paths </dev/null 2>/dev/null) || return 1
  if [ "$os" = Darwin ]; then rl_want="config data support logs"; else rl_want="config data"; fi
  rl_seen=" "
  rl_first=1
  while IFS= read -r rl_line; do
    if [ "$rl_first" = 1 ]; then
      rl_first=0
      case $rl_line in
        NAME*PATH) continue ;;
      esac
      locations_doubt "its first line is not the header NAME PATH"
      return 1
    fi
    [ -n "$rl_line" ] || continue
    rl_name=${rl_line%% *}
    rl_path=${rl_line#"$rl_name"}
    rl_path=${rl_path#"${rl_path%%[! ]*}"}
    case " $rl_want " in
      *" $rl_name "*) ;;
      *)
        locations_doubt "it names an unknown kind '$rl_name'"
        return 1
        ;;
    esac
    case $rl_seen in
      *" $rl_name "*)
        locations_doubt "it names the kind $rl_name twice"
        return 1
        ;;
    esac
    rl_seen="$rl_seen$rl_name "
    if ! valid_location "$rl_path"; then
      locations_doubt "it names '$rl_path' for $rl_name, and a data location is an absolute path ending in humanitl or Humanitl that neither is nor holds your home"
      return 1
    fi
    data_list="$data_list$rl_name $rl_path
"
  done <<EOF
$rl_out
EOF
  for rl_name in $rl_want; do
    case $rl_seen in
      *" $rl_name "*) ;;
      *)
        locations_doubt "it names no location of the kind $rl_name"
        return 1
        ;;
    esac
  done
}

# locations_doubt <why>: an answer of `config paths` this script does not
# trust. Without --purge the list is only left out; with it, nothing changes.
locations_doubt() {
  data_list=
  [ "$opt_purge" = 0 ] || refuse "--purge deletes only what humanitl config paths names, and its answer is not one this script deletes from: $1."
}

# A data location is absolute, ends in humanitl or Humanitl, and is neither
# /, nor your home, nor a directory that holds your home (also where your
# home really lies); whatever else the program might say is never deleted.
# Repeated slashes count as one (a TMPDIR or XDG value ending in / gives
# them), so the comparisons below see the same spelling for the same path.
valid_location() {
  vl_path=$(printf '%s' "$1" | tr -s /)
  case $vl_path in
    /*/humanitl | /*/Humanitl) ;;
    *) return 1 ;;
  esac
  case $vl_path in
    */../* | */./*) return 1 ;;
  esac
  for vl_home in "${HOME%/}" "$home_real"; do
    vl_home=$(printf '%s' "$vl_home" | tr -s /)
    case "${vl_home%/}/" in
      "$vl_path"/*) return 1 ;;
    esac
  done
  return 0
}

# resolve <kind> <path>: sets phys to the directory the path really is (every
# link followed), or to nothing when nothing is there; stops for anything this
# script never deletes, judged on that real directory.
resolve() {
  phys=
  [ -e "$2" ] || return 0
  [ -d "$2" ] || purge_stop "$2 ($1) is not a directory"
  phys=$(cd -P -- "$2" 2>/dev/null && pwd -P) || purge_stop "$2 ($1) cannot be resolved"
  valid_location "$phys" || purge_stop "$2 ($1) is really $phys, which this script does not delete: it does not end in humanitl or Humanitl, or it is or holds your home"
}

# resolve_all list|check: resolves every location. `list` keeps the result
# as phys_1, phys_2, ...; `check` stops when one now leads elsewhere.
resolve_all() {
  ra_i=0
  while IFS=' ' read -r ra_name ra_path; do
    [ -n "$ra_name" ] || continue
    ra_i=$((ra_i + 1))
    resolve "$ra_name" "$ra_path"
    if [ "$1" = list ]; then
      eval "phys_$ra_i=\$phys"
    else
      ra_listed=
      eval "ra_listed=\${phys_$ra_i:-}"
      [ "$phys" = "$ra_listed" ] || purge_stop "$ra_path ($ra_name) leads to ${phys:-nothing} now, not to ${ra_listed:-nothing} as listed"
    fi
  done <<EOF
$data_list
EOF
}

# purge_stop <why>: in the plan nothing has changed yet; for real the program
# steps before are done, but no data was deleted.
purge_stop() {
  [ "$phase" = run ] || refuse "--purge stops: $1."
  die "--purge stops: $1. None of your data was deleted."
}

# human_size <KiB>
human_size() {
  if [ "$1" -lt 1024 ]; then
    printf '%s KiB' "$1"
  elif [ "$1" -lt 1048576 ]; then
    printf '%s MiB' "$(($1 / 1024))"
  else
    printf '%s.%s GiB' "$(($1 / 1048576))" "$(($1 % 1048576 * 10 / 1048576))"
  fi
}

# Lists every data location with its size, one per line; with --purge also
# the real directory where it is another.
list_locations() {
  ll_i=0
  while IFS=' ' read -r ll_name ll_path; do
    [ -n "$ll_name" ] || continue
    ll_i=$((ll_i + 1))
    ll_phys=
    eval "ll_phys=\${phys_$ll_i:-}"
    ll_shown=$ll_path
    [ -z "$ll_phys" ] || [ "$ll_phys" = "$ll_path" ] || ll_shown="$ll_path, really $ll_phys"
    if [ -e "$ll_path" ]; then
      ll_kb=$(du -sk "${ll_phys:-$ll_path}" 2>/dev/null) || ll_kb=
      ll_kb=${ll_kb%%[!0-9]*}
      ll_size=$(human_size "${ll_kb:-0}")
    else
      ll_size="not there"
    fi
    printf '    %-8s %-10s %s\n' "$ll_name" "$ll_size" "$ll_shown"
  done <<EOF
$data_list
EOF
}

# ---- The steps. Each runs twice: once as the plan, once for real. ------------

# heading <text>: the number and title of a step.
heading() {
  step_no=$((step_no + 1))
  if [ "$phase" = plan ]; then
    say "  $step_no. $1"
  else
    say ""
    say "Step $step_no: $1"
  fi
}

# confirm <question>: in the plan nothing; for real, a no stops the script.
confirm() {
  [ "$phase" = run ] || return 0
  ask "$1" && return 0
  if [ "$changed" = 0 ]; then
    say "Stopped. Nothing was changed."
  else
    say "Stopped. The steps before this one are done; run this script again to finish."
  fi
  exit 1
}

step_sessions() {
  heading "No agent session may run; this script ends none."
  if [ "$phase" = plan ]; then
    printf '    %s\n' "$cli --json daemon status: no agent session runs"
    return 0
  fi
  check_sessions
  say "No agent session runs."
}

step_service() {
  heading "Stop and remove the background service, while humanitl is still there:"
  confirm "Remove the background service ($cli daemon uninstall)?"
  if ! try "$cli" daemon uninstall; then
    die "$cli daemon uninstall failed (see above). The program is still there; fix what it names and run this again."
  fi
  [ "$phase" = plan ] || changed=1
}

# The package, and the source and the key install.sh (and the script on
# apt.nurkert.de) put there. Other packages from apt.nurkert.de get no updates
# without them, so they have a question of their own, asked before the
# package step; a yes adds them to the same sudo call, so sudo runs only for
# the package step.
step_apt() {
  heading "Remove the APT package $found_apt and, on a question of its own, the source and the key of apt.nurkert.de:"
  sa_files="/etc/apt/sources.list.d/nurkert.list /usr/share/keyrings/nurkert-archive-keyring.gpg"
  if [ "$phase" = plan ]; then
    printf '    %s\n' "${sudo:+$sudo }apt-get remove --purge -y $found_apt" \
      "    with the source: the same call also runs rm -f $sa_files"
    return 0
  fi
  confirm "Remove the package $found_apt (apt-get remove --purge; your home stays as it is)?"
  if ask "Also remove the source and the key of apt.nurkert.de? Other packages from there would get no more updates."; then
    # shellcheck disable=SC2016 # $1 is expanded by the inner shell
    as_root sh -c 'apt-get remove --purge -y "$1" && rm -f /etc/apt/sources.list.d/nurkert.list /usr/share/keyrings/nurkert-archive-keyring.gpg' sh "$found_apt"
  else
    say "The source and the key stay."
    as_root apt-get remove --purge -y "$found_apt"
  fi
  changed=1
}

step_rpm() {
  heading "Remove the RPM package $found_rpm:"
  confirm "Remove the package $found_rpm?"
  # openSUSE removes with zypper even where dnf is installed as well.
  if [ "$(linux_family)" = suse ] && command -v zypper >/dev/null 2>&1; then
    as_root zypper --non-interactive remove "$found_rpm"
  elif command -v dnf >/dev/null 2>&1; then
    as_root dnf remove -y "$found_rpm"
  elif command -v zypper >/dev/null 2>&1; then
    as_root zypper --non-interactive remove "$found_rpm"
  else
    as_root rpm -e "$found_rpm"
  fi
  [ "$phase" = plan ] || changed=1
}

# Through the AUR helper that installed it, which calls sudo itself, or
# pacman -R where there is none (or as root, where helpers refuse to run).
step_aur() {
  helper=$(aur_helper) || helper=
  [ "$(id -u)" != 0 ] || helper=
  if [ -n "$helper" ]; then
    heading "Remove the package $found_aur with $helper:"
    confirm "Remove the package $found_aur ($helper -R)?"
    run "$helper" -R --noconfirm "$found_aur"
  else
    heading "Remove the package $found_aur with pacman:"
    confirm "Remove the package $found_aur (pacman -R)?"
    as_root pacman -R --noconfirm "$found_aur"
  fi
  [ "$phase" = plan ] || changed=1
}

# Exactly the paths in INSTALLED-FILES: the links first, and only when they
# still point into the directory, then the directory with the list itself.
step_tarball() {
  heading "Remove the archive installation in $prefix (the paths in INSTALLED-FILES):"
  confirm "Remove $found_tarball and its links in $prefix/bin?"
  while IFS= read -r st_link; do
    [ -n "$st_link" ] || continue
    if [ -L "$st_link" ]; then
      case $(readlink "$st_link" 2>/dev/null) in
        "$found_tarball"/*)
          run rm -f "$st_link"
          continue
          ;;
      esac
    fi
    if [ -e "$st_link" ] || [ -L "$st_link" ]; then
      [ "$phase" = plan ] || warn "$st_link no longer points into $found_tarball; it stays"
    fi
  done <<EOF
$tb_links
EOF
  run rm -rf "$found_tarball"
  [ "$phase" = plan ] || changed=1
}

step_dmg() {
  while IFS= read -r sd_app; do
    [ -n "$sd_app" ] || continue
    heading "Remove $sd_app:"
    confirm "Remove $sd_app?"
    run rm -rf "$sd_app"
    [ "$phase" = plan ] || changed=1
  done <<EOF
$found_dmg
EOF
}

# --purge: every location the program named, listed with its size, and only
# after a yes. Never a project folder: they are not in the list.
step_purge() {
  heading "Delete your data (--purge); project folders are never part of it:"
  if [ "$phase" = plan ]; then
    list_locations
    return 0
  fi
  list_locations
  if ! ask "Delete these locations for good?"; then
    say "Your data stays."
    return 0
  fi
  # Again right before deleting: every location must still lead where the
  # list said. Then the real directory goes, and a link that led there.
  resolve_all check
  sp_i=0
  while IFS=' ' read -r sp_name sp_path; do
    [ -n "$sp_name" ] || continue
    sp_i=$((sp_i + 1))
    sp_phys=
    eval "sp_phys=\${phys_$sp_i:-}"
    [ -n "$sp_phys" ] || continue
    run rm -rf "$sp_phys"
    [ ! -L "$sp_path" ] || run rm -f "$sp_path"
  done <<EOF
$data_list
EOF
  purged=1
}

steps() {
  step_no=0
  step_sessions
  step_service
  [ -z "$found_apt" ] || step_apt
  [ -z "$found_rpm" ] || step_rpm
  [ -z "$found_aur" ] || step_aur
  [ -z "$found_tarball" ] || step_tarball
  [ -z "$found_dmg" ] || step_dmg
  [ "$opt_purge" = 0 ] || step_purge
}

# ---- Before and after ----------------------------------------------------------

show_plan() {
  say "Humanitl uninstaller"
  say "  System:  $distro_name ($arch)"
  sp_label="Found:"
  [ -z "$found_apt" ] || { say "  $sp_label   APT package $found_apt (dpkg)"; sp_label="      "; }
  [ -z "$found_rpm" ] || { say "  $sp_label   RPM package $found_rpm"; sp_label="      "; }
  [ -z "$found_aur" ] || { say "  $sp_label   package $found_aur (pacman)"; sp_label="      "; }
  [ -z "$found_tarball" ] || { say "  $sp_label   archive in $found_tarball (from install.sh)"; sp_label="      "; }
  if [ -n "$found_dmg" ]; then
    while IFS= read -r sp_app; do
      [ -z "$sp_app" ] || { say "  $sp_label   $sp_app"; sp_label="      "; }
    done <<EOF
$found_dmg
EOF
  fi
  say "  Program: $cli ($("$cli" --version </dev/null 2>/dev/null || :))"
  say ""
  say "An access key for the model server in the keyring (Keychain on macOS) is not"
  say "touched; to remove it, run \`humanitl llm key remove\` before this script."
  say ""
  if [ -n "$sudo" ]; then
    say "Steps, each asked for (lines starting with sudo ask for your password):"
  else
    say "Steps, each asked for:"
  fi
}

# After the steps: what stays, and whether a humanitl is still in PATH.
after() {
  say ""
  if [ "$opt_purge" = 1 ] && [ "$purged" = 1 ]; then
    say "Humanitl and your data are removed. Project folders were not touched."
  else
    say "Humanitl is removed. Your data stays where it is."
    [ -z "$found_apt$found_rpm$found_aur" ] || say "The package manager does not touch your home either."
    if [ -n "$data_list" ]; then
      list_locations
      say "To delete it as well, run this script again with --purge while Humanitl is"
      say "installed, or remove these directories by hand."
    else
      say "Where it lies and how to delete it:"
      say "  https://github.com/nurkert/Humanitl/blob/main/docs/guides/install.md#your-data"
    fi
  fi
  hash -r 2>/dev/null || :
  if left=$(command -v humanitl 2>/dev/null); then
    warn "there is still a humanitl at $left; it did not come from a way this script knows (an AppImage, a build?)"
  fi
}

# Without a command line that runs, nothing could check for sessions or
# remove the service, and removing the program would leave the service
# registered (on macOS a launchd agent pointing into a deleted app). So
# nothing changes; the message says how to make it run.
cli_does_not_run() {
  if [ -z "$cli_blocked" ]; then
    refuse "Found Humanitl, but none of its command lines is there to ask about sessions and to remove the background service first. Reinstall it the same way, then run this again; or remove it by hand: https://github.com/nurkert/Humanitl/blob/main/docs/guides/install.md#uninstall"
  fi
  cd_why=$("$cli_blocked" --version </dev/null 2>&1 | head -n 3) || :
  if [ "$os" = Darwin ]; then
    refuse "$cli_blocked does not run (${cd_why:-no output}), so it cannot check for sessions or remove its launchd agent, and the app must not go before that. Open Humanitl once and allow it under System Settings > Privacy & Security > Open Anyway, then run this again."
  fi
  refuse "$cli_blocked does not run (${cd_why:-no output}), so it cannot check for sessions or remove its background service, and the program must not go before that. Repair it until \`$cli_blocked --version\` works (a package: install it again), then run this again; or remove it by hand: https://github.com/nurkert/Humanitl/blob/main/docs/guides/install.md#uninstall"
}

nothing_found() {
  if [ -n "$opt_method" ]; then
    refuse "Humanitl is not installed here by the way $opt_method."
  fi
  say "Found no Humanitl to remove: no package humanitl or humanitl-bin, no"
  say "$prefix/lib/humanitl/INSTALLED-FILES from install.sh, no Humanitl.app."
  if nf_path=$(command -v humanitl 2>/dev/null); then
    say "There is a humanitl at $nf_path, which none of these ways installed (an"
    say "AppImage, a build?); remove it the way it came."
  fi
  say "Nothing was changed."
  exit 0
}

main() {
  phase=plan
  sudo=
  helper=
  cli=
  cli_blocked=
  data_list=
  home_real=
  tb_links=
  changed=0
  purged=0
  parse_args "$@"
  detect_platform
  tty=0
  if have_tty; then
    tty=1
  elif [ "$opt_dry" = 0 ] && [ "$opt_yes" = 0 ]; then
    die "no terminal to ask on. Read the script, then run it with --yes, or try --dry-run first:
  curl -fsSL https://humanitl.de/uninstall | sh -s -- --yes"
  fi
  prefix=${opt_prefix:-${HOME:?HOME is not set}/.local}
  package_bin=${HUMANITL_PACKAGE_BIN:-/usr/bin}
  package_bin=${package_bin%/}
  if [ -n "$opt_prefix" ] && [ -n "$opt_method" ] && [ "$opt_method" != tarball ]; then
    die "--prefix applies to the archive method (tarball) only"
  fi
  if [ "$os" = Darwin ]; then
    [ -z "$opt_method" ] || [ "$opt_method" = dmg ] || die "--method $opt_method is for Linux; on macOS only dmg"
  else
    [ "$opt_method" != dmg ] || die "--method dmg is for macOS"
  fi

  detect
  found_any || nothing_found
  [ -z "$found_tarball" ] || check_tarball_list
  check_apps
  setup_root
  choose_cli
  [ -n "$cli" ] || cli_does_not_run
  # Before anything is shown or changed: a running session stops here.
  check_sessions
  home_real=$(cd -P -- "$HOME" 2>/dev/null && pwd -P) || home_real=${HOME%/}
  if [ "$opt_purge" = 1 ]; then
    read_locations || refuse "--purge needs the program to name your data locations (humanitl config paths), and $cli cannot; versions up to 0.4.1 do not know that command. Update Humanitl first, or run this without --purge and remove the data by hand."
    resolve_all list
  else
    read_locations || :
  fi

  show_plan
  steps
  if [ "$opt_dry" = 1 ]; then
    say ""
    say "Dry run: nothing was changed."
    exit 0
  fi
  phase=run
  steps
  after
}

main "$@"
