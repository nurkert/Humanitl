#!/usr/bin/env bash
# Prueft die Scriptlets `post` und `postun` des RPM-Pakets mit Attrappen
# (HUM-339).
#
# Aufruf: packaging/rpm/scriptlets-test.sh
#
# Was der eingesetzte Teil (`packaging/deb/maintscript-users.sh`) im Einzelnen
# tut, prueft `packaging/deb/maintscripts-test.sh` an den Skripten des .deb.
# Hier geht es um das, was rpm anders macht: die Argumente. rpm ruft `%post`
# mit 1 (Installation) oder 2 (Update) und `%postun` mit 0 (entfernt) oder 1
# (Update, nach dem `%post` der neuen Fassung). Erwartet:
#   - `post 1` und `post 2` laden neu und starten einen Daemon ohne Sitzung
#     neu;
#   - `postun 0` laedt nur neu und fragt keinen Daemon;
#   - `postun 1` tut nichts;
#   - jedes Scriptlet endet mit 0, auch wenn ein Schritt scheitert, und ohne
#     systemd geschieht nichts.
# Die Attrappen liegen vorn im `PATH`; kein Fall erreicht den systemd, die
# Nutzer oder den Daemon des Rechners.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

scripts="$work/scripts"
"$here/scriptlets.sh" "$scripts"

stubs="$work/stubs"
state="$work/state"
systemd_dir="$work/run-systemd-system"
mkdir -p "$stubs" "$systemd_dir" "$work/tmp"

# --- Attrappen ---------------------------------------------------------------

cat >"$stubs/loginctl" <<'EOF'
#!/bin/sh
echo "loginctl $*" >>"$STUB_STATE/log"
[ "$1 $2" = "list-users --no-legend" ] || exit 1
echo "1000 alice"
EOF

cat >"$stubs/systemctl" <<'EOF'
#!/bin/sh
echo "system: systemctl $*" >>"$STUB_STATE/log"
[ "$*" = "is-active --quiet user@1000.service" ]
EOF

cat >"$stubs/getent" <<'EOF'
#!/bin/sh
[ "$*" = "passwd 1000" ] || exit 2
echo "alice:x:1000:1000::/home/alice:/bin/sh"
EOF

cat >"$stubs/logger" <<'EOF'
#!/bin/sh
echo "logger $*" >>"$STUB_STATE/log"
EOF

cat >"$stubs/systemd-detect-virt" <<'EOF'
#!/bin/sh
exit 1
EOF

cat >"$stubs/setsid" <<'EOF'
#!/bin/sh
exec "$@"
EOF

# `setpriv` ueberspringt Optionen und `env -i` mit seinen Zuweisungen und
# reicht den Rest an `user-<name>` weiter.
cat >"$stubs/setpriv" <<'EOF'
#!/bin/sh
while [ $# -gt 0 ]; do
  case "$1" in
    --*) shift ;;
    *) break ;;
  esac
done
[ "$1" = env ] && [ "$2" = -i ] || exit 97
shift 2
while [ $# -gt 0 ]; do
  case "$1" in
    *=*) shift ;;
    *) break ;;
  esac
done
name=$(basename "$1")
shift
echo "as alice: $name $*" >>"$STUB_STATE/log"
exec "$STUB_DIR/user-$name" "$@"
EOF

cat >"$stubs/user-systemctl" <<'EOF'
#!/bin/sh
[ "$1" = --user ] || exit 98
shift
case "$1" in
  daemon-reload) [ ! -e "$STUB_STATE/reload-fails" ] ;;
  is-active) true ;;
  show) echo /usr/lib/systemd/user/humanitld.service ;;
  restart) [ ! -e "$STUB_STATE/restart-fails" ] ;;
  *) exit 99 ;;
esac
EOF

cat >"$stubs/user-humanitl" <<'EOF'
#!/bin/sh
[ "$*" = "--json daemon retire-if-idle" ] || exit 98
echo '{"outcome":"retired"}'
EOF
chmod 0755 "$stubs"/*

# --- Werkzeug ----------------------------------------------------------------

failures=0
fail() {
  echo "FAIL: $*" >&2
  failures=$((failures + 1))
}

reset_state() {
  rm -rf "$state"
  mkdir -p "$state"
  : >"$state/log"
}

# run <script> <args...>: faehrt das Scriptlet so, wie rpm es tut
# (`/bin/sh <datei> <zahl>`), mit den Attrappen; Exit in $rc.
run() {
  local script="$1"
  shift
  rc=0
  env -i PATH="$stubs:/usr/bin:/bin" STUB_STATE="$state" STUB_DIR="$stubs" HUMANITL_OUT_DIR="$work/tmp" \
    HUMANITL_SYSTEMD_DIR="${SYSTEMD_DIR:-$systemd_dir}" \
    /bin/sh "$scripts/$script" "$@" </dev/null >"$state/out" 2>"$state/err" || rc=$?
}

has() {
  grep -qF -- "$1" "$state/log" || fail "$2: the log lacks '$1'"
}

lacks() {
  if grep -qF -- "$1" "$state/log"; then
    fail "$2: the log must not contain '$1'"
  fi
}

# --- Faelle ------------------------------------------------------------------

for arg in 1 2; do
  reset_state
  run post "$arg"
  [[ "$rc" -eq 0 ]] || fail "post $arg exits with $rc: $(cat "$state/err")"
  has "as alice: systemctl --user daemon-reload" "post $arg reloads"
  has "as alice: humanitl --json daemon retire-if-idle" "post $arg asks the daemon to retire"
  has "as alice: systemctl --user restart humanitld.service" "post $arg restarts the retired daemon"
done

reset_state
run postun 0
[[ "$rc" -eq 0 ]] || fail "postun 0 exits with $rc: $(cat "$state/err")"
has "as alice: systemctl --user daemon-reload" "postun 0 reloads"
lacks "restart" "postun 0 restarts nothing"
lacks "humanitl --json" "postun 0 asks no daemon (the command line is gone)"

reset_state
run postun 1
[[ "$rc" -eq 0 ]] || fail "postun 1 exits with $rc"
lacks "systemctl" "postun 1 (upgrade) leaves the reload to the post of the new version"
lacks "loginctl" "postun 1 looks nobody up"

# Ein scheiterndes daemon-reload oder ein scheiternder Neustart laesst kein
# Scriptlet scheitern.
reset_state
: >"$state/reload-fails"
run post 1
[[ "$rc" -eq 0 ]] || fail "post with a failing daemon-reload exits with $rc"
grep -qF "daemon-reload failed for alice" "$state/err" || fail "a failed daemon-reload is noted"
reset_state
: >"$state/restart-fails"
run post 2
[[ "$rc" -eq 0 ]] || fail "post with a failing restart exits with $rc"
run postun 0
[[ "$rc" -eq 0 ]] || fail "postun with a failing step exits with $rc"

# Ohne systemd (Container, chroot, Bau-Wurzel) geschieht nichts.
reset_state
SYSTEMD_DIR="$work/no-such-dir" run post 1
[[ "$rc" -eq 0 ]] || fail "post without systemd exits with $rc"
lacks "loginctl" "without systemd nobody is looked up"
SYSTEMD_DIR="$work/no-such-dir" run postun 0
[[ "$rc" -eq 0 ]] || fail "postun without systemd exits with $rc"
lacks "loginctl" "without systemd nobody is looked up"

leftover=$(find "$work/tmp" -type f | wc -l)
[[ "$leftover" -eq 0 ]] || fail "the scriptlets left $leftover temporary file(s) behind"

# --- shellcheck --------------------------------------------------------------

if command -v shellcheck >/dev/null 2>&1; then
  shellcheck -s sh "$scripts/post" "$scripts/postun" || fail "shellcheck found problems"
  # Vom Wurzelverzeichnis aus, damit `-x` dem `source` von version.sh folgt.
  (cd "$here/../.." && shellcheck -x packaging/rpm/scriptlets.sh packaging/rpm/build-rpm.sh \
    packaging/rpm/check-install.sh packaging/rpm/rpmlint.sh packaging/rpm/scriptlets-test.sh) \
    || fail "shellcheck found problems in the tooling"
else
  echo "note: shellcheck is not installed; skipped (CI runs it)"
fi

if [[ "$failures" -ne 0 ]]; then
  echo "scriptlets-test: $failures failure(s)" >&2
  exit 1
fi
echo "scriptlets-test: all cases passed"
