#!/usr/bin/env bash
# uninstall.sh auf einem macOS-Runner (HUM-550), gegen eine App, die install.sh
# aus einem nachgestellten .dmg installiert hat.
#
# Aufruf: tools/tests/uninstall_sh_macos.sh
#
# Das Disk-Image ist ein Stellvertreter wie in install_sh_macos.sh. Sein
# `bin/humanitl` spielt den Vertrag, den uninstall.sh liest: `--json daemon
# status` mit `agent_session` (running, solange $work/session-running da
# ist), `sessions`, `config paths` mit den vier Orten unter macOS, und
# `daemon uninstall`, das die Datei des launchd-Agenten unter
# ~/Library/LaunchAgents entfernt. Jeder Aufruf landet in $work/cli.log. HOME
# zeigt in einen Wegwerf-Baum; die App landet, wo install.sh sie hinlegt
# (/Applications oder ~/Applications).
#
# Geprueft wird:
#   - --dry-run: nichts geaendert
#   - eine laufende Sitzung: Abbruch, App und Agent bleiben
#   - --yes: App und Agent weg, `daemon uninstall` lief vor dem Entfernen der
#     App, die Daten bleiben
#   - neu installiert, --purge --yes: die vier Orte weg, ein Projektordner
#     und ein fremder Ordner daneben bleiben
# Laesst macOS das Programm in der App nicht laufen (Gatekeeper), prueft der
# Test den Weg ohne Programm: Abbruch vor jeder Aenderung mit dem Hinweis auf
# "Open Anyway", App und Agent bleiben (sonst bliebe der Agent angemeldet).
#
# Laeuft nur auf macOS und nur auf einem Wegwerf-Runner; vorhandene
# Humanitl.app wird geloescht. Muss mit dem bash 3.2 von macOS laufen.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
installer="$here/../../install.sh"
script="${UNINSTALL_SH:-$here/../../uninstall.sh}"

[[ "$(uname -s)" == Darwin ]] || { echo "uninstall_sh_macos: macOS only" >&2; exit 2; }
[[ "${CI:-}" == true || "${INSTALL_MACOS_DISPOSABLE:-}" == 1 ]] || {
  echo "uninstall_sh_macos: replaces Humanitl.app; set INSTALL_MACOS_DISPOSABLE=1 outside CI" >&2
  exit 2
}

work="$(mktemp -d "${TMPDIR:-/tmp}/uninstall-sh-macos.XXXXXX")"
home="$work/home"
mkdir -p "$home" "$work/tmp"
fails=0
out=
ok() { echo "ok   $1"; }
bad() {
  echo "FAIL $1: $2"
  local msg="$1: $2
--- last lines of the output ---
$(printf '%s\n' "$out" | tail -n 25)"
  msg="${msg//%/%25}"
  msg="${msg//$'\n'/%0A}"
  echo "::error title=uninstall-macos::$msg"
  fails=$((fails + 1))
}
trap 'rm -rf "$work"' EXIT

if [[ -w /Applications ]]; then apps=/Applications; else apps="$home/Applications"; fi
app="$apps/Humanitl.app"
rm -rf "$app" "$HOME/Applications/Humanitl.app"
plist="$home/Library/LaunchAgents/dev.humanitl.humanitld.plist"

# ---- Das Disk-Image ----------------------------------------------------------------

v=9.9.9
src="$work/src"
rel="$work/releases/download/v$v"
mkdir -p "$src/Humanitl.app/Contents/MacOS/bin" "$rel"
cat > "$src/Humanitl.app/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>dev.humanitl.test</string>
<key>CFBundleName</key><string>Humanitl</string>
<key>CFBundleExecutable</key><string>Humanitl</string>
<key>CFBundleShortVersionString</key><string>$v</string>
</dict></plist>
EOF
printf '#!/bin/sh\necho "Humanitl %s"\n' "$v" > "$src/Humanitl.app/Contents/MacOS/Humanitl"
cat > "$src/Humanitl.app/Contents/MacOS/bin/humanitl" << EOF
#!/bin/sh
echo "\$*" >> "$work/cli.log"
case "\$*" in
  --version) echo "humanitl $v" ;;
  "--json daemon status")
    if [ -e "$work/session-running" ]; then s=running; else s=none; fi
    echo '{"agent_session":"'"\$s"'"}'
    ;;
  sessions) echo "session s-1 (running, sandbox sb-7)" ;;
  "daemon uninstall") rm -f "$plist" && echo "removed $plist" ;;
  "config paths")
    printf 'NAME     PATH\\n'
    printf 'config   %s\\n' "\$HOME/.config/humanitl"
    printf 'data     %s\\n' "\$HOME/.local/share/humanitl"
    printf 'support  %s\\n' "\$HOME/Library/Application Support/Humanitl"
    printf 'logs     %s\\n' "\$HOME/Library/Logs/Humanitl"
    ;;
  *) exit 1 ;;
esac
EOF
chmod +x "$src/Humanitl.app/Contents/MacOS/Humanitl" "$src/Humanitl.app/Contents/MacOS/bin/humanitl"
try_n=1
until err="$(hdiutil create -quiet -volname Humanitl -srcfolder "$src" -format UDZO -ov "$rel/Humanitl-$v-macos.dmg" 2>&1)"; do
  if [[ "$try_n" -ge 3 ]]; then
    echo "::error title=uninstall-macos::hdiutil create failed 3 times: $err"
    exit 1
  fi
  try_n=$((try_n + 1))
  sleep 5
done
(cd "$rel" && shasum -a 256 "Humanitl-$v-macos.dmg" > SHA256SUMS)

# ---- Ablauf ------------------------------------------------------------------------

# run_sh <skript> <argumente...>: mit HOME und TMPDIR des Tests, ohne Terminal.
run_sh() {
  local file="$1"
  shift
  rc=0
  out="$(HOME="$home" TMPDIR="$work/tmp" HUMANITL_RELEASES_URL="file://$work/releases" sh "$file" "$@" 2>&1 < /dev/null)" || rc=$?
  echo "::group::$(basename "$file") $* (exit $rc)"
  echo "$out"
  echo "::endgroup::"
}

locations=("$home/.config/humanitl" "$home/.local/share/humanitl" "$home/Library/Application Support/Humanitl" "$home/Library/Logs/Humanitl")
setup() {
  run_sh "$installer" --yes --no-setup --version "$v"
  [[ "$rc" -eq 0 && -d "$app" ]] || { bad install "install.sh did not install the stand-in app"; exit 1; }
  mkdir -p "$(dirname "$plist")" "$home/project/.humanitl" "$home/Library/Application Support/Other"
  echo '<plist/>' > "$plist"
  local location
  for location in "${locations[@]}"; do
    mkdir -p "$location"
    echo data > "$location/file"
  done
  echo keep > "$home/project/README"
  echo keep > "$home/project/.humanitl/profile.toml"
  echo keep > "$home/Library/Application Support/Other/file"
  : > "$work/cli.log"
}
data_there() {
  local location
  for location in "${locations[@]}"; do [[ -f "$location/file" ]] || return 1; done
}
data_gone() {
  local location
  for location in "${locations[@]}"; do [[ ! -e "$location" ]] || return 1; done
}
neighbours_there() {
  [[ -f "$home/project/README" && -f "$home/project/.humanitl/profile.toml" && -f "$home/Library/Application Support/Other/file" ]]
}

setup
runs=1
"$app/Contents/MacOS/bin/humanitl" --version > /dev/null 2>&1 || runs=0
: > "$work/cli.log"
echo "the command line in the quarantined app runs directly: $runs"

run_sh "$script" --dry-run --purge
if [[ "$rc" -eq 0 && -d "$app" && -f "$plist" ]] && data_there && grep -q 'Dry run: nothing was changed' <<< "$out"; then
  ok dry_run
else
  bad dry_run "expected exit 0, app, agent and data untouched (exit $rc)"
fi

if [[ "$runs" == 1 ]]; then
  touch "$work/session-running"
  run_sh "$script" --yes
  rm -f "$work/session-running"
  if [[ "$rc" -ne 0 && -d "$app" && -f "$plist" ]] && grep -q 'This script ends no session' <<< "$out"; then
    ok session_running
  else
    bad session_running "expected a stop with app and agent in place (exit $rc)"
  fi

  : > "$work/cli.log"
  run_sh "$script" --yes
  if [[ "$rc" -ne 0 ]]; then
    bad uninstall "expected exit 0, got $rc"
  elif [[ -e "$app" || -e "$plist" ]]; then
    bad uninstall "expected app and agent gone; app: $([[ -e "$app" ]] && echo there || echo gone), agent: $([[ -e "$plist" ]] && echo there || echo gone)"
  elif ! grep -qx 'daemon uninstall' "$work/cli.log"; then
    bad uninstall "daemon uninstall did not run: $(tr '\n' '|' < "$work/cli.log")"
  elif ! data_there || ! neighbours_there; then
    bad uninstall "expected the data to stay without --purge"
  else
    ok uninstall
  fi

  setup
  run_sh "$script" --yes --purge
  if [[ "$rc" -eq 0 && ! -e "$app" ]] && data_gone && neighbours_there; then
    ok purge
  else
    bad purge "expected app and the four locations gone, project and neighbour there (exit $rc)"
  fi
else
  # Gatekeeper laesst das Programm nicht laufen: Abbruch vor jeder Aenderung,
  # App und Agent bleiben, der Hinweis nennt "Open Anyway".
  run_sh "$script" --yes
  if [[ "$rc" -ne 0 && -d "$app" && -f "$plist" ]] && data_there && grep -q 'Open Anyway' <<< "$out"; then
    ok stop_without_program
  else
    bad stop_without_program "expected a stop with app, agent and data in place and the hint 'Open Anyway' (exit $rc)"
  fi
fi

rm -rf "$app"
if [[ "$fails" -ne 0 ]]; then
  echo "uninstall_sh_macos: $fails Fehler"
  exit 1
fi
echo "uninstall_sh_macos: alles gruen"
