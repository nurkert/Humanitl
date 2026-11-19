#!/usr/bin/env bash
# Test fuer uninstall.sh (HUM-550) ohne Container und ohne root.
#
# Das Skript laeuft hier gegen nachgestellte Werkzeuge: `uname`, `sudo`,
# `dpkg-query`, `rpm`, `pacman`, `apt-get`, `dnf`, `zypper` und fuer den
# AUR-Weg `yay` liegen als Stubs vorn im PATH und schreiben jeden Aufruf in ein
# Protokoll. `sudo` fuehrt nichts aus. Das installierte `humanitl` ist ein
# Stub mit dem Vertrag, den das Skript liest: `--json daemon status` mit
# `agent_session` (oder DAEMON_001 ohne Daemon), `sessions`, `config paths`
# und `daemon uninstall`, das eine nachgestellte Unit unter
# ~/.config/systemd/user entfernt. Die Archiv-Installation liegt so in HOME,
# wie install.sh sie hinterlaesst (lib/humanitl mit INSTALLED-FILES und drei
# Links in bin), die macOS-App unter ~/Applications. HOME und TMPDIR zeigen
# in einen Wegwerf-Baum; nichts ausserhalb davon wird angefasst.
#
# Ohne Terminal laeuft jeder Fall unter `setsid`; Faelle mit Rueckfragen
# laufen unter `script`, das ein Terminal stellt und die Antworten tippt.
#
# Geprueft wird:
#   - shellcheck, `sh -n` mit dash und busybox sh
#   - der gemeinsame Teil von install.sh und uninstall.sh ist Byte fuer Byte
#     derselbe (und die Pruefung findet eine Abweichung)
#   - aarch64-Linux und Windows: Meldung, nichts geaendert; nichts gefunden:
#     Meldung, Exit 0, nichts geaendert; --method ohne Fund: Abbruch
#   - ohne Terminal und ohne --yes: Abbruch; Antwort `n`: Abbruch; beides
#     ohne Aenderung
#   - --dry-run, auch mit --purge: nichts geaendert, Plan mit Groessen
#   - Archiv: genau die Pfade aus INSTALLED-FILES, `daemon uninstall` vorher,
#     Daten bleiben; mit --purge --yes sind die Orte weg, Projektordner und
#     fremde Ordner daneben bleiben; ein fremder Link bleibt; eine fremde
#     Zeile in INSTALLED-FILES bricht ab
#   - APT, RPM (dnf, zypper unter openSUSE), pacman (yay oder sudo pacman),
#     macOS-App (nachgestellt), zwei Wege zugleich
#   - laufende Sitzung oder unklare Antwort: Abbruch vor jedem Schritt, auch
#     mit --dry-run; kein Daemon: weiter
#   - --purge mit einem Programm, das die Orte nicht nennt, mit einem Ort, der
#     keiner von Humanitl ist, mit unbekannter, doppelter oder fehlender Art
#     (ein Zeilenumbruch im Pfad), oder mit einem Ort, der ueber einen Link
#     das HOME ist: Abbruch ohne Aenderung; ein Ort, der sich zwischen
#     Auflistung und Loeschen aendert: nichts von den Daten geloescht; ein Link
#     auf ein echtes humanitl-Verzeichnis: beides geloescht, Nachbarn bleiben
#   - laeuft das Programm nicht: Abbruch vor jeder Aenderung; --prefix /: Abbruch
#   - APT: Paket und, auf eigene Frage, Quelle und Schluessel in einem sudo
#   - scheitert `daemon uninstall`, bleibt das Programm
#   - Mutationsbeweise: ohne die Pruefung auf laufende Sitzungen wird der
#     Fall rot; ohne die Rueckfrage vor --purge loescht die Antwort `n` die
#     Daten, und der Fall wird rot; ohne die Aufloesung der Links loescht
#     --purge durch den Link im HOME, und der Fall wird rot.
#
# Umgebung:
#   UNINSTALL_SH=<pfad>       prueft eine andere Fassung des Skripts
#   INSTALL_TEST_STRICT=1     shellcheck und busybox muessen da sein (CI)
# Die Muster und Mutanten unten tragen $-Zeichen woertlich.
# shellcheck disable=SC2016
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
script="${UNINSTALL_SH:-$here/../../uninstall.sh}"
script="$(cd "$(dirname "$script")" && pwd)/$(basename "$script")"
installer="$here/../../install.sh"
strict="${INSTALL_TEST_STRICT:-0}"

work="$(mktemp -d "${TMPDIR:-/tmp}/uninstall-sh-test.XXXXXX")"
fails=0
cleanup() {
  if [[ "$fails" -eq 0 ]]; then
    rm -rf "$work"
  else
    echo "uninstall_sh_test: Baum bleibt in $work"
  fi
}
trap cleanup EXIT

for tool in setsid script; do
  command -v "$tool" > /dev/null || { echo "uninstall_sh_test: $tool fehlt (util-linux)" >&2; exit 2; }
done

ok() { echo "ok   $1"; }
bad() {
  echo "FAIL $1: $2"
  # Auf GitHub zusaetzlich als Annotation, damit der Befund ohne Log lesbar ist.
  if [[ "${GITHUB_ACTIONS:-}" == true ]]; then
    echo "::error title=uninstall-script $1::$(printf '%s' "$2" | head -c 1500 | tr '\n' ' ')"
  fi
  fails=$((fails + 1))
}

# ---- Werkzeuge -----------------------------------------------------------------

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

# ---- Der gemeinsame Teil -------------------------------------------------------

# shared <datei>: die Zeilen vom Anfang bis zum Ende des gemeinsamen Teils.
shared() { awk '/^# ---- Shared by install.sh and uninstall.sh/,/^# ---- End of the shared part/' "$1"; }
shared "$installer" > "$work/shared-install"
shared "$script" > "$work/shared-uninstall"
if [[ "$(wc -l < "$work/shared-install")" -lt 50 ]] || ! grep -q '^ask() {' "$work/shared-install" ||
  ! grep -q '^detect_platform() {' "$work/shared-install" || ! grep -q '^# ---- End of the shared part' "$work/shared-install"; then
  bad shared_part "install.sh hat keinen vollstaendigen gemeinsamen Teil zwischen den Markierungen"
elif ! cmp -s "$work/shared-install" "$work/shared-uninstall"; then
  bad shared_part "der gemeinsame Teil weicht ab: $(diff "$work/shared-install" "$work/shared-uninstall" | head -10)"
else
  ok "shared_part ($(wc -l < "$work/shared-install") Zeilen gleich)"
fi
# Die Pruefung selbst: Ein geaendertes Zeichen faellt auf.
sed 's/^say() {/say(){/' "$work/shared-uninstall" > "$work/shared-mutant"
if cmp -s "$work/shared-install" "$work/shared-mutant"; then
  bad shared_part_bites "eine Aenderung im gemeinsamen Teil faellt nicht auf"
else
  ok shared_part_bites
fi

# ---- Stubs -----------------------------------------------------------------

stubs="$work/stubs"
aur_stubs="$work/stubs-aur"
mkdir -p "$stubs" "$aur_stubs"
log="$work/calls.log"
for tool in apt-get dnf zypper; do
  printf '#!/bin/sh\necho "%s $*" >> "%s"\nexit 0\n' "$tool" "$log" > "$stubs/$tool"
done
cat > "$stubs/sudo" << EOF
#!/bin/sh
echo "sudo \$*" >> "$log"
exit 0
EOF
# Paketdatenbanken: Gefunden wird nur, was STUB_DPKG, STUB_RPM, STUB_PACMAN sagen.
cat > "$stubs/dpkg-query" << 'EOF'
#!/bin/sh
[ "${STUB_DPKG:-0}" = 1 ] || exit 1
printf 'install ok installed'
EOF
cat > "$stubs/rpm" << EOF
#!/bin/sh
case "\$1" in -q) [ "\${STUB_RPM:-0}" = 1 ] ;; *) echo "rpm \$*" >> "$log" ;; esac
EOF
cat > "$stubs/pacman" << EOF
#!/bin/sh
case "\$1" in -Q) [ -n "\${STUB_PACMAN:-}" ] && [ "\$2" = "\${STUB_PACMAN:-}" ] ;; *) echo "pacman \$*" >> "$log" ;; esac
EOF
cat > "$aur_stubs/yay" << EOF
#!/bin/sh
echo "yay \$*" >> "$log"
EOF
cat > "$stubs/uname" << 'EOF'
#!/bin/sh
case "$1" in
  -s) echo "${STUB_UNAME_S:-Linux}" ;;
  -m) echo "${STUB_UNAME_M:-x86_64}" ;;
  *) echo "${STUB_UNAME_S:-Linux}" ;;
esac
EOF
printf '#!/bin/sh\necho 15.1\n' > "$stubs/sw_vers"
chmod +x "$stubs"/* "$aur_stubs"/*

# Das installierte Programm. STUB_SESSION: none, running, unknown oder
# nodaemon; STUB_PATHS: ok, old (kennt `config paths` nicht) oder evil (nennt
# HOME); STUB_UNINSTALL_FAIL=1 laesst `daemon uninstall` scheitern;
# STUB_CLI_BROKEN=1 laesst es gar nicht laufen.
cat > "$work/cli-stub" << EOF
#!/bin/sh
echo "cli \$*" >> "$log"
[ "\${STUB_CLI_BROKEN:-0}" = 1 ] && exit 126
case "\$*" in
  --version) echo "humanitl 9.9.9" ;;
  "--json daemon status")
    case "\${STUB_SESSION:-none}" in
      nodaemon) echo '{"code":"DAEMON_001","severity":"blocking","why":"no daemon"}'; exit 2 ;;
      *) echo '{"socket":"/run/x","agent_session":"'"\${STUB_SESSION:-none}"'","session_id":"s"}' ;;
    esac
    ;;
  sessions) echo "session s-1 (running, sandbox sb-42)" ;;
  "daemon uninstall")
    [ "\${STUB_UNINSTALL_FAIL:-0}" = 1 ] && { echo "error[DAEMON_014]: systemctl failed" >&2; exit 1; }
    rm -f "\$HOME/.config/systemd/user/humanitld.service"
    # STUB_SWAP=1: zwischen Auflistung und Loeschen zeigt der Ort woanders hin.
    if [ "\${STUB_SWAP:-0}" = 1 ]; then
      mkdir -p "\$HOME/elsewhere/humanitl" && echo keep > "\$HOME/elsewhere/humanitl/file"
      mv "\$HOME/.config/humanitl" "\$HOME/.config/humanitl.moved"
      ln -s "\$HOME/elsewhere/humanitl" "\$HOME/.config/humanitl"
    fi
    echo "removed the unit"
    ;;
  "config paths")
    case "\${STUB_PATHS:-ok}" in
      old) echo "error[CLI_004]: unrecognized subcommand 'paths'" >&2; exit 1 ;;
      evil) printf 'NAME    PATH\nconfig  %s\ndata    %s\n' "\$HOME" "\$HOME/.local/share/humanitl" ;;
      alias) printf 'NAME    PATH\nconfig  %s\ndata    %s\n' "\$HOME/alias/humanitl" "\$HOME/.local/share/humanitl" ;;
      split) printf 'NAME    PATH\nconfig  %s\n%s\ndata    %s\n' "\$HOME/.config/humanitl" "\$HOME/project/humanitl" "\$HOME/.local/share/humanitl" ;;
      dup) printf 'NAME    PATH\nconfig  %s\ndata    %s\ndata    %s\n' "\$HOME/.config/humanitl" "\$HOME/.local/share/humanitl" "\$HOME/project/humanitl" ;;
      missing) printf 'NAME    PATH\nconfig  %s\n' "\$HOME/.config/humanitl" ;;
      *) printf 'NAME    PATH\nconfig  %s\ndata    %s\n' "\$HOME/.config/humanitl" "\$HOME/.local/share/humanitl" ;;
    esac
    ;;
  *) echo "cli stub: unknown call \$*" >&2; exit 1 ;;
esac
EOF
chmod +x "$work/cli-stub"

osr="$work/os-release"
os_release() { # <ID> [<ID_LIKE>]
  printf 'PRETTY_NAME="%s test"\nID=%s\n' "$1" "$1" > "$osr.$1"
  [[ -z "${2:-}" ]] || printf 'ID_LIKE="%s"\n' "$2" >> "$osr.$1"
}
os_release debian
os_release fedora
os_release opensuse-tumbleweed "opensuse suse"
os_release arch
os_release example

# ---- Zustand eines Falls --------------------------------------------------------

home="$work/home"
prefix="$home/.local"
pkgbin="$work/pkgbin"

# fresh: leeres HOME mit Nutzerdaten, einem Projektordner, einem fremden
# Ordner neben den Daten und einer nachgestellten Unit.
fresh() {
  rm -rf "${home:?}" "${work:?}/tmp" "$pkgbin"
  mkdir -p "$home/.config/humanitl" "$home/.local/share/humanitl/audit" "$home/project/src" \
    "$home/.local/share/other-app" "$home/.config/systemd/user" "$work/tmp"
  echo 'ui.language = "de"' > "$home/.config/humanitl/config.toml"
  head -c 3000 /dev/zero > "$home/.local/share/humanitl/humanitl.db"
  echo '{}' > "$home/.local/share/humanitl/audit/audit.jsonl"
  echo 'fn main() {}' > "$home/project/src/main.rs"
  mkdir -p "$home/project/.humanitl" && echo 'x' > "$home/project/.humanitl/profile.toml"
  echo keep > "$home/.local/share/other-app/data"
  echo '# Written by humanitl daemon install' > "$home/.config/systemd/user/humanitld.service"
  : > "$log"
}

# with_tarball: die Archiv-Installation, wie install.sh sie hinterlaesst.
with_tarball() {
  local dest="$prefix/lib/humanitl"
  mkdir -p "$dest/bin" "$dest/app" "$prefix/bin"
  cp "$work/cli-stub" "$dest/bin/humanitl"
  printf '#!/bin/sh\n' > "$dest/bin/humanitld"
  printf '#!/bin/sh\n' > "$dest/app/humanitl"
  chmod +x "$dest/bin/humanitld" "$dest/app/humanitl"
  ln -sfn "$dest/bin/humanitl" "$prefix/bin/humanitl"
  ln -sfn "$dest/bin/humanitld" "$prefix/bin/humanitld"
  ln -sfn "$dest/app/humanitl" "$prefix/bin/humanitl-app"
  printf '%s\n' "# Written by install.sh; uninstall.sh removes exactly these paths." \
    "$dest" "$prefix/bin/humanitl" "$prefix/bin/humanitld" "$prefix/bin/humanitl-app" > "$dest/INSTALLED-FILES"
}
# with_package: das Programm eines Pakets unter HUMANITL_PACKAGE_BIN.
with_package() {
  mkdir -p "$pkgbin"
  cp "$work/cli-stub" "$pkgbin/humanitl"
}
apps="$home/Applications"
app="$apps/Humanitl.app"
with_app() {
  mkdir -p "$app/Contents/MacOS/bin"
  cp "$work/cli-stub" "$app/Contents/MacOS/bin/humanitl"
}

# snapshot: jeder Pfad unter HOME mit Art, fuer "nichts geaendert".
snapshot() { (cd "$home" && find . -printf '%y %p\n' | LC_ALL=C sort); }

env_for() {
  envs=(PATH="${EXTRA_PATH:+$EXTRA_PATH:}$stubs:/usr/bin:/bin" HOME="$home" TMPDIR="$work/tmp" LANG=C
    HUMANITL_OS_RELEASE="$osr.$1" HUMANITL_PACKAGE_BIN="$pkgbin"
    STUB_UNAME_S="${STUB_UNAME_S:-Linux}" STUB_UNAME_M="${STUB_UNAME_M:-x86_64}"
    STUB_DPKG="${STUB_DPKG:-0}" STUB_RPM="${STUB_RPM:-0}" STUB_PACMAN="${STUB_PACMAN:-}"
    STUB_SESSION="${STUB_SESSION:-none}" STUB_PATHS="${STUB_PATHS:-ok}"
    STUB_UNINSTALL_FAIL="${STUB_UNINSTALL_FAIL:-0}" STUB_CLI_BROKEN="${STUB_CLI_BROKEN:-0}"
    STUB_SWAP="${STUB_SWAP:-0}")
}

# go <os-release-id> <argumente...>: im vorhandenen HOME, ohne Terminal;
# Ausgabe in $out, Exit in $rc, Zustand davor in $before.
go() {
  local id="$1"
  shift
  env_for "$id"
  before="$(snapshot)"
  : > "$log"
  rc=0
  out="$(cd "$work" && env -i "${envs[@]}" setsid -w sh "$script" "$@" 2>&1 < /dev/null)" || rc=$?
}

# go_tty <eingabe> <os-release-id> <argumente...>: mit Terminal.
go_tty() {
  local input="$1" id="$2" cmd
  shift 2
  env_for "$id"
  before="$(snapshot)"
  : > "$log"
  cmd="$(printf '%q ' cd "$work")&& $(printf '%q ' env -i "${envs[@]}" sh "$script" "$@")"
  rc=0
  # shellcheck disable=SC2059 # die Eingabe traegt printf-Folgen
  out="$(printf "$input" | script -qec "$cmd" /dev/null 2>&1 | tr -d '\r')" || rc=$?
}

# Nur lesende Aufrufe: das Programm nach Version, Sitzung, Orten fragen.
changing_calls() { grep -vE '^cli (--version|--json daemon status|config paths|sessions)$' "$log" || true; }
has() { grep -qF -- "$1" <<< "$out"; }

# unchanged <fall>: HOME wie vorher und kein aendernder Aufruf.
unchanged() {
  local c
  c="$(changing_calls)"
  if [[ "$(snapshot)" != "$before" ]]; then
    bad "$1" "HOME geaendert: $(diff <(echo "$before") <(snapshot) | head -5)"
    return 1
  fi
  if [[ -n "$c" ]]; then
    bad "$1" "Befehle aufgerufen: $c"
    return 1
  fi
}

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

data_there() { [[ -f "$home/.config/humanitl/config.toml" && -f "$home/.local/share/humanitl/humanitl.db" ]]; }
neighbours_there() {
  [[ -f "$home/project/src/main.rs" && -f "$home/project/.humanitl/profile.toml" && -f "$home/.local/share/other-app/data" ]]
}
tarball_gone() {
  [[ ! -e "$prefix/lib/humanitl" && ! -e "$prefix/bin/humanitl" && ! -L "$prefix/bin/humanitl" &&
    ! -L "$prefix/bin/humanitld" && ! -L "$prefix/bin/humanitl-app" ]]
}

# ---- Nicht unterstuetzt, nichts gefunden, ohne Terminal ----------------------------

fresh
with_tarball
STUB_UNAME_M=aarch64 expect_refusal linux_aarch64 'this machine is aarch64.' debian --yes
STUB_UNAME_S=MINGW64_NT-10.0-26100 expect_refusal windows 'Windows (MINGW64_NT-10.0-26100) is not supported.' example --yes
expect_refusal no_tty_no_yes 'error: no terminal to ask on.' example
expect_refusal method_not_found 'Humanitl is not installed here by the way apt.' example --yes --method apt
expect_refusal dmg_on_linux '--method dmg is for macOS' example --yes --method dmg

fresh
go example --yes
if [[ "$rc" -eq 0 ]] && has 'Found no Humanitl to remove' && has 'Nothing was changed.' && unchanged nothing_found; then
  ok nothing_found
else
  bad nothing_found "rc=$rc: $out"
fi

# ---- --dry-run ---------------------------------------------------------------------

fresh
with_tarball
go example --dry-run
if [[ "$rc" -ne 0 ]]; then
  bad dry_run "Exit $rc: $out"
elif ! has 'Dry run: nothing was changed.' || ! has "    $prefix/lib/humanitl/bin/humanitl daemon uninstall" ||
  ! has "    rm -rf $prefix/lib/humanitl" || ! has "    rm -f $prefix/bin/humanitl-app"; then
  bad dry_run "Plan unvollstaendig: $out"
elif unchanged dry_run; then
  ok dry_run
fi
go example --dry-run --purge
if [[ "$rc" -ne 0 ]]; then
  bad dry_run_purge "Exit $rc: $out"
elif ! grep -qE "^    data +[0-9]+ KiB +$home/.local/share/humanitl\$" <<< "$out" ||
  ! grep -qE "^    config +[0-9]+ KiB +$home/.config/humanitl\$" <<< "$out"; then
  bad dry_run_purge "Orte ohne Groesse: $out"
elif unchanged dry_run_purge; then
  ok dry_run_purge
fi

# ---- Laufende Sitzung --------------------------------------------------------------

# Die Faelle, die der Mutationsbeweis wieder faehrt.
run_session_cases() {
  fresh
  with_tarball
  STUB_SESSION=running go example --yes --purge
  if [[ "$rc" -ne 0 ]] && has 'An agent session is running:' && has 'session s-1 (running, sandbox sb-42)' &&
    has 'This script ends no session.' && unchanged session_running; then
    ok session_running
  else
    bad session_running "rc=$rc calls=$(changing_calls): $out"
  fi
  STUB_SESSION=running go example --dry-run
  if [[ "$rc" -ne 0 ]] && has 'This script ends no session.' && unchanged session_running_dry; then
    ok session_running_dry
  else
    bad session_running_dry "rc=$rc: $out"
  fi
  STUB_SESSION=unknown go example --yes
  if [[ "$rc" -ne 0 ]] && has 'Could not tell whether an agent session runs' && unchanged session_unknown; then
    ok session_unknown
  else
    bad session_unknown "rc=$rc: $out"
  fi
}
run_session_cases

# ---- Archiv-Weg ----------------------------------------------------------------------

fresh
with_tarball
STUB_SESSION=nodaemon go example --yes
order="$(grep -nE '^cli daemon uninstall$' "$log" | cut -d: -f1)"
if [[ "$rc" -ne 0 ]]; then
  bad tarball "Exit $rc: $out"
elif [[ -z "$order" ]] || [[ -e "$home/.config/systemd/user/humanitld.service" ]]; then
  bad tarball "daemon uninstall lief nicht: $(cat "$log")"
elif ! tarball_gone; then
  bad tarball "Reste: $(ls -la "$prefix/bin" "$prefix/lib" 2>&1)"
elif ! data_there || ! neighbours_there; then
  bad tarball "Daten oder Nachbarn fehlen"
elif ! has 'Your data stays where it is' || ! has "$home/.local/share/humanitl"; then
  bad tarball "kein Hinweis auf die Daten: $out"
elif grep -q '^sudo' "$log"; then
  bad tarball "sudo ohne Paket: $(cat "$log")"
else
  ok tarball
fi

fresh
with_tarball
go example --yes --purge
if [[ "$rc" -eq 0 ]] && tarball_gone && [[ ! -e "$home/.config/humanitl" && ! -e "$home/.local/share/humanitl" ]] &&
  neighbours_there && has 'Project folders were not touched.'; then
  ok tarball_purge
else
  bad tarball_purge "rc=$rc config=$(ls "$home/.config") share=$(ls "$home/.local/share"): $out"
fi

# Eine andere Wurzel ueber --prefix.
fresh
prefix="$home/opt" with_tarball
go example --yes --prefix "$home/opt"
if [[ "$rc" -eq 0 ]] && [[ ! -e "$home/opt/lib/humanitl" && ! -L "$home/opt/bin/humanitl" ]]; then
  ok tarball_prefix
else
  bad tarball_prefix "rc=$rc: $out"
fi

# Ein Link, der nicht mehr in die Installation zeigt, bleibt.
fresh
with_tarball
rm "$prefix/bin/humanitld"
echo foreign > "$prefix/bin/humanitld"
go example --yes
if [[ "$rc" -eq 0 ]] && [[ "$(cat "$prefix/bin/humanitld")" == foreign ]] && [[ ! -e "$prefix/lib/humanitl" ]] &&
  has "$prefix/bin/humanitld no longer points into"; then
  ok tarball_foreign_link
else
  bad tarball_foreign_link "rc=$rc: $out"
fi

# Eine Zeile, die install.sh nie schreibt, bricht vor jeder Aenderung ab.
fresh
with_tarball
echo "$home/project" >> "$prefix/lib/humanitl/INSTALLED-FILES"
expect_refusal tarball_foreign_line "which install.sh never writes there" example --yes

# Scheitert daemon uninstall, bleibt das Programm.
fresh
with_tarball
STUB_UNINSTALL_FAIL=1 go example --yes
if [[ "$rc" -ne 0 ]] && has 'daemon uninstall failed' && [[ -x "$prefix/lib/humanitl/bin/humanitl" && -L "$prefix/bin/humanitl" ]]; then
  ok uninstall_fails
else
  bad uninstall_fails "rc=$rc: $out"
fi

# Ein Programm, das nicht laeuft: Abbruch vor jeder Aenderung, mit dem Weg,
# es zum Laufen zu bringen; sonst bliebe der Dienst angemeldet.
fresh
with_tarball
STUB_CLI_BROKEN=1 expect_refusal cli_broken 'does not run (' example --yes

# ---- --purge, wenn das Programm die Orte nicht nennen kann oder will ------------------

fresh
with_tarball
STUB_PATHS=old expect_refusal purge_old_program '--purge needs the program to name your data locations' example --yes --purge
STUB_PATHS=evil expect_refusal purge_evil_location 'is not one this script deletes from' example --yes --purge
STUB_CLI_BROKEN=1 expect_refusal purge_cli_broken 'does not run (' example --yes --purge
# Ein Zeilenumbruch im Pfad macht aus einer Zeile zwei; doppelte, fehlende
# und unbekannte Arten brechen --purge vor jeder Aenderung ab.
STUB_PATHS=split expect_refusal purge_split_row "names an unknown kind '$home/project/humanitl'" example --yes --purge
STUB_PATHS=dup expect_refusal purge_duplicate_kind 'names the kind data twice' example --yes --purge
STUB_PATHS=missing expect_refusal purge_missing_kind 'names no location of the kind data' example --yes --purge
expect_refusal prefix_root '--prefix / is no place install.sh puts an archive' example --yes --prefix /

# Ein Ort, der dem Namen nach passt, aber ueber einen Link das HOME selbst
# ist (HOME endet auf humanitl, alias zeigt auf dessen Elternordner): Abbruch
# vor jeder Aenderung, HOME bleibt.
run_alias_case() {
  local saved_home="$home"
  home="$work/victim/humanitl"
  prefix="$home/.local"
  fresh
  with_tarball
  ln -s "$work/victim" "$home/alias"
  STUB_PATHS=alias go example --yes --purge
  if [[ "$rc" -ne 0 ]] && has 'is really' && [[ -f "$home/project/src/main.rs" && -f "$home/.config/humanitl/config.toml" ]] &&
    unchanged purge_alias_home; then
    ok purge_alias_home
  else
    bad purge_alias_home "rc=$rc home=$(ls -A "$home" 2> /dev/null | tr '\n' ' '): $out"
  fi
  rm -rf "$work/victim"
  home="$saved_home"
  prefix="$home/.local"
}
run_alias_case

# Fuehrt ein Ort beim Loeschen woanders hin als in der Auflistung, bleibt
# alles stehen, was er nennt.
fresh
with_tarball
STUB_SWAP=1 go example --yes --purge
if [[ "$rc" -ne 0 ]] && has 'not to' && has 'None of your data was deleted.' && [[ -f "$home/elsewhere/humanitl/file" ]] &&
  [[ -f "$home/.config/humanitl.moved/config.toml" && -f "$home/.local/share/humanitl/humanitl.db" ]]; then
  ok purge_location_moved
else
  bad purge_location_moved "rc=$rc: $out"
fi

# Ein Ort, der ein Link auf ein echtes humanitl-Verzeichnis ist: geloescht
# wird das echte Verzeichnis und der Link, der Ordner daneben bleibt.
fresh
with_tarball
mkdir -p "$home/dotfiles/humanitl" && echo x > "$home/dotfiles/humanitl/config.toml" && echo keep > "$home/dotfiles/README"
rm -rf "$home/.config/humanitl" && ln -s "$home/dotfiles/humanitl" "$home/.config/humanitl"
go example --yes --purge
if [[ "$rc" -eq 0 ]] && has "really $home/dotfiles/humanitl" && [[ ! -e "$home/dotfiles/humanitl" && ! -L "$home/.config/humanitl" ]] &&
  [[ -f "$home/dotfiles/README" ]] && neighbours_there; then
  ok purge_linked_location
else
  bad purge_linked_location "rc=$rc: $out"
fi

# ---- Mit Terminal ----------------------------------------------------------------------

# Nein auf die erste Frage: nichts geaendert.
fresh
with_tarball
go_tty 'n\n' example
if [[ "$rc" -ne 0 ]] && has 'Stopped. Nothing was changed.' && unchanged answer_no; then
  ok answer_no
else
  bad answer_no "rc=$rc: $out"
fi

# Ja zum Dienst, ja zum Programm, nein zum Loeschen der Daten: Die Daten bleiben.
run_purge_declined() {
  fresh
  with_tarball
  go_tty 'y\ny\nn\n' example --purge
  if [[ "$rc" -eq 0 ]] && has 'Delete these locations for good? [y/N]' && tarball_gone && data_there && neighbours_there &&
    has 'Your data stays.'; then
    ok purge_declined
  else
    bad purge_declined "rc=$rc data_there=$(data_there && echo ja || echo nein): $out"
  fi
}
run_purge_declined

fresh
with_tarball
go_tty 'y\ny\ny\n' example --purge
if [[ "$rc" -eq 0 ]] && tarball_gone && [[ ! -e "$home/.local/share/humanitl" ]] && neighbours_there; then
  ok purge_tty_yes
else
  bad purge_tty_yes "rc=$rc: $out"
fi

# ---- Paketwege -----------------------------------------------------------------------

fresh
with_package
STUB_DPKG=1 go debian --yes
want="cli daemon uninstall
sudo sh -c apt-get remove --purge -y \"\$1\" && rm -f /etc/apt/sources.list.d/nurkert.list /usr/share/keyrings/nurkert-archive-keyring.gpg sh humanitl"
if [[ "$rc" -eq 0 ]] && [[ "$(changing_calls)" == "$want" ]] && data_there && has 'APT package humanitl (dpkg)'; then
  ok apt
else
  bad apt "rc=$rc calls=$(changing_calls): $out"
fi
# Im Terminal: nein zur Quelle, das Paket geht trotzdem.
fresh
with_package
STUB_DPKG=1 go_tty 'y\ny\nn\n' debian
if [[ "$rc" -eq 0 ]] && grep -qx 'sudo apt-get remove --purge -y humanitl' "$log" && ! grep -q 'nurkert.list' "$log" &&
  has 'The source and the key stay.'; then
  ok apt_keep_source
else
  bad apt_keep_source "rc=$rc log=$(cat "$log"): $out"
fi
# Der Plan nennt sudo vor dem Paketschritt.
fresh
with_package
STUB_DPKG=1 go debian --dry-run
if [[ "$rc" -eq 0 ]] && grep -qx '    sudo apt-get remove --purge -y humanitl' <<< "$out" && unchanged apt_dry; then
  ok apt_dry
else
  bad apt_dry "rc=$rc: $out"
fi

fresh
with_package
STUB_RPM=1 go fedora --yes
if [[ "$rc" -eq 0 ]] && [[ "$(changing_calls)" == "cli daemon uninstall
sudo dnf remove -y humanitl" ]]; then
  ok rpm_dnf
else
  bad rpm_dnf "rc=$rc calls=$(changing_calls): $out"
fi
fresh
with_package
STUB_RPM=1 go opensuse-tumbleweed --yes
if [[ "$rc" -eq 0 ]] && [[ "$(changing_calls)" == "cli daemon uninstall
sudo zypper --non-interactive remove humanitl" ]]; then
  ok rpm_zypper
else
  bad rpm_zypper "rc=$rc calls=$(changing_calls): $out"
fi

fresh
with_package
STUB_PACMAN=humanitl-bin EXTRA_PATH="$aur_stubs" go arch --yes
if [[ "$rc" -eq 0 ]] && [[ "$(changing_calls)" == "cli daemon uninstall
yay -R --noconfirm humanitl-bin" ]]; then
  ok aur_helper
else
  bad aur_helper "rc=$rc calls=$(changing_calls): $out"
fi
fresh
with_package
STUB_PACMAN=humanitl go arch --yes
if [[ "$rc" -eq 0 ]] && [[ "$(changing_calls)" == "cli daemon uninstall
sudo pacman -R --noconfirm humanitl" ]]; then
  ok aur_pacman
else
  bad aur_pacman "rc=$rc calls=$(changing_calls): $out"
fi

# Zwei Wege zugleich: beide genannt, beide entfernt, der Dienst einmal.
fresh
with_package
with_tarball
STUB_DPKG=1 go debian --yes
if [[ "$rc" -eq 0 ]] && has 'APT package humanitl (dpkg)' && has "archive in $prefix/lib/humanitl" && tarball_gone &&
  grep -q 'apt-get remove --purge -y' "$log" && [[ "$(grep -c '^cli daemon uninstall$' "$log")" -eq 1 ]] &&
  [[ "$(grep -c '^sudo' "$log")" -eq 1 ]]; then
  ok two_ways
else
  bad two_ways "rc=$rc log=$(cat "$log"): $out"
fi

# ---- macOS, nachgestellt ---------------------------------------------------------------

mac() { STUB_UNAME_S=Darwin STUB_UNAME_M=arm64 "$@"; }
fresh
with_app
mac go example --yes
if [[ "$rc" -eq 0 ]] && [[ ! -e "$app" ]] && grep -qx 'cli daemon uninstall' "$log" && data_there && has "$app"; then
  ok macos
else
  bad macos "rc=$rc log=$(cat "$log"): $out"
fi
fresh
with_app
mac go example --dry-run
if [[ "$rc" -eq 0 ]] && has "    rm -rf $app" && unchanged macos_dry; then ok macos_dry; else bad macos_dry "rc=$rc: $out"; fi
fresh
with_app
mac expect_refusal macos_method 'is for Linux; on macOS only dmg' example --yes --method apt

# ---- Mutationsbeweise --------------------------------------------------------------------

if [[ -z "${UNINSTALL_SH:-}" ]]; then
  # Ohne die Pruefung auf laufende Sitzungen muss der Fall rot werden.
  mutant="$work/uninstall-no-session-check.sh"
  sed -e 's/^  check_sessions$/  :/' "$script" > "$mutant"
  if [[ "$(diff "$script" "$mutant" | grep -c '^[<>]')" -ne 4 ]]; then
    bad mutation_session "die Pruefung steht nicht mehr so im Skript; Mutant passt nicht"
  else
    before_fails=$fails
    saved="$script"
    script="$mutant"
    run_session_cases > "$work/mutant-session.out"
    script="$saved"
    fails=$before_fails
    red=$(grep -c '^FAIL session_running:' "$work/mutant-session.out" || true)
    if [[ "$red" -eq 1 ]]; then
      ok "mutation_session (ohne die Pruefung entfernt das Skript trotz laufender Sitzung)"
    else
      bad mutation_session "der Fall bleibt gruen ohne die Pruefung: $(cat "$work/mutant-session.out")"
    fi
  fi
  # Ohne die Aufloesung der Links geht --purge durch den Link ins HOME.
  mutant="$work/uninstall-no-resolution.sh"
  sed -e 's/^  phys=\$(cd -P -- "\$2" 2>\/dev\/null \&\& pwd -P) || purge_stop .*$/  phys=$2/' "$script" > "$mutant"
  if cmp -s "$script" "$mutant"; then
    bad mutation_resolution "die Aufloesung steht nicht mehr so im Skript; Mutant passt nicht"
  else
    before_fails=$fails
    saved="$script"
    script="$mutant"
    run_alias_case > "$work/mutant-alias.out"
    script="$saved"
    fails=$before_fails
    if grep -q '^FAIL purge_alias_home' "$work/mutant-alias.out"; then
      ok "mutation_resolution (ohne Aufloesung loescht --purge durch den Link im HOME)"
    else
      bad mutation_resolution "der Fall bleibt gruen ohne die Aufloesung: $(cat "$work/mutant-alias.out")"
    fi
  fi
  # Ohne die Rueckfrage vor dem Loeschen loescht auch die Antwort n.
  mutant="$work/uninstall-no-purge-question.sh"
  sed -e 's/^  if ! ask "Delete these locations for good?"; then$/  if false; then/' "$script" > "$mutant"
  if cmp -s "$script" "$mutant"; then
    bad mutation_purge "die Rueckfrage steht nicht mehr so im Skript; Mutant passt nicht"
  else
    before_fails=$fails
    saved="$script"
    script="$mutant"
    run_purge_declined > "$work/mutant-purge.out"
    script="$saved"
    fails=$before_fails
    if grep -q '^FAIL purge_declined' "$work/mutant-purge.out"; then
      ok "mutation_purge (ohne die Rueckfrage sind die Daten nach der Antwort n weg)"
    else
      bad mutation_purge "der Fall bleibt gruen ohne die Rueckfrage: $(cat "$work/mutant-purge.out")"
    fi
  fi
fi

if [[ "$fails" -ne 0 ]]; then
  echo "uninstall_sh_test: $fails Fehler"
  exit 1
fi
echo "uninstall_sh_test: alles gruen"
