#!/usr/bin/env bash
# Installiert das .deb in einem Wegwerf-Container, prueft es und entfernt es
# wieder. Nie auf einem echten Rechner ausfuehren: Das Skript installiert als
# root und entfernt danach mit `--purge`.
#
# Aufruf (als root, im Container): check-install.sh <deb> <0.0.N>
#
# Geprueft wird, was ein Mensch mit `sudo apt install ./humanitl_…deb` erlebt:
#   1. apt loest die Abhaengigkeiten aus dem Archiv auf (sonst stimmt Depends nicht);
#   2. kein Programm und keine Bibliothek des Pakets vermisst eine Bibliothek (ldd);
#   3. `humanitl --version` und `humanitld --version` nennen die Version;
#   4. `humanitl-app` startet unter Xvfb als gewoehnlicher Nutzer, laeuft
#      nach 20 s noch, zeigt sein Fenster "Humanitl" und meldet keinen
#      Startfehler (faengt Bibliotheken, die erst per dlopen kommen, und
#      Ausnahmen im Dart-Code);
#      die Desktop-Datei ist gueltig (`desktop-file-validate`);
#   5. nichts liegt unter /usr/local, /home, /root, /etc oder /var;
#   6. nach `apt-get remove --purge` ist keine Datei des Pakets mehr da, und
#      nirgends im Dateisystem steht noch etwas mit "humanitl" im Namen.
set -euo pipefail

[[ $# -eq 2 ]] || { echo "usage: $0 <deb> <0.0.N>" >&2; exit 2; }
deb="$(readlink -f "$1")"
version="$2"
[[ "$(id -u)" -eq 0 ]] || { echo "error: run as root inside a throwaway container" >&2; exit 1; }
[[ -f /.dockerenv || -f /run/.containerenv || "${HUMANITL_CHECK_IN_CONTAINER:-}" == 1 ]] \
  || { echo "error: this does not look like a container; refusing to install here" >&2; exit 1; }

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
# Das Werkzeug der Pruefung kommt vor der Bestandsaufnahme, damit es nicht als
# Rest des Pakets zaehlt.
apt-get install -y -qq --no-install-recommends desktop-file-utils file xvfb xauth x11-utils procps >/dev/null
# Die Anwendung startet nie als root; der Starttest laeuft als eigener Nutzer.
id smoke >/dev/null 2>&1 || useradd --create-home smoke

# /home/smoke bleibt aussen vor: Was die Anwendung beim Starttest in ihrem
# Heimatverzeichnis anlegt, sind Nutzerdaten, keine Dateien des Pakets.
snapshot() {
  find / -xdev \( -path /proc -o -path /sys -o -path /dev -o -path /tmp -o -path /run \
    -o -path /var/cache -o -path /var/lib/apt -o -path /var/lib/dpkg -o -path /var/log \
    -o -path /home/smoke \) -prune \
    -o -print 2>/dev/null | sort
}
snapshot >/tmp/before.txt

echo "== install"
apt-get install -y --no-install-recommends "$deb"

echo "== files of the package"
dpkg -L humanitl | sort >/tmp/owned.txt
wc -l </tmp/owned.txt
bad="$(grep -E '^/(usr/local|home|root|etc|var|opt)(/|$)' /tmp/owned.txt || true)"
if [[ -n "$bad" ]]; then
  echo "error: the package installs outside /usr:" >&2
  echo "$bad" >&2
  exit 1
fi

echo "== libraries"
missing=0
while IFS= read -r f; do
  [[ -f "$f" && ! -L "$f" ]] || continue
  if file -b "$f" | grep -q '^ELF'; then
    # Die Plugin-Bibliotheken des Bundles tragen keinen RUNPATH; sie finden
    # libflutter_linux_gtk.so, weil die Anwendung sie ueber ihren eigenen
    # RUNPATH ($ORIGIN/lib) schon geladen hat. ldd sieht sie einzeln, also
    # bekommt es denselben Suchpfad.
    if LD_LIBRARY_PATH=/usr/lib/humanitl/lib ldd "$f" 2>&1 | grep 'not found'; then
      echo "error: $f misses a library" >&2
      missing=1
    fi
  fi
done </tmp/owned.txt
[[ "$missing" -eq 0 ]] || exit 1
echo "every ELF file of the package finds its libraries"

echo "== versions"
for name in humanitl humanitld; do
  exe="$name"
  [[ "$name" == humanitld ]] && exe=/usr/lib/humanitl/bin/humanitld
  got="$("$exe" --version)"
  if [[ "$got" != "$name $version" ]]; then
    echo "error: $exe --version prints '$got', expected '$name $version'" >&2
    exit 1
  fi
  echo "ok: $got"
done
command -v humanitl-app >/dev/null || { echo "error: humanitl-app is not on PATH" >&2; exit 1; }

echo "== the app starts"
# ldd sieht nur gelinkte Bibliotheken. Was die Engine mit dlopen nachlaedt
# (libEGL, libGLESv2), faellt erst beim Start auf. Vier Bedingungen, alle
# muessen gelten:
#
# 1. `timeout` sitzt innerhalb von xvfb-run und umschliesst nur die
#    Anwendung. Exit 124 heisst also: die Anwendung selbst lief nach 20 s
#    noch. Haengt xvfb-run (Display, xauth, Xvfb), greift der aeussere
#    timeout 90, und dann fehlt der Exit-Code der Anwendung ganz.
# 2. Innerhalb von 15 s erscheint ein Fenster mit dem Titel "Humanitl",
#    waehrend die Anwendung laeuft. Gefragt wird einmal je Sekunde, weil ein
#    kalter Start mit llvmpipe auf einem vollen Runner laenger dauern kann
#    als ein fester Zeitpunkt vorsieht. Den Titel setzt der Dart-Code
#    (`configureWindow` in app/lib/main.dart); der native Runner nennt das
#    Fenster "humanitl". Das Fenster beweist, dass Engine und Dart-Code
#    wirklich laufen, und nicht nur die GTK-Schleife. Stirbt die Anwendung
#    nach dem Fenster, faellt das ueber Bedingung 1 auf.
# 3. Die Engine meldet ihr Rendering-Backend (Impeller).
# 4. Die Ausgabe enthaelt keinen fatalen Fehler. Eine Flutter-Anwendung mit
#    einer unbehandelten Ausnahme bleibt in der GTK-Schleife haengen und
#    wuerde sonst ebenfalls mit 124 enden. Harmlose Zeilen, die GTK, GIO und
#    AT-SPI in einem Container ohne D-Bus schreiben ("Failed to load module",
#    "Failed to connect to bus"), zaehlen nicht.
smoke_dir="$(mktemp -d)"
chown smoke "$smoke_dir"
set +e
# shellcheck disable=SC2016 # Das innere Skript expandiert selbst, mit $1.
runuser -u smoke -- timeout 90 xvfb-run -a -s '-screen 0 1600x1000x24' sh -c '
  timeout 20 humanitl-app >"$1/app.log" 2>&1 &
  app=$!
  # Lebt der Prozess wirklich? `kill -0` genuegt nicht: Ist `timeout`
  # beendet, bleibt es bis zum `wait` unten ein Zombie, und `kill -0` auf
  # einen Zombie gelingt. `ps` zeigt ihn mit Status Z.
  running() {
    case "$(ps -o stat= -p "$app" 2>/dev/null)" in
      "" | Z*) return 1 ;;
      *) return 0 ;;
    esac
  }
  # Eine Frist nach der Uhr, nicht nach Durchlaeufen: Unter Last dauert ein
  # Durchlauf mehr als eine Sekunde.
  start=$(date +%s)
  while :; do
    sleep 1
    elapsed=$(( $(date +%s) - start ))
    [ "$elapsed" -lt 15 ] || break
    running || break
    if xwininfo -root -tree 2>/dev/null | grep -qF "\"Humanitl\":"; then
      # Treffer nur, wenn er vor der Frist kam und die Anwendung in diesem
      # Moment noch lief.
      elapsed=$(( $(date +%s) - start ))
      if [ "$elapsed" -lt 15 ] && running; then
        echo "$elapsed" >"$1/window.seen"
      fi
      break
    fi
  done
  echo "$(( $(date +%s) - start ))" >"$1/poll.seconds"
  xwininfo -root -tree >"$1/windows.txt" 2>&1
  wait "$app"
  echo "$?" >"$1/app.rc"
' smoke "$smoke_dir" >"$smoke_dir/xvfb-run.log" 2>&1
outer=$?
set -e
app_rc="$(cat "$smoke_dir/app.rc" 2>/dev/null || echo none)"
echo "window polling ended after $(cat "$smoke_dir/poll.seconds" 2>/dev/null || echo '?') s"
echo "--- output of humanitl-app (last 15 lines)"
tail -n 15 "$smoke_dir/app.log" 2>/dev/null || true
echo "---"
start_failed=0
if [[ "$app_rc" != 124 ]]; then
  echo "error: humanitl-app did not run for 20 s (app exit: $app_rc, xvfb-run exit: $outer)" >&2
  cat "$smoke_dir/xvfb-run.log" >&2
  start_failed=1
fi
window_after="$(cat "$smoke_dir/window.seen" 2>/dev/null || true)"
if [[ -z "$window_after" ]]; then
  echo "error: no window titled \"Humanitl\" within 15 s while the app ran; the Dart code did not get as far as configureWindow" >&2
  grep -i humanitl "$smoke_dir/windows.txt" >&2 2>/dev/null || true
  start_failed=1
fi
# Die Engine meldet ihr Rendering-Backend auch im Release-Bau, gemessen
# 2026-09-18: "Using the Impeller rendering backend (OpenGLESSDF)."
if ! grep -q "Using the Impeller rendering backend" "$smoke_dir/app.log" 2>/dev/null; then
  echo "error: the engine never reported its rendering backend" >&2
  start_failed=1
fi
if grep -Ei "Unhandled exception|FlutterError|error while loading shared libraries|Couldn't open" "$smoke_dir/app.log" >&2; then
  echo "error: humanitl-app reported a startup error (lines above)" >&2
  start_failed=1
fi
[[ "$start_failed" -eq 0 ]] || exit 1
rm -rf "$smoke_dir"
echo "humanitl-app ran for 20 s under Xvfb (exit 124), showed its window \"Humanitl\" after ${window_after} s, Impeller started, no fatal error"

echo "== desktop entry"
desktop-file-validate /usr/share/applications/humanitl.desktop
echo "desktop-file-validate: ok"

echo "== unit"
grep -q '^ExecStart=/usr/lib/humanitl/bin/humanitld$' /usr/lib/systemd/user/humanitld.service \
  || { echo "error: the user unit does not start the packaged daemon" >&2; exit 1; }
echo "ExecStart points at the packaged daemon"

echo "== remove --purge"
apt-get remove --purge -y humanitl
apt-get autoremove --purge -y -qq >/dev/null
left=0
while IFS= read -r f; do
  # Verzeichnisse wie /usr/bin gehoeren auch anderen Paketen.
  if [[ -e "$f" || -L "$f" ]] && ! [[ -d "$f" && ! -L "$f" ]]; then
    echo "left behind: $f" >&2
    left=1
  fi
done </tmp/owned.txt
for dir in /usr/lib/humanitl /usr/share/humanitl /usr/share/doc/humanitl; do
  if [[ -e "$dir" ]]; then
    echo "left behind: $dir" >&2
    left=1
  fi
done
snapshot >/tmp/after.txt
stray="$(comm -13 /tmp/before.txt /tmp/after.txt | grep -i humanitl || true)"
if [[ -n "$stray" ]]; then
  echo "left behind:" >&2
  echo "$stray" >&2
  left=1
fi
[[ "$left" -eq 0 ]] || exit 1
echo "after purge: no file of the package and nothing named humanitl remains"
other="$(comm -13 /tmp/before.txt /tmp/after.txt | head -n 20 || true)"
if [[ -n "$other" ]]; then
  echo "note: paths new since before the install (dependency triggers, not the package):"
  echo "$other"
fi
echo "== check-install: passed"
