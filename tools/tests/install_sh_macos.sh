#!/usr/bin/env bash
# install.sh auf einem macOS-Runner (HUM-528), gegen ein nachgestelltes .dmg.
#
# Aufruf: tools/tests/install_sh_macos.sh
#
# Das Disk-Image ist ein Stellvertreter, mit hdiutil gebaut: Humanitl.app mit
# Info.plist und einem `bin/humanitl`, das seine Version nennt. Es liegt mit
# SHA256SUMS in einem Verzeichnis, das install.sh ueber
# HUMANITL_RELEASES_URL=file://... liest. So prueft der Test den Weg des
# Skripts (laden, pruefen, einhaengen, kopieren, markieren, ersetzen), nicht
# Gatekeeper selbst, und braucht kein echtes Release.
#
# Geprueft wird:
#   - --dry-run: nichts geaendert, kein eingehaengtes Abbild
#   - Installation: Humanitl.app in /Applications (oder ~/Applications),
#     jede Datei des Buendels traegt com.apple.quarantine, die Kopie meldet
#     ihre Version, kein Abbild bleibt eingehaengt
#   - ein zweiter Lauf ersetzt die App und markiert auch die neue
#   - eine falsche Pruefsumme bricht ab, die installierte App bleibt
#   - mit --yes laeuft nichts aus der App, das Skript nennt die Einrichtung
#     als ausstehend; im Terminal (expect) wartet es auf Enter und bietet
#     erst dann `daemon install` und `sandbox check` an
#
# Laeuft nur auf macOS und nur dort, wo eine App in /Applications ersetzt
# werden darf (ein Wegwerf-Runner); vorhandene Humanitl.app wird geloescht.
# Muss mit dem bash 3.2 von macOS laufen.
#
# Jeder Fehler geht zusaetzlich als GitHub-Annotation (`::error`) nach stdout,
# mit Erwartung, Befund und den letzten Zeilen der Skriptausgabe: Die
# oeffentliche Actions-API liefert keine Logs, aber Annotationen.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
script="${INSTALL_SH:-$here/../../install.sh}"

[[ "$(uname -s)" == Darwin ]] || { echo "install_sh_macos: macOS only" >&2; exit 2; }
[[ "${CI:-}" == true || "${INSTALL_MACOS_DISPOSABLE:-}" == 1 ]] || {
  echo "install_sh_macos: replaces Humanitl.app; set INSTALL_MACOS_DISPOSABLE=1 outside CI" >&2
  exit 2
}

work="$(mktemp -d "${TMPDIR:-/tmp}/install-sh-macos.XXXXXX")"
fails=0
out=
annotated=0
err_at=
# annotate <text>: eine Annotation; Zeilenumbrueche und % nach der Kodierung
# der Workflow-Befehle, damit der ganze Text in einer Zeile ankommt.
annotate() {
  local msg="$1"
  msg="${msg//%/%25}"
  msg="${msg//$'\r'/%0D}"
  msg="${msg//$'\n'/%0A}"
  echo "::error title=install-macos::$msg"
  annotated=1
}
ok() { echo "ok   $1"; }
# bad <case> <expected vs got>: Fehler zaehlen, melden, mit dem Ende der
# letzten Skriptausgabe.
bad() {
  echo "FAIL $1: $2"
  annotate "$1: $2
--- last lines of the install.sh output ---
$(printf '%s\n' "$out" | tail -n 25)"
  fails=$((fails + 1))
}
# Bricht set -e das Skript ab, sagt eine Annotation, woran: die Zeile, wenn
# der Befehl auf oberster Ebene fiel, sonst der letzte Befehl.
trap 'err_at="line $LINENO: $BASH_COMMAND"' ERR
on_exit() {
  local rc=$? last="$BASH_COMMAND"
  if [[ "$rc" -ne 0 && "$annotated" -eq 0 ]]; then
    annotate "aborted with exit $rc at ${err_at:-$last}
--- last lines of the install.sh output ---
$(printf '%s\n' "$out" | tail -n 25)"
  fi
  rm -rf "$work"
}
trap on_exit EXIT

# Was auf dem Runner anders sein kann als gedacht, einmal sichtbar.
echo "::group::runner"
sw_vers || true
id
ls -ld /Applications "$HOME/Applications" 2> /dev/null || true
command -v expect hdiutil ditto xattr || true
echo "::endgroup::"
command -v expect > /dev/null || { annotate "expect is missing on this runner"; exit 1; }

if [[ -w /Applications ]]; then apps=/Applications; else apps="$HOME/Applications"; fi
app="$apps/Humanitl.app"
rm -rf "$app"
leftovers() { ls -d "$apps"/.Humanitl.app.* 2> /dev/null || true; }

rel="$work/releases"
# image <version>: Humanitl-<version>-macos.dmg und SHA256SUMS
image() {
  local v="$1" src="$work/src-$1" dir="$rel/download/v$1"
  mkdir -p "$src/Humanitl.app/Contents/MacOS/bin" "$src/Humanitl.app/Contents/Resources/share" "$dir"
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
  printf '#!/bin/sh\necho "$*" >> "%s"\necho "humanitl %s"\n' "$work/cli.log" "$v" > "$src/Humanitl.app/Contents/MacOS/bin/humanitl"
  chmod +x "$src/Humanitl.app/Contents/MacOS/Humanitl" "$src/Humanitl.app/Contents/MacOS/bin/humanitl"
  echo catalog > "$src/Humanitl.app/Contents/Resources/share/domains.yaml"
  ln -s ../Resources/share "$src/Humanitl.app/Contents/MacOS/share"
  # hdiutil create scheitert auf CI-Runnern gelegentlich mit "Resource busy";
  # drei Versuche, der Fehler des letzten geht in die Annotation.
  local try_n=1 err
  until err="$(hdiutil create -quiet -volname Humanitl -srcfolder "$src" -format UDZO -ov "$dir/Humanitl-$v-macos.dmg" 2>&1)"; do
    if [[ "$try_n" -ge 3 ]]; then
      annotate "hdiutil create failed 3 times for $v: $err"
      exit 1
    fi
    try_n=$((try_n + 1))
    sleep 5
  done
  (cd "$dir" && shasum -a 256 "Humanitl-$v-macos.dmg" > SHA256SUMS)
}
image 9.9.9
image 9.9.10
image 9.9.11
echo tampered >> "$rel/download/v9.9.11/Humanitl-9.9.11-macos.dmg"

go() {
  rc=0
  out="$(HUMANITL_RELEASES_URL="file://$rel" TMPDIR="$work/tmp" sh "$script" "$@" 2>&1 < /dev/null)" || rc=$?
  echo "::group::install.sh $* (exit $rc)"
  echo "$out"
  echo "::endgroup::"
}
mkdir -p "$work/tmp"
# Ein Abbild unter $work/tmp eingehaengt? Ohne Pipe in grep -q (pipefail und
# SIGPIPE), und auch unter dem aufgeloesten Pfad (/var ist /private/var).
work_real="$(cd -P "$work" && pwd)"
mounted() {
  local info
  info="$(hdiutil info 2> /dev/null || true)"
  [[ "$info" == *"$work/tmp"* || "$info" == *"$work_real/tmp"* ]]
}

# quarantine_report <bundle>: prueft, dass jede Datei und jedes Verzeichnis
# des Buendels (ohne Symlinks) com.apple.quarantine mit der Kennung
# humanitl-install traegt. Die Flags (erstes Feld) darf macOS aendern, etwa
# beim Verschieben oder bei der ersten Pruefung; verlangt sind nur ein
# vorhandener Wert der Form <hex>;<hex>;humanitl-install;... auf jeder Datei.
# Ohne Pipelines und ohne Befehlsersetzung um die ganze Pruefung, damit kein
# pipefail- oder set-e-Effekt das Ergebnis verdeckt. Ergebnis in
# $q_report (Zeilen "Pfad: Wert (xattr exit N)"), $q_count (Dateien) und
# $q_bad (abweichende); Rueckgabe 0 nur bei q_count > 0 und q_bad = 0.
q_report=
q_count=0
q_bad=0
quarantine_report() {
  local list="$work/quarantine-files" f q qrc
  # In einer Variablen, damit bash 3.2 und 5 den Ausdruck gleich lesen.
  local q_re='^[0-9a-fA-F]+;[0-9a-fA-F]+;humanitl-install;'
  q_report=
  q_count=0
  q_bad=0
  if ! find "$1" ! -type l > "$list" 2> "$list.err"; then
    q_report="find $1 failed: $(cat "$list.err")"
  fi
  while IFS= read -r f; do
    q_count=$((q_count + 1))
    qrc=0
    q="$(xattr -p com.apple.quarantine "$f" 2>&1)" || qrc=$?
    if [[ "$qrc" -eq 0 && "$q" =~ $q_re ]]; then
      continue
    fi
    q_bad=$((q_bad + 1))
    if [[ "$q_bad" -le 8 ]]; then
      q_report="$q_report
Humanitl.app${f#"$1"}: '$q' (xattr exit $qrc)"
    fi
  done < "$list"
  if [[ "$q_count" -eq 0 ]]; then
    q_report="no files found in $1 (entries: $(listing "$1"))$q_report"
    return 1
  fi
  [[ "$q_bad" -eq 0 && -z "$q_report" ]]
}

# listing <dir>: die Eintraege in einer Zeile, fuer Meldungen.
listing() { find "$1" -mindepth 1 -maxdepth 1 -exec basename {} \; 2> /dev/null | tr '\n' ' '; }

# Die Version, die die installierte Kopie nennt (ueber sh, also ohne
# Gatekeeper), oder "<none>".
cli_version() { sh "$app/Contents/MacOS/bin/humanitl" --version 2> /dev/null || echo "<none>"; }

# --dry-run
go --dry-run --version 9.9.9
why=
[[ "$rc" -eq 0 ]] || why="expected exit 0, got $rc"
[[ -n "$why" || ! -e "$app" ]] || why="expected no $app, but it exists"
[[ -n "$why" ]] || ! mounted || why="expected no mounted image, hdiutil info lists one under $work/tmp"
[[ -n "$why" || -z "$(ls -A "$work/tmp")" ]] || why="expected an empty TMPDIR, got: $(listing "$work/tmp")"
[[ -n "$why" ]] || grep -q 'Dry run: nothing was changed' <<< "$out" || why="expected 'Dry run: nothing was changed' in the output"
[[ -n "$why" ]] || grep -q 'xattr -r -w com.apple.quarantine' <<< "$out" || why="expected the plan to show 'xattr -r -w com.apple.quarantine'"
if [[ -z "$why" ]]; then ok dry_run; else bad dry_run "$why"; fi

# Installation, mit --yes und ohne --no-setup: Aus der App laeuft nichts.
go --yes --version 9.9.9
ran="$(cat "$work/cli.log" 2> /dev/null || true)"
why=
if [[ "$rc" -ne 0 ]]; then
  why="expected exit 0, got $rc"
elif [[ ! -d "$app" ]]; then
  why="expected $app, it is missing ($apps: $(listing "$apps"))"
elif ! quarantine_report "$app"; then
  why="expected com.apple.quarantine '<flags>;<hex>;humanitl-install;' on every file, $q_bad of $q_count differ:$q_report"
elif [[ -n "$ran" ]]; then
  why="expected no call of the app with --yes, got: $ran"
elif [[ "$(cli_version)" != "humanitl 9.9.9" ]]; then
  why="expected 'humanitl 9.9.9' from the copy, got '$(cli_version)'"
elif mounted; then
  why="expected no mounted image, hdiutil info lists one under $work/tmp"
elif [[ -n "$(ls -A "$work/tmp")" || -n "$(leftovers)" ]]; then
  why="expected no leftovers, got: $(listing "$work/tmp") $(leftovers | tr '\n' ' ')"
elif ! grep -q 'Open Anyway' <<< "$out"; then
  why="expected the hint 'Open Anyway' in the output"
elif ! grep -q 'Setup is pending' <<< "$out"; then
  why="expected 'Setup is pending' in the output"
fi
if [[ -z "$why" ]]; then
  ok "install ($q_count files marked, e.g. $(xattr -p com.apple.quarantine "$app" 2> /dev/null || true))"
else
  bad install "$why"
fi

# Ersetzen
go --yes --no-setup --version 9.9.10
why=
if [[ "$rc" -ne 0 ]]; then
  why="expected exit 0, got $rc"
elif ! grep -q 'it will be replaced' <<< "$out"; then
  why="expected 'it will be replaced' in the output"
elif ! quarantine_report "$app"; then
  why="expected com.apple.quarantine '<flags>;<hex>;humanitl-install;' on every file of the new app, $q_bad of $q_count differ:$q_report"
elif [[ "$(cli_version)" != "humanitl 9.9.10" ]]; then
  why="expected 'humanitl 9.9.10' from the copy, got '$(cli_version)'"
elif [[ -n "$(leftovers)" ]]; then
  why="expected no staging directories in $apps, got: $(leftovers | tr '\n' ' ')"
fi
if [[ -z "$why" ]]; then ok replace; else bad replace "$why"; fi

# Falsche Pruefsumme: Abbruch, die App von eben bleibt.
go --yes --no-setup --version 9.9.11
why=
if [[ "$rc" -eq 0 ]]; then
  why="expected a non-zero exit, got 0"
elif ! grep -q 'checksum mismatch' <<< "$out"; then
  why="expected 'checksum mismatch' in the output (exit $rc)"
elif [[ "$(cli_version)" != "humanitl 9.9.10" ]]; then
  why="expected the app from before ('humanitl 9.9.10'), got '$(cli_version)'"
elif mounted; then
  why="expected no mounted image, hdiutil info lists one under $work/tmp"
fi
if [[ -z "$why" ]]; then ok checksum_wrong; else bad checksum_wrong "$why"; fi

# Im Terminal: Abfrage, nicht oeffnen, Enter, dann ja zu beidem. Startet die
# Kopie nicht (Gatekeeper auf dem Runner), fragt das Skript ein zweites Mal
# und nennt die Einrichtung dann als ausstehend; beides zaehlt, nur nie eine
# Einrichtung vor dem Enter.
: > "$work/cli.log"
cat > "$work/tty.exp" << 'EOF'
set timeout 120
proc stuck {what} { puts "\nEXPECT-TIMEOUT waiting for: $what"; exit 98 }
proc ended {what} { puts "\nEXPECT-EOF before: $what"; exit 97 }
spawn {*}$argv
expect {
  "Go ahead?" { send "y\r" }
  timeout { stuck "Go ahead?" }
  eof { ended "Go ahead?" }
}
expect {
  "Open Humanitl now?" { send "n\r" }
  timeout { stuck "Open Humanitl now?" }
  eof { ended "Open Humanitl now?" }
}
expect {
  "press Enter" { send "\r" }
  timeout { stuck "press Enter" }
  eof { ended "press Enter" }
}
expect {
  timeout { stuck "setup questions or end" }
  "daemon install)?" { send "y\r"; exp_continue }
  "sandbox check)?" { send "y\r"; exp_continue }
  "press Enter again" { send "\r"; exp_continue }
  "press Enter" { send "\r"; exp_continue }
  eof
}
catch wait result
exit [lindex $result 3]
EOF
rc=0
out="$(HUMANITL_RELEASES_URL="file://$rel" TMPDIR="$work/tmp" expect "$work/tty.exp" sh "$script" --version 9.9.9 2>&1)" || rc=$?
echo "::group::install.sh in a terminal via expect (exit $rc)"
echo "$out"
echo "::endgroup::"
log="$(tr '\n' '|' < "$work/cli.log")"
if [[ "$rc" -ne 0 ]]; then
  bad tty "expected exit 0, got $rc (97: ended early, 98: expect timed out)"
elif [[ "$log" == "--version|--version|daemon install|sandbox check|" ]]; then
  ok "tty (Enter, dann Einrichtung)"
elif [[ -z "$log" ]] && grep -q 'Setup is pending' <<< "$out" && grep -q 'macOS still blocks' <<< "$out"; then
  ok "tty (Gatekeeper sperrt die Kopie: zweimal Enter, dann ausstehend)"
else
  bad tty "expected app calls '--version|--version|daemon install|sandbox check|' or none plus 'Setup is pending' and 'macOS still blocks', got '$log'"
fi

rm -rf "$app"
if [[ "$fails" -ne 0 ]]; then
  echo "install_sh_macos: $fails Fehler"
  exit 1
fi
echo "install_sh_macos: alles gruen"
