#!/usr/bin/env bash
# Test fuer install.sh (HUM-528) ohne Container und ohne root.
#
# Das Skript laeuft hier gegen nachgestellte Werkzeuge: `uname`, `sudo`,
# `apt-get`, `dnf`, `yay`, `getconf`, `humanitl`, `mv` und fuer den macOS-Weg
# `hdiutil`, `ditto`, `xattr`, `open`, `defaults` liegen als Stubs vorn im
# PATH und schreiben jeden Aufruf in ein Protokoll. `sudo` fuehrt nichts aus;
# bei `sudo install` legt es nur eine Kopie der Quelldatei ab, damit der Test
# sieht, was installiert wuerde. Die Release-Dateien liegen in einem
# Verzeichnis, das install.sh ueber HUMANITL_RELEASES_URL=file://... liest,
# das APT-Repository ueber HUMANITL_APT_URL, sein Schluessel ist ueber
# HUMANITL_APT_KEY_SHA256 festgelegt, /etc/os-release kommt aus
# HUMANITL_OS_RELEASE. HOME und TMPDIR zeigen in einen Wegwerf-Baum. Echt
# installiert wird nur der Archiv-Weg und der nachgestellte macOS-Weg in
# diesen Baum.
#
# Ohne Terminal laeuft jeder Fall unter `setsid`; Faelle mit Rueckfragen
# laufen unter `script`, das ein Terminal stellt und die Antworten tippt.
#
# Geprueft wird:
#   - shellcheck, `sh -n` mit dash und busybox sh
#   - kein Entfernen der Quarantaene und kein Abschalten von Gatekeeper im
#     Quelltext (xattr -d/-c, spctl --master-disable/--global-disable/
#     --disable/--add, defaults write ... LSQuarantine, lsregister), aber das
#     Setzen von com.apple.quarantine
#   - aarch64-Linux, Windows (MINGW, MSYS, CYGWIN), ein unbekanntes System,
#     musl und ein zu altes glibc: Meldung, nichts geaendert
#   - ohne Terminal und ohne --yes: Abbruch; Antwort `n`: Abbruch; beides
#     ohne Aenderung
#   - --dry-run auf jedem Weg: nichts geaendert, kein Paketbefehl
#   - APT: Schluessel gegen die Festlegung und .deb gegen SHA256SUMS geprueft,
#     bevor das erste sudo laeuft; installiert wird das gepruefte .deb, der
#     Schluessel wie `gpg --dearmor` ihn schriebe
#   - Archiv-Weg: installieren, ersetzen, --prefix, --version=X, Zeilen mit
#     `*name` in SHA256SUMS, Rueckfragen zu daemon install und sandbox check,
#     Rueckbau, wenn das Umbenennen scheitert
#   - macOS-Weg (nachgestellt): Markierung auf der Kopie vor dem Umbenennen;
#     mit --yes "Setup is pending"; im Terminal erst nach Enter und erst,
#     wenn das Programm startet, die Einrichtung; sonst wieder "pending"
#   - falsche Pruefsumme und fehlende Zeile in SHA256SUMS: Abbruch vor jeder
#     Installation, auf Archiv-, RPM- und APT-Weg
#   - Mutationsbeweise: ohne die Pruefung der Pruefsumme werden alle sechs
#     Pruefsummen-Faelle rot, ohne die Pruefung des Schluessels der Fall
#     "falscher Schluessel", mit `xattr -dr` die Quelltextpruefung.
#
# Umgebung:
#   INSTALL_SH=<pfad>         prueft eine andere Fassung des Skripts
#   INSTALL_TEST_STRICT=1     shellcheck, busybox und gpg muessen da sein (CI);
#                             sonst wird ihr Fehlen nur gemeldet
# Die Muster und Mutanten unten tragen $-Zeichen woertlich.
# shellcheck disable=SC2016
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
script="${INSTALL_SH:-$here/../../install.sh}"
script="$(cd "$(dirname "$script")" && pwd)/$(basename "$script")"
strict="${INSTALL_TEST_STRICT:-0}"

work="$(mktemp -d "${TMPDIR:-/tmp}/install-sh-test.XXXXXX")"
fails=0
cleanup() {
  if [[ "$fails" -eq 0 ]]; then
    rm -rf "$work"
  else
    echo "install_sh_test: Baum bleibt in $work"
  fi
}
trap cleanup EXIT

for tool in setsid script; do
  command -v "$tool" > /dev/null || { echo "install_sh_test: $tool fehlt (util-linux)" >&2; exit 2; }
done

ok() { echo "ok   $1"; }
bad() {
  echo "FAIL $1: $2"
  fails=$((fails + 1))
}

# ---- Werkzeuge ---------------------------------------------------------------

missing_tool() {
  if [[ "$strict" == 1 ]]; then
    bad "$1" "$2 fehlt (INSTALL_TEST_STRICT=1)"
  else
    echo "skip $1: $2 fehlt, die CI prueft es"
  fi
}

if command -v shellcheck > /dev/null; then
  if out="$(shellcheck -s sh "$script" 2>&1)"; then ok shellcheck; else bad shellcheck "$out"; fi
else
  missing_tool shellcheck shellcheck
fi
for shell in dash "busybox sh"; do
  if command -v "${shell%% *}" > /dev/null; then
    # shellcheck disable=SC2086 # "busybox sh" sind zwei Woerter
    if out="$($shell -n "$script" 2>&1)"; then ok "$shell -n"; else bad "$shell -n" "$out"; fi
  else
    missing_tool "$shell -n" "${shell%% *}"
  fi
done

# ---- Quelltext: die Quarantaene wird gesetzt, nie entfernt ------------------

# Meldet jede Zeile, die die Quarantaene entfernt oder Gatekeeper abschaltet
# oder umgeht. Kommentare zaehlen mit: Auch ein Beispiel im Kopf des Skripts
# wuerde jemand kopieren.
forbidden() {
  grep -nE 'xattr([[:space:]]+-[A-Za-z]+)*[[:space:]]+-[A-Za-z]*[dc]|spctl.*--(master-disable|global-disable|disable|add)|defaults[[:space:]]+write.*LSQuarantine|lsregister' "$1" || true
}
found="$(forbidden "$script")"
if [[ -n "$found" ]]; then
  bad source_no_bypass "$found"
elif ! grep -qF 'run xattr -r -w com.apple.quarantine "$quarantine" "$staged_new/Humanitl.app"' "$script"; then
  bad source_no_bypass "kein xattr -r -w com.apple.quarantine auf der Kopie im Skript"
else
  ok source_no_bypass
fi
# Die Pruefung selbst: Jede verbotene Form faellt auf, das Setzen und Lesen nicht.
for line in 'xattr -dr com.apple.quarantine /Applications/Humanitl.app' 'xattr -c "$app"' \
  'xattr -r -d com.apple.quarantine "$app"' 'sudo spctl --master-disable' 'spctl --global-disable' \
  'spctl --add --label Humanitl "$app"' 'spctl --disable' \
  'defaults write com.apple.LaunchServices LSQuarantine -bool NO' \
  '/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app"'; do
  printf '%s\n' "$line" > "$work/forbidden.sh"
  if [[ -z "$(forbidden "$work/forbidden.sh")" ]]; then
    bad source_check_bites "findet '$line' nicht"
  fi
done
printf '%s\n' 'xattr -r -w com.apple.quarantine "$q" "$app"' 'xattr -p com.apple.quarantine "$app"' \
  'defaults read "$app/Contents/Info" CFBundleShortVersionString' > "$work/allowed.sh"
if [[ -n "$(forbidden "$work/allowed.sh")" ]]; then
  bad source_check_bites "meldet das Setzen oder Lesen der Quarantaene"
else
  ok source_check_bites
fi

# ---- Stubs -----------------------------------------------------------------

stubs="$work/stubs"
mkdir -p "$stubs" "$work/captured"
log="$work/calls.log"
for tool in apt-get dnf zypper pacman xattr open; do
  cat > "$stubs/$tool" << EOF
#!/bin/sh
echo "$tool \$*" >> "$log"
exit 0
EOF
done
# sudo fuehrt nichts aus; `sudo install ... <quelle> <ziel>` legt die Quelle
# unter captured/<name des ziels> ab.
cat > "$stubs/sudo" << EOF
#!/bin/sh
echo "sudo \$*" >> "$log"
if [ "\$1" = install ]; then
  for last in "\$@"; do :; done
  eval "src=\\\${\$((\$# - 1))}"
  cp "\$src" "$work/captured/\$(basename "\$last")"
fi
exit 0
EOF
# yay: -Si meldet das Paket nur, wenn STUB_AUR=1.
cat > "$stubs/yay" << EOF
#!/bin/sh
echo "yay \$*" >> "$log"
case "\$1" in -Si) [ "\${STUB_AUR:-0}" = 1 ] ;; *) exit 0 ;; esac
EOF
cat > "$stubs/uname" << 'EOF'
#!/bin/sh
case "$1" in
  -s) echo "${STUB_UNAME_S:-Linux}" ;;
  -m) echo "${STUB_UNAME_M:-x86_64}" ;;
  *) echo "${STUB_UNAME_S:-Linux}" ;;
esac
EOF
cat > "$stubs/getconf" << 'EOF'
#!/bin/sh
[ "$1" = GNU_LIBC_VERSION ] || exit 1
[ "${STUB_GLIBC:-2.41}" = none ] && exit 1
echo "glibc ${STUB_GLIBC:-2.41}"
EOF
# Ein vorhandenes humanitl im PATH, damit nie das des Rechners antwortet.
cat > "$stubs/humanitl" << EOF
#!/bin/sh
echo "humanitl \$*" >> "$log"
echo "humanitl 0.0.0-stub"
EOF
# mv scheitert mit STUB_MV_FAIL=1, wenn es die neue Fassung an ihren Platz
# bringen soll: der Fall fuer den Rueckbau.
cat > "$stubs/mv" << 'EOF'
#!/bin/sh
if [ "${STUB_MV_FAIL:-0}" = 1 ]; then
  case "$1" in */.humanitl-new.*/* | */.Humanitl.app.new.*/*) echo "mv: stub failure" >&2; exit 1 ;; esac
fi
exec /bin/mv "$@"
EOF
# macOS: hdiutil haengt ein, indem es eine App an den Einhaengepunkt legt.
# Deren humanitl schreibt jeden Aufruf ins Protokoll; mit STUB_BLOCKED=1 laesst
# "macOS" es nicht starten.
cat > "$stubs/hdiutil" << EOF
#!/bin/sh
echo "hdiutil \$*" >> "$log"
if [ "\$1" = attach ]; then
  while [ \$# -gt 0 ]; do [ "\$1" = -mountpoint ] && mnt=\$2; shift; done
  mkdir -p "\$mnt/Humanitl.app/Contents/MacOS/bin"
  cat > "\$mnt/Humanitl.app/Contents/MacOS/bin/humanitl" << 'APP'
#!/bin/sh
echo "app-cli \$*" >> "$log"
[ "\${STUB_BLOCKED:-0}" = 1 ] && exit 126
echo "humanitl 9.9.9"
APP
  chmod +x "\$mnt/Humanitl.app/Contents/MacOS/bin/humanitl"
fi
exit 0
EOF
cat > "$stubs/ditto" << 'EOF'
#!/bin/sh
cp -R "$1" "$2"
EOF
printf '#!/bin/sh\nexit 1\n' > "$stubs/defaults"
chmod +x "$stubs"/*

# ---- Release-Stellvertreter --------------------------------------------------

rel="$work/releases"
# release <version>: Archiv, RPM, .deb, .dmg und SHA256SUMS unter download/v<version>/
release() {
  local v="$1" dir="$rel/download/v$1" top="$work/build/humanitl-$1-linux-x86_64"
  mkdir -p "$dir" "$top/bin" "$top/app"
  for prog in humanitl humanitld; do
    printf '#!/bin/sh\necho "%s-tar $*" >> "%s"\necho "%s %s"\n' "$prog" "$log" "$prog" "$v" > "$top/bin/$prog"
  done
  printf '#!/bin/sh\necho "app %s"\n' "$v" > "$top/app/humanitl"
  chmod +x "$top/bin/"* "$top/app/humanitl"
  echo "archive $v" > "$top/INSTALL.txt"
  tar -C "$work/build" -czf "$dir/humanitl-$v-linux-x86_64.tar.gz" "humanitl-$v-linux-x86_64"
  echo "rpm $v" > "$dir/humanitl-$v-1.x86_64.rpm"
  echo "deb $v" > "$dir/humanitl_${v}_amd64.deb"
  echo "dmg $v" > "$dir/Humanitl-$v-macos.dmg"
  (cd "$dir" && sha256sum ./*.tar.gz ./*.rpm ./*.deb ./*.dmg | sed 's| \./| |' > SHA256SUMS)
}
release 9.9.9
release 9.9.10
# 9.9.11: jede Datei ist eine andere als die, deren Summe in SHA256SUMS steht.
release 9.9.11
for f in "$rel"/download/v9.9.11/*; do
  [[ "$f" == */SHA256SUMS ]] || echo tampered >> "$f"
done
# 9.9.12: SHA256SUMS kennt keine der Dateien.
release 9.9.12
printf '%s  humanitl-9.9.12-other.tar.gz\n' "$(printf '0%.0s' {1..64})" > "$rel/download/v9.9.12/SHA256SUMS"
# 9.9.13: SHA256SUMS im Binaermodus von sha256sum (`<summe> *<name>`).
release 9.9.13
(cd "$rel/download/v9.9.13" && sha256sum -b ./*.tar.gz ./*.rpm ./*.deb ./*.dmg | sed 's|\*\./|*|' > SHA256SUMS)

# Das APT-Repository: nur sein Schluessel, der echte von apt.nurkert.de
# (fixtures/install/apt-nurkert-KEY.gpg, oeffentlich). Seine SHA-256 muss die
# Festlegung im Skript sein; gpg --dearmor (ohne Agent) zeigt, dass das Skript
# ihn genauso dekodiert.
mkdir -p "$work/apt" "$work/apt-other"
cp "$here/fixtures/install/apt-nurkert-KEY.gpg" "$work/apt/KEY.gpg"
pinned="$(sed -n 's/^  apt_key_sha256=\${HUMANITL_APT_KEY_SHA256:-\([0-9a-f]\{64\}\)}$/\1/p' "$script")"
if [[ "$pinned" == "$(sha256sum < "$work/apt/KEY.gpg" | cut -d' ' -f1)" ]]; then
  ok apt_key_pin_matches_fixture
else
  bad apt_key_pin_matches_fixture "Festlegung '$pinned' ist nicht die SHA-256 des Schluessels in fixtures/install"
fi
gpg_ok=0
if command -v gpg > /dev/null; then
  GNUPGHOME="$work/gnupg" && mkdir -m 0700 "$GNUPGHOME" &&
    GNUPGHOME="$work/gnupg" gpg --batch --quiet --dearmor -o "$work/key-dearmored.gpg" "$work/apt/KEY.gpg" && gpg_ok=1
else
  missing_tool apt_key_dearmor gpg
fi
key_sha="$(sha256sum < "$work/apt/KEY.gpg" | cut -d' ' -f1)"
cp "$work/apt/KEY.gpg" "$work/apt-other/KEY.gpg"
echo "# changed" >> "$work/apt-other/KEY.gpg"

osr="$work/os-release"
os_release() { # <ID> [<ID_LIKE>]
  printf 'PRETTY_NAME="%s test"\nID=%s\n' "$1" "$1" > "$osr.$1"
  [[ -z "${2:-}" ]] || printf 'ID_LIKE="%s"\n' "$2" >> "$osr.$1"
}
os_release debian
os_release ubuntu debian
os_release fedora
os_release rocky "rhel centos fedora"
os_release arch
os_release opensuse-tumbleweed "opensuse suse"
os_release example

# ---- Ablauf eines Falls ------------------------------------------------------

prefix="$work/home/.local"

# env_for <os-release-id>: die Umgebung eines Laufs, in $envs.
env_for() {
  envs=(PATH="$stubs:/usr/bin:/bin" HOME="$work/home" TMPDIR="$work/tmp" LANG=C
    HUMANITL_RELEASES_URL="${URL_OVERRIDE:-file://$rel}" HUMANITL_APT_URL="file://${APT_DIR:-$work/apt}"
    HUMANITL_APT_KEY_SHA256="$key_sha" HUMANITL_OS_RELEASE="$osr.$1"
    STUB_UNAME_S="${STUB_UNAME_S:-Linux}" STUB_UNAME_M="${STUB_UNAME_M:-x86_64}"
    STUB_GLIBC="${STUB_GLIBC:-2.41}" STUB_AUR="${STUB_AUR:-0}" STUB_MV_FAIL="${STUB_MV_FAIL:-0}"
    STUB_BLOCKED="${STUB_BLOCKED:-0}")
}

fresh() {
  rm -rf "${work:?}/home" "${work:?}/tmp" "${work:?}/captured"
  mkdir -p "$work/home" "$work/tmp" "$work/captured"
  : > "$log"
}

# go <os-release-id> <argumente...>: ohne Terminal; Ausgabe in $out, Exit in $rc.
# again <...>: dasselbe im vorhandenen HOME.
again() {
  local id="$1"
  shift
  env_for "$id"
  : > "$log"
  rc=0
  out="$(cd "$work" && env -i "${envs[@]}" setsid -w sh "$script" "$@" 2>&1 < /dev/null)" || rc=$?
}
go() {
  fresh
  again "$@"
}

# go_tty <eingabe> <os-release-id> <argumente...>: mit Terminal, die Eingabe
# (printf-Folgen wie \n) wird getippt.
go_tty() {
  local input="$1" id="$2" cmd
  shift 2
  fresh
  env_for "$id"
  cmd="$(printf '%q ' cd "$work")&& $(printf '%q ' env -i "${envs[@]}" sh "$script" "$@")"
  rc=0
  # shellcheck disable=SC2059 # die Eingabe traegt printf-Folgen
  out="$(printf "$input" | script -qec "$cmd" /dev/null 2>&1 | tr -d '\r')" || rc=$?
}

# Nichts geaendert: HOME und TMPDIR leer, kein Paket- oder Installationsbefehl.
# Lesende Aufrufe (yay -Si, humanitl --version) sind erlaubt.
calls() { grep -vE '^(yay -Si|humanitl --version)' "$log" || true; }
unchanged() {
  local what="$1" left c
  left="$(find "$work/home" "$work/tmp" -mindepth 1 2> /dev/null | head -5)"
  c="$(calls)"
  if [[ -n "$left" ]]; then
    bad "$what" "Dateien angelegt: $left"
    return 1
  fi
  if [[ -n "$c" ]]; then
    bad "$what" "Befehle aufgerufen: $c"
    return 1
  fi
}
has() { grep -qF -- "$1" <<< "$out"; }

# expect_refusal <fall> <text> <os-release-id> <argumente...>
expect_refusal() {
  local name="$1" text="$2"
  shift 2
  go "$@"
  if [[ "$rc" -eq 0 ]]; then
    bad "$name" "Exit 0, erwartet Abbruch; Ausgabe: $out"
  elif ! has "$text"; then
    bad "$name" "Meldung ohne '$text': $out"
  elif unchanged "$name"; then
    ok "$name"
  fi
}

# ---- Nicht unterstuetzte Systeme ---------------------------------------------

STUB_UNAME_M=aarch64 expect_refusal linux_aarch64 'Humanitl for Linux is built for x86_64 only; this machine is aarch64.' debian --yes
STUB_UNAME_M=aarch64 expect_refusal linux_aarch64_dry 'this machine is aarch64.' debian --dry-run
STUB_UNAME_S=MINGW64_NT-10.0-26100 expect_refusal windows_mingw 'Windows (MINGW64_NT-10.0-26100) is not supported.' example --yes
STUB_UNAME_S=MSYS_NT-10.0-26100 expect_refusal windows_msys 'Windows (MSYS_NT-10.0-26100) is not supported.' example --yes
STUB_UNAME_S=CYGWIN_NT-10.0 expect_refusal windows_cygwin 'Windows (CYGWIN_NT-10.0) is not supported.' example --yes
STUB_UNAME_S=FreeBSD STUB_UNAME_M=amd64 expect_refusal unknown_os 'FreeBSD is not supported.' example --yes
STUB_GLIBC=none expect_refusal musl 'needs the GNU C library' example --yes
STUB_GLIBC=2.36 expect_refusal glibc_too_old 'glibc 2.36 is too old' debian --yes
expect_refusal dmg_on_linux '--method dmg is for macOS' example --yes --method dmg
expect_refusal bad_version '--version wants X.Y.Z' example --yes --version 1.2
URL_OVERRIDE=http://example.invalid/releases expect_refusal http_base 'refusing http://example.invalid/releases: downloads only over HTTPS' example --yes --version 9.9.9

# ---- Ohne Terminal, Antwort nein ----------------------------------------------

expect_refusal no_tty_no_yes 'error: no terminal to ask on.' example --version 9.9.9
go_tty 'n\n' example --version 9.9.9
if [[ "$rc" -ne 0 ]] && has 'Go ahead? [y/N]' && has 'Stopped. Nothing was changed.' && unchanged answer_no; then
  ok answer_no
else
  bad answer_no "rc=$rc: $out"
fi

# ---- --dry-run auf jedem Weg ---------------------------------------------------

# expect_dry <fall> <zeile> <os-release-id> <argumente...>: <zeile> steht genau so in der Ausgabe.
expect_dry() {
  local name="$1" line="$2"
  shift 2
  go "$@" --dry-run
  if [[ "$rc" -ne 0 ]]; then
    bad "$name" "Exit $rc: $out"
  elif ! has 'Dry run: nothing was changed.' || ! grep -qxF -- "$line" <<< "$out"; then
    bad "$name" "Ausgabe ohne Zeile '$line': $out"
  elif unchanged "$name"; then
    ok "$name"
  fi
}
tmpl='$TMPDIR/humanitl-install.XXXXXX'
expect_dry dry_debian "    sudo apt-get install -y $tmpl/humanitl_9.9.9_amd64.deb" debian --version 9.9.9
expect_dry dry_debian_key "    download file://$work/apt/KEY.gpg, check its SHA-256 against the pin in this script" debian --version 9.9.9
expect_dry dry_ubuntu "  Method:  APT repository file://$work/apt (ubuntu test uses APT)" ubuntu --version 9.9.9
expect_dry dry_fedora "  Method:  .rpm package with dnf (fedora test uses dnf)" fedora --version 9.9.9
expect_dry dry_rocky "    sudo dnf install -y $tmpl/humanitl-9.9.9-1.x86_64.rpm" rocky --version 9.9.9
expect_dry dry_arch_no_aur "  Method:  archive to $prefix, without root (humanitl-bin is not in the AUR (yet))" arch --version 9.9.9
STUB_AUR=1 expect_dry dry_arch_aur "  Method:  AUR package humanitl-bin with yay (yay finds humanitl-bin in the AUR)" arch
STUB_AUR=1 expect_dry dry_arch_aur_yes "    (--yes: yay installs without asking and skips the review of the PKGBUILD)" arch --yes
STUB_AUR=1 expect_dry dry_arch_aur_pinned "  Method:  archive to $prefix, without root (--version pins a release; the AUR package follows the newest)" arch --version 9.9.9
expect_dry dry_opensuse "  Method:  archive to $prefix, without root (the .rpm is not checked on openSUSE yet)" opensuse-tumbleweed --version 9.9.9
expect_dry dry_generic "    tar -xzf $tmpl/humanitl-9.9.9-linux-x86_64.tar.gz -C $prefix/lib/.humanitl-new.XXXXXX" example --version 9.9.9
expect_dry dry_version_equals "  Version: 9.9.10" example --version=9.9.10
expect_dry dry_prefix "  Method:  archive to $work/opt, without root (no package for example test)" example --version 9.9.9 --prefix "$work/opt"
STUB_UNAME_S=Darwin STUB_UNAME_M=arm64 expect_dry dry_macos \
  "    xattr -r -w com.apple.quarantine 0081;<time>;humanitl-install; $work/home/Applications/.Humanitl.app.new.XXXXXX/Humanitl.app" example --version 9.9.9
# Ohne Terminal genuegt --dry-run; nichts fragt.
expect_dry dry_no_yes "Steps:" example --version 9.9.9

# Der Plan zeigt sudo vor dem Paketschritt, nie vor dem Archiv.
go debian --dry-run --version 9.9.9
if grep -qxF '    sudo apt-get update' <<< "$out" && ! grep -q 'gnupg' <<< "$out"; then ok plan_names_sudo; else bad plan_names_sudo "$out"; fi
go example --dry-run --version 9.9.9
if grep -q 'sudo' <<< "$out"; then bad tarball_without_sudo "$out"; else ok tarball_without_sudo; fi

# ---- APT: alles geprueft vor dem ersten sudo -------------------------------------

go debian --yes --no-setup --version 9.9.9
sudo_lines="$(grep '^sudo ' "$log" || true)"
# Hat dieser Rechner den Schluessel schon (das Skript von apt.nurkert.de lief
# hier), legt install.sh ihn nicht noch einmal ab: Es hat ihn dann zu genau
# diesen Bytes dekodiert, sonst waere der Vergleich anders ausgefallen.
keyring=/usr/share/keyrings/nurkert-archive-keyring.gpg
key_there=0
if [[ "$gpg_ok" == 1 && -f "$keyring" ]] && cmp -s "$keyring" "$work/key-dearmored.gpg"; then
  key_there=1
  want_sudo=""
else
  want_sudo="sudo install -m 0644 $work/tmp/humanitl-install.X/keyring.gpg $keyring
"
fi
want_sudo="${want_sudo}sudo install -m 0644 $work/tmp/humanitl-install.X/nurkert.list /etc/apt/sources.list.d/nurkert.list
sudo apt-get update
sudo apt-get install -y $work/tmp/humanitl-install.X/humanitl_9.9.9_amd64.deb"
got_sudo="$(sed -E 's|humanitl-install\.[A-Za-z0-9]+|humanitl-install.X|g' <<< "$sudo_lines" | grep -v '^sudo mkdir -p /usr/share/keyrings$' || true)"
if [[ "$rc" -ne 0 ]]; then
  bad apt_install "Exit $rc: $out"
elif [[ "$got_sudo" != "$want_sudo" ]]; then
  bad apt_install "sudo-Aufrufe:
$got_sudo"
elif ! has 'Key matches the pin:' || ! has 'Checksum matches SHA256SUMS: humanitl_9.9.9_amd64.deb'; then
  bad apt_install "Ausgabe: $out"
elif [[ "$(cat "$work/captured/nurkert.list")" != "deb [signed-by=/usr/share/keyrings/nurkert-archive-keyring.gpg] file://$work/apt stable main" ]]; then
  bad apt_install "Quellzeile: $(cat "$work/captured/nurkert.list")"
elif [[ "$key_there" == 1 ]] && ! has "The key is already in $keyring"; then
  bad apt_install "vorhandener Schluessel nicht erkannt: $out"
elif [[ "$key_there" == 0 && "$gpg_ok" == 1 ]] && ! cmp -s "$work/captured/nurkert-archive-keyring.gpg" "$work/key-dearmored.gpg"; then
  bad apt_install "der Schluessel ist nicht, was gpg --dearmor schreibt"
elif [[ -n "$(ls -A "$work/tmp")" ]]; then
  bad apt_install "Reste in TMPDIR"
else
  ok "apt_install (sudo erst nach Schluessel und .deb; Schluessel gleich gpg --dearmor: $gpg_ok)"
fi

APT_DIR="$work/apt-other" go debian --yes --no-setup --version 9.9.9
if [[ "$rc" -ne 0 ]] && has 'but this script expects' && [[ -z "$(calls)" ]] && [[ -z "$(ls -A "$work/tmp")" ]]; then
  ok apt_wrong_key
else
  bad apt_wrong_key "rc=$rc calls=$(calls): $out"
fi

# ---- Archiv-Weg: installieren, ersetzen, Rueckfragen, Rueckbau -------------------

go example --yes --no-setup --version 9.9.9
v="$("$prefix/bin/humanitl" --version 2> /dev/null || true)"
if [[ "$rc" -ne 0 ]]; then
  bad tarball_install "Exit $rc: $out"
elif [[ "$v" != "humanitl 9.9.9" ]]; then
  bad tarball_install "humanitl --version sagt '$v'"
elif [[ ! -f "$prefix/lib/humanitl/INSTALLED-FILES" ]] || [[ "$(readlink "$prefix/bin/humanitl-app")" != "$prefix/lib/humanitl/app/humanitl" ]]; then
  bad tarball_install "INSTALLED-FILES oder humanitl-app fehlt"
elif ! has 'Checksum matches SHA256SUMS: humanitl-9.9.9-linux-x86_64.tar.gz' || ! has 'Installed: humanitl 9.9.9'; then
  bad tarball_install "Ausgabe: $out"
elif [[ -n "$(calls | grep -v '^humanitl-tar --version$' || true)" ]] || [[ -n "$(ls -A "$work/tmp")" ]]; then
  bad tarball_install "Paketbefehl oder Reste in TMPDIR: $(calls) $(ls -A "$work/tmp")"
elif [[ "$(ls -A "$prefix/lib")" != humanitl ]]; then
  bad tarball_install "Reste neben dem Ziel: $(ls -A "$prefix/lib")"
else
  ok tarball_install
fi

again example --yes --no-setup --version 9.9.10
v="$("$prefix/bin/humanitl" --version 2> /dev/null || true)"
if [[ "$rc" -eq 0 && "$v" == "humanitl 9.9.10" ]] && has 'it will be replaced' && [[ "$(ls -A "$prefix/lib")" == humanitl ]]; then
  ok tarball_replace
else
  bad tarball_replace "rc=$rc version='$v' lib=$(ls -A "$prefix/lib"): $out"
fi

# Rueckbau: Scheitert das Umbenennen der neuen Fassung, steht die alte wieder da.
STUB_MV_FAIL=1 again example --yes --no-setup --version 9.9.9
v="$("$prefix/bin/humanitl" --version 2> /dev/null || true)"
if [[ "$rc" -ne 0 ]] && has 'the previous one is back in place' && [[ "$v" == "humanitl 9.9.10" ]] &&
  [[ "$(ls -A "$prefix/lib")" == humanitl ]]; then
  ok tarball_rollback
else
  bad tarball_rollback "rc=$rc version='$v' lib=$(ls -A "$prefix/lib"): $out"
fi

# Eine fremde Datei an der Stelle eines Links wird nicht ueberschrieben.
rm "$prefix/bin/humanitld"
echo foreign > "$prefix/bin/humanitld"
again example --yes --no-setup --version 9.9.9
if [[ "$rc" -ne 0 ]] && has 'does not point into' && [[ "$(cat "$prefix/bin/humanitld")" == foreign ]] &&
  [[ "$("$prefix/bin/humanitl" --version)" == "humanitl 9.9.10" ]]; then
  ok tarball_foreign_file
else
  bad tarball_foreign_file "rc=$rc: $out"
fi

go example --yes --no-setup --version 9.9.13
if [[ "$rc" -eq 0 ]] && [[ "$("$prefix/bin/humanitl" --version)" == "humanitl 9.9.13" ]]; then
  ok sums_binary_mode
else
  bad sums_binary_mode "rc=$rc: $out"
fi

go example --yes --no-setup --version 9.9.9 --prefix "$work/home/opt"
if [[ "$rc" -eq 0 ]] && [[ "$("$work/home/opt/bin/humanitl" --version)" == "humanitl 9.9.9" ]] && [[ ! -e "$prefix" ]]; then
  ok tarball_prefix
else
  bad tarball_prefix "rc=$rc: $out"
fi

# Im Terminal: ja zu allem, auch zu daemon install und sandbox check.
go_tty 'y\ny\ny\n' example --version 9.9.9
if [[ "$rc" -eq 0 ]] && grep -qx 'humanitl-tar daemon install' "$log" && grep -qx 'humanitl-tar sandbox check' "$log"; then
  ok tarball_tty_setup
else
  bad tarball_tty_setup "rc=$rc log=$(cat "$log"): $out"
fi
go_tty 'y\nn\nn\n' example --version 9.9.9
if [[ "$rc" -eq 0 ]] && ! grep -q 'daemon install\|sandbox check' "$log"; then
  ok tarball_tty_no_setup
else
  bad tarball_tty_no_setup "rc=$rc log=$(cat "$log")"
fi

# ---- macOS-Weg, nachgestellt -------------------------------------------------------

apps="$work/home/Applications"
mac() { STUB_UNAME_S=Darwin STUB_UNAME_M=arm64 "$@"; }
mac go example --yes --version 9.9.9
q_line="$(grep '^xattr ' "$log" || true)"
if [[ "$rc" -ne 0 ]]; then
  bad macos_yes "Exit $rc: $out"
elif [[ ! "$q_line" =~ ^xattr\ -r\ -w\ com\.apple\.quarantine\ 0081\;[0-9a-f]+\;humanitl-install\;\ $apps/\.Humanitl\.app\.new\.[A-Za-z0-9]+/Humanitl\.app$ ]]; then
  bad macos_yes "Markierung nicht auf der Kopie vor dem Umbenennen: '$q_line'"
elif [[ ! -x "$apps/Humanitl.app/Contents/MacOS/bin/humanitl" ]] || [[ "$(ls -A "$apps")" != Humanitl.app ]]; then
  bad macos_yes "App fehlt oder Reste: $(ls -A "$apps")"
elif ! has 'Setup is pending.' || ! has "  \"$apps/Humanitl.app/Contents/MacOS/bin/humanitl\" daemon install"; then
  bad macos_yes "kein Hinweis auf die ausstehende Einrichtung: $out"
elif grep -q '^app-cli\|^open ' "$log"; then
  bad macos_yes "mit --yes lief etwas aus der App: $(cat "$log")"
else
  ok macos_yes
fi

# Im Terminal: nicht oeffnen lassen, Enter, dann ja zu beidem.
mac go_tty 'y\nn\n\ny\ny\n' example --version 9.9.9
order="$(grep '^app-cli' "$log" | tr '\n' '|')"
if [[ "$rc" -eq 0 ]] && has 'press Enter' && [[ "$order" == 'app-cli --version|app-cli --version|app-cli daemon install|app-cli sandbox check|' ]]; then
  ok macos_tty_setup
else
  bad macos_tty_setup "rc=$rc order=$order: $out"
fi
mac go_tty 'y\ny\n\ny\ny\n' example --version 9.9.9
if [[ "$rc" -eq 0 ]] && grep -qx "open $apps/Humanitl.app" "$log"; then ok macos_tty_open; else bad macos_tty_open "rc=$rc log=$(cat "$log")"; fi
# macOS laesst das Programm nicht starten: zweimal Enter, dann "pending", keine Einrichtung.
STUB_BLOCKED=1 mac go_tty 'y\nn\n\n\n' example --version 9.9.9
if [[ "$rc" -eq 0 ]] && has 'macOS still blocks the command line' && has 'Setup is pending.' &&
  ! grep -q 'daemon install\|sandbox check' "$log"; then
  ok macos_tty_blocked
else
  bad macos_tty_blocked "rc=$rc log=$(cat "$log"): $out"
fi
# Rueckbau: die vorhandene App bleibt, wenn die neue nicht an ihren Platz kommt.
mac go example --yes --version 9.9.9
echo old-marker > "$apps/Humanitl.app/marker"
STUB_MV_FAIL=1 mac again example --yes --version 9.9.10
if [[ "$rc" -ne 0 ]] && [[ -f "$apps/Humanitl.app/marker" ]] && [[ "$(ls -A "$apps")" == Humanitl.app ]]; then
  ok macos_rollback
else
  bad macos_rollback "rc=$rc apps=$(ls -A "$apps"): $out"
fi

# ---- Pruefsumme: falsch oder fehlend bricht vor jeder Installation ab -------

# checksum_case <fall> <text> <os-release-id> <argumente...>: Abbruch, kein
# Programm installiert, kein Paketbefehl, TMPDIR wieder leer.
checksum_case() {
  local name="$1" text="$2"
  shift 2
  go "$@"
  local c
  c="$(calls)"
  if [[ "$rc" -eq 0 ]]; then
    bad "$name" "Exit 0, erwartet Abbruch: $out"
  elif ! has "$text"; then
    bad "$name" "Meldung ohne '$text': $out"
  elif [[ -e "$work/home/.local/lib/humanitl" || -e "$work/home/.local/bin/humanitl" ]]; then
    bad "$name" "trotzdem installiert"
  elif [[ -n "$c" ]]; then
    bad "$name" "Paketbefehl trotz Abbruch: $c"
  elif [[ -n "$(ls -A "$work/tmp")" ]]; then
    bad "$name" "Reste in TMPDIR: $(ls -A "$work/tmp")"
  else
    ok "$name"
  fi
}
run_checksum_cases() {
  checksum_case checksum_wrong_tarball 'checksum mismatch for humanitl-9.9.11-linux-x86_64.tar.gz' example --yes --no-setup --version 9.9.11
  checksum_case checksum_missing_tarball 'SHA256SUMS of 9.9.12 has no line for humanitl-9.9.12-linux-x86_64.tar.gz' example --yes --no-setup --version 9.9.12
  checksum_case checksum_wrong_rpm 'checksum mismatch for humanitl-9.9.11-1.x86_64.rpm' fedora --yes --no-setup --version 9.9.11
  checksum_case checksum_missing_rpm 'SHA256SUMS of 9.9.12 has no line for humanitl-9.9.12-1.x86_64.rpm' fedora --yes --no-setup --version 9.9.12
  checksum_case checksum_wrong_deb 'checksum mismatch for humanitl_9.9.11_amd64.deb' debian --yes --no-setup --version 9.9.11
  checksum_case checksum_missing_deb 'SHA256SUMS of 9.9.12 has no line for humanitl_9.9.12_amd64.deb' debian --yes --no-setup --version 9.9.12
}
run_checksum_cases

# ---- Mutationsbeweise ----------------------------------------------------------

if [[ -z "${INSTALL_SH:-}" ]]; then
  # Ohne die Pruefung (Zeile gesucht, Summe verglichen) muessen alle sechs
  # Pruefsummen-Faelle rot werden.
  mutant="$work/install-mutant.sh"
  sed -e '/\[ -n "\$fv_want" \] || die/d' \
    -e 's/if \[ "\$fv_have" != "\$fv_want" \]; then/if false; then/' "$script" > "$mutant"
  if [[ "$(diff "$script" "$mutant" | grep -c '^[<>]')" -ne 3 ]]; then
    bad mutation_checksum "die Pruefung steht nicht mehr so im Skript; Mutant passt nicht"
  else
    before=$fails
    saved_script="$script"
    script="$mutant"
    run_checksum_cases > "$work/mutant.out"
    script="$saved_script"
    red=$(grep -c '^FAIL' "$work/mutant.out" || true)
    fails=$before
    if [[ "$red" -eq 6 ]]; then
      ok "mutation_checksum (6 von 6 Pruefsummen-Faellen rot ohne die Pruefung)"
    else
      bad mutation_checksum "nur $red von 6 Faellen rot ohne die Pruefung: $(cat "$work/mutant.out")"
    fi
  fi
  # Ohne den Vergleich mit der Festlegung laeuft sudo mit einem fremden Schluessel.
  mutant_key="$work/install-key.sh"
  sed 's/if \[ "\$apt_key_have" != "\$apt_key_sha256" \]; then/if false; then/' "$script" > "$mutant_key"
  if cmp -s "$script" "$mutant_key"; then
    bad mutation_key "Mutant ohne Aenderung"
  else
    saved_script="$script"
    script="$mutant_key"
    APT_DIR="$work/apt-other" go debian --yes --no-setup --version 9.9.9
    script="$saved_script"
    if [[ -n "$(calls)" ]]; then ok "mutation_key (fremder Schluessel erreicht sudo ohne den Vergleich)"; else bad mutation_key "kein sudo auch ohne Vergleich: $out"; fi
  fi
  mutant_src="$work/install-xattr.sh"
  sed 's/run xattr -r -w com.apple.quarantine "\$quarantine"/run xattr -dr com.apple.quarantine/' "$script" > "$mutant_src"
  if cmp -s "$script" "$mutant_src"; then
    bad mutation_source "Mutant ohne Aenderung"
  elif [[ -z "$(forbidden "$mutant_src")" ]]; then
    bad mutation_source "xattr -dr im Mutanten nicht gefunden"
  else
    ok mutation_source
  fi
fi

if [[ "$fails" -ne 0 ]]; then
  echo "install_sh_test: $fails Fehler"
  exit 1
fi
echo "install_sh_test: alles gruen"
