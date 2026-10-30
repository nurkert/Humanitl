#!/usr/bin/env bash
# Baut das AUR-Paket humanitl-bin in einem Wegwerf-Container mit Arch Linux,
# installiert es, prueft es und entfernt es wieder (HUM-339). Nie auf einem
# echten Rechner ausfuehren: Das Skript installiert als root und legt Nutzer an.
#
# Aufruf (als root, im Container archlinux:base-devel):
#   check-aur.sh <render-dir> <tarball> <0.0.N> [<reference-deb>]
#
#   <render-dir>     Ausgabe von packaging/aur/render.sh fuer diese Version
#   <tarball>        das Release-Archiv humanitl-<version>-linux-x86_64.tar.gz,
#                    lokal gebaut; makepkg nimmt es statt des Downloads, wenn
#                    die Pruefsumme stimmt
#   <reference-deb>  wahlweise das .deb derselben Version; dann muss das
#                    Arch-Paket dieselben Pfade mit denselben Rechten liefern,
#                    bis auf die Dateien, die nur Debian kennt
#
# Geprueft wird, was ein Mensch mit einem AUR-Helfer oder `makepkg -si` erlebt:
#   1. .SRCINFO ist genau das, was `makepkg --printsrcinfo` sagt;
#   2. makepkg baut aus dem Archiv mit der Pruefsumme im PKGBUILD, und
#      .INSTALL im Paket ist das gerenderte Skript;
#   3. pacman loest die Abhaengigkeiten aus dem Archiv auf (sonst stimmt
#      depends nicht), namcap meldet danach keinen Fehler, `pacman -Qkk`
#      findet keine Abweichung;
#   4. nichts liegt unter /usr/local, /home, /root, /etc, /var oder /opt;
#   5. kein Programm und keine Bibliothek des Pakets vermisst eine Bibliothek,
#      die Versionen stimmen, `humanitl-app` startet unter Xvfb (wie in
#      packaging/deb/check-install.sh);
#   6. Desktop-Datei, Units und Symbole wie im .deb, und `humanitl daemon
#      install` erkennt die Units des Pakets, ohne eine Kopie zu schreiben;
#   7. nach `pacman -Rns` ist keine Datei des Pakets mehr da, und nirgends im
#      Dateisystem steht noch etwas mit "humanitl" im Namen.
set -euo pipefail

[[ $# -eq 3 || $# -eq 4 ]] || { echo "usage: $0 <render-dir> <tarball> <0.0.N> [<reference-deb>]" >&2; exit 2; }
render="$(readlink -f "$1")"
tarball="$(readlink -f "$2")"
version="$3"
reference_deb="${4:+$(readlink -f "$4")}"
[[ "$(id -u)" -eq 0 ]] || { echo "error: run as root inside a throwaway container" >&2; exit 1; }
[[ -f /.dockerenv || -f /run/.containerenv || "${HUMANITL_CHECK_IN_CONTAINER:-}" == 1 ]] \
  || { echo "error: this does not look like a container; refusing to install here" >&2; exit 1; }
[[ -f "$render/PKGBUILD" && -f "$render/.SRCINFO" && -f "$render/humanitl-bin.install" ]] \
  || { echo "error: $render holds no output of packaging/aur/render.sh" >&2; exit 1; }
tarball_name="humanitl-$version-linux-x86_64.tar.gz"
[[ "$(basename "$tarball")" == "$tarball_name" ]] \
  || { echo "error: $tarball is not named $tarball_name" >&2; exit 1; }

echo "== tools"
# Das Werkzeug der Pruefung kommt vor der Bestandsaufnahme, damit es nicht als
# Rest des Pakets zaehlt. Die Abhaengigkeiten des Pakets gehoeren nicht
# hierher: Die soll erst `pacman -U` aus depends aufloesen.
pacman -Syu --noconfirm --needed namcap desktop-file-utils file xorg-server-xvfb xorg-xauth \
  xorg-xwininfo procps-ng python >/dev/null
# makepkg baut nie als root, und die Anwendung startet nie als root.
id builder >/dev/null 2>&1 || useradd --create-home builder
id smoke >/dev/null 2>&1 || useradd --create-home smoke

echo "== .SRCINFO"
build=/home/builder/humanitl-bin
rm -rf "$build"
install -d -o builder -g builder "$build"
cp -a "$render/." "$build/"
cp "$tarball" "$build/$tarball_name"
chown -R builder:builder "$build"
if ! (cd "$build" && runuser -u builder -- makepkg --printsrcinfo) | diff -u "$build/.SRCINFO" -; then
  echo "error: .SRCINFO differs from makepkg --printsrcinfo (lines above); render.sh writes it wrong" >&2
  exit 1
fi
echo ".SRCINFO matches makepkg --printsrcinfo"

echo "== makepkg"
# --nodeps: Die Abhaengigkeiten loest erst pacman -U auf, und makepkg kann als
# builder ohnehin nichts installieren. Die Pruefsummen prueft makepkg selbst,
# auch die des Archivs, das hier statt des Downloads liegt.
(cd "$build" && runuser -u builder -- env PKGDEST="$build" makepkg --nodeps --noconfirm --cleanbuild)
pkg="$(find "$build" -maxdepth 1 -name "humanitl-bin-$version-1-x86_64.pkg.tar.*" ! -name '*.sig' | head -n 1)"
[[ -n "$pkg" ]] || { echo "error: makepkg built no humanitl-bin-$version-1-x86_64.pkg.tar.*" >&2; exit 1; }
echo "built $pkg"
if find "$build" -maxdepth 1 -name 'humanitl-bin-debug-*' | grep -q .; then
  echo "error: makepkg split off a -debug package; options must carry !debug" >&2
  exit 1
fi
bsdtar -xOf "$pkg" .INSTALL | cmp - "$render/humanitl-bin.install" \
  || { echo "error: .INSTALL in the package is not the rendered humanitl-bin.install" >&2; exit 1; }
bash -n "$render/humanitl-bin.install"
echo ".INSTALL is the rendered install script and parses"

# /home/builder und /home/smoke bleiben aussen vor: Dort liegen der Bau und
# was die Anwendung beim Starttest anlegt, keine Dateien des Pakets.
snapshot() {
  find / -xdev \( -path /proc -o -path /sys -o -path /dev -o -path /tmp -o -path /run \
    -o -path /var/cache -o -path /var/lib/pacman -o -path /var/log \
    -o -path /home/builder -o -path /home/smoke \) -prune \
    -o -print 2>/dev/null | sort
}
snapshot >/tmp/before.txt

echo "== install"
pacman -U --noconfirm "$pkg"

echo "== namcap"
# Erst nach `pacman -U`: Vorher fehlen die Abhaengigkeiten, und namcap haelt
# jede gelinkte Bibliothek fuer unaufgeloest. namcap meldet Fehler mit "E:"
# und Hinweise mit "W:" oder "I:". Ein Fehler faellt durch; die Hinweise
# stehen im Log, und README.md sagt, welche erwartet sind und warum.
namcap_out="$(namcap -i "$build/PKGBUILD" "$pkg" 2>&1 || true)"
echo "$namcap_out"
if grep -E '^[^ ]+ E: ' <<<"$namcap_out"; then
  echo "error: namcap reports errors (lines above)" >&2
  exit 1
fi

echo "== files of the package"
pacman -Qlq humanitl-bin | sed 's|/$||' | sort >/tmp/owned.txt
wc -l </tmp/owned.txt
bad="$(grep -E '^/(usr/local|home|root|etc|var|opt)(/|$)' /tmp/owned.txt || true)"
if [[ -n "$bad" ]]; then
  echo "error: the package installs outside /usr:" >&2
  echo "$bad" >&2
  exit 1
fi
pacman -Qkk humanitl-bin
for need in /usr/lib/humanitl/humanitl /usr/lib/humanitl/lib/libapp.so /usr/lib/humanitl/data/icudtl.dat \
  /usr/lib/humanitl/bin/humanitld /usr/lib/humanitl/bin/humanitl /usr/lib/humanitl/bin/humanitl-shim \
  /usr/share/humanitl/catalog/domains.yaml /usr/share/humanitl/profiles/sandbox/default.toml \
  /usr/share/licenses/humanitl-bin/LICENSE /usr/share/licenses/humanitl-bin/copyright; do
  [[ -f "$need" ]] || { echo "error: $need is missing" >&2; exit 1; }
done
[[ "$(readlink /usr/bin/humanitl)" == ../lib/humanitl/bin/humanitl ]] \
  || { echo "error: /usr/bin/humanitl does not link to ../lib/humanitl/bin/humanitl" >&2; exit 1; }
[[ "$(readlink /usr/bin/humanitl-app)" == ../lib/humanitl/humanitl ]] \
  || { echo "error: /usr/bin/humanitl-app does not link to ../lib/humanitl/humanitl" >&2; exit 1; }
# Rechte wie im .deb; die Anwendung verweigert eine Kommandozeile unter einem
# Verzeichnis, in das Gruppe oder alle schreiben duerfen.
wrong_modes="$(while IFS= read -r f; do
  [[ -L "$f" ]] && continue
  mode="$(stat -c '%a %U:%G' "$f")"
  case "$f" in
    /usr/lib/humanitl/humanitl | /usr/lib/humanitl/bin/*) want='755 root:root' ;;
    *) if [[ -d "$f" ]]; then want='755 root:root'; else want='644 root:root'; fi ;;
  esac
  [[ "$mode" == "$want" ]] || echo "$f: $mode, expected $want"
done </tmp/owned.txt)"
if [[ -n "$wrong_modes" ]]; then
  echo "error: modes or owners differ from the .deb:" >&2
  echo "$wrong_modes" >&2
  exit 1
fi
echo "layout, links and modes as in the .deb"

if [[ -n "$reference_deb" ]]; then
  echo "== same paths and modes as the .deb"
  # Debian-eigen sind changelog und copyright unter /usr/share/doc und die
  # lintian-Overrides; Arch legt Lizenz und copyright unter
  # /usr/share/licenses/humanitl-bin ab. Verzeichnisse, die auch anderen
  # Paketen gehoeren (etwa /usr/share/icons/hicolor), listet pacman mit.
  only_deb='^/usr/share/(doc|lintian)[/ ]'
  only_arch='^/usr/share/licenses[/ ]'
  bsdtar -xOf "$reference_deb" 'data.tar.*' | bsdtar -tvf - \
    | awk '{ mode=$1; path=$NF; if ($(NF-1) == "->") path=$(NF-2); sub(/^\.\//, "/", path); sub(/\/$/, "", path); if (path != "" && path != ".") print path, mode }' \
    | grep -Ev "$only_deb" | sort >/tmp/deb-modes.txt
  while IFS= read -r f; do
    printf '%s %s\n' "$f" "$(stat -c '%A' "$f")"
  done </tmp/owned.txt | grep -Ev "$only_arch" | sort >/tmp/arch-modes.txt
  # Nur Pfade, die beide Seiten kennen oder die das .deb bringt: Verzeichnisse
  # wie /usr oder /usr/share fuehrt pacman, das .deb aber auch; beide Listen
  # sind also vollstaendig, und jeder Unterschied ist einer.
  if ! diff -u /tmp/deb-modes.txt /tmp/arch-modes.txt; then
    echo "error: the Arch package differs from $reference_deb in paths or modes (lines above)" >&2
    exit 1
  fi
  echo "every path of the .deb, with its mode, except /usr/share/doc and lintian"
fi

echo "== libraries"
missing=0
while IFS= read -r f; do
  [[ -f "$f" && ! -L "$f" ]] || continue
  if file -b "$f" | grep -q '^ELF'; then
    # Wie in check-install.sh: Die Plugin-Bibliotheken finden
    # libflutter_linux_gtk.so ueber den RUNPATH der Anwendung; ldd sieht sie
    # einzeln und bekommt deshalb denselben Suchpfad.
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
# Derselbe Starttest wie in packaging/deb/check-install.sh, dort begruendet:
# Exit 124 der Anwendung nach 20 s, ihr Fenster "Humanitl" innerhalb von
# 15 s, Impeller gemeldet, kein fataler Fehler in der Ausgabe.
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
[[ -f /usr/share/icons/hicolor/scalable/apps/humanitl.svg ]] \
  || { echo "error: the SVG icon is missing" >&2; exit 1; }
echo "PNG icons at 64, 128 and 256 pixels, plus the SVG under scalable"

echo "== daemon install finds the units of the package"
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
pacman -Rns --noconfirm humanitl-bin
left=0
while IFS= read -r f; do
  # Verzeichnisse wie /usr/bin gehoeren auch anderen Paketen.
  if [[ -e "$f" || -L "$f" ]] && ! [[ -d "$f" && ! -L "$f" ]]; then
    echo "left behind: $f" >&2
    left=1
  fi
done </tmp/owned.txt
for dir in /usr/lib/humanitl /usr/share/humanitl /usr/share/licenses/humanitl-bin; do
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
echo "after pacman -Rns: no file of the package and nothing named humanitl remains"
other="$(comm -13 /tmp/before.txt /tmp/after.txt | head -n 20 || true)"
if [[ -n "$other" ]]; then
  echo "note: paths new since before the install (hooks and caches, not the package):"
  echo "$other"
fi
echo "== check-aur: passed"
