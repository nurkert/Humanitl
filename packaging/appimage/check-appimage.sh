#!/usr/bin/env bash
# Prueft ein gebautes AppImage, ohne etwas zu installieren (HUM-053).
#
# Aufruf: packaging/appimage/check-appimage.sh <AppImage> <0.0.N>
#
# Geprueft wird:
#   1. `--cli --version` erreicht die Kommandozeile im Bild und nennt die
#      Version; `--cli` und die Daemon-Version gehen durch `AppRun`;
#   2. im Bild liegt keine Bibliothek, die vom System kommen muss (GTK, GLib,
#      Wayland, EGL), und `AppRun` setzt kein `LD_LIBRARY_PATH`;
#   3. `--cli daemon install --print` erkennt das AppImage an `$APPIMAGE` und
#      plant `ExecStart` auf die Kopie unter `~/.local/lib/humanitl/current/`,
#      nie auf den Einhaengepunkt; geschrieben wird dabei nichts.
#
# Das Bild laeuft mit `--appimage-extract-and-run`: Der Test braucht kein FUSE,
# und `$APPIMAGE` setzt die Laufzeit in beiden Faellen. Das Heimatverzeichnis
# ist ein Wegwerf-Verzeichnis.
set -euo pipefail

[[ $# -eq 2 ]] || { echo "usage: $0 <AppImage> <0.0.N>" >&2; exit 2; }
image="$(readlink -f "$1")"
version="$2"
here="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=packaging/release/version.sh
source "$here/../release/version.sh"
require_prerelease_version "$version"
[[ -x "$image" ]] || { echo "error: $image is not an executable file" >&2; exit 1; }

home="$(mktemp -d)"
trap 'rm -rf "$home"' EXIT
mkdir -p "$home/tmp"
run() {
  env -i HOME="$home" PATH=/usr/bin:/bin TMPDIR="$home/tmp" \
    "$image" --appimage-extract-and-run "$@"
}

echo "== --cli reaches the command line"
got="$(run --cli --version)"
[[ "$got" == "humanitl $version" ]] \
  || { echo "error: --cli --version prints '$got', expected 'humanitl $version'" >&2; exit 1; }
echo "ok: $got"

echo "== no system library in the image"
extract="$home/extract"
mkdir -p "$extract"
(cd "$extract" && "$image" --appimage-extract >/dev/null)
root="$extract/squashfs-root"
[[ -x "$root/AppRun" ]] || { echo "error: the image has no AppRun" >&2; exit 1; }
forbidden="$(find "$root" \( -name 'libgtk*' -o -name 'libgdk*' -o -name 'libglib*' \
  -o -name 'libgio*' -o -name 'libwayland*' -o -name 'libEGL*' -o -name 'libGL*' \) -print)"
[[ -z "$forbidden" ]] || { echo "error: the image bundles system libraries:" >&2; echo "$forbidden" >&2; exit 1; }
if grep -q 'LD_LIBRARY_PATH=' "$root/AppRun"; then
  echo "error: AppRun sets LD_LIBRARY_PATH" >&2
  exit 1
fi
for f in humanitl bin/humanitld bin/humanitl bin/humanitl-shim share/humanitl/catalog/domains.yaml \
  profiles/sandbox/default.toml; do
  [[ -e "$root/usr/lib/humanitl/$f" ]] || { echo "error: usr/lib/humanitl/$f is missing" >&2; exit 1; }
done
# ldd endet bei einem statischen Programm mit 1; unter `pipefail` zaehlt deshalb
# nur sein Text.
linkage="$(ldd "$root/usr/lib/humanitl/bin/humanitl-shim" 2>&1 || true)"
[[ "$linkage" == *"not a dynamic executable"* ]] \
  || { echo "error: the shim in the image is not static: $linkage" >&2; exit 1; }
echo "only the Flutter bundle and the three programs; the shim is static"

echo "== daemon install recognises the AppImage"
plan="$(run --cli --json daemon install --print)"
echo "$plan"
PLAN="$plan" HOME_DIR="$home" python3 - <<'PY'
import json, os, sys
plan = json.loads(os.environ["PLAN"])
home = os.environ["HOME_DIR"]
want_exec = f"{home}/.local/lib/humanitl/current/humanitld"
if plan.get("exec_start") != want_exec:
    sys.exit(f"error: ExecStart is {plan.get('exec_start')!r}, expected {want_exec!r}")
if plan.get("action") != "print":
    sys.exit(f"error: action is {plan.get('action')!r}, expected 'print'")
PY
if [[ -e "$home/.config/systemd" || -e "$home/.local/lib/humanitl" ]]; then
  echo "error: --print wrote something below $home" >&2
  exit 1
fi
echo "ExecStart names the copy under ~/.local/lib/humanitl/current, and --print wrote nothing"
echo "== check-appimage: passed"
