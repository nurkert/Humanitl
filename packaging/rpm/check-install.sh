#!/usr/bin/env bash
# Installiert das RPM in einem Wegwerf-Container mit Fedora, prueft es und
# entfernt es wieder (HUM-339). Nie auf einem echten Rechner ausfuehren: Das
# Skript installiert als root und entfernt danach.
#
# Aufruf (als root, im Container): check-install.sh <rpm> <0.0.N>
#
# Das Gegenstueck zu `packaging/deb/check-install.sh`, mit denselben
# Pruefungen in derselben Reihenfolge. Geprueft wird, was ein Mensch mit
# `sudo dnf install ./humanitl-…rpm` erlebt:
#   1. dnf loest die Abhaengigkeiten aus den Fedora-Quellen auf, ohne schwache
#      Abhaengigkeiten (sonst stimmen Requires oder die Untergrenze von glibc
#      nicht), und kein Scriptlet scheitert;
#   2. `rpm -V` findet jede Datei so, wie das Paket sie beschreibt, und kein
#      Programm und keine Bibliothek des Pakets vermisst eine Bibliothek (ldd);
#   3. `humanitl --version` und `humanitld --version` nennen die Version;
#   4. `humanitl-app` startet unter Xvfb als gewoehnlicher Nutzer, laeuft
#      nach 20 s noch, zeigt sein Fenster "Humanitl" und meldet keinen
#      Startfehler; die Desktop-Datei ist gueltig (`desktop-file-validate`);
#   5. nichts liegt unter /usr/local, /home, /root, /etc, /var, /opt oder
#      /usr/lib64;
#   6. Dienst- und Socket-Unit liegen unter /usr/lib/systemd/user, ohne die
#      Marke von `daemon install`, die PNG-Symbole haben ihre Groesse, und
#      `humanitl daemon install` erkennt das Paket: Es plant nur den Socket
#      des Pakets, ohne eine Kopie unter ~/.config zu schreiben (HUM-053);
#   7. nach `dnf remove` ist das Paket fort, keine seiner Dateien mehr da, und
#      nirgends im Dateisystem steht noch etwas mit "humanitl" im Namen.
#
# Der Starttest der Anwendung (Punkt 4) ist derselbe wie im .deb; er steht
# hier als Kopie, solange `packaging/deb/check-install.sh` ihn nicht als
# eigenes Skript anbietet.
set -euo pipefail

[[ $# -eq 2 ]] || { echo "usage: $0 <rpm> <0.0.N>" >&2; exit 2; }
rpm_file="$(readlink -f "$1")"
version="$2"
[[ "$(id -u)" -eq 0 ]] || { echo "error: run as root inside a throwaway container" >&2; exit 1; }
[[ -f /.dockerenv || -f /run/.containerenv || "${HUMANITL_CHECK_IN_CONTAINER:-}" == 1 ]] \
  || { echo "error: this does not look like a container; refusing to install here" >&2; exit 1; }
command -v dnf >/dev/null || { echo "error: this check needs dnf (Fedora)" >&2; exit 1; }
grep -E '^(NAME|VERSION_ID)=' /etc/os-release || true

# Das Werkzeug der Pruefung kommt vor der Bestandsaufnahme, damit es nicht als
# Rest des Pakets zaehlt. Schwache Abhaengigkeiten bleiben auch hier aus, damit
# das Werkzeug nicht nebenbei mitbringt, was das Paket selbst verlangen muss.
dnf install -y -q --setopt=install_weak_deps=False \
  desktop-file-utils file xorg-x11-server-Xvfb xorg-x11-xauth xwininfo procps-ng \
  python3 util-linux shadow-utils coreutils findutils grep
# Die Anwendung startet nie als root; der Starttest laeuft als eigener Nutzer.
id smoke >/dev/null 2>&1 || useradd --create-home smoke

# /home/smoke bleibt aussen vor: Was die Anwendung beim Starttest in ihrem
# Heimatverzeichnis anlegt, sind Nutzerdaten, keine Dateien des Pakets.
# /usr/lib/sysimage traegt die Datenbanken von rpm und dnf.
snapshot() {
  find / -xdev \( -path /proc -o -path /sys -o -path /dev -o -path /tmp -o -path /run \
    -o -path /var/cache -o -path /var/lib/dnf -o -path /var/lib/rpm -o -path /usr/lib/sysimage \
    -o -path /var/log -o -path /home/smoke \) -prune \
    -o -print 2>/dev/null | sort
}
snapshot >/tmp/before.txt

echo "== install"
# dnf prueft ein lokales Paket ohne Signatur nicht gegen einen Schluessel
# (localpkg_gpgcheck ist aus); das Paket ist wie das .deb nicht signiert.
dnf install -y --setopt=install_weak_deps=False "$rpm_file" 2>&1 | tee /tmp/install.log
if grep -Ei 'scriptlet.*fail|error: %post|warning: %post' /tmp/install.log; then
  echo "error: a scriptlet of the package failed during install" >&2
  exit 1
fi
installed="$(rpm -q --qf '%{VERSION}-%{RELEASE}' humanitl)"
[[ "$installed" == "$version-1" ]] \
  || { echo "error: installed humanitl is $installed, expected $version-1" >&2; exit 1; }

echo "== files of the package"
rpm -ql humanitl | sort >/tmp/owned.txt
wc -l </tmp/owned.txt
bad="$(grep -E '^/(usr/local|usr/lib64|home|root|etc|var|opt)(/|$)' /tmp/owned.txt || true)"
if [[ -n "$bad" ]]; then
  echo "error: the package installs outside its places below /usr:" >&2
  echo "$bad" >&2
  exit 1
fi
rpm -V humanitl || { echo "error: rpm -V found files that differ from the package" >&2; exit 1; }
echo "rpm -V: every file as the package describes it"

echo "== libraries"
missing=0
while IFS= read -r f; do
  [[ -f "$f" && ! -L "$f" ]] || continue
  if file -b "$f" | grep -q '^ELF'; then
    # Die Plugin-Bibliotheken des Bundles finden libflutter_linux_gtk.so,
    # weil die Anwendung sie ueber ihren RUNPATH ($ORIGIN/lib) schon geladen
    # hat. ldd sieht sie einzeln, also bekommt es denselben Suchpfad.
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
# Dieselben vier Bedingungen wie im .deb, mit derselben Begruendung
# (`packaging/deb/check-install.sh`): Exit 124 der Anwendung selbst, ein
# Fenster "Humanitl" binnen 15 s, waehrend sie laeuft, die Meldung des
# Rendering-Backends und kein fataler Fehler in der Ausgabe. Was die Engine
# mit dlopen nachlaedt (libEGL, libGLESv2), faellt erst hier auf.
smoke_dir="$(mktemp -d)"
chown smoke "$smoke_dir"
set +e
# shellcheck disable=SC2016 # Das innere Skript expandiert selbst, mit $1.
runuser -u smoke -- timeout 90 xvfb-run -a -s '-screen 0 1600x1000x24' sh -c '
  timeout 20 humanitl-app >"$1/app.log" 2>&1 &
  app=$!
  running() {
    case "$(ps -o stat= -p "$app" 2>/dev/null)" in
      "" | Z*) return 1 ;;
      *) return 0 ;;
    esac
  }
  start=$(date +%s)
  while :; do
    sleep 1
    elapsed=$(( $(date +%s) - start ))
    [ "$elapsed" -lt 15 ] || break
    running || break
    if xwininfo -root -tree 2>/dev/null | grep -qF "\"Humanitl\":"; then
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
desktop-file-validate /usr/share/applications/dev.humanitl.humanitl.desktop
echo "desktop-file-validate: ok"

echo "== units"
grep -q '^ExecStart=/usr/lib/humanitl/bin/humanitld$' /usr/lib/systemd/user/humanitld.service \
  || { echo "error: the user unit does not start the packaged daemon" >&2; exit 1; }
echo "ExecStart points at the packaged daemon"
grep -q '^Type=notify$' /usr/lib/systemd/user/humanitld.service \
  || { echo "error: the user unit is not Type=notify" >&2; exit 1; }
grep -q '^ListenStream=%t/humanitl/daemon.sock$' /usr/lib/systemd/user/humanitld.socket \
  || { echo "error: the socket unit does not listen where the client looks" >&2; exit 1; }
for name in humanitld.service humanitld.socket; do
  if [[ "$(head -n 1 "/usr/lib/systemd/user/$name")" == "# humanitl daemon install: written by Humanitl" ]]; then
    echo "error: /usr/lib/systemd/user/$name carries the marker of daemon install; the package owns it" >&2
    exit 1
  fi
done
echo "both units are in place, Type=notify, ListenStream at %t/humanitl/daemon.sock, no marker"

echo "== icons"
for size in 64 128 256; do
  png="/usr/share/icons/hicolor/${size}x${size}/apps/humanitl.png"
  file -b "$png" | grep -q "^PNG image data, $size x $size," \
    || { echo "error: $png is not a ${size}x${size} PNG: $(file -b "$png" 2>&1)" >&2; exit 1; }
done
echo "PNG icons at 64, 128 and 256 pixels, plus the SVG under scalable"

echo "== daemon install finds the units of the package"
# Als gewoehnlicher Nutzer, mit --print: Es darf nichts geschrieben werden,
# und der Plan nennt nur den Socket des Pakets (HUM-164). Ohne systemd im
# Container gibt es kein systemctl; die Namen stehen trotzdem in `units`.
plan="$(runuser -u smoke -- env -i HOME=/home/smoke PATH=/usr/bin:/bin humanitl --json daemon install --print)"
echo "$plan"
PLAN="$plan" python3 - <<'PY'
import json, os, sys
plan = json.loads(os.environ["PLAN"])
want = {
    "action": "print",
    "unit": "/usr/lib/systemd/user/humanitld.service",
    "exec_start": "/usr/lib/humanitl/bin/humanitld",
    "units": ["humanitld.socket"],
}
bad = {key: (plan.get(key), value) for key, value in want.items() if plan.get(key) != value}
if bad:
    sys.exit(f"error: daemon install --print planned something else: {bad}")
PY
if [[ -e /home/smoke/.config/systemd ]]; then
  echo "error: daemon install --print wrote below /home/smoke/.config/systemd" >&2
  exit 1
fi
runuser -u smoke -- env -i HOME=/home/smoke PATH=/usr/bin:/bin humanitl daemon install --no-start
if [[ -e /home/smoke/.config/systemd ]]; then
  echo "error: daemon install wrote a user unit although the package brings its own" >&2
  exit 1
fi
echo "daemon install plans the socket of the package and writes no copy"

echo "== remove"
# dnf entfernt dabei auch die Abhaengigkeiten, die nur das Paket brauchte
# (clean_requirements_on_remove), wie `apt-get autoremove` beim .deb.
dnf remove -y humanitl 2>&1 | tee /tmp/remove.log
if grep -Ei 'scriptlet.*fail|error: %postun|warning: %postun' /tmp/remove.log; then
  echo "error: a scriptlet of the package failed during remove" >&2
  exit 1
fi
if rpm -q humanitl >/dev/null 2>&1; then
  echo "error: humanitl is still installed after dnf remove" >&2
  exit 1
fi
left=0
while IFS= read -r f; do
  # Verzeichnisse wie /usr/bin gehoeren auch anderen Paketen.
  if [[ -e "$f" || -L "$f" ]] && ! [[ -d "$f" && ! -L "$f" ]]; then
    echo "left behind: $f" >&2
    left=1
  fi
done </tmp/owned.txt
for dir in /usr/lib/humanitl /usr/share/humanitl /usr/share/licenses/humanitl; do
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
echo "after remove: no file of the package and nothing named humanitl remains"
other="$(comm -13 /tmp/before.txt /tmp/after.txt | head -n 20 || true)"
if [[ -n "$other" ]]; then
  echo "note: paths new since before the install (dependency triggers, not the package):"
  echo "$other"
fi
echo "== check-install: passed"
